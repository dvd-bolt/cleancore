//! Сбор и нормализация сведений о запущенных процессах.

use cleaner_core::models::{FieldValue, ProcessItem, ProcessSnapshot};
use std::collections::HashMap;
use std::time::Instant;
use sysinfo::{ProcessesToUpdate, System};

/// Собирает актуальный снимок запущенных процессов системы.
///
/// Безопасность и ограничения:
/// - Все операции выполняются строго в пространстве непривилегированного пользователя.
/// - Недоступные системные пути возвращаются с честным статусом `FieldValue::AccessDenied`.
/// - Загрузка CPU каждого процесса приводится к шкале общей мощности системы (0.0 .. 100.0%)
///   путём деления на число логических ядер процессора.
/// - Модуль не содержит никаких команд или методов для завершения (kill) процессов.
pub fn collect_processes(sys: &mut System) -> ProcessSnapshot {
    collect_processes_with_baselines(sys, &mut HashMap::new())
}

pub(crate) fn collect_processes_with_baselines(
    sys: &mut System,
    baselines: &mut HashMap<u32, (u64, Instant)>,
) -> ProcessSnapshot {
    // Обновляем список процессов
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let now = Instant::now();
    baselines.retain(|pid, _| sys.process(sysinfo::Pid::from_u32(*pid)).is_some());

    let core_count = sys.cpus().len().max(1) as f32;
    let mut items = Vec::with_capacity(sys.processes().len());

    for (pid, proc) in sys.processes() {
        let pid_u32 = pid.as_u32();
        let name = proc.name().to_string_lossy().to_string();

        // Нормализация использования CPU: приведение к шкале 0..100%
        let raw_cpu = proc.cpu_usage();
        let warmed = baselines.get(&pid_u32).is_some_and(|(started, at)| {
            *started == proc.start_time()
                && now.duration_since(*at) >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL
        });
        let cpu_field = if !warmed {
            FieldValue::NotMeasured
        } else if raw_cpu.is_finite() && raw_cpu >= 0.0 {
            FieldValue::Available((raw_cpu / core_count).clamp(0.0, 100.0))
        } else {
            FieldValue::Unavailable("Недостоверный замер CPU процесса".into())
        };
        baselines.insert(pid_u32, (proc.start_time(), now));

        // Память процесса (RSS в байтах)
        let memory_bytes = proc.memory();
        let memory_field = FieldValue::Available(memory_bytes);

        // Доступный путь к исполняемому файлу
        let exe_path = match proc.exe() {
            Some(path) if !path.as_os_str().is_empty() => {
                FieldValue::Available(path.to_string_lossy().to_string())
            }
            Some(_) => FieldValue::Unavailable("Путь не указан".to_string()),
            None => {
                // В Windows без прав администратора доступ к путям системных процессов закрыт ОС
                FieldValue::Unavailable("Путь процесса недоступен".into())
            }
        };

        items.push(ProcessItem {
            pid: pid_u32,
            name,
            cpu_percent: cpu_field,
            memory_bytes: memory_field,
            executable_path: exe_path,
        });
    }

    // По умолчанию сортируем процессы по убыванию загрузки CPU,
    // а при равном CPU — по объёму используемой памяти (RSS), имени и PID
    items.sort_by(|a, b| {
        let cpu_a = a.cpu_percent.value().copied().unwrap_or(0.0);
        let cpu_b = b.cpu_percent.value().copied().unwrap_or(0.0);
        cpu_b
            .partial_cmp(&cpu_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let mem_a = a.memory_bytes.value().copied().unwrap_or(0);
                let mem_b = b.memory_bytes.value().copied().unwrap_or(0);
                mem_b.cmp(&mem_a)
            })
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.pid.cmp(&b.pid))
    });

    ProcessSnapshot {
        timestamp_utc: crate::metrics::chrono_or_simple_utc_now(),
        processes: items,
    }
}

/// Фильтрует список процессов по строке поиска (имя или PID).
pub fn filter_processes(processes: &[ProcessItem], search: &str) -> Vec<ProcessItem> {
    let query = search.trim().to_lowercase();
    if query.is_empty() {
        return processes.to_vec();
    }

    processes
        .iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&query)
                || p.pid.to_string().contains(&query)
                || p.executable_path
                    .value()
                    .map(|path| path.to_lowercase().contains(&query))
                    .unwrap_or(false)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_normalization_within_bounds() {
        let mut sys = System::new_all();
        let snapshot = collect_processes(&mut sys);

        // В работающей ОС процессы всегда присутствуют
        assert!(!snapshot.processes.is_empty());

        for p in &snapshot.processes {
            if let FieldValue::Available(cpu) = p.cpu_percent {
                assert!((0.0..=100.0).contains(&cpu));
            }
        }
    }

    #[test]
    fn test_filter_processes_by_name() {
        let items = vec![
            ProcessItem {
                pid: 100,
                name: "cargo.exe".to_string(),
                cpu_percent: FieldValue::Available(1.5),
                memory_bytes: FieldValue::Available(1024),
                executable_path: FieldValue::Available("C:\\bin\\cargo.exe".to_string()),
            },
            ProcessItem {
                pid: 200,
                name: "explorer.exe".to_string(),
                cpu_percent: FieldValue::Available(0.5),
                memory_bytes: FieldValue::Available(2048),
                executable_path: FieldValue::Available("C:\\Windows\\explorer.exe".to_string()),
            },
        ];

        let filtered = filter_processes(&items, "cargo");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].pid, 100);

        let filtered_pid = filter_processes(&items, "200");
        assert_eq!(filtered_pid.len(), 1);
        assert_eq!(filtered_pid[0].name, "explorer.exe");
    }
}
