//! Manual map selection. No frontend path is accepted by the IPC command.
use cleaner_core::errors::ApiError;
use cleaner_execution::native::{
    capture_candidate, capture_directory, lock_directory, TrustedCandidate,
};
use std::path::Path;

fn denied(code: &str, message: &str) -> ApiError {
    ApiError {
        code: code.into(),
        message: message.into(),
    }
}

pub fn prepare_indexed_file(
    path: &Path,
    expected_reference: u64,
    expected_volume: u64,
) -> Result<Vec<TrustedCandidate>, ApiError> {
    if path.extension().is_some_and(|ext| {
        ["vhd", "vhdx", "vmdk", "vdi"]
            .iter()
            .any(|blocked| ext.eq_ignore_ascii_case(blocked))
    }) {
        return Err(denied("VIRTUAL_DISK", "Виртуальный диск может содержать систему и данные. Используйте управление Docker/WSL или виртуальной машиной"));
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{
            FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music,
            FOLDERID_Pictures, FOLDERID_Videos,
        };
        for folder in [
            &FOLDERID_Desktop,
            &FOLDERID_Documents,
            &FOLDERID_Downloads,
            &FOLDERID_Music,
            &FOLDERID_Pictures,
            &FOLDERID_Videos,
        ] {
            let Ok(root) = crate::path_safety::known_folder(folder) else {
                continue;
            };
            if crate::path_safety::ensure_local_fixed(&root).is_err() {
                continue;
            }
            if let Ok(relative) = path.strip_prefix(&root) {
                if relative.as_os_str().is_empty() {
                    return Err(denied(
                        "MANUAL_SCOPE",
                        "Удаление корня пользовательской папки запрещено",
                    ));
                }
                let candidate = match capture_candidate(&root, relative) {
                    Ok(candidate) => {
                        if candidate.file_reference() != expected_reference
                            || candidate.volume_serial() != expected_volume
                        {
                            return Err(denied(
                                "OBJECT_CHANGED",
                                "Файл заменён после анализа карты. Повторите анализ",
                            ));
                        }
                        return Ok(vec![candidate]);
                    }
                    Err("WRONG_OBJECT_TYPE") => capture_directory(&root, relative)
                        .map_err(|code| denied(code, "Каталог недоступен или перенаправлен"))?,
                    Err(code) => return Err(denied(
                        code,
                        "Объект недоступен, занят, является ссылкой или имеет неподдерживаемый тип",
                    )),
                };
                if candidate.file_reference() != expected_reference
                    || candidate.volume_serial() != expected_volume
                {
                    return Err(denied(
                        "OBJECT_CHANGED",
                        "Каталог заменён после анализа карты. Повторите анализ",
                    ));
                }
                let mut objects = Vec::new();
                collect_folder(&root, path, 0, &mut objects)?;
                // collect_folder captures the selected directory last. Check that
                // it is still the very directory selected from the MFT index.
                if objects
                    .last()
                    .is_none_or(|c| c.file_reference() != expected_reference)
                    || objects.iter().any(|c| c.volume_serial() != expected_volume)
                {
                    return Err(denied("OBJECT_CHANGED", "Каталог изменился при подготовке"));
                }
                return Ok(objects);
            }
        }
        Err(denied("MANUAL_SCOPE", "Ручное удаление пока разрешено только для объектов в Рабочем столе, Загрузках, Документах, Музыке, Изображениях и Видео текущей учётной записи. Системные области и виртуальные диски здесь не очищаются автоматически"))
    }
    #[cfg(not(windows))]
    {
        let _ = (path, expected_reference, expected_volume);
        Err(denied("UNSUPPORTED_PLATFORM", "Доступно только в Windows"))
    }
}

