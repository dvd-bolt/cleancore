import React, { useState, useEffect, useCallback } from "react";
import { PowerProfileKind, PowerProfileStatus } from "../../types";
import {
  applyPowerProfile,
  fetchPowerProfileState,
  rollbackPowerProfile,
} from "../../lib/ipc";
import {
  Zap,
  Gauge,
  RotateCcw,
  CheckCircle2,
  AlertTriangle,
  RefreshCw,
  Sliders,
  ShieldCheck,
} from "lucide-react";

interface PowerTuningCardProps {
  className?: string;
}

export const PowerTuningCard: React.FC<PowerTuningCardProps> = ({ className = "" }) => {
  const [status, setStatus] = useState<PowerProfileStatus | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [applying, setApplying] = useState<boolean>(false);
  const [rollingBack, setRollingBack] = useState<boolean>(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);

  const loadStatus = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetchPowerProfileState();
      setStatus(res);
    } catch (err: unknown) {
      console.warn("Power profile status failed:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const handleApply = async (profile: PowerProfileKind) => {
    setApplying(true);
    setMessage(null);
    try {
      const res = await applyPowerProfile(profile);
      setStatus(res);
      const label =
        profile === "Quiet"
          ? "«Тихий» (турбобуст отключён, лимит 99%)"
          : profile === "Performance"
          ? "«Производительный» (100% макс / 100% мин)"
          : "«Сбалансированный» (100% макс / 5% мин)";
      setMessage({
        type: "success",
        text: `Профиль ${label} успешно применён. Исходные параметры сохранены в точке отката SQLite.`,
      });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setMessage({
        type: "error",
        text: `Не удалось применить профиль: ${msg}`,
      });
    } finally {
      setApplying(false);
    }
  };

  const handleRollback = async () => {
    setRollingBack(true);
    setMessage(null);
    try {
      const res = await rollbackPowerProfile();
      setStatus(res);
      setMessage({
        type: "success",
        text: "Исходные настройки электропитания и троттлинга успешно восстановлены из SQLite.",
      });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setMessage({
        type: "error",
        text: `Не удалось выполнить откат: ${msg}`,
      });
    } finally {
      setRollingBack(false);
    }
  };

  return (
    <div className={`bg-surface border border-line rounded-xl p-5 shadow-sm space-y-4 ${className}`}>
      {/* Header */}
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <Zap className="w-5 h-5 text-accent" />
          <div>
            <h3 className="font-heading text-sm font-semibold text-txt">
              Электропитание и процессорный троттлинг
            </h3>
            <p className="text-xs text-muted">
              Управление нагревом, шумом кулеров и частотами процессора через Win32 Power Schemes
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => void loadStatus()}
            disabled={loading}
            className="p-1.5 rounded-lg bg-surface-2 border border-line text-muted hover:text-txt transition-colors disabled:opacity-40"
            title="Обновить состояние питания"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? "animate-spin text-accent" : ""}`} />
          </button>
        </div>
      </div>

      {/* Current Active Scheme Banner */}
      <div className="bg-surface-2 border border-line rounded-lg p-3.5 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
        <div className="space-y-0.5">
          <div className="flex items-center gap-2">
            <span className="text-[11px] font-mono text-muted uppercase tracking-wider">
              Активная схема Windows:
            </span>
            <span className="text-xs font-semibold text-txt font-mono">
              {status?.active_scheme.name || "Считывание..."}
            </span>
          </div>
          <p className="text-[10px] text-dim font-mono">
            GUID: {status?.active_scheme.guid || "—"}
          </p>
        </div>

        {status && (
          <div className="flex items-center gap-2 text-xs font-mono">
            <span className="text-dim">Троттлинг:</span>
            <span className="px-2 py-0.5 rounded bg-surface border border-line text-txt">
              Макс: {status.throttles.max_ac}%
            </span>
            <span className="px-2 py-0.5 rounded bg-surface border border-line text-dim">
              Мин: {status.throttles.min_ac}%
            </span>
          </div>
        )}
      </div>

      {/* 3 Profile Selection Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-3 pt-2 border-t border-line/60">
        {/* 1. Quiet Profile */}
        <div
          className={`p-4 rounded-xl border transition-all ${
            status?.current_profile === "Quiet"
              ? "bg-accent/10 border-accent"
              : "bg-surface-2 border-line hover:border-line-hover"
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-1.5">
              <Gauge className="w-4 h-4 text-emerald-400" />
              <span className="font-semibold text-xs text-txt">Тихий (Quiet)</span>
            </div>
            {status?.current_profile === "Quiet" && (
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-accent text-accent-ink font-bold">
                Активен
              </span>
            )}
          </div>
          <p className="text-[11px] text-muted leading-relaxed mb-3">
            Лимит процессора 99%. <strong>Отключает агрессивный турбобуст</strong>, снижает температуру CPU на 5–15°C и шум вентилятора.
          </p>
          <button
            onClick={() => void handleApply("Quiet")}
            disabled={applying || status?.current_profile === "Quiet"}
            className="w-full py-1.5 rounded-lg bg-surface border border-line text-txt text-xs font-mono hover:bg-surface-2 hover:border-line-hover transition-colors disabled:opacity-40"
          >
            {status?.current_profile === "Quiet" ? "Включено" : "Включить тихий"}
          </button>
        </div>

        {/* 2. Balanced Profile */}
        <div
          className={`p-4 rounded-xl border transition-all ${
            status?.current_profile === "Balanced"
              ? "bg-accent/10 border-accent"
              : "bg-surface-2 border-line hover:border-line-hover"
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-1.5">
              <Sliders className="w-4 h-4 text-sky-400" />
              <span className="font-semibold text-xs text-txt">Сбалансированный</span>
            </div>
            {status?.current_profile === "Balanced" && (
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-accent text-accent-ink font-bold">
                Активен
              </span>
            )}
          </div>
          <p className="text-[11px] text-muted leading-relaxed mb-3">
            Заводские 100% макс, 5% мин. Стандартный баланс энергосбережения и полной мощности для повседневных задач.
          </p>
          <button
            onClick={() => void handleApply("Balanced")}
            disabled={applying || status?.current_profile === "Balanced"}
            className="w-full py-1.5 rounded-lg bg-surface border border-line text-txt text-xs font-mono hover:bg-surface-2 hover:border-line-hover transition-colors disabled:opacity-40"
          >
            {status?.current_profile === "Balanced" ? "Включено" : "Включить баланс"}
          </button>
        </div>

        {/* 3. Performance Profile */}
        <div
          className={`p-4 rounded-xl border transition-all ${
            status?.current_profile === "Performance"
              ? "bg-accent/10 border-accent"
              : "bg-surface-2 border-line hover:border-line-hover"
          }`}
        >
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-1.5">
              <Zap className="w-4 h-4 text-amber-400" />
              <span className="font-semibold text-xs text-txt">Максимальная мощность</span>
            </div>
            {status?.current_profile === "Performance" && (
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-accent text-accent-ink font-bold">
                Активен
              </span>
            )}
          </div>
          <p className="text-[11px] text-muted leading-relaxed mb-3">
            Лимит 100% макс, 100% мин. Процессор держит предельные частоты без снижения множителя для компиляции и игр.
          </p>
          <button
            onClick={() => void handleApply("Performance")}
            disabled={applying || status?.current_profile === "Performance"}
            className="w-full py-1.5 rounded-lg bg-surface border border-line text-txt text-xs font-mono hover:bg-surface-2 hover:border-line-hover transition-colors disabled:opacity-40"
          >
            {status?.current_profile === "Performance" ? "Включено" : "Включить турбо"}
          </button>
        </div>
      </div>

      {/* Rollback & Safety Section */}
      <div className="pt-2 border-t border-line/60 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-xs font-mono text-muted">
          <ShieldCheck className="w-4 h-4 text-emerald-400 shrink-0" />
          {status?.has_rollback_snapshot ? (
            <span>
              Точка отката сохранена в SQLite ({status.last_snapshot_time?.split("T")[0] || "недавно"})
            </span>
          ) : (
            <span>Снимков питания в SQLite пока не сохранено</span>
          )}
        </div>

        {status?.has_rollback_snapshot && (
          <button
            onClick={() => void handleRollback()}
            disabled={rollingBack}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-2 border border-line text-txt hover:bg-surface hover:border-line-hover text-xs font-mono transition-colors disabled:opacity-40"
            title="Восстановить заводскую схему и значения троттлинга"
          >
            <RotateCcw className={`w-3.5 h-3.5 ${rollingBack ? "animate-spin" : "text-amber-400"}`} />
            <span>Вернуть исходные настройки питания</span>
          </button>
        )}
      </div>

      {/* Messages */}
      {message && (
        <div
          className={`flex items-center gap-2 p-3 rounded-lg border text-xs font-mono ${
            message.type === "success"
              ? "bg-emerald-950/20 border-emerald-600/40 text-emerald-300"
              : "bg-red-950/20 border-red-600/40 text-red-300"
          }`}
        >
          {message.type === "success" ? (
            <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-400" />
          ) : (
            <AlertTriangle className="w-4 h-4 shrink-0 text-red-400" />
          )}
          <span>{message.text}</span>
        </div>
      )}
    </div>
  );
};
