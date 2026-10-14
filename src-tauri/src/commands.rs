//! Узкие команды Tauri IPC с типизированной валидацией и обработкой ошибок.

use cleaner_core::errors::ApiError;
use cleaner_core::models::{AppInfo, AppSettings, MetricSnapshot, ProcessSnapshot};
use cleaner_system::collector::collect_processes;
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

/// Возвращает актуальный снимок системных показателей (CPU, RAM, диски, сеть).
/// На первом замере загрузка CPU честно возвращается как `FieldValue::NotMeasured`.
#[tauri::command]
pub fn get_metric_snapshot(state: State<'_, AppState>) -> Result<MetricSnapshot, ApiError> {
    let mut collector = state.collector.lock().map_err(|e| ApiError {
        code: "COLLECTOR_LOCK_ERROR".into(),
        message: format!("Ошибка доступа к системному сборщику: {e}"),
    })?;

    let interval = state.interval_ms();
    let snapshot = collector.collect_snapshot(interval);
    Ok(snapshot)
}

/// Возвращает актуальный список процессов без прав администратора.
/// Загрузка CPU каждого процесса нормирована к общей шкале 0..100%.
/// Системные процессы с ограниченным доступом Windows возвращают `FieldValue::AccessDenied`.
/// Завершение процессов запрещено ТЗ.
#[tauri::command]
pub fn get_process_list(state: State<'_, AppState>) -> Result<ProcessSnapshot, ApiError> {
    let mut collector = state.collector.lock().map_err(|e| ApiError {
        code: "COLLECTOR_LOCK_ERROR".into(),
        message: format!("Ошибка доступа к системному сборщику: {e}"),
    })?;

    let snapshot = collect_processes(collector.system_mut());
    Ok(snapshot)
}

/// Возвращает текущие настройки приложения из локальной базы SQLite.
#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, ApiError> {
    state.storage.load_app_settings().map_err(Into::into)
}

/// Сохраняет интервал опроса метрик (в миллисекундах) в SQLite и применяет его.
#[tauri::command]
pub fn save_refresh_interval(
    state: State<'_, AppState>,
    interval_ms: u64,
) -> Result<AppSettings, ApiError> {
    // Валидация входных данных: допустимый диапазон от 500 мс до 60 сек
    if !(500..=60_000).contains(&interval_ms) {
        return Err(ApiError {
            code: "INVALID_INTERVAL".into(),
            message: format!(
                "Недопустимый интервал обновления: {interval_ms} мс (разрешено 500..60000 мс)"
            ),
        });
    }

    state.set_interval_ms(interval_ms);

    let mut settings = state.storage.load_app_settings().unwrap_or_default();
    settings.refresh_interval_ms = interval_ms;
    state.storage.save_app_settings(&settings)?;

    Ok(settings)
}

/// Переключает режим паузы мониторинга.
#[tauri::command]
pub fn toggle_pause_metrics(state: State<'_, AppState>, paused: bool) -> Result<bool, ApiError> {
    state.set_paused(paused);

    let mut settings = state.storage.load_app_settings().unwrap_or_default();
    settings.is_monitoring_paused = paused;
    let _ = state.storage.save_app_settings(&settings);

    Ok(paused)
}
