//! Менеджер локального хранилища SQLite для настроек и журнала действий CleanCore.

use cleaner_core::errors::ApiError;
use cleaner_core::models::AppSettings;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use thiserror::Error;

/// Ошибки работы со слоем хранилища SQLite.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Ошибка SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Ошибка файловой системы при создании каталога хранилища: {0}")]
    Io(#[from] std::io::Error),

    #[error("Ошибка сериализации/десериализации данных: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Ошибка блокировки подключения к базе данных: {0}")]
    Lock(String),
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
}

impl StorageManager {
    /// Открывает или создаёт базу данных SQLite по указанному пути.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db_path = path.as_ref().to_path_buf();

        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let conn = Connection::open(&db_path)?;
        let manager = Self {
            conn: Mutex::new(conn),
            db_path,
        };
        manager.init_schema()?;
        Ok(manager)
    }

    /// Создаёт хранилище в оперативной памяти (для unit-тестов).
    pub fn in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let manager = Self {
            conn: Mutex::new(conn),
            db_path: PathBuf::from(":memory:"),
        };
        manager.init_schema()?;
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

    /// Инициализация схемы базы данных (таблица настроек).
    fn init_schema(&self) -> Result<(), StorageError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
            [],
        )?;
        Ok(())
    }

    fn lock_conn(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.conn
            .lock()
            .map_err(|e| StorageError::Lock(format!("Мьютекс базы данных повреждён: {e}")))
    }

    /// Возвращает значение настройки по ключу.
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

    /// Устанавливает или перезаписывает значение настройки по ключу.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            params![key, value],
        )?;
        Ok(())
    }

    /// Загружает все пользовательские настройки с применением дефолтных значений.
    pub fn load_app_settings(&self) -> Result<AppSettings, StorageError> {
        let mut settings = AppSettings::default();

        if let Some(interval_str) = self.get_setting("refresh_interval_ms")? {
            if let Ok(interval) = interval_str.parse::<u64>() {
                // Допустимые интервалы: 2000, 5000, 10000 мс (с валидацией)
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

    /// Сохраняет пользовательские настройки в базу данных.
    pub fn save_app_settings(&self, settings: &AppSettings) -> Result<(), StorageError> {
        self.set_setting(
            "refresh_interval_ms",
            &settings.refresh_interval_ms.to_string(),
        )?;
        self.set_setting("theme", &settings.theme)?;
        self.set_setting(
            "is_monitoring_paused",
            if settings.is_monitoring_paused {
                "true"
            } else {
                "false"
            },
        )?;
        Ok(())
    }

    /// Путь к файлу базы данных.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
