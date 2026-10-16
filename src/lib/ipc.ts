import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  AntivirusStatus,
  AppInfo,
  AppSettings,
  CleanupPreviewPage,
  CsvExportConfig,
  DiagnosticsSummary,
  DiskAnalysisReport,
  DiskTaskStatus,
  DuplicateScanResult,
  InstalledProgram,
  LaunchResult,
  MetricSnapshot,
  ProcessSnapshot,
  QuickDirectory,
  ResourceHistory,
  StartupItem,
  SystemToolId,
  TaskStatus,
  TreemapData,
  LargeFileCategory,
  LargeFileEntry,
  StaleFileEntry,
  RecycleBinInfo,
  DevCacheSummary,
  PowerProfileKind,
  PowerProfileStatus,
  SmartInsight,
} from "../types";

function normalizeIpcError(error: unknown): Error {
  if (error instanceof Error) return error;
  if (error && typeof error === "object" && "message" in error) {
    return new Error(String(error.message));
  }
  return new Error(String(error));
}

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try { return await invoke<T>(command, args); }
  catch (error) { throw normalizeIpcError(error); }
}

export const isTauriEnvironment = (): boolean => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

export async function chooseAnalysisFolder(): Promise<string | null> {
  if (!isTauriEnvironment()) return null;
  return invokeCommand<string | null>("choose_analysis_folder");
}

export async function fetchAppInfo(): Promise<AppInfo> {
  if (isTauriEnvironment()) {
    return await invokeCommand<AppInfo>("get_app_info");
  }

  // Fallback для запуска в браузере (pnpm dev)
  return {
    name: "CleanCore (Web preview)",
    version: "0.0.3",
    is_local_mode: true,
    is_admin: null,
    settings_persistent: false,
    theme: "studio-pearl",
  };
}

export async function fetchMetricSnapshot(): Promise<MetricSnapshot> {
  if (isTauriEnvironment()) {
    return await invokeCommand<MetricSnapshot>("get_metric_snapshot");
  }

  // Fallback: честное состояние NotMeasured
  return {
    timestamp_utc: new Date().toISOString(),
    interval_ms: 2000,
    cpu: { status: "NotMeasured" },
    memory: { status: "NotMeasured" },
    disk: { status: "NotMeasured" },
    network: { status: "NotMeasured" },
  };
}

export const fetchInitialMetrics = fetchMetricSnapshot;

export async function fetchProcessList(): Promise<ProcessSnapshot> {
  if (isTauriEnvironment()) {
    return await invokeCommand<ProcessSnapshot>("get_process_list");
  }

  return {
    timestamp_utc: new Date().toISOString(),
    processes: [],
  };
}

export async function fetchAppSettings(): Promise<AppSettings> {
  if (isTauriEnvironment()) {
    return await invokeCommand<AppSettings>("get_app_settings");
  }

  return {
    refresh_interval_ms: 2000,
    theme: "studio-pearl",
    is_monitoring_paused: false,
  };
}

export async function saveRefreshInterval(intervalMs: number): Promise<AppSettings> {
  if (isTauriEnvironment()) {
    return await invokeCommand<AppSettings>("save_refresh_interval", { intervalMs });
  }

  return {
    refresh_interval_ms: intervalMs,
    theme: "studio-pearl",
    is_monitoring_paused: false,
  };
}

export async function togglePauseMetrics(paused: boolean): Promise<boolean> {
  if (isTauriEnvironment()) {
    return await invokeCommand<boolean>("toggle_pause_metrics", { paused });
  }
  return paused;
}

export async function subscribeToMetrics(
  callback: (snapshot: MetricSnapshot) => void,
): Promise<() => void> {
  if (!isTauriEnvironment()) {
    return () => {};
  }

  let unlisten: UnlistenFn | undefined;
  try {
    unlisten = await listen<MetricSnapshot>("metrics-update", (event) => {
      callback(event.payload);
    });
  } catch (e) {
    throw normalizeIpcError(e);
  }

  return () => {
    if (unlisten) {
      unlisten();
    }
  };
}

