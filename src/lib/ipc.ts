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
export async function startCleanupScan(categoryId = "user-temp-v1"): Promise<string> {
  requireDesktopScan(); return invokeCommand("start_cleanup_scan", { categoryId });
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
  return [
    { id: "profile", name: "Профиль пользователя", path: "C:\\Users\\User" },
    { id: "downloads", name: "Загрузки", path: "C:\\Users\\User\\Downloads" },
    { id: "documents", name: "Документы", path: "C:\\Users\\User\\Documents" },
    { id: "desktop", name: "Рабочий стол", path: "C:\\Users\\User\\Desktop" },
    { id: "pictures", name: "Изображения", path: "C:\\Users\\User\\Pictures" },
    { id: "videos", name: "Видео", path: "C:\\Users\\User\\Videos" },
  ];
}

export async function startDiskAnalysis(path: string): Promise<string> {
  if (isTauriEnvironment()) {
    return await invokeCommand<string>("start_disk_analysis", { path });
  }
  return "demo-disk-task-1";
}

export async function getDiskAnalysisProgress(taskId: string): Promise<DiskTaskStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DiskTaskStatus>("get_disk_analysis_progress", { taskId });
  }
  return {
    task_id: taskId,
    kind: "disk_analysis",
    state: "Completed",
    progress_message: "Анализ успешно завершён",
    scanned_items: 1420,
    scanned_bytes: 54_230_000_000,
    error_message: null,
  };
}

export async function getDiskAnalysisReport(reportId: string): Promise<DiskAnalysisReport> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DiskAnalysisReport>("get_disk_analysis_report", { reportId });
  }
  return {
    root_path: "C:\\Users\\User",
    total_bytes: 54_230_000_000,
    total_files: 1420,
    total_dirs: 85,
    categories: [
      { category: "Videos", display_name: "Видео", total_bytes: 25_000_000_000, file_count: 14, percentage: 46.1 },
      { category: "Images", display_name: "Изображения", total_bytes: 12_000_000_000, file_count: 530, percentage: 22.1 },
      { category: "Archives", display_name: "Архивы и образы", total_bytes: 8_500_000_000, file_count: 22, percentage: 15.7 },
      { category: "Documents", display_name: "Документы", total_bytes: 4_200_000_000, file_count: 410, percentage: 7.7 },
      { category: "Code", display_name: "Код и разметка", total_bytes: 2_530_000_000, file_count: 400, percentage: 4.7 },
      { category: "Other", display_name: "Прочее", total_bytes: 2_000_000_000, file_count: 44, percentage: 3.7 },
    ],
    largest_files: [
      { path: "C:\\Users\\User\\Videos\\screencast.mp4", name: "screencast.mp4", size_bytes: 8_400_000_000, is_dir: false, file_count: 1, percentage: 15.5, category: "Videos" },
      { path: "C:\\Users\\User\\Downloads\\archive.iso", name: "archive.iso", size_bytes: 5_200_000_000, is_dir: false, file_count: 1, percentage: 9.6, category: "Archives" },
      { path: "C:\\Users\\User\\Pictures\\raw_shoot.dng", name: "raw_shoot.dng", size_bytes: 2_100_000_000, is_dir: false, file_count: 1, percentage: 3.9, category: "Images" },
    ],
    largest_dirs: [
      { path: "C:\\Users\\User\\Videos", name: "Videos", size_bytes: 25_000_000_000, is_dir: true, file_count: 14, percentage: 46.1, category: "Videos" },
      { path: "C:\\Users\\User\\Downloads", name: "Downloads", size_bytes: 14_000_000_000, is_dir: true, file_count: 45, percentage: 25.8, category: "Other" },
      { path: "C:\\Users\\User\\Pictures", name: "Pictures", size_bytes: 12_000_000_000, is_dir: true, file_count: 530, percentage: 22.1, category: "Images" },
    ],
    subdirs: [
      { path: "C:\\Users\\User\\Videos", name: "Videos", size_bytes: 25_000_000_000, is_dir: true, file_count: 14, percentage: 46.1, category: "Videos" },
      { path: "C:\\Users\\User\\Downloads", name: "Downloads", size_bytes: 14_000_000_000, is_dir: true, file_count: 45, percentage: 25.8, category: "Other" },
      { path: "C:\\Users\\User\\Pictures", name: "Pictures", size_bytes: 12_000_000_000, is_dir: true, file_count: 530, percentage: 22.1, category: "Images" },
      { path: "C:\\Users\\User\\Documents", name: "Documents", size_bytes: 3_230_000_000, is_dir: true, file_count: 831, percentage: 6.0, category: "Documents" },
    ],
  };
}

