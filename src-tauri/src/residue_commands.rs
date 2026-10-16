//! Thin IPC and Windows Known Folder adapter for read-only residue estimates.
use crate::state::AppState;
use cleaner_core::errors::ApiError;
use cleaner_service::{
    residue::{inspect, ResidueReport},
    LaunchResult,
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;
fn fail(code: &str, message: &str) -> ApiError {
    ApiError {
        code: code.into(),
        message: message.into(),
    }
}
#[derive(Serialize)]
pub struct RemovalLaunch {
    #[serde(flatten)]
    result: LaunchResult,
    residue_context_id: String,
}
#[tauri::command]
pub async fn launch_uninstaller_with_residue_context(
    state: State<'_, AppState>,
    program_id: String,
    acknowledged: bool,
) -> Result<RemovalLaunch, ApiError> {
    if !acknowledged {
        return Err(fail(
            "CONFIRMATION_REQUIRED",
            "Подтвердите запуск штатного удаления",
        ));
    }
    if program_id.len() > 512 {
        return Err(fail("INVALID_APP", "Некорректный ID программы"));
    }
    let manager = state.residue_manager.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let app = cleaner_service::list_installed_programs(false)
            .map_err(ApiError::from)?
            .into_iter()
            .find(|p| p.id == program_id)
            .ok_or_else(|| fail("APP_NOT_FOUND", "Программа больше не зарегистрирована"))?;
        let residue_context_id = manager.remember(app)?;
        let result = cleaner_service::launch_uninstaller(&program_id).map_err(ApiError::from)?;
        Ok(RemovalLaunch {
            result,
            residue_context_id,
        })
    })
    .await
    .map_err(|_| fail("RESIDUE_WORKER", "Не удалось запустить деинсталлятор"))?
}
#[tauri::command]
pub async fn inspect_app_residue(
    state: State<'_, AppState>,
    context_id: String,
) -> Result<ResidueReport, ApiError> {
    if context_id.len() > 128 {
        return Err(fail("INVALID_ID", "Некорректный ID"));
    }
    let app = state.residue_manager.app(&context_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let installed = cleaner_service::list_installed_programs(false).map_err(ApiError::from)?;
        let registered = installed.iter().any(|p| p.id == app.id);
        Ok(inspect(&app, registered, &user_data_roots()?))
    })
    .await
    .map_err(|_| fail("RESIDUE_WORKER", "Не удалось проверить данные приложения"))?
}
fn user_data_roots() -> Result<Vec<PathBuf>, ApiError> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{FOLDERID_LocalAppData, FOLDERID_RoamingAppData};
        [&FOLDERID_LocalAppData, &FOLDERID_RoamingAppData]
            .into_iter()
            .map(|id| {
                let root =
                    cleaner_cleanup::path_safety::known_folder(id).map_err(ApiError::from)?;
                cleaner_cleanup::path_safety::ensure_local_fixed(&root).map_err(ApiError::from)?;
                Ok(root)
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        Err(fail("UNSUPPORTED_PLATFORM", "Только Windows"))
    }
}
