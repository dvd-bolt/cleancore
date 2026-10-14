use super::*;
fn operation(id: &str, preview: &str) -> CleanupOutcome {
    CleanupOutcome {
        operation_id: id.into(),
        preview_id: preview.into(),
        category: "user-temp-v1".into(),
        started_at_ms: 1000,
        status: CleanupOperationStatus::Running,
        items: vec![CleanupItemResult {
            object_id: "object-1".into(),
            display_path: "owned-test-file.tmp".into(),
            size_bytes: "42".into(),
            status: CleanupItemStatus::Pending,
            reason_code: "INTENT".into(),
        }],
        recycled_bytes: "0".into(),
        journal_ok: true,
    }
}
#[test]
fn enforces_unique_preview_and_terminal_state() {
    let store = StorageManager::in_memory().unwrap();
    let mut o = operation("first", "preview");
    store.begin(&o).unwrap();
    assert!(store.begin(&operation("second", "preview")).is_err());
    o.items[0].status = CleanupItemStatus::Skipped;
    o.status = CleanupOperationStatus::Completed;
    store.update(&o).unwrap();
    assert_eq!(store.read("first").unwrap().unwrap(), o);
    assert!(store.update(&o).is_err());
}
#[test]
fn failed_intent_transaction_leaves_no_row() {
    let store = StorageManager::in_memory().unwrap();
    store.lock_conn().unwrap().execute_batch("CREATE TRIGGER reject_intent BEFORE INSERT ON cleanup_operations BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
    assert!(store.begin(&operation("first", "preview")).is_err());
    assert!(store.read("first").unwrap().is_none());
}

#[test]
fn failed_result_write_preserves_pending_intent() {
    let store = StorageManager::in_memory().unwrap();
    let mut o = operation("first", "preview");
    store.begin(&o).unwrap();
    store.lock_conn().unwrap().execute_batch("CREATE TRIGGER reject_result BEFORE UPDATE ON cleanup_operations BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
    o.items[0].status = CleanupItemStatus::Recycled;
    assert!(store.update(&o).is_err());
    let recorded = store.read("first").unwrap().unwrap();
    assert_eq!(recorded.items[0].status, CleanupItemStatus::Pending);
    assert_eq!(recorded.status, CleanupOperationStatus::Running);
}
#[test]
fn paginates_without_duplicate_or_missing_records() {
    let store = StorageManager::in_memory().unwrap();
    for n in 0..5 {
        store
            .begin(&operation(&format!("op-{n}"), &format!("preview-{n}")))
            .unwrap();
    }
    let first = store.audit_page(None, 2).unwrap();
    let second = store.audit_page(first.next_cursor, 2).unwrap();
    let third = store.audit_page(second.next_cursor, 2).unwrap();
    let ids: Vec<_> = first
        .operations
        .iter()
        .chain(&second.operations)
        .chain(&third.operations)
        .map(|o| o.operation_id.as_str())
        .collect();
    assert_eq!(ids, ["op-4", "op-3", "op-2", "op-1", "op-0"]);
    assert!(third.next_cursor.is_none());
}
#[test]
fn recovery_preserves_known_results_and_marks_pending_unknown() {
    let store = StorageManager::in_memory().unwrap();
    let mut o = operation("first", "preview");
    o.items.push(CleanupItemResult {
        object_id: "object-2".into(),
        ..o.items[0].clone()
    });
    store.begin(&o).unwrap();
    o.items[0].status = CleanupItemStatus::Recycled;
    o.recycled_bytes = "42".into();
    store.update(&o).unwrap();
    store.init_audit().unwrap();
    let recovered = store.read("first").unwrap().unwrap();
    assert_eq!(recovered.status, CleanupOperationStatus::Interrupted);
    assert_eq!(recovered.items[0].status, CleanupItemStatus::Recycled);
    assert_eq!(recovered.items[1].status, CleanupItemStatus::Unknown);
    assert_eq!(recovered.recycled_bytes, "42");
    assert!(store.begin(&operation("new", "preview")).is_err());
}
#[cfg(windows)]
#[test]
fn persistent_restart_and_single_owner_lock() {
    let filename = format!(
        "cleancore-stage4-db-test-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(&filename);
    let lock_path = path.with_file_name(format!("{filename}.cleanup.lock"));
    let store = StorageManager::open(&path).unwrap();
    assert!(store.is_persistent());
    store.begin(&operation("first", "preview")).unwrap();
    assert!(StorageManager::open(&path).is_err());
    drop(store);
    let reopened = StorageManager::open(&path).unwrap();
    assert_eq!(
        reopened.read("first").unwrap().unwrap().status,
        CleanupOperationStatus::Interrupted
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(lock_path).unwrap();
}
