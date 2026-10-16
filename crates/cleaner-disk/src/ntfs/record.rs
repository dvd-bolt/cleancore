//! Bounds-checked NTFS 3.x metadata parser. No disk writes or pathname actions.
use std::collections::BTreeMap;

pub type Diagnostics = BTreeMap<String, u64>;
pub fn note(d: &mut Diagnostics, code: &str) {
    *d.entry(code.into()).or_default() += 1;
}
pub fn u16_at(b: &[u8], o: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        b.get(o..o + 2)
            .ok_or("Truncated NTFS u16")?
            .try_into()
            .unwrap(),
    ))
}
pub fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.get(o..o + 4)
            .ok_or("Truncated NTFS u32")?
            .try_into()
            .unwrap(),
    ))
}
pub fn u64_at(b: &[u8], o: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        b.get(o..o + 8)
            .ok_or("Truncated NTFS u64")?
            .try_into()
            .unwrap(),
    ))
}
pub const ID_MASK: u64 = (1 << 48) - 1;

/// The fixup stride is 512 bytes, including on 4K-sector volumes.
pub fn fixup(b: &mut [u8]) -> Result<(), String> {
    if b.get(..4) != Some(b"FILE") || b.len() < 512 || !b.len().is_multiple_of(512) {
        return Err("Invalid NTFS FILE signature/length".into());
    }
    let offset = u16_at(b, 4)? as usize;
    let words = u16_at(b, 6)? as usize;
    if words != b.len() / 512 + 1 || offset < 8 || offset + words * 2 > 510 {
        return Err("Invalid NTFS fixup array".into());
    }
    let tag = u16_at(b, offset)?;
    // Validate the entire transfer before changing any bytes.
    for i in 1..words {
        if u16_at(b, i * 512 - 2)? != tag {
            return Err("Torn NTFS record transfer".into());
        }
    }
    for i in 1..words {
        let saved = u16_at(b, offset + i * 2)?.to_le_bytes();
        b[i * 512 - 2..i * 512].copy_from_slice(&saved);
    }
    Ok(())
}

pub struct Attribute<'a> {
    pub kind: u32,
    pub bytes: &'a [u8],
}
impl Attribute<'_> {
    pub fn nonresident(&self) -> bool {
        self.bytes[8] == 1
    }
    pub fn unnamed(&self) -> bool {
        self.bytes[9] == 0
    }
    pub fn value(&self) -> Result<&[u8], String> {
        if self.nonresident() {
            return Err("Expected resident NTFS attribute".into());
        }
        let offset = u16_at(self.bytes, 20)? as usize;
        let length = u32_at(self.bytes, 16)? as usize;
        if offset < 24 {
            return Err("Invalid NTFS resident offset".into());
        }
        self.bytes
            .get(offset..offset.checked_add(length).ok_or("NTFS value overflow")?)
            .ok_or_else(|| "Truncated NTFS resident value".into())
    }
    pub fn runs(&self) -> Result<Vec<Run>, String> {
        if !self.nonresident() {
            return Err("Expected nonresident NTFS attribute".into());
        }
        let offset = u16_at(self.bytes, 32)? as usize;
        if offset < 64 {
            return Err("Invalid NTFS run offset".into());
        }
        parse_runs(
            self.bytes.get(offset..).ok_or("Invalid NTFS runs")?,
            u64_at(self.bytes, 16)?,
        )
    }
}
pub fn attributes(b: &[u8]) -> Result<Vec<Attribute<'_>>, String> {
    let used = u32_at(b, 24)? as usize;
    let mut offset = u16_at(b, 20)? as usize;
    if used > b.len() || offset < 42 || !offset.is_multiple_of(8) {
        return Err("Invalid NTFS attribute bounds".into());
    }
    let mut result = Vec::new();
    while offset + 4 <= used {
        let kind = u32_at(b, offset)?;
        if kind == u32::MAX {
            return Ok(result);
        }
        let length = u32_at(b, offset + 4)? as usize;
        let end = offset
            .checked_add(length)
            .ok_or("NTFS attribute overflow")?;
        if length < 24 || !length.is_multiple_of(8) || end > used {
            return Err("Invalid NTFS attribute length".into());
        }
        let bytes = &b[offset..end];
        if bytes[8] > 1 || (bytes[8] == 1 && length < 64) {
            return Err("Invalid NTFS attribute form".into());
        }
        if bytes[9] > 0 {
            let name = u16_at(bytes, 10)? as usize;
            if name < 16 || name + bytes[9] as usize * 2 > length {
                return Err("Invalid NTFS attribute name".into());
            }
        }
        result.push(Attribute { kind, bytes });
        offset = end;
    }
    Err("Missing NTFS attribute terminator".into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub vcn: u64,
    pub clusters: u64,
    pub lcn: Option<u64>,
}
pub fn parse_runs(b: &[u8], mut vcn: u64) -> Result<Vec<Run>, String> {
    let mut p = 0;
    let mut lcn = 0i64;
    let mut result = Vec::new();
    while let Some(&header) = b.get(p) {
        p += 1;
        if header == 0 {
            return Ok(result);
        }
        let n = (header & 15) as usize;
        let d = (header >> 4) as usize;
        if n == 0 || n > 8 || d > 8 || p + n + d > b.len() {
            return Err("Invalid NTFS mapping pair".into());
        }
        let mut length = [0; 8];
        length[..n].copy_from_slice(&b[p..p + n]);
        p += n;
        let clusters = u64::from_le_bytes(length);
        if clusters == 0 {
            return Err("Empty NTFS data run".into());
        }
        let physical = if d == 0 {
            None
        } else {
            let mut delta = if b[p + d - 1] & 128 != 0 {
                [255; 8]
            } else {
                [0; 8]
            };
            delta[..d].copy_from_slice(&b[p..p + d]);
            lcn = lcn
                .checked_add(i64::from_le_bytes(delta))
                .ok_or("NTFS LCN overflow")?;
            Some(u64::try_from(lcn).map_err(|_| "Negative NTFS LCN")?)
        };
        p += d;
        result.push(Run {
            vcn,
            clusters,
            lcn: physical,
        });
        vcn = vcn.checked_add(clusters).ok_or("NTFS VCN overflow")?;
    }
    Err("Unterminated NTFS mapping pairs".into())
}

