import { invoke } from "@tauri-apps/api/core";
import { AppInfo, MetricSnapshot } from "../types";

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

  // Fallback для запуска только в браузере (pnpm dev)
  return {
    name: "CleanCore (Web preview)",
    version: "0.0.1",
    is_local_mode: true,
    is_admin: false,
    theme: "studio-pearl",
  };
}

export async function fetchInitialMetrics(): Promise<MetricSnapshot> {
  if (isTauriEnvironment()) {
    try {
      return await invoke<MetricSnapshot>("get_initial_metrics");
    } catch (e) {
      console.warn("Ошибка вызова get_initial_metrics:", e);
    }
  }

  // Fallback: честное состояние NotMeasured (без выдуманных цифр)
  return {
    timestamp_utc: new Date().toISOString(),
    interval_ms: 2000,
    cpu: { status: "NotMeasured" },
    memory: { status: "NotMeasured" },
    disk: { status: "NotMeasured" },
    network: { status: "NotMeasured" },
  };
}

export async function pingBackend(): Promise<string> {
  if (isTauriEnvironment()) {
    return await invoke<string>("ping");
  }
  return "web-preview";
}
