//! Bounded read-only app-data estimates. Matches never authorize deletion.
use crate::types::InstalledProgram;
use cleaner_core::errors::ApiError;
use cleaner_execution::native::lock_directory;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
fn fail(code: &str, message: &str) -> ApiError {
    ApiError {
        code: code.into(),
        message: message.into(),
    }
}
struct Context {
    app: InstalledProgram,
    created: Instant,
}
#[derive(Default)]
pub struct ResidueManager {
    contexts: Mutex<HashMap<String, Context>>,
    counter: AtomicU64,
}
impl ResidueManager {
    pub fn remember(&self, mut app: InstalledProgram) -> Result<String, ApiError> {
        if app.display_name.is_empty() || app.display_name.len() > 512 {
            return Err(fail(
                "INVALID_APP",
                "Название программы не подходит для анализа остатков",
            ));
        }
        app.uninstall_string = None;
        app.quiet_uninstall_string = None;
        app.install_location = None;
        let mut contexts = self
            .contexts
            .lock()
            .map_err(|_| fail("RESIDUE_STATE", "Состояние анализа недоступно"))?;
        contexts.retain(|_, c| c.created.elapsed() < Duration::from_secs(1800));
        if contexts.len() >= 16 {
            if let Some(oldest) = contexts
                .iter()
                .min_by_key(|(_, c)| c.created)
                .map(|(id, _)| id.clone())
            {
                contexts.remove(&oldest);
            }
        }
        // IDs identify read-only reports, and never authorize changes.
        let id = format!(
            "residue-{}-{}",
            cleaner_execution::now_ms(),
            self.counter.fetch_add(1, Ordering::Relaxed)
        );
        contexts.insert(
            id.clone(),
            Context {
                app,
                created: Instant::now(),
            },
        );
        Ok(id)
    }
    pub fn app(&self, id: &str) -> Result<InstalledProgram, ApiError> {
        let contexts = self
            .contexts
            .lock()
            .map_err(|_| fail("RESIDUE_STATE", "Состояние анализа недоступно"))?;
        contexts
            .get(id)
            .filter(|c| c.created.elapsed() < Duration::from_secs(1800))
            .map(|c| c.app.clone())
            .ok_or_else(|| {
                fail(
                    "CONTEXT_EXPIRED",
                    "Контекст удаления устарел. Отчёты не сохраняются между запусками",
                )
            })
    }
}
#[derive(Debug, Serialize)]
pub struct ResidueCandidate {
    pub path: String,
    pub reason: String,
    pub confidence: String,
    pub logical_bytes: Option<String>,
    pub files: u64,
    pub complete: bool,
}
#[derive(Debug, Serialize)]
pub struct ResidueReport {
    pub app_name: String,
    pub still_registered: bool,
    pub complete: bool,
    pub inspected_entries: u64,
    pub candidates: Vec<ResidueCandidate>,
    pub warnings: Vec<String>,
}

