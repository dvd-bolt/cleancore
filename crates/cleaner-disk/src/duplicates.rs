//! Модуль 5.4: Безопасный поиск дубликатов на основе многофазного анализа (Czkawka adaptation).

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use uuid::Uuid;

use crate::models::{DuplicateFileEntry, DuplicateGroup, DuplicateScanResult};

const PREHASH_SIZE: usize = 16 * 1024; // 16 KB
const HASH_BUFFER_SIZE: usize = 64 * 1024; // 64 KB chunk

/// Идентификатор физического файла для защиты от ложных дубликатов (hard links).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PhysicalFileId {
    volume_serial: u32,
    file_index_high: u32,
    file_index_low: u32,
}

#[cfg(windows)]
fn get_physical_file_id(file: &File) -> Option<PhysicalFileId> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let handle = file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let success = unsafe { GetFileInformationByHandle(handle, &mut info) };
    if success != 0 {
        Some(PhysicalFileId {
            volume_serial: info.dwVolumeSerialNumber,
            file_index_high: info.nFileIndexHigh,
            file_index_low: info.nFileIndexLow,
        })
    } else {
        None
    }
}

#[cfg(not(windows))]
fn get_physical_file_id(_file: &File) -> Option<PhysicalFileId> {
    None
}

/// Проверка reparse point или symlink
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

