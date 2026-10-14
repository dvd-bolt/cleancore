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

export type ActivePage = "overview" | "processes" | "cleanup" | "journal" | "settings";
