//! Сбор и расчёт системных метрик (CPU, RAM, диски, сеть).

use cleaner_core::models::{
    CpuMetrics, DiskMetrics, FieldValue, MemoryMetrics, MetricSnapshot, NetworkMetrics,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System};

/// Интервал минимального обновления дисковых показателей (30 секунд согласно ROADMAP).
pub const DISK_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

/// Структура предыдущего замера сетевого интерфейса.
#[derive(Debug, Clone)]
struct NetworkSample {
    total_rx_bytes: u64,
    total_tx_bytes: u64,
}

/// Адаптер системного мониторинга на базе `sysinfo`.
pub struct SystemCollector {
    sys: System,
    disks: Disks,
    networks: Networks,
    sample_count: u64,
    last_disk_refresh: Option<Instant>,
    cached_disk_metrics: FieldValue<DiskMetrics>,
    prev_network_samples: HashMap<String, NetworkSample>,
    last_network_instant: Option<Instant>,
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemCollector {
    /// Инициализирует сборщик метрик.
    /// При создании выполняет первичное наполнение структур без объявления данных готовыми.
    pub fn new() -> Self {
        let refresh_kind = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything())
            .with_processes(sysinfo::ProcessRefreshKind::everything());

        let mut sys = System::new_with_specifics(refresh_kind);
        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        Self {
            sys,
            disks,
            networks,
            sample_count: 0,
            last_disk_refresh: None,
            cached_disk_metrics: FieldValue::NotMeasured,
            prev_network_samples: HashMap::new(),
            last_network_instant: None,
        }
    }

    /// Производит замер и возвращает снимок системных показателей.
    /// На первом замере CPU возвращает `FieldValue::NotMeasured`, так как дельта времени ещё не сформирована.
    pub fn collect_snapshot(&mut self, interval_ms: u64) -> MetricSnapshot {
        self.sample_count = self.sample_count.saturating_add(1);

        // 1. CPU
        self.sys.refresh_cpu_all();
        let cpu_metric = if self.sample_count <= 1 {
            // Первый замер sysinfo не содержит рассчитанной дельты времени загрузки CPU.
            // Честно возвращаем NotMeasured без выдуманных нулей.
            FieldValue::NotMeasured
        } else {
            let total_usage = self.sys.global_cpu_usage();
            let core_count = self.sys.cpus().len();
            let frequency_mhz = self.sys.cpus().first().map(|c| c.frequency());

            FieldValue::Available(CpuMetrics {
                total_usage_percent: total_usage.clamp(0.0, 100.0),
                core_count,
                frequency_mhz,
            })
        };

        // 2. RAM
        self.sys.refresh_memory();
        let total_ram = self.sys.total_memory();
        let used_ram = self.sys.used_memory();
        let memory_metric = if total_ram > 0 {
            let usage_percent = ((used_ram as f64 / total_ram as f64) * 100.0).round() as u8;
            FieldValue::Available(MemoryMetrics {
                used_bytes: used_ram,
                total_bytes: total_ram,
                usage_percent: usage_percent.min(100),
            })
        } else {
            FieldValue::Unavailable("Объём оперативной памяти не определён".to_string())
        };

        // 3. Диски (опрос не чаще чем раз в 30 секунд согласно ТЗ)
        let disk_metric = self.collect_disk_metrics();

        // 4. Сеть (расчёт скоростей приёма/передачи)
        let network_metric = self.collect_network_metrics();

        // Формирование снимка с отметкой времени в UTC
        let now_utc = chrono_or_simple_utc_now();

        MetricSnapshot {
            timestamp_utc: now_utc,
            interval_ms,
            cpu: cpu_metric,
            memory: memory_metric,
            disk: disk_metric,
            network: network_metric,
        }
    }

    /// Сбор дисковых метрик с ограничением частоты вызова.
    fn collect_disk_metrics(&mut self) -> FieldValue<DiskMetrics> {
        let now = Instant::now();
        let should_refresh = match self.last_disk_refresh {
            Some(last) => now.duration_since(last) >= DISK_REFRESH_INTERVAL,
            None => true,
        };

        if should_refresh {
            self.disks.refresh(true);
            self.last_disk_refresh = Some(now);

            // Ищем основной системный диск (обычно 'C:' на Windows) или первый доступный диск с ненулевым объёмом
            let primary_disk = self
                .disks
                .iter()
                .find(|d| {
                    let mount = d.mount_point().to_string_lossy().to_uppercase();
                    mount.starts_with("C:")
                })
                .or_else(|| self.disks.iter().find(|d| d.total_space() > 0));

            if let Some(disk) = primary_disk {
                let total = disk.total_space();
                let free = disk.available_space();
                let mount = disk.mount_point().to_string_lossy().to_string();

                let usage_percent = if total > 0 {
                    let used = total.saturating_sub(free);
                    ((used as f64 / total as f64) * 100.0).round() as u8
                } else {
                    0
                };

                self.cached_disk_metrics = FieldValue::Available(DiskMetrics {
                    mount_point: mount,
                    free_bytes: free,
                    total_bytes: total,
                    usage_percent: usage_percent.min(100),
                });
            } else if self.disks.is_empty() {
                self.cached_disk_metrics =
                    FieldValue::Unavailable("Локальные накопители не обнаружены".to_string());
            } else {
                self.cached_disk_metrics = FieldValue::AccessDenied;
            }
        }

        self.cached_disk_metrics.clone()
    }