export async function pingBackend(): Promise<string> {
  if (isTauriEnvironment()) {
    return await invokeCommand<string>("ping");
  }
  return "web-preview";
}

function requireDesktopScan(): void {
  if (!isTauriEnvironment()) throw new Error("Сканирование системы доступно только в desktop-приложении.");
}
export async function startCleanupScan(categoryId = "user-temp-v1", projectPath?: string): Promise<string> {
  requireDesktopScan(); return invokeCommand("start_cleanup_scan", { categoryId, projectPath: projectPath ?? null });
}
export async function resumeCleanupScan(taskId: string): Promise<string> {
  requireDesktopScan(); return invokeCommand("resume_cleanup_scan", { taskId });
}
export interface CleanupRuleInfo { id: string; name: string; description: string; min_age_days: number; risk_level: string; preview_only: boolean }
export async function getCleanupRules(): Promise<CleanupRuleInfo[]> {
  if (!isTauriEnvironment()) return [];
  return invokeCommand("get_cleanup_rules");
}
export async function getCleanupProfileContext(): Promise<string> {
  requireDesktopScan(); return invokeCommand("get_cleanup_profile_context");
}
export interface CleanupPreferences { excluded_paths: string[]; protected_duplicate_roots: string[] }
export async function getCleanupPreferences(): Promise<CleanupPreferences> {
  requireDesktopScan(); return invokeCommand("get_cleanup_preferences");
}
export async function saveCleanupPreferences(preferences: CleanupPreferences): Promise<void> {
  requireDesktopScan(); return invokeCommand("save_cleanup_preferences", { preferences });
}
export async function clearDuplicateHashCache(): Promise<void> {
  requireDesktopScan(); return invokeCommand("clear_duplicate_hash_cache");
}
export async function prepareDiskFileCleanup(reportId: string, entryId: string): Promise<import("../features/cleanup-execution/types").ExecutionPreview> {
  requireDesktopScan(); return invokeCommand("prepare_disk_file_cleanup", { reportId, entryId });
}
export async function prepareDuplicateCleanup(reportId: string, groupId: string, keeperIndex: number, selectedIndices: number[]): Promise<import("../features/cleanup-execution/types").ExecutionPreview> {
  requireDesktopScan(); return invokeCommand("prepare_duplicate_cleanup", { reportId, groupId, keeperIndex, selectedIndices });
}
export async function invalidateDiskReport(reportId: string): Promise<boolean> {
  requireDesktopScan(); return invokeCommand("invalidate_disk_report", { reportId });
}
export async function getScanProgress(taskId: string): Promise<TaskStatus> {
  requireDesktopScan(); return invokeCommand("get_scan_progress", { taskId });
}
export async function cancelCleanupScan(taskId: string): Promise<boolean> {
  requireDesktopScan(); return invokeCommand("cancel_cleanup_scan", { taskId });
}
export async function getCleanupPreview(previewId: string, offset = 0, limit = 20, query = ""): Promise<CleanupPreviewPage> {
  requireDesktopScan(); return invokeCommand("get_cleanup_preview", { previewId, offset, limit, query });
}

export async function fetchQuickDirectories(): Promise<QuickDirectory[]> {
  if (isTauriEnvironment()) {
    return await invokeCommand<QuickDirectory[]>("get_quick_directories");
  }
  return [];
}

