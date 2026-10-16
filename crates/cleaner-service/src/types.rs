//! Общие типы данных и контракты модуля Центра обслуживания CleanCore.

use cleaner_core::errors::ApiError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Информация об установленной программе из реестра Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledProgram {
    /// Уникальный идентификатор программы в CleanCore.
    pub id: String,
    /// Отображаемое имя программы.
    pub display_name: String,
    /// Издатель / разработчик.
    pub publisher: Option<String>,
    /// Версия программы.
    pub display_version: Option<String>,
    /// Дата установки (в формате YYYYMMDD или текстовом).
    pub install_date: Option<String>,
    /// Расчётный размер на диске в байтах (сконвертирован из KiB реестра).
    pub estimated_size_bytes: Option<u64>,
    /// Официальная строка деинсталляции.
    pub uninstall_string: Option<String>,
    /// Команда тихой деинсталляции.
    pub quiet_uninstall_string: Option<String>,
    /// Каталог установки программы.
    pub install_location: Option<String>,
    /// Ветка реестра, из которой получена запись (HKLM или HKCU).
    pub registry_hive: String,
    /// Признак 32-битного приложения на 64-битной ОС (WOW64).
    pub is_wow64: bool,
    /// Флаг системного компонента или вспомогательного обновления.
    pub is_system_component: bool,
}

/// Источник элемента автозагрузки Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupSource {
    /// Ветка реестра Run (HKCU или HKLM).
    RegistryRun,
    /// Ветка реестра RunOnce (HKCU или HKLM).
    RegistryRunOnce,
    /// Каталог автозагрузки текущего пользователя.
    StartupFolderUser,
    /// Общесистемный каталог автозагрузки (ProgramData).
    StartupFolderCommon,
}

/// Элемент автозагрузки Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupItem {
    /// Уникальный идентификатор записи автозапуска.
    pub id: String,
    /// Отображаемое имя параметра или ярлыка.
    pub name: String,
    /// Команда запуска или путь к исполняемому файлу/ярлыку.
    pub command: String,
    /// Тип источника автозагрузки.
    pub source: StartupSource,
    /// Точное расположение (ветка реестра или путь к директории).
    pub location: String,
    /// Статус активности (включён / отключён в StartupApproved).
    pub enabled: bool,
    /// Проверка фактического существования исполняемого файла на диске.
    pub target_exists: bool,
}

/// Состояние антивирусной защиты системы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionState {
    /// Защита активна, сигнатуры актуальны.
    Protected,
    /// Защита требует внимания (устарели сигнатуры или отключён RTP).
    Warning,
    /// Защита полностью отключена.
    Disabled,
    /// Ограниченный доступ к метаданным (нет прав, заблокировано политиками).
    LimitedAccess,
    /// Состояние не удалось определить.
    Unknown,
}

/// Сведения о зарегистрированном поставщике антивирусной защиты.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntivirusProvider {
    /// Отображаемое имя антивирусного продукта.
    pub name: String,
    /// Путь к основному исполняемому файлу антивируса.
    pub product_exe: Option<String>,
    /// Путь к сервисному бинарнику отчётности.
    pub reporting_exe: Option<String>,
    /// Сырой код состояния из Security Center (битовая маска STATE).
    pub state_code: Option<u32>,
    /// Флаг активности защиты в реальном времени (Real-Time Protection).
    pub real_time_protection: bool,
    /// Флаг актуальности баз сигнатур.
    pub definitions_up_to_date: bool,
}

/// Сводный статус антивирусной защиты и службы Windows Defender.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntivirusStatus {
    /// Общее состояние защиты.
    pub state: ProtectionState,
    /// Список обнаруженных антивирусных провайдеров.
    pub providers: Vec<AntivirusProvider>,
    /// Состояние службы Windows Defender (WinDefend).
    pub defender_service_running: Option<bool>,
    /// Понятное пользователю локализованное сообщение.
    pub message: String,
}

/// Уровень здоровья системы в сводке диагностики.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Все ключевые показатели в норме.
    Healthy,
    /// Обнаружены некритические отклонения или рекомендации.
    Warning,
    /// Обнаружены критические проблемы (нехватка диска, экстремальная нагрузка).
    Critical,
    /// Недостаточно данных для оценки здоровья.
    Unknown,
}

/// Диагностическое предупреждение или рекомендация.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticAlert {
    /// Идентификатор предупреждения.
    pub id: String,
    /// Уровень важности.
    pub level: HealthStatus,
    /// Категория метрики ("disk", "memory", "cpu", "security", "startup").
    pub category: String,
    /// Краткий заголовок проблемы.
    pub title: String,
    /// Подробное пояснение.
    pub message: String,
    /// Рекомендуемое действие для пользователя.
    pub recommendation: Option<String>,
}

