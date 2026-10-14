//! Модуль 5.5: Хранение 10-минутной истории метрик (CPU, RAM, Сеть) в кольцевом буфере памяти.

use cleaner_core::models::{FieldValue, MetricSnapshot};
use std::collections::VecDeque;
use std::sync::Mutex;

use crate::models::ResourceHistory;

const MAX_HISTORY_SAMPLES: usize = 600; // 600 замеров (10 минут при интервале 1с или 20 минут при 2с)

#[derive(Debug, Clone)]
struct HistoricalSample {
    timestamp_utc: String,
    cpu_percent: f32,
    memory_percent: f32,
    network_rx_bytes_sec: u64,
    network_tx_bytes_sec: u64,
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
        let cpu_val = match &snapshot.cpu {
            FieldValue::Available(m) => m.total_usage_percent,
            _ => 0.0,
        };

        let mem_val = match &snapshot.memory {
            FieldValue::Available(m) => m.usage_percent as f32,
            _ => 0.0,
        };

        let (rx, tx) = match &snapshot.network {
            FieldValue::Available(m) => (m.rx_bytes_per_sec, m.tx_bytes_per_sec),
            _ => (0, 0),
        };

        let sample = HistoricalSample {
            timestamp_utc: snapshot.timestamp_utc.clone(),
            cpu_percent: cpu_val,
            memory_percent: mem_val,
            network_rx_bytes_sec: rx,
            network_tx_bytes_sec: tx,
        };

        let mut samples = self.samples.lock().unwrap();
        if samples.len() >= MAX_HISTORY_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(sample);
    }

    /// Возвращает агрегированную историю ресурсов для отображения графиков нагрузки.
    pub fn get_history(&self) -> ResourceHistory {
        let samples = self.samples.lock().unwrap();

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

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::models::{CpuMetrics, MemoryMetrics, NetworkMetrics};

    #[test]
    fn test_history_tracker_records_and_bounds() {
        let tracker = ResourceHistoryTracker::new();

        for i in 0..650 {
            let snapshot = MetricSnapshot {
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
            };
            tracker.record_snapshot(&snapshot);
        }

        let history = tracker.get_history();
        assert_eq!(history.timestamps_utc.len(), MAX_HISTORY_SAMPLES);
        assert_eq!(history.cpu_percentages.len(), MAX_HISTORY_SAMPLES);
        assert_eq!(history.cpu_percentages[0], 25.0);
        assert_eq!(history.memory_percentages[0], 50.0);
        assert_eq!(history.network_rx_bytes_sec[0], 1024);
        assert_eq!(history.network_tx_bytes_sec[0], 2048);
    }
}
