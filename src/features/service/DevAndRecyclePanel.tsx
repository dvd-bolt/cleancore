import React, { useState, useEffect, useCallback } from "react";
import { DevCacheSummary, RecycleBinInfo } from "../../types";
import {
  emptyRecycleBin,
  fetchDevCacheSummary,
  fetchRecycleBinInfo,
  revealPathInExplorer,
} from "../../lib/ipc";
import {
  Trash2,
  Code2,
  RefreshCw,
  ExternalLink,
  AlertTriangle,
  CheckCircle2,
} from "lucide-react";

interface DevAndRecyclePanelProps {
  className?: string;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Б";
  const k = 1024;
  const sizes = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(val >= 100 || i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export const DevAndRecyclePanel: React.FC<DevAndRecyclePanelProps> = ({ className = "" }) => {
  // Recycle Bin state
  const [recycleBin, setRecycleBin] = useState<RecycleBinInfo | null>(null);
  const [rbLoading, setRbLoading] = useState(false);
  const [rbEmptying, setRbEmptying] = useState(false);
  const [rbConfirmed, setRbConfirmed] = useState(false);
  const [rbSuccessMessage, setRbSuccessMessage] = useState<string | null>(null);
  const [rbError, setRbError] = useState<string | null>(null);

  // Dev Caches state
  const [devSummary, setDevSummary] = useState<DevCacheSummary | null>(null);
  const [devLoading, setDevLoading] = useState(false);
  const [inactiveDays, setInactiveDays] = useState(60);
  const [devError, setDevError] = useState<string | null>(null);
  const [revealingPath, setRevealingPath] = useState<string | null>(null);

  const loadRecycleBin = useCallback(async () => {
    setRbLoading(true);
    setRbError(null);
    try {
      const info = await fetchRecycleBinInfo();
      setRecycleBin(info);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setRbError(msg);
    } finally {
      setRbLoading(false);
    }
  }, []);

  const loadDevCaches = useCallback(async () => {
    setDevLoading(true);
    setDevError(null);
    try {
      const summary = await fetchDevCacheSummary(undefined, inactiveDays);
      setDevSummary(summary);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setDevError(msg);
    } finally {
      setDevLoading(false);
    }
  }, [inactiveDays]);

  useEffect(() => {
    void loadRecycleBin();
    void loadDevCaches();
  }, [loadRecycleBin, loadDevCaches]);

  const handleEmptyRecycleBin = async () => {
    if (!rbConfirmed) {
      setRbConfirmed(true);
      return;
    }
    setRbEmptying(true);
    setRbError(null);
    setRbSuccessMessage(null);
    try {
      const freed = await emptyRecycleBin();
      setRbSuccessMessage(
        `Корзина успешно очищена. Освобождено ${formatBytes(freed.total_bytes)} (${freed.item_count} объектов).`
      );
      setRbConfirmed(false);
      void loadRecycleBin();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setRbError(msg || "Не удалось очистить Корзину.");
    } finally {
      setRbEmptying(false);
    }
  };

  const handleReveal = async (path: string) => {
    setRevealingPath(path);
    try {
      await revealPathInExplorer(path);
    } catch (err) {
      console.warn("Reveal in explorer failed:", err);
    } finally {
      setTimeout(() => setRevealingPath(null), 1000);
    }
  };

  const kindLabel = (kind: string) => {
    switch (kind) {
      case "node_modules":
        return { label: "node_modules", color: "text-amber-400 bg-amber-500/10 border-amber-500/30" };
      case "target":
        return { label: "Rust target", color: "text-orange-400 bg-orange-500/10 border-orange-500/30" };
      case "venv":
        return { label: "Python .venv", color: "text-blue-400 bg-blue-500/10 border-blue-500/30" };
      case "wsl_docker":
        return { label: "WSL / Docker VHDX", color: "text-purple-400 bg-purple-500/10 border-purple-500/30" };
      default:
        return { label: kind, color: "text-slate-400 bg-slate-500/10 border-slate-500/30" };
    }
  };

  return (
    <div className={`space-y-6 ${className}`}>
      {/* 1. System Recycle Bin Card */}
      <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-4">
        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <Trash2 className="w-5 h-5 text-accent" />
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt">
                Системная Корзина Windows
              </h3>
              <p className="text-xs text-muted">
                Прямой опрос и очистка через штатный Win32 Shell API
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => void loadRecycleBin()}
              disabled={rbLoading}
              className="p-1.5 rounded-lg bg-surface-2 border border-line text-muted hover:text-txt transition-colors disabled:opacity-40"
              title="Обновить размер Корзины"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${rbLoading ? "animate-spin text-accent" : ""}`} />
            </button>

            {recycleBin && recycleBin.item_count > 0 && (
              <div className="flex items-center gap-2">
                {rbConfirmed ? (
                  <div className="flex items-center gap-1.5">
                    <button
                      onClick={() => void handleEmptyRecycleBin()}
                      disabled={rbEmptying}
                      className="px-3 py-1.5 rounded-lg bg-red-600 text-white text-xs font-semibold hover:bg-red-700 transition-colors shadow-sm disabled:opacity-40"
                    >
                      {rbEmptying ? "Очистка..." : "Да, очистить Корзину"}
                    </button>
                    <button
                      onClick={() => setRbConfirmed(false)}
                      disabled={rbEmptying}
                      className="px-2.5 py-1.5 rounded-lg bg-surface-2 border border-line text-muted hover:text-txt text-xs transition-colors"
                    >
                      Отмена
                    </button>
                  </div>
                ) : (
                  <button
                    onClick={() => setRbConfirmed(true)}
                    className="px-3 py-1.5 rounded-lg bg-accent text-accent-ink text-xs font-semibold hover:opacity-90 transition-opacity shadow-sm"
                  >
                    Очистить Корзину
                  </button>
                )}
              </div>
            )}
          </div>
        </div>

        {/* Recycle Bin Stats */}
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-2 border-t border-line/60">
          <div className="bg-surface-2 border border-line rounded-lg p-3">
            <span className="text-[11px] font-mono text-muted uppercase tracking-wider block">
              Занятый объём
            </span>
            <span className="text-xl font-bold font-mono text-txt mt-0.5 block">
              {recycleBin ? formatBytes(recycleBin.total_bytes) : "—"}
            </span>
          </div>

          <div className="bg-surface-2 border border-line rounded-lg p-3">
            <span className="text-[11px] font-mono text-muted uppercase tracking-wider block">
              Удалённых объектов
            </span>
            <span className="text-xl font-bold font-mono text-txt mt-0.5 block">
              {recycleBin ? `${recycleBin.item_count} файлов/папок` : "—"}
            </span>
          </div>
        </div>

        {/* Success message */}
        {rbSuccessMessage && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-emerald-950/20 border border-emerald-600/40 text-emerald-300 text-xs font-mono">
            <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-400" />
            <span>{rbSuccessMessage}</span>
          </div>
        )}

        {/* Error message */}
        {rbError && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-red-950/20 border border-red-600/40 text-red-300 text-xs font-mono">
            <AlertTriangle className="w-4 h-4 shrink-0 text-red-400" />
            <span>{rbError}</span>
          </div>
        )}
      </div>

      {/* 2. Dev Caches & Virtual Disks Panel */}
      <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-4">
        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <Code2 className="w-5 h-5 text-accent" />
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt">
                Кэши разработки и виртуальные диски
              </h3>
              <p className="text-xs text-muted">
                Неактивные node_modules, target, .venv и виртуальные диски Docker/WSL2
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2 w-full sm:w-auto justify-end">
            <div className="flex items-center gap-1.5">
              <label className="text-[11px] font-mono text-muted whitespace-nowrap">Неактивны &ge;</label>
              <select
                value={inactiveDays}
                onChange={(e) => setInactiveDays(Number(e.target.value))}
                className="bg-surface-2 border border-line rounded-lg px-2 py-1 text-xs font-mono text-txt focus:outline-none focus:border-accent"
              >
                <option value={30}>30 дней</option>
                <option value={60}>60 дней (стандарт)</option>
                <option value={90}>90 дней</option>
                <option value={180}>180 дней</option>
              </select>
            </div>

            <button
              onClick={() => void loadDevCaches()}
              disabled={devLoading}
              className="p-1.5 rounded-lg bg-surface-2 border border-line text-muted hover:text-txt transition-colors disabled:opacity-40"
              title="Пересканировать кэши разработки"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${devLoading ? "animate-spin text-accent" : ""}`} />
            </button>
          </div>
        </div>

        {/* 4 Summary Stat Mini-Cards */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 pt-2 border-t border-line/60">
          <div className="bg-surface-2 border border-line rounded-lg p-3">
            <span className="text-[10px] font-mono text-muted uppercase tracking-wider block">
              node_modules
            </span>
            <span className="text-sm font-bold font-mono text-amber-400 mt-0.5 block">
              {devSummary ? formatBytes(devSummary.node_modules_bytes) : "—"}
            </span>
            <span className="text-[10px] text-dim font-mono">
              {devSummary ? `${devSummary.node_modules_count} каталогов` : ""}
            </span>
          </div>

          <div className="bg-surface-2 border border-line rounded-lg p-3">
            <span className="text-[10px] font-mono text-muted uppercase tracking-wider block">
              Rust target/
            </span>
            <span className="text-sm font-bold font-mono text-orange-400 mt-0.5 block">
              {devSummary ? formatBytes(devSummary.target_bytes) : "—"}
            </span>
            <span className="text-[10px] text-dim font-mono">
              {devSummary ? `${devSummary.target_count} каталогов` : ""}
            </span>
          </div>

          <div className="bg-surface-2 border border-line rounded-lg p-3">
            <span className="text-[10px] font-mono text-muted uppercase tracking-wider block">
              Python .venv
            </span>
            <span className="text-sm font-bold font-mono text-blue-400 mt-0.5 block">
              {devSummary ? formatBytes(devSummary.venv_bytes) : "—"}
            </span>
            <span className="text-[10px] text-dim font-mono">
              {devSummary ? `${devSummary.venv_count} каталогов` : ""}
            </span>
          </div>

          <div className="bg-surface-2 border border-line rounded-lg p-3">
            <span className="text-[10px] font-mono text-muted uppercase tracking-wider block">
              WSL / Docker VHDX
            </span>
            <span className="text-sm font-bold font-mono text-purple-400 mt-0.5 block">
              {devSummary ? formatBytes(devSummary.wsl_docker_bytes) : "—"}
            </span>
            <span className="text-[10px] text-dim font-mono">
              образы ext4
            </span>
          </div>
        </div>

        {/* Error state */}
        {devError && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-amber-950/20 border border-amber-600/40 text-amber-300 text-xs font-mono">
            <AlertTriangle className="w-4 h-4 shrink-0 text-amber-400" />
            <span>{devError}</span>
          </div>
        )}

        {/* Dev Caches List Table */}
        <div className="bg-surface-2 border border-line rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs font-mono">
              <thead>
                <tr className="border-b border-line bg-surface text-muted uppercase tracking-wider text-[11px]">
                  <th className="py-2 px-3 font-semibold">Проект / Каталог</th>
                  <th className="py-2 px-3 font-semibold">Тип</th>
                  <th className="py-2 px-3 font-semibold">Неактивен</th>
                  <th className="py-2 px-3 font-semibold text-right">Размер</th>
                  <th className="py-2 px-3 font-semibold text-right">Действие</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line/60">
                {!devSummary || devSummary.entries.length === 0 ? (
                  <tr>
                    <td colSpan={5} className="py-6 text-center text-muted">
                      {devLoading ? (
                        <div className="flex items-center justify-center gap-2">
                          <RefreshCw className="w-4 h-4 animate-spin text-accent" />
                          <span>Поиск неактивных каталогов разработки...</span>
                        </div>
                      ) : (
                        `Неактивных кэшей разработки старше ${inactiveDays} дней не обнаружено.`
                      )}
                    </td>
                  </tr>
                ) : (
                  devSummary.entries.map((entry, idx) => {
                    const meta = kindLabel(entry.kind);
                    const isRevealing = revealingPath === entry.path;
                    return (
                      <tr
                        key={`${entry.path}-${idx}`}
                        className="hover:bg-surface/60 transition-colors"
                      >
                        <td className="py-2 px-3 max-w-[260px] lg:max-w-[380px]">
                          <span className="font-medium text-txt block truncate" title={entry.name}>
                            {entry.name}
                          </span>
                          <span className="text-[10px] text-dim block truncate" title={entry.path}>
                            {entry.path}
                          </span>
                        </td>

                        <td className="py-2 px-3 whitespace-nowrap">
                          <span className={`inline-block px-1.5 py-0.5 rounded border text-[10px] font-medium ${meta.color}`}>
                            {meta.label}
                          </span>
                        </td>

                        <td className="py-2 px-3 whitespace-nowrap text-dim">
                          {entry.inactive_days} дн.
                        </td>

                        <td className="py-2 px-3 text-right font-bold text-txt whitespace-nowrap">
                          {formatBytes(entry.size_bytes)}
                        </td>

                        <td className="py-2 px-3 text-right whitespace-nowrap">
                          <button
                            onClick={() => void handleReveal(entry.path)}
                            disabled={isRevealing}
                            className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-surface border border-line text-txt hover:bg-surface-2 text-[11px] transition-colors disabled:opacity-40"
                            title="Открыть расположение в Проводнике"
                          >
                            <ExternalLink className="w-3 h-3 text-muted" />
                            <span>{isRevealing ? "Открыто" : "В Проводнике"}</span>
                          </button>
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
};
