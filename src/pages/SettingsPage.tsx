import React, { useState } from "react";
import { AppInfo } from "../types";
import { Sliders, Shield, Palette, Info } from "lucide-react";

interface SettingsPageProps {
  appInfo: AppInfo | null;
}

export const SettingsPage: React.FC<SettingsPageProps> = ({ appInfo }) => {
  const [intervalSec, setIntervalSec] = useState("2");

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="pb-2 border-b border-line/40">
        <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
          Настройки
        </span>
        <h2 className="font-heading text-2xl font-bold text-txt">
          В твоём ритме.
        </h2>
        <p className="text-xs text-muted mt-1">
          Параметры опроса датчиков, отображение темы и гарантии приватности.
        </p>
      </div>

      {/* Settings Grid */}
      <div className="space-y-4">
        {/* Setting 1: Refresh Interval */}
        <div className="bg-surface border border-line rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3.5">
            <div className="w-9 h-9 rounded-lg bg-shell border border-line flex items-center justify-center text-accent">
              <Sliders className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt">
                Интервал обновления показателей
              </h3>
              <p className="text-xs text-muted mt-0.5">
                Частота опроса системных датчиков CPU, RAM и накопителей
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {(["2", "5", "10"] as const).map((sec) => (
              <button
                key={sec}
                onClick={() => setIntervalSec(sec)}
                className={`px-3 py-1.5 rounded-lg text-xs font-mono border transition-all ${
                  intervalSec === sec
                    ? "bg-accent text-accent-ink border-accent font-semibold"
                    : "bg-shell border-line text-muted hover:text-txt"
                }`}
              >
                {sec} сек
              </button>
            ))}
          </div>
        </div>

        {/* Setting 2: Theme */}
        <div className="bg-surface border border-line rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3.5">
            <div className="w-9 h-9 rounded-lg bg-shell border border-line flex items-center justify-center text-accent">
              <Palette className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt">
                Тёмная тема интерфейса
              </h3>
              <p className="text-xs text-muted mt-0.5">
                Утверждённая палитра дизайна согласно решению 001
              </p>
            </div>
          </div>

          <span className="font-mono text-xs px-3 py-1.5 rounded-lg bg-shell border border-line text-txt flex items-center gap-2">
            <span className="w-2.5 h-2.5 rounded-full bg-[#e0e3e9] border border-white/30" />
            <span>Studio / Жемчуг (Pearl)</span>
          </span>
        </div>

        {/* Setting 3: Privacy */}
        <div className="bg-surface border border-line rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3.5">
            <div className="w-9 h-9 rounded-lg bg-shell border border-line flex items-center justify-center text-accent">
              <Shield className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt">
                Приватность и сеть
              </h3>
              <p className="text-xs text-muted mt-0.5">
                Автономный режим: сетевые запросы и сбор аналитики отключены
              </p>
            </div>
          </div>

          <span className="font-mono text-xs px-3 py-1.5 rounded-lg bg-emerald-950/40 border border-emerald-700/50 text-emerald-300">
            100% Local-First
          </span>
        </div>

        {/* Setting 4: About */}
        <div className="bg-surface border border-line rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3.5">
            <div className="w-9 h-9 rounded-lg bg-shell border border-line flex items-center justify-center text-dim">
              <Info className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt">
                О программе
              </h3>
              <p className="text-xs text-muted mt-0.5">
                CleanCore · Rust stable · Tauri 2 · React TypeScript
              </p>
            </div>
          </div>

          <span className="font-mono text-xs text-dim">
            Версия {appInfo?.version || "0.0.1"} (Этап 1)
          </span>
        </div>
      </div>
    </div>
  );
};
