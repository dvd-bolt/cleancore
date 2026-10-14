import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { AppInfo, AppSettings, MetricSnapshot, ProcessSnapshot } from "../types";

export const isTauriEnvironment = (): boolean => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

export async function fetchAppInfo(): Promise<AppInfo> {
  if (isTauriEnvironment()) {
    try {
      return await invoke<AppInfo>("get_app_info");
    } catch (e) {
      console.warn("Ошибка вызова get_app_info:", e);
    }
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
    try {
      return await invoke<MetricSnapshot>("get_metric_snapshot");
    } catch (e) {
      console.warn("Ошибка вызова get_metric_snapshot:", e);
    }
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
    try {
      return await invoke<ProcessSnapshot>("get_process_list");
    } catch (e) {
      console.warn("Ошибка вызова get_process_list:", e);
    }
  }

  return {
    timestamp_utc: new Date().toISOString(),
    processes: [],
  };
}

export async function fetchAppSettings(): Promise<AppSettings> {
  if (isTauriEnvironment()) {
    try {
      return await invoke<AppSettings>("get_app_settings");
    } catch (e) {
      console.warn("Ошибка вызова get_app_settings:", e);
    }
  }

  return {
    refresh_interval_ms: 2000,
    theme: "studio-pearl",
    is_monitoring_paused: false,
  };
}

export async function saveRefreshInterval(intervalMs: number): Promise<AppSettings> {
  if (isTauriEnvironment()) {
    return await invoke<AppSettings>("save_refresh_interval", { intervalMs });
  }

  return {
    refresh_interval_ms: intervalMs,
    theme: "studio-pearl",
    is_monitoring_paused: false,
  };
}

export async function togglePauseMetrics(paused: boolean): Promise<boolean> {
  if (isTauriEnvironment()) {
    return await invoke<boolean>("toggle_pause_metrics", { paused });
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
    console.warn("Не удалось подписаться на metrics-update:", e);
  }

  return () => {
    if (unlisten) {
      unlisten();
    }
  };
}

export async function pingBackend(): Promise<string> {
  if (isTauriEnvironment()) {
    return await invoke<string>("ping");
  }
  return "web-preview";
}
