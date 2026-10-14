pub mod cleanup_commands;
pub mod commands;
pub mod disk_commands;
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
            commands::get_app_info,
            commands::ping,
            commands::get_metric_snapshot,
            commands::get_process_list,
            commands::get_app_settings,
            commands::save_refresh_interval,
            commands::toggle_pause_metrics,
            cleanup_commands::get_cleanup_availability,
            cleanup_commands::get_execution_preview,
            cleanup_commands::prepare_cleanup_confirmation,
            cleanup_commands::start_confirmed_cleanup,
            cleanup_commands::get_cleanup_operation,
            cleanup_commands::cancel_cleanup,
            cleanup_commands::get_cleanup_journal,
            commands::start_cleanup_scan,
            commands::get_scan_progress,
            commands::cancel_cleanup_scan,
            commands::get_cleanup_preview,
            disk_commands::get_quick_directories,
            disk_commands::start_disk_analysis,
            disk_commands::get_disk_analysis_progress,
            disk_commands::get_disk_analysis_report,
            disk_commands::start_duplicate_scan,
            disk_commands::get_duplicate_scan_progress,
            disk_commands::get_duplicate_report,
            disk_commands::cancel_disk_task,
            disk_commands::get_resource_history,
        ])
        .setup(|app| {
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
            }
        });
}
