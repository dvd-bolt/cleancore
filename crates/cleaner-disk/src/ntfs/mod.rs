//! NTFS MFT index. Only metadata is read; this index grants no mutation authority.
mod record;
#[cfg(test)]
mod tests;
mod treemap;
#[cfg(windows)]
mod volume;

use crate::{
    analyzer::AnalyzerOptions,
    models::{
        CategorySummary, DiskAnalysisReport, DiskEntry, FileCategory, LargeFileCategory,
        LargeFileEntry,
    },
};
use record::{note, Diagnostics, Name, Record};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug)]
struct Node {
    reference: u64,
    parent: u64,
    name: String,
    directory: bool,
    logical: u64,
    allocated: u64,
    files: u64,
    dirs: u64,
    children: Vec<usize>,
    reachable: bool,
}
pub struct MftSnapshot {
    drive: String,
    nodes: Vec<Node>,
    root: usize,
    diagnostics: Diagnostics,
    warnings: Diagnostics,
    elapsed_ms: u64,
    volume_used: u64,
    volume_serial: Option<u64>,
    index: HashMap<u64, usize>,
}

/// Accept only an ordinary drive root. UNC/device paths never reach raw-volume access.
pub fn drive_letter(root: &Path) -> Option<u8> {
    let s = root.to_str()?.as_bytes();
    if s.len() == 3 && s[0].is_ascii_alphabetic() && s[1] == b':' && matches!(s[2], b'\\' | b'/') {
        Some(s[0].to_ascii_uppercase())
    } else {
        None
    }
}