export async function startDiskAnalysis(path: string): Promise<string> {
  if (isTauriEnvironment()) {
    return await invokeCommand<string>("start_disk_analysis", { path });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function getDiskAnalysisProgress(taskId: string): Promise<DiskTaskStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DiskTaskStatus>("get_disk_analysis_progress", { taskId });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function getDiskAnalysisReport(reportId: string, path?: string): Promise<DiskAnalysisReport> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DiskAnalysisReport>("get_disk_analysis_report", path ? { reportId, path } : { reportId });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function getDiskTreemap(reportId: string, path: string, sizeMode: "logical" | "allocated"): Promise<TreemapData> {
  requireDesktopScan();
  return invokeCommand("get_disk_treemap", { reportId, path, sizeMode });
}
export async function revealDiskEntry(reportId: string, entryId: string): Promise<void> {
  requireDesktopScan();
  return invokeCommand("reveal_disk_entry", { reportId, entryId });
}

export async function startDuplicateScan(path: string): Promise<string> {
  if (isTauriEnvironment()) {
    return await invokeCommand<string>("start_duplicate_scan", { path });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function getDuplicateScanProgress(taskId: string): Promise<DiskTaskStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DiskTaskStatus>("get_duplicate_scan_progress", { taskId });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function getDuplicateReport(reportId: string): Promise<DuplicateScanResult> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DuplicateScanResult>("get_duplicate_report", { reportId });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function cancelDiskTask(taskId: string): Promise<boolean> {
  if (isTauriEnvironment()) {
    return await invokeCommand<boolean>("cancel_disk_task", { taskId });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function fetchResourceHistory(): Promise<ResourceHistory> {
  if (isTauriEnvironment()) {
    return await invokeCommand<ResourceHistory>("get_resource_history");
  }
  return { timestamps_utc: [], cpu_percentages: [], memory_percentages: [], network_rx_bytes_sec: [], network_tx_bytes_sec: [] };
}

export async function fetchSpaceHogs(
  minSizeMb?: number,
  category?: LargeFileCategory
): Promise<LargeFileEntry[]> {
  if (isTauriEnvironment()) {
    return await invokeCommand<LargeFileEntry[]>("get_space_hogs", {
      minSizeMb: minSizeMb ?? 500,
      category: category ?? null,
    });
  }
  return [
    {
      id: "demo-1",
      path: "C:\\Users\\User\\Downloads\\Windows11_x64.iso",
      size_bytes: 5400000000,
      allocated_bytes: 5400000000,
      category: "DiskImage",
      extension: "iso",
      is_system_locked: false,
    },
    {
      id: "demo-2",
      path: "C:\\pagefile.sys",
      size_bytes: 8589934592,
      allocated_bytes: 8589934592,
      category: "Other",
      extension: "sys",
      is_system_locked: true,
    },
  ];
}

export async function revealPathInExplorer(path: string): Promise<void> {
  if (isTauriEnvironment()) {
    return await invokeCommand<void>("reveal_path_in_explorer", { path });
  }
}

export async function fetchStaleFiles(
  folderPath: string,
  olderThanDays: number,
  maxResults?: number
): Promise<StaleFileEntry[]> {
  if (isTauriEnvironment()) {
    return await invokeCommand<StaleFileEntry[]>("get_stale_files", {
      folderPath,
      olderThanDays,
      maxResults: maxResults ?? null,
    });
  }
  return [
    {
      path: `${folderPath}\\old_installer_v1.0.exe`,
      name: "old_installer_v1.0.exe",
      size_bytes: 420000000,
      last_modified_utc: "2023-01-15T12:00:00Z",
      days_old: 620,
      extension: "exe",
    },
    {
      path: `${folderPath}\\archived_project_2022.zip`,
      name: "archived_project_2022.zip",
      size_bytes: 850000000,
      last_modified_utc: "2022-11-20T08:30:00Z",
      days_old: 680,
      extension: "zip",
    },
  ];
}

export async function fetchRecycleBinInfo(): Promise<RecycleBinInfo> {
  if (isTauriEnvironment()) {
    return await invokeCommand<RecycleBinInfo>("query_recycle_bin");
  }
  return {
    total_bytes: 1420000000,
    item_count: 24,
  };
}

export async function emptyRecycleBin(): Promise<RecycleBinInfo> {
  if (isTauriEnvironment()) {
    return await invokeCommand<RecycleBinInfo>("empty_recycle_bin");
  }
  return {
    total_bytes: 1420000000,
    item_count: 24,
  };
}

export async function fetchDevCacheSummary(
  customRoots?: string[],
  inactiveDays?: number
): Promise<DevCacheSummary> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DevCacheSummary>("get_dev_cache_summary", {
      customRoots: customRoots ?? null,
      inactiveDays: inactiveDays ?? null,
    });
  }
  return {
    total_bytes: 8500000000,
    node_modules_count: 3,
    node_modules_bytes: 3200000000,
    target_count: 2,
    target_bytes: 2800000000,
    venv_count: 1,
    venv_bytes: 500000000,
    wsl_docker_bytes: 2000000000,
    entries: [
      {
        path: "C:\\Users\\User\\Projects\\old-web-app\\node_modules",
        name: "old-web-app/node_modules",
        kind: "node_modules",
        size_bytes: 1800000000,
        last_modified_utc: "2023-04-10T11:00:00Z",
        inactive_days: 550,
      },
      {
        path: "C:\\Users\\User\\source\\rust-tool\\target",
        name: "rust-tool/target",
        kind: "target",
        size_bytes: 2800000000,
        last_modified_utc: "2023-02-01T15:30:00Z",
        inactive_days: 620,
      },
      {
        path: "C:\\Users\\User\\AppData\\Local\\Docker\\wsl\\data\\ext4.vhdx",
        name: "Docker WSL Data (ext4.vhdx)",
        kind: "wsl_docker",
        size_bytes: 2000000000,
        last_modified_utc: "2023-06-01T09:00:00Z",
        inactive_days: 500,
      },
    ],
  };
}

export async function fetchPowerProfileState(): Promise<PowerProfileStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<PowerProfileStatus>("get_power_profile_state");
  }
  return {
    active_scheme: {
      guid: "381b4222-f694-41f0-9685-ff5bb260df2e",
      name: "Сбалансированная",
    },
    throttles: {
      max_ac: 100,
      min_ac: 5,
      max_dc: 100,
      min_dc: 5,
    },
    current_profile: "Balanced",
    has_rollback_snapshot: true,
    last_snapshot_time: "2026-10-16T12:00:00Z",
  };
}

export async function applyPowerProfile(profile: PowerProfileKind): Promise<PowerProfileStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<PowerProfileStatus>("apply_power_profile", { profile });
  }
  return {
    active_scheme: {
      guid: "381b4222-f694-41f0-9685-ff5bb260df2e",
      name: "Сбалансированная",
    },
    throttles: profile === "Quiet"
      ? { max_ac: 99, min_ac: 5, max_dc: 99, min_dc: 5 }
      : profile === "Performance"
      ? { max_ac: 100, min_ac: 100, max_dc: 100, min_dc: 5 }
      : { max_ac: 100, min_ac: 5, max_dc: 100, min_dc: 5 },
    current_profile: profile,
    has_rollback_snapshot: true,
    last_snapshot_time: "2026-10-16T12:00:00Z",
  };
}

