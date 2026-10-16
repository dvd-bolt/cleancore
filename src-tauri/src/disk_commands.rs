//! Tauri IPC команды для анализа распределения диска, поиска дубликатов и истории ресурсов.

use cleaner_core::errors::ApiError;
use cleaner_disk::models::{
    DiskAnalysisReport, DuplicateScanResult, QuickDirectory, ResourceHistory,
};
use cleaner_disk::tasks::DiskTaskStatus;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn invalidate_disk_report(
    state: State<'_, AppState>,
    report_id: String,
) -> Result<bool, ApiError> {
    if report_id.is_empty() || report_id.len() > 128 {
        return Err(ApiError {
            code: "INVALID_ID".into(),
            message: "Некорректный идентификатор".into(),
        });
    }
    Ok(state.disk_manager.invalidate_report(&report_id))
}

#[tauri::command]
pub async fn prepare_duplicate_cleanup(
    state: State<'_, AppState>,
    report_id: String,
    group_id: String,
    keeper_index: usize,
    selected_indices: Vec<usize>,
) -> Result<cleaner_core::cleanup::CleanupPreview, ApiError> {
    let fail = |code: &str, message: &str| ApiError {
        code: code.into(),
        message: message.into(),
    };
    if report_id.len() > 128
        || group_id.len() > 128
        || selected_indices.is_empty()
        || selected_indices.len() > cleaner_execution::MAX_CANDIDATES
    {
        return Err(fail("INVALID_SELECTION", "Некорректный выбор копий"));
    }
    let report = state
        .disk_manager
        .get_duplicate_report(&report_id)
        .ok_or_else(|| fail("REPORT_NOT_FOUND", "Отчёт устарел; повторите поиск"))?;
    let group = report
        .groups
        .into_iter()
        .find(|g| g.group_id == group_id)
        .ok_or_else(|| fail("GROUP_NOT_FOUND", "Группа не найдена"))?;
    if keeper_index >= group.files.len()
        || selected_indices
            .iter()
            .any(|&i| i >= group.files.len() || i == keeper_index)
        || selected_indices
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != selected_indices.len()
    {
        return Err(fail(
            "KEEPER_SELECTED",
            "Выберите сохраняемый оригинал и другие файлы для удаления",
        ));
    }
    let preferences = state
        .cleanup_preferences
        .read()
        .map_err(|_| fail("PREFERENCES_UNAVAILABLE", "Настройки защиты недоступны"))?
        .clone()
        .ok_or_else(|| fail("PREFERENCES_UNAVAILABLE", "Сохраните настройки защиты"))?;
    for &index in &selected_indices {
        let path = &group.files[index].path;
        if preferences.excludes(path) || preferences.protects_duplicate(path) {
            return Err(fail(
                "PROTECTED_ORIGINAL",
                "Выбран файл из исключений или защищённой папки оригиналов",
            ));
        }
    }
    let service = state.cleanup.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let root = std::path::Path::new(&report.root_path);
        let capture = |path: &str| {
            let path = std::path::Path::new(path);
            let relative = path
                .strip_prefix(root)
                .map_err(|_| fail("INVALID_PATH", "Файл вне отчёта"))?;
            cleaner_execution::native::capture_candidate(root, relative)
                .map_err(|code| fail(code, "Файл недоступен или изменился"))
        };
        let keeper = capture(&group.files[keeper_index].path)?;
        let mut objects = Vec::new();
        for index in selected_indices {
            let candidate = capture(&group.files[index].path)?;
            let mut checked = cleaner_cleanup::manual::prepare_indexed_file(
                std::path::Path::new(&group.files[index].path),
                candidate.file_reference(),
                candidate.volume_serial(),
            )?;
            if checked.len() != 1 {
                return Err(fail(
                    "INVALID_SELECTION",
                    "Дубликат должен быть обычным файлом",
                ));
            }
            objects.push(checked.remove(0));
        }
        let mut hash = [0u8; 32];
        if group.full_hash.len() != 64 {
            return Err(fail("INVALID_HASH", "Некорректный хеш группы"));
        }
        for (i, byte) in hash.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&group.full_hash[i * 2..i * 2 + 2], 16)
                .map_err(|_| fail("INVALID_HASH", "Некорректный хеш группы"))?;
        }
        service.publish_duplicate_group(objects, keeper, hash)
    })
    .await
    .map_err(|_| fail("DISK_WORKER", "Не удалось подготовить копии"))?
}

