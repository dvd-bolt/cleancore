//! IPC tests use Tauri MockRuntime and the REAL generated capabilities.
//! No native window, real temp scan or user database is created.
use super::*;
use cleaner_cleanup::TaskManager;
use cleaner_core::models::AppInfo;
use cleaner_execution::{native::DisabledRecycle, CleanupService};
use cleaner_storage::StorageManager;
use cleaner_system::collector::SystemCollector;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc, Mutex,
};
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{mock_builder, MockRuntime, INVOKE_KEY},
    webview::InvokeRequest,
    WebviewWindow, WebviewWindowBuilder,
};
fn app() -> tauri::App<MockRuntime> {
    let storage = Arc::new(StorageManager::in_memory().unwrap());
    let cleanup = Arc::new(CleanupService::new(
        storage.clone(),
        Arc::new(DisabledRecycle),
    ));
    let state = AppState {
        app_info: Mutex::new(AppInfo {
            name: "CleanCore test".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            is_local_mode: true,
            is_admin: None,
            settings_persistent: false,
            theme: "studio-pearl".into(),
        }),
        collector: Mutex::new(SystemCollector::new()),
        task_manager: Arc::new(TaskManager::with_execution(cleanup.clone())),
        disk_manager: Arc::new(cleaner_disk::tasks::DiskManager::new()),
        storage,
        settings: Arc::new(cleaner_storage::SettingsManager::in_memory(
            cleaner_core::models::AppSettings::default(),
        )),
        cleanup,
        is_paused: AtomicBool::new(false),
        interval_ms: AtomicU64::new(2000),
        settings_change: Mutex::new(()),
        shutdown: AtomicBool::new(false),
    };
    mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::get_app_info,
            commands::get_app_settings,
            commands::save_refresh_interval,
            commands::get_storage_resilience_status,
            commands::start_cleanup_scan,
            commands::get_scan_progress,
            commands::get_cleanup_preview,
            cleanup_commands::get_cleanup_availability,
            cleanup_commands::prepare_cleanup_confirmation,
            cleanup_commands::start_confirmed_cleanup,
            cleanup_commands::get_cleanup_journal
        ])
        .build(tauri::generate_context!())
        .unwrap()
}
fn call(
    window: &WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
    url: &str,
) -> Result<Value, Value> {
    tauri::test::get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize::<Value>().unwrap())
}
#[test]
fn main_window_can_read_settings_and_save_valid_interval() {
    let app = app();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    assert_eq!(
        call(&window, "ping", json!({}), "http://tauri.localhost").unwrap(),
        json!("pong")
    );
    let info = call(&window, "get_app_info", json!({}), "http://tauri.localhost").unwrap();
    assert!(info["is_admin"].is_null());
    assert_eq!(info["settings_persistent"], false);
    let saved = call(
        &window,
        "save_refresh_interval",
        json!({"intervalMs":5000}),
        "http://tauri.localhost",
    )
    .unwrap();
    assert_eq!(saved["refresh_interval_ms"], 5000);
    assert_eq!(
        call(
            &window,
            "get_app_settings",
            json!({}),
            "http://tauri.localhost"
        )
        .unwrap()["refresh_interval_ms"],
        5000
    );
    assert_eq!(
        call(
            &window,
            "save_refresh_interval",
            json!({"intervalMs":0}),
            "http://tauri.localhost"
        )
        .unwrap_err()["code"],
        "INVALID_INTERVAL"
    );
}
#[test]
fn arbitrary_window_remote_origin_and_shell_command_are_denied() {
    let app = app();
    let main = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let other = WebviewWindowBuilder::new(&app, "untrusted", Default::default())
        .build()
        .unwrap();
    assert!(call(&other, "ping", json!({}), "http://tauri.localhost").is_err());
    assert!(call(&main, "ping", json!({}), "https://example.org").is_err());
    assert!(call(
        &main,
        "execute_shell",
        json!({"command":"anything"}),
        "http://tauri.localhost"
    )
    .is_err());
}
#[test]
fn malformed_preview_and_unknown_category_are_errors_not_panics() {
    let app = app();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let response = call(
        &window,
        "get_cleanup_preview",
        json!({"previewId":"unknown","offset":0,"limit":0,"query":""}),
        "http://tauri.localhost",
    )
    .unwrap_err();
    assert_eq!(response["code"], "INVALID_PAGE");
    let response = call(
        &window,
        "start_cleanup_scan",
        json!({"categoryId":"invented"}),
        "http://tauri.localhost",
    )
    .unwrap_err();
    assert_eq!(response["code"], "UNKNOWN_CATEGORY");
}
#[test]
fn untrusted_selection_and_disabled_execution_are_rejected() {
    let app = app();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let availability = call(
        &window,
        "get_cleanup_availability",
        json!({}),
        "http://tauri.localhost",
    )
    .unwrap();
    assert_eq!(availability["execution_enabled"], false);
    let response = call(
        &window,
        "prepare_cleanup_confirmation",
        json!({"previewId":"unknown","selectedIds":["arbitrary-path"]}),
        "http://tauri.localhost",
    )
    .unwrap_err();
    assert_eq!(response["code"], "PREVIEW_NOT_FOUND");
    let response = call(
        &window,
        "start_confirmed_cleanup",
        json!({"confirmationId":"unknown","acknowledged":true}),
        "http://tauri.localhost",
    )
    .unwrap_err();
    assert_eq!(response["code"], "EXECUTION_DISABLED");
    assert!(call(
        &window,
        "get_cleanup_journal",
        json!({"before":null,"limit":10}),
        "http://tauri.localhost"
    )
    .unwrap()["operations"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn test_get_storage_resilience_status_ipc() {
    let app = app();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let status = call(
        &window,
        "get_storage_resilience_status",
        json!({}),
        "http://tauri.localhost",
    )
    .unwrap();
    assert_eq!(status["schema_version"], 2);
    assert_eq!(status["db_recovered"], false);
    assert_eq!(status["settings_recovered"], false);
}
