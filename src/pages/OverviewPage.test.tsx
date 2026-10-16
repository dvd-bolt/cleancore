import { renderToStaticMarkup } from "react-dom/server";
import { describe, it, expect, vi } from "vitest";
import { OverviewPage } from "./OverviewPage";
import { MetricSnapshot } from "../types";

describe("OverviewPage", () => {
  const mockMetrics: MetricSnapshot = {
    timestamp_utc: "2026-10-16T12:00:00Z",
    interval_ms: 1000,
    cpu: {
      status: "Available",
      value: {
        total_usage_percent: 24.5,
        core_count: 8,
        frequency_mhz: 3200,
      },
    },
    memory: {
      status: "Available",
      value: {
        used_bytes: 8 * 1024 * 1024 * 1024,
        total_bytes: 16 * 1024 * 1024 * 1024,
        usage_percent: 50,
      },
    },
    disk: {
      status: "Available",
      value: {
        mount_point: "C:",
        free_bytes: 120 * 1024 * 1024 * 1024,
        total_bytes: 500 * 1024 * 1024 * 1024,
        usage_percent: 76,
      },
    },
    network: {
      status: "Available",
      value: {
        interface_name: "Ethernet",
        rx_bytes_per_sec: 150000,
        tx_bytes_per_sec: 45000,
      },
    },
    power: {
      is_battery_present: true,
      is_ac_connected: true,
      battery_percent: 100,
      is_charging: false,
      power_save_active: false,
    },
  };

  it("renders metric cards, smart insights widget, and process tables", () => {
    const onNavigate = vi.fn();
    const html = renderToStaticMarkup(
      <OverviewPage
        metrics={mockMetrics}
        history={[mockMetrics]}
        processes={[]}
        onNavigate={onNavigate}
      />
    );

    expect(html).toContain("Состояние системы");
    expect(html).toContain("Процессор");
    expect(html).toContain("Память");
    expect(html).toContain("Свободно на диске");
    expect(html).toContain("Интеллектуальные рекомендации");
    expect(html).toContain("Auto-Pilot Watcher");
  });
});
