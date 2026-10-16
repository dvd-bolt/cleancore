//! Bounded cross-category snapshot and explicitly confirmed, journalled batches.
use crate::rules::RuleCatalog;
use cleaner_core::{
    cleanup::{CleanupItemStatus, CleanupOperationStatus, CleanupOutcome},
    errors::ApiError,
};
use cleaner_execution::{native::TrustedCandidate, now_ms, CleanupService, MAX_CANDIDATES};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

const MAX_OBJECTS: usize = 50_000;
const SNAPSHOT_TTL: u64 = 30 * 60 * 1000;
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn error(code: &str, message: &str) -> ApiError {
    ApiError {
        code: code.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneralItem {
    pub object_id: String,
    pub category_id: String,
    pub size_bytes: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct GeneralFile {
    #[serde(flatten)]
    pub item: GeneralItem,
    pub display_path: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct GeneralIndex {
    pub session_id: String,
    pub items: Vec<GeneralItem>,
    pub total_bytes: String,
    pub partial: bool,
    pub truncated: bool,
    pub consumed: bool,
    pub expires_at_ms: u64,
}
#[derive(Debug, Serialize)]
pub struct GeneralPage {
    pub items: Vec<GeneralFile>,
    pub total: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct GeneralConfirmation {
    pub confirmation_id: String,
    pub selected_count: usize,
    pub selected_bytes: String,
    pub expires_at_ms: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct GeneralJob {
    pub job_id: String,
    pub status: String,
    pub selected_count: usize,
    pub processed: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub failed: usize,
    pub deleted_bytes: String,
    pub operation_ids: Vec<String>,
    pub message: Option<String>,
    current: Option<String>,
}
#[derive(Clone)]
struct Record {
    category: String,
    object: TrustedCandidate,
}
impl Record {
    fn item(&self) -> GeneralItem {
        GeneralItem {
            object_id: self.object.object_id().into(),
            category_id: self.category.clone(),
            size_bytes: self.object.size_bytes().to_string(),
        }
    }
}
struct Snapshot {
    id: String,
    records: Vec<Record>,
    identities: HashSet<(u64, [u8; 16])>,
    batches: HashSet<String>,
    expires: u64,
    ready: bool,
    partial: bool,
    truncated: bool,
    consumed: bool,
}
struct Confirmation {
    dto: GeneralConfirmation,
    session: String,
    ids: HashSet<String>,
}
#[derive(Default)]
struct State {
    snapshot: Option<Snapshot>,
    confirmation: Option<Confirmation>,
    job: Option<GeneralJob>,
    cancel: Option<Arc<AtomicBool>>,
}
pub struct GeneralManager {
    state: Mutex<State>,
    execution: Option<Arc<CleanupService>>,
}
impl GeneralManager {
    pub fn new(execution: Option<Arc<CleanupService>>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            execution,
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, ApiError> {
        self.state
            .lock()
            .map_err(|_| error("GENERAL_STATE", "Состояние общей очистки недоступно"))
    }
    fn snapshot<'a>(state: &'a State, session: &str) -> Result<&'a Snapshot, ApiError> {
        state
            .snapshot
            .as_ref()
            .filter(|s| s.id == session && s.expires > now_ms())
            .ok_or_else(|| error("GENERAL_EXPIRED", "Общий список устарел. Повторите анализ"))
    }
    pub fn create(&self) -> Result<String, ApiError> {
        let mut state = self.lock()?;
        if state.job.as_ref().is_some_and(|j| j.status == "Running") {
            return Err(error("BUSY", "Дождитесь завершения общей очистки"));
        }
        let session = id();
        state.snapshot = Some(Snapshot {
            id: session.clone(),
            records: vec![],
            identities: HashSet::new(),
            batches: HashSet::new(),
            expires: now_ms() + SNAPSHOT_TTL,
            ready: false,
            partial: false,
            truncated: false,
            consumed: false,
        });
        state.confirmation = None;
        Ok(session)
    }
    /// Rust-only ingestion: never accepts filesystem paths or policies from IPC.
    pub(crate) fn collect(
        &self,
        session: &str,
        batch: String,
        category: &str,
        objects: Vec<TrustedCandidate>,
        partial: bool,
    ) -> Result<(), ApiError> {
        let catalog = RuleCatalog::new();
        let rule = catalog
            .get_rule(category)
            .ok_or_else(|| error("UNKNOWN_CATEGORY", "Неизвестная категория"))?;
        if category.starts_with("project-") || rule.preview_only {
            return Ok(());
        }
        let mut state = self.lock()?;
        Self::snapshot(&state, session)?;
        let snapshot = state.snapshot.as_mut().expect("checked");
        if snapshot.ready || snapshot.consumed {
            return Err(error("GENERAL_CLOSED", "Общий список уже закрыт"));
        }
        if !snapshot.batches.insert(batch) {
            return Ok(());
        }
        snapshot.partial |= partial;
        snapshot.expires = now_ms() + SNAPSHOT_TTL;
        for object in objects {
            if snapshot.identities.contains(&object.physical_id()) {
                continue;
            }
            if snapshot.records.len() >= MAX_OBJECTS {
                snapshot.truncated = true;
                continue;
            }
            snapshot.identities.insert(object.physical_id());
            snapshot.records.push(Record {
                category: category.into(),
                object,
            });
        }
        Ok(())
    }
    pub(crate) fn reject_category(&self, session: &str, category: &str) -> Result<(), ApiError> {
        let mut state = self.lock()?;
        Self::snapshot(&state, session)?;
        let snapshot = state.snapshot.as_mut().expect("checked");
        if snapshot.ready || snapshot.consumed {
            return Err(error("GENERAL_CLOSED", "Общий список уже закрыт"));
        }
        snapshot.records.retain(|r| r.category != category);
        snapshot.identities = snapshot
            .records
            .iter()
            .map(|r| r.object.physical_id())
            .collect();
        snapshot.partial = true;
        Ok(())
    }
    pub fn finish(
        &self,
        session: &str,
        cancelled: bool,
        failed_categories: &[String],
    ) -> Result<GeneralIndex, ApiError> {
        if failed_categories.len() > 32 {
            return Err(error("INVALID_SELECTION", "Недопустимый список категорий"));
        }
        let mut state = self.lock()?;
        Self::snapshot(&state, session)?;
        let snapshot = state.snapshot.as_mut().expect("checked");
        if snapshot.ready || snapshot.consumed {
            return Err(error("GENERAL_CLOSED", "Общий список уже закрыт"));
        }
        if !failed_categories.is_empty() {
            snapshot
                .records
                .retain(|r| !failed_categories.contains(&r.category));
            snapshot.identities = snapshot
                .records
                .iter()
                .map(|r| r.object.physical_id())
                .collect();
            snapshot.partial = true;
        }
        if cancelled {
            snapshot.consumed = true;
        }
        snapshot.ready = true;
        snapshot.expires = now_ms() + SNAPSHOT_TTL;
        Ok(Self::index(snapshot))
    }
    fn index(snapshot: &Snapshot) -> GeneralIndex {
        GeneralIndex {
            session_id: snapshot.id.clone(),
            items: snapshot.records.iter().map(Record::item).collect(),
            total_bytes: snapshot
                .records
                .iter()
                .map(|r| u128::from(r.object.size_bytes()))
                .sum::<u128>()
                .to_string(),
            partial: snapshot.partial,
            truncated: snapshot.truncated,
            consumed: snapshot.consumed,
            expires_at_ms: snapshot.expires,
        }
    }
    pub fn page(
        &self,
        session: &str,
        offset: usize,
        limit: usize,
        query: &str,
        selected_ids: Option<&[String]>,
    ) -> Result<GeneralPage, ApiError> {
        if limit == 0
            || limit > 100
            || offset > MAX_OBJECTS
            || query.len() > 256
            || selected_ids.is_some_and(|ids| ids.len() > MAX_OBJECTS)
        {
            return Err(error("INVALID_PAGE", "Недопустимые параметры страницы"));
        }
        let state = self.lock()?;
        let snapshot = Self::snapshot(&state, session)?;
        let query = query.to_lowercase();
        let selected: Option<HashSet<_>> =
            selected_ids.map(|ids| ids.iter().map(String::as_str).collect());
        let rows: Vec<_> = snapshot
            .records
            .iter()
            .filter(|r| {
                selected
                    .as_ref()
                    .is_none_or(|ids| ids.contains(r.object.object_id()))
            })
            .filter(|r| query.is_empty() || r.object.display_path().to_lowercase().contains(&query))
            .collect();
        Ok(GeneralPage {
            total: rows.len(),
            items: rows
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|r| GeneralFile {
                    item: r.item(),
                    display_path: r.object.display_path(),
                })
                .collect(),
        })
    }
    pub fn prepare(
        &self,
        session: &str,
        selected: Vec<String>,
        accept_partial: bool,
    ) -> Result<GeneralConfirmation, ApiError> {
        if selected.is_empty()
            || selected.len() > MAX_OBJECTS
            || selected.iter().any(|id| id.len() > 128)
        {
            return Err(error("INVALID_SELECTION", "Выберите файлы для очистки"));
        }
        let ids: HashSet<_> = selected.iter().cloned().collect();
        if ids.len() != selected.len() {
            return Err(error("INVALID_SELECTION", "Повторяющиеся идентификаторы"));
        }
        let mut state = self.lock()?;
        let snapshot = Self::snapshot(&state, session)?;
        if !snapshot.ready || snapshot.consumed {
            return Err(error(
                "GENERAL_CLOSED",
                "Дождитесь анализа или создайте новый список",
            ));
        }
        if (snapshot.partial || snapshot.truncated) && !accept_partial {
            return Err(error(
                "PARTIAL_ACK_REQUIRED",
                "Подтвердите выбор только из проверенной части",
            ));
        }
        let chosen: Vec<_> = snapshot
            .records
            .iter()
            .filter(|r| ids.contains(r.object.object_id()))
            .collect();
        if chosen.len() != ids.len() {
            return Err(error(
                "INVALID_SELECTION",
                "Выбор содержит неизвестные объекты",
            ));
        }
        let dto = GeneralConfirmation {
            confirmation_id: id(),
            selected_count: chosen.len(),
            selected_bytes: chosen
                .iter()
                .map(|r| u128::from(r.object.size_bytes()))
                .sum::<u128>()
                .to_string(),
            expires_at_ms: (now_ms() + 120_000).min(snapshot.expires),
        };
        state.confirmation = Some(Confirmation {
            dto: dto.clone(),
            session: session.into(),
            ids,
        });
        Ok(dto)
    }
    pub fn begin(
        self: &Arc<Self>,
        token: &str,
        acknowledged: bool,
    ) -> Result<GeneralJob, ApiError> {
        if !acknowledged {
            return Err(error(
                "CONFIRMATION_REQUIRED",
                "Требуется явное подтверждение безвозвратного удаления",
            ));
        }
        let service = self
            .execution
            .as_ref()
            .ok_or_else(|| error("EXECUTION_DISABLED", "Исполнитель недоступен"))?;
        if !service.availability().execution_enabled {
            return Err(error("EXECUTION_DISABLED", &service.availability().reason));
        }
        let (records, cancel, job) = {
            let mut state = self.lock()?;
            if state.job.as_ref().is_some_and(|j| j.status == "Running") {
                return Err(error("BUSY", "Общая очистка уже выполняется"));
            }
            let confirmation = state
                .confirmation
                .as_ref()
                .filter(|c| c.dto.confirmation_id == token && c.dto.expires_at_ms > now_ms())
                .ok_or_else(|| {
                    error(
                        "CONFIRMATION_EXPIRED",
                        "Подтверждение отсутствует, устарело или использовано",
                    )
                })?;
            let snapshot = Self::snapshot(&state, &confirmation.session)?;
            if snapshot.consumed {
                return Err(error("GENERAL_CLOSED", "Общий список уже использован"));
            }
            let records: Vec<_> = snapshot
                .records
                .iter()
                .filter(|r| confirmation.ids.contains(r.object.object_id()))
                .cloned()
                .collect();
            let job = GeneralJob {
                job_id: id(),
                status: "Running".into(),
                selected_count: records.len(),
                processed: 0,
                deleted: 0,
                skipped: 0,
                failed: 0,
                deleted_bytes: "0".into(),
                operation_ids: vec![],
                message: None,
                current: None,
            };
            let cancel = Arc::new(AtomicBool::new(false));
            state.snapshot.as_mut().expect("checked").consumed = true;
            state.confirmation = None;
            state.job = Some(job.clone());
            state.cancel = Some(cancel.clone());
            (records, cancel, job)
        };
        let manager = self.clone();
        if std::thread::Builder::new()
            .name("cleancore-general-cleanup".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    manager.work(records, &cancel)
                }));
                let error_message = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(e)) => Some(e.message),
                    Err(_) => Some("Общая очистка прервана; проверьте журнал".into()),
                };
                if let Ok(mut state) = manager.lock() {
                    if let Some(job) = &mut state.job {
                        job.status = if error_message.is_some() {
                            "Interrupted"
                        } else if cancel.load(Ordering::Acquire) {
                            "Cancelled"
                        } else {
                            "Completed"
                        }
                        .into();
                        job.message = error_message;
                    }
                }
            })
            .is_err()
        {
            if let Some(job) = &mut self.lock()?.job {
                job.status = "Interrupted".into();
                job.message = Some("Не удалось запустить исполнитель".into());
            }
            return Err(error(
                "WORKER_FAILED",
                "Не удалось запустить общую очистку. Повторите анализ",
            ));
        }
        Ok(job)
    }
    fn work(&self, records: Vec<Record>, cancel: &AtomicBool) -> Result<(), ApiError> {
        let service = self.execution.as_ref().expect("checked");
        let mut categories: BTreeMap<String, Vec<TrustedCandidate>> = BTreeMap::new();
        for record in records {
            categories
                .entry(record.category)
                .or_default()
                .push(record.object);
        }
        for (category, objects) in categories {
            for chunk in objects.chunks(MAX_CANDIDATES) {
                if cancel.load(Ordering::Acquire) {
                    return Ok(());
                }
                let preview = service.publish_category(chunk.to_vec(), &category)?;
                let confirmation = service.prepare_confirmation(
                    &preview.preview_id,
                    preview
                        .candidates
                        .iter()
                        .map(|c| c.object_id.clone())
                        .collect(),
                )?;
                if cancel.load(Ordering::Acquire) {
                    service.revoke_preview(&preview.preview_id)?;
                    return Ok(());
                }
                let outcome = service.begin(&confirmation.confirmation_id, true)?;
                {
                    let mut state = self.lock()?;
                    let job = state.job.as_mut().expect("registered");
                    job.current = Some(outcome.operation_id.clone());
                    job.operation_ids.push(outcome.operation_id.clone());
                }
                if cancel.load(Ordering::Acquire) {
                    service.cancel(&outcome.operation_id)?;
                }
                let outcome = service.run(&outcome.operation_id)?;
                {
                    let mut state = self.lock()?;
                    let job = state.job.as_mut().expect("registered");
                    accumulate(job, &outcome);
                    job.current = None;
                }
                if !outcome.journal_ok
                    || matches!(
                        outcome.status,
                        CleanupOperationStatus::Interrupted | CleanupOperationStatus::JournalFailed
                    )
                {
                    return Err(error("CLEANUP_INTERRUPTED", "Обработка остановлена из-за ошибки. Проверьте журнал; повторного удаления нет"));
                }
                if outcome.status == CleanupOperationStatus::Cancelled {
                    cancel.store(true, Ordering::Release);
                    return Ok(());
                }
            }
        }
        Ok(())
    }
    pub fn job(&self, job_id: &str) -> Result<GeneralJob, ApiError> {
        let mut job = self
            .lock()?
            .job
            .as_ref()
            .filter(|j| j.job_id == job_id)
            .cloned()
            .ok_or_else(|| {
                error(
                    "JOB_NOT_FOUND",
                    "Общая операция не найдена. Проверьте журнал",
                )
            })?;
        if let (Some(service), Some(current)) = (&self.execution, &job.current) {
            if let Some(outcome) = service.outcome(current)? {
                accumulate(&mut job, &outcome);
            }
        }
        Ok(job)
    }
    pub fn cancel(&self, job_id: &str) -> Result<bool, ApiError> {
        let (cancel, current) = {
            let state = self.lock()?;
            let job = state
                .job
                .as_ref()
                .filter(|j| j.job_id == job_id)
                .ok_or_else(|| error("JOB_NOT_FOUND", "Операция не найдена"))?;
            if job.status != "Running" {
                return Ok(false);
            }
            (state.cancel.clone(), job.current.clone())
        };
        if let Some(cancel) = cancel {
            cancel.store(true, Ordering::Release);
        }
        if let (Some(service), Some(current)) = (&self.execution, current) {
            service.cancel(&current)?;
        }
        Ok(true)
    }
    pub fn cancel_all(&self) {
        let job = self
            .lock()
            .ok()
            .and_then(|s| s.job.as_ref().map(|j| j.job_id.clone()));
        if let Some(job) = job {
            let _ = self.cancel(&job);
        }
    }
}
fn accumulate(job: &mut GeneralJob, outcome: &CleanupOutcome) {
    let mut bytes = job.deleted_bytes.parse::<u128>().unwrap_or(0);
    for item in &outcome.items {
        match item.status {
            CleanupItemStatus::Deleted | CleanupItemStatus::Recycled => {
                job.processed += 1;
                job.deleted += 1;
                bytes += item.size_bytes.parse::<u128>().unwrap_or(0);
            }
            CleanupItemStatus::Skipped => {
                job.processed += 1;
                job.skipped += 1;
            }
            CleanupItemStatus::Failed | CleanupItemStatus::Unknown => {
                job.processed += 1;
                job.failed += 1;
            }
            _ => {}
        }
    }
    job.deleted_bytes = bytes.to_string();
}

#[cfg(test)]
#[path = "general_tests.rs"]
mod tests;
