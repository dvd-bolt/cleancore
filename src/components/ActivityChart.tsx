import React, { useState } from "react";
import { Activity } from "lucide-react";

export const ActivityChart: React.FC = () => {
  const [activeMetric, setActiveMetric] = useState<"CPU" | "RAM" | "Сеть">("CPU");

  return (
    <article className="bg-surface border border-line rounded-xl p-5 flex flex-col justify-between">
      {/* Title & Tabs */}
      <div className="flex flex-wrap items-center justify-between gap-3 mb-4">
        <div>
          <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
            В реальном времени
          </span>
          <h3 className="font-heading text-base font-semibold text-txt">
            Активность системы
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
        <span className="text-dim">Ожидание измерений</span>
      </div>

      {/* Plot area */}
      <div className="relative h-44 bg-shell rounded-lg border border-line flex flex-col justify-between p-3 overflow-hidden">
        {/* Y Axis Grid */}
        <div className="absolute inset-0 flex flex-col justify-between p-3 pointer-events-none opacity-20">
          <div className="border-b border-line w-full" />
          <div className="border-b border-line w-full" />
          <div className="border-b border-line w-full" />
          <div className="border-b border-line w-full" />
        </div>

        {/* Empty placeholder state */}
        <div className="h-full flex flex-col items-center justify-center text-center z-10">
          <div className="w-10 h-10 rounded-full bg-surface border border-line flex items-center justify-center text-accent mb-2">
            <Activity className="w-5 h-5 text-accent" />
          </div>
          <strong className="text-sm font-medium text-txt">Здесь появится история</strong>
          <span className="text-xs text-muted mt-0.5">
            График начнёт заполняться после подключения сбора метрик на этапе 2
          </span>
        </div>
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