export async function startDuplicateScan(path: string): Promise<string> {
  if (isTauriEnvironment()) {
    return await invokeCommand<string>("start_duplicate_scan", { path });
  }
  return "demo-dup-task-1";
}

export async function getDuplicateScanProgress(taskId: string): Promise<DiskTaskStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DiskTaskStatus>("get_duplicate_scan_progress", { taskId });
  }
  return {
    task_id: taskId,
    kind: "duplicate_scan",
    state: "Completed",
    progress_message: "Поиск дубликатов успешно завершён",
    scanned_items: 2,
    scanned_bytes: 1_200_000_000,
    error_message: null,
  };
}

export async function getDuplicateReport(reportId: string): Promise<DuplicateScanResult> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DuplicateScanResult>("get_duplicate_report", { reportId });
  }
  return {
    root_path: "C:\\Users\\User",
    groups: [
      {
        group_id: "demo-group-1",
        file_size_bytes: 600_000_000,
        full_hash: "a4f891b2c3d4e5f678901234567890abcdef1234567890abcdef1234567890ab",
        reclaimable_bytes: 600_000_000,
        files: [
          { path: "C:\\Users\\User\\Downloads\\backup.tar.gz", modified_utc: "2026-10-10T12:00:00Z", size_bytes: 600_000_000 },
          { path: "C:\\Users\\User\\Documents\\old_backup.tar.gz", modified_utc: "2026-10-10T12:00:00Z", size_bytes: 600_000_000 },
        ],
      },
      {
        group_id: "demo-group-2",
        file_size_bytes: 300_000_000,
        full_hash: "e5d6c7b8a9012345678901234567890abcdef1234567890abcdef1234567890cd",
        reclaimable_bytes: 600_000_000,
        files: [
          { path: "C:\\Users\\User\\Pictures\\wallpaper.png", modified_utc: "2026-09-01T08:30:00Z", size_bytes: 300_000_000 },
          { path: "C:\\Users\\User\\Desktop\\wallpaper.png", modified_utc: "2026-09-01T08:30:00Z", size_bytes: 300_000_000 },
          { path: "C:\\Users\\User\\Downloads\\wallpaper(1).png", modified_utc: "2026-09-01T08:30:00Z", size_bytes: 300_000_000 },
        ],
      },
    ],
    total_wasted_bytes: 1_200_000_000,
    total_duplicate_files: 5,
  };
}

export async function cancelDiskTask(taskId: string): Promise<boolean> {
  if (isTauriEnvironment()) {
    return await invokeCommand<boolean>("cancel_disk_task", { taskId });
  }
  return true;
}

export async function fetchResourceHistory(): Promise<ResourceHistory> {
  if (isTauriEnvironment()) {
    return await invokeCommand<ResourceHistory>("get_resource_history");
  }
  const now = Date.now();
  const timestamps_utc: string[] = [];
  const cpu_percentages: number[] = [];
  const memory_percentages: number[] = [];
  const network_rx_bytes_sec: number[] = [];
  const network_tx_bytes_sec: number[] = [];

  for (let i = 29; i >= 0; i--) {
    const t = new Date(now - i * 2000).toISOString();
    timestamps_utc.push(t);
    cpu_percentages.push(10 + Math.sin(i / 3) * 8 + (i % 5));
    memory_percentages.push(42 + Math.cos(i / 4) * 2);
    network_rx_bytes_sec.push(1024 * (50 + (i % 20) * 10));
    network_tx_bytes_sec.push(1024 * (20 + (i % 10) * 5));
  }

  return {
    timestamps_utc,
    cpu_percentages,
    memory_percentages,
    network_rx_bytes_sec,
    network_tx_bytes_sec,
  };
}

// --- Stage 6: Service Center («Центр обслуживания») IPC Wrappers ---

export async function getServicePrograms(): Promise<InstalledProgram[]> {
  if (isTauriEnvironment()) {
    return await invokeCommand<InstalledProgram[]>("list_installed_programs");
  }

  return [
    {
      id: "HKLM_64_7Zip",
      display_name: "7-Zip 24.08 (x64)",
      publisher: "Igor Pavlov",
      display_version: "24.08",
      install_date: "20260115",
      estimated_size_bytes: 5242880,
      uninstall_string: "C:\\Program Files\\7-Zip\\Uninstall.exe",
      quiet_uninstall_string: null,
      install_location: "C:\\Program Files\\7-Zip",
      registry_hive: "HKLM",
      is_wow64: false,
      is_system_component: false,
    },
    {
      id: "HKCU_64_VSCode",
      display_name: "Microsoft Visual Studio Code",
      publisher: "Microsoft Corporation",
      display_version: "1.95.0",
      install_date: "20260210",
      estimated_size_bytes: 380000000,
      uninstall_string: "C:\\Users\\User\\AppData\\Local\\Programs\\Microsoft VS Code\\unins000.exe",
      quiet_uninstall_string: null,
      install_location: "C:\\Users\\User\\AppData\\Local\\Programs\\Microsoft VS Code",
      registry_hive: "HKCU",
      is_wow64: false,
      is_system_component: false,
    },
    {
      id: "HKLM_32_Git",
      display_name: "Git version 2.47.0",
      publisher: "The Git Development Community",
      display_version: "2.47.0",
      install_date: "20260120",
      estimated_size_bytes: 320000000,
      uninstall_string: "C:\\Program Files\\Git\\unins000.exe",
      quiet_uninstall_string: null,
      install_location: "C:\\Program Files\\Git",
      registry_hive: "HKLM",
      is_wow64: true,
      is_system_component: false,
    },
  ];
}