/// Сводка системной диагностики и рекомендаций.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticsSummary {
    /// Общий статус здоровья компьютера.
    pub health: HealthStatus,
    /// Процент общей загрузки CPU (если доступен).
    pub cpu_usage_percent: Option<f32>,
    /// Процент использования оперативной памяти.
    pub memory_usage_percent: Option<u8>,
    /// Свободное место на системном диске в байтах.
    pub disk_free_bytes: Option<u64>,
    /// Общий объём системного диска в байтах.
    pub disk_total_bytes: Option<u64>,
    /// Процент заполнения системного диска.
    pub disk_usage_percent: Option<u8>,
    /// Буква или точка монтирования системного диска (обычно "C:").
    pub system_drive: String,
    /// Список выявленных предупреждений.
    pub alerts: Vec<DiagnosticAlert>,
    /// Итоговый список рекомендаций по оптимизации.
    pub recommendations: Vec<String>,
    /// Отметка времени сбора метрик в формате UTC ISO 8601.
    pub timestamp_utc: String,
}

/// Строгий перечень идентификаторов штатных утилит Windows System32.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemToolId {
    /// Очистка диска Windows (`cleanmgr.exe`).
    Cleanmgr,
    /// Оптимизация и дефрагментация дисков (`dfrgui.exe`).
    Dfrgui,
    /// Монитор ресурсов Windows (`resmon.exe`).
    Resmon,
    /// Диспетчер задач Windows (`Taskmgr.exe`).
    Taskmgr,
    /// Программы и компоненты (`control.exe appwiz.cpl`).
    Appwiz,
}

/// Описание штатной системной утилиты Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemTool {
    /// Идентификатор утилиты из белого списка.
    pub id: SystemToolId,
    /// Русскоязычное название инструмента.
    pub name: String,
    /// Краткое назначение утилиты.
    pub description: String,
    /// Имя исполняемого файла в каталоге System32.
    pub executable: String,
    /// Аргументы командной строки при запуске.
    pub arguments: Vec<String>,
    /// Флаг фактического наличия утилиты в текущей системе.
    pub is_available: bool,
}

/// Результат запуска внешнего процесса (деинсталлятора или системной утилиты).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchResult {
    /// Флаг успешного запуска процесса.
    pub success: bool,
    /// Идентификатор запущенного процесса (PID), если доступен.
    pub pid: Option<u32>,
    /// Сообщение о результате запуска для пользователя.
    pub message: String,
}

/// Разделы для включения в экспортируемый отчёт.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CsvExportSection {
    /// Список установленных программ.
    Programs,
    /// Элементы автозагрузки.
    Startup,
    /// Системная диагностика и здоровье ПК.
    Diagnostics,
    /// Все разделы.
    All,
}

/// Параметры конфигурации генерации CSV-отчёта.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvExportConfig {
    /// Выбранные секции для включения в отчёт.
    pub sections: Vec<CsvExportSection>,
    /// Включать ли скрытые системные компоненты в список программ.
    pub include_system_components: bool,
    /// Целевой путь сохранения файла (опционально).
    pub target_path: Option<String>,
}

/// Типизированные ошибки сервисного модуля CleanCore.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ServiceError {
    #[error("Ошибка доступа к реестру Windows: {0}")]
    RegistryError(String),

    #[error("Программа с идентификатором '{0}' не найдена")]
    ProgramNotFound(String),

    #[error("Строка деинсталляции программы '{0}' отсутствует")]
    NoUninstallString(String),

    #[error("Некорректная или небезопасная команда запуска: {0}")]
    InvalidCommand(String),

    #[error("Исполняемый файл не найден на диске: {0}")]
    ExecutableNotFound(String),

    #[error("Запрещённый идентификатор утилиты Windows: {0}")]
    UnauthorizedTool(String),

    #[error("Не удалось запустить процесс: {0}")]
    ProcessLaunchFailed(String),

    #[error("Ошибка формирования CSV-отчёта: {0}")]
    ExportError(String),

    #[error("Доступ ограничен политиками безопасности Windows: {0}")]
    AccessDenied(String),

    #[error("Ошибка получения системной информации: {0}")]
    SystemInfoError(String),
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        let code = match &err {
            ServiceError::RegistryError(_) => "SERVICE_REGISTRY_ERROR",
            ServiceError::ProgramNotFound(_) => "SERVICE_PROGRAM_NOT_FOUND",
            ServiceError::NoUninstallString(_) => "SERVICE_NO_UNINSTALL_STRING",
            ServiceError::InvalidCommand(_) => "SERVICE_INVALID_COMMAND",
            ServiceError::ExecutableNotFound(_) => "SERVICE_EXECUTABLE_NOT_FOUND",
            ServiceError::UnauthorizedTool(_) => "SERVICE_UNAUTHORIZED_TOOL",
            ServiceError::ProcessLaunchFailed(_) => "SERVICE_PROCESS_LAUNCH_FAILED",
            ServiceError::ExportError(_) => "SERVICE_EXPORT_ERROR",
            ServiceError::AccessDenied(_) => "SERVICE_ACCESS_DENIED",
            ServiceError::SystemInfoError(_) => "SERVICE_SYSTEM_INFO_ERROR",
        };
        ApiError {
            code: code.to_string(),
            message: err.to_string(),
        }
    }
}

