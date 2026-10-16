//! Tauri IPC команды для Модулей 6.1–6.6 («Центр обслуживания — v0.3»).
//!
//! Предоставляет безопасный доступ к инвентаризации ПО, автозагрузке,
//! статусу защиты Windows Defender, диагностике здоровья, вызову штатных утилит
//! и защищённому экспорту отчётов в CSV.

use crate::state::AppState;
use cleaner_core::errors::ApiError;
use cleaner_service::{
    AntivirusStatus, CsvExportConfig, CsvExportSection, DiagnosticsSummary, InstalledProgram,
    LaunchResult, StartupItem, SystemToolId,
};
use tauri::State;

/// Возвращает список установленных программ из реестра Windows (HKLM/HKCU, 64/32 бит).
#[tauri::command(async)]
pub fn list_installed_programs() -> Result<Vec<InstalledProgram>, ApiError> {
    cleaner_service::list_installed_programs(false).map_err(ApiError::from)
}

/// Запускает официальный деинсталлятор программы по её идентификатору.
#[tauri::command]
pub fn launch_uninstaller(program_id: String) -> Result<LaunchResult, ApiError> {
    cleaner_service::launch_uninstaller(&program_id).map_err(ApiError::from)
}

/// Возвращает список компонентов автозагрузки Windows (реестр и папки Startup).
#[tauri::command(async)]
pub fn list_startup_items() -> Result<Vec<StartupItem>, ApiError> {
    cleaner_service::list_startup_items().map_err(ApiError::from)
}

/// Возвращает актуальный статус антивирусной защиты и службы Windows Defender.
#[tauri::command(async)]
pub fn get_antivirus_status() -> Result<AntivirusStatus, ApiError> {
    cleaner_service::get_antivirus_status().map_err(ApiError::from)
}

/// Возвращает сводку диагностики здоровья системы с пороговыми предупреждениями.
#[tauri::command(async)]
pub fn get_system_diagnostics() -> Result<DiagnosticsSummary, ApiError> {
    cleaner_service::get_system_diagnostics().map_err(ApiError::from)
}

/// Запускает штатную системную утилиту Windows по строгому белому списку System32.
#[tauri::command]
pub fn launch_system_tool(tool_id: SystemToolId) -> Result<LaunchResult, ApiError> {
    cleaner_service::launch_system_tool(tool_id).map_err(ApiError::from)
}

/// Генерирует локальный CSV-отчёт с нейтрализацией CSV-инъекций.
#[tauri::command(async)]
pub fn generate_csv_report(config: CsvExportConfig) -> Result<String, ApiError> {
    if config.target_path.is_some() {
        return Err(ApiError {
            code: "SERVICE_EXPORT_PATH_DENIED".into(),
            message: "Выберите файл через системный диалог сохранения".into(),
        });
    }

    let programs = if config.sections.contains(&CsvExportSection::Programs)
        || config.sections.contains(&CsvExportSection::All)
        || config.sections.is_empty()
    {
        cleaner_service::list_installed_programs(config.include_system_components)
            .map_err(ApiError::from)?
    } else {
        Vec::new()
    };

    let startup = if config.sections.contains(&CsvExportSection::Startup)
        || config.sections.contains(&CsvExportSection::All)
        || config.sections.is_empty()
    {
        cleaner_service::list_startup_items().map_err(ApiError::from)?
    } else {
        Vec::new()
    };

    let diagnostics =
        cleaner_service::get_system_diagnostics().unwrap_or_else(|_| DiagnosticsSummary {
            health: cleaner_service::HealthStatus::Unknown,
            cpu_usage_percent: None,
            memory_usage_percent: None,
            disk_free_bytes: None,
            disk_total_bytes: None,
            disk_usage_percent: None,
            system_drive: "C:".to_string(),
            alerts: Vec::new(),
            recommendations: Vec::new(),
            timestamp_utc: cleaner_service::diagnostics::utc_now_iso8601(),
        });

    let csv_content =
        cleaner_service::generate_csv_report(&config, &programs, &startup, &diagnostics)
            .map_err(ApiError::from)?;

    Ok(csv_content)
}

