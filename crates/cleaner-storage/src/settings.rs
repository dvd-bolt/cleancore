//! Менеджер настроек CleanCore (settings.json) с поддержкой атомарной записи и автокарантина.

use crate::recovery::{self, SettingsRecoveryNotice};
use crate::storage::StorageManager;
use crate::StorageError;
use cleaner_core::models::AppSettings;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{info, warn};

/// Менеджер пользовательских настроек в формате JSON.
pub struct SettingsManager {
    path: PathBuf,
    settings: Mutex<AppSettings>,
    recovery_notice: Option<SettingsRecoveryNotice>,
    is_in_memory: bool,
}

impl SettingsManager {
    /// Открывает или создаёт файл настроек по указанному пути.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        Self::open_internal(path.as_ref(), None)
    }

    /// Открывает файл настроек с возможностью миграции из старой таблицы SQLite,
    /// если файл `settings.json` ещё не был создан.
    pub fn open_with_legacy_sqlite<P: AsRef<Path>>(
        path: P,
        legacy_storage: Option<&StorageManager>,
    ) -> Result<Self, StorageError> {
        Self::open_internal(path.as_ref(), legacy_storage)
    }

    /// Создаёт менеджер настроек в оперативной памяти (для unit-тестов).
    pub fn in_memory(initial: AppSettings) -> Self {
        Self {
            path: PathBuf::from(":memory:"),
            settings: Mutex::new(initial),
            recovery_notice: None,
            is_in_memory: true,
        }
    }

    /// Возвращает путь к стандартному файлу `settings.json` в `%LOCALAPPDATA%\CleanCore`.
    pub fn default_settings_path() -> PathBuf {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(local_app_data)
                .join("CleanCore")
                .join("settings.json")
        } else {
            PathBuf::from("settings.json")
        }
    }

    fn open_internal(
        path: &Path,
        legacy_storage: Option<&StorageManager>,
    ) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Если файла нет — инициализируем из SQLite (если есть) или дефолтами
        if !path.exists() {
            let initial = if let Some(storage) = legacy_storage {
                match storage.load_app_settings() {
                    Ok(s) => {
                        info!("Импортированы существующие настройки из базы SQLite в новый settings.json");
                        s
                    }
                    Err(e) => {
                        warn!("Не удалось прочитать legacy-настройки из SQLite ({e}), используются дефолтные");
                        AppSettings::default()
                    }
                }
            } else {
                AppSettings::default()
            };

            atomic_write_json(path, &initial)?;
            return Ok(Self {
                path: path.to_path_buf(),
                settings: Mutex::new(initial),
                recovery_notice: None,
                is_in_memory: false,
            });
        }

        // Файл существует — пытаемся прочитать и распарсить
        let read_result = std::fs::read_to_string(path);
        let failure_reason = match read_result {
            Ok(content) => match serde_json::from_str::<AppSettings>(&content) {
                Ok(mut settings) => {
                    validate_and_sanitize_settings(&mut settings);
                    return Ok(Self {
                        path: path.to_path_buf(),
                        settings: Mutex::new(settings),
                        recovery_notice: None,
                        is_in_memory: false,
                    });
                }
                Err(err) => Some(format!("Ошибка парсинга JSON: {err}")),
            },
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(StorageError::Io(err));
            }
            Err(err) => Some(format!("Ошибка чтения файла: {err}")),
        };

        if let Some(reason) = failure_reason {
            warn!(
                "Файл настроек по пути {:?} повреждён ({reason}). Изоляция в .corrupt и сброс на дефолты.",
                path
            );
            let quarantined = recovery::quarantine_file(path, "corrupt")?;
            let default_settings = AppSettings::default();
            atomic_write_json(path, &default_settings)?;

            let notice = SettingsRecoveryNotice {
                original_path: path.to_path_buf(),
                quarantined_path: quarantined,
                timestamp_utc: recovery::simple_utc_now_iso(),
                reason,
            };

            return Ok(Self {
                path: path.to_path_buf(),
                settings: Mutex::new(default_settings),
                recovery_notice: Some(notice),
                is_in_memory: false,
            });
        }

        // Unreachable fallback
        let default_settings = AppSettings::default();
        Ok(Self {
            path: path.to_path_buf(),
            settings: Mutex::new(default_settings),
            recovery_notice: None,
            is_in_memory: false,
        })
    }

    /// Возвращает снимок текущих настроек приложения.
    pub fn get_settings(&self) -> AppSettings {
        self.settings.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Сохраняет настройки атомарно на диск и обновляет значение в памяти.
    pub fn save_settings(&self, new_settings: &AppSettings) -> Result<(), StorageError> {
        let mut sanitized = new_settings.clone();
        validate_and_sanitize_settings(&mut sanitized);

        if !self.is_in_memory {
            atomic_write_json(&self.path, &sanitized)?;
        }

        let mut lock = self
            .settings
            .lock()
            .map_err(|e| StorageError::Lock(format!("Мьютекс настроек повреждён: {e}")))?;
        *lock = sanitized;

        Ok(())
    }

    /// Атомарно модифицирует настройки с помощью замыкания и сохраняет их.
    pub fn update_settings<F>(&self, update_fn: F) -> Result<AppSettings, StorageError>
    where
        F: FnOnce(&mut AppSettings),
    {
        let mut lock = self
            .settings
            .lock()
            .map_err(|e| StorageError::Lock(format!("Мьютекс настроек повреждён: {e}")))?;

        let mut modified = lock.clone();
        update_fn(&mut modified);
        validate_and_sanitize_settings(&mut modified);

        if !self.is_in_memory {
            atomic_write_json(&self.path, &modified)?;
        }

        *lock = modified.clone();
        Ok(modified)
    }

    /// Возвращает уведомление о восстановлении повреждённого файла настроек, если оно производилось.
    pub fn recovery_notice(&self) -> Option<&SettingsRecoveryNotice> {
        self.recovery_notice.as_ref()
    }

    /// Проверяет, производилось ли восстановление настроек при запуске.
    pub fn was_recovered(&self) -> bool {
        self.recovery_notice.is_some()
    }

    /// Возвращает путь к файлу настроек.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Возвращает признак сохранения настроек на постоянный диск.
    pub fn is_persistent(&self) -> bool {
        !self.is_in_memory
    }
}

/// Атомарная запись настроек на диск: запись в `.tmp` с `sync_all()`, затем атомарная замена `rename()`.
fn atomic_write_json(path: &Path, settings: &AppSettings) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let json_bytes = serde_json::to_vec_pretty(settings)?;
    let tmp_path = format!("{}.{}.tmp", path.display(), std::process::id());
    let tmp_path = PathBuf::from(tmp_path);

    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;
        file.write_all(&json_bytes)?;
        file.sync_all()?;
    }

    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        StorageError::Io(e)
    })?;

    Ok(())
}

/// Валидирует и нормализует значения настроек в допустимые диапазоны ТЗ.
fn validate_and_sanitize_settings(settings: &mut AppSettings) {
    if !(500..=60_000).contains(&settings.refresh_interval_ms) {
        settings.refresh_interval_ms = 2000;
    }
    if settings.theme.trim().is_empty() {
        settings.theme = "studio-pearl".to_string();
    }
}
