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
  is_admin: boolean | null;
  settings_persistent: boolean;
  theme: string;
}

export interface ApiError {
  code: string;
  message: string;
}

export type TaskState = "Pending" | "Running" | "Completed" | "Partial" | "Cancelled" | "Failed";

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
  size_bytes: string;
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
  total_bytes: string;
  items: CleanupCandidate[];
  unfiltered_candidates: number;
  rule_version: string;
  complete: boolean;
  cancelled: boolean;
  diagnostics: Record<string, number>;
  execution_preview_id: string | null;
}

export type ActivePage = "overview" | "processes" | "disk" | "cleanup" | "journal" | "settings";

export type FileCategory =
  | "Videos"
  | "Images"
  | "Documents"
  | "Archives"
  | "Audio"
  | "Code"
  | "Other";

export interface DiskEntry {
  path: string;
  name: string;
  size_bytes: number;
  is_dir: boolean;
  file_count: number;
  percentage: number;
  category: FileCategory;
}

export interface CategorySummary {
  category: FileCategory;
  display_name: string;
  total_bytes: number;
  file_count: number;
  percentage: number;
}

export interface DiskAnalysisReport {
  root_path: string;
  total_bytes: number;
  total_files: number;
  total_dirs: number;
  categories: CategorySummary[];
  largest_files: DiskEntry[];
  largest_dirs: DiskEntry[];
  subdirs: DiskEntry[];
}

export interface DuplicateFileEntry {
  path: string;
  modified_utc: string;
  size_bytes: number;
}

export interface DuplicateGroup {
  group_id: string;
  file_size_bytes: number;
  full_hash: string;
  files: DuplicateFileEntry[];
  reclaimable_bytes: number;
}

export interface DuplicateScanResult {
  root_path: string;
  groups: DuplicateGroup[];
  total_wasted_bytes: number;
  total_duplicate_files: number;
}

export interface QuickDirectory {
  id: string;
  name: string;
  path: string;
}

export interface ResourceHistory {
  timestamps_utc: string[];
  cpu_percentages: number[];
  memory_percentages: number[];
  network_rx_bytes_sec: number[];
  network_tx_bytes_sec: number[];
}

export type DiskTaskState = "Running" | "Completed" | "Cancelled" | "Failed";

export interface DiskTaskStatus {
  task_id: string;
  kind: string;
  state: DiskTaskState;
  progress_message: string;
  scanned_items: number;
  scanned_bytes: number;
  error_message?: string | null;
}

