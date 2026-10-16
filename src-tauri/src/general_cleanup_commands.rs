use crate::state::AppState;
use cleaner_cleanup::general::{GeneralConfirmation, GeneralIndex, GeneralJob, GeneralPage};
use cleaner_core::errors::ApiError;
use tauri::State;

#[tauri::command]
pub fn create_general_cleanup(state: State<'_, AppState>) -> Result<String, ApiError> {
    state.task_manager.general.create()
}
#[tauri::command]
pub fn collect_general_cleanup(
    state: State<'_, AppState>,
    session_id: String,
    task_id: String,
) -> Result<(), ApiError> {
    state
        .task_manager
        .collect_general_batch(&session_id, &task_id)
}
#[tauri::command]
pub fn finish_general_cleanup(
    state: State<'_, AppState>,
    session_id: String,
    cancelled: bool,
    failed_categories: Vec<String>,
) -> Result<GeneralIndex, ApiError> {
    state
        .task_manager
        .general
        .finish(&session_id, cancelled, &failed_categories)
}
#[tauri::command]
pub fn get_general_cleanup_page(
    state: State<'_, AppState>,
    session_id: String,
    offset: usize,
    limit: usize,
    query: String,
    selected_ids: Option<Vec<String>>,
) -> Result<GeneralPage, ApiError> {
    state
        .task_manager
        .general
        .page(&session_id, offset, limit, &query, selected_ids.as_deref())
}
#[tauri::command]
pub fn prepare_general_cleanup(
    state: State<'_, AppState>,
    session_id: String,
    selected_ids: Vec<String>,
    accept_partial: bool,
) -> Result<GeneralConfirmation, ApiError> {
    state
        .task_manager
        .general
        .prepare(&session_id, selected_ids, accept_partial)
}
#[tauri::command]
pub fn execute_general_cleanup(
    state: State<'_, AppState>,
    confirmation_id: String,
    acknowledged: bool,
) -> Result<GeneralJob, ApiError> {
    state
        .task_manager
        .general
        .begin(&confirmation_id, acknowledged)
}
#[tauri::command]
pub fn get_general_cleanup_job(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<GeneralJob, ApiError> {
    state.task_manager.general.job(&job_id)
}
#[tauri::command]
pub fn cancel_general_cleanup_job(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<bool, ApiError> {
    state.task_manager.general.cancel(&job_id)
}
