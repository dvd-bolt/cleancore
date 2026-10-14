//! Локальное хранилище настроек и журнала на базе SQLite.

pub mod audit;
pub mod storage;

pub use storage::{StorageError, StorageManager};
