export type FieldStatus = "Available" | "NotMeasured" | "AccessDenied" | "Unavailable";

export interface FieldValue<T> {
  status: FieldStatus;
  value?: T;
}

export interface CpuMetrics {
  total_usage_percent: number;
  core_count: number;
  frequency_mhz?: number;
}

export interface MemoryMetrics {
  used_bytes: number;
  total_bytes: number;
  usage_percent: number;
}

export interface DiskMetrics {
  mount_point: string;
  free_bytes: number;
  total_bytes: number;
  usage_percent: number;
}

export interface NetworkMetrics {
  rx_bytes_per_sec: number;
  tx_bytes_per_sec: number;
  interface_name?: string;
}

export interface MetricSnapshot {
  timestamp_utc: string;
  interval_ms: number;
  cpu: FieldValue<CpuMetrics>;
  memory: FieldValue<MemoryMetrics>;
  disk: FieldValue<DiskMetrics>;
  network: FieldValue<NetworkMetrics>;
}

export interface ProcessItem {
  pid: number;
  name: string;
  cpu_percent: FieldValue<number>;
  memory_bytes: FieldValue<number>;
  executable_path: FieldValue<string>;
}

export interface ProcessSnapshot {
  timestamp_utc: string;
  processes: ProcessItem[];
}

export interface AppSettings {
  refresh_interval_ms: number;
  theme: string;
  is_monitoring_paused: boolean;
}

export interface AppInfo {
  name: string;
  version: string;
  is_local_mode: boolean;
  is_admin: boolean;
  theme: string;
}

export interface ApiError {
  code: string;
  message: string;
}

export type TaskState = "Pending" | "Running" | "Completed" | "Cancelled" | "Failed";

export interface TaskStatus {
  id: string;
  kind: string;
  state: TaskState;
  processed_items: number;
  total_items?: number | null;
  error_message?: string | null;
}

export type RiskLevel = "Low" | "Medium" | "High" | "Critical";

export interface CleanupCandidate {
  obj_id: string;
  display_path: string;
  absolute_path: string;
  size_bytes: number;
  category_id: string;
  reason: string;
  modified_time_utc: string;
  risk_level: RiskLevel;
}

export interface CleanupPreviewPage {
  preview_id: string;
  category_id: string;
  offset: number;
  limit: number;
  total_candidates: number;
  total_bytes: number;
  items: CleanupCandidate[];
}

export type ActivePage = "overview" | "processes" | "cleanup" | "journal" | "settings";
