//! Сканер «забытых» и неиспользуемых файлов (OPT-03).
//!
//! Анализирует каталоги пользователя (Downloads, Desktop и др.) на наличие файлов,
//! к которым не обращались и которые не изменялись дольше заданного порога дней (90 / 180 / 365).
//! Строго защищён от path traversal, junctions, симлинков и системных объектов.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::models::StaleFileEntry;

/// Игнорируемые имена файлов и расширения (системные, служебные)
const IGNORED_EXTENSIONS: &[&str] = &["lnk", "ini", "sys", "git", "lock"];
const IGNORED_FILENAMES: &[&str] = &["desktop.ini", "thumbs.db", ".ds_store", ".gitignore"];
const IGNORED_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "appdata",
    "$recycle.bin",
    "system volume information",
];

const MAX_SCAN_DEPTH: usize = 8;
const MAX_SCANNED_ENTRIES: usize = 20_000;

/// Сканирует каталог и возвращает список файлов старше `older_than_days` дней.
pub fn scan_stale_files(
    folder_path: &Path,
    older_than_days: u32,
    max_results: usize,
) -> Result<Vec<StaleFileEntry>, std::io::Error> {
    if !folder_path.exists() || !folder_path.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Каталог не существует или не является папкой",
        ));
    }

    let now = SystemTime::now();
    let threshold_secs = (older_than_days as u64).saturating_mul(86400);

    let mut results = Vec::new();
    let mut scanned_count = 0;

    scan_dir_recursive(
        folder_path,
        0,
        now,
        threshold_secs,
        &mut results,
        &mut scanned_count,
        max_results,
    )?;

    // Сортировка по размеру (самые крупные среди забытых — первыми)
    results.sort_by_key(|b| std::cmp::Reverse(b.size_bytes));
    if results.len() > max_results {
        results.truncate(max_results);
    }

    Ok(results)
}

fn scan_dir_recursive(
    dir: &Path,
    depth: usize,
    now: SystemTime,
    threshold_secs: u64,
    results: &mut Vec<StaleFileEntry>,
    scanned_count: &mut usize,
    max_results: usize,
) -> Result<(), std::io::Error> {
    if depth > MAX_SCAN_DEPTH || *scanned_count >= MAX_SCANNED_ENTRIES {
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()), // Игнорируем Access Denied и защищённые каталоги
    };

    for entry in entries.flatten() {
        *scanned_count += 1;
        if *scanned_count >= MAX_SCANNED_ENTRIES {
            break;
        }

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        // Защита от симлинков и reparse points
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let lower_name = file_name.to_lowercase();

        if file_type.is_dir() {
            if IGNORED_DIR_NAMES.iter().any(|&d| lower_name == d) || lower_name.starts_with('.') {
                continue;
            }
            let _ = scan_dir_recursive(
                &path,
                depth + 1,
                now,
                threshold_secs,
                results,
                scanned_count,
                max_results,
            );
        } else if file_type.is_file() {
            if IGNORED_FILENAMES.iter().any(|&f| lower_name == f) || lower_name.starts_with("~$") {
                continue;
            }

            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if IGNORED_EXTENSIONS.iter().any(|&e| ext == e) {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let accessed = metadata.accessed().unwrap_or(modified);

            // Берём более свежую метку из modified и accessed для безопасности
            let most_recent = if accessed > modified {
                accessed
            } else {
                modified
            };

            let age_duration = match now.duration_since(most_recent) {
                Ok(d) => d,
                Err(_) => continue, // Файл из будущего — не трогаем
            };

            let age_secs = age_duration.as_secs();
            if age_secs >= threshold_secs {
                let days_old = (age_secs / 86400) as u32;
                let iso_utc = format_system_time_utc(modified);

                results.push(StaleFileEntry {
                    path: path.to_string_lossy().to_string(),
                    name: file_name,
                    size_bytes: metadata.len(),
                    last_modified_utc: iso_utc,
                    days_old,
                    extension: ext,
                });

                if results.len() >= max_results * 2 {
                    // Ограничиваем рабочий буфер перед сортировкой
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Форматирует SystemTime в ISO-8601 UTC строку (YYYY-MM-DDTHH:MM:SSZ) без внешних крейтов.
pub fn format_system_time_utc(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();

    let sec = total_secs % 60;
    let min = (total_secs / 60) % 60;
    let hour = (total_secs / 3600) % 24;

    let days = (total_secs / 86400) as i64;
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn days_to_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_format_system_time_utc() {
        let ts = format_system_time_utc(SystemTime::UNIX_EPOCH);
        assert_eq!(ts, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_stale_scanner_filters_correctly() {
        let temp_dir =
            std::env::temp_dir().join(format!("cleancore_test_stale_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // 1. Свежий файл
        let fresh_file = temp_dir.join("fresh.txt");
        let mut f1 = File::create(&fresh_file).unwrap();
        f1.write_all(b"fresh content").unwrap();

        // 2. Игнорируемый desktop.ini
        let ini_file = temp_dir.join("desktop.ini");
        let mut f2 = File::create(&ini_file).unwrap();
        f2.write_all(b"ini content").unwrap();

        // 3. Игнорируемый .lnk
        let lnk_file = temp_dir.join("shortcut.lnk");
        let mut f3 = File::create(&lnk_file).unwrap();
        f3.write_all(b"lnk content").unwrap();

        drop(f1);
        drop(f2);
        drop(f3);

        // Сканируем с порогом 0 дней (все файлы подходят по возрасту)
        let results = scan_stale_files(&temp_dir, 0, 100).unwrap();

        // Должен найти fresh.txt, но пропустить desktop.ini и shortcut.lnk
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fresh.txt");
        assert_eq!(results[0].size_bytes, 13);
        assert_eq!(results[0].extension, "txt");

        // Очищаем тестовый каталог
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
