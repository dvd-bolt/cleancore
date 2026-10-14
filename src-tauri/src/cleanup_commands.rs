//! Stage-4 IPC accepts identifiers only. No paths or preview ingestion.
use crate::state::AppState;
use cleaner_core::{cleanup::*, errors::ApiError};
use tauri::State;
fn validate_id(id: &str) -> Result<(), ApiError> {
    if id.is_empty() || id.len() > 128 {
        return Err(ApiError {
            code: "INVALID_ID".into(),
            message: "Некорректный идентификатор".into(),
        });
    }
    Ok(())
}
#[tauri::command]
pub fn get_cleanup_availability(state: State<'_, AppState>) -> CleanupAvailability {
    state.cleanup.availability()
}
#[tauri::command]
pub fn get_execution_preview(
    state: State<'_, AppState>,
    preview_id: Option<String>,
) -> Result<Option<CleanupPreview>, ApiError> {
    if let Some(id) = preview_id {
        validate_id(&id)?;
        state.cleanup.preview(&id)
    } else {
        Ok(None)
    }
}
#[tauri::command]
pub fn prepare_cleanup_confirmation(
    state: State<'_, AppState>,
    preview_id: String,
    selected_ids: Vec<String>,
) -> Result<CleanupConfirmation, ApiError> {
    validate_id(&preview_id)?;
    if selected_ids.len() > cleaner_execution::MAX_CANDIDATES {
        return Err(ApiError {
            code: "LIMIT".into(),
            message: "Слишком большой выбор".into(),
        });
    }
    for id in &selected_ids {
        validate_id(id)?;
    }
    state
        .cleanup
        .prepare_confirmation(&preview_id, selected_ids)
}
#[tauri::command]
pub async fn start_confirmed_cleanup(
    state: State<'_, AppState>,
    confirmation_id: String,
    acknowledged: bool,
) -> Result<CleanupOutcome, ApiError> {
    validate_id(&confirmation_id)?;
    let service = state.cleanup.clone();
    let initial =
        tauri::async_runtime::spawn_blocking(move || service.begin(&confirmation_id, acknowledged))
            .await
            .map_err(|_| ApiError {
                code: "WORKER_FAILED".into(),
                message: "Не удалось подготовить операцию; проверьте журнал.".into(),
            })??;
    let service = state.cleanup.clone();
    let operation_id = initial.operation_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = service.run(&operation_id) {
            tracing::error!(code = %e.code, "Cleanup worker stopped");
        }
    });
    Ok(initial)
}
#[tauri::command]
pub fn get_cleanup_operation(
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<Option<CleanupOutcome>, ApiError> {
    validate_id(&operation_id)?;
    state.cleanup.outcome(&operation_id)
}
#[tauri::command]
pub fn cancel_cleanup(state: State<'_, AppState>, operation_id: String) -> Result<bool, ApiError> {
    validate_id(&operation_id)?;
    state.cleanup.cancel(&operation_id)
}
#[tauri::command]
pub async fn get_cleanup_journal(
    state: State<'_, AppState>,
    before: Option<i64>,
    limit: u32,
) -> Result<CleanupAuditPage, ApiError> {
    if !(1..=25).contains(&limit) || before.is_some_and(|v| v <= 0) {
        return Err(ApiError {
            code: "INVALID_PAGE".into(),
            message: "Некорректная порция журнала".into(),
        });
    }
    let storage = state.storage.clone();
    tauri::async_runtime::spawn_blocking(move || {
        storage.audit_page(before, limit).map_err(Into::into)
    })
    .await
    .map_err(|_| ApiError {
        code: "WORKER_FAILED".into(),
        message: "Не удалось прочитать журнал".into(),
    })?
}