fn normalized(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn publisher_name(text: &str) -> String {
    normalized(
        &text
            .split_whitespace()
            .filter(|word| {
                !["llc", "inc", "inc.", "ltd", "ltd.", "corporation", "gmbh"]
                    .contains(&word.to_ascii_lowercase().as_str())
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}
fn relation(app: &InstalledProgram, folder: &str) -> Option<(&'static str, &'static str)> {
    let folder = normalized(folder);
    if folder.len() < 3 {
        return None;
    }
    if folder == normalized(&app.display_name) {
        return Some((
            "Совпадает полное имя папки и приложения; это не доказательство ненужности",
            "совпадение имени",
        ));
    }
    if app
        .display_name
        .split_whitespace()
        .any(|word| word.chars().count() >= 5 && normalized(word) == folder)
    {
        return Some((
            "Совпадает отдельная часть имени приложения; возможна связь с другой программой",
            "низкая",
        ));
    }
    None
}
struct Budget {
    start: Instant,
    entries: u64,
    limited: bool,
}
impl Budget {
    fn step(&mut self) -> bool {
        if self.entries >= 10000 || self.start.elapsed() >= Duration::from_secs(2) {
            self.limited = true;
            false
        } else {
            self.entries += 1;
            true
        }
    }
}
pub fn inspect(app: &InstalledProgram, still_registered: bool, roots: &[PathBuf]) -> ResidueReport {
    let mut report = ResidueReport {
        app_name: app.display_name.clone(),
        still_registered,
        complete: true,
        inspected_entries: 0,
        candidates: vec![],
        warnings: vec![],
    };
    let mut budget = Budget {
        start: Instant::now(),
        entries: 0,
        limited: false,
    };
    let mut seen = HashSet::new();
    let publisher = app
        .publisher
        .as_deref()
        .map(publisher_name)
        .unwrap_or_default();
    for root in roots {
        let Ok(_lease) = lock_directory(root, Path::new("")) else {
            report.complete = false;
            report
                .warnings
                .push("Один из каталогов профиля недоступен или перенаправлен".into());
            continue;
        };
        let Ok(entries) = std::fs::read_dir(root) else {
            report.complete = false;
            continue;
        };
        for entry in entries {
            if !budget.step() {
                break;
            }
            let Ok(entry) = entry else {
                report.complete = false;
                continue;
            };
            let path = entry.path();
            if !entry.file_type().is_ok_and(|t| t.is_dir())
                || cleaner_execution::native::is_reparse_point_or_symlink(&path)
            {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let normalized_name = normalized(&name);
            let shared_publisher = (!publisher.is_empty() && normalized_name == publisher)
                || [
                    "microsoft",
                    "google",
                    "mozilla",
                    "adobe",
                    "jetbrains",
                    "valve",
                    "epicgames",
                ]
                .contains(&normalized_name.as_str());
            if let Some((reason, confidence)) = relation(app, &name).filter(|_| !shared_publisher) {
                add_candidate(
                    root,
                    &path,
                    reason,
                    confidence,
                    &mut report,
                    &mut budget,
                    &mut seen,
                );
            } else if shared_publisher {
                // Shared publisher folders are never themselves called residue.
                let Ok(_publisher) = lock_directory(root, Path::new(name.as_ref())) else {
                    report.complete = false;
                    continue;
                };
                if let Ok(children) = std::fs::read_dir(&path) {
                    for child in children {
                        if !budget.step() {
                            break;
                        }
                        let Ok(child) = child else {
                            report.complete = false;
                            continue;
                        };
                        if !child.file_type().is_ok_and(|t| t.is_dir())
                            || cleaner_execution::native::is_reparse_point_or_symlink(&child.path())
                        {
                            continue;
                        }
                        if let Some((reason, confidence)) =
                            relation(app, &child.file_name().to_string_lossy())
                        {
                            add_candidate(
                                root,
                                &child.path(),
                                reason,
                                confidence,
                                &mut report,
                                &mut budget,
                                &mut seen,
                            );
                        }
                    }
                }
            }
        }
    }
    if budget.limited {
        report.complete = false;
        report.warnings.push(
            "Достигнут бюджет анализа: 10 000 записей или 2 секунды; размеры неполные".into(),
        );
    }
    report.inspected_entries = budget.entries;
    report
}
fn add_candidate(
    root: &Path,
    path: &Path,
    reason: &str,
    confidence: &str,
    report: &mut ResidueReport,
    budget: &mut Budget,
    seen: &mut HashSet<String>,
) {
    if report.candidates.len() >= 128 {
        report.complete = false;
        if !report.warnings.iter().any(|w| w.contains("128")) {
            report
                .warnings
                .push("Список ограничен 128 совпадениями".into());
        }
        return;
    }
    if !seen.insert(path.to_string_lossy().to_lowercase()) {
        return;
    }
    let (bytes, files, complete) = estimate(root, path, 0, budget);
    if !complete {
        report.complete = false;
    }
    report.candidates.push(ResidueCandidate {
        path: path.to_string_lossy().into_owned(),
        reason: reason.into(),
        confidence: confidence.into(),
        logical_bytes: if files == 0 && !complete {
            None
        } else {
            Some(bytes.to_string())
        },
        files,
        complete,
    });
}
fn estimate(root: &Path, dir: &Path, depth: usize, budget: &mut Budget) -> (u64, u64, bool) {
    if depth > 16 {
        return (0, 0, false);
    }
    let Ok(relative) = dir.strip_prefix(root) else {
        return (0, 0, false);
    };
    let Ok(_lease) = lock_directory(root, relative) else {
        return (0, 0, false);
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0, false);
    };
    let (mut bytes, mut files, mut complete) = (0u64, 0u64, true);
    for entry in entries {
        if !budget.step() {
            complete = false;
            break;
        }
        let Ok(entry) = entry else {
            complete = false;
            continue;
        };
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            complete = false;
            continue;
        };
        if cleaner_execution::native::is_reparse_point_or_symlink(&path) {
            complete = false;
            continue;
        }
        if meta.is_dir() {
            let (b, f, c) = estimate(root, &path, depth + 1, budget);
            bytes = bytes.saturating_add(b);
            files += f;
            complete &= c;
        } else if meta.is_file() {
            bytes = bytes.saturating_add(meta.len());
            files += 1;
        }
    }
    (bytes, files, complete)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn residue_estimates_are_read_only_and_never_select_shared_publisher_folder() {
        let root =
            std::env::temp_dir().join(format!("cleancore_residue_{}", cleaner_execution::now_ms()));
        std::fs::create_dir_all(root.join("Microsoft/Widget")).unwrap();
        std::fs::create_dir_all(root.join("Microsoft/OtherApp")).unwrap();
        std::fs::write(
            root.join("Microsoft/Widget/settings.json"),
            b"owned fixture",
        )
        .unwrap();
        let app: InstalledProgram = serde_json::from_value(serde_json::json!({"id":"fixture", "display_name":"Microsoft Widget", "publisher":"Microsoft Corporation", "display_version":null, "install_date":null, "estimated_size_bytes":null, "uninstall_string":null, "quiet_uninstall_string":null, "install_location":null, "registry_hive":"fixture", "is_wow64":false, "is_system_component":false})).unwrap();
        let report = inspect(&app, false, std::slice::from_ref(&root));
        assert!(report.complete);
        assert_eq!(report.candidates.len(), 1);
        assert!(report.candidates[0].path.ends_with("Microsoft\\Widget"));
        assert_eq!(report.candidates[0].logical_bytes.as_deref(), Some("13"));
        assert!(root.join("Microsoft/Widget/settings.json").exists());
        assert!(!report
            .candidates
            .iter()
            .any(|c| c.path.ends_with("Microsoft")));
        let manager = ResidueManager::default();
        let id = manager.remember(app).unwrap();
        assert!(manager.app(&id).is_ok());
        assert!(manager.app("invented").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
