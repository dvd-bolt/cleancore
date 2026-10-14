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
  getServicePrograms,
  launchServiceUninstaller,
  getServiceStartup,
  getServiceAntivirus,
  getServiceDiagnostics,
  launchServiceTool,
  exportServiceCsv,
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

  it("provides realistic browser preview data for Stage 6 Service Center APIs", async () => {
    const progs = await getServicePrograms();
    expect(Array.isArray(progs)).toBe(true);
    expect(progs.length).toBeGreaterThan(0);
    expect(progs[0]).toHaveProperty("display_name");
    expect(progs[0]).toHaveProperty("id");
    expect(progs[0]).toHaveProperty("registry_hive");

    const uninstall = await launchServiceUninstaller("HKLM_64_7Zip");
    expect(uninstall.success).toBe(true);
    expect(uninstall.pid).toBe(1234);

    const startup = await getServiceStartup();
    expect(Array.isArray(startup)).toBe(true);
    expect(startup.length).toBeGreaterThan(0);
    expect(startup[0]).toHaveProperty("source");
    expect(startup[0]).toHaveProperty("command");

    const av = await getServiceAntivirus();
    expect(av.state).toBe("protected");
    expect(Array.isArray(av.providers)).toBe(true);
    expect(av.providers.length).toBeGreaterThan(0);

    const diag = await getServiceDiagnostics();
    expect(diag.health).toBe("healthy");
    expect(diag.system_drive).toBe("C:");
    expect(Array.isArray(diag.alerts)).toBe(true);
    expect(Array.isArray(diag.recommendations)).toBe(true);

    const tool = await launchServiceTool("cleanmgr");
    expect(tool.success).toBe(true);
    expect(tool.pid).toBe(5678);

    const csv = await exportServiceCsv({
      sections: ["programs"],
      include_system_components: false,
      target_path: null,
    });
    expect(typeof csv).toBe("string");
    expect(csv).toContain("ОТЧЁТ ЦЕНТРА ОБСЛУЖИВАНИЯ CLEANCORE");
  });

  it("invokes native Tauri commands and propagates errors for Stage 6 Service Center APIs", async () => {
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });

    vi.mocked(invoke).mockResolvedValueOnce([{ id: "p1", display_name: "App 1" }]);
    const progs = await getServicePrograms();
    expect(invoke).toHaveBeenCalledWith("list_installed_programs", undefined);
    expect(progs).toEqual([{ id: "p1", display_name: "App 1" }]);

    vi.mocked(invoke).mockResolvedValueOnce({ success: true, pid: 999, message: "OK" });
    const uninst = await launchServiceUninstaller("p1");
    expect(invoke).toHaveBeenCalledWith("launch_uninstaller", { programId: "p1" });
    expect(uninst.pid).toBe(999);

    vi.mocked(invoke).mockResolvedValueOnce([{ id: "s1", name: "Startup 1" }]);
    const startup = await getServiceStartup();
    expect(invoke).toHaveBeenCalledWith("list_startup_items", undefined);
    expect(startup).toEqual([{ id: "s1", name: "Startup 1" }]);

    vi.mocked(invoke).mockResolvedValueOnce({ state: "warning", providers: [] });
    const av = await getServiceAntivirus();
    expect(invoke).toHaveBeenCalledWith("get_antivirus_status", undefined);
    expect(av.state).toBe("warning");

    vi.mocked(invoke).mockResolvedValueOnce({ health: "critical", system_drive: "C:" });
    const diag = await getServiceDiagnostics();
    expect(invoke).toHaveBeenCalledWith("get_system_diagnostics", undefined);
    expect(diag.health).toBe("critical");

    vi.mocked(invoke).mockResolvedValueOnce({ success: true, pid: 888, message: "Tool launched" });
    const tool = await launchServiceTool("taskmgr");
    expect(invoke).toHaveBeenCalledWith("launch_system_tool", { toolId: "taskmgr" });
    expect(tool.pid).toBe(888);

    vi.mocked(invoke).mockResolvedValueOnce("csv-output-data");
    const csv = await exportServiceCsv({
      sections: ["all"],
      include_system_components: true,
      target_path: "C:\\report.csv",
    });
    expect(invoke).toHaveBeenCalledWith("generate_csv_report", {
      config: {
        sections: ["all"],
        include_system_components: true,
        target_path: "C:\\report.csv",
      },
    });
    expect(csv).toBe("csv-output-data");

    // Error propagation test
    vi.mocked(invoke).mockRejectedValueOnce({ code: "SERVICE_ACCESS_DENIED", message: "Access denied" });
    await expect(getServiceAntivirus()).rejects.toThrow("Access denied");
  });
});
