use super::*;
use native::ObjectIdentity;
use std::sync::atomic::AtomicUsize;
#[derive(Default)]
struct Journal {
    rows: Mutex<HashMap<String, CleanupOutcome>>,
    fail_begin: AtomicBool,
    fail_update: AtomicBool,
    persistent: AtomicBool,
}
impl CleanupJournal for Journal {
    fn is_persistent(&self) -> bool {
        self.persistent.load(Ordering::SeqCst)
    }
    fn begin(&self, o: &CleanupOutcome) -> Result<(), String> {
        if self.fail_begin.load(Ordering::SeqCst) {
            return Err("disk full".into());
        }
        let mut rows = self.rows.lock().unwrap();
        if rows.values().any(|r| r.preview_id == o.preview_id) {
            return Err("used".into());
        }
        rows.insert(o.operation_id.clone(), o.clone());
        Ok(())
    }
    fn update(&self, o: &CleanupOutcome) -> Result<(), String> {
        if self.fail_update.load(Ordering::SeqCst) {
            return Err("disk full".into());
        }
        self.rows
            .lock()
            .unwrap()
            .insert(o.operation_id.clone(), o.clone());
        Ok(())
    }
    fn read(&self, id: &str) -> Result<Option<CleanupOutcome>, String> {
        Ok(self.rows.lock().unwrap().get(id).cloned())
    }
}
struct Action {
    journal: Arc<Journal>,
    calls: AtomicUsize,
    after: Box<dyn Fn(usize) + Send + Sync>,
}
impl ObjectAction for Action {
    fn available(&self) -> bool {
        true
    }
    fn apply(&self, c: &TrustedCandidate) -> ActionResult {
        assert!(self.journal.rows.lock().unwrap().values().any(|o| o
            .items
            .iter()
            .any(|i| i.object_id == c.id && i.status == CleanupItemStatus::Pending)));
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        (self.after)(n);
        if n == 0 {
            ActionResult::Recycled
        } else {
            ActionResult::Skipped("OBJECT_CHANGED".into())
        }
    }
}
fn candidate(name: &str) -> TrustedCandidate {
    TrustedCandidate {
        id: name.into(),
        root: "C:\\test-only".into(),
        relative: name.into(),
        identity: ObjectIdentity {
            volume: 1,
            index: 1,
            size: 42,
            modified_ticks: 116_444_736_000_000_000,
        },
        parents: vec![],
    }
}
fn setup(
    after: impl Fn(usize) + Send + Sync + 'static,
) -> (
    Arc<CleanupService>,
    Arc<Journal>,
    Arc<Action>,
    CleanupPreview,
) {
    let j = Arc::new(Journal::default());
    j.persistent.store(true, Ordering::SeqCst);
    let a = Arc::new(Action {
        journal: j.clone(),
        calls: AtomicUsize::new(0),
        after: Box::new(after),
    });
    let s = Arc::new(CleanupService::new(j.clone(), a.clone()));
    let p = s
        .publish_preview(vec![candidate("a"), candidate("b")])
        .unwrap();
    (s, j, a, p)
}
fn confirmation(s: &CleanupService, p: &CleanupPreview) -> CleanupConfirmation {
    s.prepare_confirmation(&p.preview_id, vec!["a".into(), "b".into()])
        .unwrap()
}
#[test]
fn binds_selection_and_invalidates_old_confirmation() {
    let (s, _, _, p) = setup(|_| {});
    for ids in [
        vec![],
        vec!["a".into(), "a".into()],
        vec!["C:\\secret".into()],
    ] {
        assert!(s.prepare_confirmation(&p.preview_id, ids).is_err());
    }
    assert!(s.prepare_confirmation("unknown", vec!["a".into()]).is_err());
    let old = s
        .prepare_confirmation(&p.preview_id, vec!["a".into()])
        .unwrap();
    let new = s
        .prepare_confirmation(&p.preview_id, vec!["b".into()])
        .unwrap();
    assert_eq!(new.selected[0].object_id, "b");
    assert_eq!(new.selected_bytes, "42");
    assert!(s.begin(&old.confirmation_id, true).is_err());
    assert!(s.begin(&new.confirmation_id, false).is_err());
}
#[test]
fn expires_preview_and_confirmation() {
    let (s, _, _, _) = setup(|_| {});
    let p = s
        .publish_at(vec![candidate("old")], now_ms() - PREVIEW_TTL_MS - 1)
        .unwrap();
    assert!(s
        .prepare_confirmation(&p.preview_id, vec!["old".into()])
        .is_err());
    let p = s.publish_preview(vec![candidate("new")]).unwrap();
    let c = s
        .confirm_at(
            &p.preview_id,
            vec!["new".into()],
            now_ms() - CONFIRM_TTL_MS - 1,
        )
        .unwrap();
    assert_eq!(
        s.begin(&c.confirmation_id, true).unwrap_err().code,
        "CONFIRMATION_EXPIRED"
    );
}
#[test]
fn journals_intents_and_prevents_repeat() {
    let (s, j, a, p) = setup(|_| {});
    let c = confirmation(&s, &p);
    let running = s.begin(&c.confirmation_id, true).unwrap();
    assert_eq!(a.calls.load(Ordering::SeqCst), 0);
    let o = s.run(&running.operation_id).unwrap();
    assert_eq!(o.items[0].status, CleanupItemStatus::Recycled);
    assert_eq!(o.items[1].status, CleanupItemStatus::Skipped);
    assert_eq!(o.recycled_bytes, "42");
    assert_eq!(j.read(&o.operation_id).unwrap().unwrap(), o);
    assert!(s.run(&o.operation_id).is_err());
    assert!(s.begin(&c.confirmation_id, true).is_err());
    assert!(s
        .prepare_confirmation(&p.preview_id, vec!["a".into()])
        .is_err());
}
#[test]
fn failed_intent_never_starts_action() {
    let (s, j, a, p) = setup(|_| {});
    j.fail_begin.store(true, Ordering::SeqCst);
    let c = confirmation(&s, &p);
    assert_eq!(
        s.begin(&c.confirmation_id, true).unwrap_err().code,
        "JOURNAL_UNAVAILABLE"
    );
    assert_eq!(a.calls.load(Ordering::SeqCst), 0);
    assert!(s.begin(&c.confirmation_id, true).is_err());
}
#[test]
fn failed_result_stops_batch_and_is_unknown() {
    let (s, j, a, p) = setup(|_| {});
    j.fail_update.store(true, Ordering::SeqCst);
    let c = confirmation(&s, &p);
    let running = s.begin(&c.confirmation_id, true).unwrap();
    let o = s.run(&running.operation_id).unwrap();
    assert_eq!(o.status, CleanupOperationStatus::JournalFailed);
    assert_eq!(o.items[0].status, CleanupItemStatus::Unknown);
    assert_eq!(o.items[1].status, CleanupItemStatus::Cancelled);
    assert!(!o.journal_ok);
    assert_eq!(o.recycled_bytes, "0");
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn volatile_journal_and_disabled_adapter_block_execution() {
    let (s, j, a, p) = setup(|_| {});
    j.persistent.store(false, Ordering::SeqCst);
    let c = confirmation(&s, &p);
    assert_eq!(
        s.begin(&c.confirmation_id, true).unwrap_err().code,
        "EXECUTION_DISABLED"
    );
    assert_eq!(a.calls.load(Ordering::SeqCst), 0);
    j.persistent.store(true, Ordering::SeqCst);
    let blocked = CleanupService::new(j, Arc::new(native::DisabledRecycle));
    assert!(!blocked.availability().execution_enabled);
}
#[test]
fn cancellation_stops_future_actions_and_rejects_second_worker() {
    let entered = Arc::new(std::sync::Barrier::new(2));
    let released = Arc::new(std::sync::Barrier::new(2));
    let (e, r) = (entered.clone(), released.clone());
    let (s, _, a, p) = setup(move |n| {
        if n == 0 {
            e.wait();
            r.wait();
        }
    });
    let c = confirmation(&s, &p);
    let running = s.begin(&c.confirmation_id, true).unwrap();
    let (ws, oid) = (s.clone(), running.operation_id.clone());
    let worker = std::thread::spawn(move || ws.run(&oid).unwrap());
    entered.wait();
    assert_eq!(
        s.run(&running.operation_id).unwrap_err().code,
        "OPERATION_CLAIMED"
    );
    assert_eq!(s.begin(&c.confirmation_id, true).unwrap_err().code, "BUSY");
    assert!(s.cancel(&running.operation_id).unwrap());
    released.wait();
    let o = worker.join().unwrap();
    assert_eq!(o.status, CleanupOperationStatus::Cancelled);
    assert_eq!(o.items[0].status, CleanupItemStatus::Recycled);
    assert_eq!(o.items[1].status, CleanupItemStatus::Cancelled);
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn adapter_panic_becomes_unknown_without_retry() {
    let (s, _, a, p) = setup(|_| panic!("test failure"));
    let c = confirmation(&s, &p);
    let running = s.begin(&c.confirmation_id, true).unwrap();
    let o = s.run(&running.operation_id).unwrap();
    assert_eq!(o.status, CleanupOperationStatus::Interrupted);
    assert_eq!(o.items[0].status, CleanupItemStatus::Unknown);
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
}
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    struct Fixture {
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("cleancore-stage4-test-{}", id()));
            fs::create_dir(&root).unwrap();
            Self { root }
        }
        fn file(&self) -> PathBuf {
            let p = self.root.join("candidate.tmp");
            fs::write(&p, "owned test data").unwrap();
            p
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let (Ok(root), Ok(temp)) = (
                self.root.canonicalize(),
                std::env::temp_dir().canonicalize(),
            ) {
                if root.parent() == Some(temp.as_path())
                    && root
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("cleancore-stage4-test-")
                {
                    let _ = fs::remove_dir_all(root);
                }
            }
        }
    }
    #[test]
    fn detects_file_replacement_and_changed_content() {
        let f = Fixture::new();
        let p = f.file();
        let canonical =
            native::capture_candidate(&f.root.canonicalize().unwrap(), Path::new("candidate.tmp"))
                .unwrap();
        assert!(native::revalidate(&canonical).is_ok());
        let c = native::capture_candidate(&f.root, Path::new("candidate.tmp")).unwrap();
        assert!(native::revalidate(&c).is_ok());
        fs::rename(&p, f.root.join("old.tmp")).unwrap();
        fs::write(&p, "owned test data").unwrap();
        assert_eq!(native::revalidate(&c), Err("OBJECT_CHANGED"));
        let c = native::capture_candidate(&f.root, Path::new("candidate.tmp")).unwrap();
        fs::write(p, "different length").unwrap();
        assert_eq!(native::revalidate(&c), Err("OBJECT_CHANGED"));
    }
    #[test]
    fn rejects_busy_files_and_hard_links() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        let p = f.file();
        let lock = fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(&p)
            .unwrap();
        assert_eq!(
            native::capture_candidate(&f.root, Path::new("candidate.tmp")).unwrap_err(),
            "FILE_IN_USE"
        );
        drop(lock);
        fs::hard_link(p, f.root.join("alias.tmp")).unwrap();
        assert_eq!(
            native::capture_candidate(&f.root, Path::new("candidate.tmp")).unwrap_err(),
            "HARD_LINK"
        );
    }
    #[test]
    fn detects_parent_replacement() {
        let f = Fixture::new();
        fs::create_dir(f.root.join("parent")).unwrap();
        fs::write(f.root.join("parent/file.tmp"), "test").unwrap();
        let c = native::capture_candidate(&f.root, Path::new("parent/file.tmp")).unwrap();
        fs::rename(f.root.join("parent"), f.root.join("old-parent")).unwrap();
        fs::create_dir(f.root.join("parent")).unwrap();
        fs::write(f.root.join("parent/file.tmp"), "test").unwrap();
        assert_eq!(native::revalidate(&c), Err("OBJECT_CHANGED"));
    }
    #[test]
    fn rejects_traversal_and_alternate_streams() {
        let f = Fixture::new();
        for p in ["../secret", "a/../b", "stream:ads", ""] {
            assert_eq!(
                native::capture_candidate(&f.root, Path::new(p)).unwrap_err(),
                "INVALID_PATH"
            );
        }
    }
    #[test]
    #[ignore = "requires symlink privilege or Developer Mode; no elevation"]
    fn rejects_symlink() {
        let f = Fixture::new();
        let p = f.file();
        std::os::windows::fs::symlink_file(p, f.root.join("link.tmp")).unwrap();
        assert_eq!(
            native::capture_candidate(&f.root, Path::new("link.tmp")).unwrap_err(),
            "REPARSE_POINT"
        );
    }
}
