//! Streaming, bounded, read-only directory inspection with partial reports.
use crate::models::{CategorySummary, DiskAnalysisReport, DiskEntry, FileCategory};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Select the MFT engine for complete drive roots; folder scans use the directory walker.
/// A failed MFT bootstrap is visible in the fallback report, never disguised as MFT success.
pub fn analyze_indexed<F: Fn(u64, u64)>(
    root: &Path,
    options: &AnalyzerOptions,
    cancel: Arc<AtomicBool>,
    progress: F,
) -> Result<(DiskAnalysisReport, Option<Arc<crate::ntfs::MftSnapshot>>), String> {
    let mut fallback = None;
    if let Some(letter) = crate::ntfs::drive_letter(root) {
        match crate::ntfs::scan(letter, cancel.clone(), &progress) {
            Ok(snapshot) => {
                let report = snapshot
                    .report(root, options, &cancel)
                    .ok_or("Не удалось построить отчёт MFT")?;
                return Ok((report, Some(snapshot)));
            }
            Err(error) => fallback = Some(error),
        }
    }
    let mut report = analyze_directory_tree(root, options, cancel, progress)?;
    report.engine_note = fallback;
    Ok((report, None))
}
#[derive(Debug, Clone)]
pub struct AnalyzerOptions {
    pub max_depth: usize,
    pub max_largest_items: usize,
}
impl Default for AnalyzerOptions {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_largest_items: 20,
        }
    }
}
const MAX_ENTRIES: usize = 1_000_000;
const MAX_QUEUED_DIRS: usize = 50_000;
fn linked(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
pub fn analyze_directory_tree<F>(
    root: &Path,
    options: &AnalyzerOptions,
    cancel: Arc<AtomicBool>,
    progress: F,
) -> Result<DiskAnalysisReport, String>
where
    F: Fn(u64, u64),
{
    let started = std::time::Instant::now();
    let _root_lease =
        cleaner_execution::native::lock_directory(root, Path::new("")).map_err(str::to_string)?;
    let mut report = DiskAnalysisReport {
        root_path: root.to_string_lossy().into_owned(),
        total_bytes: 0,
        total_files: 0,
        total_dirs: 0,
        categories: vec![],
        largest_files: vec![],
        largest_dirs: vec![],
        subdirs: vec![],
        complete: true,
        diagnostics: BTreeMap::new(),
        engine: "directory".into(),
        engine_note: None,
        elapsed_ms: 0,
        allocated_bytes: None,
        volume_used_bytes: None,
        warnings: BTreeMap::new(),
    };
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut direct: HashMap<PathBuf, (u64, u64)> = HashMap::new();
    let mut categories: HashMap<FileCategory, (u64, u64)> = HashMap::new();
    let mut visited = 0;
    while let Some((dir, depth)) = pending.pop() {
        if cancel.load(Ordering::Acquire) {
            *report.diagnostics.entry("CANCELLED".into()).or_default() += 1;
            break;
        }
        if depth > options.max_depth {
            *report.diagnostics.entry("DEPTH_LIMIT".into()).or_default() += 1;
            continue;
        }
        let lease = cleaner_execution::native::lock_directory(
            root,
            dir.strip_prefix(root).map_err(|e| e.to_string())?,
        );
        let _lease = match lease {
            Ok(v) => v,
            Err(code) => {
                *report.diagnostics.entry(code.into()).or_default() += 1;
                continue;
            }
        };
        let entries = match fs::read_dir(&dir) {
            Ok(v) => v,
            Err(e) => {
                if depth == 0 {
                    return Err(e.to_string());
                }
                *report
                    .diagnostics
                    .entry("ACCESS_OR_READ_DENIED".into())
                    .or_default() += 1;
                continue;
            }
        };
        for item in entries {
            if cancel.load(Ordering::Acquire) {
                *report.diagnostics.entry("CANCELLED".into()).or_default() += 1;
                break;
            }
            if visited >= MAX_ENTRIES {
                *report.diagnostics.entry("ENTRY_LIMIT".into()).or_default() += 1;
                pending.clear();
                break;
            }
            visited += 1;
            let item = match item {
                Ok(v) => v,
                Err(_) => {
                    *report
                        .diagnostics
                        .entry("READ_ENTRY_FAILED".into())
                        .or_default() += 1;
                    continue;
                }
            };
            let path = item.path();
            let meta = match fs::symlink_metadata(&path) {
                Ok(v) => v,
                Err(_) => {
                    *report
                        .diagnostics
                        .entry("METADATA_UNAVAILABLE".into())
                        .or_default() += 1;
                    continue;
                }
            };
            if linked(&meta) {
                *report.diagnostics.entry("LINK_SKIPPED".into()).or_default() += 1;
                continue;
            }
            if meta.is_dir() {
                report.total_dirs += 1;
                if depth == 0 {
                    direct.insert(path.clone(), (0, 0));
                }
                if pending.len() < MAX_QUEUED_DIRS {
                    pending.push((path, depth + 1));
                } else {
                    *report.diagnostics.entry("QUEUE_LIMIT".into()).or_default() += 1;
                }
            } else if meta.is_file() {
                let size = meta.len();
                report.total_files += 1;
                report.total_bytes = report.total_bytes.saturating_add(size);
                let category = FileCategory::from_extension(
                    &path
                        .extension()
                        .map(|e| e.to_string_lossy())
                        .unwrap_or_default(),
                );
                let stat = categories.entry(category).or_default();
                stat.0 = stat.0.saturating_add(size);
                stat.1 += 1;
                if let Some(first) = path
                    .strip_prefix(root)
                    .ok()
                    .and_then(|r| r.components().next())
                {
                    if let Some(stat) = direct.get_mut(&root.join(first)) {
                        stat.0 = stat.0.saturating_add(size);
                        stat.1 += 1;
                    }
                }
                report.largest_files.push(DiskEntry {
                    name: item.file_name().to_string_lossy().into_owned(),
                    path: path.to_string_lossy().into_owned(),
                    size_bytes: size,
                    is_dir: false,
                    file_count: 1,
                    percentage: 0.,
                    category,
                });
                report
                    .largest_files
                    .sort_by_key(|f| std::cmp::Reverse(f.size_bytes));
                report
                    .largest_files
                    .truncate(options.max_largest_items.min(100));
                if report.total_files.is_multiple_of(256) {
                    progress(report.total_files, report.total_bytes);
                }
            }
        }
        if visited >= MAX_ENTRIES || cancel.load(Ordering::Acquire) {
            if visited >= MAX_ENTRIES && !pending.is_empty() {
                *report.diagnostics.entry("ENTRY_LIMIT".into()).or_default() += 1;
            }
            break;
        }
    }
    report.complete = report.diagnostics.is_empty();
    let percentage = |bytes: u64| {
        if report.total_bytes > 0 {
            (bytes as f64 / report.total_bytes as f64 * 100.) as f32
        } else {
            0.
        }
    };
    for file in &mut report.largest_files {
        file.percentage = percentage(file.size_bytes);
    }
    report.subdirs = direct
        .into_iter()
        .map(|(path, (bytes, count))| DiskEntry {
            name: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            path: path.to_string_lossy().into_owned(),
            size_bytes: bytes,
            is_dir: true,
            file_count: count,
            percentage: percentage(bytes),
            category: FileCategory::Other,
        })
        .collect();
    report
        .subdirs
        .sort_by_key(|f| std::cmp::Reverse(f.size_bytes));
    report.largest_dirs = report
        .subdirs
        .iter()
        .take(options.max_largest_items.min(100))
        .cloned()
        .collect();
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
        .sort_by_key(|c| std::cmp::Reverse(c.total_bytes));
    progress(report.total_files, report.total_bytes);
    report.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_directory_structure() {
        let temp_dir =
            std::env::temp_dir().join(format!("cleancore_test_disk_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let sub = temp_dir.join("sub_folder");
        fs::create_dir_all(&sub).unwrap();

        let file1 = temp_dir.join("video.mp4");
        fs::write(&file1, vec![0u8; 1000]).unwrap();

        let file2 = sub.join("doc.pdf");
        fs::write(&file2, vec![1u8; 500]).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let options = AnalyzerOptions::default();

        let report = analyze_directory_tree(&temp_dir, &options, cancel, |_f, _b| {}).unwrap();

        assert_eq!(report.total_files, 2);
        assert_eq!(report.total_bytes, 1500);
        assert_eq!(report.categories.len(), 2);
        assert_eq!(report.largest_files.len(), 2);
        assert_eq!(report.largest_files[0].size_bytes, 1000);
        assert_eq!(report.subdirs.len(), 1);
        assert_eq!(report.subdirs[0].size_bytes, 500);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_analyze_directory_cancellation() {
        let temp_dir =
            std::env::temp_dir().join(format!("cleancore_test_cancel_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let cancel = Arc::new(AtomicBool::new(true)); // Already cancelled
        let options = AnalyzerOptions::default();

        let report = analyze_directory_tree(&temp_dir, &options, cancel, |_f, _b| {}).unwrap();
        assert_eq!(report.total_files, 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

#[cfg(all(test, windows))]
mod partial_tests {
    use super::*;
    #[test]
    fn depth_and_locked_directory_keep_the_readable_result() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = std::env::temp_dir().join(format!("disk_partial_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("blocked")).unwrap();
        fs::write(root.join("known.txt"), b"known").unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .custom_flags(0x02000000)
            .open(root.join("blocked"))
            .unwrap();
        let report = analyze_directory_tree(
            &root,
            &AnalyzerOptions::default(),
            Arc::new(AtomicBool::new(false)),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(report.total_bytes, 5);
        assert_eq!(report.largest_files.len(), 1);
        assert!(!report.complete);
        assert!(report.diagnostics.contains_key("FILE_IN_USE"));
        drop(lock);
        let report = analyze_directory_tree(
            &root,
            &AnalyzerOptions {
                max_depth: 0,
                max_largest_items: 20,
            },
            Arc::new(AtomicBool::new(false)),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(report.total_bytes, 5);
        assert!(!report.complete);
        assert_eq!(report.diagnostics["DEPTH_LIMIT"], 1);
        fs::remove_dir_all(root).unwrap();
    }
}
