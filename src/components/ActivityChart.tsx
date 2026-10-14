import React, { useState } from "react";
import { MetricSnapshot } from "../types";
import { Activity } from "lucide-react";
import { buildChartData } from "../lib/chart";

interface ActivityChartProps {
  history: MetricSnapshot[];
}

export const ActivityChart: React.FC<ActivityChartProps> = ({ history }) => {
  const [activeMetric, setActiveMetric] = useState<"CPU" | "RAM" | "Сеть">("CPU");

  const data = buildChartData(history, activeMetric);
  const points = data.points;
  const hasRealData = points.some((point) => point.value !== null);
  const latestValue = data.latest;
  const maxValue = data.max;
  const formattedLatest = latestValue === null ? "нет данных" : activeMetric === "Сеть" ? `${latestValue.toFixed(0)} КБ/с` : `${latestValue.toFixed(1)}%`;
  const width = 500;
  const height = 140;
  const paddingX = 10;
  const paddingY = 10;
  const chartW = width - paddingX * 2;
  const chartH = height - paddingY * 2;
  const pathD = data.path;
  // Не соединять отсутствующие измерения заливкой или линией.
  const areaD = "";

  return (
    <article className="bg-surface border border-line rounded-xl p-5 flex flex-col justify-between">
      {/* Title & Tabs */}
      <div className="flex flex-wrap items-center justify-between gap-3 mb-4">
        <div>
          <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
            В реальном времени
          </span>
          <h3 className="font-heading text-base font-semibold text-txt flex items-center gap-2">
            <span>Активность системы</span>
            {hasRealData && (
              <span className="text-xs font-mono font-normal text-muted">
                ({formattedLatest})
              </span>
            )}
          </h3>
        </div>

        <div className="flex items-center gap-1 bg-shell p-1 rounded-lg border border-line">
          {(["CPU", "RAM", "Сеть"] as const).map((m) => (
            <button
              key={m}
              onClick={() => setActiveMetric(m)}
              className={`px-3 py-1 rounded text-xs font-mono transition-all ${
                activeMetric === m
                  ? "bg-accent text-accent-ink font-semibold"
                  : "text-muted hover:text-txt"
              }`}
            >
              {m}
            </button>
          ))}
        </div>
      </div>

      {/* Sublabel */}
      <div className="flex items-center justify-between text-xs text-muted mb-2 font-mono">
        <span>{activeMetric} / динамика нагрузки</span>
        <span className="text-dim">
          {hasRealData ? `Точек: ${points.length}` : "Ожидание измерений"}
        </span>
      </div>

      {/* Plot area */}
      <div className="relative h-44 bg-shell rounded-lg border border-line flex flex-col justify-between p-2 overflow-hidden">
        {/* Y Axis Grid */}
        <div className="absolute inset-0 flex flex-col justify-between p-3 pointer-events-none opacity-20">
          <div className="border-b border-line w-full" />
          <div className="border-b border-line w-full" />
          <div className="border-b border-line w-full" />
          <div className="border-b border-line w-full" />
        </div>

        {!hasRealData ? (
          /* Empty placeholder state */
          <div className="h-full flex flex-col items-center justify-center text-center z-10">
            <div className="w-10 h-10 rounded-full bg-surface border border-line flex items-center justify-center text-accent mb-2">
              <Activity className="w-5 h-5 text-accent" />
            </div>
            <strong className="text-sm font-medium text-txt">
              Формирование дельты измерений
            </strong>
            <span className="text-xs text-muted mt-0.5">
              Нужны достоверные измерения; недоступные данные не заменяются нулями
            </span>
          </div>
        ) : (
          /* Live SVG Dynamic Chart */
          <div className="h-full w-full flex items-center justify-center z-10">
            <svg
              viewBox={`0 0 ${width} ${height}`}
              className="w-full h-full overflow-visible"
              preserveAspectRatio="none"
            >
              <defs>
                <linearGradient id="chartGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#e0e3e9" stopOpacity="0.25" />
                  <stop offset="100%" stopColor="#e0e3e9" stopOpacity="0.0" />
                </linearGradient>
              </defs>

              {areaD && (
                <path d={areaD} fill="url(#chartGradient)" />
              )}

              {pathD && (
                <path
                  d={pathD}
                  fill="none"
                  stroke="#e0e3e9"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              )}

              {/* Point on the latest value */}
              {latestValue !== null && (
                <circle
                  cx={paddingX + chartW}
                  cy={
                    height -
                    paddingY -
                    Math.min(Math.max(latestValue / (maxValue || 1), 0), 1) * chartH
                  }
                  r="3.5"
                  fill="#ffffff"
                  stroke="#16181d"
                  strokeWidth="2"
                />
              )}
            </svg>
          </div>
        )}
      </div>

      {/* Time axis */}
      <div className="flex items-center justify-between text-[10px] font-mono text-dim mt-2 px-1">
        <span>−60 сек</span>
        <span>−45</span>
        <span>−30</span>
        <span>−15</span>
        <span>Сейчас</span>
      </div>
    </article>
  );
};
