//! Resumable, bounded read-only traversal with scan-time object identities.
use crate::{path_safety::is_reparse_point_or_symlink, rules::CleanupRule};
use cleaner_core::models::CleanupCandidate;
use cleaner_execution::native::{
    capture_candidate, lock_directory, ObjectIdentity, TrustedCandidate,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
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
    pub excluded_paths: Vec<String>,
}
impl Default for ScannerOptions {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_scanned_files: 50_000,
            max_candidates: cleaner_execution::MAX_CANDIDATES,
            max_duration: Duration::from_secs(2),
            excluded_paths: vec![],
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
    pub resumable: bool,
    pub batch_ready: bool,
}
impl ScanResult {
    pub fn execution_subset_allowed(&self) -> bool {
        !self.was_cancelled
            && !self.reached_limit
            && self.fatal_error.is_none()
            && !self.diagnostics.keys().any(|code| code.ends_with("_LIMIT"))
    }
}
struct Frame {
    path: PathBuf,
    depth: usize,
    entries: std::fs::ReadDir,
    stamp: Vec<ObjectIdentity>,
}
/// ReadDir cursors stay on the backend. Ancestors are unlocked between slices
/// and their identities must match before the next entry is consumed.
pub struct ScanSession {
    root: PathBuf,
    rule: CleanupRule,
    options: ScannerOptions,
    frames: Vec<Frame>,
    pending_roots: Vec<PathBuf>,
    result: ScanResult,
    now: SystemTime,
    initialized: bool,
    finished: bool,
}
impl ScanSession {
    pub fn new(root: PathBuf, rule: CleanupRule, options: ScannerOptions) -> Self {
        Self {
            root,
            rule,
            options,
            frames: vec![],
            pending_roots: vec![],
            now: SystemTime::now(),
            initialized: false,
            finished: false,
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
                resumable: false,
                batch_ready: false,
            },
        }
    }
    pub fn new_roots(mut roots: Vec<PathBuf>, rule: CleanupRule, options: ScannerOptions) -> Self {
        let first = roots.remove(0);
        roots.reverse();
        let mut session = Self::new(first, rule, options);
        session.pending_roots = roots;
        session
    }
    fn reason(&mut self, code: &str) {
        *self.result.diagnostics.entry(code.into()).or_default() += 1;
    }
    fn enter(&mut self, path: PathBuf, depth: usize) {
        if depth > self.options.max_depth {
            self.result.complete = false;
            self.reason("DEPTH_LIMIT");
            return;
        }
        let relative = path
            .strip_prefix(&self.root)
            .expect("internally constructed child path");
        let attempt = lock_directory(&self.root, relative).and_then(|lease| {
            let entries = std::fs::read_dir(&path).map_err(|_| "READ_DIRECTORY_FAILED")?;
            Ok((entries, lease.stamp()))
        });
        match attempt {
            Ok((entries, stamp)) => self.frames.push(Frame {
                path,
                depth,
                entries,
                stamp,
            }),
            Err(code) => {
                self.reason(code);
                self.result.complete = false;
                if depth == 0 {
                    self.result.fatal_error = Some(code.into());
                }
            }
        }
    }
    /// Starts a fresh confirmation batch; the caller must revoke its predecessor.
    pub fn next_batch(&mut self) {
        if self.result.batch_ready {
            self.result.candidates.clear();
            self.result.identities.clear();
            self.result.total_matched_bytes = 0;
            self.result.batch_ready = false;
        }
    }
    pub fn run_slice<F: Fn(u64, u64, u64)>(
        &mut self,
        cancel: &AtomicBool,
        progress: F,
    ) -> ScanResult {
        if self.finished {
            return self.result.clone();
        }
        self.result.resumable = false;
        self.result.reached_limit = false;
        for code in ["ENTRY_LIMIT", "TIME_LIMIT"] {
            self.result.diagnostics.remove(code);
        }
        if !self.initialized {
            self.initialized = true;
            self.enter(self.root.clone(), 0);
        }
        let start = Instant::now();
        let initial_count = self.result.total_scanned_items;
        loop {
            if cancel.load(Ordering::Acquire) {
                self.result.was_cancelled = true;
                self.result.complete = false;
                self.finished = true;
                break;
            }
            if self.frames.is_empty() {
                if let Some(root) = self.pending_roots.pop() {
                    self.root = root;
                    self.enter(self.root.clone(), 0);
                    continue;
                }
                self.finished = true;
                break;
            }
            if self.result.candidates.len() >= self.options.max_candidates {
                self.result.batch_ready = true;
                self.result.resumable = true;
                break;
            }
            let pause = if self.result.total_scanned_items - initial_count
                >= self.options.max_scanned_files as u64
            {
                Some("ENTRY_LIMIT")
            } else if start.elapsed() >= self.options.max_duration {
                Some("TIME_LIMIT")
            } else {
                None
            };
            if let Some(code) = pause {
                self.reason(code);
                self.result.reached_limit = true;
                self.result.resumable = true;
                break;
            }
            let frame = self.frames.last_mut().expect("nonempty stack");
            let lease = lock_directory(
                &self.root,
                frame.path.strip_prefix(&self.root).expect("child path"),
            );
            let lease = match lease {
                Ok(lease) if lease.stamp() == frame.stamp => lease,
                _ => {
                    self.reason("DIRECTORY_CHANGED");
                    self.result.complete = false;
                    self.frames.pop();
                    continue;
                }
            };
            let depth = frame.depth;
            let entry = frame.entries.next();
            let Some(entry) = entry else {
                self.frames.pop();
                continue;
            };
            self.result.total_scanned_items += 1;
            if self.result.total_scanned_items.is_multiple_of(100) {
                progress(
                    self.result.total_scanned_items,
                    self.result.candidates.len() as u64,
                    self.result.total_matched_bytes,
                );
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    self.reason("READ_ENTRY_FAILED");
                    self.result.complete = false;
                    continue;
                }
            };
            let path = entry.path();
            if self.options.excluded_paths.iter().any(|root| {
                cleaner_core::cleanup_preferences::contains_path(root, &path.to_string_lossy())
            }) {
                self.reason("USER_EXCLUDED");
                continue;
            }
            if is_reparse_point_or_symlink(&path) {
                self.reason("REPARSE_OR_UNAVAILABLE");
                continue;
            }
            let kind = match entry.file_type() {
                Ok(k) => k,
                Err(_) => {
                    self.reason("TYPE_UNAVAILABLE");
                    continue;
                }
            };
            if kind.is_dir() {
                self.enter(path, depth + 1);
                continue;
            }
            if !kind.is_file() {
                self.reason("NOT_REGULAR_FILE");
                continue;
            }
            if path.extension().is_some_and(|extension| {
                ["vhd", "vhdx", "vmdk", "vdi"]
                    .iter()
                    .any(|blocked| extension.eq_ignore_ascii_case(blocked))
            }) {
                self.reason("VIRTUAL_DISK");
                continue;
            }
            if self.rule.extension.as_ref().is_some_and(|extension| {
                !path
                    .extension()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(extension))
            }) {
                self.reason("EXTENSION_EXCLUDED");
                continue;
            }
            let relative = path.strip_prefix(&self.root).expect("child path");
            let object = match capture_candidate(&self.root, relative) {
                Ok(o) => o,
                Err(code) => {
                    self.reason(code);
                    continue;
                }
            };
            let Some(modified) = object.modified_time() else {
                self.reason("TIME_UNAVAILABLE");
                continue;
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
                risk_level: self.rule.risk_level,
            });
            self.result.identities.push(object);
            drop(lease);
        }
        if self.finished {
            self.frames.clear();
        }
        let mut snapshot = self.result.clone();
        snapshot.complete = snapshot.complete
            && self.finished
            && !snapshot.was_cancelled
            && snapshot.fatal_error.is_none();
        progress(
            snapshot.total_scanned_items,
            snapshot.candidates.len() as u64,
            snapshot.total_matched_bytes,
        );
        snapshot
    }
}
pub fn scan_directory<F: Fn(u64, u64, u64)>(
    root: &Path,
    rule: &CleanupRule,
    options: &ScannerOptions,
    cancel: Arc<AtomicBool>,
    progress: F,
) -> ScanResult {
    let mut session = ScanSession::new(root.to_owned(), rule.clone(), options.clone());
    let mut result = session.run_slice(&cancel, progress);
    // Legacy one-shot callers cannot continue a batch.
    if result.batch_ready {
        result.reached_limit = true;
        result.diagnostics.insert("CANDIDATE_LIMIT".into(), 1);
    }
    result
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
