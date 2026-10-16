//! Хранилище снимков настроек электропитания для безопасного отката (OPT-05).

use crate::recovery::simple_utc_now_iso;
use crate::{StorageError, StorageManager};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Запись сохранённого снимка настроек электропитания.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSnapshotRecord {
    pub id: i64,
    pub created_at_utc: String,
    pub active_scheme_guid: String,
    pub active_scheme_name: String,
    pub throttle_max_ac: u32,
    pub throttle_min_ac: u32,
    pub throttle_max_dc: u32,
    pub throttle_min_dc: u32,
    pub applied_profile: String,
}

impl StorageManager {
    pub(crate) fn init_power_store(&self) -> Result<(), StorageError> {
        self.lock_conn()?.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS power_state_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at_utc TEXT NOT NULL,
                active_scheme_guid TEXT NOT NULL,
                active_scheme_name TEXT NOT NULL,
                throttle_max_ac INTEGER NOT NULL,
                throttle_min_ac INTEGER NOT NULL,
                throttle_max_dc INTEGER NOT NULL,
                throttle_min_dc INTEGER NOT NULL,
                applied_profile TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_power_snapshots_id ON power_state_snapshots(id DESC);
            "#,
        )?;
        Ok(())
    }

    /// Сохраняет моментальный снимок схемы питания и троттлинга перед применением профиля.
    #[allow(clippy::too_many_arguments)]
    pub fn save_power_snapshot(
        &self,
        active_scheme_guid: &str,
        active_scheme_name: &str,
        throttle_max_ac: u32,
        throttle_min_ac: u32,
        throttle_max_dc: u32,
        throttle_min_dc: u32,
        applied_profile: &str,
    ) -> Result<i64, StorageError> {
        let conn = self.lock_conn()?;
        let now = simple_utc_now_iso();
        conn.execute(
            r#"
            INSERT INTO power_state_snapshots (
                created_at_utc, active_scheme_guid, active_scheme_name,
                throttle_max_ac, throttle_min_ac, throttle_max_dc, throttle_min_dc,
                applied_profile
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);
            "#,
            params![
                now,
                active_scheme_guid,
                active_scheme_name,
                throttle_max_ac,
                throttle_min_ac,
                throttle_max_dc,
                throttle_min_dc,
                applied_profile
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Возвращает самый свежий сохранённый снимок электропитания.
    pub fn get_latest_power_snapshot(&self) -> Result<Option<PowerSnapshotRecord>, StorageError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, created_at_utc, active_scheme_guid, active_scheme_name,
                   throttle_max_ac, throttle_min_ac, throttle_max_dc, throttle_min_dc,
                   applied_profile
            FROM power_state_snapshots
            ORDER BY id DESC
            LIMIT 1;
            "#,
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(PowerSnapshotRecord {
                id: row.get(0)?,
                created_at_utc: row.get(1)?,
                active_scheme_guid: row.get(2)?,
                active_scheme_name: row.get(3)?,
                throttle_max_ac: row.get(4)?,
                throttle_min_ac: row.get(5)?,
                throttle_max_dc: row.get(6)?,
                throttle_min_dc: row.get(7)?,
                applied_profile: row.get(8)?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_snapshot_crud() {
        let storage = StorageManager::in_memory().unwrap();
        let none = storage.get_latest_power_snapshot().unwrap();
        assert_eq!(none, None);

        let id = storage
            .save_power_snapshot(
                "381b4222-f694-41f0-9685-ff5bb260df2e",
                "Сбалансированная",
                100,
                5,
                100,
                5,
                "Quiet",
            )
            .unwrap();
        assert!(id > 0);

        let snap = storage.get_latest_power_snapshot().unwrap().unwrap();
        assert_eq!(
            snap.active_scheme_guid,
            "381b4222-f694-41f0-9685-ff5bb260df2e"
        );
        assert_eq!(snap.applied_profile, "Quiet");
        assert_eq!(snap.throttle_max_ac, 100);
    }
}
