//! Потоковый безопасный сканер временных файлов с поддержкой отмены и лимитов.

use cleaner_core::models::CleanupCandidate;
use cleaner_core::safety::RiskLevel;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

use crate::path_safety::{is_reparse_point_or_symlink, is_safe_target_file};
use crate::rules::CleanupRule;

/// Ограничения и параметры безопасного обхода.
#[derive(Debug, Clone)]
pub struct ScannerOptions {
    /// Максимальная глубина рекурсии (защита от зацикливаний и бесконечных путей).
    pub max_depth: usize,
    /// Максимальное число проверяемых файлов за один сеанс.
    pub max_scanned_files: usize,
    /// Минимальный возраст файла для включения в кандидаты.
    pub min_age: Duration,
}

impl Default for ScannerOptions {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_scanned_files: 50_000,
            min_age: Duration::from_secs(7 * 24 * 3600), // 7 дней согласно ТЗ
        }
    }
}

/// Результат работы сканера.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Найденные кандидаты на очистку.
    pub candidates: Vec<CleanupCandidate>,
    /// Общее количество просмотренных объектов.
    pub total_scanned_items: u64,
    /// Общий размер найденных кандидатов в байтах.
    pub total_matched_bytes: u64,
    /// Был ли сканер остановлен по запросу отмены.
    pub was_cancelled: bool,
    /// Был ли достигнут лимит файлов (частичный результат).
    pub reached_limit: bool,
}

/// Выполняет постепенный безопасный обход каталога в соответствии с правилом.
///
/// Безопасность:
/// - Проверяет `cancellation_token` на каждом шаге.
/// - Не следует по symlinks, junctions и точкам монтирования.
/// - Пропускает файлы с ошибками чтения без паники.
/// - Никакие файлы не изменяются, не блокируются и не удаляются.
pub fn scan_directory<F>(
    root: &Path,
    rule: &CleanupRule,
    options: &ScannerOptions,
    cancellation_token: Arc<AtomicBool>,
    progress_fn: F,
) -> ScanResult
where
    F: Fn(u64, u64, u64), // (scanned_items, matched_items, matched_bytes)
{
    let now = SystemTime::now();
    let mut context = ScanContext {
        root,
        rule,
        options,
        now: &now,
        cancellation_token: &cancellation_token,
        candidates: Vec::new(),
        scanned_count: 0,
        matched_bytes: 0,
        was_cancelled: false,
        reached_limit: false,
        progress_fn,
    };

    context.scan_dir(root, 0);

    ScanResult {
        candidates: context.candidates,
        total_scanned_items: context.scanned_count,
        total_matched_bytes: context.matched_bytes,
        was_cancelled: context.was_cancelled,
        reached_limit: context.reached_limit,
    }
}

struct ScanContext<'a, F> {
    root: &'a Path,
    rule: &'a CleanupRule,
    options: &'a ScannerOptions,
    now: &'a SystemTime,
    cancellation_token: &'a Arc<AtomicBool>,
    candidates: Vec<CleanupCandidate>,
    scanned_count: u64,
    matched_bytes: u64,
    was_cancelled: bool,
    reached_limit: bool,
    progress_fn: F,
}

