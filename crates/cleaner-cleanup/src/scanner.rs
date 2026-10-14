//! Bounded read-only traversal with scan-time identities and visible exclusions.
use crate::{path_safety::is_reparse_point_or_symlink, rules::CleanupRule};
use cleaner_core::{models::CleanupCandidate, safety::RiskLevel};
use cleaner_execution::native::{capture_candidate, lock_directory, TrustedCandidate};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime},
};
#[derive(Debug, Clone)]
pub struct ScannerOptions {
    pub max_depth: usize,
    pub max_scanned_files: usize,
    pub max_candidates: usize,
    pub max_duration: Duration,
}
impl Default for ScannerOptions {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_scanned_files: 50_000,
            max_candidates: cleaner_execution::MAX_CANDIDATES,
            max_duration: Duration::from_secs(60),
        }
    }
}
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub candidates: Vec<CleanupCandidate>,
    pub identities: Vec<TrustedCandidate>,
    pub total_scanned_items: u64,
    pub total_matched_bytes: u64,
    pub was_cancelled: bool,
    pub reached_limit: bool,
    pub complete: bool,
    pub diagnostics: BTreeMap<String, u64>,
    pub fatal_error: Option<String>,
}
pub fn scan_directory<F: Fn(u64, u64, u64)>(
    root: &Path,
    rule: &CleanupRule,
    options: &ScannerOptions,
    cancel: Arc<AtomicBool>,
    progress: F,
) -> ScanResult {
    let mut context = ScanContext {
        root,
        rule,
        options,
        cancel,
        progress,
        now: SystemTime::now(),
        started: Instant::now(),
        result: ScanResult {
            candidates: vec![],
            identities: vec![],
            total_scanned_items: 0,
            total_matched_bytes: 0,
            was_cancelled: false,
            reached_limit: false,
            complete: true,
            diagnostics: BTreeMap::new(),
            fatal_error: None,
        },
    };
    context.scan_dir(root, 0);
    if context.result.was_cancelled
        || context.result.reached_limit
        || context.result.fatal_error.is_some()
    {
        context.result.complete = false;
    }
    (context.progress)(
        context.result.total_scanned_items,
        context.result.candidates.len() as u64,
        context.result.total_matched_bytes,
    );
    context.result
}
struct ScanContext<'a, F> {
    root: &'a Path,
    rule: &'a CleanupRule,
    options: &'a ScannerOptions,
    cancel: Arc<AtomicBool>,
    progress: F,
    now: SystemTime,
    started: Instant,
    result: ScanResult,
}
impl<F: Fn(u64, u64, u64)> ScanContext<'_, F> {
    fn reason(&mut self, code: &str) {
        *self.result.diagnostics.entry(code.into()).or_default() += 1;
    }
    fn stop(&mut self) -> bool {
        if self.cancel.load(Ordering::Acquire) {
            self.result.was_cancelled = true;
            return true;
        }
        let code = if self.result.total_scanned_items >= self.options.max_scanned_files as u64 {
            Some("ENTRY_LIMIT")
        } else if self.result.candidates.len() >= self.options.max_candidates {
            Some("CANDIDATE_LIMIT")
        } else if self.started.elapsed() >= self.options.max_duration {
            Some("TIME_LIMIT")
        } else {
            None
        };
        if let Some(code) = code {
            self.result.reached_limit = true;
            self.reason(code);
            return true;
        }
        false
    }
    fn scan_dir(&mut self, dir: &Path, depth: usize) {
        if self.stop() {
            return;
        }
        if depth > self.options.max_depth {
            self.result.complete = false;
            self.reason("DEPTH_LIMIT");
            return;
        }
        let Ok(relative) = dir.strip_prefix(self.root) else {
            self.result.complete = false;
            self.reason("OUTSIDE_ROOT");
            return;
        };
        // Bound all ancestors while iterating this directory. This blocks parent
        // replacement/reparse mutation, rather than checking only string prefixes.
        let _lease = match lock_directory(self.root, relative) {
            Ok(v) => v,
            Err(code) => {
                self.reason(code);
                self.result.complete = false;
                if depth == 0 {
                    self.result.fatal_error = Some(code.into());
                }
                return;
            }
        };
        let entries = match std::fs::read_dir(dir) {
            Ok(v) => v,
            Err(_) => {
                self.reason("READ_DIRECTORY_FAILED");
                self.result.complete = false;
                if depth == 0 {
                    self.result.fatal_error = Some("READ_DIRECTORY_FAILED".into());
                }
                return;
            }
        };
        for entry in entries {
            if self.stop() {
                return;
            }
            self.result.total_scanned_items += 1;
            if self.result.total_scanned_items.is_multiple_of(100) {
                (self.progress)(
                    self.result.total_scanned_items,
                    self.result.candidates.len() as u64,
                    self.result.total_matched_bytes,
                );
            }
            let entry = match entry {
                Ok(v) => v,
                Err(_) => {
                    self.reason("READ_ENTRY_FAILED");
                    self.result.complete = false;
                    continue;
                }
            };
            let path = entry.path();
            if is_reparse_point_or_symlink(&path) {
                self.reason("REPARSE_OR_UNAVAILABLE");
                continue;
            }
            let kind = match entry.file_type() {
                Ok(v) => v,
                Err(_) => {
                    self.reason("TYPE_UNAVAILABLE");
                    continue;
                }
            };
            if kind.is_dir() {
                self.scan_dir(&path, depth + 1);
                if self.result.was_cancelled || self.result.reached_limit {
                    return;
                }
                continue;
            }
            if !kind.is_file() {
                self.reason("NOT_REGULAR_FILE");
                continue;
            }
            let relative = match path.strip_prefix(self.root) {
                Ok(v) => v,
                Err(_) => {
                    self.reason("OUTSIDE_ROOT");
                    continue;
                }
            };
            let object = match capture_candidate(self.root, relative) {
                Ok(v) => v,
                Err(code) => {
                    self.reason(code);
                    continue;
                }
            };
            let modified = match object.modified_time() {
                Some(v) => v,
                None => {
                    self.reason("TIME_UNAVAILABLE");
                    continue;
                }
            };
            if !self
                .now
                .duration_since(modified)
                .is_ok_and(|age| age >= self.rule.min_age_duration())
            {
                self.reason("TOO_RECENT_OR_FUTURE");
                continue;
            }
            let size = object.size_bytes();
            self.result.total_matched_bytes = self.result.total_matched_bytes.saturating_add(size);
            self.result.candidates.push(CleanupCandidate {
                obj_id: object.object_id().into(),
                display_path: relative.to_string_lossy().into_owned(),
                absolute_path: object.display_path(),
                size_bytes: size.to_string(),
                category_id: self.rule.id.clone(),
                reason: format!("Не изменялся не менее {} суток", self.rule.min_age_days),
                modified_time_utc: system_time_to_iso(modified),
                risk_level: RiskLevel::Low,
            });
            self.result.identities.push(object);
        }
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
