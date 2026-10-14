//! Управление состоянием настольного приложения CleanCore.

use cleaner_cleanup::tasks::TaskManager;
use cleaner_core::cleanup::CleanupJournal;
use cleaner_core::models::AppInfo;
use cleaner_disk::tasks::DiskManager;
use cleaner_storage::StorageManager;
use cleaner_system::collector::SystemCollector;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tracing::warn;

pub struct AppState {
    pub app_info: Mutex<AppInfo>,
    pub collector: Mutex<SystemCollector>,
    pub storage: Arc<StorageManager>,
    pub task_manager: Arc<TaskManager>,
    pub disk_manager: Arc<DiskManager>,
    pub is_paused: AtomicBool,
    pub interval_ms: AtomicU64,
    pub settings_change: Mutex<()>,
    pub cleanup: Arc<cleaner_execution::CleanupService>,
    pub shutdown: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        let storage = match StorageManager::open(StorageManager::default_db_path()) {
            Ok(mgr) => Arc::new(mgr),
            Err(e) => {
                warn!("Не удалось открыть SQLite на диске ({e}), используется временная память");
                Arc::new(StorageManager::in_memory().expect("in-memory sqlite must succeed"))
            }
        };

        let initial_settings = storage.load_app_settings().unwrap_or_default();

        let cleanup = Arc::new(cleaner_execution::CleanupService::new(
            storage.clone(),
            Arc::new(cleaner_execution::native::DisabledRecycle),
        ));
        Self {
            app_info: Mutex::new(AppInfo {
                name: "CleanCore".to_string(),
                version: "0.0.3".to_string(),
                is_local_mode: true,
                is_admin: cleaner_system::privileges::is_elevated(),
                settings_persistent: storage.is_persistent(),
                theme: initial_settings.theme,
            }),
            collector: Mutex::new(SystemCollector::new()),
            storage,
            task_manager: Arc::new(TaskManager::with_execution(cleanup.clone())),
            disk_manager: Arc::new(DiskManager::new()),
            is_paused: AtomicBool::new(initial_settings.is_monitoring_paused),
            interval_ms: AtomicU64::new(initial_settings.refresh_interval_ms),
            settings_change: Mutex::new(()),
            cleanup,
            shutdown: AtomicBool::new(false),
        }
    }
}

impl AppState {
    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, paused: bool) {
        self.is_paused.store(paused, Ordering::Relaxed);
    }

    pub fn interval_ms(&self) -> u64 {
        self.interval_ms.load(Ordering::Relaxed)
    }

    pub fn set_interval_ms(&self, interval: u64) {
        self.interval_ms.store(interval, Ordering::Relaxed);
    }
}
