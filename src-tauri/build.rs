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
            "start_cleanup_scan",
            "get_scan_progress",
            "cancel_cleanup_scan",
            "get_cleanup_preview",
        ]),
    ))
    .expect("failed to build CleanCore permissions");
}
