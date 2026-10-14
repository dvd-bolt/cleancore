import React, { useState, useEffect, useRef, useMemo, useCallback } from "react";
import {
  ShieldCheck,
  FolderArchive,
  Search,
  Square,
  RefreshCw,
  FileText,
  AlertTriangle,
  ChevronLeft,
  ChevronRight,
  Clock,
  HardDrive,
  Info,
} from "lucide-react";
import {
  startCleanupScan,
  getScanProgress,
  cancelCleanupScan,
  getCleanupPreview,
} from "../lib/ipc";
import { CleanupCandidate, TaskStatus } from "../types";

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Б";
  const k = 1024;
  const sizes = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export function formatDate(isoString: string): string {
  try {
    const d = new Date(isoString);
    if (isNaN(d.getTime())) return isoString;
    return d.toLocaleString("ru-RU", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return isoString;
  }
}

const PAGE_SIZE = 15;

export const CleanupPage: React.FC = () => {
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [taskStatus, setTaskStatus] = useState<TaskStatus | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [candidates, setCandidates] = useState<CleanupCandidate[]>([]);
  const [totalCandidates, setTotalCandidates] = useState(0);
  const [totalBytes, setTotalBytes] = useState(0);
  const [page, setPage] = useState(0);
  const [isLoadingPage, setIsLoadingPage] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [scanError, setScanError] = useState<string | null>(null);

  const pollIntervalRef = useRef<number | null>(null);

  // Загрузка порции preview с сервера
  const loadPreviewPage = useCallback(async (pId: string, pageIndex: number) => {
    setIsLoadingPage(true);
    try {
      const offset = pageIndex * PAGE_SIZE;
      const res = await getCleanupPreview(pId, offset, PAGE_SIZE);
      setCandidates(res.items);
      setTotalCandidates(res.total_candidates);
      setTotalBytes(res.total_bytes);
      setPage(pageIndex);
    } catch (err) {
      setScanError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoadingPage(false);
    }
  }, []);

  // Опрос прогресса сканирования
  useEffect(() => {
    if (!activeTaskId || !isScanning) return;

    pollIntervalRef.current = window.setInterval(async () => {
      try {
        const status = await getScanProgress(activeTaskId);
        setTaskStatus(status);

        if (status.state === "Completed" || status.state === "Cancelled" || status.state === "Failed") {
          setIsScanning(false);
          if (pollIntervalRef.current) {
            clearInterval(pollIntervalRef.current);
            pollIntervalRef.current = null;
          }

          if (status.state === "Completed" || status.state === "Cancelled") {
            setPreviewId(activeTaskId);
            await loadPreviewPage(activeTaskId, 0);
          } else if (status.state === "Failed") {
            setScanError(status.error_message || "Ошибка во время анализа каталога");
          }
        }
      } catch (err) {
        setIsScanning(false);
        if (pollIntervalRef.current) {
          clearInterval(pollIntervalRef.current);
          pollIntervalRef.current = null;
        }
        setScanError(err instanceof Error ? err.message : String(err));
      }
    }, 350);

    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
    };
  }, [activeTaskId, isScanning, loadPreviewPage]);

  // Запуск анализа
  const handleStartScan = async () => {
    setScanError(null);
    setCandidates([]);
    setTotalCandidates(0);
    setTotalBytes(0);
    setPage(0);
    setPreviewId(null);
    setIsScanning(true);

    try {
      const taskId = await startCleanupScan("user-temp-v1");
      setActiveTaskId(taskId);
      setTaskStatus({
        id: taskId,
        kind: "scan:user-temp-v1",
        state: "Running",
        processed_items: 0,
      });
    } catch (err) {
      setIsScanning(false);
      setScanError(err instanceof Error ? err.message : String(err));
    }
  };

  // Отмена анализа
  const handleCancelScan = async () => {
    if (!activeTaskId) return;
    try {
      await cancelCleanupScan(activeTaskId);
    } catch (err) {
      setScanError(err instanceof Error ? err.message : String(err));
    }
  };

  // Фильтрация локального списка на текущей странице по строке поиска
  const filteredCandidates = useMemo(() => {
    if (!searchQuery.trim()) return candidates;
    const q = searchQuery.toLowerCase();
    return candidates.filter(
      (c) =>
        c.display_path.toLowerCase().includes(q) ||
        c.absolute_path.toLowerCase().includes(q) ||
        c.reason.toLowerCase().includes(q)
    );
  }, [candidates, searchQuery]);

  const totalPages = Math.ceil(totalCandidates / PAGE_SIZE);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="pb-2 border-b border-line/40">
        <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
          Анализатор очистки · v0.0.3
        </span>
        <h2 className="font-heading text-2xl font-bold text-txt">
          Безопасный анализ. <span className="text-dim">Без изменения файлов.</span>
        </h2>
        <p className="text-xs text-muted mt-1">
          Обнаружение временных файлов старше 7 дней с проверкой границ корня и исключением символических ссылок.
        </p>
      </div>

      {/* Steps Pipeline */}
      <div className="grid grid-cols-4 gap-2 text-xs font-mono select-none">
        {[
          { num: "01", label: "Анализ", active: isScanning || !previewId, done: !!previewId },
          { num: "02", label: "Просмотр", active: !!previewId && !isScanning, done: false },
          { num: "03", label: "Подтверждение", active: false, done: false, badge: "Этап 4" },
          { num: "04", label: "Очистка", active: false, done: false, badge: "Этап 4" },
        ].map((step) => (
          <div
            key={step.num}
            className={`p-3 rounded-xl border flex items-center justify-between transition-all ${
              step.active
                ? "bg-surface border-accent text-txt font-semibold shadow-sm"
                : step.done
                ? "bg-surface/80 border-accent/40 text-txt"
                : "bg-shell/60 border-line text-dim"
            }`}
          >
            <div className="flex items-center gap-2.5">
              <span
                className={`text-[10px] px-1.5 py-0.5 rounded font-bold ${
                  step.active
                    ? "bg-accent text-accent-ink"
                    : step.done
                    ? "bg-accent/20 text-accent"
                    : "bg-surface text-dim"
                }`}
              >
                {step.num}
              </span>
              <span>{step.label}</span>
            </div>
            {step.badge && (
              <span className="text-[9px] px-1.5 py-0.5 rounded bg-shell border border-line text-dim">
                {step.badge}
              </span>
            )}
          </div>
        ))}
      </div>

      {/* Category Card */}
      <article className="bg-surface border border-line rounded-xl p-5 transition-all">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 rounded-xl bg-shell border border-line flex items-center justify-center text-accent">
              <FolderArchive className="w-6 h-6" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h3 className="font-heading text-base font-semibold text-txt">
                  Временные файлы пользователя
                </h3>
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-shell border border-line text-dim">
                  user-temp-v1
                </span>
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-accent-wash text-accent border border-accent/20">
                  Низкий риск
                </span>
              </div>
              <p className="text-xs text-muted mt-0.5">
                Каталог: <span className="font-mono text-dim">%LOCALAPPDATA%\Temp</span> · Файлы старше 7 дней
              </p>
              <div className="flex items-center gap-4 mt-2 text-[11px] font-mono text-dim">
                <span className="flex items-center gap-1">
                  <Clock className="w-3.5 h-3.5 text-accent" /> Возраст: ≥ 7 дней
                </span>
                <span className="flex items-center gap-1">
                  <ShieldCheck className="w-3.5 h-3.5 text-accent" /> Symlinks/Junctions: пропуск
                </span>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-3">
            {isScanning ? (
              <button
                onClick={handleCancelScan}
                className="flex items-center gap-2 px-5 py-2.5 rounded-lg bg-shell border border-red-500/40 text-red-400 font-semibold text-xs hover:bg-red-500/10 transition-all shadow-sm"
              >
                <Square className="w-4 h-4 fill-current" />
                <span>Остановить</span>
              </button>
            ) : (
              <button
                onClick={handleStartScan}
                className="flex items-center gap-2 px-5 py-2.5 rounded-lg bg-accent text-accent-ink font-semibold text-xs hover:opacity-90 transition-all shadow-sm"
              >
                {previewId ? (
                  <>
                    <RefreshCw className="w-4 h-4" />
                    <span>Повторить анализ</span>
                  </>
                ) : (
                  <>
                    <Search className="w-4 h-4" />
                    <span>Запустить анализ</span>
                  </>
                )}
              </button>
            )}
          </div>
        </div>

        {/* Live scanning progress bar */}
        {isScanning && (
          <div className="mt-5 pt-4 border-t border-line/60">
            <div className="flex items-center justify-between text-xs font-mono mb-2">
              <span className="text-muted flex items-center gap-2">
                <span className="inline-block w-2 h-2 rounded-full bg-accent animate-ping" />
                Идёт безопасный обход каталога...
              </span>
              <span className="text-txt font-semibold">
                Проверено: {taskStatus?.processed_items ?? 0} файлов
              </span>
            </div>
            <div className="w-full h-1.5 bg-shell rounded-full overflow-hidden">
              <div className="h-full bg-accent rounded-full animate-pulse w-full" />
            </div>
          </div>
        )}

        {/* Error message */}
        {scanError && (
          <div className="mt-4 p-3 rounded-lg bg-red-950/20 border border-red-500/30 flex items-center gap-2.5 text-xs text-red-300">
            <AlertTriangle className="w-4 h-4 text-red-400 shrink-0" />
            <span>{scanError}</span>
          </div>
        )}
      </article>

      {/* Summary Banner after scan */}
      {previewId && !isScanning && (
        <section aria-label="Сводка результатов анализа" className="bg-surface/80 border border-line rounded-xl p-4 flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-6">
            <div>
              <span className="text-[10px] font-mono uppercase text-dim block">Найдено кандидатов</span>
              <span className="text-xl font-heading font-bold text-txt">{totalCandidates}</span>
            </div>
            <div className="h-8 w-px bg-line/60" />
            <div>
              <span className="text-[10px] font-mono uppercase text-dim block">Общий объём</span>
              <span className="text-xl font-heading font-bold text-accent">{formatBytes(totalBytes)}</span>
            </div>
            <div className="h-8 w-px bg-line/60" />
            <div>
              <span className="text-[10px] font-mono uppercase text-dim block">Статус анализа</span>
              <span className="text-xs font-mono font-medium text-emerald-400 flex items-center gap-1.5 mt-1">
                <ShieldCheck className="w-3.5 h-3.5" />
                {taskStatus?.state === "Cancelled" ? "Остановлено пользователем" : "Анализ завершён"}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-3">
            <div className="relative">
              <Search className="w-3.5 h-3.5 text-dim absolute left-3 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                placeholder="Фильтр по имени файла..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-8 pr-3 py-1.5 bg-shell border border-line rounded-lg text-xs text-txt placeholder:text-dim focus:outline-none focus:border-accent w-56 font-mono"
              />
            </div>
          </div>
        </section>
      )}

      {/* Preview Candidates Table (3.5 & 3.6) */}
      {previewId && !isScanning && (
        <section aria-label="Таблица кандидатов на очистку" className="bg-surface border border-line rounded-xl overflow-hidden shadow-sm">
          <div className="px-5 py-3 border-b border-line flex items-center justify-between">
            <div className="flex items-center gap-2">
              <FileText className="w-4 h-4 text-accent" />
              <h4 className="font-heading text-sm font-semibold text-txt">
                Предварительный просмотр (Preview)
              </h4>
              <span className="text-[10px] font-mono text-dim px-2 py-0.5 rounded bg-shell border border-line">
                Порция {totalCandidates > 0 ? page * PAGE_SIZE + 1 : 0}–{Math.min((page + 1) * PAGE_SIZE, totalCandidates)} из {totalCandidates}
              </span>
            </div>

            {/* Pagination Controls */}
            {totalPages > 1 && (
              <div className="flex items-center gap-2">
                <button
                  disabled={page === 0 || isLoadingPage}
                  onClick={() => loadPreviewPage(previewId, page - 1)}
                  className="p-1.5 rounded-lg bg-shell border border-line text-txt hover:bg-surface disabled:opacity-40 disabled:cursor-not-allowed transition-all"
                  title="Предыдущая страница"
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>
                <span className="text-xs font-mono text-dim px-2">
                  {page + 1} / {totalPages}
                </span>
                <button
                  disabled={page >= totalPages - 1 || isLoadingPage}
                  onClick={() => loadPreviewPage(previewId, page + 1)}
                  className="p-1.5 rounded-lg bg-shell border border-line text-txt hover:bg-surface disabled:opacity-40 disabled:cursor-not-allowed transition-all"
                  title="Следующая страница"
                >
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>
            )}
          </div>

          {filteredCandidates.length === 0 ? (
            <div className="p-12 text-center text-muted text-xs">
              <HardDrive className="w-8 h-8 text-dim mx-auto mb-2 opacity-50" />
              {totalCandidates === 0 ? (
                <>
                  <p className="font-semibold text-txt text-sm mb-1">Кандидатов не найдено</p>
                  <p>В каталоге нет файлов старше 7 дней, либо они заблокированы/защищены.</p>
                </>
              ) : (
                <p>Нет файлов, соответствующих поисковому фильтру «{searchQuery}».</p>
              )}
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs">
                <thead>
                  <tr className="border-b border-line/60 bg-shell/40 text-[10px] font-mono text-dim uppercase">
                    <th className="py-2.5 px-4 font-semibold">Имя / Относительный путь</th>
                    <th className="py-2.5 px-4 font-semibold w-28 text-right">Размер</th>
                    <th className="py-2.5 px-4 font-semibold w-36">Дата изменения</th>
                    <th className="py-2.5 px-4 font-semibold w-24">Риск</th>
                    <th className="py-2.5 px-4 font-semibold">Причина включения</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-line/30 font-mono">
                  {filteredCandidates.map((c) => (
                    <tr key={c.obj_id} className="hover:bg-shell/50 transition-colors">
                      <td className="py-2 px-4 text-txt font-sans">
                        <span className="font-mono text-xs block text-txt truncate max-w-md" title={c.absolute_path}>
                          {c.display_path}
                        </span>
                        <span className="text-[10px] font-mono text-dim truncate block max-w-md" title={c.absolute_path}>
                          {c.absolute_path}
                        </span>
                      </td>
                      <td className="py-2 px-4 text-right text-accent font-semibold whitespace-nowrap">
                        {formatBytes(c.size_bytes)}
                      </td>
                      <td className="py-2 px-4 text-dim whitespace-nowrap text-[11px]">
                        {formatDate(c.modified_time_utc)}
                      </td>
                      <td className="py-2 px-4 whitespace-nowrap">
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-950/40 text-emerald-400 border border-emerald-500/30">
                          {c.risk_level}
                        </span>
                      </td>
                      <td className="py-2 px-4 text-muted text-[11px] font-sans">
                        {c.reason}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>
      )}

      {/* Safety Guarantee Notice (Strict Stage 3 Requirement: No Delete Button) */}
      <div className="p-4 rounded-xl bg-shell/50 border border-line/80 flex items-start gap-4 text-xs">
        <div className="w-8 h-8 rounded-lg bg-surface border border-line flex items-center justify-center text-accent shrink-0">
          <Info className="w-4 h-4 text-accent" />
        </div>
        <div>
          <h4 className="font-heading text-xs font-semibold text-txt flex items-center gap-2">
            <span>Режим анализа (Этап 3) — Сканируемые файлы не изменяются</span>
            <span className="text-[9px] font-mono px-1.5 py-0.2 rounded bg-amber-950/40 text-amber-300 border border-amber-500/30">
              Read-Only
            </span>
          </h4>
          <p className="text-muted mt-1 leading-relaxed text-[11px]">
            На этапе 3 реализован исключительно безопасный сканер и предварительный просмотр (preview).
            Кнопка удаления отсутствует: файлы на диске не удаляются и не изменяются.
            Двухэтапное подтверждение и отправка в Корзину будут реализованы на следующем Этапе 4.
          </p>
        </div>
      </div>
    </div>
  );
};
