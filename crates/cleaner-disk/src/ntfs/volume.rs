//! Read-only raw volume access. The caller can select a drive letter, never a device path.
use super::record::{attributes, fixup, u16_at, u32_at, u64_at, Run, ID_MASK};
use std::{
    fs::File,
    os::windows::{
        fs::{FileExt, OpenOptionsExt},
        io::AsRawHandle,
    },
};
use windows_sys::Win32::System::{
    Ioctl::{FSCTL_GET_NTFS_VOLUME_DATA, NTFS_VOLUME_DATA_BUFFER},
    IO::DeviceIoControl,
};

pub struct Volume {
    file: File,
    pub sector: u64,
    pub cluster: u64,
    pub record_size: usize,
    pub mft_length: u64,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub serial: u64,
    pub runs: Vec<Run>,
}

impl Volume {
    pub fn open(letter: u8) -> Result<Self, String> {
        if !letter.is_ascii_uppercase() {
            return Err("Invalid drive letter".into());
        }
        let file = std::fs::OpenOptions::new().read(true).share_mode(7)
            .open(format!("\\\\.\\{}:", letter as char))
            .map_err(|e| format!("Не удалось открыть том {}: только для чтения: {e}. Для MFT нужны права администратора.", letter as char))?;
        let mut data: NTFS_VOLUME_DATA_BUFFER = unsafe { std::mem::zeroed() };
        let mut returned = 0;
        let ok = unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                FSCTL_GET_NTFS_VOLUME_DATA,
                std::ptr::null(),
                0,
                (&mut data as *mut NTFS_VOLUME_DATA_BUFFER).cast(),
                std::mem::size_of_val(&data) as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(format!(
                "Том недоступен для NTFS MFT: {}",
                std::io::Error::last_os_error()
            ));
        }
        if returned < std::mem::size_of_val(&data) as u32
            || data.TotalClusters <= 0
            || data.FreeClusters < 0
            || data.FreeClusters > data.TotalClusters
            || ![512, 1024, 2048, 4096].contains(&data.BytesPerSector)
            || !data.BytesPerCluster.is_power_of_two()
            || data.BytesPerCluster < data.BytesPerSector
            || data.BytesPerCluster > 2 * 1024 * 1024
            || !data.BytesPerFileRecordSegment.is_power_of_two()
            || !(512..=65536).contains(&data.BytesPerFileRecordSegment)
            || data.MftValidDataLength <= 0
            || data.MftStartLcn < 0
        {
            return Err("Неподдерживаемая геометрия NTFS".into());
        }
        let cluster = data.BytesPerCluster as u64;
        let total_bytes = (data.TotalClusters as u64)
            .checked_mul(cluster)
            .ok_or("NTFS volume size overflow")?;
        if data.MftValidDataLength as u64 > total_bytes {
            return Err("$MFT length exceeds volume".into());
        }
        let mut volume = Self {
            file,
            sector: data.BytesPerSector as u64,
            cluster,
            record_size: data.BytesPerFileRecordSegment as usize,
            mft_length: data.MftValidDataLength as u64,
            total_bytes,
            used_bytes: ((data.TotalClusters - data.FreeClusters) as u64) * cluster,
            serial: data.VolumeSerialNumber as u64,
            runs: Vec::new(),
        };
        // Check the boot sector version before interpreting NTFS 3.x structures.
        let mut boot = vec![0; volume.sector as usize];
        volume.physical(0, &mut boot)?;
        if &boot[3..11] != b"NTFS    " {
            return Err("Том не является NTFS".into());
        }
        let mut record = vec![0; volume.record_size];
        volume.physical(
            (data.MftStartLcn as u64)
                .checked_mul(cluster)
                .ok_or("NTFS offset overflow")?,
            &mut record,
        )?;
        fixup(&mut record)?;
        if u64_at(&record, 32)? != 0 || u16_at(&record, 22)? & 1 == 0 {
            return Err("Invalid $MFT base record".into());
        }
        let attrs = attributes(&record)?;
        let data_attr = attrs
            .iter()
            .find(|a| {
                a.kind == 0x80 && a.unnamed() && a.nonresident() && u64_at(a.bytes, 16) == Ok(0)
            })
            .ok_or("$MFT unnamed DATA is unavailable")?;
        volume.runs = data_attr.runs()?;
        volume.validate_runs()?;
        // Attribute lists describe fragmented $MFT runs in extension records.
        if let Some(list) = attrs.iter().find(|a| a.kind == 0x20) {
            let list_bytes = if list.nonresident() {
                let length = u64_at(list.bytes, 48)?;
                if length > 16 * 1024 * 1024 {
                    return Err("$MFT attribute list exceeds 16 MiB".into());
                }
                let mut value = vec![0; length as usize];
                volume.stream(&list.runs()?, 0, &mut value)?;
                value
            } else {
                list.value()?.to_vec()
            };
            let mut entries = Vec::new();
            let mut offset = 0;
            while offset + 4 <= list_bytes.len() {
                let kind = u32_at(&list_bytes, offset)?;
                if kind == u32::MAX || kind == 0 {
                    break;
                }
                let length = u16_at(&list_bytes, offset + 4)? as usize;
                if length < 26 || offset + length > list_bytes.len() {
                    return Err("Invalid $MFT attribute list entry".into());
                }
                if kind == 0x80 && list_bytes[offset + 6] == 0 {
                    let vcn = u64_at(&list_bytes, offset + 8)?;
                    let reference = u64_at(&list_bytes, offset + 16)?;
                    let instance = u16_at(&list_bytes, offset + 24)?;
                    if vcn != 0 {
                        entries.push((vcn, reference, instance));
                    }
                }
                offset += length;
            }
            entries.sort_unstable();
            entries.dedup();
            for (vcn, reference, instance) in entries {
                let mut extension = vec![0; volume.record_size];
                volume.stream(
                    &volume.runs,
                    (reference & ID_MASK)
                        .checked_mul(volume.record_size as u64)
                        .ok_or("$MFT offset overflow")?,
                    &mut extension,
                )?;
                fixup(&mut extension)?;
                if u16_at(&extension, 16)? != (reference >> 48) as u16
                    || u64_at(&extension, 32)? != (u16_at(&record, 16)? as u64) << 48
                {
                    return Err("Stale $MFT extension reference".into());
                }
                let attrs = attributes(&extension)?;
                let attr = attrs
                    .iter()
                    .find(|a| {
                        a.kind == 0x80
                            && a.unnamed()
                            && a.nonresident()
                            && u64_at(a.bytes, 16) == Ok(vcn)
                            && u16_at(a.bytes, 14) == Ok(instance)
                    })
                    .ok_or("Missing $MFT extension DATA")?;
                volume.runs.extend(attr.runs()?);
                volume.runs.sort_by_key(|r| r.vcn);
                volume.validate_runs()?;
            }
        }
        let covered = volume
            .runs
            .last()
            .and_then(|r| r.vcn.checked_add(r.clusters))
            .and_then(|v| v.checked_mul(cluster))
            .ok_or("Empty $MFT runlist")?;
        if covered < volume.mft_length
            || !volume.mft_length.is_multiple_of(volume.record_size as u64)
        {
            return Err("$MFT mapping does not cover valid data".into());
        }
        // The $Volume information attribute supplies the actual NTFS version.
        let mut version_record = vec![0; volume.record_size];
        volume.stream(
            &volume.runs,
            3 * volume.record_size as u64,
            &mut version_record,
        )?;
        fixup(&mut version_record)?;
        let version_attrs = attributes(&version_record)?;
        let version = version_attrs
            .iter()
            .find(|a| a.kind == 0x70)
            .ok_or("Missing NTFS volume version")?
            .value()?;
        if version.get(8) != Some(&3) || !matches!(version.get(9), Some(0 | 1)) {
            return Err("Поддерживается NTFS 3.0/3.1".into());
        }
        Ok(volume)
    }
    fn validate_runs(&self) -> Result<(), String> {
        let mut next = 0;
        for r in &self.runs {
            let lcn = r.lcn.ok_or("Sparse $MFT is unsupported")?;
            if r.vcn != next
                || lcn
                    .checked_add(r.clusters)
                    .and_then(|v| v.checked_mul(self.cluster))
                    .is_none_or(|v| v > self.total_bytes)
            {
                return Err("Invalid $MFT extent bounds".into());
            }
            next = next
                .checked_add(r.clusters)
                .ok_or("$MFT mapping overflow")?;
        }
        Ok(())
    }
    fn physical(&self, offset: u64, out: &mut [u8]) -> Result<(), String> {
        if offset
            .checked_add(out.len() as u64)
            .is_none_or(|v| v > self.total_bytes)
        {
            return Err("NTFS read outside volume".into());
        }
        // Raw volume reads must respect the sector alignment, including 4Kn disks.
        let start = offset / self.sector * self.sector;
        let end = (offset + out.len() as u64).div_ceil(self.sector) * self.sector;
        let mut buffer = vec![0; (end - start) as usize];
        let read = self
            .file
            .seek_read(&mut buffer, start)
            .map_err(|e| format!("Ошибка чтения NTFS: {e}"))?;
        if read != buffer.len() {
            return Err("Short raw NTFS read".into());
        }
        let from = (offset - start) as usize;
        out.copy_from_slice(&buffer[from..from + out.len()]);
        Ok(())
    }
    pub fn stream(&self, runs: &[Run], mut offset: u64, mut out: &mut [u8]) -> Result<(), String> {
        while !out.is_empty() {
            let vcn = offset / self.cluster;
            let run = runs
                .iter()
                .find(|r| vcn >= r.vcn && vcn - r.vcn < r.clusters)
                .ok_or("$MFT extent is not mapped")?;
            let start = run
                .vcn
                .checked_mul(self.cluster)
                .ok_or("NTFS stream offset overflow")?;
            let inside = offset
                .checked_sub(start)
                .ok_or("Invalid NTFS stream offset")?;
            let run_bytes = run
                .clusters
                .checked_mul(self.cluster)
                .ok_or("NTFS stream length overflow")?;
            let remaining = run_bytes
                .checked_sub(inside)
                .ok_or("Invalid NTFS extent size")?;
            let length = out.len().min(remaining as usize);
            let (head, tail) = out.split_at_mut(length);
            if let Some(lcn) = run.lcn {
                let physical = lcn
                    .checked_mul(self.cluster)
                    .and_then(|v| v.checked_add(inside))
                    .ok_or("NTFS physical offset overflow")?;
                self.physical(physical, head)?;
            } else {
                head.fill(0);
            }
            offset += length as u64;
            out = tail;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_rejects_untrusted_mapping_overflow_before_io() {
        let file = File::open(std::env::current_exe().unwrap()).unwrap();
        let volume = Volume {
            file,
            sector: 512,
            cluster: 4096,
            record_size: 1024,
            mft_length: 1024,
            total_bytes: 4096,
            used_bytes: 0,
            serial: 0,
            runs: vec![],
        };
        let mut out = [0; 8];
        assert!(volume
            .stream(
                &[Run {
                    vcn: 0,
                    clusters: u64::MAX,
                    lcn: Some(1)
                }],
                0,
                &mut out
            )
            .is_err());
        assert!(volume
            .stream(
                &[Run {
                    vcn: 0,
                    clusters: 1,
                    lcn: Some(u64::MAX)
                }],
                0,
                &mut out
            )
            .is_err());
        assert!(volume
            .stream(
                &[Run {
                    vcn: 1,
                    clusters: 1,
                    lcn: Some(0)
                }],
                0,
                &mut out
            )
            .is_err());
    }
}