#[tauri::command]
pub async fn prepare_disk_file_cleanup(
    state: State<'_, AppState>,
    report_id: String,
    entry_id: String,
) -> Result<cleaner_core::cleanup::CleanupPreview, ApiError> {
    if report_id.len() > 128 || entry_id.len() > 32 {
        return Err(ApiError {
            code: "INVALID_ID".into(),
            message: "Некорректный идентификатор".into(),
        });
    }
    let reference = entry_id.parse::<u64>().map_err(|_| ApiError {
        code: "INVALID_ID".into(),
        message: "Группа карты не является отдельным файлом".into(),
    })?;
    let (path, volume) = state
        .disk_manager
        .indexed_entry_identity(&report_id, &entry_id)
        .ok_or_else(|| ApiError {
            code: "ENTRY_NOT_FOUND".into(),
            message: "Объект отсутствует в текущем индексе; повторите анализ".into(),
        })?;
    let service = state.cleanup.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let candidate = cleaner_cleanup::manual::prepare_indexed_file(
            std::path::Path::new(&path),
            reference,
            volume,
        )?;
        service.publish_category(candidate, "user-files-v1")
    })
    .await
    .map_err(|_| ApiError {
        code: "DISK_WORKER".into(),
        message: "Не удалось подготовить список".into(),
    })?
}

#[tauri::command]
pub async fn get_disk_treemap(
    state: State<'_, AppState>,
    report_id: String,
    path: String,
    size_mode: cleaner_disk::models::TreemapSize,
) -> Result<cleaner_disk::models::TreemapData, ApiError> {
    let manager = state.disk_manager.clone();
    tauri::async_runtime::spawn_blocking(move||manager.treemap(&report_id,&path,size_mode).ok_or_else(||ApiError{code:"TREEMAP_UNAVAILABLE".into(),message:"Подробная карта доступна после MFT-анализа целого NTFS-диска. Повторите анализ диска, если индекс уже заменён.".into()})).await.map_err(|e|ApiError{code:"DISK_WORKER".into(),message:e.to_string()})?
}

#[tauri::command]
pub async fn reveal_disk_entry(
    state: State<'_, AppState>,
    report_id: String,
    entry_id: String,
) -> Result<(), ApiError> {
    let path = state
        .disk_manager
        .indexed_entry_path(&report_id, &entry_id)
        .ok_or_else(|| ApiError {
            code: "ENTRY_NOT_FOUND".into(),
            message: "Объект отсутствует в текущем индексе".into(),
        })?;
    tauri::async_runtime::spawn_blocking(move || reveal_in_explorer(&path))
        .await
        .map_err(|e| ApiError {
            code: "DISK_WORKER".into(),
            message: e.to_string(),
        })?
        .map_err(|e| ApiError {
            code: "REVEAL_FAILED".into(),
            message: e,
        })
}

#[tauri::command]
pub async fn reveal_path_in_explorer(path: String) -> Result<(), ApiError> {
    let p = std::path::Path::new(&path);
    if !p.is_absolute() || !p.exists() {
        return Err(ApiError {
            code: "INVALID_PATH".into(),
            message: "Указанный путь не существует или не является абсолютным".into(),
        });
    }
    tauri::async_runtime::spawn_blocking(move || reveal_in_explorer(&path))
        .await
        .map_err(|e| ApiError {
            code: "DISK_WORKER".into(),
            message: e.to_string(),
        })?
        .map_err(|e| ApiError {
            code: "REVEAL_FAILED".into(),
            message: e,
        })
}

