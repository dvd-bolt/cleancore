export interface ModelInfo { id: string; name: string; size_bytes: string; architecture: string; declared_license: string | null; license_source: string | null; license_accepted: boolean }
export interface Recommendation { label: string; page: "processes" | "disk" | "cleanup" | "journal" }
export interface AssistantTask { id: string; state: "running" | "completed" | "cancelled" | "failed"; answer: string | null; error: string | null; recommendations: Recommendation[] }
export interface AssistantStatus { enabled: boolean; runtime_present: boolean; model: ModelInfo | null; task: AssistantTask | null }
export interface ContextSelection { metrics: boolean; preview_id: string | null; journal_ids: string[] }
export interface ContextSummary {
  metrics: { cpu_percent: number | null; memory_percent: number | null; disk_free_bytes: string | null } | null;
  scan: { category: string; candidates: number; bytes: string; complete: boolean } | null;
  journal: { status: string; item_count: number; unknown_count: number; failed_count: number }[];
}
export interface PreparedContext { id: string; summary: ContextSummary; recommendations: Recommendation[] }
