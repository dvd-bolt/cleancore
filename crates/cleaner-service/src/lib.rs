//! CleanCore Service Center Crate (`cleaner-service`)
//!
//! Реализует функциональность Этапа 6 («Центр обслуживания — v0.3»):
//! - Модуль 6.1: Инвентаризация установленных программ и запуск штатных деинсталляторов.
//! - Модуль 6.2: Обзор запускаемых компонентов и элементов автозагрузки Windows.
//! - Модуль 6.3: Статус антивирусной защиты и Windows Defender без повышения привилегий.
//! - Модуль 6.4: Обоснованная системная диагностика, пороговые правила и рекомендации.
//! - Модуль 6.5: Безопасный переход к штатным утилитам Windows System32 по белому списку.
//! - Модуль 6.6: Защищённый экспорт отчётов в CSV (RFC 4180, CWE-1236, UTF-8 BOM).

pub mod defender;
pub mod diagnostics;
pub mod export;
pub mod programs;
pub mod startup;
pub mod tools;
pub mod types;

pub use defender::{decode_provider_state, get_antivirus_status};
pub use diagnostics::{evaluate_diagnostics_rules, get_system_diagnostics};
pub use export::{format_csv_cell, generate_csv_report, sanitize_csv_cell, UTF8_BOM};
pub use programs::{launch_uninstaller, list_installed_programs, parse_uninstaller_command};
pub use startup::{list_startup_items, parse_startup_approved_byte};
pub use tools::{launch_system_tool, list_system_tools};
pub use types::*;