export async function rollbackPowerProfile(): Promise<PowerProfileStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<PowerProfileStatus>("rollback_power_profile");
  }
  return {
    active_scheme: {
      guid: "381b4222-f694-41f0-9685-ff5bb260df2e",
      name: "Сбалансированная",
    },
    throttles: {
      max_ac: 100,
      min_ac: 5,
      max_dc: 100,
      min_dc: 5,
    },
    current_profile: "Balanced",
    has_rollback_snapshot: false,
    last_snapshot_time: null,
  };
}

// --- Stage 6: Service Center («Центр обслуживания») IPC Wrappers ---

export async function getServicePrograms(): Promise<InstalledProgram[]> {
  if (isTauriEnvironment()) {
    return await invokeCommand<InstalledProgram[]>("list_installed_programs");
  }
  return [];
}

export async function launchServiceUninstaller(programId: string): Promise<LaunchResult> {
  if (isTauriEnvironment()) {
    return await invokeCommand<LaunchResult>("launch_uninstaller_with_residue_context", { programId, acknowledged: true });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function getServiceStartup(): Promise<StartupItem[]> {
  if (isTauriEnvironment()) {
    return await invokeCommand<StartupItem[]>("list_startup_items");
  }
  return [];
}

export async function getServiceAntivirus(): Promise<AntivirusStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<AntivirusStatus>("get_antivirus_status");
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function getServiceDiagnostics(): Promise<DiagnosticsSummary> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DiagnosticsSummary>("get_system_diagnostics");
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function launchServiceTool(toolId: SystemToolId): Promise<LaunchResult> {
  if (isTauriEnvironment()) {
    return await invokeCommand<LaunchResult>("launch_system_tool", { toolId });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}

export async function exportServiceCsv(config: CsvExportConfig): Promise<string> {
  if (isTauriEnvironment()) {
    return await invokeCommand<string>("generate_csv_report", { config });
  }
  throw new Error("Данные и действия доступны только в desktop-приложении CleanCore.");
}



export async function saveServiceCsv(config: CsvExportConfig): Promise<string | null> {
  if (!isTauriEnvironment()) throw new Error("Сохранение доступно только в desktop-приложении.");
  return invokeCommand<string | null>("save_csv_report", { config: { ...config, target_path: null } });
}

export interface StorageResilienceStatus {
  db_path: string; settings_path: string; schema_version: number;
  database_persistent: boolean; settings_persistent: boolean;
  db_recovered: boolean; settings_recovered: boolean;
  db_recovery_notice: { quarantined_path: string; reason: string } | null;
  settings_recovery_notice: { quarantined_path: string; reason: string } | null;
}
export async function fetchStorageResilience(): Promise<StorageResilienceStatus | null> {
  return isTauriEnvironment() ? invokeCommand<StorageResilienceStatus>("get_storage_resilience_status") : null;
}

export async function exportCleanupJournal(): Promise<string | null> {
  if (!isTauriEnvironment()) throw new Error("Журнал доступен только в desktop-приложении.");
  return invokeCommand<string | null>("export_cleanup_journal");
}


export async function setStartupEnabled(itemId:string,expectedCommand:string,enabled:boolean):Promise<void> {
  if(!isTauriEnvironment()) throw new Error("Изменение автозагрузки доступно в приложении Windows");
  return invokeCommand<void>("set_startup_enabled",{itemId,expectedCommand,enabled,acknowledged:true});
}

export async function inspectAppResidue(contextId: string): Promise<import("../types").AppResidueReport> {
  requireDesktopScan(); return invokeCommand("inspect_app_residue", { contextId });
}

export async function fetchSmartInsights(params?: {
  processes?: string[];
  cpuUsage?: number;
  memoryUsage?: number;
  diskFreeBytes?: number;
  diskTotalBytes?: number;
}): Promise<SmartInsight[]> {
  if (isTauriEnvironment()) {
    return await invokeCommand<SmartInsight[]>("get_smart_insights", {
      processes: params?.processes,
      cpuUsage: params?.cpuUsage,
      memoryUsage: params?.memoryUsage,
      diskFreeBytes: params?.diskFreeBytes,
      diskTotalBytes: params?.diskTotalBytes,
    });
  }
  return [
    {
      id: "all_systems_optimal",
      title: "Система в оптимальном состоянии",
      description: "Свободное место в норме, системная Корзина не переполнена, параметры электропитания сбалансированы.",
      category: "system",
      level: "info",
      estimatedSavings: null,
      actionLabel: "Центр обслуживания",
      targetPage: "service",
      targetTab: null,
    },
  ];
}

