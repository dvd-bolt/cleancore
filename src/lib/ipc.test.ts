import { describe, it, expect, vi, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import {
  fetchAppInfo,
  fetchInitialMetrics,
  fetchMetricSnapshot,
  fetchProcessList,
  fetchAppSettings,
  saveRefreshInterval,
  togglePauseMetrics,
  pingBackend,
  startCleanupScan,
  getScanProgress,
  cancelCleanupScan,
  getCleanupPreview,
  fetchQuickDirectories,
  fetchResourceHistory,
} from "./ipc";

describe("IPC Bridge & Fallback behavior", () => {
  afterEach(() => { vi.unstubAllGlobals(); vi.clearAllMocks(); });

  it("propagates a native IPC error instead of silently returning browser preview data", async () => {
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    vi.mocked(invoke).mockRejectedValue({ code: "STORAGE_ERROR", message: "Database is read-only" });
    await expect(fetchAppSettings()).rejects.toThrow("Database is read-only");
    await expect(fetchMetricSnapshot()).rejects.toThrow("Database is read-only");
    await expect(fetchProcessList()).rejects.toThrow("Database is read-only");
  });
  it("provides safe fallback app info in non-Tauri web environment", async () => {
    const info = await fetchAppInfo();
    expect(info.name).toContain("CleanCore");
    expect(info.is_local_mode).toBe(true);
    expect(info.is_admin).toBeNull();
    expect(info.settings_persistent).toBe(false);
    expect(info.theme).toBe("studio-pearl");
  });

  it("provides unmeasured metric status initially without fake numbers", async () => {
    const metrics = await fetchInitialMetrics();
    expect(metrics.cpu.status).toBe("NotMeasured");
    expect(metrics.memory.status).toBe("NotMeasured");
    expect(metrics.disk.status).toBe("NotMeasured");
    expect(metrics.network.status).toBe("NotMeasured");

    const snapshot = await fetchMetricSnapshot();
    expect(snapshot.cpu.status).toBe("NotMeasured");
  });

  it("returns process snapshot structure safely", async () => {
    const snapshot = await fetchProcessList();
    expect(Array.isArray(snapshot.processes)).toBe(true);
    expect(typeof snapshot.timestamp_utc).toBe("string");
  });

  it("loads and saves settings with expected types", async () => {
    const settings = await fetchAppSettings();
    expect(settings.refresh_interval_ms).toBeGreaterThanOrEqual(500);
    expect(settings.theme).toBe("studio-pearl");

    const saved = await saveRefreshInterval(5000);
    expect(saved.refresh_interval_ms).toBe(5000);

    const paused = await togglePauseMetrics(true);
    expect(paused).toBe(true);
  });

  it("handles ping gracefully", async () => {
    const pong = await pingBackend();
    expect(typeof pong).toBe("string");
  });

  it("does not invent scanner tasks or file lists in browser preview", async () => {
    await expect(startCleanupScan()).rejects.toThrow("desktop");
    await expect(getScanProgress("unknown")).rejects.toThrow("desktop");
    await expect(cancelCleanupScan("unknown")).rejects.toThrow("desktop");
    await expect(getCleanupPreview("unknown")).rejects.toThrow("desktop");
  });

  it("fetches quick directories and resource history safely in browser preview", async () => {
    const dirs = await fetchQuickDirectories();
    expect(Array.isArray(dirs)).toBe(true);
    expect(dirs.length).toBeGreaterThan(0);
    expect(dirs[0]).toHaveProperty("path");

    const history = await fetchResourceHistory();
    expect(Array.isArray(history.cpu_percentages)).toBe(true);
    expect(history.cpu_percentages.length).toBeGreaterThan(0);
  });
});