/// Преобразование SystemTime в ISO 8601 UTC.
fn system_time_to_iso(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();

    let sec = total_secs % 60;
    let min = (total_secs / 60) % 60;
    let hour = (total_secs / 3600) % 24;

    let mut days = total_secs / 86400;
    let mut year = 1970;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let days_in_months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month = 1;
    for &dim in &days_in_months {
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }
    let day = days + 1;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Выполняет безопасный многофазный поиск дубликатов файлов.
pub fn find_duplicates<F>(
    root_path: &Path,
    cancellation_token: Arc<AtomicBool>,
    progress_callback: F,
) -> Result<DuplicateScanResult, String>
where
    F: Fn(&str, u64, u64), // (phase_description, current, total)
{
    if !root_path.exists() || !root_path.is_dir() {
        return Err(format!("Каталог не найден: {}", root_path.display()));
    }

    // -------------------------------------------------------------
    // Фаза 1: Сбор файлов и группировка по точному размеру в байтах
    // -------------------------------------------------------------
    progress_callback(
        "Фаза 1/3: Сканирование файлов и группировка по размеру...",
        0,
        0,
    );

    let mut size_groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut total_scanned = 0u64;

    collect_files_by_size(
        root_path,
        &cancellation_token,
        &mut size_groups,
        &mut total_scanned,
        &progress_callback,
    );

    if cancellation_token.load(Ordering::Relaxed) {
        return Ok(DuplicateScanResult {
            root_path: root_path.to_string_lossy().to_string(),
            groups: Vec::new(),
            total_wasted_bytes: 0,
            total_duplicate_files: 0,
        });
    }

    // Оставляем только размеры с 2 и более файлами (> 0 байт)
    let candidate_sizes: Vec<(u64, Vec<PathBuf>)> = size_groups
        .into_iter()
        .filter(|(size, paths)| *size > 0 && paths.len() >= 2)
        .collect();

    let total_phase2_files: u64 = candidate_sizes.iter().map(|(_, p)| p.len() as u64).sum();

    type PrehashKey = (u64, [u8; 32]);
    type PrehashItem = (PathBuf, Option<PhysicalFileId>);
    type PrehashGroupMap = HashMap<PrehashKey, Vec<PrehashItem>>;
    type SurvivingPrehashList = Vec<(PrehashKey, Vec<PrehashItem>)>;

    // -------------------------------------------------------------
    // Фаза 2: Вычисление предварительного хеша (prehash: первые 16 КБ)
    // -------------------------------------------------------------
    progress_callback(
        "Фаза 2/3: Предварительное хеширование (prehash)...",
        0,
        total_phase2_files,
    );

    let mut prehash_groups: PrehashGroupMap = HashMap::new();
    let mut phase2_processed = 0u64;

    for (size, paths) in candidate_sizes {
        if cancellation_token.load(Ordering::Relaxed) {
            break;
        }

        for path in paths {
            if cancellation_token.load(Ordering::Relaxed) {
                break;
            }

            phase2_processed += 1;
            if phase2_processed.is_multiple_of(20) {
                progress_callback(
                    "Фаза 2/3: Предварительное хеширование...",
                    phase2_processed,
                    total_phase2_files,
                );
            }

            if let Ok((prehash, phys_id)) = compute_prehash(&path) {
                prehash_groups
                    .entry((size, prehash))
                    .or_default()
                    .push((path, phys_id));
            }
        }
    }

    // Оставляем группы, где prehash совпал минимум у 2 файлов
    let surviving_prehash: SurvivingPrehashList = prehash_groups
        .into_iter()
        .filter(|(_, items)| items.len() >= 2)
        .collect();

    let total_phase3_files: u64 = surviving_prehash.iter().map(|(_, p)| p.len() as u64).sum();

    // -------------------------------------------------------------
    // Фаза 3: Полный криптографический хеш (SHA-256)
    // -------------------------------------------------------------
    progress_callback(
        "Фаза 3/3: Полный расчет контрольных сумм...",
        0,
        total_phase3_files,
    );

    let mut full_hash_groups: HashMap<(u64, String), Vec<DuplicateFileEntry>> = HashMap::new();
    let mut phase3_processed = 0u64;

    for ((size, _prehash), items) in surviving_prehash {
        if cancellation_token.load(Ordering::Relaxed) {
            break;
        }

        // Фильтр hard links: одинаковый physical_id исключается
        let mut unique_physical: HashMap<PhysicalFileId, PathBuf> = HashMap::new();
        let mut filtered_items: Vec<PathBuf> = Vec::new();

        for (path, maybe_phys_id) in items {
            if let Some(phys_id) = maybe_phys_id {
                if unique_physical.contains_key(&phys_id) {
                    // Это hard link на уже учтённый файл — пропускаем, не дубликат!
                    continue;
                }
                unique_physical.insert(phys_id, path.clone());
            }
            filtered_items.push(path);
        }

        if filtered_items.len() < 2 {
            continue;
        }

        for path in filtered_items {
            if cancellation_token.load(Ordering::Relaxed) {
                break;
            }

            phase3_processed += 1;
            if phase3_processed.is_multiple_of(5) {
                progress_callback(
                    "Фаза 3/3: Полный расчет контрольных сумм...",
                    phase3_processed,
                    total_phase3_files,
                );
            }

            if let Ok((full_hash, modified)) = compute_full_hash(&path) {
                full_hash_groups
                    .entry((size, full_hash))
                    .or_default()
                    .push(DuplicateFileEntry {
                        path: path.to_string_lossy().to_string(),
                        modified_utc: system_time_to_iso(modified),
                        size_bytes: size,
                    });
            }
        }
    }

    // -------------------------------------------------------------
    // Формирование итогового отчёта
    // -------------------------------------------------------------
    let mut final_groups: Vec<DuplicateGroup> = Vec::new();
    let mut total_wasted_bytes: u64 = 0;
    let mut total_duplicate_files: usize = 0;

    for ((size, hash), files) in full_hash_groups {
        if files.len() >= 2 {
            let count = files.len() as u64;
            let reclaimable = size.saturating_mul(count.saturating_sub(1));
            total_wasted_bytes = total_wasted_bytes.saturating_add(reclaimable);
            total_duplicate_files += files.len();

            final_groups.push(DuplicateGroup {
                group_id: Uuid::new_v4().to_string(),
                file_size_bytes: size,
                full_hash: hash,
                files,
                reclaimable_bytes: reclaimable,
            });
        }
    }

    // Сортировка групп по потенциально освобождаемому месту
    final_groups.sort_by_key(|b| std::cmp::Reverse(b.reclaimable_bytes));

    progress_callback(
        "Анализ дубликатов завершён.",
        total_phase3_files,
        total_phase3_files,
    );

    Ok(DuplicateScanResult {
        root_path: root_path.to_string_lossy().to_string(),
        groups: final_groups,
        total_wasted_bytes,
        total_duplicate_files,
    })
}

fn collect_files_by_size<F>(
    dir: &Path,
    cancellation_token: &Arc<AtomicBool>,
    size_groups: &mut HashMap<u64, Vec<PathBuf>>,
    total_scanned: &mut u64,
    progress_callback: &F,
) where
    F: Fn(&str, u64, u64),
{
    if cancellation_token.load(Ordering::Relaxed) {
        return;
    }

    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
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
            collect_files_by_size(
                &path,
                cancellation_token,
                size_groups,
                total_scanned,
                progress_callback,
            );
        } else if file_type.is_file() {
            if let Ok(meta) = entry.metadata() {
                let size = meta.len();
                if size > 0 {
                    size_groups.entry(size).or_default().push(path);
                    *total_scanned += 1;
                    if (*total_scanned).is_multiple_of(200) {
                        progress_callback("Фаза 1/3: Поиск файлов...", *total_scanned, 0);
                    }
                }
            }
        }
    }
}

