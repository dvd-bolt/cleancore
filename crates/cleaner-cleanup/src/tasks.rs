//! One active scan, resumable bounded batches and exact scan-to-execution binding.
use crate::{
    path_safety::{browser_process, resolve_category_roots},
    rules::RuleCatalog,
    scanner::{ScanSession, ScannerOptions},
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
    pub resumable: bool,
    pub batch_ready: bool,
    created: Instant,
}
struct Entry {
    status: TaskStatus,
    cancel: Arc<AtomicBool>,
    preview: Option<StoredPreview>,
    continuation: Option<ScanSession>,
    trusted: Vec<cleaner_execution::native::TrustedCandidate>,
    revision: u64,
    created: Instant,
}
struct Registry {
    entries: HashMap<String, Entry>,
    active: Option<String>,
}
pub struct TaskManager {
    catalog: RuleCatalog,
    pub general: Arc<crate::general::GeneralManager>,
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
            general: Arc::new(crate::general::GeneralManager::new(None)),
            registry: Arc::new(Mutex::new(Registry {
                entries: HashMap::new(),
                active: None,
            })),
            execution: None,
        }
    }
    pub fn with_execution(execution: Arc<CleanupService>) -> Self {
        let mut manager = Self::new();
        manager.general = Arc::new(crate::general::GeneralManager::new(Some(execution.clone())));
        manager.execution = Some(execution);
        manager
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Registry>, ApiError> {
        self.registry
            .lock()
            .map_err(|_| error("SCAN_STATE", "Состояние сканирования недоступно"))
    }
    pub fn start_scan(&self, category: &str) -> Result<String, ApiError> {
        self.start_scan_with_exclusions(category, vec![])
    }
    pub fn start_scan_with_exclusions(
        &self,
        category: &str,
        excluded_paths: Vec<String>,
    ) -> Result<String, ApiError> {
        self.catalog
            .validate()
            .map_err(|e| error("INVALID_CATALOG", e))?;
        if self.catalog.get_rule(category).is_none() {
            return Err(error("UNKNOWN_CATEGORY", "Неизвестная категория"));
        }
        if let Some(process) = browser_process(category) {
            if cleaner_execution::native::process_running(process).map_err(|_| {
                error(
                    "PROCESS_CHECK_FAILED",
                    "Не удалось проверить запущенные приложения",
                )
            })? {
                return Err(error(
                    "APPLICATION_RUNNING",
                    "Закройте браузер и его фоновые процессы перед анализом кеша",
                ));
            }
        }
        let roots = resolve_category_roots(category)
            .map_err(|e| error("CATEGORY_UNAVAILABLE", &e.to_string()))?;
        self.start_at_roots(
            category,
            roots,
            ScannerOptions {
                excluded_paths,
                ..ScannerOptions::default()
            },
        )
    }
    #[cfg(test)]
    fn start_at_root(
        &self,
        category: &str,
        root: PathBuf,
        options: ScannerOptions,
    ) -> Result<String, ApiError> {
        self.start_at_roots(category, vec![root], options)
    }
    pub fn collect_general_batch(&self, session: &str, task: &str) -> Result<(), ApiError> {
        let r = self.lock()?;
        let entry = r
            .entries
            .get(task)
            .ok_or_else(|| error("SCAN_NOT_FOUND", "Задача не найдена"))?;
        let preview = entry
            .preview
            .as_ref()
            .filter(|_| entry.created.elapsed() < RETENTION)
            .ok_or_else(|| error("PREVIEW_EXPIRED", "Нет готовой партии"))?;
        if matches!(
            entry.status.state,
            TaskState::Running | TaskState::Pending | TaskState::Failed | TaskState::Cancelled
        ) {
            return Err(error("SCAN_NOT_READY", "Нет проверенной партии"));
        }
        if preview.resumable && !preview.batch_ready {
            return Ok(());
        }
        if preview
            .diagnostics
            .keys()
            .any(|code| code.ends_with("_LIMIT"))
        {
            return self.general.reject_category(session, &preview.category_id);
        }
        self.general.collect(
            session,
            format!("{task}:{}", entry.revision),
            &preview.category_id,
            entry.trusted.clone(),
            !preview.complete && !preview.batch_ready,
        )
    }
    pub fn rules(&self) -> Vec<crate::rules::CleanupRule> {
        self.catalog.all_rules().to_vec()
    }
    pub fn start_project_scan(
        &self,
        category: &str,
        project: &std::path::Path,
        excluded_paths: Vec<String>,
    ) -> Result<String, ApiError> {
        self.catalog
            .validate()
            .map_err(|e| error("INVALID_CATALOG", e))?;
        let root = crate::path_safety::resolve_project_cache(project, category)
            .map_err(|e| error("INVALID_PROJECT", &e.to_string()))?;
        self.start_at_roots(
            category,
            vec![root],
            ScannerOptions {
                excluded_paths,
                ..ScannerOptions::default()
            },
        )
    }
    fn start_at_roots(
        &self,
        category: &str,
        roots: Vec<PathBuf>,
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
                    continuation: None,
                    trusted: vec![],
                    revision: 0,
                    created: Instant::now(),
                },
            );
            r.active = Some(id.clone());
        }
        self.spawn(
            id.clone(),
            ScanSession::new_roots(roots, rule, options),
            cancel,
        )?;
        Ok(id)
    }
    pub fn resume_scan(&self, id: &str) -> Result<String, ApiError> {
        if id.len() > 128 {
            return Err(error("INVALID_TASK", "Некорректный ID задачи"));
        }
        let (mut session, cancel) = {
            let mut r = self.lock()?;
            if r.active.is_some() {
                return Err(error("SCAN_BUSY", "Сканирование уже выполняется"));
            }
            let entry = r
                .entries
                .get_mut(id)
                .ok_or_else(|| error("SCAN_NOT_FOUND", "Задача не найдена"))?;
            if entry.created.elapsed() >= RETENTION {
                entry.continuation = None;
                return Err(error(
                    "PREVIEW_EXPIRED",
                    "Задача устарела; выполните новый анализ",
                ));
            }
            if entry.cancel.load(Ordering::Acquire) || entry.continuation.is_none() {
                return Err(error("NOT_RESUMABLE", "Продолжение этой задачи недоступно"));
            }
            if let Some(preview_id) = entry
                .preview
                .as_ref()
                .and_then(|p| p.execution_preview_id.as_ref())
            {
                if let Some(service) = &self.execution {
                    service.revoke_preview(preview_id)?;
                }
            }
            let session = entry.continuation.take().expect("checked above");
            entry.preview = None;
            entry.trusted.clear();
            entry.revision += 1;
            entry.status.state = TaskState::Running;
            entry.created = Instant::now();
            let cancel = entry.cancel.clone();
            r.active = Some(id.into());
            (session, cancel)
        };
        session.next_batch();
        self.spawn(id.into(), session, cancel)?;
        Ok(id.into())
    }
    fn spawn(
        &self,
        id: String,
        mut session: ScanSession,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), ApiError> {
        let registry = self.registry.clone();
        let execution = self.execution.clone();
        let worker_id = id.clone();
        let catalog = self.catalog.clone();
        let spawned = thread::Builder::new()
            .name("cleancore-scan".into())
            .spawn(move || {
                let progress_registry = registry.clone();
                let progress_id = worker_id.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let progress = |count, _, _| {
                        if let Ok(mut r) = progress_registry.lock() {
                            if let Some(entry) = r.entries.get_mut(&progress_id) {
                                entry.status.processed_items = count;
                            }
                        }
                    };
                    let mut previous = 0;
                    loop {
                        let slice = session.run_slice(&cancel, progress);
                        // Resource slices yield backend progress automatically.
                        // Only a confirmation batch asks the user to continue.
                        // No-progress configurations must not spin forever.
                        if !slice.resumable
                            || slice.batch_ready
                            || slice.total_scanned_items == previous
                        {
                            break slice;
                        }
                        previous = slice.total_scanned_items;
                        thread::yield_now();
                    }
                }));
                if let Ok(mut r) = registry.lock() {
                    r.active = None;
                    if let Some(entry) = r.entries.get_mut(&worker_id) {
                        entry.created = Instant::now();
                        match result {
                            Err(_) => {
                                entry.status.state = TaskState::Failed;
                                entry.status.error_message =
                                    Some("Сканер прерван; результат не считается полным".into());
                            }
                            Ok(mut result) => {
                                entry.status.processed_items = result.total_scanned_items;
                                let category = entry
                                    .status
                                    .kind
                                    .strip_prefix("scan:")
                                    .unwrap_or("")
                                    .to_owned();
                                let mut execution_id = None;
                                let rule = catalog.get_rule(&category);
                                entry.trusted = if result.execution_subset_allowed()
                                    && rule.is_some_and(|r| !r.preview_only)
                                {
                                    result.identities.clone()
                                } else {
                                    vec![]
                                };
                                if result.execution_subset_allowed()
                                    && rule.is_some_and(|r| !r.preview_only)
                                    && !result.identities.is_empty()
                                {
                                    if let Some(service) = execution {
                                        match service.publish_category(
                                            std::mem::take(&mut result.identities),
                                            &category,
                                        ) {
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
                                    category_id: category,
                                    rule_version: rule
                                        .map(|r| r.version.clone())
                                        .unwrap_or_default(),
                                    scanned_time_utc: crate::scanner::system_time_to_iso(
                                        std::time::SystemTime::now(),
                                    ),
                                    total_bytes: result.total_matched_bytes,
                                    candidates: result.candidates,
                                    complete: result.complete,
                                    cancelled: result.was_cancelled,
                                    diagnostics: result.diagnostics,
                                    execution_preview_id: execution_id,
                                    resumable: result.resumable,
                                    batch_ready: result.batch_ready,
                                    created: Instant::now(),
                                });
                                if result.resumable {
                                    entry.continuation = Some(session);
                                }
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
        Ok(())
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
        self.general.cancel_all();
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
            resumable: stored.resumable,
            batch_ready: stored.batch_ready,
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