#[derive(Debug, Clone)]
pub struct Name {
    pub parent: u64,
    pub name: String,
    pub namespace: u8,
}
#[derive(Debug, Clone)]
pub struct Record {
    pub reference: u64,
    pub base: u64,
    pub directory: bool,
    pub names: Vec<Name>,
    pub logical: u64,
    pub allocated: u64,
    pub has_data: bool,
    pub attribute_list: bool,
    pub extra_streams: bool,
    pub index_root: bool,
}
pub fn parse_record(b: &mut [u8], id: u64) -> Result<Option<Record>, String> {
    // Unused records may contain zeroes or old attributes; do not parse them.
    if b.iter().all(|&v| v == 0) {
        return Ok(None);
    }
    if b.get(..4) == Some(b"FILE") && u16_at(b, 22)? & 1 == 0 {
        return Ok(None);
    }
    fixup(b)?;
    let flags = u16_at(b, 22)?;
    if flags & 1 == 0 {
        return Ok(None);
    }
    let mut record = Record {
        reference: id | ((u16_at(b, 16)? as u64) << 48),
        base: u64_at(b, 32)?,
        directory: flags & 2 != 0,
        names: Vec::new(),
        logical: 0,
        allocated: 0,
        has_data: false,
        attribute_list: false,
        extra_streams: false,
        index_root: false,
    };
    for attr in attributes(b)? {
        match attr.kind {
            0x90 => record.index_root = true,
            0x20 => record.attribute_list = true,
            0x30 => {
                let v = attr.value()?;
                let len = *v.get(64).ok_or("Truncated NTFS file name")? as usize;
                let namespace = *v.get(65).ok_or("Truncated NTFS name namespace")?;
                let bytes = v.get(66..66 + len * 2).ok_or("Truncated NTFS name")?;
                let words: Vec<_> = bytes
                    .chunks_exact(2)
                    .map(|p| u16::from_le_bytes([p[0], p[1]]))
                    .collect();
                let name = String::from_utf16(&words).map_err(|_| "Invalid NTFS UTF-16 name")?;
                if name.is_empty() || name.contains(['\\', '/', '\0', ':']) || name == ".." {
                    return Err("Invalid NTFS path component".into());
                }
                if namespace > 3 {
                    return Err("Unknown NTFS name namespace".into());
                }
                record.names.push(Name {
                    parent: u64_at(v, 0)?,
                    name,
                    namespace,
                });
            }
            0x80 if !attr.unnamed() => record.extra_streams = true,
            0x80 => {
                if attr.nonresident() {
                    if u64_at(attr.bytes, 16)? == 0 {
                        record.logical = u64_at(attr.bytes, 48)?;
                        // Compressed/sparse allocation lives in TotalAllocated, not AllocatedLength.
                        record.allocated = if u16_at(attr.bytes, 12)? & 0x80ff != 0 {
                            u64_at(attr.bytes, 64)?
                        } else {
                            u64_at(attr.bytes, 40)?
                        };
                        record.has_data = true;
                    }
                } else {
                    record.logical = attr.value()?.len() as u64;
                    record.allocated = 0; // Resident data occupies the MFT, not a separate cluster.
                    record.has_data = true;
                }
            }
            _ => {}
        }
    }
    Ok(Some(record))
}
