fn main() {
    // The library test executable also links Windows common-controls APIs.
    // Unlike the app binary, it does not receive Tauri's application manifest.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
    }
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
            "get_quick_directories",
            "start_disk_analysis",
            "get_disk_analysis_progress",
            "get_disk_analysis_report",
            "start_duplicate_scan",
            "get_duplicate_scan_progress",
            "get_duplicate_report",
            "cancel_disk_task",
            "get_resource_history",
            "list_installed_programs",
            "launch_uninstaller",
            "list_startup_items",
            "get_antivirus_status",
            "get_system_diagnostics",
            "launch_system_tool",
            "generate_csv_report",
        ]),
    ))
    .expect("failed to build CleanCore permissions");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Tauri already embeds resource ID 1 in the app executable.
        println!("cargo:rustc-link-arg-bin=cleancore=/MANIFEST:NO");
        println!("cargo:rustc-link-arg-cdylib=/MANIFEST:NO");
    }
}
