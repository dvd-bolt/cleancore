export interface ExecutionCandidate {
  object_id: string; display_path: string; size_bytes: string; reason: string;
}
export interface ExecutionPreview {
  preview_id: string; category: string; rule_version: number; expires_at_ms: number;
  candidates: ExecutionCandidate[];
}
export interface CleanupConfirmation {
  confirmation_id: string; preview_id: string; expires_at_ms: number;
  selected: ExecutionCandidate[]; selected_bytes: string; consequences: string;
}
export type ItemStatus = "Pending" | "Recycled" | "Skipped" | "Failed" | "Cancelled" | "Unknown";
export type OperationStatus = "Running" | "Completed" | "Cancelled" | "Interrupted" | "JournalFailed";
export interface CleanupOutcome {
  operation_id: string; preview_id: string; category: string; started_at_ms: number;
  status: OperationStatus; items: Array<Omit<ExecutionCandidate, "reason"> & { status: ItemStatus; reason_code: string }>;
  recycled_bytes: string; journal_ok: boolean;
}
export interface CleanupAvailability {
  execution_enabled: boolean; persistent_journal: boolean; reason: string;
}
export interface AuditPage { operations: CleanupOutcome[]; next_cursor: number | null }
