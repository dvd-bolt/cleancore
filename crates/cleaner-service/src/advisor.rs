//! Модуль OPT-06: Интеллектуальный советник и Auto-Pilot Watcher.
//!
//! Анализирует комплексное состояние рабочей станции (источник питания, активные
//! компиляторы/IDE, состояние Корзины, дисковое пространство, кэши проектов и RAM)
//! и формирует обоснованные рекомендации в Pearl-стиле без спама.

use serde::{Deserialize, Serialize};

/// Модель интеллектуальной рекомендации CleanCore.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SmartInsight {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub level: String, // "info" | "tip" | "warning" | "critical"
    pub estimated_savings: Option<String>,
    pub action_label: String,
    pub target_page: String,
    pub target_tab: Option<String>,
}

/// Входные данные для расчёта рекомендаций.
#[derive(Debug, Clone, Default)]
pub struct AdvisorInput {
    pub is_ac_connected: Option<bool>,
    pub battery_percent: Option<f64>,
    pub power_save_active: bool,
    pub current_power_profile: Option<String>,
    pub active_processes: Vec<String>,
    pub cpu_usage_percent: Option<f32>,
    pub memory_usage_percent: Option<u8>,
    pub disk_free_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub recycle_bin_bytes: Option<u64>,
    pub recycle_bin_items: Option<u64>,
    pub dev_cache_bytes: Option<u64>,
    pub dev_cache_count: Option<usize>,
}

/// Форматирует байты в удобную читаемую строку.
pub fn format_insight_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    if bytes >= GIB {
        format!("{:.1} ГБ", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} МБ", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.0} КБ", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} Б")
    }
}

/// Список известных компиляторов, сред разработки и тяжёлых инструментов сборщика.
pub const HEAVY_DEV_PROCESSES: &[&str] = &[
    "rustc.exe",
    "cargo.exe",
    "cl.exe",
    "msbuild.exe",
    "gcc.exe",
    "g++.exe",
    "clang.exe",
    "dotnet.exe",
    "devenv.exe",
    "code.exe",
    "idea64.exe",
    "pycharm64.exe",
];

/// Проверяет, запущены ли в системе тяжёлые процессы разработки.
pub fn is_heavy_dev_workload_active(processes: &[String]) -> bool {
    processes.iter().any(|p| {
        let lower = p.to_ascii_lowercase();
        HEAVY_DEV_PROCESSES
            .iter()
            .any(|dev| lower == *dev || lower.starts_with(dev))
    })
}

