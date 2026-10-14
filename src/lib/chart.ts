import { MetricSnapshot } from "../types";

export type ChartMetric = "CPU" | "RAM" | "Сеть";

export function buildChartData(history: MetricSnapshot[], metric: ChartMetric) {
  const readings = history.map((snapshot) => {
    const field = metric === "CPU" ? snapshot.cpu : metric === "RAM" ? snapshot.memory : snapshot.network;
    let value: number | null = null;
    if (field.status === "Available") {
      if (metric === "CPU") value = snapshot.cpu.value?.total_usage_percent ?? null;
      if (metric === "RAM") value = snapshot.memory.value?.usage_percent ?? null;
      if (metric === "Сеть") value = snapshot.network.value ? snapshot.network.value.rx_bytes_per_sec / 1024 : null;
    }
    if (value !== null && (!Number.isFinite(value) || value < 0)) value = null;
    return { time: Date.parse(snapshot.timestamp_utc), value };
  }).filter((reading) => Number.isFinite(reading.time)).sort((a, b) => a.time - b.time);

  const end = readings.at(-1)?.time ?? 0;
  const points = readings.filter((reading) => reading.time >= end - 60_000);
  const max = metric === "Сеть" ? Math.max(50, ...points.map((point) => point.value ?? 0)) : 100;
  let path = "";
  let connected = false;
  for (const point of points) {
    if (point.value === null) { connected = false; continue; }
    const x = 10 + ((point.time - (end - 60_000)) / 60_000) * 480;
    const y = 130 - Math.min(point.value / max, 1) * 120;
    path += `${connected ? "L" : "M"} ${x.toFixed(1)} ${y.toFixed(1)} `;
    connected = true;
  }
  return { points, path: path.trim(), max, latest: points.at(-1)?.value ?? null };
}
