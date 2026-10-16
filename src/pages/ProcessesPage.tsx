import React, { useState, useEffect } from "react";
import { ProcessItem, ProcessSnapshot } from "../types";
import { fetchProcessList } from "../lib/ipc";
import { Search, RefreshCw, Cpu, ArrowUpDown, ShieldAlert, Folder } from "lucide-react";

type SortField = "cpu" | "memory" | "name" | "pid";

interface ProcessesPageProps {
  initialProcesses?: ProcessItem[];
  onRefresh?: () => Promise<ProcessSnapshot>;
}

export const ProcessesPage: React.FC<ProcessesPageProps> = ({
  initialProcesses = [],
  onRefresh,
}) => {
  const [processes, setProcesses] = useState<ProcessItem[]>(initialProcesses);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [sortField, setSortField] = useState<SortField>("cpu");
  const [sortDesc, setSortDesc] = useState(true);
  const [copyMessage, setCopyMessage] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<string>("");

  const loadProcesses = async () => {
    setLoading(true);
    setError(null);
    try {
      const snapshot = await (onRefresh ? onRefresh() : fetchProcessList());
      setProcesses(snapshot.processes);
      setLastUpdated(
        new Date(snapshot.timestamp_utc).toLocaleTimeString("ru-RU", {
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
        })
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    setProcesses(initialProcesses);
    setLoading(false);
  }, [initialProcesses]);

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDesc(!sortDesc);
    } else {
      setSortField(field);
      setSortDesc(true);
    }
  };

  // Фильтрация по поисковому запросу
  const filtered = processes.filter((proc) => {
    const q = searchTerm.toLowerCase().trim();
    if (!q) return true;
    const matchName = proc.name.toLowerCase().includes(q);
    const matchPid = proc.pid.toString().includes(q);
    const matchPath =
      proc.executable_path.status === "Available" && proc.executable_path.value
        ? proc.executable_path.value.toLowerCase().includes(q)
        : false;
    return matchName || matchPid || matchPath;
  });

  // Детерминированная сортировка
  const sorted = [...filtered].sort((a, b) => {
    let comparison = 0;
    switch (sortField) {
      case "cpu": {
        const valA = a.cpu_percent.status === "Available" ? (a.cpu_percent.value ?? 0) : -1;
        const valB = b.cpu_percent.status === "Available" ? (b.cpu_percent.value ?? 0) : -1;
        comparison = valA - valB;
        if (comparison === 0) {
          const memA = a.memory_bytes.status === "Available" ? (a.memory_bytes.value ?? 0) : -1;
          const memB = b.memory_bytes.status === "Available" ? (b.memory_bytes.value ?? 0) : -1;
          comparison = memA - memB;
        }
        break;
      }
      case "memory": {
        const valA = a.memory_bytes.status === "Available" ? (a.memory_bytes.value ?? 0) : -1;
        const valB = b.memory_bytes.status === "Available" ? (b.memory_bytes.value ?? 0) : -1;
        comparison = valA - valB;
        break;
      }
      case "pid":
        comparison = a.pid - b.pid;
        break;
      case "name":
        comparison = a.name.toLowerCase().localeCompare(b.name.toLowerCase());
        break;
    }
    if (comparison === 0) {
      comparison = a.pid - b.pid;
    }
    return sortDesc ? -comparison : comparison;
  });

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-wrap items-end justify-between gap-4 pb-2 border-b border-line/40">
        <div>
          <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
            Мониторинг
          </span>
          <h2 className="font-heading text-2xl font-bold text-txt">
            Процессы системы
          </h2>
          <p className="text-xs text-muted mt-1">
            Понятная таблица запущенных программ с идентификаторами, нормированным CPU и памятью.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <div className="text-right font-mono text-[11px] text-dim hidden sm:block">
            <span>Обновлено: </span>
            <strong className="text-txt">{lastUpdated || "Актуально"}</strong>
          </div>
          <button
            onClick={loadProcesses}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface border border-line text-xs font-mono text-txt hover:bg-surface-2 disabled:opacity-50 transition-all cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 text-accent ${loading ? "animate-spin" : ""}`} />
            <span>Обновить</span>
          </button>
        </div>
      </div>

      {/* Filter and Search Bar */}
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="relative flex-1 min-w-[260px] max-w-md">
          <Search className="w-4 h-4 text-dim absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            type="search"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            aria-label="Поиск процессов по имени, PID или пути"
              placeholder="Поиск по названию, PID или пути..."
            className="w-full bg-shell border border-line rounded-lg pl-9 pr-4 py-2 text-xs text-txt placeholder:text-dim focus:outline-none focus:border-accent transition-all font-mono"
          />
        </div>

        <div className="flex items-center gap-2 text-xs font-mono text-dim">
          <span>Найдено: </span>
          <strong className="text-txt">
            {sorted.length} из {processes.length}
          </strong>
        </div>
      </div>

      {error && processes.length > 0 && <p role="alert" className="text-sm text-warn">Обновить список не удалось: {error}. Показаны последние полученные данные.</p>}
      {copyMessage && <p role="status" className="text-xs text-muted">{copyMessage}</p>}
      {/* Table Container */}
      <div className="bg-surface border border-line rounded-xl overflow-hidden shadow-sm">
        {/* Table Head */}
        <div className="grid grid-cols-12 text-[11px] font-mono text-dim bg-shell/90 px-4 py-3 border-b border-line items-center select-none">
          <button
            onClick={() => handleSort("name")}
            className="col-span-5 flex items-center gap-1.5 font-medium hover:text-txt text-left transition-colors cursor-pointer"
          >
            <span>ПРОЦЕСС {sortField === "name" ? (sortDesc ? "↓" : "↑") : ""}</span>
            <ArrowUpDown className="w-3 h-3" />
          </button>

          <button
            onClick={() => handleSort("pid")}
            className="col-span-2 flex items-center justify-end gap-1 font-medium hover:text-txt text-right transition-colors cursor-pointer"
          >
            <span>PID {sortField === "pid" ? (sortDesc ? "↓" : "↑") : ""}</span>
            <ArrowUpDown className="w-3 h-3" />
          </button>

          <button
            onClick={() => handleSort("cpu")}
            className="col-span-2 flex items-center justify-end gap-1 font-medium hover:text-txt text-right transition-colors cursor-pointer"
          >
            <span>CPU % {sortField === "cpu" ? (sortDesc ? "↓" : "↑") : ""}</span>
            <ArrowUpDown className="w-3 h-3 text-accent" />
          </button>

          <button
            onClick={() => handleSort("memory")}
            className="col-span-3 flex items-center justify-end gap-1 font-medium hover:text-txt text-right transition-colors cursor-pointer"
          >
            <span>ПАМЯТЬ {sortField === "memory" ? (sortDesc ? "↓" : "↑") : ""}</span>
            <ArrowUpDown className="w-3 h-3" />
          </button>
        </div>

        {/* Content Body */}
        {loading && processes.length === 0 ? (
          <div className="py-20 flex flex-col items-center justify-center text-center px-4">
            <div className="w-8 h-8 border-2 border-line border-t-accent rounded-full animate-spin mb-3" />
            <span className="text-xs font-mono text-muted">Чтение списка процессов...</span>
          </div>
        ) : error && !processes.length ? (
          <div className="py-16 flex flex-col items-center justify-center text-center px-4">
            <div className="w-10 h-10 rounded-full bg-red-950/40 border border-red-700/50 flex items-center justify-center text-red-400 mb-2">
              <ShieldAlert className="w-5 h-5" />
            </div>
            <strong className="text-sm font-semibold text-txt">Ошибка получения процессов</strong>
            <p className="text-xs text-muted max-w-sm mt-1 font-mono">{error}</p>
          </div>
        ) : sorted.length === 0 ? (
          <div className="py-20 flex flex-col items-center justify-center text-center px-4">
            <div className="w-12 h-12 rounded-full bg-shell border border-line flex items-center justify-center text-dim mb-3">
              <Cpu className="w-6 h-6" />
            </div>
            <h3 className="font-heading text-base font-semibold text-txt">
              Ничего не найдено
            </h3>
            <p className="text-xs text-muted max-w-md mt-1 leading-relaxed">
              {searchTerm ? `По запросу «${searchTerm}» подходящих процессов не обнаружено.` : "Нет данных о процессах. Мониторинг может быть недоступен."}
            </p>
          </div>
        ) : (
          <div className="divide-y divide-line/30 max-h-[600px] overflow-y-auto font-mono text-xs">
            {sorted.map((proc) => {
              const cpuVal = proc.cpu_percent.status === "Available" ? proc.cpu_percent.value : undefined;
              const memBytes = proc.memory_bytes.status === "Available" ? proc.memory_bytes.value : undefined;
              const memMb =
                memBytes !== undefined
                  ? memBytes > 1024 * 1024 * 1024
                    ? `${(memBytes / (1024 * 1024 * 1024)).toFixed(2)} ГБ`
                    : `${(memBytes / (1024 * 1024)).toFixed(0)} МБ`
                  : "—";

              return (
                <div
                  key={proc.pid}
                  className="grid grid-cols-12 items-center px-4 py-2.5 hover:bg-shell/50 transition-colors"
                >
                  {/* Process Name & Path */}
                  <div className="col-span-5 pr-3 min-w-0">
                    <div className="text-txt font-semibold truncate flex items-center gap-1.5">
                      <span>{proc.name}</span>
                    </div>

                    <div className="text-[10px] text-dim truncate mt-0.5 flex items-center gap-1">
                      {proc.executable_path.status === "Available" && proc.executable_path.value ? (
                        <>
                          <Folder className="w-2.5 h-2.5 flex-shrink-0 text-dim" />
                          <button type="button" aria-label={`Копировать путь процесса ${proc.name}`} onClick={() => { void navigator.clipboard.writeText(proc.executable_path.value!).then(() => setCopyMessage("Путь скопирован")).catch(() => setCopyMessage("Не удалось скопировать путь")); }}
                            className="truncate text-left text-muted hover:text-txt"
                            title={`Копировать путь: ${proc.executable_path.value}`}
                          >
                            {proc.executable_path.value}
                          </button>
                        </>
                      ) : proc.executable_path.status === "AccessDenied" ? (
                        <span className="text-amber-400/80 bg-amber-950/30 px-1 rounded text-[9px] border border-amber-800/40">
                          Системный (доступ ОС ограничен)
                        </span>
                      ) : (
                        <span className="text-dim">Недоступно</span>
                      )}
                    </div>
                  </div>

                  {/* PID */}
                  <div className="col-span-2 text-right text-dim font-mono">
                    {proc.pid}
                  </div>

                  {/* CPU % */}
                  <div className="col-span-2 text-right font-mono">
                    {cpuVal !== undefined ? (
                      <span
                        className={
                          cpuVal > 10
                            ? "text-accent font-bold bg-accent/10 px-1.5 py-0.5 rounded"
                            : cpuVal > 1
                            ? "text-txt font-semibold"
                            : "text-muted"
                        }
                      >
                        {cpuVal.toFixed(1)}%
                      </span>
                    ) : (
                      <span className="text-dim">—</span>
                    )}
                  </div>

                  {/* Memory */}
                  <div className="col-span-3 text-right font-mono text-txt">
                    {memMb}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Safety Note */}
      <div className="p-4 rounded-xl bg-shell/60 border border-line text-xs text-muted flex items-center justify-between">
        <span>
          Только просмотр процессов. Системные ограничения доступа отображаются в таблице.
        </span>
        <span className="text-dim font-mono text-[11px]">Без изменений системы</span>
      </div>
    </div>
  );
};
