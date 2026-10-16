//! Модуль 5.5: Хранение 10-минутной истории метрик (CPU, RAM, Сеть) в кольцевом буфере памяти.

use cleaner_core::models::{FieldValue, MetricSnapshot};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::models::ResourceHistory;

const MAX_HISTORY_SAMPLES: usize = 1201; // 10 minutes at the minimum 500 ms interval.
const HISTORY_WINDOW: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
struct HistoricalSample {
    recorded_at: Instant,
    timestamp_utc: String,
    cpu_percent: Option<f32>,
    memory_percent: Option<f32>,
    network_rx_bytes_sec: Option<u64>,
    network_tx_bytes_sec: Option<u64>,
}

pub struct ResourceHistoryTracker {
    samples: Mutex<VecDeque<HistoricalSample>>,
}

impl Default for ResourceHistoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceHistoryTracker {
    pub fn new() -> Self {
        Self {
            samples: Mutex::new(VecDeque::with_capacity(MAX_HISTORY_SAMPLES)),
        }
    }

    /// Регистрирует очередной срез метрик системы.
    pub fn record_snapshot(&self, snapshot: &MetricSnapshot) {
        self.record_at(snapshot, Instant::now());
    }

    fn record_at(&self, snapshot: &MetricSnapshot, now: Instant) {
        let cpu_val = match &snapshot.cpu {
            FieldValue::Available(m) if m.total_usage_percent.is_finite() => {
                Some(m.total_usage_percent)
            }
            _ => None,
        };

        let mem_val = match &snapshot.memory {
            FieldValue::Available(m) => Some(m.usage_percent as f32),
            _ => None,
        };

        let (rx, tx) = match &snapshot.network {
            FieldValue::Available(m) => (Some(m.rx_bytes_per_sec), Some(m.tx_bytes_per_sec)),
            _ => (None, None),
        };

        let sample = HistoricalSample {
            recorded_at: now,
            timestamp_utc: snapshot.timestamp_utc.clone(),
            cpu_percent: cpu_val,
            memory_percent: mem_val,
            network_rx_bytes_sec: rx,
            network_tx_bytes_sec: tx,
        };

        let mut samples = self.samples.lock().unwrap();
        prune(&mut samples, now);
        if samples.len() >= MAX_HISTORY_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(sample);
    }

    /// Возвращает агрегированную историю ресурсов для отображения графиков нагрузки.
    pub fn get_history(&self) -> ResourceHistory {
        let mut samples = self.samples.lock().unwrap();
        prune(&mut samples, Instant::now());

        let mut timestamps_utc = Vec::with_capacity(samples.len());
        let mut cpu_percentages = Vec::with_capacity(samples.len());
        let mut memory_percentages = Vec::with_capacity(samples.len());
        let mut network_rx_bytes_sec = Vec::with_capacity(samples.len());
        let mut network_tx_bytes_sec = Vec::with_capacity(samples.len());

        for s in samples.iter() {
            timestamps_utc.push(s.timestamp_utc.clone());
            cpu_percentages.push(s.cpu_percent);
            memory_percentages.push(s.memory_percent);
            network_rx_bytes_sec.push(s.network_rx_bytes_sec);
            network_tx_bytes_sec.push(s.network_tx_bytes_sec);
        }

        ResourceHistory {
            timestamps_utc,
            cpu_percentages,
            memory_percentages,
            network_rx_bytes_sec,
            network_tx_bytes_sec,
        }
    }
}

fn prune(samples: &mut VecDeque<HistoricalSample>, now: Instant) {
    while samples
        .front()
        .is_some_and(|sample| now.saturating_duration_since(sample.recorded_at) > HISTORY_WINDOW)
    {
        samples.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::models::{CpuMetrics, MemoryMetrics, NetworkMetrics};

    #[test]
    fn test_history_tracker_records_and_bounds() {
        let tracker = ResourceHistoryTracker::new();

        for i in 0..1250 {
            let snapshot = MetricSnapshot {
                disks: Vec::new(),
                network_interfaces: Vec::new(),
                timestamp_utc: format!("2026-10-14T12:{i:02}:00Z"),
                interval_ms: 1000,
                cpu: FieldValue::Available(CpuMetrics {
                    total_usage_percent: 25.0,
                    core_count: 4,
                    frequency_mhz: Some(3600),
                }),
                memory: FieldValue::Available(MemoryMetrics {
                    used_bytes: 8_000_000_000,
                    total_bytes: 16_000_000_000,
                    usage_percent: 50,
                }),
                disk: FieldValue::NotMeasured,
                network: FieldValue::Available(NetworkMetrics {
                    rx_bytes_per_sec: 1024,
                    tx_bytes_per_sec: 2048,
                    interface_name: Some("Ethernet".into()),
                }),
                power: None,
            };
            tracker.record_snapshot(&snapshot);
        }

        let history = tracker.get_history();
        assert_eq!(history.timestamps_utc.len(), MAX_HISTORY_SAMPLES);
        assert_eq!(history.cpu_percentages.len(), MAX_HISTORY_SAMPLES);
        assert_eq!(history.cpu_percentages[0], Some(25.0));
        assert_eq!(history.memory_percentages[0], Some(50.0));
        assert_eq!(history.network_rx_bytes_sec[0], Some(1024));
        assert_eq!(history.network_tx_bytes_sec[0], Some(2048));
    }

    #[test]
    fn unavailable_values_are_null_and_window_is_time_based() {
        let tracker = ResourceHistoryTracker::new();
        let snapshot = MetricSnapshot {
            disks: Vec::new(),
            network_interfaces: Vec::new(),
            timestamp_utc: "2026-10-14T12:00:00Z".into(),
            interval_ms: 60_000,
            cpu: FieldValue::NotMeasured,
            memory: FieldValue::AccessDenied,
            disk: FieldValue::NotMeasured,
            network: FieldValue::Unavailable("offline".into()),
            power: None,
        };
        let now = Instant::now();
        tracker.record_at(&snapshot, now - Duration::from_secs(601));
        tracker.record_at(&snapshot, now);
        let history = tracker.get_history();
        assert_eq!(history.timestamps_utc.len(), 1);
        assert_eq!(history.cpu_percentages, vec![None]);
        assert_eq!(history.network_rx_bytes_sec, vec![None]);
    }
}
