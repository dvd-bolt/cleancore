import React, { useState, useEffect, useMemo, useCallback } from "react";
import { QuickDirectory, StaleFileEntry } from "../../types";
import { fetchStaleFiles, fetchQuickDirectories, revealPathInExplorer } from "../../lib/ipc";
import {
  Clock,
  Folder,
  Calendar,
  Search,
  ExternalLink,
  RefreshCw,
  AlertTriangle,
  FolderDown,
  FileText,
} from "lucide-react";

interface StaleFilesViewProps {
  className?: string;
  defaultPath?: string;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Б";
  const k = 1024;
  const sizes = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(val >= 100 || i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export const StaleFilesView: React.FC<StaleFilesViewProps> = ({
  className = "",
  defaultPath = "",
}) => {
  const [quickDirs, setQuickDirs] = useState<QuickDirectory[]>([]);
  const [selectedFolder, setSelectedFolder] = useState<string>(defaultPath);
  const [olderThanDays, setOlderThanDays] = useState<number>(180);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [items, setItems] = useState<StaleFileEntry[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [revealingPath, setRevealingPath] = useState<string | null>(null);

  useEffect(() => {
    fetchQuickDirectories()
      .then((dirs) => {
        setQuickDirs(dirs);
        if (!selectedFolder && dirs.length > 0) {
          // Ищем папку Загрузки или рабочий стол
          const dl = dirs.find((d) => d.name.toLowerCase().includes("загруз") || d.id === "downloads");
          setSelectedFolder(dl ? dl.path : dirs[0].path);
        }
      })
      .catch(() => {});
  }, [selectedFolder]);

  const loadStaleFiles = useCallback(async () => {
    if (!selectedFolder) return;
    setLoading(true);
    setError(null);
    try {
      const res = await fetchStaleFiles(selectedFolder, olderThanDays, 300);
      setItems(res || []);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || "Не удалось просканировать каталог на наличие забытых файлов.");
    } finally {
      setLoading(false);
    }
  }, [selectedFolder, olderThanDays]);

  useEffect(() => {
    if (selectedFolder) {
      void loadStaleFiles();
    }
  }, [selectedFolder, olderThanDays, loadStaleFiles]);

  const filteredItems = useMemo(() => {
    if (!searchQuery.trim()) return items;
    const q = searchQuery.toLowerCase();
    return items.filter(
      (f) => f.name.toLowerCase().includes(q) || f.extension.toLowerCase().includes(q)
    );
  }, [items, searchQuery]);

  const totalBytes = useMemo(() => {
    return filteredItems.reduce((acc, f) => acc + f.size_bytes, 0);
  }, [filteredItems]);

  const oldestDays = useMemo(() => {
    if (filteredItems.length === 0) return 0;
    return Math.max(...filteredItems.map((f) => f.days_old));
  }, [filteredItems]);

  const handleReveal = async (path: string) => {
    setRevealingPath(path);
    try {
      await revealPathInExplorer(path);
    } catch (err) {
      console.warn("Reveal failed:", err);
    } finally {
      setTimeout(() => setRevealingPath(null), 1000);
    }
  };

  return (
    <div className={`space-y-4 ${className}`}>
      {/* Controls Header */}
      <div className="bg-surface border border-line rounded-xl p-5 shadow-sm space-y-4">
        <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Clock className="w-4 h-4 text-accent" />
            <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono">
              Забытые файлы · Сканер неиспользуемых файлов
            </span>
          </div>

          <div className="flex items-center gap-2 w-full md:w-auto justify-end">
            <button
              onClick={() => void loadStaleFiles()}
              disabled={loading || !selectedFolder}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-2 border border-line text-txt hover:bg-surface text-xs font-mono transition-colors disabled:opacity-40"
              title="Запустить повторный поиск"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${loading ? "animate-spin text-accent" : ""}`} />
              <span>Поиск</span>
            </button>
          </div>
        </div>

        {/* Quick Directory Chips */}
        {quickDirs.length > 0 && (
          <div className="space-y-1.5">
            <span className="text-[11px] text-muted font-mono block">Быстрый выбор папки:</span>
            <div className="flex flex-wrap gap-2">
              {quickDirs.map((dir) => {
                const isSelected = selectedFolder === dir.path;
                return (
                  <button
                    key={dir.id}
                    onClick={() => setSelectedFolder(dir.path)}
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
          </div>
        )}

        {/* Target Path & Age Threshold */}
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 pt-2 border-t border-line/60">
          <div className="sm:col-span-2">
            <label className="block text-[11px] font-mono text-muted mb-1 flex items-center gap-1">
              <FolderDown className="w-3 h-3 text-muted" />
              Путь к сканируемому каталогу
            </label>
            <input
              type="text"
              value={selectedFolder}
              onChange={(e) => setSelectedFolder(e.target.value)}
              placeholder="C:\Users\Username\Downloads..."
              className="w-full bg-surface-2 border border-line rounded-lg px-3 py-1.5 text-xs font-mono text-txt placeholder:text-dim focus:outline-none focus:border-accent"
            />
          </div>

          <div>
            <label className="block text-[11px] font-mono text-muted mb-1 flex items-center gap-1">
              <Calendar className="w-3 h-3 text-muted" />
              Порог неактивности
            </label>
            <select
              value={olderThanDays}
              onChange={(e) => setOlderThanDays(Number(e.target.value))}
              className="w-full bg-surface-2 border border-line rounded-lg px-2.5 py-1.5 text-xs font-mono text-txt focus:outline-none focus:border-accent"
            >
              <option value={30}>&ge; 30 дней (1 месяц)</option>
              <option value={90}>&ge; 90 дней (3 месяца)</option>
              <option value={180}>&ge; 180 дней (полгода)</option>
              <option value={365}>&ge; 365 дней (1 год)</option>
              <option value={730}>&ge; 730 дней (2 года)</option>
            </select>
          </div>
        </div>

        {/* Search Query Filter */}
        <div>
          <div className="relative">
            <Search className="w-3.5 h-3.5 absolute left-3 top-2.5 text-dim" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Быстрый поиск по имени файла или расширению..."
              className="w-full bg-surface-2 border border-line rounded-lg pl-9 pr-3 py-1.5 text-xs font-mono text-txt placeholder:text-dim focus:outline-none focus:border-accent"
            />
          </div>
        </div>

        {/* Aggregated Stats Row */}
        <div className="flex flex-wrap items-center gap-3 pt-2 text-xs font-mono text-muted border-t border-line/40">
          <span>
            Забытых файлов: <strong className="text-txt">{filteredItems.length}</strong>
          </span>
          <span>•</span>
          <span>
            Занимаемый объём: <strong className="text-accent">{formatBytes(totalBytes)}</strong>
          </span>
          {oldestDays > 0 && (
            <>
              <span>•</span>
              <span>
                Максимальный возраст: <strong className="text-txt">{oldestDays} дн.</strong>
              </span>
            </>
          )}
        </div>
      </div>

      {/* Error state */}
      {error && (
        <div className="p-4 rounded-xl bg-amber-950/20 border border-amber-600/40 text-amber-200 text-xs font-mono flex items-start gap-2">
          <AlertTriangle className="w-4 h-4 shrink-0 text-amber-400 mt-0.5" />
          <p className="font-semibold">{error}</p>
        </div>
      )}

      {/* Results Table */}
      <div className="bg-surface border border-line rounded-xl overflow-hidden shadow-sm">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs font-mono">
            <thead>
              <tr className="border-b border-line bg-surface-2 text-muted uppercase tracking-wider text-[11px]">
                <th className="py-2.5 px-4 font-semibold">Имя файла</th>
                <th className="py-2.5 px-4 font-semibold">Возраст</th>
                <th className="py-2.5 px-4 font-semibold text-right">Размер</th>
                <th className="py-2.5 px-4 font-semibold text-right">Действие</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line/60">
              {filteredItems.length === 0 ? (
                <tr>
                  <td colSpan={4} className="py-8 text-center text-muted">
                    {loading ? (
                      <div className="flex items-center justify-center gap-2">
                        <RefreshCw className="w-4 h-4 animate-spin text-accent" />
                        <span>Поиск файлов без изменений старше {olderThanDays} дней...</span>
                      </div>
                    ) : (
                      `В каталоге не обнаружено файлов старше ${olderThanDays} дней.`
                    )}
                  </td>
                </tr>
              ) : (
                filteredItems.map((file, idx) => {
                  const isRevealing = revealingPath === file.path;
                  return (
                    <tr
                      key={`${file.path}-${idx}`}
                      className="hover:bg-surface-2/60 transition-colors"
                    >
                      <td className="py-2.5 px-4 max-w-[320px] lg:max-w-[480px]">
                        <div className="flex items-center gap-2">
                          <FileText className="w-4 h-4 shrink-0 text-muted" />
                          <div className="truncate">
                            <span className="font-medium text-txt truncate block" title={file.name}>
                              {file.name}
                            </span>
                            <span className="text-[10px] text-dim truncate block" title={file.path}>
                              {file.path}
                            </span>
                          </div>
                        </div>
                      </td>

                      <td className="py-2.5 px-4 whitespace-nowrap">
                        <div className="flex items-center gap-1.5">
                          <span className="inline-block px-1.5 py-0.5 rounded bg-surface-2 border border-line text-txt text-[10px] font-semibold">
                            {file.days_old} дн.
                          </span>
                          <span className="text-[10px] text-dim">
                            ({file.last_modified_utc.split("T")[0]})
                          </span>
                        </div>
                      </td>

                      <td className="py-2.5 px-4 text-right font-bold text-txt whitespace-nowrap">
                        {formatBytes(file.size_bytes)}
                      </td>

                      <td className="py-2.5 px-4 text-right whitespace-nowrap">
                        <button
                          onClick={() => void handleReveal(file.path)}
                          disabled={isRevealing}
                          className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded bg-surface-2 border border-line text-txt hover:bg-surface hover:border-line-hover text-[11px] transition-colors disabled:opacity-40"
                          title="Показать файл в Проводнике Windows"
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
  );
};
