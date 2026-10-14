//! Tauri IPC команды для Модулей 6.1–6.6 («Центр обслуживания — v0.3»).
//!
//! Предоставляет безопасный доступ к инвентаризации ПО, автозагрузке,
//! статусу защиты Windows Defender, диагностике здоровья, вызову штатных утилит
//! и защищённому экспорту отчётов в CSV.

use cleaner_core::errors::ApiError;
use cleaner_service::{
    AntivirusStatus, CsvExportConfig, CsvExportSection, DiagnosticsSummary, InstalledProgram,
    LaunchResult, StartupItem, SystemToolId,
};

/// Возвращает список установленных программ из реестра Windows (HKLM/HKCU, 64/32 бит).
#[tauri::command]
pub fn list_installed_programs() -> Result<Vec<InstalledProgram>, ApiError> {
    cleaner_service::list_installed_programs(false).map_err(ApiError::from)
}

/// Запускает официальный деинсталлятор программы по её идентификатору.
#[tauri::command]
pub fn launch_uninstaller(program_id: String) -> Result<LaunchResult, ApiError> {
    cleaner_service::launch_uninstaller(&program_id).map_err(ApiError::from)
}

/// Возвращает список компонентов автозагрузки Windows (реестр и папки Startup).
#[tauri::command]
pub fn list_startup_items() -> Result<Vec<StartupItem>, ApiError> {
    cleaner_service::list_startup_items().map_err(ApiError::from)
}

/// Возвращает актуальный статус антивирусной защиты и службы Windows Defender.
#[tauri::command]
pub fn get_antivirus_status() -> Result<AntivirusStatus, ApiError> {
    cleaner_service::get_antivirus_status().map_err(ApiError::from)
}

/// Возвращает сводку диагностики здоровья системы с пороговыми предупреждениями.
#[tauri::command]
pub fn get_system_diagnostics() -> Result<DiagnosticsSummary, ApiError> {
    cleaner_service::get_system_diagnostics().map_err(ApiError::from)
}

/// Запускает штатную системную утилиту Windows по строгому белому списку System32.
#[tauri::command]
pub fn launch_system_tool(tool_id: SystemToolId) -> Result<LaunchResult, ApiError> {
    cleaner_service::launch_system_tool(tool_id).map_err(ApiError::from)
}

/// Генерирует локальный CSV-отчёт с нейтрализацией CSV-инъекций.
#[tauri::command]
pub fn generate_csv_report(config: CsvExportConfig) -> Result<String, ApiError> {
    let programs = if config.sections.contains(&CsvExportSection::Programs)
        || config.sections.contains(&CsvExportSection::All)
        || config.sections.is_empty()
    {
        cleaner_service::list_installed_programs(config.include_system_components)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let startup = if config.sections.contains(&CsvExportSection::Startup)
        || config.sections.contains(&CsvExportSection::All)
        || config.sections.is_empty()
    {
        cleaner_service::list_startup_items().unwrap_or_default()
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

    if let Some(target_path) = &config.target_path {
        std::fs::write(target_path, &csv_content).map_err(|e| ApiError {
            code: "SERVICE_EXPORT_ERROR".into(),
            message: format!("Не удалось сохранить файл отчёта '{target_path}': {e}"),
        })?;
    }

    Ok(csv_content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_service::CsvExportSection;

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
