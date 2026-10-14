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

export type ActivePage = "overview" | "processes" | "disk" | "cleanup" | "service" | "journal" | "settings";

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

// --- Stage 6: Service Center («Центр обслуживания») types ---

export interface InstalledProgram {
  id: string;
  display_name: string;
  publisher: string | null;
  display_version: string | null;
  install_date: string | null;
  estimated_size_bytes: number | null;
  uninstall_string: string | null;
  quiet_uninstall_string: string | null;
  install_location: string | null;
  registry_hive: string;
  is_wow64: boolean;
  is_system_component: boolean;
}

export type StartupSource =
  | "registry_run"
  | "registry_run_once"
  | "startup_folder_user"
  | "startup_folder_common";

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  source: StartupSource;
  location: string;
  enabled: boolean;
  target_exists: boolean;
}

export type ProtectionState =
  | "protected"
  | "warning"
  | "disabled"
  | "limited_access"
  | "unknown";

export interface AntivirusProvider {
  name: string;
  product_exe: string | null;
  reporting_exe: string | null;
  state_code: number | null;
  real_time_protection: boolean;
  definitions_up_to_date: boolean;
}

export interface AntivirusStatus {
  state: ProtectionState;
  providers: AntivirusProvider[];
  defender_service_running: boolean | null;
  message: string;
}

export type HealthStatus = "healthy" | "warning" | "critical" | "unknown";

export interface DiagnosticAlert {
  id: string;
  level: HealthStatus;
  category: string;
  title: string;
  message: string;
  recommendation: string | null;
}

export interface DiagnosticsSummary {
  health: HealthStatus;
  cpu_usage_percent: number | null;
  memory_usage_percent: number | null;
  disk_free_bytes: number | null;
  disk_total_bytes: number | null;
  disk_usage_percent: number | null;
  system_drive: string;
  alerts: DiagnosticAlert[];
  recommendations: string[];
  timestamp_utc: string;
}

export type SystemToolId = "cleanmgr" | "dfrgui" | "resmon" | "taskmgr" | "appwiz";

export interface SystemTool {
  id: SystemToolId;
  name: string;
  description: string;
  executable: string;
  arguments: string[];
  is_available: boolean;
}

export interface LaunchResult {
  success: boolean;
  pid: number | null;
  message: string;
}

export type CsvExportSection = "programs" | "startup" | "diagnostics" | "all";

export interface CsvExportConfig {
  sections: CsvExportSection[];
  include_system_components: boolean;
  target_path: string | null;
}


