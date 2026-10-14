//! Модули 5.1, 5.2, 5.3: Безопасный обход каталогов, поиск крупных объектов и карта диска.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::models::{CategorySummary, DiskAnalysisReport, DiskEntry, FileCategory};

#[derive(Debug, Clone)]
pub struct AnalyzerOptions {
    pub max_depth: usize,
    pub max_largest_items: usize,
}

impl Default for AnalyzerOptions {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_largest_items: 20,
        }
    }
}

/// Проверка, является ли объект reparse point или символической ссылкой.
#[cfg(windows)]
fn is_reparse_point_or_symlink(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return true;
            }
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
            (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0
        }
        Err(_) => true,
    }
}

#[cfg(not(windows))]
fn is_reparse_point_or_symlink(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type().is_symlink(),
        Err(_) => true,
    }
}

/// Выполняет отменяемый анализ дискового пространства выбранного каталога.
pub fn analyze_directory_tree<F>(
    root_path: &Path,
    options: &AnalyzerOptions,
    cancellation_token: Arc<AtomicBool>,
    progress_callback: F,
) -> Result<DiskAnalysisReport, String>
where
    F: Fn(u64, u64), // (scanned_files, scanned_bytes)
{
    if !root_path.exists() {
        return Err(format!("Каталог не найден: {}", root_path.display()));
    }
    if !root_path.is_dir() {
        return Err(format!(
            "Путь не является каталогом: {}",
            root_path.display()
        ));
    }

    let mut scanned_files: u64 = 0;
    let mut scanned_dirs: u64 = 0;
    let mut scanned_bytes: u64 = 0;

    let mut all_files: Vec<DiskEntry> = Vec::new();
    let mut direct_subdirs: HashMap<PathBuf, (u64, u64)> = HashMap::new(); // path -> (bytes, file_count)
    let mut category_bytes: HashMap<FileCategory, (u64, u64)> = HashMap::new(); // cat -> (bytes, count)

    // Первичный сбор прямых подкаталогов
    if let Ok(entries) = fs::read_dir(root_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_reparse_point_or_symlink(&path) {
                continue;
            }
            if path.is_dir() {
                direct_subdirs.insert(path, (0, 0));
            }
        }
    }

    // Рекурсивный обход с подсчётом объёма
    traverse_recursive(
        root_path,
        root_path,
        0,
        options.max_depth,
        &cancellation_token,
        &mut scanned_files,
        &mut scanned_dirs,
        &mut scanned_bytes,
        &mut all_files,
        &mut direct_subdirs,
        &mut category_bytes,
        &progress_callback,
    );

    // Сортировка крупнейших файлов
    all_files.sort_by_key(|b| std::cmp::Reverse(b.size_bytes));
    let largest_files: Vec<DiskEntry> = all_files
        .into_iter()
        .take(options.max_largest_items)
        .map(|mut entry| {
            if scanned_bytes > 0 {
                entry.percentage = (entry.size_bytes as f64 / scanned_bytes as f64 * 100.0) as f32;
            }
            entry
        })
        .collect();

    // Преобразование прямых подкаталогов в список
    let mut subdirs: Vec<DiskEntry> = direct_subdirs
        .into_iter()
        .map(|(path, (bytes, count))| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let percentage = if scanned_bytes > 0 {
                (bytes as f64 / scanned_bytes as f64 * 100.0) as f32
            } else {
                0.0
            };
            DiskEntry {
                path: path.to_string_lossy().to_string(),
                name,
                size_bytes: bytes,
                is_dir: true,
                file_count: count,
                percentage,
                category: FileCategory::Other,
            }
        })
        .collect();

    subdirs.sort_by_key(|b| std::cmp::Reverse(b.size_bytes));
    let largest_dirs: Vec<DiskEntry> = subdirs
        .iter()
        .take(options.max_largest_items)
        .cloned()
        .collect();

    // Формирование категорий
    let mut categories: Vec<CategorySummary> = category_bytes
        .into_iter()
        .map(|(cat, (bytes, count))| {
            let percentage = if scanned_bytes > 0 {
                (bytes as f64 / scanned_bytes as f64 * 100.0) as f32
            } else {
                0.0
            };
            CategorySummary {
                category: cat,
                display_name: cat.display_name().to_string(),
                total_bytes: bytes,
                file_count: count,
                percentage,
            }
        })
        .collect();

    categories.sort_by_key(|b| std::cmp::Reverse(b.total_bytes));

    Ok(DiskAnalysisReport {
        root_path: root_path.to_string_lossy().to_string(),
        total_bytes: scanned_bytes,
        total_files: scanned_files,
        total_dirs: scanned_dirs,
        categories,
        largest_files,
        largest_dirs,
        subdirs,
    })
}

#[allow(clippy::too_many_arguments)]
fn traverse_recursive<F>(
    root_path: &Path,
    current_dir: &Path,
    current_depth: usize,
    max_depth: usize,
    cancellation_token: &Arc<AtomicBool>,
    scanned_files: &mut u64,
    scanned_dirs: &mut u64,
    scanned_bytes: &mut u64,
    all_files: &mut Vec<DiskEntry>,
    direct_subdirs: &mut HashMap<PathBuf, (u64, u64)>,
    category_bytes: &mut HashMap<FileCategory, (u64, u64)>,
    progress_callback: &F,
) where
    F: Fn(u64, u64),
{
    if cancellation_token.load(Ordering::Relaxed) {
        return;
    }
    if current_depth > max_depth {
        return;
    }

    let read_dir = match fs::read_dir(current_dir) {
        Ok(rd) => rd,
        Err(_) => return, // Отказ в доступе безопасно пропускается
    };

    for entry in read_dir.flatten() {
        if cancellation_token.load(Ordering::Relaxed) {
            return;
        }

        let path = entry.path();
        if is_reparse_point_or_symlink(&path) {
            continue;
        }

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            *scanned_dirs = scanned_dirs.saturating_add(1);
            traverse_recursive(
                root_path,
                &path,
                current_depth + 1,
                max_depth,
                cancellation_token,
                scanned_files,
                scanned_dirs,
                scanned_bytes,
                all_files,
                direct_subdirs,
                category_bytes,
                progress_callback,
            );
        } else if file_type.is_file() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size = meta.len();
            *scanned_files = scanned_files.saturating_add(1);
            *scanned_bytes = scanned_bytes.saturating_add(size);

            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            let category = FileCategory::from_extension(&ext);

            // Обновление агрегата категории
            let cat_stat = category_bytes.entry(category).or_insert((0, 0));
            cat_stat.0 = cat_stat.0.saturating_add(size);
            cat_stat.1 = cat_stat.1.saturating_add(1);

            // Приписывание размера прямому подкаталогу корня
            if let Ok(relative) = path.strip_prefix(root_path) {
                if let Some(first_comp) = relative.components().next() {
                    let direct_subdir_path = root_path.join(first_comp);
                    if let Some(stat) = direct_subdirs.get_mut(&direct_subdir_path) {
                        stat.0 = stat.0.saturating_add(size);
                        stat.1 = stat.1.saturating_add(1);
                    }
                }
            }

            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            all_files.push(DiskEntry {
                path: path.to_string_lossy().to_string(),
                name,
                size_bytes: size,
                is_dir: false,
                file_count: 1,
                percentage: 0.0,
                category,
            });

            if (*scanned_files).is_multiple_of(100) {
                progress_callback(*scanned_files, *scanned_bytes);
            }
        }
    }

    progress_callback(*scanned_files, *scanned_bytes);
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