    /// Сбор сетевых метрик и расчёт скорости передачи/приёма байт/с.
    fn collect_network_metrics(&mut self) -> FieldValue<NetworkMetrics> {
        let now = Instant::now();
        self.networks.refresh(true);

        let elapsed_sec = self
            .last_network_instant
            .map(|prev| now.duration_since(prev).as_secs_f64())
            .unwrap_or(0.0);

        self.last_network_instant = Some(now);

        // Если это первый замер скорости или прошло слишком мало времени (< 100 мс)
        if elapsed_sec < 0.1 || self.prev_network_samples.is_empty() {
            // Сохраняем текущие базисные счётчики для последующего расчёта дельты
            self.prev_network_samples.clear();
            let mut active_iface: Option<String> = None;

            for (iface_name, data) in &self.networks {
                let rx = data.total_received();
                let tx = data.total_transmitted();
                self.prev_network_samples.insert(
                    iface_name.clone(),
                    NetworkSample {
                        total_rx_bytes: rx,
                        total_tx_bytes: tx,
                    },
                );
                if active_iface.is_none() && (rx > 0 || tx > 0) {
                    active_iface = Some(iface_name.clone());
                }
            }

            return FieldValue::Available(NetworkMetrics {
                rx_bytes_per_sec: 0,
                tx_bytes_per_sec: 0,
                interface_name: active_iface.or_else(|| Some("Сеть".to_string())),
            });
        }

        // Вычисляем суммарную скорость по всем активным сетевым интерфейсам
        let mut sum_rx_speed = 0u64;
        let mut sum_tx_speed = 0u64;
        let mut best_interface: Option<String> = None;
        let mut max_speed = 0u64;

        for (iface_name, data) in &self.networks {
            let current_rx = data.total_received();
            let current_tx = data.total_transmitted();

            if let Some(prev) = self.prev_network_samples.get_mut(iface_name) {
                // Защита от сброса счётчиков операционной системой при перезапуске адаптера:
                // если счётчик уменьшился, то delta = 0 и обновляем базис.
                let rx_delta = current_rx.saturating_sub(prev.total_rx_bytes);
                let tx_delta = current_tx.saturating_sub(prev.total_tx_bytes);

                let rx_speed = (rx_delta as f64 / elapsed_sec).round() as u64;
                let tx_speed = (tx_delta as f64 / elapsed_sec).round() as u64;

                sum_rx_speed = sum_rx_speed.saturating_add(rx_speed);
                sum_tx_speed = sum_tx_speed.saturating_add(tx_speed);

                let iface_speed = rx_speed.saturating_add(tx_speed);
                if iface_speed > max_speed || best_interface.is_none() {
                    max_speed = iface_speed;
                    best_interface = Some(iface_name.clone());
                }

                prev.total_rx_bytes = current_rx;
                prev.total_tx_bytes = current_tx;
            } else {
                self.prev_network_samples.insert(
                    iface_name.clone(),
                    NetworkSample {
                        total_rx_bytes: current_rx,
                        total_tx_bytes: current_tx,
                    },
                );
            }
        }

        FieldValue::Available(NetworkMetrics {
            rx_bytes_per_sec: sum_rx_speed,
            tx_bytes_per_sec: sum_tx_speed,
            interface_name: best_interface.or_else(|| Some("Ethernet/Wi-Fi".to_string())),
        })
    }

    /// Ссылка на экземпляр `System` для повторного использования модулем процессов.
    pub fn system(&self) -> &System {
        &self.sys
    }

    /// Мутабельная ссылка на `System`.
    pub fn system_mut(&mut self) -> &mut System {
        &mut self.sys
    }
}

/// Формирует читаемую строку текущего времени UTC без тяжёлых зависимостей.
fn chrono_or_simple_utc_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now();
    let duration = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();

    let sec = total_secs % 60;
    let min = (total_secs / 60) % 60;
    let hour = (total_secs / 3600) % 24;

    // Вычисление даты (григорианский календарь от 1970-01-01)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_collect_returns_not_measured_for_cpu() {
        let mut collector = SystemCollector::new();
        let snapshot = collector.collect_snapshot(2000);

        // Первый замер CPU обязан быть NotMeasured
        assert_eq!(snapshot.cpu, FieldValue::NotMeasured);
        assert_eq!(snapshot.interval_ms, 2000);
        assert!(!snapshot.timestamp_utc.is_empty());
    }

    #[test]
    fn test_memory_is_measured_when_available() {
        let mut collector = SystemCollector::new();
        let snapshot = collector.collect_snapshot(2000);

        // RAM на реальной системе доступна сразу
        if let FieldValue::Available(mem) = &snapshot.memory {
            assert!(mem.total_bytes > 0);
            assert!(mem.usage_percent <= 100);
        }
    }

    #[test]
    fn test_utc_timestamp_formatting() {
        let ts = chrono_or_simple_utc_now();
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20); // YYYY-MM-DDTHH:MM:SSZ
    }
}
