import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  AppInfo,
  AppSettings,
  CleanupPreviewPage,
  MetricSnapshot,
  ProcessSnapshot,
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
    version: "0.0.2",
    is_local_mode: true,
    is_admin: false,
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

export async function startCleanupScan(categoryId: string = "user-temp-v1"): Promise<string> {
  if (isTauriEnvironment()) {
    return await invokeCommand<string>("start_cleanup_scan", { categoryId });
  }

  return "mock-scan-task-1";
}

export async function getScanProgress(taskId: string): Promise<TaskStatus> {
  if (isTauriEnvironment()) {
    return await invokeCommand<TaskStatus>("get_scan_progress", { taskId });
  }

  return {
    id: taskId,
    kind: "scan:user-temp-v1",
    state: "Completed",
    processed_items: 42,
    total_items: 42,
  };
}

export async function cancelCleanupScan(taskId: string): Promise<boolean> {
  if (isTauriEnvironment()) {
    return await invokeCommand<boolean>("cancel_cleanup_scan", { taskId });
  }
  return true;
}

export async function getCleanupPreview(
  previewId: string,
  offset: number = 0,
  limit: number = 20,
): Promise<CleanupPreviewPage> {
  if (isTauriEnvironment()) {
    return await invokeCommand<CleanupPreviewPage>("get_cleanup_preview", {
      previewId,
      offset,
      limit,
    });
  }

  return {
    preview_id: previewId,
    category_id: "user-temp-v1",
    offset,
    limit,
    total_candidates: 2,
    total_bytes: 1450200,
    items: [
      {
        obj_id: "mock-1",
        display_path: "temp_cache_old_01.tmp",
        absolute_path: "C:\\Users\\Mock\\AppData\\Local\\Temp\\temp_cache_old_01.tmp",
        size_bytes: 1048576,
        category_id: "user-temp-v1",
        reason: "Файл не изменялся более 7 суток",
        modified_time_utc: "2026-10-01T10:00:00Z",
        risk_level: "Low",
      },
      {
        obj_id: "mock-2",
        display_path: "crash_log_dump_2026.log",
        absolute_path: "C:\\Users\\Mock\\AppData\\Local\\Temp\\crash_log_dump_2026.log",
        size_bytes: 401624,
        category_id: "user-temp-v1",
        reason: "Файл не изменялся более 7 суток",
        modified_time_utc: "2026-09-28T14:30:00Z",
        risk_level: "Low",
      },
    ],
  };
}
