//! Узкие команды Tauri IPC с типизированной валидацией и обработкой ошибок.

use cleaner_core::errors::ApiError;
use cleaner_core::models::{
    AppInfo, AppSettings, CleanupPreviewPage, MetricSnapshot, ProcessSnapshot, TaskStatus,
};
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
pub async fn get_metric_snapshot(state: State<'_, AppState>) -> Result<MetricSnapshot, ApiError> {
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
pub async fn get_process_list(state: State<'_, AppState>) -> Result<ProcessSnapshot, ApiError> {
    let mut collector = state.collector.lock().map_err(|e| ApiError {
        code: "COLLECTOR_LOCK_ERROR".into(),
        message: format!("Ошибка доступа к системному сборщику: {e}"),
    })?;

    let snapshot = collector.collect_processes();
    Ok(snapshot)
}

use cleaner_storage::{SettingsRecoveryNotice, StorageRecoveryNotice};
use serde::{Deserialize, Serialize};

/// Сведения об устойчивости хранилища и настроек CleanCore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageResilienceStatus {
    pub db_path: String,
    pub settings_path: String,
    pub db_recovered: bool,
    pub db_recovery_notice: Option<StorageRecoveryNotice>,
    pub settings_recovered: bool,
    pub settings_recovery_notice: Option<SettingsRecoveryNotice>,
    pub schema_version: i32,
    pub database_persistent: bool,
    pub settings_persistent: bool,
}

/// Возвращает диагностические сведения об устойчивости хранилища и настроек.
#[tauri::command]
pub fn get_storage_resilience_status(
    state: State<'_, AppState>,
) -> Result<StorageResilienceStatus, ApiError> {
    let schema_version = state
        .storage
        .current_schema_version()
        .map_err(ApiError::from)?;
    Ok(StorageResilienceStatus {
        db_path: state.storage.db_path().to_string_lossy().to_string(),
        settings_path: state.settings.path().to_string_lossy().to_string(),
        db_recovered: state.storage.was_recovered(),
        db_recovery_notice: state.storage.recovery_notice().cloned(),
        settings_recovered: state.settings.was_recovered(),
        settings_recovery_notice: state.settings.recovery_notice().cloned(),
        schema_version,
        database_persistent: state.storage.is_persistent(),
        settings_persistent: state.settings.is_persistent(),
    })
}

/// Возвращает текущие настройки приложения из settings.json.
#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, ApiError> {
    Ok(state.settings.get_settings())
}

/// Сохраняет интервал опроса метрик (в миллисекундах) в settings.json и применяет его.
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

    let _guard = state.settings_change.lock().map_err(|_| ApiError {
        code: "STATE_LOCK_ERROR".into(),
        message: "Настройки недоступны".into(),
    })?;
    let updated = state
        .settings
        .update_settings(|s| {
            s.refresh_interval_ms = interval_ms;
        })
        .map_err(ApiError::from)?;
    let _ = state.storage.save_app_settings(&updated);
    state.set_interval_ms(interval_ms);

    Ok(updated)
}

/// Переключает режим паузы мониторинга.
#[tauri::command]
pub fn toggle_pause_metrics(state: State<'_, AppState>, paused: bool) -> Result<bool, ApiError> {
    let _guard = state.settings_change.lock().map_err(|_| ApiError {
        code: "STATE_LOCK_ERROR".into(),
        message: "Настройки недоступны".into(),
    })?;
    let updated = state
        .settings
        .update_settings(|s| {
            s.is_monitoring_paused = paused;
        })
        .map_err(ApiError::from)?;
    let _ = state.storage.save_app_settings(&updated);
    state.set_paused(paused);

    Ok(paused)
}

