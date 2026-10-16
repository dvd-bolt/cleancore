use crate::{StorageError, StorageManager};
use cleaner_core::cleanup_preferences::CleanupPreferences;
use rusqlite::OptionalExtension;
impl StorageManager {
    pub fn load_cleanup_preferences(&self) -> Result<CleanupPreferences, StorageError> {
        let raw: Option<String> = self
            .lock_conn()?
            .query_row(
                "SELECT value FROM settings WHERE key='cleanup_preferences_v1'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let preferences: CleanupPreferences = raw
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        preferences
            .validate()
            .map_err(|e| StorageError::Lock(e.into()))?;
        Ok(preferences)
    }
    pub fn save_cleanup_preferences(
        &self,
        preferences: &CleanupPreferences,
    ) -> Result<(), StorageError> {
        preferences
            .validate()
            .map_err(|e| StorageError::Lock(e.into()))?;
        self.lock_conn()?.execute(
            "INSERT OR REPLACE INTO settings(key,value) VALUES('cleanup_preferences_v1',?1)",
            [serde_json::to_string(preferences)?],
        )?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_roundtrip_and_corruption_does_not_drop_protection_silently() {
        let store = StorageManager::in_memory().unwrap();
        let prefs = CleanupPreferences {
            excluded_paths: vec![r"C:\Keep".into()],
            protected_duplicate_roots: vec![r"D:\Originals".into()],
        };
        store.save_cleanup_preferences(&prefs).unwrap();
        assert_eq!(store.load_cleanup_preferences().unwrap(), prefs);
        store
            .lock_conn()
            .unwrap()
            .execute(
                "UPDATE settings SET value='invalid' WHERE key='cleanup_preferences_v1'",
                [],
            )
            .unwrap();
        assert!(store.load_cleanup_preferences().is_err());
    }
}
