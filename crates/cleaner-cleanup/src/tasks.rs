//! One active scan, bounded snapshots, typed failures and exact scan-to-execution binding.
use crate::{
    path_safety::resolve_user_temp_root,
    rules::RuleCatalog,
    scanner::{scan_directory, ScannerOptions},
};
use cleaner_core::{
    errors::ApiError,
    models::{CleanupCandidate, CleanupPreviewPage, TaskState, TaskStatus},
};
use cleaner_execution::CleanupService;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;
#[derive(Debug, Clone)]
pub struct StoredPreview {
    pub preview_id: String,
    pub category_id: String,
    pub rule_version: String,
    pub scanned_time_utc: String,
    pub total_bytes: u64,
    pub candidates: Vec<CleanupCandidate>,
    pub complete: bool,
    pub cancelled: bool,
    pub diagnostics: BTreeMap<String, u64>,
    pub execution_preview_id: Option<String>,
    created: Instant,
}
struct Entry {
    status: TaskStatus,
    cancel: Arc<AtomicBool>,
    preview: Option<StoredPreview>,
    created: Instant,
}
struct Registry {
    entries: HashMap<String, Entry>,
    active: Option<String>,
}
pub struct TaskManager {
    catalog: RuleCatalog,
    registry: Arc<Mutex<Registry>>,
    execution: Option<Arc<CleanupService>>,
}
fn error(code: &str, message: &str) -> ApiError {
    ApiError {
        code: code.into(),
        message: message.into(),
    }
}
const RETENTION: Duration = Duration::from_secs(600);
impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}
impl TaskManager {
    pub fn new() -> Self {
        Self {
            catalog: RuleCatalog::new(),
            registry: Arc::new(Mutex::new(Registry {
                entries: HashMap::new(),
                active: None,
            })),
            execution: None,
        }
    }
    pub fn with_execution(execution: Arc<CleanupService>) -> Self {
        let mut manager = Self::new();
        manager.execution = Some(execution);
        manager
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Registry>, ApiError> {
        self.registry
            .lock()
            .map_err(|_| error("SCAN_STATE", "Состояние сканирования недоступно"))
    }
    pub fn start_scan(&self, category: &str) -> Result<String, ApiError> {
        if self.catalog.get_rule(category).is_none() {
            return Err(error("UNKNOWN_CATEGORY", "Неизвестная категория"));
        }
        let root = resolve_user_temp_root().map_err(|_| {
            error(
                "INVALID_TEMP_ROOT",
                "Windows-каталог временных файлов недоступен или перенаправлен; анализ не начат",
            )
        })?;
        self.start_at_root(category, root, ScannerOptions::default())
    }
    fn start_at_root(
        &self,
        category: &str,
        root: PathBuf,
        options: ScannerOptions,
    ) -> Result<String, ApiError> {
        let rule = self
            .catalog
            .get_rule(category)
            .ok_or_else(|| error("UNKNOWN_CATEGORY", "Неизвестная категория"))?
            .clone();
        let id = Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut r = self.lock()?;
            if r.active.is_some() {
                return Err(error("SCAN_BUSY", "Сканирование уже выполняется"));
            }
            r.entries.retain(|_, e| e.created.elapsed() < RETENTION);
            if r.entries.len() >= 8 {
                if let Some(oldest) = r
                    .entries
                    .iter()
                    .min_by_key(|(_, e)| e.created)
                    .map(|(id, _)| id.clone())
                {
                    r.entries.remove(&oldest);
                }
            }
            r.entries.insert(
                id.clone(),
                Entry {
                    status: TaskStatus {
                        id: id.clone(),
                        kind: format!("scan:{}", rule.id),
                        state: TaskState::Running,
                        processed_items: 0,
                        total_items: None,
                        error_message: None,
                    },
                    cancel: cancel.clone(),
                    preview: None,
                    created: Instant::now(),
                },
            );
            r.active = Some(id.clone());
        }
        let registry = self.registry.clone();
        let execution = self.execution.clone();
        let worker_id = id.clone();
        let spawned = thread::Builder::new()
            .name("cleancore-scan".into())
            .spawn(move || {
                let progress_registry = registry.clone();
                let progress_id = worker_id.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    scan_directory(&root, &rule, &options, cancel, move |count, _, _| {
                        if let Ok(mut r) = progress_registry.lock() {
                            if let Some(entry) = r.entries.get_mut(&progress_id) {
                                entry.status.processed_items = count;
                            }
                        }
                    })
                }));
                if let Ok(mut r) = registry.lock() {
                    r.active = None;
                    if let Some(entry) = r.entries.get_mut(&worker_id) {
                        match result {
                            Err(_) => {
                                entry.status.state = TaskState::Failed;
                                entry.status.error_message =
                                    Some("Сканер прерван; результат не считается полным".into());
                            }
                            Ok(mut result) => {
                                entry.status.processed_items = result.total_scanned_items;
                                let mut execution_id = None;
                                if result.complete {
                                    if let Some(service) = execution {
                                        match service.publish_preview(result.identities) {
                                            Ok(preview) => execution_id = Some(preview.preview_id),
                                            Err(e) => {
                                                result.complete = false;
                                                *result.diagnostics.entry(e.code).or_default() += 1;
                                            }
                                        }
                                    }
                                }
                                entry.status.state = if result.fatal_error.is_some() {
                                    TaskState::Failed
                                } else if result.was_cancelled {
                                    TaskState::Cancelled
                                } else if !result.complete {
                                    TaskState::Partial
                                } else {
                                    TaskState::Completed
                                };
                                entry.status.total_items = if result.complete {
                                    Some(result.total_scanned_items)
                                } else {
                                    None
                                };
                                entry.status.error_message = result
                                    .fatal_error
                                    .map(|code| format!("Не удалось прочитать корень: {code}"));
                                entry.preview = Some(StoredPreview {
                                    preview_id: worker_id.clone(),
                                    category_id: rule.id,
                                    rule_version: rule.version,
                                    scanned_time_utc: crate::scanner::system_time_to_iso(
                                        std::time::SystemTime::now(),
                                    ),
                                    total_bytes: result.total_matched_bytes,
                                    candidates: result.candidates,
                                    complete: result.complete,
                                    cancelled: result.was_cancelled,
                                    diagnostics: result.diagnostics,
                                    execution_preview_id: execution_id,
                                    created: Instant::now(),
                                });
                            }
                        }
                    }
                }
            });
        if spawned.is_err() {
            let mut r = self.lock()?;
            r.active = None;
            if let Some(entry) = r.entries.get_mut(&id) {
                entry.status.state = TaskState::Failed;
                entry.status.error_message = Some("Не удалось запустить рабочий поток".into());
            }
            return Err(error("SCAN_WORKER", "Не удалось запустить сканирование"));
        }
        Ok(id)
    }
    pub fn cancel_scan(&self, id: &str) -> Result<bool, ApiError> {
        let r = self.lock()?;
        if let Some(entry) = r.entries.get(id) {
            if entry.status.state == TaskState::Running {
                entry.cancel.store(true, Ordering::Release);
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn cancel_all(&self) {
        if let Ok(r) = self.registry.lock() {
            for entry in r.entries.values() {
                entry.cancel.store(true, Ordering::Release);
            }
        }
    }
    pub fn get_task_status(&self, id: &str) -> Result<Option<TaskStatus>, ApiError> {
        Ok(self.lock()?.entries.get(id).map(|e| e.status.clone()))
    }
    pub fn get_preview_page(
        &self,
        id: &str,
        offset: usize,
        limit: usize,
        query: &str,
    ) -> Result<CleanupPreviewPage, ApiError> {
        if id.len() > 128 || !(1..=100).contains(&limit) || query.len() > 512 {
            return Err(error("INVALID_PAGE", "Некорректные параметры preview"));
        }
        let r = self.lock()?;
        let stored = r
            .entries
            .get(id)
            .and_then(|e| e.preview.as_ref())
            .ok_or_else(|| {
                error(
                    "PREVIEW_NOT_FOUND",
                    "Снимок не найден или сканирование ещё выполняется",
                )
            })?;
        if stored.created.elapsed() >= RETENTION {
            return Err(error(
                "PREVIEW_EXPIRED",
                "Снимок устарел; выполните новый анализ",
            ));
        }
        let query = query.trim().to_lowercase();
        let filtered: Vec<_> = stored
            .candidates
            .iter()
            .filter(|c| {
                query.is_empty()
                    || c.display_path.to_lowercase().contains(&query)
                    || c.reason.to_lowercase().contains(&query)
            })
            .collect();
        let count = filtered.len();
        let items = filtered
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        Ok(CleanupPreviewPage {
            preview_id: stored.preview_id.clone(),
            category_id: stored.category_id.clone(),
            offset,
            limit,
            total_candidates: count,
            total_bytes: stored.total_bytes.to_string(),
            items,
            rule_version: stored.rule_version.clone(),
            complete: stored.complete,
            cancelled: stored.cancelled,
            diagnostics: stored.diagnostics.clone(),
            execution_preview_id: stored.execution_preview_id.clone(),
            unfiltered_candidates: stored.candidates.len(),
        })
    }
}
impl Drop for TaskManager {
    fn drop(&mut self) {
        self.cancel_all();
    }
}
#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