impl MftSnapshot {
    fn build(
        drive: String,
        mut records: HashMap<u64, Record>,
        mut diagnostics: Diagnostics,
        mut warnings: Diagnostics,
        cancel: &AtomicBool,
    ) -> Result<Self, String> {
        // Extension records are metadata of their base object, never additional files.
        let extensions: Vec<_> = records
            .iter()
            .filter(|(_, r)| r.base != 0)
            .map(|(&id, _)| id)
            .collect();
        for id in extensions {
            let extension = records.remove(&id).unwrap();
            if let Some(base) = records.get_mut(&extension.base) {
                base.names.extend(extension.names);
                if extension.has_data {
                    if base.has_data {
                        note(&mut diagnostics, "MFT_DUPLICATE_DATA");
                    } else {
                        base.logical = extension.logical;
                        base.allocated = extension.allocated;
                        base.has_data = true;
                    }
                }
                base.extra_streams |= extension.extra_streams;
                base.index_root |= extension.index_root;
            } else {
                note(&mut diagnostics, "MFT_STALE_EXTENSION");
            }
        }
        let mut nodes = Vec::new();
        nodes
            .try_reserve(records.len())
            .map_err(|_| "Недостаточно памяти для индекса MFT")?;
        let mut index = HashMap::new();
        index
            .try_reserve(records.len())
            .map_err(|_| "Недостаточно памяти для связей MFT")?;
        let valid_parents: HashSet<_> = records
            .values()
            .filter(|r| r.directory)
            .map(|r| r.reference)
            .collect();
        for (_, record) in records {
            let mut names = record.names;
            let valid_names: Vec<_> = names
                .iter()
                .filter(|n| valid_parents.contains(&n.parent))
                .cloned()
                .collect();
            if !valid_names.is_empty() {
                names = valid_names;
            }
            if record.attribute_list {
                note(&mut warnings, "EXTENDED_ATTRIBUTES");
            }
            if !record.directory
                && !record.has_data
                && !record.extra_streams
                && !record.index_root
                && record.reference & record::ID_MASK >= 16
            {
                note(&mut diagnostics, "MFT_DATA_UNAVAILABLE");
            }
            // A physical object is counted once. Prefer a long Win32 name; DOS aliases are not files.
            names.sort_unstable_by(|a, b| {
                let rank = |n: &Name| if n.namespace == 2 { 1 } else { 0 };
                (rank(a), a.parent, &a.name).cmp(&(rank(b), b.parent, &b.name))
            });
            names.dedup_by(|a, b| a.parent == b.parent && a.name == b.name);
            if names.iter().filter(|n| n.namespace != 2).count() > 1 {
                note(&mut warnings, "HARD_LINK_COUNTED_ONCE");
            }
            if record.extra_streams {
                note(&mut warnings, "NAMED_STREAMS_EXCLUDED");
            }
            let Some(name) = names.into_iter().next() else {
                // Slots 0..15 include reserved internal records without pathname attributes.
                if record.reference & record::ID_MASK < 16 {
                    note(&mut warnings, "SYSTEM_RECORD_WITHOUT_NAME");
                } else {
                    note(&mut diagnostics, "MFT_NAME_UNAVAILABLE");
                }
                continue;
            };
            index.insert(record.reference, nodes.len());
            nodes.push(Node {
                reference: record.reference,
                parent: name.parent,
                name: name.name,
                directory: record.directory,
                logical: if record.directory { 0 } else { record.logical },
                allocated: if record.directory {
                    0
                } else {
                    record.allocated
                },
                files: u64::from(!record.directory),
                dirs: 0,
                children: Vec::new(),
                reachable: false,
            });
        }
        let root = nodes
            .iter()
            .position(|n| n.reference & record::ID_MASK == 5 && n.directory)
            .ok_or("Корневая запись MFT не прочитана")?;
        // Sort the attachment order for reproducible results regardless of HashMap iteration.
        for child in 0..nodes.len() {
            if child == root {
                continue;
            }
            if let Some(&parent) = index.get(&nodes[child].parent) {
                if nodes[parent].directory && parent != child {
                    nodes[parent].children.push(child);
                } else {
                    note(&mut diagnostics, "MFT_INVALID_PARENT");
                }
            } else {
                note(&mut diagnostics, "MFT_STALE_PARENT");
            }
        }
        // Iterative traversal and reverse aggregation: O(records), no recursion or per-file ancestor walk.
        let mut order = Vec::new();
        order
            .try_reserve(nodes.len())
            .map_err(|_| "Недостаточно памяти для дерева MFT")?;
        let mut queue = VecDeque::from([root]);
        while let Some(id) = queue.pop_front() {
            if nodes[id].reachable {
                note(&mut diagnostics, "MFT_PARENT_CYCLE");
                continue;
            }
            nodes[id].reachable = true;
            order.push(id);
            queue.extend(nodes[id].children.iter().copied());
        }
        let disconnected = nodes.iter().filter(|n| !n.reachable).count();
        if disconnected > 0 {
            diagnostics.insert("MFT_UNREACHABLE".into(), disconnected as u64);
        }
        for &id in order.iter().rev() {
            if id == root {
                continue;
            }
            if let Some(&parent) = index.get(&nodes[id].parent) {
                let (logical, allocated, files, dirs) = (
                    nodes[id].logical,
                    nodes[id].allocated,
                    nodes[id].files,
                    nodes[id].dirs + u64::from(nodes[id].directory),
                );
                nodes[parent].logical = nodes[parent].logical.saturating_add(logical);
                nodes[parent].allocated = nodes[parent].allocated.saturating_add(allocated);
                nodes[parent].files += files;
                nodes[parent].dirs += dirs;
            }
        }
        if cancel.load(Ordering::Acquire) {
            diagnostics.insert("CANCELLED".into(), 1);
        }
        Ok(Self {
            drive,
            nodes,
            root,
            diagnostics,
            warnings,
            elapsed_ms: 0,
            volume_used: 0,
            volume_serial: None,
            index,
        })
    }
    fn path(&self, mut id: usize, parents: &HashMap<u64, usize>) -> Option<String> {
        let mut parts = Vec::new();
        while id != self.root {
            if parts.len() >= 4096 {
                return None;
            }
            let node = &self.nodes[id];
            parts.push(node.name.as_str());
            id = *parents.get(&node.parent)?;
        }
        parts.reverse();
        Some(format!("{}{}", self.drive, parts.join("\\")))
    }
    /// Folder navigation uses the same immutable scan; it never touches the volume again.
    pub fn report(
        &self,
        root: &Path,
        options: &AnalyzerOptions,
        cancel: &AtomicBool,
    ) -> Option<DiskAnalysisReport> {
        let requested = root.to_str()?.replace('/', "\\");
        let normalized = requested.trim_end_matches('\\');
        let drive_root = self.drive.trim_end_matches('\\');
        if !normalized.eq_ignore_ascii_case(drive_root)
            && !normalized
                .get(..3)
                .is_some_and(|s| s.eq_ignore_ascii_case(&self.drive))
        {
            return None;
        }
        let mut selected = self.root;
        if !normalized.eq_ignore_ascii_case(drive_root) {
            for name in normalized.get(3..)?.split('\\') {
                selected = *self.nodes[selected].children.iter().find(|&&id| {
                    self.nodes[id].directory && self.nodes[id].name.eq_ignore_ascii_case(name)
                })?;
            }
        }
        let root_node = &self.nodes[selected];
        let mut report = DiskAnalysisReport {
            root_path: requested,
            total_bytes: root_node.logical,
            total_files: root_node.files,
            total_dirs: root_node.dirs,
            categories: Vec::new(),
            largest_files: Vec::new(),
            largest_dirs: Vec::new(),
            subdirs: Vec::new(),
            complete: self.diagnostics.is_empty(),
            diagnostics: self.diagnostics.clone(),
            engine: "ntfs-mft".into(),
            elapsed_ms: self.elapsed_ms,
            allocated_bytes: Some(root_node.allocated),
            engine_note: None,
            volume_used_bytes: Some(self.volume_used),
            warnings: self.warnings.clone(),
        };
        let percentage = |n: u64| {
            if root_node.logical > 0 {
                (n as f64 / root_node.logical as f64 * 100.) as f32
            } else {
                0.
            }
        };
        let entry = |id: usize| -> Option<DiskEntry> {
            let node = &self.nodes[id];
            Some(DiskEntry {
                path: self.path(id, &self.index)?,
                name: node.name.clone(),
                size_bytes: node.logical,
                is_dir: node.directory,
                file_count: node.files,
                percentage: percentage(node.logical),
                category: FileCategory::from_extension(
                    Path::new(&node.name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or(""),
                ),
            })
        };
        for &id in &root_node.children {
            if self.nodes[id].directory {
                if let Some(e) = entry(id) {
                    report.subdirs.push(e);
                } else {
                    note(&mut report.diagnostics, "MFT_PATH_TOO_DEEP");
                }
            }
        }
        report
            .subdirs
            .sort_unstable_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then(a.path.cmp(&b.path)));
        report.largest_dirs = report
            .subdirs
            .iter()
            .take(options.max_largest_items.min(100))
            .cloned()
            .collect();
        let mut categories: HashMap<FileCategory, (u64, u64)> = HashMap::new();
        let mut largest = Vec::new();
        let mut pending = vec![selected];
        while let Some(id) = pending.pop() {
            // Cancelled scans still retain their bounded top-file/category summary.
            // A new cancellation while browsing is handled between task operations.
            let node = &self.nodes[id];
            if node.directory {
                pending.extend(node.children.iter().copied());
            } else {
                let cat = FileCategory::from_extension(
                    Path::new(&node.name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or(""),
                );
                let stat = categories.entry(cat).or_default();
                stat.0 = stat.0.saturating_add(node.logical);
                stat.1 += 1;
                largest.push(id);
                largest.sort_unstable_by(|&a, &b| {
                    self.nodes[b]
                        .logical
                        .cmp(&self.nodes[a].logical)
                        .then(self.nodes[a].reference.cmp(&self.nodes[b].reference))
                });
                largest.truncate(options.max_largest_items.min(100));
            }
        }
        if cancel.load(Ordering::Acquire) {
            report.diagnostics.insert("CANCELLED".into(), 1);
        }
        report.largest_files = largest.into_iter().filter_map(entry).collect();
        report.categories = categories
            .into_iter()
            .map(|(category, (bytes, count))| CategorySummary {
                category,
                display_name: category.display_name().into(),
                total_bytes: bytes,
                file_count: count,
                percentage: percentage(bytes),
            })
            .collect();
        report
            .categories
            .sort_unstable_by_key(|c| std::cmp::Reverse(c.total_bytes));
        report.complete = report.diagnostics.is_empty();
        Some(report)
    }

    /// Выбирает самые крупные файлы из снимка MFT (OPT-02).
    pub fn space_hogs(
        &self,
        min_size_bytes: u64,
        limit: usize,
        category_filter: Option<LargeFileCategory>,
    ) -> Vec<LargeFileEntry> {
        let max_items = limit.clamp(1, 1000);
        let mut candidates = Vec::new();

        for (idx, node) in self.nodes.iter().enumerate() {
            if node.directory || !node.reachable || node.logical < min_size_bytes {
                continue;
            }

            let ext = Path::new(&node.name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let category = LargeFileCategory::classify_extension(ext);

            if let Some(filter) = category_filter {
                if category != filter {
                    continue;
                }
            }

            candidates.push((idx, node.logical));
        }

        candidates.sort_unstable_by_key(|b| std::cmp::Reverse(b.1));
        candidates.truncate(max_items);

        candidates
            .into_iter()
            .filter_map(|(id, _)| {
                let node = &self.nodes[id];
                let full_path = self.path(id, &self.index)?;
                let name_lower = node.name.to_ascii_lowercase();

                let is_system_locked = matches!(
                    name_lower.as_str(),
                    "pagefile.sys" | "swapfile.sys" | "hiberfil.sys" | "dumpstack.log"
                ) || full_path.contains("System Volume Information")
                    || full_path.contains("$Recycle.Bin");

                let ext = Path::new(&node.name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                Some(LargeFileEntry {
                    id: format!("mft-file-{}", node.reference),
                    path: full_path,
                    size_bytes: node.logical,
                    allocated_bytes: node.allocated,
                    category: LargeFileCategory::classify_extension(&ext),
                    extension: ext,
                    is_system_locked,
                })
            })
            .collect()
    }
}

#[cfg(windows)]
pub fn scan<F: Fn(u64, u64)>(
    letter: u8,
    cancel: Arc<AtomicBool>,
    progress: &F,
) -> Result<Arc<MftSnapshot>, String> {
    let started = std::time::Instant::now();
    let volume = volume::Volume::open(letter)?;
    let mut records = HashMap::new();
    // Keep a valid root even if the user cancels before the first bulk read.
    let mut root_record = vec![0; volume.record_size];
    volume.stream(
        &volume.runs,
        5 * volume.record_size as u64,
        &mut root_record,
    )?;
    let root =
        record::parse_record(&mut root_record, 5)?.ok_or("Корневая запись NTFS недоступна")?;
    records.insert(root.reference, root);
    let mut diagnostics = Diagnostics::new();
    let warnings = Diagnostics::new();
    let mut buffer = vec![0; 4 * 1024 * 1024];
    let mut offset = 0;
    let mut estimated_memory = 0usize;
    let mut files = 0;
    let mut bytes = 0u64;
    while offset < volume.mft_length {
        if cancel.load(Ordering::Acquire) {
            note(&mut diagnostics, "CANCELLED");
            break;
        }
        let length = buffer.len().min((volume.mft_length - offset) as usize);
        if let Err(e) = volume.stream(&volume.runs, offset, &mut buffer[..length]) {
            if records.is_empty() {
                return Err(e);
            }
            note(&mut diagnostics, "MFT_READ_FAILED");
            break;
        }
        for (i, chunk) in buffer[..length]
            .chunks_exact_mut(volume.record_size)
            .enumerate()
        {
            if i.is_multiple_of(256) && cancel.load(Ordering::Acquire) {
                note(&mut diagnostics, "CANCELLED");
                break;
            }
            let id = offset / volume.record_size as u64 + i as u64;
            match record::parse_record(chunk, id) {
                Ok(Some(record)) => {
                    estimated_memory += 256
                        + record
                            .names
                            .iter()
                            .map(|n| n.name.len() + 64)
                            .sum::<usize>();
                    if estimated_memory > 1024 * 1024 * 1024 || records.len() >= 4_000_000 {
                        note(&mut diagnostics, "MFT_MEMORY_LIMIT");
                        break;
                    }
                    if records.try_reserve(1).is_err() {
                        note(&mut diagnostics, "MFT_MEMORY_LIMIT");
                        break;
                    }
                    if record.base == 0 && !record.directory {
                        files += 1;
                        bytes = bytes.saturating_add(record.logical);
                    }
                    records.insert(record.reference, record);
                }
                Ok(None) => {}
                Err(_) => note(&mut diagnostics, "MFT_INVALID_RECORD"),
            }
        }
        progress(files, bytes);
        if diagnostics.contains_key("MFT_MEMORY_LIMIT") || diagnostics.contains_key("CANCELLED") {
            break;
        }
        offset += length as u64;
    }
    let mut snapshot = MftSnapshot::build(
        format!("{}:\\", letter as char),
        records,
        diagnostics,
        warnings,
        &cancel,
    )?;
    snapshot.elapsed_ms = started.elapsed().as_millis() as u64;
    snapshot.volume_used = volume.used_bytes;
    snapshot.volume_serial = Some(volume.serial);
    Ok(Arc::new(snapshot))
}

#[cfg(not(windows))]
pub fn scan<F: Fn(u64, u64)>(
    _letter: u8,
    _cancel: Arc<AtomicBool>,
    _progress: &F,
) -> Result<Arc<MftSnapshot>, String> {
    Err("NTFS MFT доступен только на Windows".into())
}
