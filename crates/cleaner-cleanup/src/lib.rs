//! Ядро безопасного анализа очистки CleanCore (Этап 3).
//!
//! Модули:
//! - `rules`: каталог версионируемых правил (`user-temp-v1`).
//! - `path_safety`: верификация корня временных файлов и фильтрация ссылок/junctions.
//! - `scanner`: потоковый безопасный обход файлов с возрастным фильтром (7 дней) и отменой.
//! - `tasks`: управление жизненным циклом задач сканирования и порционной выдачей preview.

pub mod general;
pub mod manual;
pub mod path_safety;
pub mod rules;
pub mod scanner;
pub mod tasks;

pub use path_safety::{is_reparse_point_or_symlink, is_safe_target_file, resolve_user_temp_root};
pub use rules::{CleanupRule, RuleCatalog};
pub use scanner::{scan_directory, ScanResult, ScannerOptions};
pub use tasks::{StoredPreview, TaskManager};
