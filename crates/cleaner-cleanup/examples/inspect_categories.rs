//! Read-only native smoke probe. No executor or journal is constructed.
use cleaner_cleanup::TaskManager;
use cleaner_core::models::TaskState;
use std::time::{Duration, Instant};
fn main() {
    let manager = TaskManager::new();
    for rule in manager.rules() {
        let task = if rule.id.starts_with("project-") {
            manager.start_project_scan(&rule.id, &std::env::current_dir().unwrap(), vec![])
        } else {
            manager.start_scan(&rule.id)
        };
        let id = match task {
            Ok(id) => id,
            Err(error) => {
                println!("{}: unavailable ({})", rule.id, error.code);
                continue;
            }
        };
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut cancellation_sent = false;
        loop {
            let status = manager.get_task_status(&id).unwrap().unwrap();
            if status.state != TaskState::Running {
                match manager.get_preview_page(&id, 0, 1, "") {
                    Ok(page) => println!(
                        "{}: {:?}, inspected={}, batch={}, resumable={}, diagnostics={:?}",
                        rule.id,
                        status.state,
                        status.processed_items,
                        page.unfiltered_candidates,
                        page.resumable,
                        page.diagnostics
                    ),
                    Err(error) => println!("{}: {:?} ({})", rule.id, status.state, error.code),
                }
                break;
            }
            if Instant::now() >= deadline && !cancellation_sent {
                manager.cancel_scan(&id).unwrap();
                cancellation_sent = true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    manager.cancel_all();
}
