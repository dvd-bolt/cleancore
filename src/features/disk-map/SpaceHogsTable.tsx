import React, { useState, useEffect, useMemo, useCallback } from "react";
import {
  LargeFileCategory,
  LargeFileEntry,
} from "../../types";
import { fetchSpaceHogs, revealPathInExplorer } from "../../lib/ipc";
import {
  HardDrive,
  Filter,
  Search,
  ExternalLink,
  ShieldAlert,
  Film,
  Archive,
  Disc,
  Boxes,
  Cpu,
  FileQuestion,
  RefreshCw,
  AlertTriangle,
} from "lucide-react";

interface SpaceHogsTableProps {
  className?: string;
}

const CATEGORY_LABELS: Record<LargeFileCategory, { label: string; icon: React.FC<{ className?: string }>; color: string }> = {
  DiskImage: { label: "Образы дисков", icon: Disc, color: "text-amber-400 bg-amber-500/10 border-amber-500/30" },
  Archive: { label: "Архивы", icon: Archive, color: "text-purple-400 bg-purple-500/10 border-purple-500/30" },
  Video: { label: "Видео", icon: Film, color: "text-sky-400 bg-sky-500/10 border-sky-500/30" },
  Installer: { label: "Установщики", icon: Boxes, color: "text-emerald-400 bg-emerald-500/10 border-emerald-500/30" },
  VirtualDisk: { label: "Виртуальные диски", icon: Cpu, color: "text-rose-400 bg-rose-500/10 border-rose-500/30" },
  Other: { label: "Прочее / Системное", icon: FileQuestion, color: "text-slate-400 bg-slate-500/10 border-slate-500/30" },
};

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Б";
  const k = 1024;
  const sizes = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(val >= 100 || i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export const SpaceHogsTable: React.FC<SpaceHogsTableProps> = ({ className = "" }) => {
  const [minSizeMb, setMinSizeMb] = useState<number>(500);
  const [selectedCategory, setSelectedCategory] = useState<LargeFileCategory | "ALL">("ALL");
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [items, setItems] = useState<LargeFileEntry[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [revealingPath, setRevealingPath] = useState<string | null>(null);

  const loadSpaceHogs = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const catArg = selectedCategory === "ALL" ? undefined : selectedCategory;
      const res = await fetchSpaceHogs(minSizeMb, catArg);
      setItems(res || []);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || "Не удалось загрузить список крупных файлов. Убедитесь, что выполнен анализ диска.");
    } finally {
      setLoading(false);
    }
  }, [minSizeMb, selectedCategory]);

  useEffect(() => {
    void loadSpaceHogs();
  }, [loadSpaceHogs]);

  const filteredItems = useMemo(() => {
    if (!searchQuery.trim()) return items;
    const q = searchQuery.toLowerCase();
    return items.filter((f) => f.path.toLowerCase().includes(q) || f.extension.toLowerCase().includes(q));
  }, [items, searchQuery]);

  const totalBytes = useMemo(() => {
    return filteredItems.reduce((acc, f) => acc + f.size_bytes, 0);
  }, [filteredItems]);

  const systemLockedCount = useMemo(() => {
    return filteredItems.filter((f) => f.is_system_locked).length;
  }, [filteredItems]);

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

  const extractFileName = (path: string) => {
    const parts = path.split(/[\\/]/);
    return parts[parts.length - 1] || path;
  };

  return (
    <div className={`space-y-4 ${className}`}>
      {/* Controls & Filter Bar */}
      <div className="bg-surface border border-line rounded-xl p-4 shadow-sm space-y-4">
        <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <HardDrive className="w-4 h-4 text-accent" />
            <span className="text-xs font-semibold text-txt uppercase tracking-wider font-mono">
              Space Hogs · Самые крупные файлы диска
            </span>
          </div>

          <div className="flex items-center gap-2 w-full md:w-auto justify-end">
            <button
              onClick={() => void loadSpaceHogs()}
              disabled={loading}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-2 border border-line text-txt hover:bg-surface text-xs font-mono transition-colors disabled:opacity-40"
              title="Обновить выборку крупных файлов"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${loading ? "animate-spin text-accent" : ""}`} />
              <span>Обновить</span>
            </button>
          </div>
        </div>

        {/* Filters */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3 pt-2 border-t border-line/60">
          {/* Min Size Selector */}
          <div>
            <label className="block text-[11px] font-mono text-muted mb-1 flex items-center gap-1">
              <Filter className="w-3 h-3 text-muted" />
              Минимальный размер
            </label>
            <select
              value={minSizeMb}
              onChange={(e) => setMinSizeMb(Number(e.target.value))}
              className="w-full bg-surface-2 border border-line rounded-lg px-2.5 py-1.5 text-xs font-mono text-txt focus:outline-none focus:border-accent"
            >
              <option value={100}>&ge; 100 МБ</option>
              <option value={500}>&ge; 500 МБ (стандарт)</option>
              <option value={1024}>&ge; 1 ГБ</option>
              <option value={2048}>&ge; 2 ГБ</option>
              <option value={5120}>&ge; 5 ГБ</option>
              <option value={10240}>&ge; 10 ГБ</option>
            </select>
          </div>

          {/* Category Selector */}
          <div>
            <label className="block text-[11px] font-mono text-muted mb-1">
              Категория
            </label>
            <select
              value={selectedCategory}
              onChange={(e) => setSelectedCategory(e.target.value as LargeFileCategory | "ALL")}
              className="w-full bg-surface-2 border border-line rounded-lg px-2.5 py-1.5 text-xs font-mono text-txt focus:outline-none focus:border-accent"
            >
              <option value="ALL">Все категории</option>
              <option value="DiskImage">Образы дисков (.iso, .img, .vhd)</option>
              <option value="Archive">Архивы (.zip, .7z, .rar, .tar)</option>
              <option value="Video">Видео (.mp4, .mkv, .avi, .mov)</option>
              <option value="Installer">Установщики (.exe, .msi)</option>
              <option value="VirtualDisk">Виртуальные диски (.vmdk, .vhdx, .qcow2)</option>
              <option value="Other">Прочее / Системное</option>
            </select>
          </div>

          {/* Search Input */}
          <div className="sm:col-span-2">
            <label className="block text-[11px] font-mono text-muted mb-1 flex items-center gap-1">
              <Search className="w-3 h-3 text-muted" />
              Поиск по имени или пути
            </label>
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Фильтр по имени файла..."
              className="w-full bg-surface-2 border border-line rounded-lg px-2.5 py-1.5 text-xs font-mono text-txt placeholder:text-dim focus:outline-none focus:border-accent"
            />
          </div>
        </div>

        {/* Stats Pill Row */}
        <div className="flex flex-wrap items-center gap-3 pt-2 text-xs font-mono text-muted border-t border-line/40">
          <span>
            Найдено файлов: <strong className="text-txt">{filteredItems.length}</strong>
          </span>
          <span>•</span>
          <span>
            Суммарный объём: <strong className="text-accent">{formatBytes(totalBytes)}</strong>
          </span>
          {systemLockedCount > 0 && (
            <>
              <span>•</span>
              <span className="flex items-center gap-1 text-amber-400">
                <ShieldAlert className="w-3.5 h-3.5" />
                {systemLockedCount} файлов занято системой (pagefile/hiberfil)
              </span>
            </>
          )}
        </div>
      </div>

      {/* Error state */}
      {error && (
        <div className="p-4 rounded-xl bg-amber-950/20 border border-amber-600/40 text-amber-200 text-xs font-mono flex items-start gap-2">
          <AlertTriangle className="w-4 h-4 shrink-0 text-amber-400 mt-0.5" />
          <div className="space-y-1">
            <p className="font-semibold">{error}</p>
            <p className="text-dim text-[11px]">
              Выборка Space Hogs извлекает данные из актуального снимка диска (NTFS MFT). Запустите «Анализ диска» на вкладке «Карта и объекты», если анализ ещё не выполнялся.
            </p>
          </div>
        </div>
      )}

      {/* Table */}
      <div className="bg-surface border border-line rounded-xl overflow-hidden shadow-sm">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs font-mono">
            <thead>
              <tr className="border-b border-line bg-surface-2 text-muted uppercase tracking-wider text-[11px]">
                <th className="py-2.5 px-4 font-semibold">Имя файла</th>
                <th className="py-2.5 px-4 font-semibold">Категория</th>
                <th className="py-2.5 px-4 font-semibold text-right">Размер</th>
                <th className="py-2.5 px-4 font-semibold text-center">Статус</th>
                <th className="py-2.5 px-4 font-semibold text-right">Действие</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line/60">
              {filteredItems.length === 0 ? (
                <tr>
                  <td colSpan={5} className="py-8 text-center text-muted">
                    {loading ? (
                      <div className="flex items-center justify-center gap-2">
                        <RefreshCw className="w-4 h-4 animate-spin text-accent" />
                        <span>Чтение данных из таблицы MFT...</span>
                      </div>
                    ) : (
                      "Крупные файлы не найдены с заданными параметрами фильтра."
                    )}
                  </td>
                </tr>
              ) : (
                filteredItems.map((file, idx) => {
                  const catMeta = CATEGORY_LABELS[file.category] || CATEGORY_LABELS.Other;
                  const Icon = catMeta.icon;
                  const fileName = extractFileName(file.path);
                  const isRevealing = revealingPath === file.path;

                  return (
                    <tr
                      key={`${file.path}-${idx}`}
                      className="hover:bg-surface-2/60 transition-colors"
                    >
                      {/* Name & Path */}
                      <td className="py-2.5 px-4 max-w-[280px] lg:max-w-[420px]">
                        <div className="flex items-center gap-2">
                          <Icon className="w-4 h-4 shrink-0 text-muted" />
                          <div className="truncate">
                            <span className="font-medium text-txt truncate block" title={fileName}>
                              {fileName}
                            </span>
                            <span className="text-[10px] text-dim truncate block" title={file.path}>
                              {file.path}
                            </span>
                          </div>
                        </div>
                      </td>

                      {/* Category Badge */}
                      <td className="py-2.5 px-4 whitespace-nowrap">
                        <span
                          className={`inline-flex items-center gap-1 px-2 py-0.5 rounded border text-[10px] font-medium ${catMeta.color}`}
                        >
                          {catMeta.label}
                        </span>
                      </td>

                      {/* Size */}
                      <td className="py-2.5 px-4 text-right font-bold text-txt whitespace-nowrap">
                        {formatBytes(file.size_bytes)}
                      </td>

                      {/* Status / System Lock */}
                      <td className="py-2.5 px-4 text-center whitespace-nowrap">
                        {file.is_system_locked ? (
                          <span
                            className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-red-950/40 border border-red-700/40 text-red-300 text-[10px]"
                            title="Файл используется или заблокирован операционной системой Windows"
                          >
                            <ShieldAlert className="w-3 h-3 text-red-400" />
                            Системный
                          </span>
                        ) : (
                          <span className="text-dim text-[11px]">—</span>
                        )}
                      </td>

                      {/* Action */}
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
