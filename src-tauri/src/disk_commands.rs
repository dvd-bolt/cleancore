//! Tauri IPC команды для анализа распределения диска, поиска дубликатов и истории ресурсов.

use cleaner_core::errors::ApiError;
use cleaner_disk::models::{
    DiskAnalysisReport, DuplicateScanResult, QuickDirectory, ResourceHistory,
};
use cleaner_disk::tasks::DiskTaskStatus;
use tauri::State;

use crate::state::AppState;

/// Возвращает список стандартных каталогов Windows для быстрого выбора.
#[tauri::command]
pub fn get_quick_directories(state: State<'_, AppState>) -> Result<Vec<QuickDirectory>, ApiError> {
    Ok(state.disk_manager.get_quick_directories())
}

/// Запускает фоновый анализ структуры и крупнейших файлов каталога.
#[tauri::command]
pub fn start_disk_analysis(state: State<'_, AppState>, path: String) -> Result<String, ApiError> {
    state
        .disk_manager
        .start_disk_analysis(&path)
        .map_err(|err| ApiError {
            code: "DISK_ANALYSIS_ERROR".into(),
            message: err,
        })
}

/// Получение актуального прогресса анализа структуры диска.
#[tauri::command]
pub fn get_disk_analysis_progress(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<DiskTaskStatus, ApiError> {
    state
        .disk_manager
        .get_task_status(&task_id)
        .ok_or_else(|| ApiError {
            code: "TASK_NOT_FOUND".into(),
            message: format!("Задача анализа диска не найдена: {task_id}"),
        })
}

/// Получение готового отчёта анализа структуры диска.
#[tauri::command]
pub fn get_disk_analysis_report(
    state: State<'_, AppState>,
    report_id: String,
) -> Result<DiskAnalysisReport, ApiError> {
    state
        .disk_manager
        .get_disk_report(&report_id)
        .ok_or_else(|| ApiError {
            code: "REPORT_NOT_FOUND".into(),
            message: format!("Отчёт анализа диска не найден: {report_id}"),
        })
}

/// Запуск безопасного трёхфазного поиска дубликатов в указанном каталоге.
#[tauri::command]
pub fn start_duplicate_scan(state: State<'_, AppState>, path: String) -> Result<String, ApiError> {
    state
        .disk_manager
        .start_duplicate_scan(&path)
        .map_err(|err| ApiError {
            code: "DUPLICATE_SCAN_ERROR".into(),
            message: err,
        })
}

/// Получение актуального прогресса поиска дубликатов.
#[tauri::command]
pub fn get_duplicate_scan_progress(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<DiskTaskStatus, ApiError> {
    state
        .disk_manager
        .get_task_status(&task_id)
        .ok_or_else(|| ApiError {
            code: "TASK_NOT_FOUND".into(),
            message: format!("Задача поиска дубликатов не найдена: {task_id}"),
        })
}

/// Получение готового отчёта найденных дубликатов.
#[tauri::command]
pub fn get_duplicate_report(
    state: State<'_, AppState>,
    report_id: String,
) -> Result<DuplicateScanResult, ApiError> {
    state
        .disk_manager
        .get_duplicate_report(&report_id)
        .ok_or_else(|| ApiError {
            code: "REPORT_NOT_FOUND".into(),
            message: format!("Отчёт дубликатов не найден: {report_id}"),
        })
}

/// Запрос безопасной отмены любой выполняющейся задачи дискового анализа.
#[tauri::command]
pub fn cancel_disk_task(state: State<'_, AppState>, task_id: String) -> Result<bool, ApiError> {
    Ok(state.disk_manager.cancel_task(&task_id))
}

/// Получение 10-минутного среза истории потребления ресурсов (CPU, RAM, Network).
#[tauri::command]
pub fn get_resource_history(state: State<'_, AppState>) -> Result<ResourceHistory, ApiError> {
    Ok(state.disk_manager.get_resource_history())
}
