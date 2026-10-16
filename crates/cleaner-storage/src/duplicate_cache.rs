//! Optional, bounded, disposable duplicate-analysis cache; no paths or contents.
use crate::{StorageError, StorageManager};
use cleaner_core::duplicate_cache::DuplicateHashCache;
use rusqlite::{params, OptionalExtension};
impl StorageManager {
    pub(crate) fn init_duplicate_cache(&self) -> Result<(), StorageError> {
        self.lock_conn()?.execute_batch("CREATE TABLE IF NOT EXISTS duplicate_hash_cache (cache_key TEXT PRIMARY KEY NOT NULL, sha256 TEXT NOT NULL, stored_at INTEGER NOT NULL);")?;
        Ok(())
    }
    pub fn clear_duplicate_cache(&self) -> Result<(), StorageError> {
        self.lock_conn()?
            .execute("DELETE FROM duplicate_hash_cache", [])?;
        Ok(())
    }
}
impl DuplicateHashCache for StorageManager {
    fn get(&self, key: &str) -> Option<String> {
        if key.len() > 256 {
            return None;
        }
        let conn = self.lock_conn().ok()?;
        let hash: String = conn.query_row("SELECT sha256 FROM duplicate_hash_cache WHERE cache_key=?1 AND stored_at >= unixepoch()-2592000", [key], |row| row.get(0)).optional().ok()??;
        (hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit())).then_some(hash)
    }
    fn put(&self, key: &str, sha256: &str) {
        if key.len() > 256 || sha256.len() != 64 || !sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
            return;
        }
        let Ok(mut conn) = self.lock_conn() else {
            return;
        };
        let Ok(tx) = conn.transaction() else {
            return;
        };
        if tx.execute("INSERT OR REPLACE INTO duplicate_hash_cache(cache_key,sha256,stored_at) VALUES(?1,?2,unixepoch())", params![key, sha256]).is_err() { return; }
        if tx.execute("DELETE FROM duplicate_hash_cache WHERE stored_at < unixepoch()-2592000 OR cache_key IN (SELECT cache_key FROM duplicate_hash_cache ORDER BY stored_at DESC, cache_key LIMIT -1 OFFSET 10000)", []).is_err() { return; }
        let _ = tx.commit();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_is_persistent_validated_and_can_be_cleared() {
        let path =
            std::env::temp_dir().join(format!("cleancore_hash_cache_{}.db", std::process::id()));
        let store = StorageManager::open(&path).unwrap();
        store.put("v1:identity:metadata:prehash", &"ab".repeat(32));
        store.put("bad", "invalid");
        assert!(store.get("bad").is_none());
        drop(store);
        let store = StorageManager::open(&path).unwrap();
        assert_eq!(
            store.get("v1:identity:metadata:prehash"),
            Some("ab".repeat(32))
        );
        store.clear_duplicate_cache().unwrap();
        assert!(store.get("v1:identity:metadata:prehash").is_none());
        drop(store);
        std::fs::remove_file(&path).unwrap();
        // SQLite lock sidecar has no user data. Its test fixture may remain if Windows holds it.
    }
}
