//! Узкие команды Tauri IPC с типизированной валидацией и обработкой ошибок.

use cleaner_core::errors::ApiError;
use cleaner_core::models::{AppInfo, FieldValue, MetricSnapshot};
use tauri::State;

use crate::state::AppState;

/// Возвращает сведения о приложении, режиме работы и правах.
#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> Result<AppInfo, ApiError> {
    let info = state.app_info.lock().map_err(|e| ApiError {
        code: "STATE_LOCK_ERROR".into(),
        message: format!("Ошибка доступа к состоянию: {e}"),
    })?;
    Ok(info.clone())
}

/// Проверка доступности IPC моста между React и Rust.
#[tauri::command]
pub fn ping() -> Result<String, ApiError> {
    Ok("pong".into())
}

/// Возвращает базовый снимок метрик для инициализации интерфейса.
/// На этапе 1 возвращает статус NotMeasured («Ожидание данных»), не подменяя его нулями.
#[tauri::command]
pub fn get_initial_metrics() -> Result<MetricSnapshot, ApiError> {
    Ok(MetricSnapshot {
        timestamp_utc: "2026-10-14T00:00:00Z".to_string(),
        interval_ms: 2000,
        cpu: FieldValue::NotMeasured,
        memory: FieldValue::NotMeasured,
        disk: FieldValue::NotMeasured,
        network: FieldValue::NotMeasured,
    })
}