/// The webview chooses report sections, never a filesystem write target.
#[tauri::command]
pub async fn save_csv_report(mut config: CsvExportConfig) -> Result<Option<String>, ApiError> {
    config.target_path = None;
    tauri::async_runtime::spawn_blocking(move || {
        let content = generate_csv_report(config)?;
        save_report_dialog(&content)
    })
    .await
    .map_err(|error| ApiError {
        code: "SERVICE_EXPORT_ERROR".into(),
        message: error.to_string(),
    })?
}

#[tauri::command]
pub async fn export_cleanup_journal(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<Option<String>, ApiError> {
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut operations = Vec::new();
        let mut before = None;
        loop {
            let page = storage.audit_page(before, 25).map_err(ApiError::from)?;
            operations.extend(page.operations);
            before = page.next_cursor;
            if before.is_none() || operations.len() >= 200 {
                break;
            }
        }
        let csv = format_journal_report(&operations)?;
        save_report_dialog(&csv)
    })
    .await
    .map_err(|error| ApiError {
        code: "SERVICE_EXPORT_ERROR".into(),
        message: error.to_string(),
    })?
}

fn format_journal_report(
    operations: &[cleaner_core::cleanup::CleanupOutcome],
) -> Result<String, ApiError> {
    use cleaner_service::export::{format_csv_row, UTF8_BOM};
    if operations
        .iter()
        .map(|operation| operation.items.len())
        .sum::<usize>()
        > 10_000
    {
        return Err(ApiError {
            code: "EXPORT_LIMIT".into(),
            message: "Журнал содержит более 10000 объектов в последних 200 операциях".into(),
        });
    }
    let mut csv = String::from(UTF8_BOM);
    csv.push_str(&format_csv_row(&[
        "operation_id",
        "started_at_ms",
        "category",
        "operation_status",
        "object_id",
        "path",
        "size_bytes",
        "item_status",
        "reason_code",
    ]));
    for operation in operations {
        for item in &operation.items {
            csv.push_str(&format_csv_row(&[
                &operation.operation_id,
                &operation.started_at_ms.to_string(),
                &operation.category,
                &format!("{:?}", operation.status),
                &item.object_id,
                &item.display_path,
                &item.size_bytes,
                &format!("{:?}", item.status),
                &item.reason_code,
            ]));
        }
    }
    Ok(csv)
}

#[cfg(windows)]
fn save_report_dialog(content: &str) -> Result<Option<String>, ApiError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetSaveFileNameW, OFN_DONTADDTORECENT, OFN_EXPLORER, OFN_NOCHANGEDIR,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let mut filename = vec![0u16; 32768];
    let suggested: Vec<u16> = "CleanCore-report.csv".encode_utf16().collect();
    filename[..suggested.len()].copy_from_slice(&suggested);
    let filter: Vec<u16> = "CSV (*.csv)\0*.csv\0\0".encode_utf16().collect();
    let extension: Vec<u16> = "csv\0".encode_utf16().collect();
    // SAFETY: all pointers refer to live buffers for the duration of the modal call.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFile = filename.as_mut_ptr();
    dialog.nMaxFile = filename.len() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrDefExt = extension.as_ptr();
    dialog.Flags = OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST | OFN_DONTADDTORECENT;
    if unsafe { GetSaveFileNameW(&mut dialog) } == 0 {
        let code = unsafe { CommDlgExtendedError() };
        return if code == 0 {
            Ok(None)
        } else {
            Err(ApiError {
                code: "SERVICE_EXPORT_DIALOG".into(),
                message: format!("Ошибка диалога сохранения: {code}"),
            })
        };
    }
    let end = filename
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(filename.len());
    let path = std::path::PathBuf::from(std::ffi::OsString::from_wide(&filename[..end]));
    write_new_report(&path, content)?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[cfg(not(windows))]
