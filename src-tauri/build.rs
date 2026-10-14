fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_app_info",
            "ping",
            "get_metric_snapshot",
            "get_process_list",
            "get_app_settings",
            "save_refresh_interval",
            "toggle_pause_metrics",
            "get_cleanup_availability",
            "get_execution_preview",
            "prepare_cleanup_confirmation",
            "start_confirmed_cleanup",
            "get_cleanup_operation",
            "cancel_cleanup",
            "get_cleanup_journal",
            "start_cleanup_scan",
            "get_scan_progress",
            "cancel_cleanup_scan",
            "get_cleanup_preview",
        ]),
    ))
    .expect("failed to build CleanCore permissions");
}
