use cleaner_core::models::AppSettings;
use cleaner_storage::{SettingsManager, StorageManager};
use std::path::PathBuf;

fn temp_path(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("cleaner_test_set_{}_{}", name, std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(name)
}

#[test]
fn test_atomic_settings_write_persists() {
    let path = temp_path("settings_atomic.json");

    let manager = SettingsManager::open(&path).expect("failed to open settings");
    assert_eq!(manager.get_settings(), AppSettings::default());

    let updated = AppSettings {
        refresh_interval_ms: 5000,
        theme: "pearl-dark".to_string(),
        is_monitoring_paused: true,
    };
    manager.save_settings(&updated).expect("save failed");
    drop(manager);

    // Убеждаемся, что файл на диске существует и временный .tmp не остался
    assert!(path.exists());
    let parent = path.parent().unwrap();
    let tmp_files: Vec<_> = std::fs::read_dir(parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().contains(".tmp"))
        .collect();
    assert_eq!(
        tmp_files.len(),
        0,
        "Файлы .tmp должны удаляться после атомарной замены"
    );

    // Повторное открытие должно прочитать сохранённые данные
    let reloaded_mgr = SettingsManager::open(&path).expect("reopen failed");
    assert_eq!(reloaded_mgr.get_settings(), updated);

    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn test_settings_seeding_from_legacy_sqlite() {
    let base_dir = std::env::temp_dir().join(format!("cleaner_test_seed_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&base_dir);
    let db_path = base_dir.join("cleancore.db");
    let settings_path = base_dir.join("settings.json");

    // Создаём базу SQLite с сохранёнными legacy-настройками
    let storage = StorageManager::open(&db_path).unwrap();
    let legacy_settings = AppSettings {
        refresh_interval_ms: 10000,
        theme: "legacy-nord".to_string(),
        is_monitoring_paused: true,
    };
    storage.save_app_settings(&legacy_settings).unwrap();

    // Открываем SettingsManager с передачей legacy-хранилища
    assert!(!settings_path.exists());
    let settings_mgr =
        SettingsManager::open_with_legacy_sqlite(&settings_path, Some(&storage)).unwrap();

    // Проверяем, что settings.json создан и содержит значения из SQLite
    assert!(settings_path.exists());
    assert_eq!(settings_mgr.get_settings(), legacy_settings);

    drop(settings_mgr);
    drop(storage);
    let _ = std::fs::remove_dir_all(&base_dir);
}

#[test]
fn test_settings_validation_and_clamping() {
    let path = temp_path("validation.json");
    let manager = SettingsManager::open(&path).unwrap();

    // Недопустимо малый интервал (10 мс) должен сброситься на дефолтный (2000 мс)
    let bad_settings = AppSettings {
        refresh_interval_ms: 10,
        theme: "   ".to_string(),
        is_monitoring_paused: false,
    };
    manager.save_settings(&bad_settings).unwrap();

    let loaded = manager.get_settings();
    assert_eq!(loaded.refresh_interval_ms, 2000);
    assert_eq!(loaded.theme, "studio-pearl");

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
