import React, { useState } from "react";
import { isTauriEnvironment } from "../lib/ipc";
import { AppInfo, AppSettings } from "../types";
import { Sliders, Shield, Palette, Info, Database, Check } from "lucide-react";

interface SettingsPageProps {
  appInfo: AppInfo | null;
  settings: AppSettings | null;
  onUpdateInterval: (intervalMs: number) => Promise<void>;
}

export const SettingsPage: React.FC<SettingsPageProps> = ({
  appInfo,
  settings,
  onUpdateInterval,
}) => {
  const [saving, setSaving] = useState(false);
  const [savedSuccess, setSavedSuccess] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const currentIntervalSec = Math.round(
    (settings?.refresh_interval_ms || 2000) / 1000
  ).toString();

  const handleIntervalChange = async (sec: "2" | "5" | "10") => {
    setSaving(true);
    setSavedSuccess(false);
    setSaveError(null);
    try {
      const ms = parseInt(sec, 10) * 1000;
      await onUpdateInterval(ms);
      setSavedSuccess(true);
      setTimeout(() => setSavedSuccess(false), 2500);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

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
          Параметры опроса датчиков, база данных настроек и гарантии приватности.
        </p>
      </div>

      {saveError && <p role="alert" className="text-xs text-red-300">Не удалось сохранить настройки: {saveError}</p>}
      {/* Settings Grid */}
      <div className="space-y-4">
        {/* Setting 1: Refresh Interval */}
        <div className="bg-surface border border-line rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3.5">
            <div className="w-9 h-9 rounded-lg bg-shell border border-line flex items-center justify-center text-accent">
              <Sliders className="w-4 h-4" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h3 className="font-heading text-sm font-semibold text-txt">
                  Интервал обновления показателей
                </h3>
                {savedSuccess && (
                  <span className="flex items-center gap-1 text-[11px] text-emerald-400 font-mono">
                    <Check className="w-3 h-3" /> {isTauriEnvironment() ? "Сохранено в SQLite" : "Изменено только в предпросмотре"}
                  </span>
                )}
              </div>
              <p className="text-xs text-muted mt-0.5">
                Частота опроса системных датчиков CPU, RAM и накопителей (диски: 30 сек)
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {(["2", "5", "10"] as const).map((sec) => (
              <button
                key={sec}
                disabled={saving}
                onClick={() => handleIntervalChange(sec)}
                className={`px-3 py-1.5 rounded-lg text-xs font-mono border transition-all ${
                  currentIntervalSec === sec
                    ? "bg-accent text-accent-ink border-accent font-semibold"
                    : "bg-shell border-line text-muted hover:text-txt"
                } disabled:opacity-50`}
              >
                {sec} сек
              </button>
            ))}
          </div>
        </div>

        {/* Setting 2: SQLite Storage */}
        <div className="bg-surface border border-line rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3.5">
            <div className="w-9 h-9 rounded-lg bg-shell border border-line flex items-center justify-center text-accent">
              <Database className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt">
                Хранилище настроек (SQLite)
              </h3>
              <p className="text-xs text-muted mt-0.5">
                Локальная база данных параметров приложения
              </p>
            </div>
          </div>

          <span className="font-mono text-xs px-3 py-1.5 rounded-lg bg-shell border border-line text-muted">
            %LOCALAPPDATA%\CleanCore\cleancore.db
          </span>
        </div>

        {/* Setting 3: Theme */}
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

        {/* Setting 4: Privacy */}
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
                Автономный режим: телеметрия, аналитика и внешние запросы отсутствуют
              </p>
            </div>
          </div>

          <span className="font-mono text-xs px-3 py-1.5 rounded-lg bg-emerald-950/40 border border-emerald-700/50 text-emerald-300">
            100% Local-First
          </span>
        </div>

        {/* Setting 5: About */}
        <div className="bg-surface border border-line rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3.5">
            <div className="w-9 h-9 rounded-lg bg-shell border border-line flex items-center justify-center text-dim">
              <Info className="w-4 h-4" />
            </div>
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt">
                О программе CleanCore
              </h3>
              <p className="text-xs text-muted mt-0.5">
                Rust stable · Tauri 2 · React 19 TypeScript · Vite
              </p>
            </div>
          </div>

          <span className="font-mono text-xs text-dim">
            Версия {appInfo?.version || "0.0.2"} (Этап 2)
          </span>
        </div>
      </div>
    </div>
  );
};