/// Запуск безопасного фонового анализа выбранной категории очистки.
/// На этапе 3 поддерживается категория "user-temp-v1".
/// Сканируемые файлы НЕ удаляются и НЕ изменяются.
#[tauri::command]
pub fn start_cleanup_scan(
    state: State<'_, AppState>,
    category_id: String,
    project_path: Option<String>,
) -> Result<String, ApiError> {
    let preferences = state
        .cleanup_preferences
        .read()
        .map_err(|_| ApiError {
            code: "PREFERENCES_UNAVAILABLE".into(),
            message: "Настройки защиты недоступны".into(),
        })?
        .clone()
        .ok_or_else(|| ApiError {
            code: "PREFERENCES_UNAVAILABLE".into(),
            message: "Настройки защиты повреждены. Сохраните проверенные исключения в настройках"
                .into(),
        })?;
    if category_id.starts_with("project-") {
        let project = project_path
            .filter(|p| !p.is_empty() && p.len() <= 4096)
            .ok_or_else(|| ApiError {
                code: "INVALID_PROJECT".into(),
                message: "Укажите полный путь к выбранному проекту".into(),
            })?;
        state.task_manager.start_project_scan(
            &category_id,
            std::path::Path::new(&project),
            preferences.excluded_paths,
        )
    } else {
        if project_path.is_some() {
            return Err(ApiError {
                code: "INVALID_PROJECT".into(),
                message: "Для этой категории нельзя менять корень".into(),
            });
        }
        state
            .task_manager
            .start_scan_with_exclusions(&category_id, preferences.excluded_paths)
    }
}

#[tauri::command]
pub fn resume_cleanup_scan(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<String, ApiError> {
    state.task_manager.resume_scan(&task_id)
}

#[tauri::command]
pub fn get_cleanup_rules(state: State<'_, AppState>) -> Vec<cleaner_cleanup::rules::CleanupRule> {
    state.task_manager.rules()
}
#[tauri::command]
pub fn get_cleanup_profile_context() -> Result<String, ApiError> {
    cleaner_cleanup::path_safety::current_profile_context().map_err(Into::into)
}

#[tauri::command]
pub fn get_cleanup_preferences(
    state: State<'_, AppState>,
) -> Result<cleaner_core::cleanup_preferences::CleanupPreferences, ApiError> {
    state.storage.load_cleanup_preferences().map_err(Into::into)
}
#[tauri::command]
pub async fn save_cleanup_preferences(
    state: State<'_, AppState>,
    preferences: cleaner_core::cleanup_preferences::CleanupPreferences,
) -> Result<(), ApiError> {
    preferences.validate().map_err(|e| ApiError {
        code: "INVALID_EXCLUSION".into(),
        message: e.into(),
    })?;
    let storage = state.storage.clone();
    let shared = state.cleanup_preferences.clone();
    let service = state.cleanup.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = shared.write().map_err(|_| ApiError {
            code: "PREFERENCES_UNAVAILABLE".into(),
            message: "Настройки недоступны".into(),
        })?;
        service.revoke_all_previews()?;
        storage
            .save_cleanup_preferences(&preferences)
            .map_err(ApiError::from)?;
        *guard = Some(preferences);
        Ok(())
    })
    .await
    .map_err(|_| ApiError {
        code: "WORKER_FAILED".into(),
        message: "Не удалось сохранить исключения".into(),
    })?
}
#[tauri::command]
pub fn clear_duplicate_hash_cache(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.storage.clear_duplicate_cache().map_err(Into::into)
}

/// Получение актуального прогресса и статуса задачи сканирования.
#[tauri::command]
pub fn get_scan_progress(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<TaskStatus, ApiError> {
    state
        .task_manager
        .get_task_status(&task_id)?
        .ok_or_else(|| ApiError {
            code: "TASK_NOT_FOUND".into(),
            message: format!("Задача сканирования не найдена: {task_id}"),
        })
}

/// Запрос безопасной отмены выполняющегося сканирования.
#[tauri::command]
pub fn cancel_cleanup_scan(state: State<'_, AppState>, task_id: String) -> Result<bool, ApiError> {
    state.task_manager.cancel_scan(&task_id)
}

/// Порционная выдача результатов предварительного просмотра (Preview) без блокировки UI.
#[tauri::command]
pub fn get_cleanup_preview(
    state: State<'_, AppState>,
    preview_id: String,
    offset: usize,
    limit: usize,
    query: Option<String>,
) -> Result<CleanupPreviewPage, ApiError> {
    state
        .task_manager
        .get_preview_page(&preview_id, offset, limit, query.as_deref().unwrap_or(""))
}
