//! Wire contracts and the journal boundary shared by stages 3 and 4.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CleanupCandidate {
    pub object_id: String,
    pub display_path: String,
    /// Decimal string: no loss of integer precision in JavaScript.
    pub size_bytes: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CleanupPreview {
    pub preview_id: String,
    pub category: String,
    pub rule_version: u32,
    pub expires_at_ms: u64,
    pub candidates: Vec<CleanupCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CleanupConfirmation {
    pub confirmation_id: String,
    pub preview_id: String,
    pub expires_at_ms: u64,
    pub selected: Vec<CleanupCandidate>,
    pub selected_bytes: String,
    pub consequences: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CleanupItemStatus {
    Pending,
    Recycled,
    Skipped,
    Failed,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CleanupItemResult {
    pub object_id: String,
    pub display_path: String,
    pub size_bytes: String,
    pub status: CleanupItemStatus,
    pub reason_code: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CleanupOperationStatus {
    Running,
    Completed,
    Cancelled,
    Interrupted,
    JournalFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CleanupOutcome {
    pub operation_id: String,
    pub preview_id: String,
    pub category: String,
    pub started_at_ms: u64,
    pub status: CleanupOperationStatus,
    pub items: Vec<CleanupItemResult>,
    /// Bytes sent to Recycle Bin, not actual free disk space.
    pub recycled_bytes: String,
    pub journal_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupAvailability {
    pub execution_enabled: bool,
    pub persistent_journal: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupAuditPage {
    pub operations: Vec<CleanupOutcome>,
    pub next_cursor: Option<i64>,
}

/// begin must atomically persist ALL intents and reject an already used preview.
/// Every update must commit before returning. Implementations must not retry actions.
pub trait CleanupJournal: Send + Sync {
    fn is_persistent(&self) -> bool;
    fn begin(&self, operation: &CleanupOutcome) -> Result<(), String>;
    fn update(&self, operation: &CleanupOutcome) -> Result<(), String>;
    fn read(&self, operation_id: &str) -> Result<Option<CleanupOutcome>, String>;
}