fn reveal_in_explorer(path: &str) -> Result<(), String> {
    use windows_sys::Win32::{
        System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED},
        UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName},
    };
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { CoUninitialize() }
        }
    }
    struct Pidl(*mut windows_sys::Win32::UI::Shell::Common::ITEMIDLIST);
    impl Drop for Pidl {
        fn drop(&mut self) {
            unsafe { CoTaskMemFree(self.0.cast()) }
        }
    }
    fn parse(path: &std::path::Path) -> Result<Pidl, String> {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut ptr = std::ptr::null_mut();
        let hr = unsafe {
            SHParseDisplayName(
                wide.as_ptr(),
                std::ptr::null_mut(),
                &mut ptr,
                0,
                std::ptr::null_mut(),
            )
        };
        if hr < 0 {
            Err(format!("Объект недоступен в Проводнике ({hr:#x})"))
        } else {
            Ok(Pidl(ptr))
        }
    }
    let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    if hr < 0 {
        return Err(format!("Не удалось открыть Проводник ({hr:#x})"));
    }
    let _com = Com;
    let path = std::path::Path::new(path);
    let full = parse(path)?;
    let hr = unsafe { SHOpenFolderAndSelectItems(full.0, 0, std::ptr::null(), 0) };
    if hr < 0 {
        Err(format!("Не удалось показать объект ({hr:#x})"))
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn reveal_in_explorer(_path: &str) -> Result<(), String> {
    Err("Проводник доступен только в Windows".into())
}

/// Возвращает список стандартных каталогов Windows для быстрого выбора.
#[tauri::command]
pub fn get_quick_directories(state: State<'_, AppState>) -> Result<Vec<QuickDirectory>, ApiError> {
    Ok(state.disk_manager.get_quick_directories())
}

/// Запускает фоновый анализ структуры и крупнейших файлов каталога.
#[tauri::command]
pub fn start_disk_analysis(state: State<'_, AppState>, path: String) -> Result<String, ApiError> {
    state
        .disk_manager
        .start_disk_analysis(&path)
        .map_err(|err| ApiError {
            code: "DISK_ANALYSIS_ERROR".into(),
            message: err,
        })
}

/// Получение актуального прогресса анализа структуры диска.
#[tauri::command]
pub fn get_disk_analysis_progress(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<DiskTaskStatus, ApiError> {
    state
        .disk_manager
        .get_task_status(&task_id)
        .ok_or_else(|| ApiError {
            code: "TASK_NOT_FOUND".into(),
            message: format!("Задача анализа диска не найдена: {task_id}"),
        })
}

/// Получение готового отчёта анализа структуры диска.
#[tauri::command]
pub async fn get_disk_analysis_report(
    state: State<'_, AppState>,
    report_id: String,
    path: Option<String>,
) -> Result<DiskAnalysisReport, ApiError> {
    let manager = state.disk_manager.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(path) = path {
            manager.get_indexed_directory(&report_id, &path)
        } else {
            manager.get_disk_report(&report_id)
        }
        .ok_or_else(|| ApiError {
            code: "REPORT_NOT_FOUND".into(),
            message: format!("Отчёт анализа диска не найден: {report_id}"),
        })
    })
    .await
    .map_err(|e| ApiError {
        code: "DISK_WORKER".into(),
        message: e.to_string(),
    })?
}

/// Запуск безопасного трёхфазного поиска дубликатов в указанном каталоге.
#[tauri::command]
pub fn start_duplicate_scan(state: State<'_, AppState>, path: String) -> Result<String, ApiError> {
    state
        .disk_manager
        .start_duplicate_scan(&path)
        .map_err(|err| ApiError {
            code: "DUPLICATE_SCAN_ERROR".into(),
            message: err,
        })
}

