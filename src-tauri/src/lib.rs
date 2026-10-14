pub mod commands;
pub mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::ping,
            commands::get_initial_metrics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CleanCore tauri application");
}
