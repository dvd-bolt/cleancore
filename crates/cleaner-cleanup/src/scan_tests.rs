use super::*;
use crate::{rules::CleanupRule, scanner::scan_directory};
use cleaner_core::cleanup::{CleanupItemStatus, CleanupJournal};
use cleaner_execution::{
    native::{revalidate, TrustedCandidate},
    ActionResult, ObjectAction,
};
use std::{fs, path::Path, time::SystemTime};

struct Fixture {
    base: PathBuf,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base =
            std::env::temp_dir().join(format!("cleancore-integration-test-{}", Uuid::new_v4()));
        let root = base.join("Temp");
        fs::create_dir_all(&root).unwrap();
        Self { base, root }
    }
    fn file(&self, name: &str, old: bool) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, b"owned fixture").unwrap();
        if old {
            fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(SystemTime::now() - Duration::from_secs(8 * 24 * 3600))
                .unwrap();
        }
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let (Ok(base), Ok(temp)) = (
            self.base.canonicalize(),
            std::env::temp_dir().canonicalize(),
        ) {
            if base.parent() == Some(temp.as_path())
                && base
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("cleancore-integration-test-")
            {
                let _ = fs::remove_dir_all(base);
            }
        }
    }
}
fn wait(manager: &TaskManager, id: &str) -> TaskStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = manager.get_task_status(id).unwrap().unwrap();
        if status.state != TaskState::Running {
            return status;
        }
        assert!(Instant::now() < deadline, "scan did not finish");
        thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn rejects_unknown_category_and_invalid_paging_without_panics() {
    let manager = TaskManager::new();
    assert!(manager.start_scan("invented").is_err());
    assert!(manager
        .get_preview_page("unknown", usize::MAX, 0, "")
        .is_err());
    assert!(manager.get_preview_page("unknown", 0, 101, "").is_err());
    assert!(manager
        .get_preview_page("unknown", 0, 10, &"a".repeat(513))
        .is_err());
}
#[cfg(windows)]
#[test]
fn respects_age_rule_and_never_changes_file_contents() {
    let f = Fixture::new();
    let old = f.file("старый.tmp", true);
    let fresh = f.file("fresh.tmp", false);
    let result = scan_directory(
        &f.root,
        &CleanupRule::user_temp_v1(),
        &ScannerOptions::default(),
        Arc::new(AtomicBool::new(false)),
        |_, _, _| {},
    );
    assert!(result.complete);
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.identities.len(), 1);
    assert_eq!(
        result.candidates[0].obj_id,
        result.identities[0].object_id()
    );
    assert_eq!(fs::read(old).unwrap(), b"owned fixture");
    assert_eq!(fs::read(fresh).unwrap(), b"owned fixture");
}
#[cfg(windows)]
#[test]
fn limits_depth_entries_candidates_and_time_are_partial() {
    let f = Fixture::new();
    for i in 0..4 {
        f.file(&format!("{i}.tmp"), true);
    }
    for options in [
        ScannerOptions {
            max_scanned_files: 1,
            ..ScannerOptions::default()
        },
        ScannerOptions {
            max_candidates: 1,
            ..ScannerOptions::default()
        },
        ScannerOptions {
            max_duration: Duration::ZERO,
            ..ScannerOptions::default()
        },
    ] {
        let result = scan_directory(
            &f.root,
            &CleanupRule::user_temp_v1(),
            &options,
            Arc::new(AtomicBool::new(false)),
            |_, _, _| {},
        );
        assert!(!result.complete);
        assert!(result.reached_limit);
        assert!(!result.diagnostics.is_empty());
    }
    fs::create_dir(f.root.join("child")).unwrap();
    fs::write(f.root.join("child/file.tmp"), b"owned fixture").unwrap();
    let result = scan_directory(
        &f.root,
        &CleanupRule::user_temp_v1(),
        &ScannerOptions {
            max_depth: 0,
            ..ScannerOptions::default()
        },
        Arc::new(AtomicBool::new(false)),
        |_, _, _| {},
    );
    assert!(!result.complete);
    assert!(result.diagnostics.contains_key("DEPTH_LIMIT"));
}
#[cfg(windows)]
#[test]
fn cancellation_during_traversal_stops_and_keeps_results_partial() {
    let f = Fixture::new();
    for i in 0..200 {
        f.file(&format!("{i}.tmp"), true);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let result = scan_directory(
        &f.root,
        &CleanupRule::user_temp_v1(),
        &ScannerOptions::default(),
        cancel,
        move |count, _, _| {
            if count >= 100 {
                signal.store(true, Ordering::Release);
            }
        },
    );
    assert!(result.was_cancelled);
    assert!(!result.complete);
    assert_eq!(result.total_scanned_items, 100);
}
#[cfg(windows)]
#[test]
fn hard_links_busy_files_and_missing_roots_are_visible() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let path = f.file("hard.tmp", true);
    fs::hard_link(path, f.root.join("alias.tmp")).unwrap();
    let busy = f.file("busy.tmp", true);
    let _lock = fs::OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(busy)
        .unwrap();
    let result = scan_directory(
        &f.root,
        &CleanupRule::user_temp_v1(),
        &ScannerOptions::default(),
        Arc::new(AtomicBool::new(false)),
        |_, _, _| {},
    );
    assert!(result.candidates.is_empty());
    assert_eq!(result.diagnostics["HARD_LINK"], 2);
    assert!(result.diagnostics.contains_key("FILE_IN_USE"));
    let result = scan_directory(
        &f.root.join("missing"),
        &CleanupRule::user_temp_v1(),
        &ScannerOptions::default(),
        Arc::new(AtomicBool::new(false)),
        |_, _, _| {},
    );
    assert!(!result.complete);
    assert!(result.fatal_error.is_some());
}
#[cfg(windows)]
#[test]
fn directory_lease_blocks_parent_rename_at_controlled_point() {
    let f = Fixture::new();
    let root = f.root.clone();
    let renamed = f.base.join("replaced");
    for i in 0..110 {
        f.file(&format!("{i}.tmp"), false);
    }
    let result = scan_directory(
        &f.root,
        &CleanupRule::user_temp_v1(),
        &ScannerOptions::default(),
        Arc::new(AtomicBool::new(false)),
        move |count, _, _| {
            if count == 100 {
                assert!(fs::rename(&root, &renamed).is_err());
            }
        },
    );
    assert!(result.complete);
    assert!(f.root.exists());
}
#[cfg(windows)]
#[test]
fn filters_entire_snapshot_before_pagination_and_bounds_offset() {
    let f = Fixture::new();
    for i in 0..20 {
        f.file(&format!("{i:02}-old.tmp"), true);
    }
    let m = TaskManager::new();
    let id = m
        .start_at_root("user-temp-v1", f.root.clone(), ScannerOptions::default())
        .unwrap();
    assert_eq!(wait(&m, &id).state, TaskState::Completed);
    let page = m.get_preview_page(&id, 0, 5, "19-old").unwrap();
    assert_eq!(page.total_candidates, 1);
    assert_eq!(page.unfiltered_candidates, 20);
    assert!(page.items[0].display_path.contains("19-old"));
    assert!(m
        .get_preview_page(&id, usize::MAX, 5, "")
        .unwrap()
        .items
        .is_empty());
}
#[cfg(windows)]
#[test]
fn partial_and_failed_scans_never_publish_execution_preview() {
    let f = Fixture::new();
    for i in 0..4 {
        f.file(&format!("{i}.tmp"), true);
    }
    let service = Arc::new(CleanupService::new(
        Arc::new(cleaner_storage::StorageManager::in_memory().unwrap()),
        Arc::new(cleaner_execution::native::DisabledRecycle),
    ));
    let m = TaskManager::with_execution(service.clone());
    let id = m
        .start_at_root(
            "user-temp-v1",
            f.root.clone(),
            ScannerOptions {
                max_candidates: 1,
                ..ScannerOptions::default()
            },
        )
        .unwrap();
    assert_eq!(wait(&m, &id).state, TaskState::Partial);
    let page = m.get_preview_page(&id, 0, 5, "").unwrap();
    assert!(!page.complete);
    assert!(page.execution_preview_id.is_none());
    assert!(service.latest_preview().unwrap().is_none());
    let id = m
        .start_at_root(
            "user-temp-v1",
            f.root.join("missing"),
            ScannerOptions::default(),
        )
        .unwrap();
    assert_eq!(wait(&m, &id).state, TaskState::Failed);
}
struct ReadOnlyAction;
impl ObjectAction for ReadOnlyAction {
    fn available(&self) -> bool {
        true
    }
    fn apply(&self, c: &TrustedCandidate) -> ActionResult {
        ActionResult::Skipped(revalidate(c).err().unwrap_or("READ_ONLY_TEST").into())
    }
}
#[cfg(windows)]
#[test]
fn scan_to_confirmation_revalidation_and_persistent_journal_uses_same_ids() {
    let f = Fixture::new();
    let path = f.file("old.tmp", true);
    let journal = Arc::new(cleaner_storage::StorageManager::open(f.base.join("audit.db")).unwrap());
    let service = Arc::new(CleanupService::new(
        journal.clone(),
        Arc::new(ReadOnlyAction),
    ));
    let m = TaskManager::with_execution(service.clone());
    let id = m
        .start_at_root("user-temp-v1", f.root.clone(), ScannerOptions::default())
        .unwrap();
    assert_eq!(wait(&m, &id).state, TaskState::Completed);
    let page = m.get_preview_page(&id, 0, 5, "").unwrap();
    let execution_id = page.execution_preview_id.unwrap();
    let native = service.preview(&execution_id).unwrap().unwrap();
    assert_eq!(native.candidates[0].object_id, page.items[0].obj_id);
    let confirmation = service
        .prepare_confirmation(&execution_id, vec![page.items[0].obj_id.clone()])
        .unwrap();
    fs::write(&path, b"modified fixture with different length").unwrap();
    let started = service.begin(&confirmation.confirmation_id, true).unwrap();
    let outcome = service.run(&started.operation_id).unwrap();
    assert_eq!(outcome.items[0].status, CleanupItemStatus::Skipped);
    assert_eq!(outcome.items[0].reason_code, "OBJECT_CHANGED");
    assert_eq!(
        journal.read(&outcome.operation_id).unwrap().unwrap(),
        outcome
    );
    assert!(path.exists());
}
#[cfg(windows)]
#[test]
fn junction_is_excluded_without_following_target() {
    use std::os::windows::process::CommandExt;
    let f = Fixture::new();
    let outside = f.base.join("outside");
    fs::create_dir(&outside).unwrap();
    let secret = outside.join("owned-outside.tmp");
    fs::write(&secret, b"outside owned fixture").unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&secret)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(8 * 24 * 3600))
        .unwrap();
    let link = f.root.join("junction");
    // Test-only fixed PowerShell invocation creates an owned junction fixture.
    // Product code has no shell invocation and receives no paths from frontend.
    let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let result=std::process::Command::new(powershell).creation_flags(0x08000000).args(["-NoProfile","-NonInteractive","-Command","New-Item -ItemType Junction -Path $env:CC_LINK -Target $env:CC_TARGET -ErrorAction Stop | Out-Null"]).env("CC_LINK",&link).env("CC_TARGET",&outside).output().unwrap();
    assert!(
        result.status.success(),
        "junction setup failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = scan_directory(
        &f.root,
        &CleanupRule::user_temp_v1(),
        &ScannerOptions::default(),
        Arc::new(AtomicBool::new(false)),
        |_, _, _| {},
    );
    assert!(result.candidates.is_empty());
    assert!(result.diagnostics.contains_key("REPARSE_OR_UNAVAILABLE"));
    assert!(cleaner_execution::native::lock_directory(&link, Path::new("")).is_err());
    fs::remove_dir(link).unwrap();
    assert_eq!(fs::read(secret).unwrap(), b"outside owned fixture");
}
