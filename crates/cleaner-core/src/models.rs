//! Модели данных, DTO и структуры состояния CleanCore.

use serde::{Deserialize, Serialize};

/// Статус доступности значения системного датчика или свойства.
/// Предотвращает подмену отсутствующих данных нулями или фиктивными числами.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "status", content = "value")]
pub enum FieldValue<T> {
    /// Значение получено и достоверно.
    Available(T),
    /// Значение ещё не замерялось (например, первый замер CPU).
    #[default]
    NotMeasured,
    /// Доступ к данным запрещён операционной системой.
    AccessDenied,
    /// Значение недоступно по указанной причине.
    Unavailable(String),
}

impl<T> FieldValue<T> {
    pub fn is_available(&self) -> bool {
        matches!(self, FieldValue::Available(_))
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            FieldValue::Available(val) => Some(val),
            _ => None,
        }
    }
}

/// Метрики центрального процессора.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuMetrics {
    /// Общая загрузка CPU в процентах (0.0 .. 100.0).
    pub total_usage_percent: f32,
    /// Количество логических ядер.
    pub core_count: usize,
    /// Текущая базовая частота (МГц), если доступна.
    pub frequency_mhz: Option<u64>,
}

/// Метрики оперативной памяти.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Использованный объём в байтах.
    pub used_bytes: u64,
    /// Общий объём установленной памяти в байтах.
    pub total_bytes: u64,
    /// Процент занятой памяти (0 .. 100).
    pub usage_percent: u8,
}

/// Метрики накопителя (диска).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskMetrics {
    /// Точка монтирования или буква диска (например, "C:").
    pub mount_point: String,
    /// Свободное место в байтах.
    pub free_bytes: u64,
    /// Общий объём в байтах.
    pub total_bytes: u64,
    /// Процент занятого места (0 .. 100).
    pub usage_percent: u8,
}

/// Метрики сетевого адаптера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// Скорость приёма (байт/с).
    pub rx_bytes_per_sec: u64,
    /// Скорость передачи (байт/с).
    pub tx_bytes_per_sec: u64,
    /// Имя активного интерфейса, если определено.
    pub interface_name: Option<String>,
}

/// Полный снимок системных показателей в момент времени.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSnapshot {
    /// Время формирования снимка в формате ISO 8601 UTC.
    pub timestamp_utc: String,
    /// Интервал измерения в миллисекундах.
    pub interval_ms: u64,
    /// Данные процессора.
    pub cpu: FieldValue<CpuMetrics>,
    /// Данные оперативной памяти.
    pub memory: FieldValue<MemoryMetrics>,
    /// Данные системного диска.
    pub disk: FieldValue<DiskMetrics>,
    /// Данные сети.
    pub network: FieldValue<NetworkMetrics>,
}

/// Сведения об одном запущенном процессе.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessItem {
    /// Идентификатор процесса (PID).
    pub pid: u32,
    /// Название исполняемого файла процесса.
    pub name: String,
    /// Доля использования CPU (0.0 .. 100.0).
    pub cpu_percent: FieldValue<f32>,
    /// Объём используемой физической памяти (RSS в байтах).
    pub memory_bytes: FieldValue<u64>,
    /// Доступный путь к исполняемому файлу (может быть недоступен без elevation).
    pub executable_path: FieldValue<String>,
}

/// Снимок списка процессов.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    pub timestamp_utc: String,
    pub processes: Vec<ProcessItem>,
}

/// Жизненный цикл асинхронной задачи.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Partial,
    Cancelled,
    Failed,
}

/// Статус длительной операции (сканирование, экспорт и т.д.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStatus {
    pub id: String,
    pub kind: String,
    pub state: TaskState,
    pub processed_items: u64,
    pub total_items: Option<u64>,
    pub error_message: Option<String>,
}

/// Сведения о приложении и текущем окружении.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    /// Строгий локальный режим без сетевых запросов.
    pub is_local_mode: bool,
    /// Запущено ли приложение с правами администратора (для MVP должно быть false).
    pub is_admin: Option<bool>,
    pub settings_persistent: bool,
    /// Идентификатор выбранной темы оформления.
    pub theme: String,
}

/// Пользовательские настройки приложения CleanCore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    /// Интервал фонового опроса метрик в миллисекундах (по умолчанию 2000 мс).
    pub refresh_interval_ms: u64,
    /// Идентификатор цветовой темы оформления.
    pub theme: String,
    /// Состояние ручной паузы мониторинга.
    pub is_monitoring_paused: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 2000,
            theme: "studio-pearl".to_string(),
            is_monitoring_paused: false,
        }
    }
}

/// Кандидат на очистку, обнаруженный сканером.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupCandidate {
    /// Уникальный безопасный идентификатор объекта в рамках сессии сканирования.
    pub obj_id: String,
    /// Отображаемый путь к файлу.
    pub display_path: String,
    /// Абсолютный путь к файлу на диске (для повторной проверки перед действием).
    pub absolute_path: String,
    /// Размер файла в байтах.
    pub size_bytes: String,
    /// Идентификатор правила/категории очистки (например, "user-temp-v1").
    pub category_id: String,
    /// Причина включения файла в список кандидатов.
    pub reason: String,
    /// Время последнего изменения файла (UTC ISO 8601).
    pub modified_time_utc: String,
    /// Оценка потенциального риска.
    pub risk_level: crate::safety::RiskLevel,
}

/// Порция списка кандидатов для постраничной выдачи в интерфейс без блокировки UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPreviewPage {
    pub preview_id: String,
    pub category_id: String,
    pub offset: usize,
    pub limit: usize,
    pub total_candidates: usize,
    pub unfiltered_candidates: usize,
    pub total_bytes: String,
    pub items: Vec<CleanupCandidate>,
    pub rule_version: String,
    pub complete: bool,
    pub cancelled: bool,
    pub diagnostics: std::collections::BTreeMap<String, u64>,
    pub execution_preview_id: Option<String>,
}
