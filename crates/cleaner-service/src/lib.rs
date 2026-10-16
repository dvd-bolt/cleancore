//! CleanCore Service Center Crate (`cleaner-service`)
//!
//! Реализует функциональность Этапа 6 («Центр обслуживания — v0.3»):
//! - Модуль 6.1: Инвентаризация установленных программ и запуск штатных деинсталляторов.
//! - Модуль 6.2: Обзор запускаемых компонентов и элементов автозагрузки Windows.
//! - Модуль 6.3: Статус антивирусной защиты и Windows Defender без повышения привилегий.
//! - Модуль 6.4: Обоснованная системная диагностика, пороговые правила и рекомендации.
//! - Модуль 6.5: Безопасный переход к штатным утилитам Windows System32 по белому списку.
//! - Модуль 6.6: Защищённый экспорт отчётов в CSV (RFC 4180, CWE-1236, UTF-8 BOM).

pub mod advisor;
pub mod defender;
pub mod dev_caches;
pub mod diagnostics;
pub mod export;
pub mod power_tuning;
pub mod programs;
pub mod recycle_bin;
pub mod residue;
pub mod startup;
pub mod startup_control;
pub mod tools;
pub mod types;

pub use advisor::{
    evaluate_smart_insights, format_insight_bytes, is_heavy_dev_workload_active, AdvisorInput,
    SmartInsight, HEAVY_DEV_PROCESSES,
};

pub use defender::{decode_provider_state, get_antivirus_status};
pub use dev_caches::scan_dev_caches;
pub use diagnostics::{evaluate_diagnostics_rules, get_system_diagnostics};
pub use export::{format_csv_cell, generate_csv_report, sanitize_csv_cell, UTF8_BOM};
pub use power_tuning::{
    apply_throttle_values, determine_profile, read_active_scheme, read_throttle_values,
    set_active_scheme, PowerProfileKind, PowerProfileStatus, PowerSchemeInfo, PowerThrottleValues,
};
pub use programs::{launch_uninstaller, list_installed_programs, parse_uninstaller_command};
pub use recycle_bin::{empty_recycle_bin, query_recycle_bin};
pub use startup::{list_startup_items, parse_startup_approved_byte};
pub use tools::{launch_system_tool, list_system_tools};
pub use types::*;
