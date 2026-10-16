import { useEffect, useState } from "react";
import type { DiskAnalysisReport, TreemapData, TreemapNode } from "../../types";
import { getDiskTreemap, revealDiskEntry, prepareDiskFileCleanup, invalidateDiskReport } from "../../lib/ipc";
import { AccessibleDialog } from "../../components/AccessibleDialog";
import { MapCanvas, bytes } from "./MapCanvas";
import { CleanupExecutionPanel } from "../cleanup-execution/CleanupExecutionPanel";
import type { ExecutionPreview } from "../cleanup-execution/types";
import { extensionColor } from "./layout";
import { summaryMap } from "./summary";

export function DiskTreemap({ report, reportId, onNavigate }: { report: DiskAnalysisReport; reportId: string | null; onNavigate?: (path: string) => void }) {
  const [deletion, setDeletion] = useState<ExecutionPreview | null>(null);
  const [preparing, setPreparing] = useState(false);
  const [mapRunning, setMapRunning] = useState(false);
  const [stale, setStale] = useState(false);
  useEffect(() => { setStale(false); setDeletion(null); }, [reportId]);
  const [path, setPath] = useState(report.root_path);
  const [mode, setMode] = useState<"logical" | "allocated">("logical");
  const [data, setData] = useState<TreemapData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [selected, setSelected] = useState<TreemapNode | null>(null);
  const [filter, setFilter] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [height, setHeight] = useState(500);
  const [menu, setMenu] = useState<{ node: TreemapNode; x: number; y: number } | null>(null);
  useEffect(() => { setPath(report.root_path); if (report.engine !== "ntfs-mft") setMode("logical"); }, [report.root_path, report.engine, reportId]);
  useEffect(() => {
    let active = true;
    setData(null); setSelected(null); setMenu(null); setError(null); setNotice(null);
    if (report.engine !== "ntfs-mft") {
      setData(summaryMap(report));
      return;
    }
    if (!reportId) {
      setError("Отчёт больше не доступен. Повторите анализ диска.");
      return;
    }
    getDiskTreemap(reportId, path, mode).then(result => { if (active) setData(result); }).catch(error => { if (active) setError(error instanceof Error ? error.message : String(error)); });
    return () => { active = false; };
  }, [reportId, report, path, mode]);
  const jumpTo = (target: string) => { if (data?.indexed) setPath(target); else onNavigate?.(target); };
  const navigate = (node: TreemapNode) => {
    setMenu(null);
    if (node.is_aggregate && node.path === path) { setNotice("Эта группа объединяет объекты меньше доступного масштаба. Их суммарный размер учтён; содержимое можно просмотреть в Проводнике родительской папки."); return; }
    if (node.is_dir) jumpTo(node.path);
  };
  const copy = async (node: TreemapNode) => {
    setMenu(null);
    try { await navigator.clipboard.writeText(node.path); setNotice("Путь скопирован"); }
    catch { setNotice("Не удалось скопировать автоматически. Выделите путь в поле ниже и нажмите Ctrl+C."); }
  };
  const reveal = async (node: TreemapNode) => {
    setMenu(null);
    if (!reportId || node.is_aggregate || !data?.indexed) return;
    try { await revealDiskEntry(reportId, node.id); setNotice("Объект показан в Проводнике"); }
    catch (error) { setNotice(error instanceof Error ? error.message : String(error)); }
  };
  const prepareDelete = async (node: TreemapNode) => {
    setMenu(null);
    if (!reportId || node.is_aggregate || !data?.indexed || preparing || stale) return;
    setPreparing(true); setNotice(null);
    try { setDeletion(await prepareDiskFileCleanup(reportId, node.id)); }
    catch (e) { setNotice(e instanceof Error ? e.message : String(e)); }
    finally { setPreparing(false); }
  };
  const normalized = path.replace(/\//g, "\\");
  const segments = normalized.split("\\").filter(Boolean);
  const panel = <section className={`bg-surface border border-line rounded-xl p-4 space-y-3 ${expanded ? "w-full h-full flex flex-col" : ""}`} aria-label="Карта диска">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div><h2 className="text-sm font-semibold text-txt">Карта диска</h2><p className="text-xs text-muted mt-1">Площадь — размер · цвет — расширение · двойной щелчок — внутрь папки</p></div>
      <div className="flex items-center gap-2 text-xs">
        <label className="sr-only" htmlFor="treemap-size-mode">Размер для карты</label>
        <select id="treemap-size-mode" value={mode} disabled={report.engine !== "ntfs-mft"} onChange={e => setMode(e.target.value as typeof mode)} className="bg-surface-2 border border-line rounded-lg px-2 py-2 text-txt">
          <option value="logical">Размер файлов</option><option value="allocated">Занято на диске</option>
        </select>
        <button type="button" className="border border-line rounded-lg px-3 py-2 hover:bg-surface-2" onClick={() => setExpanded(v => !v)}>{expanded ? "Свернуть · Esc" : "Развернуть"}</button>
      </div>
    </div>
    <nav aria-label="Путь на карте" className="flex items-center gap-1 flex-wrap text-xs">
      <button className="border border-line rounded px-2 py-1 disabled:opacity-40" disabled={!data?.parent_path} onClick={() => data?.parent_path && jumpTo(data.parent_path)}>↑ Выше</button>
      {segments.map((segment, i) => <button key={i} className="rounded px-2 py-1 hover:bg-surface-2 break-all" onClick={() => jumpTo(segments.slice(0, i + 1).join("\\") + "\\")}>{segment}{i === 0 ? "\\" : ""}</button>)}
      <span className="ml-auto font-mono text-muted">{data ? bytes(data.root.size_bytes) : ""}</span>
    </nav>
    {stale && <p role="alert" className="text-sm text-warn border border-line rounded-lg p-3">После файловой операции карта и суммы устарели. Повторите анализ диска. До нового анализа удаление с этой карты отключено.</p>}
    {error && <p role="status" className="text-sm text-muted border border-line rounded-lg p-5">{error}</p>}
    {data && !data.indexed && <p className="text-xs text-muted">Сводная карта обычного обхода: папки и крупнейшие файлы. Подробная вложенная карта доступна после MFT-анализа целого NTFS-диска.</p>}
    {!error && <div className={`flex gap-3 min-h-0 ${expanded ? "flex-1" : ""}`}>
      <div className="relative min-w-0 flex-1 border border-line rounded overflow-hidden" style={expanded ? undefined : { height }}>
        {data ? <MapCanvas root={data.root} selected={selected?.id ?? null} filter={filter} onSelect={node => { setSelected(node); setMenu(null); }} onOpen={navigate} onContext={(node, x, y) => setMenu({ node, x, y })} /> : <div role="status" className="h-full flex items-center justify-center text-sm text-muted">Подготовка карты из индекса…</div>}
        {menu && <div role="menu" aria-label="Действия с объектом карты" className="absolute z-20 bg-[#232528] border border-white/20 shadow-xl rounded-lg p-1 text-xs" style={{ left: Math.max(0, menu.x - 160), top: Math.max(0, menu.y - 90) }} onKeyDown={e => { if (e.key === "Escape") setMenu(null); }}>
          <button role="menuitem" autoFocus className="block w-full text-left px-3 py-2 hover:bg-white/10" onClick={() => void copy(menu.node)}>Копировать путь</button>
          <button role="menuitem" disabled={menu.node.is_aggregate || !data?.indexed} className="block w-full text-left px-3 py-2 hover:bg-white/10 disabled:opacity-40" onClick={() => void reveal(menu.node)}>Показать в Проводнике</button>
          <button role="menuitem" disabled={menu.node.is_aggregate || !data?.indexed || preparing || stale} title="Файлы и небольшие папки в пользовательских каталогах; без Корзины" className="block w-full text-left px-3 py-2 hover:bg-white/10 disabled:opacity-40" onClick={() => void prepareDelete(menu.node)}>Удалить…</button>
          <button role="menuitem" className="block w-full text-left px-3 py-2 hover:bg-white/10" onClick={() => setMenu(null)}>Закрыть</button>
        </div>}
      </div>
    </div>}
    <div className="flex flex-wrap items-center gap-3 text-xs text-muted">
      {!expanded && <label className="flex gap-2 items-center">Высота карты<input aria-label="Высота карты" type="range" min={320} max={900} step={20} value={height} onChange={e => setHeight(Number(e.target.value))} /></label>}
      <span>Белая рамка — выбранный объект. Серые группы сохраняют размер мелких объектов.</span>
      {mode === "allocated" && <span>Данные внутри MFT не занимают отдельных кластеров и могут иметь нулевую площадь.</span>}
    </div>
    {data && <div className="flex flex-wrap gap-1.5 max-h-24 overflow-y-auto" aria-label="Легенда расширений">
      <button className="text-xs border border-line rounded px-2 py-1" aria-pressed={filter === null} onClick={() => setFilter(null)}>Все типы</button>
      {data.extensions.slice(0, 128).map(ext => <button key={ext.extension} disabled={ext.extension === "*"} aria-pressed={filter === ext.extension} title={`${bytes(ext.bytes)} · ${ext.files.toLocaleString("ru-RU")} файлов`} className={`flex gap-1.5 items-center text-xs rounded border px-2 py-1 ${filter === ext.extension ? "border-white" : "border-line"}`} onClick={() => setFilter(filter === ext.extension ? null : ext.extension)}><span className="w-2 h-2 rounded-sm" style={{ background: extensionColor(ext.extension) }} />{ext.extension === "*" ? "Другие расширения" : ext.extension ? `.${ext.extension}` : "Без расширения"}</button>)}
    </div>}
    {selected && <div className="border border-line rounded-lg p-3 text-xs space-y-2" aria-live="polite">
      <div className="flex items-center justify-between gap-3"><strong className="break-all">{selected.name}</strong><span className="shrink-0 font-mono">{bytes(selected.size_bytes)}</span></div>
      <input aria-label="Путь выбранного объекта" readOnly value={selected.path} className="w-full bg-surface-2 rounded px-2 py-1 text-muted" onFocus={e => e.target.select()} />
      <div className="flex flex-wrap gap-3 text-muted"><span>Размер: {bytes(selected.logical_bytes)}</span><span>Выделено: {selected.allocated_bytes == null ? "нет данных" : bytes(selected.allocated_bytes)}</span>{selected.is_dir && <span>{selected.file_count.toLocaleString("ru-RU")} файлов</span>}</div>
      <div className="flex flex-wrap gap-2"><button className="border border-line rounded px-2 py-1" onClick={() => void copy(selected)}>Копировать путь</button><button disabled={selected.is_aggregate || !data?.indexed} className="border border-line rounded px-2 py-1 disabled:opacity-40" onClick={() => void reveal(selected)}>Показать в Проводнике</button>{<button disabled={selected.is_aggregate || !data?.indexed || preparing || stale} className="border border-line rounded px-2 py-1 disabled:opacity-40" onClick={() => void prepareDelete(selected)}>Удалить…</button>}{selected.is_dir && <button className="border border-line rounded px-2 py-1" onClick={() => navigate(selected)}>Показать папку на карте</button>}</div>
    </div>}
    {preparing && <p role="status" className="text-xs text-muted">Проверяем файл и готовим свежий предпросмотр…</p>}
    {deletion && <AccessibleDialog title="Ручное удаление выбранных объектов" busy={mapRunning} onClose={() => { if (!mapRunning) setDeletion(null); }}><div className="w-full max-w-4xl max-h-[90vh] overflow-y-auto rounded-xl bg-surface p-5"><button disabled={mapRunning} onClick={() => setDeletion(null)} className="mb-4 border border-line rounded px-3 py-2 text-xs disabled:opacity-40">Закрыть предпросмотр</button><p className="text-sm text-warn mb-4">Объекты выбраны вручную и могут содержать нужные данные. Для папки показан свежий список содержимого; новые файлы после подтверждения не удаляются. Доступно только безвозвратное удаление. Для сохранения возможности восстановления закройте это окно и используйте Проводник.</p><CleanupExecutionPanel previewId={deletion.preview_id} selectedIds={deletion.candidates.map(item => item.object_id)} onRunningChange={setMapRunning} onCompleted={outcome => { if (outcome.items.some(item => item.status === "Deleted" || item.status === "Recycled" || item.status === "Unknown")) { setStale(true); if (reportId) void invalidateDiskReport(reportId).catch(e => setNotice(String(e))); } }}/></div></AccessibleDialog>}
    {notice && <p role="status" className="text-xs text-muted">{notice}</p>}
    {!!data?.empty_file_count && <details className="text-xs text-muted"><summary>Пустые файлы в текущей папке: {data.empty_file_count} (первые 100)</summary><div className="max-h-24 overflow-auto mt-2">{data.empty_files.map(file => <button key={file.id} className="block py-1 text-left" onClick={() => setSelected(file)}>{file.name}</button>)}</div></details>}
  </section>;
  return expanded ? <AccessibleDialog title="Карта диска" onClose={() => setExpanded(false)}>{panel}</AccessibleDialog> : panel;
}