impl<'a, F: Fn(u64, u64, u64)> ScanContext<'a, F> {
    fn scan_dir(&mut self, current_dir: &Path, depth: usize) {
        // 1. Проверка отмены
        if self.cancellation_token.load(Ordering::Relaxed) {
            self.was_cancelled = true;
            return;
        }

        // 2. Проверка ограничений глубины
        if depth > self.options.max_depth {
            return;
        }

        // 3. Проверка лимита файлов
        if self.scanned_count as usize >= self.options.max_scanned_files {
            self.reached_limit = true;
            return;
        }

        // 4. Чтение содержимого каталога (безопасный пропуск при отказе доступа)
        let read_dir = match std::fs::read_dir(current_dir) {
            Ok(rd) => rd,
            Err(_) => return, // AccessDenied или занятый каталог безопасно пропускается
        };

        for entry in read_dir.flatten() {
            if self.cancellation_token.load(Ordering::Relaxed) {
                self.was_cancelled = true;
                return;
            }

            if self.scanned_count as usize >= self.options.max_scanned_files {
                self.reached_limit = true;
                return;
            }

            self.scanned_count = self.scanned_count.saturating_add(1);
            let path = entry.path();

            // Пропуск reparse points и symlinks
            if is_reparse_point_or_symlink(&path) {
                continue;
            }

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                // Рекурсивный спуск в подкаталог
                self.scan_dir(&path, depth + 1);

                if self.was_cancelled || self.reached_limit {
                    return;
                }
            } else if file_type.is_file() {
                // Проверка обычного файла
                if !is_safe_target_file(self.root, &path) {
                    continue;
                }

                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let size = meta.len();
                let modified = match meta.modified() {
                    Ok(mod_time) => mod_time,
                    Err(_) => continue,
                };

                // Проверка возраста: файл должен быть не моложе min_age (7 дней)
                let age_ok = match self.now.duration_since(modified) {
                    Ok(age) => age >= self.options.min_age,
                    Err(_) => false, // Время в будущем пропускается
                };

                if age_ok {
                    let obj_id = Uuid::new_v4().to_string();
                    let display_path = path
                        .strip_prefix(self.root)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string_lossy().to_string());

                    let modified_utc = system_time_to_iso(modified);

                    self.matched_bytes = self.matched_bytes.saturating_add(size);

                    self.candidates.push(CleanupCandidate {
                        obj_id,
                        display_path,
                        absolute_path: path.to_string_lossy().to_string(),
                        size_bytes: size,
                        category_id: self.rule.id.clone(),
                        reason: format!("Файл не изменялся более {} суток", self.rule.min_age_days),
                        modified_time_utc: modified_utc,
                        risk_level: RiskLevel::Low,
                    });

                    // Периодическое оповещение о прогрессе (каждые 20 кандидатов или файлов)
                    if self.candidates.len().is_multiple_of(20)
                        || self.scanned_count.is_multiple_of(100)
                    {
                        (self.progress_fn)(
                            self.scanned_count,
                            self.candidates.len() as u64,
                            self.matched_bytes,
                        );
                    }
                }
            }
        }

        // Финальное оповещение по каталогу
        (self.progress_fn)(
            self.scanned_count,
            self.candidates.len() as u64,
            self.matched_bytes,
        );
    }
}

pub fn system_time_to_iso(time: SystemTime) -> String {
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

    #[test]
    fn test_scanner_respects_age_filter() {
        let temp_dir = std::env::temp_dir().join(format!("cc_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let fresh_file = temp_dir.join("fresh.tmp");
        std::fs::write(&fresh_file, b"fresh data").unwrap();

        let rule = CleanupRule::user_temp_v1();
        let options = ScannerOptions::default();
        let cancel = Arc::new(AtomicBool::new(false));

        let res = scan_directory(&temp_dir, &rule, &options, cancel, |_, _, _| {});

        // Свежий файл (возраст < 7 дней) НЕ должен попасть в кандидаты
        assert_eq!(res.candidates.len(), 0);
        assert_eq!(res.total_scanned_items, 1);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_scanner_cancellation() {
        let temp_dir = std::env::temp_dir().join(format!("cc_test_cancel_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        for i in 0..10 {
            std::fs::write(temp_dir.join(format!("{i}.tmp")), b"data").unwrap();
        }

        let rule = CleanupRule::user_temp_v1();
        let options = ScannerOptions::default();
        let cancel = Arc::new(AtomicBool::new(true)); // Изначально отменён

        let res = scan_directory(&temp_dir, &rule, &options, cancel, |_, _, _| {});
        assert!(res.was_cancelled);

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
