//! Менеджер локального хранилища SQLite для настроек и журнала действий CleanCore.

use crate::migrations;
use crate::recovery::{self, StorageRecoveryNotice};
use cleaner_core::errors::ApiError;
use cleaner_core::models::AppSettings;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use thiserror::Error;
use tracing::warn;

/// Ошибки работы со слоем хранилища SQLite.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Ошибка SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Ошибка файловой системы: {0}")]
    Io(#[from] std::io::Error),

    #[error("Ошибка сериализации/десериализации данных: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Ошибка блокировки подключения к базе данных: {0}")]
    Lock(String),

    #[error("Неподдерживаемая версия схемы базы данных ({0}), требуется обновление CleanCore")]
    UnsupportedSchemaVersion(i32),

    #[error("Повреждение базы данных SQLite: {0}")]
    CorruptDatabase(String),
}

impl From<StorageError> for ApiError {
    fn from(err: StorageError) -> Self {
        ApiError {
            code: "STORAGE_ERROR".to_string(),
            message: err.to_string(),
        }
    }
}

/// Менеджер постоянного локального хранилища SQLite.
pub struct StorageManager {
    conn: Mutex<Connection>,
    db_path: PathBuf,
    // Prevent another desktop process from recovering our live journal as a crash.
    _audit_lock: Option<std::fs::File>,
    recovery_notice: Option<StorageRecoveryNotice>,
}

impl StorageManager {
    /// Открывает или создаёт базу данных SQLite по указанному пути с применением миграций
    /// и автоматическим карантином при обнаружении повреждений.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db_path = path.as_ref().to_path_buf();

        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut recovery_notice: Option<StorageRecoveryNotice> = None;
        // Retain single-process ownership throughout corruption recovery as well.
        let audit_lock = Self::acquire_audit_lock(&db_path)?;

        let open_and_migrate = |target: &Path| -> Result<Connection, StorageError> {
            let mut conn = Connection::open(target)?;
            conn.execute_batch("PRAGMA synchronous = FULL;")?;
            migrations::apply_migrations(&mut conn)?;
            Ok(conn)
        };

        let conn = match open_and_migrate(&db_path) {
            Ok(res) => res,
            Err(err) if db_path.exists() && is_corruption_error(&err) => {
                warn!(
                    "База данных SQLite по пути {:?} повреждена ({err}). Изоляция в .corrupt и пересоздание.",
                    db_path
                );
                // На Windows открытые дескрипторы уже сброшены, так как open_and_migrate вернул ошибку
                let quarantined = recovery::quarantine_file(&db_path, "corrupt")?;
                recovery::quarantine_sqlite_sidecars(&db_path)?;

                let notice = StorageRecoveryNotice {
                    original_path: db_path.clone(),
                    quarantined_path: quarantined,
                    timestamp_utc: recovery::simple_utc_now_iso(),
                    reason: err.to_string(),
                };
                recovery_notice = Some(notice);

                // Повторная попытка открытия на чистом файле
                open_and_migrate(&db_path)?
            }
            Err(err) => return Err(err),
        };

