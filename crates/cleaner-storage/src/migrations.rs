//! Управление версионированием схемы данных SQLite через PRAGMA user_version и запуск миграций.

use crate::recovery::simple_utc_now_iso;
use crate::StorageError;
use rusqlite::{params, Connection};
use tracing::info;

/// Описание отдельного шага миграции схемы SQLite.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub sql: &'static str,
}

/// Список всех миграций схемы базы данных CleanCore.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "Базовая схема: таблицы settings и cleanup_operations с индексами",
        sql: r#"
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS cleanup_operations (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                operation_id TEXT NOT NULL UNIQUE,
                preview_id TEXT NOT NULL UNIQUE,
                state TEXT NOT NULL,
                document TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_cleanup_ops_state ON cleanup_operations(state);
            CREATE INDEX IF NOT EXISTS idx_cleanup_ops_seq ON cleanup_operations(seq DESC);
        "#,
    },
    Migration {
        version: 2,
        description: "Журнал применённых миграций схемы: таблица schema_migrations",
        sql: r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at_utc TEXT NOT NULL,
                description TEXT NOT NULL
            );
        "#,
    },
];

/// Возвращает текущее значение `PRAGMA user_version`.
pub fn get_user_version(conn: &Connection) -> Result<i32, StorageError> {
    let version: i32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    Ok(version)
}

/// Устанавливает новое значение `PRAGMA user_version`.
pub fn set_user_version(conn: &Connection, version: i32) -> Result<(), StorageError> {
    conn.execute(&format!("PRAGMA user_version = {version};"), [])?;
    Ok(())
}

/// Выполняет быструю проверку целостности базы данных (`PRAGMA quick_check;`).
/// Возвращает ошибку, если база данных повреждена.
pub fn verify_integrity(conn: &Connection) -> Result<(), StorageError> {
    let result: String = conn.query_row("PRAGMA quick_check;", [], |row| row.get(0))?;
    if result.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(StorageError::CorruptDatabase(format!(
            "PRAGMA quick_check вернул ошибку: {result}"
        )))
    }
}

/// Применяет все недостающие миграции к базе данных в транзакциях.
/// Возвращает актуальный номер версии схемы.
pub fn apply_migrations(conn: &mut Connection) -> Result<i32, StorageError> {
    verify_integrity(conn)?;

    let current_version = get_user_version(conn)?;
    let latest_version = MIGRATIONS.last().map(|m| m.version).unwrap_or(0);

    if current_version > latest_version {
        return Err(StorageError::UnsupportedSchemaVersion(current_version));
    }

    if current_version == latest_version {
        return Ok(current_version);
    }

    info!(
        "Текущая версия схемы SQLite: {current_version}, целевая: {latest_version}. Запуск миграций..."
    );

    for migration in MIGRATIONS {
        if migration.version > current_version {
            let tx = conn.transaction()?;
            tx.execute_batch(migration.sql)?;

            // Если миграция 2 или выше, фиксируем факт применения в schema_migrations
            if migration.version >= 2 {
                let now_utc = simple_utc_now_iso();
                // Также вносим запись о базовой миграции v1, если её ещё нет
                let _ = tx.execute(
                    "INSERT OR IGNORE INTO schema_migrations (version, applied_at_utc, description)
                     VALUES (?1, ?2, ?3);",
                    params![1, &now_utc, MIGRATIONS[0].description],
                );
                tx.execute(
                    "INSERT OR REPLACE INTO schema_migrations (version, applied_at_utc, description)
                     VALUES (?1, ?2, ?3);",
                    params![migration.version, &now_utc, migration.description],
                )?;
            }

            tx.commit()?;
            set_user_version(conn, migration.version)?;
            info!(
                "Применена миграция версии {}: {}",
                migration.version, migration.description
            );
        }
    }

    Ok(latest_version)
}

/// Возвращает список всех применённых миграций из таблицы `schema_migrations`.
pub fn get_applied_migrations(
    conn: &Connection,
) -> Result<Vec<(i32, String, String)>, StorageError> {
    let has_table: bool = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='schema_migrations';",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|cnt| cnt > 0)
        .unwrap_or(false);

    if !has_table {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT version, applied_at_utc, description FROM schema_migrations ORDER BY version ASC;",
    )?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}
