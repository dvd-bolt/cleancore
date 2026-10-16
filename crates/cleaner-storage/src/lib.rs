//! Локальное хранилище настроек и журнала на базе SQLite и settings.json.

pub mod audit;
pub mod cleanup_preferences;
pub mod duplicate_cache;
pub mod migrations;
pub mod power_store;
pub mod recovery;
pub mod settings;
pub mod storage;

pub use migrations::{
    apply_migrations, get_applied_migrations, get_user_version, set_user_version, Migration,
    MIGRATIONS,
};
pub use power_store::PowerSnapshotRecord;
pub use recovery::{SettingsRecoveryNotice, StorageRecoveryNotice};
pub use settings::SettingsManager;
pub use storage::{StorageError, StorageManager};

/// Псевдоним базы данных для соответствия архитектурному контракту Этапа 7.
pub type Database = StorageManager;
