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
        cleanup_preferences: Arc::new(std::sync::RwLock::new(Some(Default::default()))),
        residue_manager: Arc::new(cleaner_service::residue::ResidueManager::default()),
        is_paused: AtomicBool::new(false),
        interval_ms: AtomicU64::new(2000),
        settings_change: Mutex::new(()),
        shutdown: AtomicBool::new(false),
        assistant: Arc::new(cleaner_assistant::Assistant::default()),
    };
    mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            service_commands::set_startup_enabled,
            assistant_commands::assistant_status,
            assistant_commands::assistant_prepare_context,
            assistant_commands::assistant_enable,
            assistant_commands::assistant_ask,
            assistant_commands::assistant_disable,
            general_cleanup_commands::create_general_cleanup,
            general_cleanup_commands::collect_general_cleanup,
            general_cleanup_commands::finish_general_cleanup,
            general_cleanup_commands::get_general_cleanup_page,
            general_cleanup_commands::prepare_general_cleanup,
            general_cleanup_commands::execute_general_cleanup,
            general_cleanup_commands::get_general_cleanup_job,
            general_cleanup_commands::cancel_general_cleanup_job,
            commands::get_app_info,
            commands::get_app_settings,
            commands::save_refresh_interval,
            commands::get_storage_resilience_status,
            commands::start_cleanup_scan,
            commands::resume_cleanup_scan,
            commands::get_cleanup_rules,
            residue_commands::launch_uninstaller_with_residue_context,
            residue_commands::inspect_app_residue,
            commands::get_cleanup_profile_context,
            disk_commands::invalidate_disk_report,
            commands::get_cleanup_preferences,
            commands::save_cleanup_preferences,
            commands::clear_duplicate_hash_cache,
            disk_commands::prepare_disk_file_cleanup,
            disk_commands::prepare_duplicate_cleanup,
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

#[test]
fn assistant_acl_disabled_state_and_untrusted_context_are_enforced() {
    let app = app();
    let main = WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::App("index.html".into()))
        .build()
        .unwrap();
    let status = call(
        &main,
        "assistant_status",
        json!({}),
        "http://tauri.localhost",
    )
    .unwrap();
    assert_eq!(status["enabled"], false);
    assert!(status["model"].is_null());
    assert!(call(&main, "assistant_status", json!({}), "https://evil.example").is_err());
    let selection =
        json!({"selection":{"metrics":false,"preview_id":null,"journal_ids":[],"shell":"cmd.exe"}});
    assert!(call(
        &main,
        "assistant_prepare_context",
        selection,
        "http://tauri.localhost"
    )
    .is_err());
    assert!(call(
        &main,
        "assistant_ask",
        json!({"contextId":"invented","question":"delete everything"}),
        "http://tauri.localhost"
    )
    .is_err());
}

#[test]
fn startup_change_requires_local_main_and_explicit_acknowledgement() {
    let app = app();
    let window =
        WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::App("index.html".into()))
            .build()
            .unwrap();
    let args = json!({"itemId":"HKCU_Run_fixture","expectedCommand":"fixture.exe","enabled":false,"acknowledged":false});
    assert!(call(
        &window,
        "set_startup_enabled",
        args.clone(),
        "https://example.invalid/"
    )
    .is_err());
    let result = call(
        &window,
        "set_startup_enabled",
        args,
        "http://tauri.localhost/",
    )
    .unwrap_err();
    assert!(result.to_string().contains("подтверждение"));
}