export async function launchServiceUninstaller(programId: string): Promise<LaunchResult> {
  if (isTauriEnvironment()) {
    return await invokeCommand<LaunchResult>("launch_uninstaller", { programId });
  }

  return {
    success: true,
    pid: 1234,
    message: `Штатный деинсталлятор для программы '${programId}' запущен (PID: 1234).`,
  };
}

export async function getServiceStartup(): Promise<StartupItem[]> {
  if (isTauriEnvironment()) {
    return await invokeCommand<StartupItem[]>("list_startup_items");
  }

  return [
    {
      id: "HKCU_Run_OneDrive",
      name: "OneDrive",
      command: "\"C:\\Users\\User\\AppData\\Local\\Microsoft\\OneDrive\\OneDrive.exe\" /background",
      source: "registry_run",
      location: "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
      enabled: true,
      target_exists: true,
    },
    {
      id: "HKLM_Run_SecurityHealth",
      name: "SecurityHealth",
      command: "%windir%\\system32\\SecurityHealthSystray.exe",
      source: "registry_run",
      location: "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
      enabled: true,
      target_exists: true,
    },
    {
      id: "User_Startup_CleanCoreHelper",
      name: "CleanCoreHelper.lnk",
      command: "C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\CleanCoreHelper.lnk",
      source: "startup_folder_user",
      location: "C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup",
      enabled: true,
      target_exists: false,
    },
  ];
}

export async function getServiceAntivirus(): Promise<AntivirusStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<AntivirusStatus>("get_antivirus_status");
  }

  return {
    state: "protected",
    providers: [
      {
        name: "Windows Defender",
        product_exe: "C:\\Program Files\\Windows Defender\\MsMpEng.exe",
        reporting_exe: null,
        state_code: 397568,
        real_time_protection: true,
        definitions_up_to_date: true,
      },
    ],
    defender_service_running: true,
    message: "Защита активна: Windows Defender включен, сигнатуры актуальны.",
  };
}

export async function getServiceDiagnostics(): Promise<DiagnosticsSummary> {
  if (isTauriEnvironment()) {
    return await invokeCommand<DiagnosticsSummary>("get_system_diagnostics");
  }

  return {
    health: "healthy",
    cpu_usage_percent: 18.5,
    memory_usage_percent: 54,
    disk_free_bytes: 48_500_000_000,
    disk_total_bytes: 256_000_000_000,
    disk_usage_percent: 81,
    system_drive: "C:",
    alerts: [
      {
        id: "diag_disk_ok",
        level: "healthy",
        category: "disk",
        title: "Свободное место на системном диске",
        message: "На диске C: свободно 48.5 ГБ (19%).",
        recommendation: null,
      },
    ],
    recommendations: [
      "Система функционирует штатно, критических предупреждений нет.",
    ],
    timestamp_utc: new Date().toISOString(),
  };
}

export async function launchServiceTool(toolId: SystemToolId): Promise<LaunchResult> {
  if (isTauriEnvironment()) {
    return await invokeCommand<LaunchResult>("launch_system_tool", { toolId });
  }

  return {
    success: true,
    pid: 5678,
    message: `Штатная системная утилита '${toolId}' успешно запущена (PID: 5678).`,
  };
}

export async function exportServiceCsv(config: CsvExportConfig): Promise<string> {
  if (isTauriEnvironment()) {
    return await invokeCommand<string>("generate_csv_report", { config });
  }

  return (
    "\uFEFF\"ОТЧЁТ ЦЕНТРА ОБСЛУЖИВАНИЯ CLEANCORE\"\r\n\"Дата формирования (UTC)\",\"" +
    new Date().toISOString() +
    "\"\r\n\r\n\"[РАЗДЕЛ: ДИАГНОСТИКА И ЗДОРОВЬЕ СИСТЕМЫ]\"\r\n\"Общее состояние здоровья\",\"Штатное (Healthy)\"\r\n"
  );
}


