//! Manual read-only acceptance probe: run the compiled example elevated, passing C:\.
//! Only a local JSON report is saved; no filesystem objects under analysis are changed.
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};
fn main() {
    let drive = std::env::args().nth(1).unwrap_or_else(|| "C:\\".into());
    let cancel_ms = std::env::args()
        .nth(3)
        .map(|s| s.parse::<u64>().expect("cancel milliseconds"));
    let result_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(if cancel_ms.is_some() {
        "../../artifacts/mft-probe-cancel.json"
    } else {
        "../../artifacts/mft-probe.json"
    });
    let started = Instant::now();
    let result = (|| -> Result<serde_json::Value, String> {
        let letter = cleaner_disk::ntfs::drive_letter(Path::new(&drive))
            .ok_or("Pass an ordinary drive root")?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(ms) = cancel_ms {
            let token = cancel.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(ms));
                token.store(true, std::sync::atomic::Ordering::Release);
            });
        }
        let snapshot = cleaner_disk::ntfs::scan(letter, cancel.clone(), &|_, _| {})?;
        let report = snapshot
            .report(
                Path::new(&drive),
                &cleaner_disk::AnalyzerOptions::default(),
                &cancel,
            )
            .ok_or("No report")?;
        let folder = std::env::args().nth(2).and_then(|p| {
            snapshot.report(
                Path::new(&p),
                &cleaner_disk::AnalyzerOptions::default(),
                &cancel,
            )
        });
        let tree_started = Instant::now();
        let treemap = snapshot
            .treemap(
                Path::new(&drive),
                cleaner_disk::models::TreemapSize::Logical,
            )
            .ok_or("No treemap")?;
        let tree_ms = tree_started.elapsed().as_millis();
        let map_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/mft-treemap.json");
        std::fs::write(
            map_path,
            serde_json::to_vec(&treemap).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(
            serde_json::json!({"report":report,"folder":folder,"treemap_ms":tree_ms,"wall_ms":started.elapsed().as_millis()}),
        )
    })();
    let output = match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({"error":e,"wall_ms":started.elapsed().as_millis()}),
    };
    let bytes = serde_json::to_vec_pretty(&output).unwrap();
    // Fixed workspace artifact; this example is never exposed as a Tauri command.
    std::fs::write(&result_path, bytes).expect("Unable to save local acceptance artifact");
    println!("{}", result_path.display());
}
