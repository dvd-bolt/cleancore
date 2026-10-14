use cleaner_storage::migrations::{apply_migrations, get_applied_migrations, get_user_version};
use cleaner_storage::{StorageError, StorageManager};
use rusqlite::Connection;
use std::path::PathBuf;

fn temp_db_path(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("cleaner_test_mig_{}_{}", name, std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{name}.db"))
}

#[test]
fn test_migrations_runner_advances_user_version() {
    let db_path = temp_db_path("advances_user_version");
    if db_path.exists() {
        let _ = std::fs::remove_file(&db_path);
    }

    let storage = StorageManager::open(&db_path).expect("Failed to open storage");
    assert_eq!(storage.current_schema_version().unwrap(), 2);
    assert!(!storage.was_recovered());

    // Проверяем прямое состояние в SQLite
    let conn = storage.lock_conn().unwrap();
    assert_eq!(get_user_version(&conn).unwrap(), 2);

    // Проверяем наличие таблицы schema_migrations и записей
    let migrations = get_applied_migrations(&conn).unwrap();
    assert_eq!(migrations.len(), 2);
    assert_eq!(migrations[0].0, 1);
    assert_eq!(migrations[1].0, 2);

    // Проверяем наличие индексов
    let index_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='index' AND name IN ('idx_cleanup_ops_state', 'idx_cleanup_ops_seq');",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(index_count, 2);

    drop(conn);
    drop(storage);
    let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
}

#[test]
fn test_migration_preserves_existing_legacy_data() {
    let db_path = temp_db_path("preserves_legacy");
    if db_path.exists() {
        let _ = std::fs::remove_file(&db_path);
    }

    // Создаём сырую legacy-базу v0.0.3 (user_version = 0) с реальными данными
    {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "PRAGMA user_version = 0;
            CREATE TABLE settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE cleanup_operations (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                operation_id TEXT NOT NULL UNIQUE,
                preview_id TEXT NOT NULL UNIQUE,
                state TEXT NOT NULL,
                document TEXT NOT NULL
            );
            INSERT INTO settings (key, value) VALUES ('theme', 'studio-pearl-legacy');
            INSERT INTO cleanup_operations (operation_id, preview_id, state, document)
            VALUES ('op-legacy-1', 'prev-legacy-1', 'Completed', '{\"status\":\"Completed\",\"items\":[]}');",
        )
        .unwrap();
    }

    // Открываем через StorageManager: мигратор должен наложить индексы, создать schema_migrations и поднять версию до 2
    let storage = StorageManager::open(&db_path).expect("Failed to open legacy storage");
    assert_eq!(storage.current_schema_version().unwrap(), 2);
    assert!(!storage.was_recovered());

    // Проверяем сохранность данных
    assert_eq!(
        storage.get_setting("theme").unwrap(),
        Some("studio-pearl-legacy".to_string())
    );

    let conn = storage.lock_conn().unwrap();
    let op_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM cleanup_operations WHERE operation_id = 'op-legacy-1';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        op_count, 1,
        "Журнал аудита не должен быть повреждён или утерян при миграциях"
    );

    drop(conn);
    drop(storage);
    let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
}

#[test]
fn test_unsupported_future_schema_version_rejected() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA user_version = 99;").unwrap();

    let result = apply_migrations(&mut conn);
    match result {
        Err(StorageError::UnsupportedSchemaVersion(ver)) => {
            assert_eq!(ver, 99);
        }
        other => panic!("Expected UnsupportedSchemaVersion(99), got {:?}", other),
    }
}
