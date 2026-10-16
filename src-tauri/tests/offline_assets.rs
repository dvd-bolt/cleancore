#![cfg(feature = "custom-protocol")]

#[test]
fn packaged_frontend_contains_its_entrypoint_and_assets() {
    assert!(!tauri::is_dev(), "packaged app must not use devUrl");
    let context: tauri::Context<tauri::Wry> = tauri::generate_context!();
    let index = context
        .assets()
        .get(&"/index.html".into())
        .expect("offline index.html must be embedded");
    let html = std::str::from_utf8(&index).expect("HTML must be UTF-8");
    assert!(html.contains("id=\"root\""));
    let mut scripts = 0;
    let mut styles = 0;
    for asset in html.split('"').filter(|part| part.starts_with("/assets/")) {
        assert!(
            context.assets().get(&asset.into()).is_some(),
            "missing embedded asset: {asset}"
        );
        scripts += usize::from(asset.ends_with(".js"));
        styles += usize::from(asset.ends_with(".css"));
    }
    assert!(scripts > 0, "offline JavaScript must be embedded");
    assert!(styles > 0, "offline CSS must be embedded");
}