/// Информация о состоянии Корзины Windows (OPT-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecycleBinInfo {
    pub total_bytes: u64,
    pub item_count: u64,
}

/// Запись о заброшенном кэше разработчика или виртуальном диске (OPT-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevCacheEntry {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub size_bytes: u64,
    pub last_modified_utc: String,
    pub inactive_days: u32,
}

/// Сводка по кэшам разработчика (OPT-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevCacheSummary {
    pub total_bytes: u64,
    pub node_modules_count: usize,
    pub node_modules_bytes: u64,
    pub target_count: usize,
    pub target_bytes: u64,
    pub venv_count: usize,
    pub venv_bytes: u64,
    pub wsl_docker_bytes: u64,
    pub entries: Vec<DevCacheEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_error_to_api_error_mapping() {
        let errs = [
            (
                ServiceError::RegistryError("Access error".to_string()),
                "SERVICE_REGISTRY_ERROR",
            ),
            (
                ServiceError::ProgramNotFound("prog1".to_string()),
                "SERVICE_PROGRAM_NOT_FOUND",
            ),
            (
                ServiceError::NoUninstallString("App".to_string()),
                "SERVICE_NO_UNINSTALL_STRING",
            ),
            (
                ServiceError::InvalidCommand("cmd.exe".to_string()),
                "SERVICE_INVALID_COMMAND",
            ),
            (
                ServiceError::ExecutableNotFound("calc.exe".to_string()),
                "SERVICE_EXECUTABLE_NOT_FOUND",
            ),
            (
                ServiceError::UnauthorizedTool("forbidden".to_string()),
                "SERVICE_UNAUTHORIZED_TOOL",
            ),
            (
                ServiceError::ProcessLaunchFailed("Failed".to_string()),
                "SERVICE_PROCESS_LAUNCH_FAILED",
            ),
            (
                ServiceError::ExportError("CSV error".to_string()),
                "SERVICE_EXPORT_ERROR",
            ),
            (
                ServiceError::AccessDenied("Sec policies".to_string()),
                "SERVICE_ACCESS_DENIED",
            ),
            (
                ServiceError::SystemInfoError("Sysinfo".to_string()),
                "SERVICE_SYSTEM_INFO_ERROR",
            ),
        ];

        for (err, expected_code) in errs {
            let api_err: ApiError = err.into();
            assert_eq!(api_err.code, expected_code);
            assert!(!api_err.message.is_empty());
        }
    }

    #[test]
    fn test_types_serialization_roundtrip() {
        let prog = InstalledProgram {
            id: "HKLM_64_Test".to_string(),
            display_name: "Test App".to_string(),
            publisher: Some("Publisher".to_string()),
            display_version: Some("1.0.0".to_string()),
            install_date: Some("20261014".to_string()),
            estimated_size_bytes: Some(1024 * 1024),
            uninstall_string: Some("uninst.exe".to_string()),
            quiet_uninstall_string: None,
            install_location: Some("C:\\App".to_string()),
            registry_hive: "HKLM".to_string(),
            is_wow64: false,
            is_system_component: false,
        };

        let json = serde_json::to_string(&prog).unwrap();
        let deserialized: InstalledProgram = serde_json::from_str(&json).unwrap();
        assert_eq!(prog, deserialized);

        let startup = StartupItem {
            id: "HKCU_Run_App".to_string(),
            name: "App".to_string(),
            command: "app.exe".to_string(),
            source: StartupSource::RegistryRun,
            location: "HKCU\\Run".to_string(),
            enabled: true,
            target_exists: true,
        };
        let s_json = serde_json::to_string(&startup).unwrap();
        let s_deser: StartupItem = serde_json::from_str(&s_json).unwrap();
        assert_eq!(startup, s_deser);

        let status = AntivirusStatus {
            state: ProtectionState::Protected,
            providers: vec![AntivirusProvider {
                name: "Windows Defender".to_string(),
                product_exe: None,
                reporting_exe: None,
                state_code: Some(0x61100),
                real_time_protection: true,
                definitions_up_to_date: true,
            }],
            defender_service_running: Some(true),
            message: "Защита активна".to_string(),
        };
        let av_json = serde_json::to_string(&status).unwrap();
        let av_deser: AntivirusStatus = serde_json::from_str(&av_json).unwrap();
        assert_eq!(status, av_deser);
    }
}
