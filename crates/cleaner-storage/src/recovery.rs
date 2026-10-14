//! Утилиты устойчивости к повреждениям, изоляции (.corrupt) и форматирования времени.

use rusqlite::ErrorCode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Уведомление о восстановлении базы данных SQLite после повреждения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageRecoveryNotice {
    pub original_path: PathBuf,
    pub quarantined_path: PathBuf,
    pub timestamp_utc: String,
    pub reason: String,
}

/// Уведомление о восстановлении настроек settings.json после повреждения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsRecoveryNotice {
    pub original_path: PathBuf,
    pub quarantined_path: PathBuf,
    pub timestamp_utc: String,
    pub reason: String,
}

/// Проверяет, указывает ли ошибка SQLite на фактическое повреждение файла базы данных.
/// ВНИМАНИЕ: Ошибки блокировок, занятости файла (Busy, Locked) или отсутствия прав
/// НЕ являются повреждением данных!
pub fn is_sqlite_corrupt_error(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(ffi_err, msg) => {
            let code = ffi_err.code;
            if code == ErrorCode::DatabaseCorrupt || code == ErrorCode::NotADatabase {
                return true;
            }
            if let Some(m) = msg {
                let lower = m.to_lowercase();
                if lower.contains("file is not a database")
                    || lower.contains("database disk image is malformed")
                    || lower.contains("file is encrypted or is not a database")
                    || lower.contains("file is corrupt")
                {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// Изолирует повреждённый файл по указанному пути в `<path>.<timestamp>.<tag>`.
/// Не перезаписывает существующие файлы карантина при повторных вызовах в течение одной секунды.
pub fn quarantine_file(path: &Path, tag: &str) -> Result<PathBuf, std::io::Error> {
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Файл для карантина не найден: {}", path.display()),
        ));
    }

    let timestamp = simple_utc_now_compact();
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("corrupt_file");

    let mut candidate = parent.join(format!("{file_name}.{timestamp}.{tag}"));
    let mut counter = 1;
    while candidate.exists() {
        candidate = parent.join(format!("{file_name}.{timestamp}_{counter}.{tag}"));
        counter += 1;
    }

    std::fs::rename(path, &candidate)?;
    Ok(candidate)
}

/// Очищает сопутствующие файлы SQLite журналов (-wal, -shm, -journal),
/// которые могли остаться после сбоя или повреждения базы.
pub fn clean_sqlite_sidecars(db_path: &Path) {
    let base = db_path.as_os_str();
    for suffix in &["-wal", "-shm", "-journal", ".wal", ".shm", ".journal"] {
        let mut sidecar_name = base.to_owned();
        sidecar_name.push(suffix);
        let sidecar_path = PathBuf::from(sidecar_name);
        if sidecar_path.exists() {
            let _ = std::fs::remove_file(sidecar_path);
        }
    }
}

/// Возвращает текущее время UTC в формате `YYYYMMDD_HHMMSS`.
pub fn simple_utc_now_compact() -> String {
    format_timestamp_compact(SystemTime::now())
}

/// Возвращает текущее время UTC в формате ISO 8601 `YYYY-MM-DDTHH:MM:SSZ`.
pub fn simple_utc_now_iso() -> String {
    format_timestamp_iso(SystemTime::now())
}

pub fn format_timestamp_compact(time: SystemTime) -> String {
    let (year, month, day, hour, min, sec) = decompose_system_time(time);
    format!("{year:04}{month:02}{day:02}_{hour:02}{min:02}{sec:02}")
}

pub fn format_timestamp_iso(time: SystemTime) -> String {
    let (year, month, day, hour, min, sec) = decompose_system_time(time);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn decompose_system_time(time: SystemTime) -> (i32, u32, u32, u64, u64, u64) {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let sec = total_secs % 60;
    let min = (total_secs / 60) % 60;
    let hour = (total_secs / 3600) % 24;

    let days = (total_secs / 86400) as i64;
    let (year, month, day) = days_to_ymd(days);
    (year, month, day, hour, min, sec)
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

    #[test]
    fn test_timestamp_formatting() {
        let compact = simple_utc_now_compact();
        assert_eq!(compact.len(), 15);
        assert!(compact.contains('_'));

        let iso = simple_utc_now_iso();
        assert!(iso.ends_with('Z'));
        assert!(iso.contains('T'));
    }

    #[test]
    fn test_quarantine_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_quarantine_{}", simple_utc_now_compact()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let target = temp_dir.join("test.db");
        std::fs::write(&target, b"broken database content").unwrap();

        let quarantined = quarantine_file(&target, "corrupt").unwrap();
        assert!(!target.exists());
        assert!(quarantined.exists());
        assert!(quarantined.to_string_lossy().ends_with(".corrupt"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
