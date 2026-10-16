use super::*;
use cleaner_core::cleanup::CleanupJournal;
use cleaner_execution::{
    native::{capture_candidate, BoundDelete},
    ActionResult, ObjectAction,
};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::AtomicUsize,
    time::{Duration, Instant, SystemTime},
};

#[derive(Default)]
struct Journal {
    rows: Mutex<HashMap<String, CleanupOutcome>>,
    fail: AtomicBool,
}
impl CleanupJournal for Journal {
    fn is_persistent(&self) -> bool {
        true
    }
    fn begin(&self, outcome: &CleanupOutcome) -> Result<(), String> {
        self.rows
            .lock()
            .unwrap()
            .insert(outcome.operation_id.clone(), outcome.clone());
        Ok(())
    }
    fn update(&self, outcome: &CleanupOutcome) -> Result<(), String> {
        if self.fail.load(Ordering::Acquire) {
            return Err("fixture journal failure".into());
        }
        self.rows
            .lock()
            .unwrap()
            .insert(outcome.operation_id.clone(), outcome.clone());
        Ok(())
    }
    fn read(&self, id: &str) -> Result<Option<CleanupOutcome>, String> {
        Ok(self.rows.lock().unwrap().get(id).cloned())
    }
}
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("cleancore-general-owned-{}", id()));
        fs::create_dir(&root).unwrap();
        Self { root }
    }
    fn file(&self, n: usize) -> TrustedCandidate {
        let name = format!("{n}.tmp");
        let path = self.root.join(&name);
        fs::write(&path, b"fixture").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(90 * 86400))
            .unwrap();
        capture_candidate(&self.root, std::path::Path::new(&name)).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn setup(action: Arc<dyn ObjectAction>, journal: Arc<Journal>) -> Arc<GeneralManager> {
    Arc::new(GeneralManager::new(Some(Arc::new(CleanupService::new(
        journal, action,
    )))))
}
fn finished(manager: &GeneralManager, job: &str) -> GeneralJob {
    let start = Instant::now();
    loop {
        let value = manager.job(job).unwrap();
        if value.status != "Running" {
            return value;
        }
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn deduplicates_physical_identity_excludes_preview_only_and_freezes_selection() {
    let fixture = Fixture::new();
    let a = fixture.file(0);
    let b = fixture.file(1);
    let manager = GeneralManager::new(None);
    let session = manager.create().unwrap();
    manager
        .collect(
            &session,
            "temp-0".into(),
            "user-temp-v1",
            vec![a.clone(), b],
            false,
        )
        .unwrap();
    let same = capture_candidate(&fixture.root, std::path::Path::new("0.tmp")).unwrap();
    assert_ne!(a.object_id(), same.object_id());
    manager
        .collect(
            &session,
            "dump-0".into(),
            "crash-dumps-v1",
            vec![same],
            false,
        )
        .unwrap();
    manager
        .collect(
            &session,
            "logs-0".into(),
            "user-temp-logs-v1",
            vec![fixture.file(2)],
            false,
        )
        .unwrap();
    manager
        .collect(
            &session,
            "temp-0".into(),
            "user-temp-v1",
            vec![fixture.file(3)],
            false,
        )
        .unwrap();
    let index = manager.finish(&session, false, &[]).unwrap();
    assert_eq!(index.items.len(), 2);
    assert_eq!(index.total_bytes, "14");
    let selected = vec![a.object_id().into()];
    let page = manager.page(&session, 0, 50, "", Some(&selected)).unwrap();
    assert_eq!(page.total, 1);
    assert!(manager
        .prepare(&session, vec!["forged".into()], false)
        .is_err());
    assert!(manager
        .prepare(
            &session,
            vec![a.object_id().into(), a.object_id().into()],
            false
        )
        .is_err());
    assert_eq!(
        manager
            .prepare(&session, selected, false)
            .unwrap()
            .selected_count,
        1
    );
    assert!(manager.reject_category(&session, "user-temp-v1").is_err());
    assert!(manager
        .finish(&session, false, &["user-temp-v1".into()])
        .is_err());
}

#[test]
fn incomplete_failed_cancelled_and_expired_snapshots_do_not_silently_authorize() {
    let fixture = Fixture::new();
    let manager = GeneralManager::new(None);
    let session = manager.create().unwrap();
    let a = fixture.file(0);
    manager
        .collect(&session, "0".into(), "user-temp-v1", vec![a.clone()], true)
        .unwrap();
    manager
        .collect(
            &session,
            "1".into(),
            "crash-dumps-v1",
            vec![fixture.file(1)],
            false,
        )
        .unwrap();
    let index = manager
        .finish(&session, false, &["crash-dumps-v1".into()])
        .unwrap();
    assert_eq!(index.items.len(), 1);
    assert!(index.partial);
    assert_eq!(
        manager
            .prepare(&session, vec![a.object_id().into()], false)
            .unwrap_err()
            .code,
        "PARTIAL_ACK_REQUIRED"
    );
    manager
        .prepare(&session, vec![a.object_id().into()], true)
        .unwrap();
    manager.lock().unwrap().snapshot.as_mut().unwrap().expires = 0;
    assert!(manager
        .prepare(&session, vec![a.object_id().into()], true)
        .is_err());
    let session = manager.create().unwrap();
    manager
        .collect(&session, "0".into(), "user-temp-v1", vec![a.clone()], false)
        .unwrap();
    manager.finish(&session, true, &[]).unwrap();
    assert!(manager
        .prepare(&session, vec![a.object_id().into()], true)
        .is_err());
}

#[test]
fn real_executor_deletes_only_selected_unchanged_fixture_and_rejects_replay() {
    let fixture = Fixture::new();
    let a = fixture.file(0);
    let b = fixture.file(1);
    let c = fixture.file(2);
    let manager = setup(Arc::new(BoundDelete), Arc::new(Journal::default()));
    let session = manager.create().unwrap();
    manager
        .collect(
            &session,
            "0".into(),
            "user-temp-v1",
            vec![a.clone(), b.clone(), c],
            false,
        )
        .unwrap();
    manager.finish(&session, false, &[]).unwrap();
    let confirmation = manager
        .prepare(
            &session,
            vec![a.object_id().into(), b.object_id().into()],
            false,
        )
        .unwrap();
    manager
        .lock()
        .unwrap()
        .confirmation
        .as_mut()
        .unwrap()
        .dto
        .expires_at_ms = 0;
    assert_eq!(
        manager
            .begin(&confirmation.confirmation_id, true)
            .unwrap_err()
            .code,
        "CONFIRMATION_EXPIRED"
    );
    let confirmation = manager
        .prepare(
            &session,
            vec![a.object_id().into(), b.object_id().into()],
            false,
        )
        .unwrap();
    fs::write(fixture.root.join("1.tmp"), b"changed after preview").unwrap();
    assert_eq!(
        manager
            .begin(&confirmation.confirmation_id, false)
            .unwrap_err()
            .code,
        "CONFIRMATION_REQUIRED"
    );
    assert!(fixture.root.join("0.tmp").exists());
    let job = manager.begin(&confirmation.confirmation_id, true).unwrap();
    assert!(manager.begin(&confirmation.confirmation_id, true).is_err());
    let result = finished(&manager, &job.job_id);
    assert_eq!(result.status, "Completed");
    assert_eq!(result.deleted, 1);
    assert_eq!(result.skipped, 1);
    assert!(!fixture.root.join("0.tmp").exists());
    assert!(fixture.root.join("1.tmp").exists());
    assert!(fixture.root.join("2.tmp").exists());
}

struct FakeDelete {
    journal: Arc<Journal>,
    calls: AtomicUsize,
    after: Box<dyn Fn() + Send + Sync>,
}

#[test]
fn cancellation_stops_remaining_objects_and_later_categories() {
    let fixture = Fixture::new();
    let journal = Arc::new(Journal::default());
    let manager_ref: Arc<Mutex<Option<std::sync::Weak<GeneralManager>>>> =
        Arc::new(Mutex::new(None));
    let callback = manager_ref.clone();
    let action = Arc::new(FakeDelete {
        journal: journal.clone(),
        calls: AtomicUsize::new(0),
        after: Box::new(move || {
            callback
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .upgrade()
                .unwrap()
                .cancel_all();
        }),
    });
    let manager = setup(action.clone(), journal);
    *manager_ref.lock().unwrap() = Some(Arc::downgrade(&manager));
    let session = manager.create().unwrap();
    manager
        .collect(
            &session,
            "temp".into(),
            "user-temp-v1",
            vec![fixture.file(0), fixture.file(1)],
            false,
        )
        .unwrap();
    manager
        .collect(
            &session,
            "crash".into(),
            "crash-dumps-v1",
            vec![fixture.file(2)],
            false,
        )
        .unwrap();
    let index = manager.finish(&session, false, &[]).unwrap();
    let confirmation = manager
        .prepare(
            &session,
            index.items.into_iter().map(|i| i.object_id).collect(),
            false,
        )
        .unwrap();
    let job = manager.begin(&confirmation.confirmation_id, true).unwrap();
    let result = finished(&manager, &job.job_id);
    assert_eq!(result.status, "Cancelled");
    assert_eq!(action.calls.load(Ordering::Acquire), 1);
    assert_eq!(result.processed, 1);
    assert_eq!(result.operation_ids.len(), 1);
}
impl ObjectAction for FakeDelete {
    fn available(&self) -> bool {
        true
    }
    fn irreversible(&self) -> bool {
        true
    }
    fn apply(&self, object: &TrustedCandidate) -> ActionResult {
        assert!(self
            .journal
            .rows
            .lock()
            .unwrap()
            .values()
            .any(|o| o.items.iter().any(
                |i| i.object_id == object.object_id() && i.status == CleanupItemStatus::Pending
            )));
        self.calls.fetch_add(1, Ordering::AcqRel);
        (self.after)();
        ActionResult::Deleted
    }
}
#[test]
fn executes_1005_selected_objects_in_two_bounded_journalled_operations() {
    let fixture = Fixture::new();
    let objects: Vec<_> = (0..1005).map(|n| fixture.file(n)).collect();
    let journal = Arc::new(Journal::default());
    let action = Arc::new(FakeDelete {
        journal: journal.clone(),
        calls: AtomicUsize::new(0),
        after: Box::new(|| {}),
    });
    let manager = setup(action.clone(), journal.clone());
    let session = manager.create().unwrap();
    manager
        .collect(&session, "0".into(), "user-temp-v1", objects, false)
        .unwrap();
    let index = manager.finish(&session, false, &[]).unwrap();
    let confirmation = manager
        .prepare(
            &session,
            index.items.iter().map(|i| i.object_id.clone()).collect(),
            false,
        )
        .unwrap();
    let job = manager.begin(&confirmation.confirmation_id, true).unwrap();
    let result = finished(&manager, &job.job_id);
    assert_eq!(result.status, "Completed");
    assert_eq!(result.deleted, 1005);
    assert_eq!(result.processed, 1005);
    assert_eq!(result.operation_ids.len(), 2);
    assert_eq!(action.calls.load(Ordering::Acquire), 1005);
    assert!(journal
        .rows
        .lock()
        .unwrap()
        .values()
        .all(|o| o.items.len() <= MAX_CANDIDATES));
}

#[test]
fn journal_failure_stops_the_entire_selection_without_retry() {
    let fixture = Fixture::new();
    let journal = Arc::new(Journal::default());
    journal.fail.store(true, Ordering::Release);
    let action = Arc::new(FakeDelete {
        journal: journal.clone(),
        calls: AtomicUsize::new(0),
        after: Box::new(|| {}),
    });
    let manager = setup(action.clone(), journal);
    let session = manager.create().unwrap();
    manager
        .collect(
            &session,
            "0".into(),
            "user-temp-v1",
            vec![fixture.file(0), fixture.file(1)],
            false,
        )
        .unwrap();
    let index = manager.finish(&session, false, &[]).unwrap();
    let confirmation = manager
        .prepare(
            &session,
            index.items.into_iter().map(|i| i.object_id).collect(),
            false,
        )
        .unwrap();
    let job = manager.begin(&confirmation.confirmation_id, true).unwrap();
    let result = finished(&manager, &job.job_id);
    assert_eq!(result.status, "Interrupted");
    assert_eq!(action.calls.load(Ordering::Acquire), 1);
    assert_eq!(result.failed, 1);
}
