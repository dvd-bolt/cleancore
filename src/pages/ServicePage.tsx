import React, { useState, useEffect, useMemo, useCallback } from "react";
import {
  ActivePage,
  AntivirusStatus,
  CsvExportConfig,
  CsvExportSection,
  DiagnosticsSummary,
  InstalledProgram,
  LaunchResult,
  StartupItem,
  SystemToolId,
} from "../types";
import {
  getServicePrograms,
  launchServiceUninstaller,
  getServiceStartup,
  getServiceAntivirus,
  getServiceDiagnostics,
  launchServiceTool,
  exportServiceCsv,
  isTauriEnvironment,
} from "../lib/ipc";
import {
  Package,
  ShieldCheck,
  ShieldAlert,
  Wrench,
  ExternalLink,
  Search,
  RefreshCw,
  FileSpreadsheet,
  Trash2,
  AlertCircle,
  CheckCircle2,
  XCircle,
  Info,
  Copy,
  Check,
  Activity,
  Cpu,
  HardDrive,
  Layers,
  Network,
  X,
  ChevronUp,
  ChevronDown,
  Lightbulb,
} from "lucide-react";

export type ServiceTab = "programs" | "startup" | "diagnostics" | "tools";

export interface ServicePageProps {
  onNavigate?: (page: ActivePage) => void;
  initialTab?: ServiceTab;
  initialPrograms?: InstalledProgram[];
  initialStartup?: StartupItem[];
  initialAntivirus?: AntivirusStatus;
  initialDiagnostics?: DiagnosticsSummary;
  initialLoading?: boolean;
  initialSearch?: string;
  initialFilterArch?: "all" | "64" | "32" | "sized";
  initialStartupSourceFilter?: string;
  initialUninstallerProg?: InstalledProgram | null;
  initialUninstallerAcknowledged?: boolean;
  initialExportModalOpen?: boolean;
  initialFeedbackMessage?: { type: "success" | "error" | "info"; text: string } | null;
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || isNaN(bytes)) return "—";
  if (bytes === 0) return "0 Б";
  const k = 1024;
  const sizes = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(val >= 100 || i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export function formatInstallDate(dateStr: string | null | undefined): string {
  if (!dateStr) return "—";
  const cleaned = dateStr.trim();
  if (/^\d{8}$/.test(cleaned)) {
    const y = cleaned.substring(0, 4);
    const m = cleaned.substring(4, 6);
    const d = cleaned.substring(6, 8);
    return `${d}.${m}.${y}`;
  }
  return cleaned;
}

interface WindowsToolDef {
  id: SystemToolId;
  name: string;
  binary: string;
  executablePath: string;
  description: string;
  icon: React.ReactNode;
}

const WINDOWS_TOOLS: WindowsToolDef[] = [
  {
    id: "cleanmgr",
    name: "Очистка диска",
    binary: "cleanmgr.exe",
    executablePath: "C:\\Windows\\System32\\cleanmgr.exe",
    description: "Штатная утилита Windows для безопасного удаления временных файлов, кэша обновлений и старых файлов ОС.",
    icon: <HardDrive className="w-5 h-5 text-sky-400" />,
  },
  {
    id: "dfrgui",
    name: "Дефрагментация и оптимизация",
    binary: "dfrgui.exe",
    executablePath: "C:\\Windows\\System32\\dfrgui.exe",
    description: "Оптимизация накопителей: TRIM для SSD и дефрагментация кластеров для жестких дисков (HDD).",
    icon: <Layers className="w-5 h-5 text-purple-400" />,
  },
  {
    id: "resmon",
    name: "Монитор ресурсов",
    binary: "resmon.exe",
    executablePath: "C:\\Windows\\System32\\resmon.exe",
    description: "Глубокий системный мониторинг нагрузки ЦП, дисковых очередей ввода-вывода, памяти и сетевых соединений.",
    icon: <Activity className="w-5 h-5 text-emerald-400" />,
  },
  {
    id: "taskmgr",
    name: "Диспетчер задач",
    binary: "Taskmgr.exe",
    executablePath: "C:\\Windows\\System32\\Taskmgr.exe",
    description: "Штатный диспетчер процессов, системных служб, производительности и управления автозагрузкой.",
    icon: <Cpu className="w-5 h-5 text-amber-400" />,
  },
  {
    id: "appwiz",
    name: "Программы и компоненты",
    binary: "control.exe appwiz.cpl",
    executablePath: "C:\\Windows\\System32\\control.exe appwiz.cpl",
    description: "Классический апплет панели управления Windows для удаления и изменения программ и компонентов системы.",
    icon: <Package className="w-5 h-5 text-blue-400" />,
  },
];

export const ServicePage: React.FC<ServicePageProps> = ({
  onNavigate,
  initialTab = "programs",
  initialPrograms,
  initialStartup,
  initialAntivirus,
  initialDiagnostics,
  initialLoading,
  initialSearch = "",
  initialFilterArch = "all",
  initialStartupSourceFilter = "all",
  initialUninstallerProg = null,
  initialUninstallerAcknowledged = false,
  initialExportModalOpen = false,
  initialFeedbackMessage = null,
}) => {
  const [activeTab, setActiveTab] = useState<ServiceTab>(initialTab);

  // Данные модулей 6.1–6.4
  const [programs, setPrograms] = useState<InstalledProgram[]>(initialPrograms ?? []);
  const [startupItems, setStartupItems] = useState<StartupItem[]>(initialStartup ?? []);
  const [antivirus, setAntivirus] = useState<AntivirusStatus | null>(initialAntivirus ?? null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticsSummary | null>(initialDiagnostics ?? null);

  // Состояния загрузки и ошибок
  const [loading, setLoading] = useState<boolean>(
    initialLoading !== undefined ? initialLoading : (initialPrograms ? false : true)
  );
  const [refreshing, setRefreshing] = useState<boolean>(false);
  const [feedbackMessage, setFeedbackMessage] = useState<{ type: "success" | "error" | "info"; text: string } | null>(
    initialFeedbackMessage
  );

  // Вкладка 1: Программы — поиск, сортировка, фильтрация
  const [progSearch, setProgSearch] = useState<string>(initialSearch);
  const [progFilterArch, setProgFilterArch] = useState<"all" | "64" | "32" | "sized">(initialFilterArch);
  const [progSortField, setProgSortField] = useState<"name" | "publisher" | "version" | "date" | "size">("name");
  const [progSortAsc, setProgSortAsc] = useState<boolean>(true);

  // Модальное окно деинсталлятора
  const [uninstallerModalProg, setUninstallerModalProg] = useState<InstalledProgram | null>(initialUninstallerProg);
  const [uninstallerAcknowledged, setUninstallerAcknowledged] = useState<boolean>(initialUninstallerAcknowledged);
  const [uninstallerBusy, setUninstallerBusy] = useState<boolean>(false);
  const [uninstallerCopied, setUninstallerCopied] = useState<boolean>(false);

  // Вкладка 2: Автозагрузка — поиск, фильтр источника
  const [startupSearch, setStartupSearch] = useState<string>("");
  const [startupSourceFilter, setStartupSourceFilter] = useState<string>(initialStartupSourceFilter);

  // Вкладка 4: Запуск утилит
  const [runningToolId, setRunningToolId] = useState<SystemToolId | null>(null);

  // Модальное окно экспорта в CSV
  const [exportModalOpen, setExportModalOpen] = useState<boolean>(initialExportModalOpen);
  const [exportSections, setExportSections] = useState<{
    programs: boolean;
    startup: boolean;
    diagnostics: boolean;
  }>({ programs: true, startup: true, diagnostics: true });
  const [exportIncludeSystem, setExportIncludeSystem] = useState<boolean>(false);
  const [exportCustomPath, setExportCustomPath] = useState<string>("");
  const [exportBusy, setExportBusy] = useState<boolean>(false);
  const [exportResult, setExportResult] = useState<string | null>(null);

  // Загрузка всех данных
  const loadData = useCallback(async (isRefresh = false) => {
    if (isRefresh) setRefreshing(true);
    else setLoading(true);

    try {
      const [progsRes, startupRes, avRes, diagRes] = await Promise.all([
        getServicePrograms(),
        getServiceStartup(),
        getServiceAntivirus(),
        getServiceDiagnostics(),
      ]);

      setPrograms(progsRes);
      setStartupItems(startupRes);
      setAntivirus(avRes);
      setDiagnostics(diagRes);
    } catch (err) {
      setFeedbackMessage({
        type: "error",
        text: `Ошибка загрузки данных обслуживания: ${err instanceof Error ? err.message : String(err)}`,
      });
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    if (initialPrograms || initialStartup || initialAntivirus || initialDiagnostics) {
      return;
    }
    loadData();
  }, [loadData, initialPrograms, initialStartup, initialAntivirus, initialDiagnostics]);

  // Сортировка и фильтрация программ
  const filteredPrograms = useMemo(() => {
    let list = [...programs];

    // Поиск
    if (progSearch.trim()) {
      const query = progSearch.toLowerCase().trim();
      list = list.filter((p) => {
        const id = p.id.toLowerCase();
        const name = p.display_name.toLowerCase();
        const pub = (p.publisher || "").toLowerCase();
        const ver = (p.display_version || "").toLowerCase();
        return name.includes(query) || pub.includes(query) || ver.includes(query) || id.includes(query);
      });
    }

    // Архитектура / размер
    if (progFilterArch === "64") {
      list = list.filter((p) => !p.is_wow64);
    } else if (progFilterArch === "32") {
      list = list.filter((p) => p.is_wow64);
    } else if (progFilterArch === "sized") {
      list = list.filter((p) => p.estimated_size_bytes && p.estimated_size_bytes > 0);
    }

    // Сортировка
    list.sort((a, b) => {
      let cmp = 0;
      switch (progSortField) {
        case "name":
          cmp = a.display_name.localeCompare(b.display_name);
          break;
        case "publisher":
          cmp = (a.publisher || "").localeCompare(b.publisher || "");
          break;
        case "version":
          cmp = (a.display_version || "").localeCompare(b.display_version || "");
          break;
        case "date":
          cmp = (a.install_date || "").localeCompare(b.install_date || "");
          break;
        case "size":
          cmp = (a.estimated_size_bytes || 0) - (b.estimated_size_bytes || 0);
          break;
      }
      return progSortAsc ? cmp : -cmp;
    });

    return list;
  }, [programs, progSearch, progFilterArch, progSortField, progSortAsc]);

  // Статистика программ
  const progStats = useMemo(() => {
    const total = programs.length;
    const totalBytes = programs.reduce((acc, p) => acc + (p.estimated_size_bytes || 0), 0);
    const count64 = programs.filter((p) => !p.is_wow64).length;
    const count32 = programs.filter((p) => p.is_wow64).length;
    return { total, totalBytes, count64, count32 };
  }, [programs]);

  // Фильтрация автозагрузки
  const filteredStartup = useMemo(() => {
    let list = [...startupItems];

    if (startupSearch.trim()) {
      const q = startupSearch.toLowerCase().trim();
      list = list.filter((item) => item.name.toLowerCase().includes(q) || item.command.toLowerCase().includes(q));
    }

    if (startupSourceFilter !== "all") {
      list = list.filter((item) => item.source === startupSourceFilter);
    }

    return list;
  }, [startupItems, startupSearch, startupSourceFilter]);

  // Статистика автозагрузки
  const startupStats = useMemo(() => {
    const total = startupItems.length;
    const enabled = startupItems.filter((i) => i.enabled).length;
    const missing = startupItems.filter((i) => !i.target_exists).length;
    return { total, enabled, missing };
  }, [startupItems]);

  // Запуск штатного деинсталлятора
  const handleLaunchUninstaller = async () => {
    if (!uninstallerModalProg || !uninstallerAcknowledged || uninstallerBusy) return;

    setUninstallerBusy(true);
    try {
      const res: LaunchResult = await launchServiceUninstaller(uninstallerModalProg.id);
      setFeedbackMessage({
        type: res.success ? "success" : "error",
        text: res.message,
      });
      setUninstallerModalProg(null);
      setUninstallerAcknowledged(false);
    } catch (err) {
      setFeedbackMessage({
        type: "error",
        text: `Сбой запуска деинсталлятора: ${err instanceof Error ? err.message : String(err)}`,
      });
    } finally {
      setUninstallerBusy(false);
    }
  };

  // Запуск утилиты Windows
  const handleLaunchTool = async (toolId: SystemToolId) => {
    setRunningToolId(toolId);
    try {
      const res: LaunchResult = await launchServiceTool(toolId);
      setFeedbackMessage({
        type: res.success ? "success" : "error",
        text: res.message,
      });
    } catch (err) {
      setFeedbackMessage({
        type: "error",
        text: `Сбой запуска утилиты: ${err instanceof Error ? err.message : String(err)}`,
      });
    } finally {
      setRunningToolId(null);
    }
  };

  // Экспорт CSV
  const handleExportCsv = async () => {
    const selectedSections: CsvExportSection[] = [];
    if (exportSections.programs) selectedSections.push("programs");
    if (exportSections.startup) selectedSections.push("startup");
    if (exportSections.diagnostics) selectedSections.push("diagnostics");

    if (selectedSections.length === 0) {
      setFeedbackMessage({
        type: "error",
        text: "Выберите хотя бы один раздел для экспорта в CSV.",
      });
      return;
    }

    setExportBusy(true);
    setExportResult(null);

    const config: CsvExportConfig = {
      sections: selectedSections,
      include_system_components: exportIncludeSystem,
      target_path: exportCustomPath.trim() || null,
    };

    try {
      const csvData = await exportServiceCsv(config);
      setExportResult(csvData);

      // В не-Tauri режиме (браузер) инициируем скачивание файла
      if (!isTauriEnvironment() && typeof window !== "undefined") {
        const blob = new Blob([csvData], { type: "text/csv;charset=utf-8;" });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.setAttribute("href", url);
        link.setAttribute("download", `cleancore_service_report_${Date.now()}.csv`);
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
      }

      setFeedbackMessage({
        type: "success",
        text: "Отчёт Центра обслуживания успешно сформирован и экранирован от CSV-инъекций.",
      });
    } catch (err) {
      setFeedbackMessage({
        type: "error",
        text: `Ошибка экспорта CSV: ${err instanceof Error ? err.message : String(err)}`,
      });
    } finally {
      setExportBusy(false);
    }
  };

  const toggleSort = (field: "name" | "publisher" | "version" | "date" | "size") => {
    if (progSortField === field) {
      setProgSortAsc(!progSortAsc);
    } else {
      setProgSortField(field);
      setProgSortAsc(true);
    }
  };

  return (
    <div className="space-y-6">
      {/* Верхняя шапка Центра обслуживания */}
      <div className="bg-shell border border-line rounded-2xl p-6 shadow-sm">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-surface border border-line flex items-center justify-center text-accent">
                <Wrench className="w-5 h-5 text-accent" />
              </div>
              <div>
                <div className="flex items-center gap-2">
                  <h1 className="font-heading text-xl font-bold tracking-wide text-txt">
                    Центр обслуживания
                  </h1>
                  <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-surface border border-line text-accent font-semibold">
                    v0.3 • МОДУЛИ 6.1–6.6
                  </span>
                </div>
                <p className="text-xs text-muted mt-0.5">
                  Инвентаризация ПО, аудит автозагрузки, безопасность Defender, диагностика и штатные утилиты Windows
                </p>
              </div>
            </div>
          </div>

          {/* Кнопки управления и Экспорт */}
          <div className="flex items-center gap-2.5">
            <button
              onClick={() => setExportModalOpen(true)}
              className="flex items-center gap-2 px-3.5 py-2 rounded-xl bg-surface border border-line text-xs font-medium text-txt hover:bg-surface-2 transition-all shadow-sm"
              title="Сформировать отчёт CSV со списком программ, автозагрузки и метрик"
            >
              <FileSpreadsheet className="w-4 h-4 text-emerald-400" />
              <span>Экспорт отчёта (CSV)</span>
            </button>

            <button
              onClick={() => loadData(true)}
              disabled={refreshing || loading}
              className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-surface border border-line text-xs font-medium text-muted hover:text-txt hover:bg-surface-2 transition-all disabled:opacity-50"
              title="Обновить сведения обслуживания"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${refreshing ? "animate-spin text-accent" : ""}`} />
              <span>Обновить</span>
            </button>
          </div>
        </div>

        {/* Переключатель 4 вкладок */}
        <div className="flex items-center gap-1.5 mt-6 pt-5 border-t border-line/60 overflow-x-auto">
          <button
            onClick={() => setActiveTab("programs")}
            className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-medium transition-all ${
              activeTab === "programs"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <Package className="w-4 h-4" />
            <span>1. Программы</span>
            <span
              className={`text-[10px] font-mono px-1.5 py-0.2 rounded ${
                activeTab === "programs" ? "bg-accent-ink/10 text-accent-ink font-bold" : "bg-surface border border-line text-dim"
              }`}
            >
              {programs.length}
            </span>
          </button>

          <button
            onClick={() => setActiveTab("startup")}
            className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-medium transition-all ${
              activeTab === "startup"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <Activity className="w-4 h-4" />
            <span>2. Автозагрузка</span>
            <span
              className={`text-[10px] font-mono px-1.5 py-0.2 rounded ${
                activeTab === "startup" ? "bg-accent-ink/10 text-accent-ink font-bold" : "bg-surface border border-line text-dim"
              }`}
            >
              {startupItems.length}
            </span>
          </button>

          <button
            onClick={() => setActiveTab("diagnostics")}
            className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-medium transition-all ${
              activeTab === "diagnostics"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <ShieldCheck className="w-4 h-4" />
            <span>3. Защита и здоровье</span>
            {diagnostics && (
              <span
                className={`text-[10px] font-mono px-1.5 py-0.2 rounded ${
                  diagnostics.health === "healthy"
                    ? "bg-emerald-950/40 text-emerald-400 border border-emerald-800/40"
                    : diagnostics.health === "warning"
                    ? "bg-amber-950/40 text-amber-400 border border-amber-800/40"
                    : "bg-red-950/40 text-red-400 border border-red-800/40"
                }`}
              >
                {diagnostics.health === "healthy" ? "Норма" : diagnostics.health === "warning" ? "Внимание" : "Критично"}
              </span>
            )}
          </button>

          <button
            onClick={() => setActiveTab("tools")}
            className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-medium transition-all ${
              activeTab === "tools"
                ? "bg-accent text-accent-ink font-semibold shadow-sm"
                : "text-muted hover:text-txt hover:bg-surface-2"
            }`}
          >
            <ExternalLink className="w-4 h-4" />
            <span>4. Инструменты Windows</span>
            <span
              className={`text-[10px] font-mono px-1.5 py-0.2 rounded ${
                activeTab === "tools" ? "bg-accent-ink/10 text-accent-ink font-bold" : "bg-surface border border-line text-dim"
              }`}
            >
              {WINDOWS_TOOLS.length}
            </span>
          </button>
        </div>
      </div>

      {/* Информационный тост / баннер уведомлений */}
      {feedbackMessage && (
        <div
          role="status"
          className={`flex items-center justify-between p-3.5 rounded-xl border text-xs font-medium ${
            feedbackMessage.type === "success"
              ? "bg-emerald-950/30 border-emerald-700/50 text-emerald-200"
              : feedbackMessage.type === "error"
              ? "bg-red-950/30 border-red-700/50 text-red-200"
              : "bg-surface border-line text-txt"
          }`}
        >
          <div className="flex items-center gap-2.5">
            {feedbackMessage.type === "success" && <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />}
            {feedbackMessage.type === "error" && <AlertCircle className="w-4 h-4 text-red-400 shrink-0" />}
            {feedbackMessage.type === "info" && <Info className="w-4 h-4 text-sky-400 shrink-0" />}
            <span>{feedbackMessage.text}</span>
          </div>
          <button
            onClick={() => setFeedbackMessage(null)}
            className="text-muted hover:text-txt p-1 rounded hover:bg-surface-2"
            title="Закрыть уведомление"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* Вкладка 1: Программы */}
      {activeTab === "programs" && (
        <div className="space-y-4">
          {/* Сводные карточки */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            <div className="bg-surface border border-line rounded-xl p-3.5">
              <span className="text-[11px] text-muted">Всего программ</span>
              <p className="text-lg font-heading font-bold text-txt mt-1">{progStats.total}</p>
            </div>
            <div className="bg-surface border border-line rounded-xl p-3.5">
              <span className="text-[11px] text-muted">Расчётный объём</span>
              <p className="text-lg font-heading font-bold text-txt mt-1">{formatBytes(progStats.totalBytes)}</p>
            </div>
            <div className="bg-surface border border-line rounded-xl p-3.5">
              <span className="text-[11px] text-muted">64-бит программы</span>
              <p className="text-lg font-heading font-bold text-txt mt-1">{progStats.count64}</p>
            </div>
            <div className="bg-surface border border-line rounded-xl p-3.5">
              <span className="text-[11px] text-muted">32-бит (Wow64)</span>
              <p className="text-lg font-heading font-bold text-txt mt-1">{progStats.count32}</p>
            </div>
          </div>

          {/* Фильтры и поиск */}
          <div className="bg-surface border border-line rounded-xl p-3 flex flex-wrap items-center justify-between gap-3">
            <div className="relative flex-1 min-w-[240px]">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-dim" />
              <input
                type="text"
                placeholder="Поиск по названию, издателю или версии..."
                value={progSearch}
                onChange={(e) => setProgSearch(e.target.value)}
                className="w-full bg-shell border border-line rounded-lg pl-9 pr-8 py-1.5 text-xs text-txt placeholder:text-dim focus:outline-none focus:border-accent font-sans"
              />
              {progSearch && (
                <button
                  onClick={() => setProgSearch("")}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-dim hover:text-txt"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              )}
            </div>

            <div className="flex items-center gap-1 bg-shell p-1 rounded-lg border border-line text-xs">
              <button
                onClick={() => setProgFilterArch("all")}
                className={`px-2.5 py-1 rounded text-[11px] transition-all ${
                  progFilterArch === "all" ? "bg-surface-2 text-txt font-semibold" : "text-muted hover:text-txt"
                }`}
              >
                Все ({programs.length})
              </button>
              <button
                onClick={() => setProgFilterArch("64")}
                className={`px-2.5 py-1 rounded text-[11px] transition-all ${
                  progFilterArch === "64" ? "bg-surface-2 text-txt font-semibold" : "text-muted hover:text-txt"
                }`}
              >
                64-бит
              </button>
              <button
                onClick={() => setProgFilterArch("32")}
                className={`px-2.5 py-1 rounded text-[11px] transition-all ${
                  progFilterArch === "32" ? "bg-surface-2 text-txt font-semibold" : "text-muted hover:text-txt"
                }`}
              >
                32-бит
              </button>
              <button
                onClick={() => setProgFilterArch("sized")}
                className={`px-2.5 py-1 rounded text-[11px] transition-all ${
                  progFilterArch === "sized" ? "bg-surface-2 text-txt font-semibold" : "text-muted hover:text-txt"
                }`}
              >
                С размером
              </button>
            </div>
          </div>

          {/* Таблица программ */}
          <div className="bg-surface border border-line rounded-xl overflow-hidden shadow-sm">
            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs border-collapse">
                <thead>
                  <tr className="bg-shell/80 border-b border-line text-muted select-none">
                    <th
                      onClick={() => toggleSort("name")}
                      className="py-3 px-4 font-medium cursor-pointer hover:text-txt"
                    >
                      <div className="flex items-center gap-1.5">
                        <span>Название программы</span>
                        {progSortField === "name" && (
                          progSortAsc ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />
                        )}
                      </div>
                    </th>
                    <th
                      onClick={() => toggleSort("publisher")}
                      className="py-3 px-3 font-medium cursor-pointer hover:text-txt"
                    >
                      <div className="flex items-center gap-1.5">
                        <span>Издатель</span>
                        {progSortField === "publisher" && (
                          progSortAsc ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />
                        )}
                      </div>
                    </th>
                    <th
                      onClick={() => toggleSort("version")}
                      className="py-3 px-3 font-medium cursor-pointer hover:text-txt"
                    >
                      <div className="flex items-center gap-1.5">
                        <span>Версия</span>
                        {progSortField === "version" && (
                          progSortAsc ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />
                        )}
                      </div>
                    </th>
                    <th
                      onClick={() => toggleSort("date")}
                      className="py-3 px-3 font-medium cursor-pointer hover:text-txt"
                    >
                      <div className="flex items-center gap-1.5">
                        <span>Дата установки</span>
                        {progSortField === "date" && (
                          progSortAsc ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />
                        )}
                      </div>
                    </th>
                    <th
                      onClick={() => toggleSort("size")}
                      className="py-3 px-3 font-medium cursor-pointer hover:text-txt text-right"
                    >
                      <div className="flex items-center justify-end gap-1.5">
                        <span>Размер</span>
                        {progSortField === "size" && (
                          progSortAsc ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />
                        )}
                      </div>
                    </th>
                    <th className="py-3 px-4 font-medium text-center">Действие</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-line/40">
                  {filteredPrograms.length === 0 ? (
                    <tr>
                      <td colSpan={6} className="py-8 text-center text-muted font-mono">
                        {loading ? "Загрузка реестра программ..." : "Программы не найдены."}
                      </td>
                    </tr>
                  ) : (
                    filteredPrograms.map((prog) => (
                      <tr key={prog.id} className="hover:bg-surface-2/60 transition-colors">
                        <td className="py-2.5 px-4 font-medium text-txt">
                          <div className="flex items-center gap-2.5">
                            <Package className="w-4 h-4 text-accent shrink-0" />
                            <div>
                              <div className="flex items-center gap-2">
                                <span className="font-semibold">{prog.display_name}</span>
                                {prog.is_wow64 ? (
                                  <span className="text-[9px] font-mono px-1 rounded bg-shell border border-line text-dim">
                                    32-bit
                                  </span>
                                ) : (
                                  <span className="text-[9px] font-mono px-1 rounded bg-shell border border-line text-dim">
                                    64-bit
                                  </span>
                                )}
                                <span className="text-[9px] font-mono px-1 rounded bg-shell/50 text-dim">
                                  {prog.registry_hive}
                                </span>
                              </div>
                              {prog.install_location && (
                                <p className="text-[10px] text-dim font-mono truncate max-w-sm" title={prog.install_location}>
                                  {prog.install_location}
                                </p>
                              )}
                            </div>
                          </div>
                        </td>
                        <td className="py-2.5 px-3 text-muted truncate max-w-[160px]" title={prog.publisher || ""}>
                          {prog.publisher || "—"}
                        </td>
                        <td className="py-2.5 px-3 font-mono text-[11px] text-muted">
                          {prog.display_version || "—"}
                        </td>
                        <td className="py-2.5 px-3 font-mono text-[11px] text-muted">
                          {formatInstallDate(prog.install_date)}
                        </td>
                        <td className="py-2.5 px-3 font-mono text-[11px] text-txt text-right font-medium">
                          {formatBytes(prog.estimated_size_bytes)}
                        </td>
                        <td className="py-2.5 px-4 text-center">
                          <button
                            onClick={() => {
                              setUninstallerModalProg(prog);
                              setUninstallerAcknowledged(false);
                            }}
                            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg border border-line bg-surface hover:bg-red-950/30 hover:border-red-700/50 hover:text-red-300 text-muted text-xs transition-all"
                            title="Открыть диалог безопасного запуска официального деинсталлятора"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                            <span>Удалить</span>
                          </button>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* Вкладка 2: Автозагрузка */}
      {activeTab === "startup" && (
        <div className="space-y-4">
          {/* Дисклеймер безопасности (Read-Only) */}
          <div className="bg-surface border border-line rounded-xl p-4 flex flex-col md:flex-row md:items-center justify-between gap-4">
            <div className="flex items-start gap-3">
              <Info className="w-5 h-5 text-sky-400 shrink-0 mt-0.5" />
              <div>
                <h4 className="text-xs font-semibold text-txt">Безопасный аудит автозапуска (Только чтение)</h4>
                <p className="text-xs text-muted mt-0.5 leading-relaxed">
                  CleanCore работает в режиме строгого чтения метаданных: приложение не отключает службы и не изменяет
                  системный реестр, защищая систему от сбоев загрузки. Для отключения элементов используйте встроенный Диспетчер задач.
                </p>
              </div>
            </div>
            <button
              onClick={() => handleLaunchTool("taskmgr")}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-shell border border-line text-xs font-medium text-txt hover:bg-surface-2 transition-all shrink-0 self-start md:self-auto"
            >
              <ExternalLink className="w-3.5 h-3.5 text-accent" />
              <span>Открыть Диспетчер задач</span>
            </button>
          </div>

          {/* Сводные показатели */}
          <div className="grid grid-cols-3 gap-3">
            <div className="bg-surface border border-line rounded-xl p-3.5">
              <span className="text-[11px] text-muted">Всего записей</span>
              <p className="text-lg font-heading font-bold text-txt mt-1">{startupStats.total}</p>
            </div>
            <div className="bg-surface border border-line rounded-xl p-3.5">
              <span className="text-[11px] text-muted">Активные записи</span>
              <p className="text-lg font-heading font-bold text-emerald-400 mt-1">{startupStats.enabled}</p>
            </div>
            <div className="bg-surface border border-line rounded-xl p-3.5">
              <span className="text-[11px] text-muted">Файл отсутствует на диске</span>
              <p className="text-lg font-heading font-bold text-amber-400 mt-1">{startupStats.missing}</p>
            </div>
          </div>

          {/* Фильтры и поиск */}
          <div className="bg-surface border border-line rounded-xl p-3 flex flex-wrap items-center justify-between gap-3">
            <div className="relative flex-1 min-w-[240px]">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-dim" />
              <input
                type="text"
                placeholder="Поиск по имени или пути команды..."
                value={startupSearch}
                onChange={(e) => setStartupSearch(e.target.value)}
                className="w-full bg-shell border border-line rounded-lg pl-9 pr-8 py-1.5 text-xs text-txt placeholder:text-dim focus:outline-none focus:border-accent font-sans"
              />
              {startupSearch && (
                <button
                  onClick={() => setStartupSearch("")}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-dim hover:text-txt"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              )}
            </div>

            <div className="flex items-center gap-1 bg-shell p-1 rounded-lg border border-line text-xs">
              <button
                onClick={() => setStartupSourceFilter("all")}
                className={`px-2.5 py-1 rounded text-[11px] transition-all ${
                  startupSourceFilter === "all" ? "bg-surface-2 text-txt font-semibold" : "text-muted hover:text-txt"
                }`}
              >
                Все ({startupItems.length})
              </button>
              <button
                onClick={() => setStartupSourceFilter("registry_run")}
                className={`px-2.5 py-1 rounded text-[11px] transition-all ${
                  startupSourceFilter === "registry_run" ? "bg-surface-2 text-txt font-semibold" : "text-muted hover:text-txt"
                }`}
              >
                Реестр Run
              </button>
              <button
                onClick={() => setStartupSourceFilter("startup_folder_user")}
                className={`px-2.5 py-1 rounded text-[11px] transition-all ${
                  startupSourceFilter === "startup_folder_user" ? "bg-surface-2 text-txt font-semibold" : "text-muted hover:text-txt"
                }`}
              >
                Startup (User)
              </button>
              <button
                onClick={() => setStartupSourceFilter("startup_folder_common")}
                className={`px-2.5 py-1 rounded text-[11px] transition-all ${
                  startupSourceFilter === "startup_folder_common" ? "bg-surface-2 text-txt font-semibold" : "text-muted hover:text-txt"
                }`}
              >
                Startup (Common)
              </button>
            </div>
          </div>

          {/* Список элементов автозагрузки */}
          <div className="bg-surface border border-line rounded-xl overflow-hidden shadow-sm">
            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs border-collapse">
                <thead>
                  <tr className="bg-shell/80 border-b border-line text-muted select-none">
                    <th className="py-3 px-4 font-medium">Элемент</th>
                    <th className="py-3 px-3 font-medium">Команда запуска</th>
                    <th className="py-3 px-3 font-medium">Источник</th>
                    <th className="py-3 px-3 font-medium text-center">Статус</th>
                    <th className="py-3 px-4 font-medium text-right">Наличие файла</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-line/40">
                  {filteredStartup.length === 0 ? (
                    <tr>
                      <td colSpan={5} className="py-8 text-center text-muted font-mono">
                        {loading ? "Чтение источников автозагрузки..." : "Элементы автозагрузки не найдены."}
                      </td>
                    </tr>
                  ) : (
                    filteredStartup.map((item) => (
                      <tr key={item.id} className="hover:bg-surface-2/60 transition-colors">
                        <td className="py-3 px-4 font-semibold text-txt">
                          {item.name}
                        </td>
                        <td className="py-3 px-3 font-mono text-[11px] text-muted max-w-md truncate" title={item.command}>
                          {item.command}
                        </td>
                        <td className="py-3 px-3">
                          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-shell border border-line text-dim">
                            {item.source === "registry_run" && "Реестр Run"}
                            {item.source === "registry_run_once" && "Реестр RunOnce"}
                            {item.source === "startup_folder_user" && "Папка пользователя"}
                            {item.source === "startup_folder_common" && "Общая папка"}
                          </span>
                        </td>
                        <td className="py-3 px-3 text-center">
                          <span
                            className={`text-[10px] font-mono px-2 py-0.5 rounded border ${
                              item.enabled
                                ? "bg-emerald-950/40 border-emerald-700/50 text-emerald-300"
                                : "bg-shell border-line text-dim"
                            }`}
                          >
                            {item.enabled ? "Включено" : "Отключено"}
                          </span>
                        </td>
                        <td className="py-3 px-4 text-right">
                          <span
                            className={`inline-flex items-center gap-1.5 text-[11px] font-mono ${
                              item.target_exists ? "text-emerald-400" : "text-amber-400"
                            }`}
                          >
                            {item.target_exists ? (
                              <>
                                <CheckCircle2 className="w-3.5 h-3.5" />
                                <span>Файл найден</span>
                              </>
                            ) : (
                              <>
                                <AlertCircle className="w-3.5 h-3.5" />
                                <span>Файл отсутствует</span>
                              </>
                            )}
                          </span>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* Вкладка 3: Защита и здоровье */}
      {activeTab === "diagnostics" && (
        <div className="space-y-5">
          {/* Карточка Антивируса / Defender */}
          <div className="bg-surface border border-line rounded-2xl p-5 shadow-sm">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
              <div className="flex items-center gap-3.5">
                <div
                  className={`w-12 h-12 rounded-xl border flex items-center justify-center shrink-0 ${
                    antivirus?.state === "protected"
                      ? "bg-emerald-950/40 border-emerald-700/60 text-emerald-400"
                      : antivirus?.state === "limited_access"
                      ? "bg-amber-950/40 border-amber-700/60 text-amber-400"
                      : "bg-red-950/40 border-red-700/60 text-red-400"
                  }`}
                >
                  {antivirus?.state === "protected" ? (
                    <ShieldCheck className="w-6 h-6" />
                  ) : (
                    <ShieldAlert className="w-6 h-6" />
                  )}
                </div>
                <div>
                  <div className="flex items-center gap-2">
                    <h3 className="font-heading text-base font-bold text-txt">
                      Антивирусная защита Windows
                    </h3>
                    <span
                      className={`text-[10px] font-mono px-2 py-0.5 rounded border ${
                        antivirus?.state === "protected"
                          ? "bg-emerald-950/40 border-emerald-700/50 text-emerald-300"
                          : antivirus?.state === "limited_access"
                          ? "bg-amber-950/40 border-amber-700/50 text-amber-300"
                          : "bg-red-950/40 border-red-700/50 text-red-300"
                      }`}
                    >
                      {antivirus?.state === "protected" && "Защита активна"}
                      {antivirus?.state === "warning" && "Требует внимания"}
                      {antivirus?.state === "disabled" && "Отключена"}
                      {antivirus?.state === "limited_access" && "Ограниченный доступ"}
                      {antivirus?.state === "unknown" && "Неизвестно"}
                    </span>
                  </div>
                  <p className="text-xs text-muted mt-0.5">{antivirus?.message || "Опрос статуса антивируса..."}</p>
                </div>
              </div>

              {antivirus?.providers && antivirus.providers.length > 0 && (
                <div className="text-right">
                  <span className="text-[11px] text-dim">Поставщик:</span>
                  <p className="text-xs font-semibold text-txt font-mono">
                    {antivirus.providers[0].name}
                  </p>
                </div>
              )}
            </div>

            {/* Подробности состояния Defender */}
            {antivirus?.providers && antivirus.providers.length > 0 && (
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 mt-4 pt-4 border-t border-line/60 text-xs">
                <div className="bg-shell/60 border border-line/60 rounded-xl p-3">
                  <span className="text-[11px] text-muted">Защита в реальном времени</span>
                  <p className="font-semibold text-txt mt-1">
                    {antivirus.providers[0].real_time_protection ? (
                      <span className="text-emerald-400 flex items-center gap-1">
                        <CheckCircle2 className="w-3.5 h-3.5" /> Включена
                      </span>
                    ) : (
                      <span className="text-red-400 flex items-center gap-1">
                        <XCircle className="w-3.5 h-3.5" /> Отключена
                      </span>
                    )}
                  </p>
                </div>
                <div className="bg-shell/60 border border-line/60 rounded-xl p-3">
                  <span className="text-[11px] text-muted">Антивирусные базы</span>
                  <p className="font-semibold text-txt mt-1">
                    {antivirus.providers[0].definitions_up_to_date ? (
                      <span className="text-emerald-400 flex items-center gap-1">
                        <CheckCircle2 className="w-3.5 h-3.5" /> Актуальны
                      </span>
                    ) : (
                      <span className="text-amber-400 flex items-center gap-1">
                        <AlertCircle className="w-3.5 h-3.5" /> Требуют обновления
                      </span>
                    )}
                  </p>
                </div>
                <div className="bg-shell/60 border border-line/60 rounded-xl p-3">
                  <span className="text-[11px] text-muted">Служба WinDefend</span>
                  <p className="font-semibold text-txt mt-1">
                    {antivirus.defender_service_running === true ? (
                      <span className="text-emerald-400">Работает (SCM)</span>
                    ) : antivirus.defender_service_running === false ? (
                      <span className="text-red-400">Остановлена</span>
                    ) : (
                      <span className="text-dim">Недоступно без UAC</span>
                    )}
                  </p>
                </div>
              </div>
            )}
          </div>

          {/* 4 Карточки ключевых системных метрик здоровья */}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3.5">
            {/* CPU */}
            <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
              <div className="flex items-center justify-between text-muted mb-2">
                <span className="text-xs font-medium">Процессор (CPU)</span>
                <Cpu className="w-4 h-4 text-accent" />
              </div>
              <div className="flex items-baseline gap-2">
                <span className="text-2xl font-heading font-bold text-txt">
                  {diagnostics?.cpu_usage_percent !== null && diagnostics?.cpu_usage_percent !== undefined
                    ? `${diagnostics.cpu_usage_percent.toFixed(1)}%`
                    : "—"}
                </span>
                <span
                  className={`text-[10px] font-mono px-1.5 py-0.5 rounded ${
                    (diagnostics?.cpu_usage_percent || 0) > 85
                      ? "bg-red-950/40 text-red-400"
                      : (diagnostics?.cpu_usage_percent || 0) > 65
                      ? "bg-amber-950/40 text-amber-400"
                      : "bg-emerald-950/40 text-emerald-400"
                  }`}
                >
                  {(diagnostics?.cpu_usage_percent || 0) > 85
                    ? "Высокая"
                    : (diagnostics?.cpu_usage_percent || 0) > 65
                    ? "Повышенная"
                    : "Норма"}
                </span>
              </div>
            </div>

            {/* Память */}
            <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
              <div className="flex items-center justify-between text-muted mb-2">
                <span className="text-xs font-medium">Память (RAM)</span>
                <Activity className="w-4 h-4 text-accent" />
              </div>
              <div className="flex items-baseline gap-2">
                <span className="text-2xl font-heading font-bold text-txt">
                  {diagnostics?.memory_usage_percent !== null && diagnostics?.memory_usage_percent !== undefined
                    ? `${diagnostics.memory_usage_percent.toFixed(0)}%`
                    : "—"}
                </span>
                <span
                  className={`text-[10px] font-mono px-1.5 py-0.5 rounded ${
                    (diagnostics?.memory_usage_percent || 0) > 90
                      ? "bg-red-950/40 text-red-400"
                      : (diagnostics?.memory_usage_percent || 0) > 80
                      ? "bg-amber-950/40 text-amber-400"
                      : "bg-emerald-950/40 text-emerald-400"
                  }`}
                >
                  {(diagnostics?.memory_usage_percent || 0) > 90
                    ? "Критично"
                    : (diagnostics?.memory_usage_percent || 0) > 80
                    ? "Внимание"
                    : "Норма"}
                </span>
              </div>
            </div>

            {/* Диск C: */}
            <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
              <div className="flex items-center justify-between text-muted mb-2">
                <span className="text-xs font-medium">Системный диск ({diagnostics?.system_drive || "C:"})</span>
                <HardDrive className="w-4 h-4 text-accent" />
              </div>
              <div className="flex items-baseline gap-2">
                <span className="text-2xl font-heading font-bold text-txt">
                  {diagnostics?.disk_free_bytes !== null && diagnostics?.disk_free_bytes !== undefined
                    ? formatBytes(diagnostics.disk_free_bytes)
                    : "—"}
                </span>
                <span className="text-[11px] text-dim font-mono">свободно</span>
              </div>
              {diagnostics?.disk_usage_percent !== null && diagnostics?.disk_usage_percent !== undefined && (
                <div className="w-full bg-shell h-1.5 rounded-full overflow-hidden mt-3">
                  <div
                    className={`h-full rounded-full ${
                      diagnostics.disk_usage_percent > 90
                        ? "bg-red-500"
                        : diagnostics.disk_usage_percent > 80
                        ? "bg-amber-500"
                        : "bg-emerald-500"
                    }`}
                    style={{ width: `${Math.min(100, diagnostics.disk_usage_percent)}%` }}
                  />
                </div>
              )}
            </div>

            {/* Сеть */}
            <div className="bg-surface border border-line rounded-xl p-4 shadow-sm">
              <div className="flex items-center justify-between text-muted mb-2">
                <span className="text-xs font-medium">Сетевой стек</span>
                <Network className="w-4 h-4 text-accent" />
              </div>
              <div className="flex items-baseline gap-2">
                <span className="text-2xl font-heading font-bold text-emerald-400">
                  Активен
                </span>
                <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-emerald-950/40 text-emerald-400">
                  OK
                </span>
              </div>
            </div>
          </div>

          {/* Рекомендации и системные оповещения */}
          <div className="bg-surface border border-line rounded-2xl p-5 shadow-sm space-y-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2.5">
                <Lightbulb className="w-5 h-5 text-amber-400" />
                <h3 className="font-heading text-sm font-bold text-txt">
                  Рекомендации по оптимизации и здоровью системы
                </h3>
              </div>
              <span
                className={`text-[11px] font-mono px-2 py-0.5 rounded border ${
                  diagnostics?.health === "healthy"
                    ? "bg-emerald-950/40 border-emerald-700/50 text-emerald-300"
                    : diagnostics?.health === "warning"
                    ? "bg-amber-950/40 border-amber-700/50 text-amber-300"
                    : "bg-red-950/40 border-red-700/50 text-red-300"
                }`}
              >
                Статус: {diagnostics?.health === "healthy" ? "Отличное" : diagnostics?.health === "warning" ? "Требует внимания" : "Критическое"}
              </span>
            </div>

            {/* Список рекомендаций */}
            <div className="space-y-2">
              {diagnostics?.recommendations && diagnostics.recommendations.length > 0 ? (
                diagnostics.recommendations.map((rec, i) => (
                  <div
                    key={i}
                    className="flex items-start gap-2.5 p-3 rounded-xl bg-shell/70 border border-line/70 text-xs text-txt"
                  >
                    <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
                    <span className="leading-relaxed">{rec}</span>
                  </div>
                ))
              ) : (
                <p className="text-xs text-muted font-mono">Рекомендации формируются на основе системных метрик.</p>
              )}
            </div>

            {/* Кнопка перехода к очистке при необходимости */}
            {onNavigate && (
              <div className="pt-2 flex items-center justify-end">
                <button
                  onClick={() => onNavigate("cleanup")}
                  className="flex items-center gap-2 px-3.5 py-1.5 rounded-lg bg-surface-2 border border-line text-xs font-medium text-txt hover:bg-surface hover:border-accent transition-all"
                >
                  <Trash2 className="w-3.5 h-3.5 text-accent" />
                  <span>Перейти к мастеру очистки диска</span>
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Вкладка 4: Штатные инструменты Windows */}
      {activeTab === "tools" && (
        <div className="space-y-4">
          <div className="bg-surface border border-line rounded-xl p-4">
            <h3 className="font-heading text-sm font-bold text-txt">
              Штатные системные инструменты Windows (Strict Whitelist)
            </h3>
            <p className="text-xs text-muted mt-1 leading-relaxed">
              Запуск проверенных системных апплетов и утилит из папки <code className="font-mono text-accent">System32</code> напрямую,
              без промежуточных командных оболочек (cmd/powershell) и без скрытых фоновых модификаций.
            </p>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {WINDOWS_TOOLS.map((tool) => (
              <div
                key={tool.id}
                className="bg-surface border border-line rounded-2xl p-5 flex flex-col justify-between shadow-sm hover:border-accent/40 transition-all"
              >
                <div>
                  <div className="flex items-center justify-between mb-3">
                    <div className="w-10 h-10 rounded-xl bg-shell border border-line flex items-center justify-center">
                      {tool.icon}
                    </div>
                    <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-shell border border-line text-dim">
                      {tool.binary}
                    </span>
                  </div>
                  <h4 className="font-heading text-base font-bold text-txt">{tool.name}</h4>
                  <p className="text-xs text-muted mt-1.5 leading-relaxed">{tool.description}</p>
                  <p className="text-[10px] text-dim font-mono mt-3 truncate" title={tool.executablePath}>
                    {tool.executablePath}
                  </p>
                </div>

                <div className="mt-5 pt-4 border-t border-line/60">
                  <button
                    onClick={() => handleLaunchTool(tool.id)}
                    disabled={runningToolId === tool.id}
                    className="w-full flex items-center justify-center gap-2 px-3.5 py-2 rounded-xl bg-accent text-accent-ink font-semibold text-xs hover:bg-white transition-all disabled:opacity-50 shadow-sm"
                  >
                    {runningToolId === tool.id ? (
                      <>
                        <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                        <span>Запуск...</span>
                      </>
                    ) : (
                      <>
                        <ExternalLink className="w-3.5 h-3.5" />
                        <span>Запустить</span>
                      </>
                    )}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Модальное окно деинсталлятора (Safety Confirmation Dialog) */}
      {uninstallerModalProg && (
        <div
          role="dialog"
          aria-modal="true"
          className="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4"
        >
          <div className="bg-surface border border-line rounded-2xl max-w-xl w-full p-6 shadow-2xl space-y-4">
            <div className="flex items-start justify-between gap-4">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-red-950/40 border border-red-700/50 flex items-center justify-center text-red-400">
                  <Trash2 className="w-5 h-5" />
                </div>
                <div>
                  <h3 className="font-heading text-base font-bold text-txt">
                    Запуск официального деинсталлятора
                  </h3>
                  <p className="text-xs text-muted">
                    Подтверждение вызова штатной процедуры удаления ПО
                  </p>
                </div>
              </div>
              <button
                onClick={() => setUninstallerModalProg(null)}
                className="text-muted hover:text-txt p-1 rounded-lg hover:bg-surface-2"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Данные программы */}
            <div className="bg-shell/80 border border-line rounded-xl p-3.5 space-y-2 text-xs">
              <div className="flex justify-between">
                <span className="text-dim">Программа:</span>
                <span className="font-semibold text-txt text-right">{uninstallerModalProg.display_name}</span>
              </div>
              {uninstallerModalProg.publisher && (
                <div className="flex justify-between">
                  <span className="text-dim">Издатель:</span>
                  <span className="text-muted text-right">{uninstallerModalProg.publisher}</span>
                </div>
              )}
              {uninstallerModalProg.display_version && (
                <div className="flex justify-between">
                  <span className="text-dim">Версия:</span>
                  <span className="font-mono text-muted text-right">{uninstallerModalProg.display_version}</span>
                </div>
              )}
              {uninstallerModalProg.estimated_size_bytes && (
                <div className="flex justify-between">
                  <span className="text-dim">Расчётный размер:</span>
                  <span className="font-mono text-txt text-right">{formatBytes(uninstallerModalProg.estimated_size_bytes)}</span>
                </div>
              )}
            </div>

            {/* Команда деинсталлятора */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between text-[11px] text-muted">
                <span>Команда деинсталлятора из реестра:</span>
                <button
                  onClick={() => {
                    if (uninstallerModalProg.uninstall_string) {
                      navigator.clipboard.writeText(uninstallerModalProg.uninstall_string);
                      setUninstallerCopied(true);
                      setTimeout(() => setUninstallerCopied(false), 2000);
                    }
                  }}
                  className="flex items-center gap-1 text-dim hover:text-txt"
                >
                  {uninstallerCopied ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
                  <span>{uninstallerCopied ? "Скопировано" : "Копировать"}</span>
                </button>
              </div>
              <div className="bg-shell border border-line rounded-lg p-2.5 font-mono text-[11px] text-accent-ink bg-slate-900/90 text-slate-200 break-all select-all">
                {uninstallerModalProg.uninstall_string || "Строка деинсталляции отсутствует в реестре"}
              </div>
            </div>

            {/* Предупреждение безопасности */}
            <div className="bg-amber-950/20 border border-amber-700/40 rounded-xl p-3 flex items-start gap-2.5 text-xs text-amber-200">
              <AlertCircle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
              <p className="leading-relaxed">
                CleanCore <strong>не производит принудительное удаление файлов</strong> напрямую. Вызов запускает официальный
                мастер деинсталляции от разработчика программы в отдельном системном процессе.
              </p>
            </div>

            {/* Чекбокс подтверждения */}
            <label className="flex items-center gap-2.5 text-xs text-txt cursor-pointer select-none">
              <input
                type="checkbox"
                checked={uninstallerAcknowledged}
                onChange={(e) => setUninstallerAcknowledged(e.target.checked)}
                className="w-4 h-4 rounded border-line bg-shell text-accent focus:ring-0"
              />
              <span>Я подтверждаю запуск штатного деинсталлятора этой программы</span>
            </label>

            {/* Кнопки действия */}
            <div className="flex items-center justify-end gap-3 pt-3 border-t border-line/60">
              <button
                onClick={() => setUninstallerModalProg(null)}
                className="px-4 py-2 rounded-xl bg-shell border border-line text-xs font-medium text-muted hover:text-txt hover:bg-surface-2 transition-all"
              >
                Отмена
              </button>
              <button
                onClick={handleLaunchUninstaller}
                disabled={!uninstallerAcknowledged || uninstallerBusy}
                className="flex items-center gap-2 px-4 py-2 rounded-xl bg-red-600 text-white font-semibold text-xs hover:bg-red-500 transition-all disabled:opacity-40 disabled:hover:bg-red-600 shadow-sm"
              >
                {uninstallerBusy ? (
                  <>
                    <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                    <span>Запуск процесса...</span>
                  </>
                ) : (
                  <>
                    <ExternalLink className="w-3.5 h-3.5" />
                    <span>Запустить деинсталлятор</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Модальное окно экспорта в CSV (Mod 6.6) */}
      {exportModalOpen && (
        <div
          role="dialog"
          aria-modal="true"
          className="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex items-center justify-center p-4"
        >
          <div className="bg-surface border border-line rounded-2xl max-w-lg w-full p-6 shadow-2xl space-y-4">
            <div className="flex items-start justify-between gap-4">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-emerald-950/40 border border-emerald-700/50 flex items-center justify-center text-emerald-400">
                  <FileSpreadsheet className="w-5 h-5" />
                </div>
                <div>
                  <h3 className="font-heading text-base font-bold text-txt">
                    Экспорт отчёта обслуживания (CSV)
                  </h3>
                  <p className="text-xs text-muted">
                    Формирование структурированного локального файла по стандарту RFC 4180
                  </p>
                </div>
              </div>
              <button
                onClick={() => setExportModalOpen(false)}
                className="text-muted hover:text-txt p-1 rounded-lg hover:bg-surface-2"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Выбор разделов отчёта */}
            <div className="space-y-2 text-xs">
              <span className="text-dim font-medium">Включить в отчёт:</span>
              <div className="bg-shell/80 border border-line rounded-xl p-3.5 space-y-2.5">
                <label className="flex items-center gap-2.5 cursor-pointer text-txt select-none">
                  <input
                    type="checkbox"
                    checked={exportSections.programs}
                    onChange={(e) => setExportSections({ ...exportSections, programs: e.target.checked })}
                    className="w-4 h-4 rounded border-line bg-shell text-accent focus:ring-0"
                  />
                  <span>Установленные программы ({programs.length} записей)</span>
                </label>
                <label className="flex items-center gap-2.5 cursor-pointer text-txt select-none">
                  <input
                    type="checkbox"
                    checked={exportSections.startup}
                    onChange={(e) => setExportSections({ ...exportSections, startup: e.target.checked })}
                    className="w-4 h-4 rounded border-line bg-shell text-accent focus:ring-0"
                  />
                  <span>Элементы автозагрузки ({startupItems.length} записей)</span>
                </label>
                <label className="flex items-center gap-2.5 cursor-pointer text-txt select-none">
                  <input
                    type="checkbox"
                    checked={exportSections.diagnostics}
                    onChange={(e) => setExportSections({ ...exportSections, diagnostics: e.target.checked })}
                    className="w-4 h-4 rounded border-line bg-shell text-accent focus:ring-0"
                  />
                  <span>Диагностика здоровья и статус защиты Defender</span>
                </label>
              </div>
            </div>

            {/* Дополнительные опции */}
            <div className="space-y-2 text-xs">
              <label className="flex items-center gap-2.5 cursor-pointer text-muted select-none">
                <input
                  type="checkbox"
                  checked={exportIncludeSystem}
                  onChange={(e) => setExportIncludeSystem(e.target.checked)}
                  className="w-4 h-4 rounded border-line bg-shell text-accent focus:ring-0"
                />
                <span>Включать скрытые системные компоненты Windows (SystemComponent=1)</span>
              </label>
            </div>

            {/* Путь сохранения (опционально) */}
            <div className="space-y-1.5 text-xs">
              <span className="text-dim">Пользовательский путь файла (опционально):</span>
              <input
                type="text"
                placeholder="По умолчанию: папка Загрузки пользователя"
                value={exportCustomPath}
                onChange={(e) => setExportCustomPath(e.target.value)}
                className="w-full bg-shell border border-line rounded-lg px-3 py-1.5 text-xs text-txt placeholder:text-dim font-mono focus:outline-none focus:border-accent"
              />
            </div>

            {/* Гарантия безопасности (CWE-1236) */}
            <div className="bg-shell/80 border border-line rounded-xl p-3 flex items-start gap-2.5 text-[11px] text-muted">
              <ShieldCheck className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
              <p className="leading-relaxed">
                Все ячейки защищены от CSV-инъекций (CWE-1236): символы формул <code className="font-mono text-accent">=, +, -, @</code> экранируются
                апострофом, гарантируя безопасность при открытии в MS Excel и LibreOffice Calc.
              </p>
            </div>

            {/* Результат экспорта */}
            {exportResult && (
              <div className="p-2.5 rounded-xl bg-emerald-950/30 border border-emerald-700/50 text-xs text-emerald-300 font-mono flex items-center justify-between">
                <span>Отчёт успешно сформирован!</span>
                <span className="text-[10px] text-dim">{exportResult.length} байт</span>
              </div>
            )}

            {/* Кнопки */}
            <div className="flex items-center justify-end gap-3 pt-3 border-t border-line/60">
              <button
                onClick={() => setExportModalOpen(false)}
                className="px-4 py-2 rounded-xl bg-shell border border-line text-xs font-medium text-muted hover:text-txt hover:bg-surface-2 transition-all"
              >
                Закрыть
              </button>
              <button
                onClick={handleExportCsv}
                disabled={exportBusy}
                className="flex items-center gap-2 px-4 py-2 rounded-xl bg-accent text-accent-ink font-semibold text-xs hover:bg-white transition-all disabled:opacity-40 shadow-sm"
              >
                {exportBusy ? (
                  <>
                    <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                    <span>Формирование...</span>
                  </>
                ) : (
                  <>
                    <FileSpreadsheet className="w-3.5 h-3.5" />
                    <span>Сформировать отчёт</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default ServicePage;