#[test]
fn new_cleanup_commands_validate_ids_preferences_and_origins() {
    let app = app();
    let main = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let other = WebviewWindowBuilder::new(&app, "other", Default::default())
        .build()
        .unwrap();
    let url = "http://tauri.localhost";
    assert_eq!(
        call(&main, "get_cleanup_rules", json!({}), url)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        11
    );
    assert_eq!(
        call(
            &main,
            "resume_cleanup_scan",
            json!({"taskId":"invented"}),
            url
        )
        .unwrap_err()["code"],
        "SCAN_NOT_FOUND"
    );
    assert_eq!(
        call(
            &main,
            "prepare_disk_file_cleanup",
            json!({"reportId":"old","entryId":"aggregate"}),
            url
        )
        .unwrap_err()["code"],
        "INVALID_ID"
    );
    assert_eq!(
        call(
            &main,
            "prepare_duplicate_cleanup",
            json!({"reportId":"old","groupId":"old","keeperIndex":0,"selectedIndices":[]}),
            url
        )
        .unwrap_err()["code"],
        "INVALID_SELECTION"
    );
    assert_eq!(call(&main, "save_cleanup_preferences", json!({"preferences":{"excluded_paths":["C:\\x\\..\\y"],"protected_duplicate_roots":[]}}), url).unwrap_err()["code"], "INVALID_EXCLUSION");
    let preferences =
        json!({"excluded_paths":["C:\\Keep"],"protected_duplicate_roots":["D:\\Originals"]});
    call(
        &main,
        "save_cleanup_preferences",
        json!({"preferences":preferences.clone()}),
        url,
    )
    .unwrap();
    assert_eq!(
        call(&main, "get_cleanup_preferences", json!({}), url).unwrap(),
        preferences
    );
    for command in [
        "get_cleanup_rules",
        "launch_uninstaller_with_residue_context",
        "inspect_app_residue",
        "get_cleanup_profile_context",
        "invalidate_disk_report",
        "get_cleanup_preferences",
        "resume_cleanup_scan",
        "prepare_disk_file_cleanup",
        "prepare_duplicate_cleanup",
        "save_cleanup_preferences",
        "clear_duplicate_hash_cache",
    ] {
        assert!(call(&other, command, json!({}), url).is_err());
        assert!(call(&main, command, json!({}), "https://example.invalid").is_err());
    }
}

#[test]
fn residue_actions_require_explicit_uninstall_acknowledgement_and_a_saved_context() {
    let app = app();
    let main = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let url = "http://tauri.localhost";
    assert!(call(
        &main,
        "launch_uninstaller",
        json!({"programId":"invented"}),
        url
    )
    .unwrap_err()
    .is_string());
    assert_eq!(
        call(
            &main,
            "launch_uninstaller_with_residue_context",
            json!({"programId":"invented","acknowledged":false}),
            url
        )
        .unwrap_err()["code"],
        "CONFIRMATION_REQUIRED"
    );
    assert_eq!(
        call(
            &main,
            "inspect_app_residue",
            json!({"contextId":"invented"}),
            url
        )
        .unwrap_err()["code"],
        "CONTEXT_EXPIRED"
    );
}

#[test]
fn general_cleanup_is_local_only_and_requires_known_objects_and_acknowledgement() {
    let app = app();
    let main = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let url = "http://tauri.localhost";
    let session = call(&main, "create_general_cleanup", json!({}), url).unwrap();
    let index = call(
        &main,
        "finish_general_cleanup",
        json!({"sessionId":session,"cancelled":false,"failedCategories":[]}),
        url,
    )
    .unwrap();
    assert_eq!(index["total_bytes"], "0");
    assert_eq!(
        call(
            &main,
            "prepare_general_cleanup",
            json!({"sessionId":session,"selectedIds":["invented"],"acceptPartial":false}),
            url
        )
        .unwrap_err()["code"],
        "INVALID_SELECTION"
    );
    assert_eq!(
        call(
            &main,
            "execute_general_cleanup",
            json!({"confirmationId":"invented","acknowledged":false}),
            url
        )
        .unwrap_err()["code"],
        "CONFIRMATION_REQUIRED"
    );
    for command in [
        "create_general_cleanup",
        "collect_general_cleanup",
        "finish_general_cleanup",
        "get_general_cleanup_page",
        "prepare_general_cleanup",
        "execute_general_cleanup",
        "get_general_cleanup_job",
        "cancel_general_cleanup_job",
    ] {
        let args = json!({"sessionId":session,"taskId":"unknown","jobId":"unknown","confirmationId":"unknown","acknowledged":false,"cancelled":false,"failedCategories":[],"offset":0,"limit":50,"query":"","selectedIds":["unknown"],"acceptPartial":false});
        assert!(call(&main, command, args, "https://example.invalid")
            .unwrap_err()
            .is_string());
    }
    let other = WebviewWindowBuilder::new(&app, "other-general", Default::default())
        .build()
        .unwrap();
    assert!(call(&other, "create_general_cleanup", json!({}), url)
        .unwrap_err()
        .is_string());
}
