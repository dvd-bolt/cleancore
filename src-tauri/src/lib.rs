pub mod assistant_commands;
pub mod cleanup_commands;
pub mod commands;
pub mod disk_commands;
pub mod folder_dialog;
pub mod general_cleanup_commands;
pub mod residue_commands;
pub mod service_commands;
pub mod state;

#[cfg(test)]
mod ipc_tests;

use state::AppState;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};
use tracing::{error, info};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            general_cleanup_commands::create_general_cleanup,
            general_cleanup_commands::collect_general_cleanup,
            general_cleanup_commands::finish_general_cleanup,
            general_cleanup_commands::get_general_cleanup_page,
            general_cleanup_commands::prepare_general_cleanup,
            general_cleanup_commands::execute_general_cleanup,
            general_cleanup_commands::get_general_cleanup_job,
            general_cleanup_commands::cancel_general_cleanup_job,
            commands::get_app_info,
            commands::ping,
            assistant_commands::assistant_status,
            assistant_commands::assistant_choose_model,
            assistant_commands::assistant_enable,
            assistant_commands::assistant_disable,
            assistant_commands::assistant_cancel,
            assistant_commands::assistant_ask,
            assistant_commands::assistant_prepare_context,
            commands::get_metric_snapshot,
            commands::get_process_list,
            commands::get_app_settings,
            commands::save_refresh_interval,
            commands::toggle_pause_metrics,
            commands::get_storage_resilience_status,
            cleanup_commands::get_cleanup_availability,
            cleanup_commands::get_execution_preview,
            cleanup_commands::prepare_cleanup_confirmation,
            cleanup_commands::start_confirmed_cleanup,
            cleanup_commands::get_cleanup_operation,
            cleanup_commands::cancel_cleanup,
            cleanup_commands::get_cleanup_journal,
            commands::start_cleanup_scan,
            commands::resume_cleanup_scan,
            commands::get_cleanup_rules,
            commands::get_cleanup_profile_context,
            commands::get_cleanup_preferences,
            commands::save_cleanup_preferences,
            commands::clear_duplicate_hash_cache,
            commands::get_scan_progress,
            commands::cancel_cleanup_scan,
            commands::get_cleanup_preview,
            disk_commands::get_quick_directories,
            folder_dialog::choose_analysis_folder,
            disk_commands::start_disk_analysis,
            disk_commands::get_disk_analysis_progress,
            disk_commands::get_disk_analysis_report,
            disk_commands::get_disk_treemap,
            disk_commands::reveal_disk_entry,
            disk_commands::prepare_disk_file_cleanup,
            disk_commands::prepare_duplicate_cleanup,
            disk_commands::invalidate_disk_report,
            disk_commands::start_duplicate_scan,
            disk_commands::get_duplicate_scan_progress,
            disk_commands::get_duplicate_report,
            disk_commands::cancel_disk_task,
            disk_commands::get_resource_history,
            disk_commands::get_space_hogs,
            disk_commands::reveal_path_in_explorer,
            disk_commands::get_stale_files,
            service_commands::list_installed_programs,
            residue_commands::launch_uninstaller_with_residue_context,
            residue_commands::inspect_app_residue,
            service_commands::list_startup_items,
            service_commands::set_startup_enabled,
            service_commands::get_antivirus_status,
            service_commands::get_system_diagnostics,
            service_commands::launch_system_tool,
            service_commands::generate_csv_report,
            service_commands::save_csv_report,
            service_commands::export_cleanup_journal,
            service_commands::query_recycle_bin,
            service_commands::empty_recycle_bin,
            service_commands::get_dev_cache_summary,
            service_commands::get_power_profile_state,
            service_commands::apply_power_profile,
            service_commands::rollback_power_profile,
            service_commands::get_smart_insights,
        ])
        .setup(|app| {
            let bundled = app
                .path()
                .resource_dir()?
                .join("runtime")
                .join(cleaner_assistant::runtime::RUNTIME_FOLDER);
            if bundled
                .join(cleaner_assistant::runtime::RUNTIME_EXE)
                .is_file()
            {
                app.state::<AppState>().assistant.set_runtime(bundled);
            }

            let app_handle = app.handle().clone();
            info!("Инициализация фонового цикла мониторинга CleanCore");

            // Фоновый поток циклического сбора метрик с защитой от наложения и поддержкой паузы
            thread::spawn(move || {
                let state = app_handle.state::<AppState>();
                loop {
                    let interval_ms = state.interval_ms().max(500);
                    thread::sleep(Duration::from_millis(interval_ms));

                    if state.shutdown.load(Ordering::Acquire) {
                        break;
                    }

                    // Если мониторинг на паузе — пропускаем такт
                    if state.is_paused() {
                        continue;
                    }

                    // Защита от перекрывающихся сборов:
                    // Если предыдущий сбор или ручной запрос ещё держит мьютекс,
                    // аккуратно пропускаем такт без создания задержек интерфейса
                    if let Ok(mut collector) = state.collector.try_lock() {
                        let snapshot = collector.collect_snapshot(interval_ms);
                        drop(collector);

                        state.disk_manager.history.record_snapshot(&snapshot);

                        if let Err(e) = app_handle.emit("metrics-update", &snapshot) {
                            error!("Ошибка отправки события metrics-update: {e}");
                        }
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building CleanCore tauri application")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                let state = app.state::<AppState>();
                state.shutdown.store(true, Ordering::Release);
                state.task_manager.cancel_all();
                state.disk_manager.cancel_all();
                let _ = state.assistant.disable();
            }
        });
}
