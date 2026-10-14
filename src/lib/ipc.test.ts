import { describe, it, expect } from "vitest";
import { fetchAppInfo, fetchInitialMetrics, pingBackend } from "./ipc";

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
  });

  it("handles ping gracefully", async () => {
    const pong = await pingBackend();
    expect(typeof pong).toBe("string");
  });
});
