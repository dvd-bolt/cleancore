import React, { useState, useEffect, useRef, useCallback } from "react";
import { DiskTreemap } from "../features/disk-map/DiskTreemap";
import { DuplicateActions } from "../features/disk-map/DuplicateActions";
import { SpaceHogsTable } from "../features/disk-map/SpaceHogsTable";
import { StaleFilesView } from "../features/disk-map/StaleFilesView";
import {
  DiskAnalysisReport,
  DiskTaskStatus,
  DuplicateScanResult,
  FileCategory,
  QuickDirectory,
  ResourceHistory,
} from "../types";
import {
  cancelDiskTask,
  chooseAnalysisFolder,
  isTauriEnvironment,
  fetchQuickDirectories,
  fetchResourceHistory,
  getDiskAnalysisProgress,
  getDiskAnalysisReport,
  getDuplicateReport,
  getDuplicateScanProgress,
  startDiskAnalysis,
  startDuplicateScan,
} from "../lib/ipc";
import {
  HardDrive,
  Folder,
  FolderOpen,
  File,
  Film,
  Image as ImageIcon,
  FileText,
  Archive,
  Music,
  Code2,
  Package,
  Boxes,
  Search,
  Copy,
  Activity,
  CheckCircle2,
  AlertCircle,
  XCircle,
  Layers,
  ShieldCheck,
  TrendingUp,
  Clock,
  History,
} from "lucide-react";

type DiskTab = "map" | "duplicates" | "space_hogs" | "stale" | "history";

const CATEGORY_COLORS: Record<FileCategory, { bg: string; text: string; hex: string }> = {
  Videos: { bg: "bg-sky-500", text: "text-sky-400", hex: "#0ea5e9" },
  Images: { bg: "bg-pink-500", text: "text-pink-400", hex: "#ec4899" },
  Documents: { bg: "bg-amber-500", text: "text-amber-400", hex: "#f59e0b" },
  Archives: { bg: "bg-purple-500", text: "text-purple-400", hex: "#a855f7" },
  Audio: { bg: "bg-emerald-500", text: "text-emerald-400", hex: "#10b981" },
  Code: { bg: "bg-blue-500", text: "text-blue-400", hex: "#3b82f6" },
  Other: { bg: "bg-slate-500", text: "text-slate-400", hex: "#64748b" },
};