#[cfg(windows)]
fn collect_folder(
    root: &Path,
    dir: &Path,
    depth: usize,
    objects: &mut Vec<TrustedCandidate>,
) -> Result<(), ApiError> {
    if depth > 32 || objects.len() >= cleaner_execution::MAX_CANDIDATES {
        return Err(denied("MANUAL_LIMIT", "Папка превышает предел 1000 объектов или 32 уровня. Выберите меньшую подпапку; ничего не удалено"));
    }
    let relative = dir
        .strip_prefix(root)
        .map_err(|_| denied("MANUAL_SCOPE", "Каталог вне разрешённой области"))?;
    let _lease = lock_directory(root, relative)
        .map_err(|code| denied(code, "Не удалось проверить каталог"))?;
    let directory = capture_directory(root, relative)
        .map_err(|code| denied(code, "Не удалось проверить каталог"))?;
    for entry in std::fs::read_dir(dir).map_err(|_| {
        denied(
            "READ_DIRECTORY_FAILED",
            "Часть папки недоступна; выберите меньшую область",
        )
    })? {
        let entry = entry.map_err(|_| denied("READ_ENTRY_FAILED", "Ошибка чтения папки"))?;
        let path = entry.path();
        if crate::path_safety::is_reparse_point_or_symlink(&path) {
            return Err(denied(
                "REPARSE_POINT",
                "Папка содержит ссылку или непроверяемый объект; выберите отдельные файлы",
            ));
        }
        if path.extension().is_some_and(|ext| {
            ["vhd", "vhdx", "vmdk", "vdi"]
                .iter()
                .any(|x| ext.eq_ignore_ascii_case(x))
        }) {
            return Err(denied(
                "VIRTUAL_DISK",
                "Папка содержит виртуальный диск; автоматический обход для удаления запрещён",
            ));
        }
        let kind = entry
            .file_type()
            .map_err(|_| denied("TYPE_UNAVAILABLE", "Тип объекта недоступен"))?;
        if kind.is_dir() {
            collect_folder(root, &path, depth + 1, objects)?;
        } else if kind.is_file() {
            let child = capture_candidate(
                root,
                path.strip_prefix(root)
                    .map_err(|_| denied("MANUAL_SCOPE", "Неверный путь"))?,
            )
            .map_err(|code| denied(code, "Файл недоступен; папка не подготовлена к удалению"))?;
            objects.push(child);
        } else {
            return Err(denied(
                "WRONG_OBJECT_TYPE",
                "Неподдерживаемый объект внутри папки",
            ));
        }
        if objects.len() >= cleaner_execution::MAX_CANDIDATES {
            return Err(denied("MANUAL_LIMIT", "Папка превышает предел 1000 объектов. Выберите меньшую подпапку; ничего не удалено"));
        }
    }
    objects.push(directory);
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{fs, sync::Arc};
    #[test]
    fn manual_folder_snapshot_deletes_only_confirmed_children_and_empty_directories() {
        let root = std::env::temp_dir().join(format!("cleancore_manual_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("chosen/child")).unwrap();
        fs::write(root.join("chosen/child/a.txt"), b"fixture").unwrap();
        let mut objects = Vec::new();
        collect_folder(&root, &root.join("chosen"), 0, &mut objects).unwrap();
        assert_eq!(objects.len(), 3);
        let journal =
            Arc::new(cleaner_storage::StorageManager::open(root.join("journal.db")).unwrap());
        let service = cleaner_execution::CleanupService::new(
            journal,
            Arc::new(cleaner_execution::native::BoundDelete),
        );
        let preview = service.publish_category(objects, "user-files-v1").unwrap();
        let confirmation = service
            .prepare_confirmation(
                &preview.preview_id,
                preview
                    .candidates
                    .iter()
                    .map(|c| c.object_id.clone())
                    .collect(),
            )
            .unwrap();
        // A new file is not in the immutable snapshot and must survive execution.
        fs::write(root.join("chosen/new.txt"), b"new fixture").unwrap();
        let started = service.begin(&confirmation.confirmation_id, true).unwrap();
        let result = service.run(&started.operation_id).unwrap();
        assert!(!root.join("chosen/child").exists());
        assert!(root.join("chosen/new.txt").exists());
        assert_eq!(
            result.items.last().unwrap().status,
            cleaner_core::cleanup::CleanupItemStatus::Skipped
        );
        assert_eq!(
            result
                .items
                .iter()
                .filter(|i| i.status == cleaner_core::cleanup::CleanupItemStatus::Deleted)
                .count(),
            2
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn folder_limit_and_virtual_disks_fail_before_any_mutation() {
        let root =
            std::env::temp_dir().join(format!("cleancore_manual_limit_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("chosen")).unwrap();
        fs::write(root.join("chosen/docker_data.vhdx"), b"fixture").unwrap();
        assert_eq!(
            collect_folder(&root, &root.join("chosen"), 0, &mut vec![])
                .unwrap_err()
                .code,
            "VIRTUAL_DISK"
        );
        assert_eq!(
            prepare_indexed_file(&root.join("chosen/docker_data.vhdx"), 1, 0)
                .unwrap_err()
                .code,
            "VIRTUAL_DISK"
        );
        assert_eq!(
            collect_folder(&root, &root.join("chosen"), 33, &mut vec![])
                .unwrap_err()
                .code,
            "MANUAL_LIMIT"
        );
        assert!(root.join("chosen/docker_data.vhdx").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
