//! Stage 4 lifecycle, independent of the scanner implementation.
pub mod native;
use cleaner_core::{cleanup::*, errors::ApiError};
use native::TrustedCandidate;
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{SystemTime, UNIX_EPOCH};
pub const MAX_CANDIDATES: usize = 1000;
const PREVIEW_TTL_MS: u64 = 600_000;
const CONFIRM_TTL_MS: u64 = 120_000;
const MIN_AGE_MS: u64 = 7 * 24 * 60 * 60 * 1000;
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn err(code: &str, message: &str) -> ApiError {
    ApiError {
        code: code.into(),
        message: message.into(),
    }
}
pub enum ActionResult {
    Recycled,
    Skipped(String),
    Failed(String),
    Unknown(String),
}
/// Must combine validation AND mutation with proven object and ancestor binding.
pub trait ObjectAction: Send + Sync {
    fn available(&self) -> bool;
    fn apply(&self, candidate: &TrustedCandidate) -> ActionResult;
}
struct Snapshot {
    preview: CleanupPreview,
    objects: Vec<TrustedCandidate>,
    consumed: bool,
}
struct Work {
    objects: Vec<TrustedCandidate>,
    cancel: Arc<AtomicBool>,
    claimed: bool,
}
struct Registry {
    previews: HashMap<String, Snapshot>,
    confirmations: HashMap<String, CleanupConfirmation>,
    active: HashMap<String, Work>,
    recent: HashMap<String, CleanupOutcome>,
}
pub struct CleanupService {
    registry: Mutex<Registry>,
    journal: Arc<dyn CleanupJournal>,
    action: Arc<dyn ObjectAction>,
}
impl CleanupService {
    pub fn new(journal: Arc<dyn CleanupJournal>, action: Arc<dyn ObjectAction>) -> Self {
        Self {
            registry: Mutex::new(Registry {
                previews: HashMap::new(),
                confirmations: HashMap::new(),
                active: HashMap::new(),
                recent: HashMap::new(),
            }),
            journal,
            action,
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Registry>, ApiError> {
        self.registry
            .lock()
            .map_err(|_| err("CLEANUP_STATE", "Состояние очистки недоступно"))
    }
    pub fn availability(&self) -> CleanupAvailability {
        let persistent = self.journal.is_persistent();
        CleanupAvailability {
            execution_enabled: persistent && self.action.available(),
            persistent_journal: persistent,
            reason: if !persistent {
                "Постоянный журнал недоступен. Исполнение заблокировано."
            } else if !self.action.available() {
                "Безопасный Windows-адаптер Корзины ещё не допущен. Исполнение заблокировано."
            } else {
                "Нужны preview завершённого сканирования и явное подтверждение."
            }
            .into(),
        }
    }
    /// Rust-only ingestion. Scanner establishes the allowed root and captures
    /// object/ancestor identities at scan time, not after frontend selection.
    pub fn publish_preview(
        &self,
        objects: Vec<TrustedCandidate>,
    ) -> Result<CleanupPreview, ApiError> {
        self.publish_at(objects, now_ms())
    }
    fn publish_at(
        &self,
        objects: Vec<TrustedCandidate>,
        now: u64,
    ) -> Result<CleanupPreview, ApiError> {
        if objects.len() > MAX_CANDIDATES {
            return Err(err("LIMIT", "Preview превышает 1000 объектов"));
        }
        let mut ids = HashSet::new();
        if objects.iter().any(|c| {
            !ids.insert(c.object_id()) || now.saturating_sub(c.modified_unix_ms()) < MIN_AGE_MS
        }) {
            return Err(err(
                "INVALID_CANDIDATES",
                "Кандидаты не соответствуют правилу user-temp-v1",
            ));
        }
        let preview = CleanupPreview {
            preview_id: id(),
            category: "user-temp-v1".into(),
            rule_version: 1,
            expires_at_ms: now + PREVIEW_TTL_MS,
            candidates: objects
                .iter()
                .map(|c| CleanupCandidate {
                    object_id: c.object_id().into(),
                    display_path: c.display_path(),
                    size_bytes: c.size_bytes().to_string(),
                    reason: "Временный файл старше 7 суток; выбран сканером".into(),
                })
                .collect(),
        };
        let mut r = self.lock()?;
        r.previews
            .retain(|_, p| p.preview.expires_at_ms > now && !p.consumed);
        r.confirmations.retain(|_, c| c.expires_at_ms > now);
        if r.previews.len() >= 8 {
            return Err(err("LIMIT", "Слишком много активных preview"));
        }
        r.previews.insert(
            preview.preview_id.clone(),
            Snapshot {
                preview: preview.clone(),
                objects,
                consumed: false,
            },
        );
        Ok(preview)
    }
    pub fn latest_preview(&self) -> Result<Option<CleanupPreview>, ApiError> {
        Ok(self
            .lock()?
            .previews
            .values()
            .filter(|p| !p.consumed && p.preview.expires_at_ms > now_ms())
            .max_by_key(|p| p.preview.expires_at_ms)
            .map(|p| p.preview.clone()))
    }
    pub fn prepare_confirmation(
        &self,
        preview_id: &str,
        selected: Vec<String>,
    ) -> Result<CleanupConfirmation, ApiError> {
        self.confirm_at(preview_id, selected, now_ms())
    }
    fn confirm_at(
        &self,
        preview_id: &str,
        selected: Vec<String>,
        now: u64,
    ) -> Result<CleanupConfirmation, ApiError> {
        if selected.is_empty() || selected.len() > MAX_CANDIDATES {
            return Err(err("INVALID_SELECTION", "Выберите от 1 до 1000 объектов"));
        }
        let ids: HashSet<_> = selected.iter().collect();
        if ids.len() != selected.len() {
            return Err(err("INVALID_SELECTION", "Повторяющиеся ID"));
        }
        let mut r = self.lock()?;
        let p = r.previews.get(preview_id).ok_or_else(|| {
            err(
                "PREVIEW_NOT_FOUND",
                "Preview не найден; запустите сканирование",
            )
        })?;
        if p.consumed || p.preview.expires_at_ms <= now {
            return Err(err(
                "PREVIEW_EXPIRED",
                "Preview устарел или уже использован",
            ));
        }
        let chosen: Vec<_> = p
            .preview
            .candidates
            .iter()
            .filter(|c| ids.contains(&c.object_id))
            .cloned()
            .collect();
        if chosen.len() != ids.len() {
            return Err(err("INVALID_SELECTION", "Неизвестный объект"));
        }
        let bytes: u128 = chosen
            .iter()
            .map(|c| c.size_bytes.parse::<u128>().unwrap_or(0))
            .sum();
        let c = CleanupConfirmation { confirmation_id: id(), preview_id: preview_id.into(), expires_at_ms: (now + CONFIRM_TTL_MS).min(p.preview.expires_at_ms), selected: chosen, selected_bytes: bytes.to_string(),
            consequences: "Выбранные объекты будут повторно проверены и отправлены в Корзину. Восстановление и освобождение места не гарантируются. Отмена останавливает следующие объекты без отката завершённых.".into() };
        r.confirmations
            .retain(|_, old| old.preview_id != preview_id && old.expires_at_ms > now);
        r.confirmations.insert(c.confirmation_id.clone(), c.clone());
        Ok(c)
    }
    /// Persists all intents and consumes the confirmation BEFORE spawning work.
    pub fn begin(&self, token: &str, acknowledged: bool) -> Result<CleanupOutcome, ApiError> {
        if !acknowledged {
            return Err(err(
                "CONFIRMATION_REQUIRED",
                "Требуется явное подтверждение",
            ));
        }
        if !self.availability().execution_enabled {
            return Err(err("EXECUTION_DISABLED", &self.availability().reason));
        }
        let mut r = self.lock()?;
        if !r.active.is_empty() {
            return Err(err("BUSY", "Очистка уже выполняется"));
        }
        let c = r
            .confirmations
            .get(token)
            .ok_or_else(|| {
                err(
                    "CONFIRMATION_NOT_FOUND",
                    "Подтверждение отсутствует или уже использовано",
                )
            })?
            .clone();
        if c.expires_at_ms <= now_ms() {
            return Err(err("CONFIRMATION_EXPIRED", "Подтверждение устарело"));
        }
        let p = r
            .previews
            .get(&c.preview_id)
            .ok_or_else(|| err("PREVIEW_NOT_FOUND", "Preview не найден"))?;
        if p.consumed {
            return Err(err("PREVIEW_USED", "Preview уже использован"));
        }
        let chosen: HashSet<_> = c.selected.iter().map(|x| &x.object_id).collect();
        let objects: Vec<_> = p
            .objects
            .iter()
            .filter(|x| chosen.contains(&x.id))
            .cloned()
            .collect();
        let o = CleanupOutcome {
            operation_id: id(),
            preview_id: c.preview_id.clone(),
            category: p.preview.category.clone(),
            started_at_ms: now_ms(),
            status: CleanupOperationStatus::Running,
            recycled_bytes: "0".into(),
            journal_ok: true,
            items: c
                .selected
                .iter()
                .map(|x| CleanupItemResult {
                    object_id: x.object_id.clone(),
                    display_path: x.display_path.clone(),
                    size_bytes: x.size_bytes.clone(),
                    status: CleanupItemStatus::Pending,
                    reason_code: "INTENT".into(),
                })
                .collect(),
        };
        r.previews
            .get_mut(&c.preview_id)
            .expect("snapshot checked")
            .consumed = true;
        r.confirmations.remove(token);
        self.journal.begin(&o).map_err(|_| {
            err(
                "JOURNAL_UNAVAILABLE",
                "Намерение не записано. Файлы не изменены. Нужен новый preview.",
            )
        })?;
        r.active.insert(
            o.operation_id.clone(),
            Work {
                objects,
                cancel: Arc::new(AtomicBool::new(false)),
                claimed: false,
            },
        );
        r.recent.clear();
        r.recent.insert(o.operation_id.clone(), o.clone());
        Ok(o)
    }
    /// Exactly one worker may claim a registered operation.
    pub fn run(&self, operation_id: &str) -> Result<CleanupOutcome, ApiError> {
        let (objects, cancel, mut o) = {
            let mut r = self.lock()?;
            let work = r
                .active
                .get_mut(operation_id)
                .ok_or_else(|| err("OPERATION_NOT_FOUND", "Операция не найдена"))?;
            if work.claimed {
                return Err(err("OPERATION_CLAIMED", "Исполнитель уже запущен"));
            }
            work.claimed = true;
            let objects = std::mem::take(&mut work.objects);
            let cancel = work.cancel.clone();
            let o = r
                .recent
                .get(operation_id)
                .expect("registered operation")
                .clone();
            (objects, cancel, o)
        };
        for (index, object) in objects.iter().enumerate() {
            let result = if cancel.load(Ordering::Acquire) {
                None
            } else {
                // A crashing adapter must never cause a silent retry or continued batch.
                Some(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        self.action.apply(object)
                    }))
                    .unwrap_or(ActionResult::Unknown("ACTION_PANICKED".into())),
                )
            };
            (o.items[index].status, o.items[index].reason_code) = match result {
                None => (CleanupItemStatus::Cancelled, "CANCELLED".into()),
                Some(ActionResult::Recycled) => (CleanupItemStatus::Recycled, "RECYCLED".into()),
                Some(ActionResult::Skipped(code)) => (CleanupItemStatus::Skipped, code),
                Some(ActionResult::Failed(code)) => (CleanupItemStatus::Failed, code),
                Some(ActionResult::Unknown(code)) => (CleanupItemStatus::Unknown, code),
            };
            count_bytes(&mut o);
            if self.journal.update(&o).is_err() {
                o.items[index].status = CleanupItemStatus::Unknown;
                o.items[index].reason_code = "RESULT_NOT_PERSISTED".into();
                count_bytes(&mut o);
                o.journal_ok = false;
                o.status = CleanupOperationStatus::JournalFailed;
                break;
            }
            self.lock()?
                .recent
                .insert(o.operation_id.clone(), o.clone());
            if o.items[index].status == CleanupItemStatus::Unknown {
                o.status = CleanupOperationStatus::Interrupted;
                break;
            }
        }
        if o.status == CleanupOperationStatus::Running {
            o.status = if cancel.load(Ordering::Acquire) {
                CleanupOperationStatus::Cancelled
            } else {
                CleanupOperationStatus::Completed
            };
        }
        for item in &mut o.items {
            if item.status == CleanupItemStatus::Pending {
                item.status = CleanupItemStatus::Cancelled;
                item.reason_code = "STOPPED".into();
            }
        }
        if o.journal_ok && self.journal.update(&o).is_err() {
            o.journal_ok = false;
            o.status = CleanupOperationStatus::JournalFailed;
        }
        let mut r = self.lock()?;
        r.active.remove(operation_id);
        r.recent.insert(o.operation_id.clone(), o.clone());
        Ok(o)
    }
    pub fn cancel(&self, id: &str) -> Result<bool, ApiError> {
        if let Some(work) = self.lock()?.active.get(id) {
            work.cancel.store(true, Ordering::Release);
            return Ok(true);
        }
        Ok(false)
    }
    pub fn outcome(&self, id: &str) -> Result<Option<CleanupOutcome>, ApiError> {
        if let Some(o) = self.lock()?.recent.get(id) {
            return Ok(Some(o.clone()));
        }
        self.journal
            .read(id)
            .map_err(|_| err("JOURNAL_UNAVAILABLE", "Журнал недоступен"))
    }
}
fn count_bytes(o: &mut CleanupOutcome) {
    o.recycled_bytes = o
        .items
        .iter()
        .filter(|i| i.status == CleanupItemStatus::Recycled)
        .map(|i| i.size_bytes.parse::<u128>().unwrap_or(0))
        .sum::<u128>()
        .to_string();
}
#[cfg(test)]
mod tests;
