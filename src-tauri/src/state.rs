//! Управление состоянием настольного приложения CleanCore.

use cleaner_core::models::AppInfo;
use std::sync::Mutex;

pub struct AppState {
    pub app_info: Mutex<AppInfo>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            app_info: Mutex::new(AppInfo {
                name: "CleanCore".to_string(),
                version: "0.0.1".to_string(),
                is_local_mode: true,
                is_admin: false,
                theme: "studio-pearl".to_string(),
            }),
        }
    }
}