/// Получение актуального прогресса поиска дубликатов.
#[tauri::command]
pub fn get_duplicate_scan_progress(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<DiskTaskStatus, ApiError> {
    state
        .disk_manager
        .get_task_status(&task_id)
        .ok_or_else(|| ApiError {
            code: "TASK_NOT_FOUND".into(),
            message: format!("Задача поиска дубликатов не найдена: {task_id}"),
        })
}

/// Получение готового отчёта найденных дубликатов.
#[tauri::command]
pub fn get_duplicate_report(
    state: State<'_, AppState>,
    report_id: String,
) -> Result<DuplicateScanResult, ApiError> {
    state
        .disk_manager
        .get_duplicate_report(&report_id)
        .ok_or_else(|| ApiError {
            code: "REPORT_NOT_FOUND".into(),
            message: format!("Отчёт дубликатов не найден: {report_id}"),
        })
}

/// Запрос безопасной отмены любой выполняющейся задачи дискового анализа.
#[tauri::command]
pub fn cancel_disk_task(state: State<'_, AppState>, task_id: String) -> Result<bool, ApiError> {
    Ok(state.disk_manager.cancel_task(&task_id))
}

/// Получение 10-минутного среза истории потребления ресурсов (CPU, RAM, Network).
#[tauri::command]
pub fn get_resource_history(state: State<'_, AppState>) -> Result<ResourceHistory, ApiError> {
    Ok(state.disk_manager.get_resource_history())
}

/// Выборка самых крупных файлов (Space Hogs) из активного MFT-индекса (OPT-02).
#[tauri::command]
pub async fn get_space_hogs(
    state: State<'_, AppState>,
    min_size_mb: Option<u64>,
    category: Option<String>,
) -> Result<Vec<cleaner_core::models::LargeFileEntry>, ApiError> {
    let min_bytes = min_size_mb.unwrap_or(500).saturating_mul(1024 * 1024);
    let cat_filter = category.as_deref().and_then(|cat_str| match cat_str {
        "DiskImage" => Some(cleaner_core::models::LargeFileCategory::DiskImage),
        "Archive" => Some(cleaner_core::models::LargeFileCategory::Archive),
        "Video" => Some(cleaner_core::models::LargeFileCategory::Video),
        "Installer" => Some(cleaner_core::models::LargeFileCategory::Installer),
        "VirtualDisk" => Some(cleaner_core::models::LargeFileCategory::VirtualDisk),
        "Other" => Some(cleaner_core::models::LargeFileCategory::Other),
        _ => None,
    });

    let manager = state.disk_manager.clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager
            .get_space_hogs(min_bytes, 200, cat_filter)
            .ok_or_else(|| ApiError {
                code: "NO_MFT_INDEX".into(),
                message:
                    "Для выборки крупных файлов необходимо сначала запустить анализ диска (MFT)"
                        .into(),
            })
    })
    .await
    .map_err(|e| ApiError {
        code: "DISK_WORKER".into(),
        message: e.to_string(),
    })?
}

/// Сканирование устаревших и неиспользуемых файлов (OPT-03).
#[tauri::command]
pub async fn get_stale_files(
    folder_path: String,
    older_than_days: u32,
    max_results: Option<usize>,
) -> Result<Vec<cleaner_core::models::StaleFileEntry>, ApiError> {
    let p = std::path::PathBuf::from(&folder_path);
    if !p.is_absolute() || !p.exists() || !p.is_dir() {
        return Err(ApiError {
            code: "INVALID_FOLDER".into(),
            message: "Указанный каталог не существует или не является папкой".into(),
        });
    }

    let limit = max_results.unwrap_or(200).clamp(1, 1000);
    tauri::async_runtime::spawn_blocking(move || {
        cleaner_disk::scan_stale_files(&p, older_than_days, limit).map_err(|e| ApiError {
            code: "SCAN_STALE_FAILED".into(),
            message: format!("Ошибка сканирования каталога: {e}"),
        })
    })
    .await
    .map_err(|e| ApiError {
        code: "DISK_WORKER".into(),
        message: e.to_string(),
    })?
}
