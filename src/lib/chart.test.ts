import { describe, expect, it } from "vitest";
import { buildChartData } from "./chart";
import { MetricSnapshot } from "../types";

const sample = (second: number, cpu: MetricSnapshot["cpu"]): MetricSnapshot => ({
  timestamp_utc: `2026-10-14T12:00:${String(second).padStart(2, "0")}Z`, interval_ms: 5000,
  cpu, memory: { status: "NotMeasured" }, disk: { status: "NotMeasured" }, network: { status: "NotMeasured" },
});
const available = (value: number): MetricSnapshot["cpu"] => ({ status: "Available", value: { total_usage_percent: value, core_count: 8 } });

describe("chart data availability and elapsed time", () => {
  it("leaves a gap for unavailable readings instead of a fabricated zero", () => {
    const data = buildChartData([sample(0, available(30)), sample(5, { status: "NotMeasured" }), sample(10, available(40))], "CPU");
    expect(data.points[1].value).toBeNull();
    expect(data.path.match(/M /g)).toHaveLength(2);
    expect(data.path).not.toContain("L ");
  });
  it("uses timestamps for the minute axis instead of stretching sample indexes", () => {
    const data = buildChartData([sample(0, available(10)), sample(5, available(20)), sample(15, available(30))], "CPU");
    const x = [...data.path.matchAll(/[ML] ([\d.]+)/g)].map((match) => Number(match[1]));
    expect(x[2]).toBe(490);
    expect(x[2] - x[1]).toBe(2 * (x[1] - x[0]));
  });
  it("does not show an old value as the latest when the latest measurement is unavailable", () => {
    const data = buildChartData([sample(0, available(10)), sample(5, { status: "AccessDenied" })], "CPU");
    expect(data.latest).toBeNull();
  });
});
