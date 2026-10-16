//! Disposable analysis hints. Cached hashes never authorize file deletion.
pub trait DuplicateHashCache: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn put(&self, key: &str, sha256: &str);
}
