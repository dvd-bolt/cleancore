//! Модуль 6.4: Обоснованная диагностика состояния и сводка здоровья системы.

use crate::types::{DiagnosticAlert, DiagnosticsSummary, HealthStatus, ServiceError};
use std::time::SystemTime;

/// Порог предупреждения по свободному месту на системном диске (15 ГБ).
pub const DISK_WARNING_THRESHOLD_BYTES: u64 = 15 * 1024 * 1024 * 1024;

/// Критический порог по свободному месту на системном диске (10 ГБ).
pub const DISK_CRITICAL_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// Порог предупреждения по загрузке RAM (80%).
pub const MEMORY_WARNING_THRESHOLD_PERCENT: u8 = 80;

/// Критический порог по загрузке RAM (92%).
pub const MEMORY_CRITICAL_THRESHOLD_PERCENT: u8 = 92;

/// Порог предупреждения по нагрузке на процессор (85%).
pub const CPU_WARNING_THRESHOLD_PERCENT: f32 = 85.0;

/// Порог предупреждения по количеству активных элементов автозагрузки (15 шт).
pub const STARTUP_WARNING_THRESHOLD_COUNT: usize = 15;

/// Формирует строку текущего времени UTC в формате ISO 8601 (YYYY-MM-DDTHH:MM:SSZ).
pub fn utc_now_iso8601() -> String {
    let now = SystemTime::now();
    let duration = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();

    let sec = total_secs % 60;
    let min = (total_secs / 60) % 60;
    let hour = (total_secs / 3600) % 24;

    let days = (total_secs / 86400) as i64;
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn days_to_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Чистая функция оценки диагностических правил и генерации рекомендаций.
pub fn evaluate_diagnostics_rules(
    cpu_usage_percent: Option<f32>,
    memory_usage_percent: Option<u8>,
    disk_free_bytes: Option<u64>,
    disk_total_bytes: Option<u64>,
    system_drive: &str,
    startup_items_count: usize,
) -> (HealthStatus, Vec<DiagnosticAlert>, Vec<String>) {
    let mut alerts = Vec::new();
    let mut recommendations = Vec::new();

    // 1. Проверка свободного места на системном диске
    if let Some(free) = disk_free_bytes {
        let free_gb = free as f64 / (1024.0 * 1024.0 * 1024.0);
        if free < DISK_CRITICAL_THRESHOLD_BYTES {
            let rec = "Освободите место на системном диске (рекомендуется очистка временных файлов через CleanCore или Очистку диска Windows).".to_string();
            alerts.push(DiagnosticAlert {
                id: "disk_critical".to_string(),
                level: HealthStatus::Critical,
                category: "disk".to_string(),
                title: "Критически мало свободного места на диске".to_string(),
                message: format!(
                    "На системном диске {} осталось всего {:.1} ГБ свободного места (< 10 ГБ). Это может приводить к сбоям Windows и блокировке обновлений.",
                    system_drive, free_gb
                ),
                recommendation: Some(rec.clone()),
            });
            recommendations.push(rec);
        } else if free < DISK_WARNING_THRESHOLD_BYTES {
            let rec = "Рекомендуется провести профилактическую очистку кэша и временных файлов."
                .to_string();
            alerts.push(DiagnosticAlert {
                id: "disk_warning".to_string(),
                level: HealthStatus::Warning,
                category: "disk".to_string(),
                title: "Мало свободного места на диске".to_string(),
                message: format!(
                    "На системном диске {} осталось {:.1} ГБ свободного места (< 15 ГБ).",
                    system_drive, free_gb
                ),
                recommendation: Some(rec.clone()),
            });
            recommendations.push(rec);
        }
    }

    // 2. Проверка загрузки оперативной памяти
    if let Some(mem) = memory_usage_percent {
        if mem > MEMORY_CRITICAL_THRESHOLD_PERCENT {
            let rec = "Закройте неиспользуемые ресурсоёмкие приложения или вкладки браузера для предотвращения исчерпания памяти.".to_string();
            alerts.push(DiagnosticAlert {
                id: "memory_critical".to_string(),
                level: HealthStatus::Critical,
                category: "memory".to_string(),
                title: "Критическая загрузка оперативной памяти".to_string(),
                message: format!(
                    "Оперативная память загружена на {}% (> {}%). Возможны зависания интерфейса и сбои фоновых задач.",
                    mem, MEMORY_CRITICAL_THRESHOLD_PERCENT
                ),
                recommendation: Some(rec.clone()),
            });
            recommendations.push(rec);
        } else if mem > MEMORY_WARNING_THRESHOLD_PERCENT {
            let rec =
                "Рекомендуется проверить список активных процессов и закрыть лишние программы."
                    .to_string();
            alerts.push(DiagnosticAlert {
                id: "memory_warning".to_string(),
                level: HealthStatus::Warning,
                category: "memory".to_string(),
                title: "Высокая загрузка оперативной памяти".to_string(),
                message: format!(
                    "Оперативная память загружена на {}% (> {}%).",
                    mem, MEMORY_WARNING_THRESHOLD_PERCENT
                ),
                recommendation: Some(rec.clone()),
            });
            recommendations.push(rec);
        }
    }

    // 3. Проверка загрузки процессора
    if let Some(cpu) = cpu_usage_percent {
        if cpu > CPU_WARNING_THRESHOLD_PERCENT {
            let rec = "Рекомендуется проверить фоновые процессы в Диспетчере задач или Мониторе ресурсов.".to_string();
            alerts.push(DiagnosticAlert {
                id: "cpu_warning".to_string(),
                level: HealthStatus::Warning,
                category: "cpu".to_string(),
                title: "Высокая нагрузка на процессор".to_string(),
                message: format!(
                    "Нагрузка на процессор составляет {:.1}% (> {:.0}%).",
                    cpu, CPU_WARNING_THRESHOLD_PERCENT
                ),
                recommendation: Some(rec.clone()),
            });
            recommendations.push(rec);
        }
    }

    // 4. Проверка количества элементов автозагрузки
    if startup_items_count > STARTUP_WARNING_THRESHOLD_COUNT {
        let rec = "Рекомендуется проверить автозапуск в Центре обслуживания и отключить неиспользуемые программы.".to_string();
        alerts.push(DiagnosticAlert {
            id: "startup_warning".to_string(),
            level: HealthStatus::Warning,
            category: "startup".to_string(),
            title: "Много программ в автозапуске".to_string(),
            message: format!(
                "В автозагрузке активно {} программ (> {} шт). Это может замедлять запуск операционной системы.",
                startup_items_count, STARTUP_WARNING_THRESHOLD_COUNT
            ),
            recommendation: Some(rec.clone()),
        });
        recommendations.push(rec);
    }

    // 5. Итоговый статус здоровья
    let has_critical = alerts.iter().any(|a| a.level == HealthStatus::Critical);
    let has_warning = alerts.iter().any(|a| a.level == HealthStatus::Warning);

    let health = if has_critical {
        HealthStatus::Critical
    } else if has_warning {
        HealthStatus::Warning
    } else if cpu_usage_percent.is_some()
        || memory_usage_percent.is_some()
        || disk_free_bytes.is_some()
    {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unknown
    };

    if recommendations.is_empty() && health == HealthStatus::Healthy {
        recommendations.push(
            "Система работает в штатном режиме, критических проблем не обнаружено.".to_string(),
        );
    }

    let _ = disk_total_bytes;
    (health, alerts, recommendations)
}

fn sysinfo_minimum_interval() -> std::time::Duration {
    std::time::Duration::from_millis(250)
}

/// Собирает актуальные системные метрики и формирует сводку диагностики здоровья.
pub fn get_system_diagnostics() -> Result<DiagnosticsSummary, ServiceError> {
    let mut collector = cleaner_system::metrics::SystemCollector::new();
    collector.collect_snapshot(0);
    std::thread::sleep(sysinfo_minimum_interval());
    let snapshot = collector.collect_snapshot(250);

    let cpu_usage_percent = snapshot.cpu.value().map(|c| c.total_usage_percent);
    let memory_usage_percent = snapshot.memory.value().map(|m| m.usage_percent);

    let (disk_free_bytes, disk_total_bytes, disk_usage_percent, system_drive) =
        match snapshot.disk.value() {
            Some(d) => (
                Some(d.free_bytes),
                Some(d.total_bytes),
                Some(d.usage_percent),
                d.mount_point.clone(),
            ),
            None => (None, None, None, "C:".to_string()),
        };

    let startup_items = crate::startup::list_startup_items().unwrap_or_default();
    let startup_count = startup_items.iter().filter(|i| i.enabled).count();

    let (health, alerts, recommendations) = evaluate_diagnostics_rules(
        cpu_usage_percent,
        memory_usage_percent,
        disk_free_bytes,
        disk_total_bytes,
        &system_drive,
        startup_count,
    );

    Ok(DiagnosticsSummary {
        health,
        cpu_usage_percent,
        memory_usage_percent,
        disk_free_bytes,
        disk_total_bytes,
        disk_usage_percent,
        system_drive,
        alerts,
        recommendations,
        timestamp_utc: utc_now_iso8601(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_critical_threshold() {
        let free_bytes = 8 * 1024 * 1024 * 1024; // 8 GB (< 10 GB)
        let total_bytes = 256 * 1024 * 1024 * 1024;
        let (health, alerts, recs) = evaluate_diagnostics_rules(
            Some(20.0),
            Some(50),
            Some(free_bytes),
            Some(total_bytes),
            "C:",
            5,
        );

        assert_eq!(health, HealthStatus::Critical);
        assert!(alerts.iter().any(|a| a.id == "disk_critical"));
        assert!(recs.iter().any(|r| r.contains("Освободите место")));
    }

    #[test]
    fn test_disk_warning_threshold() {
        let free_bytes = 12 * 1024 * 1024 * 1024; // 12 GB (>= 10 GB and < 15 GB)
        let total_bytes = 256 * 1024 * 1024 * 1024;
        let (health, alerts, recs) = evaluate_diagnostics_rules(
            Some(20.0),
            Some(50),
            Some(free_bytes),
            Some(total_bytes),
            "C:",
            5,
        );

        assert_eq!(health, HealthStatus::Warning);
        assert!(alerts.iter().any(|a| a.id == "disk_warning"));
        assert!(recs.iter().any(|r| r.contains("профилактическую очистку")));
    }

    #[test]
    fn test_memory_critical_threshold() {
        let (health, alerts, _) = evaluate_diagnostics_rules(
            Some(30.0),
            Some(95), // > 92%
            Some(100 * 1024 * 1024 * 1024),
            Some(256 * 1024 * 1024 * 1024),
            "C:",
            5,
        );

        assert_eq!(health, HealthStatus::Critical);
        assert!(alerts.iter().any(|a| a.id == "memory_critical"));
    }

    #[test]
    fn test_memory_warning_threshold() {
        let (health, alerts, _) = evaluate_diagnostics_rules(
            Some(30.0),
            Some(85), // > 80% and <= 92%
            Some(100 * 1024 * 1024 * 1024),
            Some(256 * 1024 * 1024 * 1024),
            "C:",
            5,
        );

        assert_eq!(health, HealthStatus::Warning);
        assert!(alerts.iter().any(|a| a.id == "memory_warning"));
    }

    #[test]
    fn test_cpu_warning_threshold() {
        let (health, alerts, _) = evaluate_diagnostics_rules(
            Some(89.5), // > 85%
            Some(50),
            Some(100 * 1024 * 1024 * 1024),
            Some(256 * 1024 * 1024 * 1024),
            "C:",
            5,
        );

        assert_eq!(health, HealthStatus::Warning);
        assert!(alerts.iter().any(|a| a.id == "cpu_warning"));
    }

    #[test]
    fn test_startup_warning_threshold() {
        let (health, alerts, _) = evaluate_diagnostics_rules(
            Some(20.0),
            Some(50),
            Some(100 * 1024 * 1024 * 1024),
            Some(256 * 1024 * 1024 * 1024),
            "C:",
            22, // > 15
        );

        assert_eq!(health, HealthStatus::Warning);
        assert!(alerts.iter().any(|a| a.id == "startup_warning"));
    }

    #[test]
    fn test_healthy_status_when_all_normal() {
        let (health, alerts, recs) = evaluate_diagnostics_rules(
            Some(15.0),
            Some(45),
            Some(50 * 1024 * 1024 * 1024), // 50 GB free
            Some(256 * 1024 * 1024 * 1024),
            "C:",
            6,
        );

        assert_eq!(health, HealthStatus::Healthy);
        assert!(alerts.is_empty());
        assert!(recs.iter().any(|r| r.contains("в штатном режиме")));
    }

    #[test]
    fn test_utc_now_iso8601_format() {
        let ts = utc_now_iso8601();
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20); // YYYY-MM-DDTHH:MM:SSZ
    }
}