fn save_report_dialog(_: &str) -> Result<Option<String>, ApiError> {
    Err(ApiError {
        code: "UNSUPPORTED_PLATFORM".into(),
        message: "Диалог доступен в Windows".into(),
    })
}

fn write_new_report(path: &std::path::Path, content: &str) -> Result<(), ApiError> {
    use std::io::Write;
    // Never truncate an existing file, including a link substituted after the dialog.
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()
    })();
    result.map_err(|error| ApiError {
        code: "SERVICE_EXPORT_ERROR".into(),
        message: format!("Не удалось создать CSV (выберите новое имя файла): {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_service::CsvExportSection;

    #[test]
    fn journal_export_escapes_untrusted_paths_and_retains_unknown_status() {
        use cleaner_core::cleanup::*;
        let operation = CleanupOutcome {
            operation_id: "op".into(),
            preview_id: "preview".into(),
            category: "temp".into(),
            started_at_ms: 1,
            status: CleanupOperationStatus::Interrupted,
            recycled_bytes: "0".into(),
            deleted_bytes: "0".into(),
            journal_ok: true,
            items: vec![CleanupItemResult {
                object_id: "item".into(),
                display_path: "=malicious,\"name\"".into(),
                size_bytes: "12".into(),
                status: CleanupItemStatus::Unknown,
                reason_code: "RECOVERED".into(),
            }],
        };
        let csv = format_journal_report(&[operation]).unwrap();
        assert!(csv.starts_with('\u{FEFF}'));
        assert!(csv.contains("Interrupted"));
        assert!(csv.contains("Unknown"));
        assert!(csv.contains("'=malicious"));
        assert!(csv.contains("\"\"name\"\""));
    }

    #[test]
    fn export_cannot_overwrite_existing_files_or_accept_webview_paths() {
        let path =
            std::env::temp_dir().join(format!("cleancore_export_test_{}.csv", std::process::id()));
        std::fs::write(&path, "preserve").unwrap();
        assert!(write_new_report(&path, "replacement").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserve");
        let config = CsvExportConfig {
            sections: vec![CsvExportSection::Programs],
            include_system_components: false,
            target_path: Some(path.to_string_lossy().into_owned()),
        };
        assert_eq!(
            generate_csv_report(config).unwrap_err().code,
            "SERVICE_EXPORT_PATH_DENIED"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserve");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_list_installed_programs_ipc() {
        let res = list_installed_programs();
        assert!(res.is_ok(), "list_installed_programs should return Ok");
    }

    #[test]
    fn test_launch_uninstaller_nonexistent_returns_err() {
        let res = launch_uninstaller("NON_EXISTENT_PROGRAM_ID_12345".to_string());
        assert!(res.is_err(), "Nonexistent program should fail");
        let err = res.unwrap_err();
        assert_eq!(err.code, "SERVICE_PROGRAM_NOT_FOUND");
    }

    #[test]
    fn test_list_startup_items_ipc() {
        let res = list_startup_items();
        assert!(res.is_ok(), "list_startup_items should return Ok");
    }

    #[test]
    fn test_get_antivirus_status_ipc() {
        let res = get_antivirus_status();
        assert!(res.is_ok(), "get_antivirus_status should return Ok");
    }

    #[test]
    fn test_get_system_diagnostics_ipc() {
        let res = get_system_diagnostics();
        assert!(res.is_ok(), "get_system_diagnostics should return Ok");
        let diag = res.unwrap();
        assert!(!diag.system_drive.is_empty());
    }

    #[test]
    fn test_generate_csv_report_ipc() {
        let config = CsvExportConfig {
            sections: vec![CsvExportSection::Programs],
            include_system_components: false,
            target_path: None,
        };
        let res = generate_csv_report(config);
        assert!(res.is_ok(), "generate_csv_report should return Ok");
        let csv = res.unwrap();
        assert!(csv.starts_with('\u{FEFF}'));
    }
}

#[tauri::command(async)]
pub fn set_startup_enabled(
    item_id: String,
    expected_command: String,
    enabled: bool,
    acknowledged: bool,
) -> Result<(), ApiError> {
    cleaner_service::startup_control::set_enabled(
        &item_id,
        &expected_command,
        enabled,
        acknowledged,
    )
    .map_err(ApiError::from)
}

/// Запрос сведений о Корзине Windows (OPT-04).
#[tauri::command(async)]
pub fn query_recycle_bin() -> Result<cleaner_service::RecycleBinInfo, ApiError> {
    cleaner_service::query_recycle_bin().map_err(|e| ApiError {
        code: "RECYCLE_BIN_QUERY_FAILED".into(),
        message: e,
    })
}

/// Очистка Корзины Windows после явного подтверждения (OPT-04).
#[tauri::command(async)]
pub fn empty_recycle_bin() -> Result<cleaner_service::RecycleBinInfo, ApiError> {
    cleaner_service::empty_recycle_bin().map_err(|e| ApiError {
        code: "RECYCLE_BIN_EMPTY_FAILED".into(),
        message: e,
    })
}

/// Сканирование неактивных кэшей разработчика и виртуальных дисков (OPT-04).
#[tauri::command(async)]
pub fn get_dev_cache_summary(
    custom_roots: Option<Vec<String>>,
    inactive_days: Option<u32>,
) -> Result<cleaner_service::DevCacheSummary, ApiError> {
    let roots = custom_roots.map(|paths| paths.into_iter().map(std::path::PathBuf::from).collect());
    Ok(cleaner_service::scan_dev_caches(roots, inactive_days))
}

/// Получение текущего состояния профиля электропитания (OPT-05).
#[tauri::command(async)]
pub fn get_power_profile_state(
    state: State<'_, AppState>,
) -> Result<cleaner_service::PowerProfileStatus, ApiError> {
    let active_scheme = cleaner_service::read_active_scheme().map_err(|e| ApiError {
        code: "POWER_SCHEME_ERROR".into(),
        message: e,
    })?;
    let throttles = cleaner_service::read_throttle_values().map_err(|e| ApiError {
        code: "POWER_THROTTLE_ERROR".into(),
        message: e,
    })?;
    let current_profile = cleaner_service::determine_profile(&throttles);

    let latest_snap = state
        .storage
        .get_latest_power_snapshot()
        .map_err(|e| ApiError {
            code: "STORAGE_ERROR".into(),
            message: e.to_string(),
        })?;

    let (has_rollback_snapshot, last_snapshot_time) = match latest_snap {
        Some(snap) => (true, Some(snap.created_at_utc)),
        None => (false, None),
    };

    Ok(cleaner_service::PowerProfileStatus {
        active_scheme,
        throttles,
        current_profile,
        has_rollback_snapshot,
        last_snapshot_time,
    })
}

/// Применение профиля энергопотребления с сохранением точки отката (OPT-05).
#[tauri::command(async)]
pub fn apply_power_profile(
    state: State<'_, AppState>,
    profile: String,
) -> Result<cleaner_service::PowerProfileStatus, ApiError> {
    let scheme = cleaner_service::read_active_scheme().map_err(|e| ApiError {
        code: "POWER_SCHEME_ERROR".into(),
        message: e,
    })?;
    let throttles = cleaner_service::read_throttle_values().map_err(|e| ApiError {
        code: "POWER_THROTTLE_ERROR".into(),
        message: e,
    })?;

    // 1. Сохраняем снимок перед применением
    state
        .storage
        .save_power_snapshot(
            &scheme.guid,
            &scheme.name,
            throttles.max_ac,
            throttles.min_ac,
            throttles.max_dc,
            throttles.min_dc,
            &profile,
        )
        .map_err(|e| ApiError {
            code: "STORAGE_ERROR".into(),
            message: format!("Не удалось сохранить снимок электропитания в SQLite: {e}"),
        })?;

    // 2. Рассчитываем и применяем целевые параметры
    let target = match profile.as_str() {
        "Quiet" => cleaner_service::PowerThrottleValues {
            max_ac: 99,
            min_ac: 5,
            max_dc: 99,
            min_dc: 5,
        },
        "Performance" => cleaner_service::PowerThrottleValues {
            max_ac: 100,
            min_ac: 100,
            max_dc: 100,
            min_dc: 5,
        },
        _ => cleaner_service::PowerThrottleValues {
            max_ac: 100,
            min_ac: 5,
            max_dc: 100,
            min_dc: 5,
        },
    };

    cleaner_service::apply_throttle_values(&target).map_err(|e| ApiError {
        code: "POWER_APPLY_FAILED".into(),
        message: e,
    })?;

    get_power_profile_state(state)
}

/// Откат настроек питания к последнему сохранённому снимку (OPT-05).
#[tauri::command(async)]
pub fn rollback_power_profile(
    state: State<'_, AppState>,
) -> Result<cleaner_service::PowerProfileStatus, ApiError> {
    let snap = state
        .storage
        .get_latest_power_snapshot()
        .map_err(|e| ApiError {
            code: "STORAGE_ERROR".into(),
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: "NO_ROLLBACK_SNAPSHOT".into(),
            message: "Сохранённые точки отката электропитания отсутствуют".into(),
        })?;

    // Восстанавливаем схему и троттлинг
    let _ = cleaner_service::set_active_scheme(&snap.active_scheme_guid);
    let target = cleaner_service::PowerThrottleValues {
        max_ac: snap.throttle_max_ac,
        min_ac: snap.throttle_min_ac,
        max_dc: snap.throttle_max_dc,
        min_dc: snap.throttle_min_dc,
    };
    cleaner_service::apply_throttle_values(&target).map_err(|e| ApiError {
        code: "POWER_ROLLBACK_FAILED".into(),
        message: e,
    })?;

    get_power_profile_state(state)
}

/// Интеллектуальный советник CleanCore: сбор метрик и генерация персонализированных рекомендаций (OPT-06).
#[tauri::command(async)]
pub fn get_smart_insights(
    processes: Option<Vec<String>>,
    cpu_usage: Option<f32>,
    memory_usage: Option<u8>,
    disk_free_bytes: Option<u64>,
    disk_total_bytes: Option<u64>,
) -> Result<Vec<cleaner_service::SmartInsight>, ApiError> {
    use cleaner_service::advisor::{evaluate_smart_insights, AdvisorInput};

    // 1. Питание и батарея
    let power = cleaner_system::power::read_power_status();
    let is_ac_connected = power.as_ref().map(|p| p.is_ac_connected);
    let battery_percent = power.as_ref().and_then(|p| p.battery_percent);
    let power_save_active = power.as_ref().map(|p| p.power_save_active).unwrap_or(false);

    // 2. Текущий профиль питания
    let current_power_profile = cleaner_service::read_throttle_values()
        .ok()
        .map(|t| cleaner_service::determine_profile(&t));

    // 3. Системная Корзина
    let bin = cleaner_service::query_recycle_bin().ok();
    let recycle_bin_bytes = bin.as_ref().map(|b| b.total_bytes);
    let recycle_bin_items = bin.as_ref().map(|b| b.item_count);

    let input = AdvisorInput {
        is_ac_connected,
        battery_percent,
        power_save_active,
        current_power_profile,
        active_processes: processes.unwrap_or_default(),
        cpu_usage_percent: cpu_usage,
        memory_usage_percent: memory_usage,
        disk_free_bytes,
        disk_total_bytes,
        recycle_bin_bytes,
        recycle_bin_items,
        dev_cache_bytes: None,
        dev_cache_count: None,
    };

    Ok(evaluate_smart_insights(&input))
}
