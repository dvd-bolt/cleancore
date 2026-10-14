import { describe, it, expect } from "vitest";
import {
  fetchAppInfo,
  fetchInitialMetrics,
  fetchMetricSnapshot,
  fetchProcessList,
  fetchAppSettings,
  saveRefreshInterval,
  togglePauseMetrics,
  pingBackend,
} from "./ipc";

describe("IPC Bridge & Fallback behavior", () => {
  it("provides safe fallback app info in non-Tauri web environment", async () => {
    const info = await fetchAppInfo();
    expect(info.name).toContain("CleanCore");
    expect(info.is_local_mode).toBe(true);
    expect(info.is_admin).toBe(false);
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
});
