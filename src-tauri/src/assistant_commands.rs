//! No command accepts a runtime path, arbitrary context text or an action to execute.
use crate::state::AppState;
use cleaner_assistant::{context::*, model::ModelInfo, AssistantError, PreparedContext, Status};
use cleaner_core::errors::ApiError;
use serde::Deserialize;
use tauri::State;
fn api(error: AssistantError) -> ApiError {
    ApiError {
        code: error.code,
        message: error.message,
    }
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSelection {
    pub metrics: bool,
    pub preview_id: Option<String>,
    pub journal_ids: Vec<String>,
}
#[tauri::command]
pub fn assistant_status(state: State<'_, AppState>) -> Result<Status, ApiError> {
    state.assistant.status().map_err(api)
}
#[tauri::command]
pub async fn assistant_choose_model(
    state: State<'_, AppState>,
) -> Result<Option<ModelInfo>, ApiError> {
    let assistant = state.assistant.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = choose_gguf()?;
        path.map(|path| assistant.select_model(&path).map_err(api))
            .transpose()
    })
    .await
    .map_err(|e| ApiError {
        code: "ASSISTANT_WORKER".into(),
        message: e.to_string(),
    })?
}
#[tauri::command]
pub fn assistant_enable(
    state: State<'_, AppState>,
    model_id: String,
    license_source: String,
    accepted: bool,
) -> Result<(), ApiError> {
    state
        .assistant
        .enable(&model_id, &license_source, accepted)
        .map_err(api)
}
#[tauri::command]
pub fn assistant_disable(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.assistant.disable().map_err(api)
}
#[tauri::command]
pub fn assistant_cancel(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.assistant.cancel().map_err(api)
}
#[tauri::command]
pub fn assistant_ask(
    state: State<'_, AppState>,
    context_id: String,
    question: String,
) -> Result<String, ApiError> {
    state.assistant.start(&context_id, &question).map_err(api)
}
#[tauri::command]
pub async fn assistant_prepare_context(
    state: State<'_, AppState>,
    selection: ContextSelection,
) -> Result<PreparedContext, ApiError> {
    if selection.journal_ids.len() > 5
        || selection
            .journal_ids
            .iter()
            .chain(selection.preview_id.iter())
            .any(|s| s.is_empty() || s.len() > 128)
    {
        return Err(ApiError {
            code: "CONTEXT_SELECTION".into(),
            message: "Некорректный выбор данных".into(),
        });
    }
    let assistant = state.assistant.clone();
    let tasks = state.task_manager.clone();
    let cleanup = state.cleanup.clone();
    let metrics = if selection.metrics {
        let mut collector = state.collector.lock().map_err(|_| ApiError {
            code: "COLLECTOR_LOCK_ERROR".into(),
            message: "Метрики недоступны".into(),
        })?;
        let snapshot = collector.collect_snapshot(state.interval_ms());
        Some(MetricSummary {
            cpu_percent: snapshot.cpu.value().map(|m| m.total_usage_percent),
            memory_percent: snapshot.memory.value().map(|m| m.usage_percent),
            disk_free_bytes: snapshot.disk.value().map(|m| m.free_bytes.to_string()),
        })
    } else {
        None
    };
    tauri::async_runtime::spawn_blocking(move || {
        let scan = selection
            .preview_id
            .map(|id| {
                tasks
                    .get_preview_page(&id, 0, 1, "")
                    .map(|page| ScanSummary {
                        category: page.category_id,
                        candidates: page.unfiltered_candidates,
                        bytes: page.total_bytes,
                        complete: page.complete,
                    })
            })
            .transpose()?;
        let mut journal = Vec::new();
        for id in selection.journal_ids {
            let outcome = cleanup.outcome(&id)?.ok_or_else(|| ApiError {
                code: "CONTEXT_NOT_FOUND".into(),
                message: "Выбранная операция не найдена".into(),
            })?;
            use cleaner_core::cleanup::CleanupItemStatus;
            journal.push(JournalSummary {
                status: format!("{:?}", outcome.status),
                item_count: outcome.items.len(),
                unknown_count: outcome
                    .items
                    .iter()
                    .filter(|i| i.status == CleanupItemStatus::Unknown)
                    .count(),
                failed_count: outcome
                    .items
                    .iter()
                    .filter(|i| i.status == CleanupItemStatus::Failed)
                    .count(),
            });
        }
        assistant
            .prepare(AssistantContext {
                metrics,
                scan,
                journal,
            })
            .map_err(api)
    })
    .await
    .map_err(|e| ApiError {
        code: "ASSISTANT_WORKER".into(),
        message: e.to_string(),
    })?
}
#[cfg(windows)]
fn choose_gguf() -> Result<Option<std::path::PathBuf>, ApiError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, OFN_DONTADDTORECENT, OFN_EXPLORER,
        OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OPENFILENAMEW,
    };
    let mut filename = vec![0u16; 32768];
    let filter: Vec<u16> = "GGUF (*.gguf)\0*.gguf\0\0".encode_utf16().collect();
    // SAFETY: all pointers are backed by live buffers for the duration of the modal call.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFile = filename.as_mut_ptr();
    dialog.nMaxFile = filename.len() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.Flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR | OFN_DONTADDTORECENT;
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        let code = unsafe { CommDlgExtendedError() };
        return if code == 0 {
            Ok(None)
        } else {
            Err(ApiError {
                code: "MODEL_DIALOG".into(),
                message: format!("Диалог выбора модели недоступен: {code}"),
            })
        };
    }
    let end = filename
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(filename.len());
    Ok(Some(std::path::PathBuf::from(
        std::ffi::OsString::from_wide(&filename[..end]),
    )))
}
#[cfg(not(windows))]
fn choose_gguf() -> Result<Option<std::path::PathBuf>, ApiError> {
    Err(ApiError {
        code: "UNSUPPORTED_PLATFORM".into(),
        message: "Выбор модели доступен в Windows".into(),
    })
}