/// Вычисляет предварительный хеш первых 16 КБ файла с проверкой неизменности.
fn compute_prehash(path: &Path) -> Result<([u8; 32], Option<PhysicalFileId>), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;

    let meta_before = file.metadata().map_err(|e| e.to_string())?;
    let modified_before = meta_before.modified().map_err(|e| e.to_string())?;

    let phys_id = get_physical_file_id(&file);

    let mut buf = vec![0u8; PREHASH_SIZE];
    let n = file.read(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(n);

    // Повторная проверка времени модификации (защита от изменений на лету)
    let meta_after = file.metadata().map_err(|e| e.to_string())?;
    let modified_after = meta_after.modified().map_err(|e| e.to_string())?;

    if modified_before != modified_after {
        return Err("Файл изменился во время чтения предварительного хеша".into());
    }

    let mut hasher = Sha256::new();
    hasher.update(&buf);
    let result = hasher.finalize();

    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(&result);

    Ok((hash_arr, phys_id))
}

/// Вычисляет полный SHA-256 хеш содержимого файла с потоковым чтением.
fn compute_full_hash(path: &Path) -> Result<(String, SystemTime), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;

    let meta_before = file.metadata().map_err(|e| e.to_string())?;
    let modified_before = meta_before.modified().map_err(|e| e.to_string())?;

    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;

    let mut hasher = Sha256::new();
    let mut buf = [0u8; HASH_BUFFER_SIZE];

    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    // Повторная проверка времени модификации (TOCTOU protection)
    let meta_after = file.metadata().map_err(|e| e.to_string())?;
    let modified_after = meta_after.modified().map_err(|e| e.to_string())?;

    if modified_before != modified_after {
        return Err("Файл изменился во время вычисления полного хеша".into());
    }

    let result = hasher.finalize();
    let hex_hash = format!("{result:x}");

    Ok((hex_hash, modified_after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_real_duplicates() {
        let temp_dir = std::env::temp_dir().join(format!("cleancore_test_dup_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let content = b"This is identical duplicate content for testing CleanCore stage 5";

        let file1 = temp_dir.join("orig.txt");
        fs::write(&file1, content).unwrap();

        let file2 = temp_dir.join("copy1.txt");
        fs::write(&file2, content).unwrap();

        let file3 = temp_dir.join("different.txt");
        fs::write(&file3, b"Completely different content of different length").unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let res = find_duplicates(&temp_dir, cancel, |_p, _c, _t| {}).unwrap();

        assert_eq!(res.groups.len(), 1);
        assert_eq!(res.groups[0].files.len(), 2);
        assert_eq!(res.total_duplicate_files, 2);
        assert_eq!(res.total_wasted_bytes, content.len() as u64);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_name_collision_is_not_duplicate() {
        let temp_dir =
            std::env::temp_dir().join(format!("cleancore_test_collision_{}", Uuid::new_v4()));
        let dir1 = temp_dir.join("d1");
        let dir2 = temp_dir.join("d2");
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();

        // Одинаковое имя, одинаковый размер, но РАЗНОЕ содержимое!
        let file1 = dir1.join("same_name.bin");
        fs::write(&file1, b"AAAAAAAAAA").unwrap();

        let file2 = dir2.join("same_name.bin");
        fs::write(&file2, b"BBBBBBBBBB").unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let res = find_duplicates(&temp_dir, cancel, |_p, _c, _t| {}).unwrap();

        // Не должно считаться дубликатами, так как хеш разный!
        assert_eq!(res.groups.len(), 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
