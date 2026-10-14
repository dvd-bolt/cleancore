//! cleaner-disk: Модули Этапа 5 (Анализ диска и дубликатов, история ресурсов).

pub mod analyzer;
pub mod duplicates;
pub mod history;
pub mod models;
pub mod tasks;

pub use analyzer::{analyze_directory_tree, AnalyzerOptions};
pub use duplicates::find_duplicates;
pub use history::ResourceHistoryTracker;
pub use models::*;
pub use tasks::{DiskManager, DiskTaskState, DiskTaskStatus};