/// Чистая функция генерации интеллектуальных рекомендаций на основе системных метрик.
pub fn evaluate_smart_insights(input: &AdvisorInput) -> Vec<SmartInsight> {
    let mut insights = Vec::new();

    // 1. Анализ питания: батарея и необходимость тихого профиля
    if input.is_ac_connected == Some(false)
        && input.current_power_profile.as_deref() != Some("Quiet")
    {
        insights.push(SmartInsight {
            id: "power_battery_quiet".to_string(),
            title: "Работа от аккумулятора: тихий режим".to_string(),
            description: "Отключение агрессивного турбобуста (лимит 99%) снижает нагрев процессора на 5–15°C и продлевает время автономной работы ноутбука.".to_string(),
            category: "power".to_string(),
            level: "tip".to_string(),
            estimated_savings: Some("+20–35% к автономности".to_string()),
            action_label: "Настроить питание".to_string(),
            target_page: "service".to_string(),
            target_tab: Some("diagnostics".to_string()),
        });
    }

    // 2. Auto-Pilot: Нагрузка разработчика / компиляция на питании от сети
    let dev_workload = is_heavy_dev_workload_active(&input.active_processes);
    if dev_workload
        && input.is_ac_connected != Some(false)
        && input.current_power_profile.as_deref() != Some("Performance")
    {
        insights.push(SmartInsight {
            id: "power_heavy_dev".to_string(),
            title: "Обнаружена компиляция / разработка".to_string(),
            description: "Запущены среды разработки или компиляторы кода. Профиль «Максимальная мощность» удерживает пиковые частоты CPU без задержек множителя.".to_string(),
            category: "power".to_string(),
            level: "tip".to_string(),
            estimated_savings: Some("Минимальные задержки сборки".to_string()),
            action_label: "Включить производительность".to_string(),
            target_page: "service".to_string(),
            target_tab: Some("diagnostics".to_string()),
        });
    }

    // 3. Анализ системной Корзины Windows
    if let (Some(bytes), Some(items)) = (input.recycle_bin_bytes, input.recycle_bin_items) {
        if bytes >= 100 * 1024 * 1024 && items > 0 {
            let is_large = bytes >= 1024 * 1024 * 1024;
            insights.push(SmartInsight {
                id: "recycle_bin_full".to_string(),
                title: "Накопление файлов в Корзине".to_string(),
                description: format!(
                    "В системной Корзине Windows находится {items} удалённых файлов объёмом {}.",
                    format_insight_bytes(bytes)
                ),
                category: "recycle_bin".to_string(),
                level: if is_large { "warning" } else { "info" }.to_string(),
                estimated_savings: Some(format_insight_bytes(bytes)),
                action_label: "Очистить Корзину".to_string(),
                target_page: "service".to_string(),
                target_tab: Some("dev_recycle".to_string()),
            });
        }
    }

    // 4. Неактивные кэши разработки (node_modules, target, .venv)
    if let Some(bytes) = input.dev_cache_bytes {
        if bytes >= 500 * 1024 * 1024 {
            let count_info = input
                .dev_cache_count
                .map(|c| format!(" ({c} каталогов)"))
                .unwrap_or_default();
            insights.push(SmartInsight {
                id: "dev_caches_reclaim".to_string(),
                title: "Неактивные кэши проектов".to_string(),
                description: format!(
                    "Обнаружены неактивные каталоги сборки проектов node_modules / target / .venv суммарно на {}{count_info}.",
                    format_insight_bytes(bytes)
                ),
                category: "dev_cache".to_string(),
                level: "tip".to_string(),
                estimated_savings: Some(format_insight_bytes(bytes)),
                action_label: "Проверить кэши".to_string(),
                target_page: "service".to_string(),
                target_tab: Some("dev_recycle".to_string()),
            });
        }
    }

    // 5. Дисковое пространство и крупные файлы (Space Hogs)
    if let Some(free) = input.disk_free_bytes {
        const LOW_DISK_THRESHOLD: u64 = 20 * 1024 * 1024 * 1024;
        const CRITICAL_DISK_THRESHOLD: u64 = 10 * 1024 * 1024 * 1024;

        if free < LOW_DISK_THRESHOLD {
            let is_critical = free < CRITICAL_DISK_THRESHOLD;
            insights.push(SmartInsight {
                id: "disk_space_hogs".to_string(),
                title: if is_critical {
                    "Критически мало места на диске".to_string()
                } else {
                    "Низкий объём свободного места".to_string()
                },
                description: format!(
                    "На системном накопителе осталось всего {}. Рекомендуется проверить категорию крупных файлов (Space Hogs) и старые загрузки.",
                    format_insight_bytes(free)
                ),
                category: "disk".to_string(),
                level: if is_critical { "critical" } else { "warning" }.to_string(),
                estimated_savings: Some("Освобождение диска".to_string()),
                action_label: "Анализ крупных файлов".to_string(),
                target_page: "disk".to_string(),
                target_tab: Some("space_hogs".to_string()),
            });
        }
    }

    // 6. Высокая загрузка оперативной памяти
    if let Some(mem) = input.memory_usage_percent {
        if mem >= 85 {
            let is_critical = mem >= 92;
            insights.push(SmartInsight {
                id: "memory_pressure".to_string(),
                title: if is_critical {
                    "Критическая загрузка оперативной памяти".to_string()
                } else {
                    "Высокая нагрузка на оперативную память".to_string()
                },
                description: format!(
                    "Оперативная память загружена на {mem}%. Рекомендуется завершить неиспользуемые фоновые процессы."
                ),
                category: "memory".to_string(),
                level: if is_critical { "critical" } else { "warning" }.to_string(),
                estimated_savings: Some("Снижение нагрузки RAM".to_string()),
                action_label: "Диспетчер процессов".to_string(),
                target_page: "processes".to_string(),
                target_tab: None,
            });
        }
    }

    // 7. Если нет никаких замечаний — возвращаем статус идеального состояния
    if insights.is_empty() {
        insights.push(SmartInsight {
            id: "all_systems_optimal".to_string(),
            title: "Система в оптимальном состоянии".to_string(),
            description: "Свободное место в норме, системная Корзина не переполнена, параметры электропитания сбалансированы.".to_string(),
            category: "system".to_string(),
            level: "info".to_string(),
            estimated_savings: None,
            action_label: "Центр обслуживания".to_string(),
            target_page: "service".to_string(),
            target_tab: None,
        });
    }

    insights
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_insight_bytes() {
        assert_eq!(format_insight_bytes(500), "500 Б");
        assert_eq!(format_insight_bytes(1024), "1 КБ");
        assert_eq!(format_insight_bytes(1048576), "1.0 МБ");
        assert_eq!(format_insight_bytes(1073741824), "1.0 ГБ");
    }

    #[test]
    fn test_is_heavy_dev_workload() {
        assert!(is_heavy_dev_workload_active(&[
            "code.exe".to_string(),
            "explorer.exe".to_string()
        ]));
        assert!(is_heavy_dev_workload_active(&["RUSTC.EXE".to_string()]));
        assert!(!is_heavy_dev_workload_active(&[
            "notepad.exe".to_string(),
            "calc.exe".to_string()
        ]));
    }

    #[test]
    fn test_all_systems_optimal_when_no_issues() {
        let input = AdvisorInput {
            is_ac_connected: Some(true),
            battery_percent: None,
            power_save_active: false,
            current_power_profile: Some("Balanced".to_string()),
            active_processes: vec!["explorer.exe".to_string()],
            cpu_usage_percent: Some(15.0),
            memory_usage_percent: Some(40),
            disk_free_bytes: Some(100 * 1024 * 1024 * 1024),
            disk_total_bytes: Some(500 * 1024 * 1024 * 1024),
            recycle_bin_bytes: Some(5 * 1024 * 1024), // 5 MB, under threshold
            recycle_bin_items: Some(3),
            dev_cache_bytes: None,
            dev_cache_count: None,
        };

        let insights = evaluate_smart_insights(&input);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].id, "all_systems_optimal");
        assert_eq!(insights[0].level, "info");
    }

    #[test]
    fn test_battery_power_advises_quiet() {
        let input = AdvisorInput {
            is_ac_connected: Some(false),
            battery_percent: Some(45.0),
            power_save_active: false,
            current_power_profile: Some("Balanced".to_string()),
            active_processes: vec![],
            cpu_usage_percent: Some(10.0),
            memory_usage_percent: Some(40),
            disk_free_bytes: Some(100 * 1024 * 1024 * 1024),
            disk_total_bytes: Some(500 * 1024 * 1024 * 1024),
            recycle_bin_bytes: None,
            recycle_bin_items: None,
            dev_cache_bytes: None,
            dev_cache_count: None,
        };

        let insights = evaluate_smart_insights(&input);
        assert!(insights.iter().any(|i| i.id == "power_battery_quiet"));
    }

    #[test]
    fn test_heavy_dev_workload_advises_performance() {
        let input = AdvisorInput {
            is_ac_connected: Some(true),
            battery_percent: None,
            power_save_active: false,
            current_power_profile: Some("Balanced".to_string()),
            active_processes: vec!["cargo.exe".to_string(), "rustc.exe".to_string()],
            cpu_usage_percent: Some(80.0),
            memory_usage_percent: Some(50),
            disk_free_bytes: Some(100 * 1024 * 1024 * 1024),
            disk_total_bytes: Some(500 * 1024 * 1024 * 1024),
            recycle_bin_bytes: None,
            recycle_bin_items: None,
            dev_cache_bytes: None,
            dev_cache_count: None,
        };

        let insights = evaluate_smart_insights(&input);
        assert!(insights.iter().any(|i| i.id == "power_heavy_dev"));
    }

    #[test]
    fn test_recycle_bin_full_advises_cleanup() {
        let input = AdvisorInput {
            is_ac_connected: Some(true),
            battery_percent: None,
            power_save_active: false,
            current_power_profile: Some("Balanced".to_string()),
            active_processes: vec![],
            cpu_usage_percent: Some(10.0),
            memory_usage_percent: Some(40),
            disk_free_bytes: Some(100 * 1024 * 1024 * 1024),
            disk_total_bytes: Some(500 * 1024 * 1024 * 1024),
            recycle_bin_bytes: Some(2 * 1024 * 1024 * 1024), // 2 GB
            recycle_bin_items: Some(42),
            dev_cache_bytes: None,
            dev_cache_count: None,
        };

        let insights = evaluate_smart_insights(&input);
        let bin_insight = insights
            .iter()
            .find(|i| i.id == "recycle_bin_full")
            .unwrap();
        assert_eq!(bin_insight.level, "warning");
        assert_eq!(bin_insight.target_tab, Some("dev_recycle".to_string()));
    }

    #[test]
    fn test_low_disk_and_memory_pressure_triggers_critical() {
        let input = AdvisorInput {
            is_ac_connected: Some(true),
            battery_percent: None,
            power_save_active: false,
            current_power_profile: Some("Balanced".to_string()),
            active_processes: vec![],
            cpu_usage_percent: Some(10.0),
            memory_usage_percent: Some(94),                // > 92%
            disk_free_bytes: Some(8 * 1024 * 1024 * 1024), // 8 GB (< 10 GB)
            disk_total_bytes: Some(500 * 1024 * 1024 * 1024),
            recycle_bin_bytes: None,
            recycle_bin_items: None,
            dev_cache_bytes: None,
            dev_cache_count: None,
        };

        let insights = evaluate_smart_insights(&input);
        assert!(insights
            .iter()
            .any(|i| i.id == "disk_space_hogs" && i.level == "critical"));
        assert!(insights
            .iter()
            .any(|i| i.id == "memory_pressure" && i.level == "critical"));
    }
}
