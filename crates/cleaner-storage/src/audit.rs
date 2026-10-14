//! Durable intents, bounded history and conservative crash recovery.
use crate::{StorageError, StorageManager};
use cleaner_core::cleanup::*;
use rusqlite::{params, OptionalExtension};

impl StorageManager {
    pub(crate) fn init_audit(&self) -> Result<(), StorageError> {
        let mut conn = self.lock_conn()?;
        conn.execute_batch(
            "PRAGMA synchronous = FULL;
            CREATE TABLE IF NOT EXISTS cleanup_operations (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                operation_id TEXT NOT NULL UNIQUE,
                preview_id TEXT NOT NULL UNIQUE,
                state TEXT NOT NULL,
                document TEXT NOT NULL
            );",
        )?;
        let tx = conn.transaction()?;
        let interrupted: Vec<(String, String)> = {
            let mut stmt = tx.prepare(
                "SELECT operation_id, document FROM cleanup_operations WHERE state = 'Running'",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        for (operation_id, json) in interrupted {
            let mut operation: CleanupOutcome = serde_json::from_str(&json)?;
            operation.status = CleanupOperationStatus::Interrupted;
            for item in &mut operation.items {
                if item.status == CleanupItemStatus::Pending {
                    item.status = CleanupItemStatus::Unknown;
                    item.reason_code = "INTERRUPTED_BEFORE_RESULT".into();
                }
            }
            tx.execute("UPDATE cleanup_operations SET state = 'Interrupted', document = ?1 WHERE operation_id = ?2",
                params![serde_json::to_string(&operation)?, operation_id])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn audit_page(
        &self,
        before: Option<i64>,
        limit: u32,
    ) -> Result<CleanupAuditPage, StorageError> {
        let conn = self.lock_conn()?;
        let limit = limit.clamp(1, 100);
        let mut stmt = conn.prepare(
            "SELECT seq, document FROM cleanup_operations
            WHERE seq < ?1 ORDER BY seq DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![before.unwrap_or(i64::MAX), limit + 1], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut rows = rows.collect::<Result<Vec<_>, _>>()?;
        let more = rows.len() > limit as usize;
        if more {
            rows.pop();
        }
        let next_cursor = if more { rows.last().map(|r| r.0) } else { None };
        let operations = rows
            .into_iter()
            .map(|(_, doc)| serde_json::from_str(&doc))
            .collect::<Result<_, _>>()?;
        Ok(CleanupAuditPage {
            operations,
            next_cursor,
        })
    }
}

impl CleanupJournal for StorageManager {
    fn is_persistent(&self) -> bool {
        self.db_path() != std::path::Path::new(":memory:")
    }
    fn begin(&self, operation: &CleanupOutcome) -> Result<(), String> {
        let mut conn = self.lock_conn().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let json = serde_json::to_string(operation).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO cleanup_operations (operation_id, preview_id, state, document) VALUES (?1, ?2, 'Running', ?3)",
            params![operation.operation_id, operation.preview_id, json]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn update(&self, operation: &CleanupOutcome) -> Result<(), String> {
        let mut conn = self.lock_conn().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let state = format!("{:?}", operation.status);
        let json = serde_json::to_string(operation).map_err(|e| e.to_string())?;
        let count = tx.execute("UPDATE cleanup_operations SET state = ?1, document = ?2 WHERE operation_id = ?3 AND preview_id = ?4 AND state = 'Running'",
            params![state, json, operation.operation_id, operation.preview_id]).map_err(|e| e.to_string())?;
        if count != 1 {
            return Err("Операция не найдена или уже завершена".into());
        }
        tx.commit().map_err(|e| e.to_string())
    }
    fn read(&self, operation_id: &str) -> Result<Option<CleanupOutcome>, String> {
        let conn = self.lock_conn().map_err(|e| e.to_string())?;
        let json: Option<String> = conn
            .query_row(
                "SELECT document FROM cleanup_operations WHERE operation_id = ?1",
                [operation_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        json.map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            .transpose()
    }
}
