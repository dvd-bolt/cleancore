//! CleanCore Core Library
//! Общие модели, типизированные ошибки, правила безопасности путей и события.

pub mod cleanup;
pub mod cleanup_preferences;
pub mod duplicate_cache;
pub mod errors;
pub mod events;
pub mod models;
pub mod safety;

pub mod prelude {
    pub use crate::errors::{ApiError, CoreError};
    pub use crate::events::{AppEvent, NotificationEvent, NotificationLevel};
    pub use crate::models::{
        AppInfo, CpuMetrics, DiskMetrics, FieldValue, MemoryMetrics, MetricSnapshot,
        NetworkMetrics, ProcessItem, ProcessSnapshot, TaskState, TaskStatus,
    };
    pub use crate::safety::{PathGuard, RiskLevel};
}

#[cfg(test)]
mod tests {
    use super::prelude::*;

    #[test]
    fn test_dto_serialization_roundtrip() {
        let snapshot = MetricSnapshot {
            disks: Vec::new(),
            network_interfaces: Vec::new(),
            timestamp_utc: "2026-10-14T03:00:00Z".to_string(),
            interval_ms: 2000,
            cpu: FieldValue::Available(CpuMetrics {
                total_usage_percent: 18.5,
                core_count: 8,
                frequency_mhz: Some(3600),
            }),
            memory: FieldValue::Available(MemoryMetrics {
                used_bytes: 8 * 1024 * 1024 * 1024,
                total_bytes: 16 * 1024 * 1024 * 1024,
                usage_percent: 50,
            }),
            disk: FieldValue::Available(DiskMetrics {
                mount_point: "C:".to_string(),
                free_bytes: 120 * 1024 * 1024 * 1024,
                total_bytes: 512 * 1024 * 1024 * 1024,
                usage_percent: 76,
            }),
            network: FieldValue::Available(NetworkMetrics {
                rx_bytes_per_sec: 1024 * 50,
                tx_bytes_per_sec: 1024 * 20,
                interface_name: Some("Ethernet".to_string()),
            }),
            power: None,
        };

        let json = serde_json::to_string(&snapshot).expect("Serialize snapshot");
        assert!(json.contains("total_usage_percent"));
        assert!(json.contains("Available"));

        let deserialized: MetricSnapshot =
            serde_json::from_str(&json).expect("Deserialize snapshot");
        assert_eq!(snapshot, deserialized);
    }

    #[test]
    fn test_field_value_unmeasured_state() {
        let cpu_value: FieldValue<CpuMetrics> = FieldValue::NotMeasured;
        assert!(!cpu_value.is_available());
        assert_eq!(cpu_value.value(), None);

        let json = serde_json::to_string(&cpu_value).expect("Serialize NotMeasured");
        assert_eq!(json, "{\"status\":\"NotMeasured\"}");
    }

    #[test]
    fn test_api_error_conversion() {
        let core_err = CoreError::AccessDenied("C:\\Windows\\System32".into());
        let api_err: ApiError = core_err.into();
        assert_eq!(api_err.code, "ACCESS_DENIED");
    }
}