        let manager = Self {
            conn: Mutex::new(conn),
            db_path,
            _audit_lock: audit_lock,
            recovery_notice,
        };
        manager.init_audit()?;
        manager.init_power_store()?;
        if let Err(error) = manager.init_duplicate_cache() {
            warn!(%error, "Кеш хешей недоступен; анализ выполнит полное чтение");
        }
        Ok(manager)
    }

    /// Создаёт хранилище в оперативной памяти (для unit-тестов).
    pub fn in_memory() -> Result<Self, StorageError> {
        let mut conn = Connection::open_in_memory()?;
        migrations::apply_migrations(&mut conn)?;
        let manager = Self {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
            _audit_lock: None,
            recovery_notice: None,
        };
        manager.init_audit()?;
        manager.init_power_store()?;
        if let Err(error) = manager.init_duplicate_cache() {
            warn!(%error, "Кеш хешей недоступен; анализ выполнит полное чтение");
        }
        Ok(manager)
    }

    /// Возвращает путь к стандартной базе данных CleanCore в `%LOCALAPPDATA%`.
    pub fn default_db_path() -> PathBuf {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(local_app_data)
                .join("CleanCore")
                .join("cleancore.db")
        } else {
            PathBuf::from("cleancore.db")
        }
    }

    pub fn lock_conn(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.conn
            .lock()
            .map_err(|e| StorageError::Lock(format!("Мьютекс базы данных повреждён: {e}")))
    }

    /// Возвращает текущую версию схемы базы данных.
    pub fn current_schema_version(&self) -> Result<i32, StorageError> {
        let conn = self.lock_conn()?;
        migrations::get_user_version(&conn)
    }

    /// Возвращает уведомление о восстановлении базы данных, если оно производилось.
    pub fn recovery_notice(&self) -> Option<&StorageRecoveryNotice> {
        self.recovery_notice.as_ref()
    }

    /// Проверяет, производилось ли восстановление базы данных при запуске.
    pub fn was_recovered(&self) -> bool {
        self.recovery_notice.is_some()
    }

    /// Признак сохранения данных на постоянном носителе.
    pub fn is_persistent(&self) -> bool {
        self.db_path != Path::new(":memory:")
    }

    /// Возвращает значение настройки по ключу из таблицы `settings` SQLite.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;

        if let Some(row) = rows.next()? {
            let val: String = row.get(0)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    /// Устанавливает или перезаписывает значение настройки по ключу в SQLite.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            params![key, value],
        )?;
        Ok(())
    }

    /// Загружает пользовательские настройки из таблицы `settings` SQLite.
    pub fn load_app_settings(&self) -> Result<AppSettings, StorageError> {
        let mut settings = AppSettings::default();

        if let Some(interval_str) = self.get_setting("refresh_interval_ms")? {
            if let Ok(interval) = interval_str.parse::<u64>() {
                if (500..=60000).contains(&interval) {
                    settings.refresh_interval_ms = interval;
                }
            }
        }

        if let Some(theme) = self.get_setting("theme")? {
            if !theme.trim().is_empty() {
                settings.theme = theme;
            }
        }

        if let Some(paused_str) = self.get_setting("is_monitoring_paused")? {
            settings.is_monitoring_paused = paused_str == "true" || paused_str == "1";
        }

        Ok(settings)
    }

    /// Сохраняет пользовательские настройки в SQLite (для обратной совместимости).
    pub fn save_app_settings(&self, settings: &AppSettings) -> Result<(), StorageError> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        for (key, value) in [
            (
                "refresh_interval_ms",
                settings.refresh_interval_ms.to_string(),
            ),
            ("theme", settings.theme.clone()),
            (
                "is_monitoring_paused",
                settings.is_monitoring_paused.to_string(),
            ),
        ] {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
                params![key, value],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Путь к файлу базы данных.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn acquire_audit_lock(path: &Path) -> Result<Option<std::fs::File>, StorageError> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let mut name = path.as_os_str().to_owned();
            name.push(".cleanup.lock");
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(0)
                .open(PathBuf::from(name))?;
            Ok(Some(file))
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            Ok(None)
        }
    }
}

/// Проверяет, указывает ли ошибка на повреждение файла базы данных.
fn is_corruption_error(err: &StorageError) -> bool {
    match err {
        StorageError::Sqlite(sqlite_err) => recovery::is_sqlite_corrupt_error(sqlite_err),
        StorageError::CorruptDatabase(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_save_rolls_back_on_partial_failure() {
        let storage = StorageManager::in_memory().unwrap();
        storage.save_app_settings(&AppSettings::default()).unwrap();
        storage
            .lock_conn()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_theme BEFORE UPDATE ON settings
             WHEN NEW.key = 'theme' BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
            )
            .unwrap();
        let changed = AppSettings {
            refresh_interval_ms: 5000,
            is_monitoring_paused: true,
            ..AppSettings::default()
        };
        assert!(storage.save_app_settings(&changed).is_err());
        assert_eq!(
            storage.load_app_settings().unwrap().refresh_interval_ms,
            2000
        );
        assert!(!storage.load_app_settings().unwrap().is_monitoring_paused);
    }

    #[test]
    fn test_in_memory_settings_crud() {
        let storage = StorageManager::in_memory().expect("failed to create in-memory storage");

        // Чтение отсутствующего ключа
        let val = storage.get_setting("unknown_key").unwrap();
        assert_eq!(val, None);

        // Запись и чтение
        storage.set_setting("theme", "studio-pearl").unwrap();
        let val2 = storage.get_setting("theme").unwrap();
        assert_eq!(val2, Some("studio-pearl".to_string()));

        // Перезапись существующего ключа
        storage.set_setting("theme", "pearl-dark").unwrap();
        let val3 = storage.get_setting("theme").unwrap();
        assert_eq!(val3, Some("pearl-dark".to_string()));
    }

    #[test]
    fn test_default_app_settings() {
        let storage = StorageManager::in_memory().expect("failed to create in-memory storage");
        let settings = storage.load_app_settings().unwrap();

        // Проверяем начальные параметры по ROADMAP (интервал 2000 мс)
        assert_eq!(settings.refresh_interval_ms, 2000);
        assert_eq!(settings.theme, "studio-pearl");
        assert!(!settings.is_monitoring_paused);
    }

    #[test]
    fn test_save_and_reload_app_settings() {
        let storage = StorageManager::in_memory().expect("failed to create in-memory storage");

        let custom = AppSettings {
            refresh_interval_ms: 5000,
            theme: "studio-pearl".to_string(),
            is_monitoring_paused: true,
        };

        storage.save_app_settings(&custom).unwrap();
        let reloaded = storage.load_app_settings().unwrap();

        assert_eq!(reloaded.refresh_interval_ms, 5000);
        assert_eq!(reloaded.theme, "studio-pearl");
        assert!(reloaded.is_monitoring_paused);
    }

    #[test]
    fn test_schema_version_is_current() {
        let storage = StorageManager::in_memory().expect("in-memory db");
        assert_eq!(storage.current_schema_version().unwrap(), 2);
    }
}