function getCategoryIcon(cat: FileCategory) {
  switch (cat) {
    case "Videos":
      return <Film className="w-4 h-4 text-sky-400" />;
    case "Images":
      return <ImageIcon className="w-4 h-4 text-pink-400" />;
    case "Documents":
      return <FileText className="w-4 h-4 text-amber-400" />;
    case "Archives":
      return <Archive className="w-4 h-4 text-purple-400" />;
    case "Audio":
      return <Music className="w-4 h-4 text-emerald-400" />;
    case "Code":
      return <Code2 className="w-4 h-4 text-blue-400" />;
    default:
      return <Package className="w-4 h-4 text-slate-400" />;
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Б";
  const k = 1024;
  const sizes = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(val >= 100 || i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export const DiskPage: React.FC = () => {
  const [activeTab, setActiveTab] = useState<DiskTab>("map");
  const [quickDirs, setQuickDirs] = useState<QuickDirectory[]>([]);
  const [selectedPath, setSelectedPath] = useState<string>("");
  const [customPath, setCustomPath] = useState<string>("");

  // Состояние анализа диска
  const [diskTaskId, setDiskTaskId] = useState<string | null>(() => sessionStorage.getItem("cleancore-disk-task"));
  const [directoryPage,setDirectoryPage]=useState(0);
  const [diskTaskStatus, setDiskTaskStatus] = useState<DiskTaskStatus | null>(null);
  const [diskReport, setDiskReport] = useState<DiskAnalysisReport | null>(null);
  const [browsingDirectory, setBrowsingDirectory] = useState(false);

  // Состояние поиска дубликатов
  const [dupTaskId, setDupTaskId] = useState<string | null>(() => sessionStorage.getItem("cleancore-dup-task"));
  const [dupTaskStatus, setDupTaskStatus] = useState<DiskTaskStatus | null>(null);
  const [dupReport, setDupReport] = useState<DuplicateScanResult | null>(null);

  // Состояние 10-минутной истории ресурсов
  const [history, setHistory] = useState<ResourceHistory | null>(null);

  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const isPollingRef = useRef(false);

  // Загрузка стандартных папок и предварительного отчета
  useEffect(() => {
    let mounted = true;
    async function loadInitial() {
      try {
        const dirs = await fetchQuickDirectories();
        if (mounted && dirs.length > 0) {
          setQuickDirs(dirs);
          const defaultPath = dirs[0].path;
          setSelectedPath(defaultPath);
          setCustomPath(defaultPath);
        }
      } catch (err) {
        if (mounted) {
          setErrorMessage(err instanceof Error ? err.message : String(err));
        }
      }
    }
    loadInitial();
    return () => {
      mounted = false;
    };
  }, []);

  // Периодический опрос активной задачи анализа диска
  useEffect(() => {
    if (!diskTaskId) return;
    let timer: NodeJS.Timeout;
    let cancelled = false;

    async function poll() {
      if (cancelled) return;
      isPollingRef.current = true;
      try {
        const status = await getDiskAnalysisProgress(diskTaskId!);
        if (cancelled) return;
        setDiskTaskStatus(status);

        if (status.state === "Completed") {
          const report = await getDiskAnalysisReport(diskTaskId!);
          if (!cancelled) {
            setDiskReport(report);
            setDiskTaskId(null);
          }
        } else if (status.state === "Failed" || status.state === "Cancelled") {
          if(status.state==="Cancelled") {
            const report=await getDiskAnalysisReport(diskTaskId!);
            if(cancelled)return;
            setDiskReport(report);
          }
          setErrorMessage(status.error_message || (status.state === "Cancelled" ? "Анализ отменён. Ниже сохранена прочитанная часть." : "Не удалось завершить анализ"));
          if(status.state==="Failed") sessionStorage.removeItem("cleancore-disk-task");
          setDiskTaskId(null);
        } else {
          timer = setTimeout(poll, 700);
        }
      } catch (err) {
        if (!cancelled) {
          sessionStorage.removeItem("cleancore-disk-task");
          setErrorMessage(err instanceof Error ? err.message : String(err));
          setDiskTaskId(null);
        }
      } finally {
        isPollingRef.current = false;
      }
    }

    poll();

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [diskTaskId]);

  // Периодический опрос активной задачи дубликатов
  useEffect(() => {
    if (!dupTaskId) return;
    let timer: NodeJS.Timeout;
    let cancelled = false;

    async function poll() {
      if (cancelled) return;
      isPollingRef.current = true;
      try {
        const status = await getDuplicateScanProgress(dupTaskId!);
        if (cancelled) return;
        setDupTaskStatus(status);

        if (status.state === "Completed") {
          const report = await getDuplicateReport(dupTaskId!);
          if (!cancelled) {
            setDupReport(report);
            setDupTaskId(null);
          }
        } else if (status.state === "Failed" || status.state === "Cancelled") {
          setErrorMessage(status.error_message || (status.state === "Cancelled" ? "Поиск отменён" : "Не удалось завершить поиск"));
          sessionStorage.removeItem("cleancore-dup-task");
          setDupTaskId(null);
        } else {
          timer = setTimeout(poll, 700);
        }
      } catch (err) {
        if (!cancelled) {
          sessionStorage.removeItem("cleancore-dup-task");
          setErrorMessage(err instanceof Error ? err.message : String(err));
          setDupTaskId(null);
        }
      } finally {
        isPollingRef.current = false;
      }
    }

    poll();

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [dupTaskId]);

  // Загрузка истории ресурсов при переходе на вкладку
  const loadHistory = useCallback(async () => {
    try {
      const data = await fetchResourceHistory();
      setHistory(data);
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    if (activeTab === "history") {
      loadHistory();
      const interval = setInterval(loadHistory, 3000);
      return () => clearInterval(interval);
    }
  }, [activeTab, loadHistory]);

  const handleStartDiskAnalysis = async (path?: string) => {
    const target = path || customPath.trim() || selectedPath;
    if (!target) return;
    setErrorMessage(null);
    setDiskReport(null);
    try {
      const taskId = await startDiskAnalysis(target);
      sessionStorage.removeItem("cleancore-dup-task");
      sessionStorage.setItem("cleancore-disk-task",taskId);
      setCustomPath(target);setSelectedPath(target);setDirectoryPage(0);
      setDiskTaskId(taskId);
      setActiveTab("map");
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  const browseDirectory = async (path: string) => {
    const reportId = sessionStorage.getItem("cleancore-disk-task");
    if (diskReport?.engine !== "ntfs-mft" || !reportId) {
      await handleStartDiskAnalysis(path);
      return;
    }
    setBrowsingDirectory(true);
    setErrorMessage(null);
    try {
      const report = await getDiskAnalysisReport(reportId, path);
      setDiskReport(report);
      setDirectoryPage(0);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBrowsingDirectory(false);
    }
  };

  const handleStartDuplicateScan = async () => {
    const target = customPath.trim() || selectedPath;
    if (!target) return;
    setErrorMessage(null);
    setDupReport(null);
    try {
      const taskId = await startDuplicateScan(target);
      sessionStorage.removeItem("cleancore-disk-task");
      sessionStorage.setItem("cleancore-dup-task",taskId);
      setDupTaskId(taskId);
      setActiveTab("duplicates");
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCancelActiveTask = async (taskId: string) => {
    try {
      await cancelDiskTask(taskId);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  const [choosingFolder, setChoosingFolder] = useState(false);
  async function chooseFolder() {
    if (choosingFolder) return;
    setChoosingFolder(true);
    try {
      const path = await chooseAnalysisFolder();
      if (path) { setSelectedPath(path); setCustomPath(path); setErrorMessage(null); }
    } catch (e) { setErrorMessage(e instanceof Error ? e.message : String(e)); }
    finally { setChoosingFolder(false); }
  }
  const currentTargetPath = customPath.trim() || selectedPath;

  return (
    <div className="space-y-6">
      {/* Top Banner / Breadcrumb */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-2 border-b border-line">
        <div>
          <div className="flex items-center gap-2">
            <HardDrive className="w-5 h-5 text-accent" />
            <h1 className="font-heading text-xl font-bold tracking-tight text-txt">
              Анализ диска и дубликатов
            </h1>

          </div>
          <p className="text-xs text-muted mt-1">
            Безопасная инспекция структуры каталогов, категорийный срез и поиск доказанных дубликатов без автоматического удаления
          </p>
        </div>

        {/* Tab Switcher */}
        <div className="flex items-center gap-1 bg-surface p-1 rounded-xl border border-line self-start md:self-auto">
          <button
            onClick={() => setActiveTab("map")}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
              activeTab === "map"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <Layers className="w-3.5 h-3.5" />
            <span>Карта и объекты</span>
          </button>
          <button
            onClick={() => setActiveTab("duplicates")}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
              activeTab === "duplicates"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <Copy className="w-3.5 h-3.5" />
            <span>Поиск дубликатов</span>
            {dupReport && dupReport.groups.length > 0 && (
              <span className="ml-1 px-1.5 py-0.2 rounded-full bg-amber-500/20 text-amber-300 text-[10px] font-mono">
                {dupReport.groups.length}
              </span>
            )}
          </button>
          <button
            onClick={() => setActiveTab("space_hogs")}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
              activeTab === "space_hogs"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <Boxes className="w-3.5 h-3.5" />
            <span>Крупные файлы</span>
          </button>
          <button
            onClick={() => setActiveTab("stale")}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
              activeTab === "stale"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <History className="w-3.5 h-3.5" />
            <span>Забытые файлы</span>
          </button>
          <button
            onClick={() => setActiveTab("history")}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
              activeTab === "history"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <Clock className="w-3.5 h-3.5" />
            <span>Графики за 10 минут</span>
          </button>
        </div>
      </div>

      {/* Directory Selector Panel */}
      <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-4">
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono flex items-center gap-2">
            <FolderOpen className="w-4 h-4 text-accent" />
            Выбор исследуемого каталога
          </span>
          <span className="text-[11px] text-muted font-mono">
            Анализ читает метаданные • Изменения требуют подтверждения
          </span>
        </div>

        {/* Quick Directory Chips */}
        {quickDirs.length > 0 && (
          <div className="flex flex-wrap gap-2">
            {quickDirs.map((dir) => {
              const isSelected = selectedPath === dir.path && customPath === dir.path;
              return (
                <button
                  key={dir.id}
                  onClick={() => {
                    setSelectedPath(dir.path);
                    setCustomPath(dir.path);
                  }}
                  className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs transition-all border ${
                    isSelected
                      ? "bg-accent/15 border-accent text-accent font-medium"
                      : "bg-surface-2 border-line text-muted hover:text-txt hover:border-line-hover"
                  }`}
                >
                  <Folder className="w-3.5 h-3.5" />
                  <span>{dir.name}</span>
                </button>
              );
            })}
          </div>
        )}

        {/* Custom Path Input and Scan Trigger Buttons */}
        <div className="flex flex-col lg:flex-row items-stretch gap-3">
          <div className="relative flex-1 w-full">
            <input
              type="text"
              aria-label="Путь к папке или диску для анализа"
              value={customPath}
              onChange={(e) => setCustomPath(e.target.value)}
              placeholder="C:\Users\Username..."
              className="w-full bg-surface-2 border border-line rounded-lg px-3.5 py-2 text-xs font-mono text-txt placeholder:text-dim focus:outline-none focus:border-accent"
            />
          </div>

          <button onClick={() => void chooseFolder()} title={!isTauriEnvironment() ? "Выбор папки доступен в установленном приложении" : "Выберите папку, затем отдельно запустите анализ"} disabled={!isTauriEnvironment() || choosingFolder || Boolean(diskTaskId || dupTaskId)} className="shrink-0 border border-line rounded-lg px-3 py-2 text-sm disabled:opacity-40">{choosingFolder ? "Выбор папки…" : "Выбрать папку…"}</button>
          <div className="flex items-center gap-2 w-full sm:w-auto justify-end">
            <button
              onClick={()=>handleStartDiskAnalysis()}
              disabled={Boolean(diskTaskId || dupTaskId || !currentTargetPath)}
              className="flex-1 sm:flex-none flex items-center justify-center gap-2 px-4 py-2 rounded-lg bg-accent text-accent-ink text-xs font-semibold hover:opacity-90 transition-opacity disabled:opacity-40 disabled:cursor-not-allowed shadow-sm"
            >
              <Search className="w-3.5 h-3.5" />
              <span>Анализ диска</span>
            </button>
            <button
              onClick={handleStartDuplicateScan}
              disabled={Boolean(diskTaskId || dupTaskId || !currentTargetPath)}
              className="flex-1 sm:flex-none flex items-center justify-center gap-2 px-4 py-2 rounded-lg bg-surface-2 border border-line text-txt text-xs font-medium hover:bg-surface hover:border-line-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              <Copy className="w-3.5 h-3.5" />
              <span>Поиск дубликатов</span>
            </button>
          </div>
        </div>

        {/* Error message */}
        {errorMessage && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-red-950/30 border border-red-700/50 text-red-300 text-xs font-mono">
            <AlertCircle className="w-4 h-4 shrink-0 text-red-400" />
            <span className="flex-1">{errorMessage}</span>
            <button
              onClick={() => setErrorMessage(null)}
              className="text-red-400 hover:text-red-200"
            >
              <XCircle className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* Active Scan Banner */}
        {(diskTaskId || dupTaskId) && (
          <div className="p-4 rounded-xl bg-surface-2 border border-accent/30 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 rounded-full bg-accent animate-ping" />
                <span className="text-xs font-semibold text-txt">
                  {diskTaskId ? "Выполняется обход структуры каталогов..." : "Выполняется трёхфазный поиск дубликатов..."}
                </span>
              </div>
              <button
                onClick={() => handleCancelActiveTask(diskTaskId || dupTaskId!)}
                className="px-2.5 py-1 rounded bg-red-950/50 border border-red-700/50 text-red-300 hover:text-red-100 text-xs font-mono transition-colors"
              >
                Отмена
              </button>
            </div>

            <p className="text-xs text-muted font-mono">
              {diskTaskId
                ? diskTaskStatus?.progress_message || "Чтение файлов и папок..."
                : dupTaskStatus?.progress_message || "Фаза 1: группировка по точным размерам..."}
            </p>

            <div className="w-full h-1.5 rounded-full bg-surface overflow-hidden">
              <div className="h-full bg-accent animate-pulse w-full" />
            </div>

            <div className="flex items-center justify-between text-[11px] text-dim font-mono">
              <span>
                Обработано объектов:{" "}
                <strong className="text-txt">
                  {(diskTaskId ? diskTaskStatus?.scanned_items : dupTaskStatus?.scanned_items) ?? 0}
                </strong>
              </span>
              <span>
                {diskTaskId
                  ? `Размер: ${formatBytes(diskTaskStatus?.scanned_bytes ?? 0)}`
                  : `Избыточный объём: ${formatBytes(dupTaskStatus?.scanned_bytes ?? 0)}`}
              </span>
            </div>
          </div>
        )}
      </div>

      {/* Main Tab Content */}
      {activeTab === "map" && (
        <div className="space-y-6">
          {diskReport ? (
            <>
              <p className="text-xs text-muted break-all">Отчёт: {diskReport.root_path}</p>
              <div className="text-xs text-muted space-y-1">
                <p>{diskReport.engine === "ntfs-mft" ? "Быстрый анализ NTFS · MFT" : "Обычный обход каталогов"}{diskReport.elapsed_ms !== undefined ? ` · ${(diskReport.elapsed_ms / 1000).toFixed(2)} с` : ""}</p>
                {diskReport.engine_note && <p role="status">Быстрый анализ недоступен: {diskReport.engine_note}</p>}
                {diskReport.allocated_bytes != null && <p>Выделено под данные файлов: {formatBytes(diskReport.allocated_bytes)}. Занято на томе Windows: {formatBytes(diskReport.volume_used_bytes ?? 0)}.</p>}
                {diskReport.engine === "ntfs-mft" && <p>Снимок работающего диска. Жёсткие ссылки учтены один раз; именованные потоки и служебные структуры вне данных файлов в сумму не входят.</p>}
                {!!Object.keys(diskReport.warnings ?? {}).length && <details><summary>Особенности учёта</summary>{Object.entries(diskReport.warnings ?? {}).map(([code, count]) => <p key={code}>{diskSkipReason(code)}: {count}</p>)}</details>}
              </div>
              {!diskReport.complete && <div role="status" className="border border-amber-500/40 rounded-xl p-4 text-sm text-amber-200">Частичный результат. Недоступные каталоги, ссылки или объекты за пределами лимитов пропущены. Показан размер прочитанных файлов, а не всего занятого места.<details className="mt-2"><summary>Причины пропусков</summary>{Object.entries(diskReport.diagnostics).map(([code,count])=><p key={code}>{diskSkipReason(code)}: {count}</p>)}</details></div>}
              {/* Summary Stats Row */}
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
                  <span className="text-[11px] text-muted font-mono uppercase tracking-wider">
                    Общий объём
                  </span>
                  <div className="text-2xl font-bold font-mono text-txt mt-1">
                    {formatBytes(diskReport.total_bytes)}
                  </div>
                  <span className="text-[11px] text-dim font-mono">
                    в каталоге {diskReport.root_path}
                  </span>
                </div>

                <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
                  <span className="text-[11px] text-muted font-mono uppercase tracking-wider">
                    Всего файлов
                  </span>
                  <div className="text-2xl font-bold font-mono text-txt mt-1">
                    {diskReport.total_files.toLocaleString("ru-RU")}
                  </div>
                  <span className="text-[11px] text-dim font-mono">
                    учтено в текущем отчёте
                  </span>
                </div>

                <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
                  <span className="text-[11px] text-muted font-mono uppercase tracking-wider">
                    Каталогов и папок
                  </span>
                  <div className="text-2xl font-bold font-mono text-txt mt-1">
                    {diskReport.total_dirs.toLocaleString("ru-RU")}
                  </div>
                  <span className="text-[11px] text-dim font-mono">
                    иерархическая структура
                  </span>
                </div>
              </div>

              {/* Categorical Breakdown Bar */}
              <DiskTreemap report={diskReport} reportId={sessionStorage.getItem("cleancore-disk-task")} onNavigate={path => void browseDirectory(path)} />
              <details className="border border-line rounded-xl"><summary className="px-5 py-3 text-sm cursor-pointer">Список подпапок · {diskReport.subdirs.length}</summary>
              <section className="bg-surface border border-line rounded-xl p-5 space-y-3" aria-label="Все подпапки">
                <h2 className="font-semibold">Подпапки · {diskReport.subdirs.length}</h2>
                {diskReport.engine === "ntfs-mft" && diskReport.root_path.replace(/\//g, "\\").length > 3 && <button disabled={browsingDirectory} className="text-sm text-accent" onClick={() => {
                  const current = diskReport.root_path.replace(/\//g, "\\").replace(/\\+$/, "");
                  void browseDirectory(current.slice(0, current.lastIndexOf("\\") + 1));
                }}>На уровень выше</button>}
                <p className="text-xs text-muted">Нажмите на папку, чтобы изучить её содержимое.</p>
                {diskReport.subdirs.slice(directoryPage*50,(directoryPage+1)*50).map(dir=><button key={dir.path} disabled={browsingDirectory||!!diskTaskId||!!dupTaskId} onClick={()=>void browseDirectory(dir.path)} className="w-full flex justify-between gap-4 text-left border-b border-line py-2 text-sm"><span className="truncate">{dir.name}</span><span className="shrink-0 font-mono">{formatBytes(dir.size_bytes)}</span></button>)}
                <div className="flex gap-3 items-center text-xs"><button disabled={!directoryPage} onClick={()=>setDirectoryPage(p=>p-1)}>Назад</button><span>Страница {directoryPage+1} из {Math.max(1,Math.ceil(diskReport.subdirs.length/50))}</span><button disabled={(directoryPage+1)*50>=diskReport.subdirs.length} onClick={()=>setDirectoryPage(p=>p+1)}>Далее</button></div>
              </section>
              </details>
              <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-4">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono flex items-center gap-2">
                    <Layers className="w-4 h-4 text-accent" />
                    Карта распределения по типам файлов
                  </span>
                  <span className="text-[11px] text-dim font-mono">
                    {diskReport.categories.length} категорий
                  </span>
                </div>

                {/* Multi-segment Colored Bar */}
                <div className="w-full h-4 rounded-lg bg-surface-2 overflow-hidden flex shadow-inner">
                  {diskReport.categories
                    .filter((cat) => cat.percentage > 0)
                    .map((cat) => {
                      const style = CATEGORY_COLORS[cat.category] || CATEGORY_COLORS.Other;
                      return (
                        <div
                          key={cat.category}
                          className={`${style.bg} transition-all duration-300 hover:brightness-125`}
                          style={{ width: `${Math.max(cat.percentage, 0.5)}%` }}
                          title={`${cat.display_name}: ${formatBytes(cat.total_bytes)} (${cat.percentage.toFixed(1)}%)`}
                        />
                      );
                    })}
                </div>

                {/* Category Legend Grid */}
                <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3 pt-2">
                  {diskReport.categories.map((cat) => {
                    return (
                      <div
                        key={cat.category}
                        className="bg-surface-2 border border-line rounded-lg p-2.5 flex items-start gap-2.5"
                      >
                        <div className="p-1.5 rounded bg-surface border border-line shrink-0">
                          {getCategoryIcon(cat.category)}
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center justify-between text-xs">
                            <span className="font-medium text-txt truncate">
                              {cat.display_name}
                            </span>
                            <span className="font-mono text-muted text-[11px] ml-1">
                              {cat.percentage.toFixed(1)}%
                            </span>
                          </div>
                          <div className="flex items-center justify-between text-[11px] font-mono text-dim mt-0.5">
                            <span className="text-accent font-semibold">{formatBytes(cat.total_bytes)}</span>
                            <span>{cat.file_count} шт</span>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Two Column Layout: Largest Files and Largest Directories */}
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                {/* Largest Files Table */}
                <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono flex items-center gap-2">
                      <File className="w-4 h-4 text-accent" />
                      Крупнейшие файлы
                    </span>
                    <span className="text-[11px] text-muted font-mono">
                      Топ-{diskReport.largest_files.length} потребителей
                    </span>
                  </div>

                  <div className="divide-y divide-line">
                    {diskReport.largest_files.map((file, idx) => (
                      <div
                        key={file.path + idx}
                        className="py-2.5 flex items-center justify-between gap-3 text-xs hover:bg-surface-2/50 px-2 rounded transition-colors"
                      >
                        <div className="flex items-center gap-2.5 min-w-0 flex-1">
                          {getCategoryIcon(file.category)}
                          <div className="min-w-0 flex-1">
                            <div className="font-medium text-txt truncate" title={file.name}>
                              {file.name}
                            </div>
                            <div className="text-[10px] text-dim font-mono truncate" title={file.path}>
                              {file.path}
                            </div>
                          </div>
                        </div>
                        <div className="text-right shrink-0">
                          <div className="font-mono font-semibold text-txt">
                            {formatBytes(file.size_bytes)}
                          </div>
                          <div className="text-[10px] text-muted font-mono">
                            {file.percentage.toFixed(1)}%
                          </div>
                        </div>
                      </div>
                    ))}
                    {diskReport.largest_files.length === 0 && (
                      <p className="text-xs text-muted py-4 text-center">Файлы не обнаружены</p>
                    )}
                  </div>
                </div>

                {/* Largest Directories Table */}
                <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono flex items-center gap-2">
                      <Folder className="w-4 h-4 text-accent" />
                      Крупнейшие папки
                    </span>
                    <span className="text-[11px] text-muted font-mono">
                      Топ-{diskReport.largest_dirs.length} подпапок
                    </span>
                  </div>

                  <div className="divide-y divide-line">
                    {diskReport.largest_dirs.map((dir, idx) => (
                      <div
                        key={dir.path + idx}
                        className="py-2.5 flex items-center justify-between gap-3 text-xs hover:bg-surface-2/50 px-2 rounded transition-colors"
                      >
                        <div className="flex items-center gap-2.5 min-w-0 flex-1">
                          <Folder className="w-4 h-4 text-amber-400 shrink-0" />
                          <div className="min-w-0 flex-1">
                            <div className="font-medium text-txt truncate" title={dir.name}>
                              {dir.name}
                            </div>
                            <div className="text-[10px] text-dim font-mono truncate" title={dir.path}>
                              {dir.path}
                            </div>
                          </div>
                        </div>
                        <div className="text-right shrink-0">
                          <div className="font-mono font-semibold text-txt">
                            {formatBytes(dir.size_bytes)}
                          </div>
                          <div className="text-[10px] text-muted font-mono">
                            {dir.file_count} файлов • {dir.percentage.toFixed(1)}%
                          </div>
                        </div>
                      </div>
                    ))}
                    {diskReport.largest_dirs.length === 0 && (
                      <p className="text-xs text-muted py-4 text-center">Подпапки не обнаружены</p>
                    )}
                  </div>
                </div>
              </div>
            </>
          ) : (
            <div className="bg-surface border border-line rounded-xl p-12 text-center space-y-3">
              <div className="w-12 h-12 rounded-full bg-surface-2 border border-line flex items-center justify-center text-muted mx-auto">
                <Search className="w-6 h-6" />
              </div>
              <h3 className="font-heading text-base font-semibold text-txt">
                Анализ структуры каталога ещё не проведён
              </h3>
              <p className="text-xs text-muted max-w-md mx-auto">
                Выберите стандартную папку или укажите свой путь выше и нажмите «Анализ диска», чтобы увидеть карту распределения и крупнейшие файлы.
              </p>
            </div>
          )}
        </div>
      )}

      {/* Duplicates Tab */}
      {activeTab === "duplicates" && (
        <div className="space-y-6">
          {/* Security Rules Box */}
          <div className="bg-surface border border-line rounded-xl p-4 flex items-start gap-3">
            <ShieldCheck className="w-5 h-5 text-accent shrink-0 mt-0.5" />
            <div className="text-xs space-y-1">
              <span className="font-semibold text-txt">
                Режим безопасной инспекции дубликатов (Czkawka 3-Phase Logic)
              </span>
              <p className="text-muted leading-relaxed">
                Поиск выполняется строго в три фазы: группировка по размеру, затем прехеш первых 16 КБ, затем полный SHA-256. Жесткие ссылки (Hard links) исключаются и не считаются дубликатами. В соответствии с правилами безопасности CleanCore <strong>автоматическое удаление отключено</strong>.
              </p>
            </div>
          </div>

          {dupReport ? (
            <>
              {!!dupReport.cached_hashes && <p role="status" className="text-xs text-muted">Из кеша анализа: {dupReport.cached_hashes} хешей. Кеш сопоставляется по идентичности, размеру, времени создания/изменения и предварительному хешу. Перед удалением содержимое всегда проверяется заново.</p>}
              {/* Duplicates Summary Stats */}
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
                  <span className="text-[11px] text-muted font-mono uppercase tracking-wider">
                    Оценка объёма копий
                  </span>
                  <div className="text-2xl font-bold font-mono text-amber-400 mt-1">
                    {formatBytes(dupReport.total_wasted_bytes)}
                  </div>
                  <span className="text-[11px] text-dim font-mono">
                    потенциально высвобождаемый объём
                  </span>
                </div>

                <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
                  <span className="text-[11px] text-muted font-mono uppercase tracking-wider">
                    Групп совпадений SHA-256
                  </span>
                  <div className="text-2xl font-bold font-mono text-txt mt-1">
                    {dupReport.groups.length}
                  </div>
                  <span className="text-[11px] text-dim font-mono">
                    полные хеши, включая проверенный по метаданным кеш
                  </span>
                </div>

                <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
                  <span className="text-[11px] text-muted font-mono uppercase tracking-wider">
                    Всего файлов-копий
                  </span>
                  <div className="text-2xl font-bold font-mono text-txt mt-1">
                    {dupReport.total_duplicate_files}
                  </div>
                  <span className="text-[11px] text-dim font-mono">
                    участвуют в группах дубликатов
                  </span>
                </div>
              </div>

              {/* Duplicate Groups List */}
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono">
                    Группы обнаруженных копий
                  </span>
                  <span className="text-[11px] text-dim font-mono">
                    Отсортировано по размеру
                  </span>
                </div>

                {dupReport.groups.length > 0 ? (
                  dupReport.groups.map((group, gIdx) => (
                    <div
                      key={group.group_id}
                      className="bg-surface border border-line rounded-xl p-4 shadow-sm space-y-3"
                    >
                      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line pb-2.5">
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-mono font-semibold text-txt bg-surface-2 px-2 py-0.5 rounded border border-line">
                            Группа #{gIdx + 1}
                          </span>
                          <span className="text-xs text-muted">
                            Размер копии:{" "}
                            <strong className="text-txt font-mono">
                              {formatBytes(group.file_size_bytes)}
                            </strong>
                          </span>
                          <span className="text-xs text-amber-400 font-mono font-semibold">
                            (перерасход {formatBytes(group.reclaimable_bytes)})
                          </span>
                        </div>
                        <div className="text-[11px] font-mono text-dim truncate max-w-xs" title={group.full_hash}>
                          SHA-256: {group.full_hash.slice(0, 16)}...
                        </div>
                      </div>

                      {/* File Items in Group */}
                      <div className="space-y-2">
                        {group.files.map((file, fIdx) => (
                          <div
                            key={file.path + fIdx}
                            className="bg-surface-2 border border-line/60 rounded-lg p-2.5 flex items-center justify-between gap-3 text-xs"
                          >
                            <div className="flex items-center gap-2 min-w-0 flex-1">
                              <File className="w-3.5 h-3.5 text-accent shrink-0" />
                              <span className="font-mono text-txt truncate" title={file.path}>
                                {file.path}
                              </span>
                            </div>
                            <span className="text-[11px] text-dim font-mono shrink-0">
                              {file.modified_utc.replace("T", " ").slice(0, 19)} UTC
                            </span>
                          </div>
                        ))}
                      </div>
                      <DuplicateActions group={group} reportId={sessionStorage.getItem("cleancore-dup-task")} />
                    </div>
                  ))
                ) : (
                  <div className="bg-surface border border-line rounded-xl p-8 text-center space-y-2">
                    <CheckCircle2 className="w-8 h-8 text-emerald-400 mx-auto" />
                    <h4 className="font-semibold text-txt text-sm">
                      Дубликатов не обнаружено
                    </h4>
                    <p className="text-xs text-muted">
                      Все файлы в каталоге имеют уникальные размеры или хеши.
                    </p>
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="bg-surface border border-line rounded-xl p-12 text-center space-y-3">
              <div className="w-12 h-12 rounded-full bg-surface-2 border border-line flex items-center justify-center text-muted mx-auto">
                <Copy className="w-6 h-6" />
              </div>
              <h3 className="font-heading text-base font-semibold text-txt">
                Поиск дубликатов ещё не запускался
              </h3>
              <p className="text-xs text-muted max-w-md mx-auto">
                Нажмите «Поиск дубликатов», чтобы запустить поиск файлов с одинаковым содержимым в выбранном каталоге.
              </p>
            </div>
          )}
        </div>
      )}

      {/* Space Hogs Tab */}
      {activeTab === "space_hogs" && (
        <SpaceHogsTable />
      )}

      {/* Stale Files Tab */}
      {activeTab === "stale" && (
        <StaleFilesView defaultPath={selectedPath || customPath} />
      )}

      {/* Resource History Tab */}
      {activeTab === "history" && (
        <div className="space-y-6">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-heading text-sm font-semibold text-txt flex items-center gap-2">
                <Activity className="w-4 h-4 text-accent" />
                Динамика системных ресурсов за последние 10 минут
              </h3>
              <p className="text-xs text-muted mt-0.5">
                Кольцевой буфер в памяти процесса, отображающий долгосрочные тренды нагрузки CPU, RAM и сети
              </p>
            </div>
            <button
              onClick={loadHistory}
              className="px-3 py-1.5 rounded-lg bg-surface border border-line text-xs font-mono text-txt hover:bg-surface-2 transition-colors flex items-center gap-1.5"
            >
              <TrendingUp className="w-3.5 h-3.5 text-accent" />
              <span>Обновить</span>
            </button>
          </div>

          {history && history.timestamps_utc.length > 0 ? (
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
              {/* CPU History Card */}
              <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono">
                    Загрузка CPU (%)
                  </span>
                  <span className="text-xs font-mono font-bold text-accent">
                    {history.cpu_percentages.at(-1)?.toFixed(1) ?? "нет данных"}%
                  </span>
                </div>
                <div className="h-32 w-full flex items-end gap-1 pt-4 pb-2 border-b border-line">
                  {renderSparkline(history.cpu_percentages, 100, "#0ea5e9")}
                </div>
                <div className="flex items-center justify-between text-[11px] font-mono text-dim">
                  <span>Мин: {historyStatistic(history.cpu_percentages, "min")}%</span>
                  <span>
                    Ср: {historyStatistic(history.cpu_percentages, "mean")}%
                  </span>
                  <span>Макс: {historyStatistic(history.cpu_percentages, "max")}%</span>
                </div>
              </div>

              {/* Memory History Card */}
              <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono">
                    Занято RAM (%)
                  </span>
                  <span className="text-xs font-mono font-bold text-emerald-400">
                    {history.memory_percentages.at(-1)?.toFixed(1) ?? "нет данных"}%
                  </span>
                </div>
                <div className="h-32 w-full flex items-end gap-1 pt-4 pb-2 border-b border-line">
                  {renderSparkline(history.memory_percentages, 100, "#10b981")}
                </div>
                <div className="flex items-center justify-between text-[11px] font-mono text-dim">
                  <span>Мин: {historyStatistic(history.memory_percentages, "min")}%</span>
                  <span>
                    Ср: {historyStatistic(history.memory_percentages, "mean")}%
                  </span>
                  <span>Макс: {historyStatistic(history.memory_percentages, "max")}%</span>
                </div>
              </div>

              {/* Network History Card */}
              <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono">
                    Сетевой трафик (RX)
                  </span>
                  <span className="text-xs font-mono font-bold text-purple-400">
                    {historySpeed(history.network_rx_bytes_sec.at(-1))}/с
                  </span>
                </div>
                <div className="h-32 w-full flex items-end gap-1 pt-4 pb-2 border-b border-line">
                  {renderSparkline(
                    history.network_rx_bytes_sec,
                    Math.max(...history.network_rx_bytes_sec.filter(isNumber), 1024),
                    "#a855f7"
                  )}
                </div>
                <div className="flex items-center justify-between text-[11px] font-mono text-dim">
                  <span>
                    Пик: {history.network_rx_bytes_sec.some(isNumber) ? formatBytes(Math.max(...history.network_rx_bytes_sec.filter(isNumber))) : "нет данных"}/с
                  </span>
                  <span>
                    TX: {historySpeed(history.network_tx_bytes_sec.at(-1))}/с
                  </span>
                </div>
              </div>
            </div>
          ) : (
            <div className="bg-surface border border-line rounded-xl p-8 text-center text-muted text-xs">
              Накопление замеров в кольцевом буфере... Подождите несколько секунд.
            </div>
          )}
        </div>
      )}
    </div>
  );
};

function renderSparkline(values: (number | null)[], max: number, color: string) {
  if (values.length === 0) return null;
  const width = 360;
  const height = 110;
  const step = width / Math.max(values.length - 1, 1);

  let connected = false;
  const path = values.map((v, i) => {
    if (v === null || !Number.isFinite(v)) { connected = false; return ""; }
    const x = i * step;
    const y = height - (Math.min(v / max, 1) * (height - 10) + 5);
    const command = connected ? "L" : "M";
    connected = true;
    return `${command} ${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ");

  return (
    <svg className="w-full h-full overflow-visible" viewBox={`0 0 ${width} ${height}`}>
      <path
        d={path}
        fill="none"
        stroke={color}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function isNumber(value: number | null): value is number { return value !== null && Number.isFinite(value); }
function historySpeed(value: number | null | undefined): string { return value == null ? "нет данных" : formatBytes(value); }
function historyStatistic(values: (number | null)[], statistic: "min" | "max" | "mean"): string {
  const measured = values.filter(isNumber);
  if (!measured.length) return "нет данных";
  const result = statistic === "min" ? Math.min(...measured) : statistic === "max" ? Math.max(...measured) : measured.reduce((a, b) => a + b, 0) / measured.length;
  return result.toFixed(1);
}


function diskSkipReason(code:string):string {
 const reasons:Record<string,string>={ACCESS_DENIED:"Нет доступа",FILE_IN_USE:"Каталог занят",ACCESS_OR_READ_DENIED:"Каталог недоступен",READ_ENTRY_FAILED:"Не удалось прочитать объект",METADATA_UNAVAILABLE:"Нет сведений о файле",LINK_SKIPPED:"Ссылка или точка перенаправления",DEPTH_LIMIT:"Достигнут предел глубины",ENTRY_LIMIT:"Достигнут предел числа объектов",QUEUE_LIMIT:"Достигнут предел очереди",SYSTEM_RECORD_WITHOUT_NAME:"Внутренние записи NTFS без имени",MFT_NAME_UNAVAILABLE:"Имя объекта MFT недоступно",MFT_INVALID_RECORD:"Повреждённая или изменившаяся запись MFT",MFT_READ_FAILED:"Ошибка чтения части MFT",MFT_MEMORY_LIMIT:"Достигнут предел памяти индекса",MFT_STALE_PARENT:"Устаревшая связь с родительским каталогом",MFT_UNREACHABLE:"Объекты с недоступной цепочкой каталогов",MFT_STALE_EXTENSION:"Устаревшее расширение записи",MFT_DATA_UNAVAILABLE:"Размер данных недоступен",MFT_DUPLICATE_DATA:"Противоречивые атрибуты данных",MFT_INVALID_PARENT:"Недопустимая связь каталогов",MFT_PARENT_CYCLE:"Цикл в связях каталогов",MFT_PATH_TOO_DEEP:"Слишком длинная цепочка каталогов",HARD_LINK_COUNTED_ONCE:"Объекты с несколькими именами: учтены один раз",NAMED_STREAMS_EXCLUDED:"Файлы с именованными потоками: потоки не включены",EXTENDED_ATTRIBUTES:"Файлы с расширениями записей MFT",CANCELLED:"Остановлено пользователем",REPARSE_POINT:"Точка перенаправления пропущена"};
 return reasons[code] || code;
}
