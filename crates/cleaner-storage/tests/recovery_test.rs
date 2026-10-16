use cleaner_core::models::AppSettings;
use cleaner_storage::{SettingsManager, StorageManager};
use std::path::PathBuf;

fn temp_path(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("cleaner_test_rec_{}_{}", name, std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(name)
}

#[test]
fn test_corrupt_sqlite_quarantined_and_recreated() {
    let base_dir =
        std::env::temp_dir().join(format!("cleaner_corrupt_db_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&base_dir);
    let db_path = base_dir.join("cleancore.db");

    // Записываем мусорные байты вместо валидного заголовка SQLite
    std::fs::write(&db_path, b"CORRUPTED HEADER NOT A SQLITE DATABASE").unwrap();

    // Создаём фиктивные файлы журналов для проверки их очистки
    let wal_path = base_dir.join("cleancore.db-wal");
    let shm_path = base_dir.join("cleancore.db-shm");
    std::fs::write(&wal_path, b"stale wal data").unwrap();
    std::fs::write(&shm_path, b"stale shm data").unwrap();

    // Открываем хранилище — оно должно обнаружить порчу, изолировать файл в .corrupt и пересоздать чистую базу
    let storage = StorageManager::open(&db_path)
        .expect("Storage should successfully recover from corruption");
    assert!(
        storage.was_recovered(),
        "Должен быть выставлен флаг восстановления"
    );

    let notice = storage
        .recovery_notice()
        .expect("Должно присутствовать уведомление о восстановлении");
    assert_eq!(notice.original_path, db_path);
    assert!(
        notice.quarantined_path.exists(),
        "Файл карантина должен существовать на диске"
    );
    assert!(notice
        .quarantined_path
        .to_string_lossy()
        .contains(".corrupt"));

    // Проверяем, что в файле карантина сохранены исходные испорченные данные
    let quarantined_content = std::fs::read(&notice.quarantined_path).unwrap();
    assert_eq!(
        quarantined_content,
        b"CORRUPTED HEADER NOT A SQLITE DATABASE"
    );

    // Проверяем, что новая база создана, рабочая и содержит схему версии 2
    assert_eq!(storage.current_schema_version().unwrap(), 2);

    // Проверяем, что висячие файлы -wal и -shm были удалены
    assert!(!wal_path.exists(), "Висячий -wal файл должен быть очищен");
    assert!(!shm_path.exists(), "Висячий -shm файл должен быть очищен");

    drop(storage);
    let _ = std::fs::remove_dir_all(&base_dir);
}

#[test]
fn test_corrupt_settings_json_quarantined_and_defaulted() {
    let settings_path = temp_path("corrupt_settings.json");

    // Записываем невалидный JSON
    std::fs::write(
        &settings_path,
        b"{\"refresh_interval_ms\": 2000, \"theme\": ",
    )
    .unwrap();

    let manager = SettingsManager::open(&settings_path)
        .expect("SettingsManager should recover from invalid JSON");
    assert!(
        manager.was_recovered(),
        "Менеджер настроек должен зафиксировать факт восстановления"
    );

    let notice = manager
        .recovery_notice()
        .expect("Recovery notice must be present");
    assert_eq!(notice.original_path, settings_path);
    assert!(
        notice.quarantined_path.exists(),
        "Файл карантина должен быть создан"
    );
    assert!(notice
        .quarantined_path
        .to_string_lossy()
        .contains(".corrupt"));

    // Проверяем, что активный файл настроек теперь содержит корректный дефолт
    let settings = manager.get_settings();
    assert_eq!(settings, AppSettings::default());

    let _ = std::fs::remove_dir_all(settings_path.parent().unwrap());
}

#[cfg(windows)]
#[test]
fn test_lock_contention_does_not_quarantine() {
    let db_path = temp_path("lock_test.db");

    let storage1 = StorageManager::open(&db_path).unwrap();
    assert!(!storage1.was_recovered());

    // Вторая попытка открыть базу с тем же путём пока storage1 держит .cleanup.lock
    let storage2_res = StorageManager::open(&db_path);
    assert!(
        storage2_res.is_err(),
        "Второй экземпляр должен получить ошибку блокировки"
    );

    // Проверяем, что база НЕ была отправлена в карантин из-за блокировки
    let parent = db_path.parent().unwrap();
    let corrupt_files: Vec<_> = std::fs::read_dir(parent)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().contains(".corrupt"))
        .collect();
    assert_eq!(
        corrupt_files.len(),
        0,
        "Блокировка доступа не должна приводить к карантину базы данных!"
    );

    drop(storage1);
    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn sqlite_sidecars_are_preserved_as_evidence() {
    let dir = std::env::temp_dir().join(format!("cleancore_sidecars_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("data.db");
    let sidecar = dir.join("data.db-wal");
    std::fs::write(&sidecar, b"recovery evidence").unwrap();
    cleaner_storage::recovery::quarantine_sqlite_sidecars(&db).unwrap();
    assert!(!sidecar.exists());
    let archived = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(std::fs::read(archived).unwrap(), b"recovery evidence");
    std::fs::remove_dir_all(dir).unwrap();
}
