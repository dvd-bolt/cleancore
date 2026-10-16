import { CleanupSteps } from "../../components/CleanupSteps";
import { CleanupProgress } from "../../components/CleanupProgress";
import { useEffect, useMemo, useRef, useState } from "react";
import type { CleanupRuleInfo } from "../../lib/ipc";
import { cancelGeneralCleanupJob, executeGeneralCleanup, formatCleanupBytes, getGeneralCleanupJob, getGeneralCleanupPage, prepareGeneralCleanup, type GeneralConfirmation, type GeneralIndex, type GeneralJob, type GeneralPage } from "./generalCleanupIpc";

const JOB_KEY = "cleancore-general-cleanup-job";
export function GeneralCleanupSelection({ index, rules, onRunningChange, onOpenJournal }: { index: GeneralIndex | null; rules: CleanupRuleInfo[]; onRunningChange: (running: boolean) => void; onOpenJournal?: () => void }) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [page, setPage] = useState(0), [query, setQuery] = useState("");
  const [files, setFiles] = useState<GeneralPage | null>(null), [loading, setLoading] = useState(false);
  const [showSelected, setShowSelected] = useState(false);
  const [confirmation, setConfirmation] = useState<GeneralConfirmation | null>(null);
  const [acknowledged, setAcknowledged] = useState(false), [acceptPartial, setAcceptPartial] = useState(false);
  const [job, setJob] = useState<GeneralJob | null>(null), [busy, setBusy] = useState(false);
  const [restoring, setRestoring] = useState(!!sessionStorage.getItem(JOB_KEY));
  const [error, setError] = useState<string | null>(null), [clock, setClock] = useState(Date.now());
  const confirmationPanel = useRef<HTMLDivElement>(null);
  useEffect(() => { if (confirmation) confirmationPanel.current?.scrollIntoView?.({ block: "start" }); }, [confirmation]);
  const guard = useRef(false), mounted = useRef(true);
  const filterSelection = useMemo(() => showSelected ? [...selected] : null, [showSelected, selected]);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => {
    let live = true; const id = sessionStorage.getItem(JOB_KEY);
    if (id) void getGeneralCleanupJob(id).then(value => { if (live) setJob(value); }).catch(e => { if (live) { setError(String(e)); sessionStorage.removeItem(JOB_KEY); } }).finally(() => { if (live) setRestoring(false); });
    return () => { live = false; };
  }, []);
  useEffect(() => { if (!index) return; setSelected(new Set()); setPage(0); setQuery(""); setConfirmation(null); setAcknowledged(false); setAcceptPartial(false); setShowSelected(false); setError(null); setJob(null); sessionStorage.removeItem(JOB_KEY); }, [index?.session_id]);
  useEffect(() => { onRunningChange(busy || restoring || job?.status === "Running"); return () => onRunningChange(false); }, [busy, restoring, job?.status, onRunningChange]);
  useEffect(() => { const timer = window.setInterval(() => setClock(Date.now()), 1000); return () => clearInterval(timer); }, []);
  useEffect(() => {
    if (job?.status !== "Running") return;
    let disposed = false, pending = false;
    const timer = window.setInterval(async () => { if (pending) return; pending = true;
      try { const value = await getGeneralCleanupJob(job.job_id); if (!disposed) setJob(value); }
      catch (e) { if (!disposed) setError(String(e)); } finally { pending = false; }
    }, 500);
    return () => { disposed = true; clearInterval(timer); };
  }, [job?.job_id, job?.status]);
  useEffect(() => {
    if (!index) { setFiles(null); return; }
    let live = true; setLoading(true);
    const timer = window.setTimeout(() => { void getGeneralCleanupPage(index.session_id, page * 50, query, filterSelection).then(value => { if (live) setFiles(value); }).catch(e => { if (live) setError(String(e)); }).finally(() => { if (live) setLoading(false); }); }, 150);
    return () => { live = false; clearTimeout(timer); };
  }, [index?.session_id, page, query, filterSelection]);
  const groups = useMemo(() => rules.map(rule => {
    const items = index?.items.filter(item => item.category_id === rule.id) ?? [];
    return { rule, items, bytes: items.reduce((sum, item) => sum + BigInt(item.size_bytes), 0n).toString() };
  }).filter(group => group.items.length > 0), [index, rules]);
  const selectedBytes = useMemo(() => (index?.items.reduce((sum, item) => sum + (selected.has(item.object_id) ? BigInt(item.size_bytes) : 0n), 0n) ?? 0n).toString(), [index, selected]);
  const expired = !!index && clock >= index.expires_at_ms;
  const locked = busy || restoring || !!confirmation || !!job || !!index?.consumed || expired;
  function toggle(ids: string[], checked: boolean) { setPage(0); setSelected(old => { const next = new Set(old); for (const id of ids) { if (checked) next.add(id); else next.delete(id); } return next; }); }
  async function prepare() {
    if (!index || !selected.size || locked || guard.current) return;
    guard.current = true; setBusy(true); setError(null);
    try { const value = await prepareGeneralCleanup(index.session_id, [...selected], acceptPartial); if (mounted.current) { setClock(Date.now()); setConfirmation(value); setAcknowledged(false); setShowSelected(true); setPage(0); setQuery(""); } }
    catch (e) { if (mounted.current) setError(String(e)); } finally { guard.current = false; if (mounted.current) setBusy(false); }
  }
  async function execute() {
    if (!confirmation || !acknowledged || Date.now() >= confirmation.expires_at_ms || guard.current) return;
    guard.current = true; setBusy(true); setError(null);
    try { const value = await executeGeneralCleanup(confirmation.confirmation_id, true); sessionStorage.setItem(JOB_KEY, value.job_id); if (mounted.current) { setJob(value); setConfirmation(null); } }
    catch (e) { if (mounted.current) { setError(String(e)); setConfirmation(null); setAcknowledged(false); } }
    finally { guard.current = false; if (mounted.current) setBusy(false); }
  }
  return <section className="space-y-4" aria-label="Выбор и общая очистка">
    {(index || job || restoring) && <>
    <div className="action-panel" aria-label="Управление общей очисткой">
      <CleanupSteps step={job ? 4 : confirmation ? 3 : 2}/>
      {!job && index && <>      <p className="text-sm font-semibold" role="status">Выбрано: {selected.size} · примерно {formatCleanupBytes(selectedBytes)}</p>
      <button title={expired ? "Повторите анализ: список устарел" : !selected.size ? "Выберите категории или отдельные файлы" : "Перейти к проверке точного списка"} disabled={locked || !selected.size || ((index.partial || index.truncated) && !acceptPartial)} onClick={() => void prepare()} className="bg-accent text-accent-ink rounded-lg px-4 py-2 text-sm disabled:opacity-40">Очистить выбранное ({selected.size})</button>
      {expired && <p className="text-xs text-warn">Список устарел. Повторите общий анализ.</p>}
      {!selected.size && <p className="text-xs text-muted">Выберите категории или отдельные файлы ниже.</p>}
      {restoring && <p role="status">Восстанавливаем состояние очистки…</p>}
      {busy && <p role="status">Проверяем выбранные файлы…</p>}
      </>}
    {job && <div aria-label="Результат общей очистки">
      <CleanupProgress running={job.status === "Running"} processed={job.processed} total={job.selected_count} deleted={job.deleted} skipped={job.skipped} failed={job.failed} bytes={formatCleanupBytes(job.deleted_bytes)} message={job.message} onCancel={() => { void cancelGeneralCleanupJob(job.job_id).catch(e => setError(String(e))); }} onOpenJournal={onOpenJournal}/>
      <p className="font-semibold mt-2">{job.status === "Running" ? "" : job.status === "Completed" ? "Очистка завершена" : "Очистка остановлена"}</p>
      {job.status !== "Running" && <p className="text-xs text-muted">Список использован. Повторите общий анализ для новой очистки. Результаты и причины пропусков сохранены в разделе «Журнал».</p>}
    </div>}
    {error && <p role="alert" className="text-xs text-warn break-words">{error}</p>}
    </div>
    </>}
    {!index && !job && error && <p role="alert" className="text-sm text-warn">{error}</p>}
    {index && <>
      <div className="border border-line rounded-xl p-5 space-y-2"><p className="text-xs text-muted">НАЙДЕНО ДЛЯ ОЧИСТКИ</p><h3 className="font-heading text-2xl font-bold">Можно освободить до {formatCleanupBytes(index.total_bytes)}</h3><p className="text-xs text-muted">{index.items.length} уникальных файлов. Повторы исключены. Это размер файлов: фактически свободное место может измениться иначе. Занятые и изменённые файлы будут пропущены.</p></div>
      {(index.partial || index.truncated) && <label className="flex gap-3 text-sm text-warn"><input type="checkbox" checked={acceptPartial} disabled={locked} onChange={e => setAcceptPartial(e.target.checked)}/>{index.truncated ? "Список ограничен 50 000 файлами. " : "Часть категорий или каталогов недоступна. "}Разрешаю очистку только выбранных проверенных файлов.</label>}
      <div className="flex flex-wrap gap-3"><button disabled={locked || !index.items.length} className="border border-line rounded-lg px-3 py-2 text-sm disabled:opacity-40" onClick={() => toggle(index.items.map(item => item.object_id), true)}>Выбрать всё</button><button disabled={locked || !selected.size} className="border border-line rounded-lg px-3 py-2 text-sm disabled:opacity-40" onClick={() => setSelected(new Set())}>Снять выбор</button></div>
      <ul className="divide-y divide-line">{groups.map(({ rule, items, bytes }) => <li key={rule.id} className="py-3"><label className="flex items-start gap-3 text-sm"><input type="checkbox" aria-label={`Очистить категорию ${rule.name}`} checked={items.every(item => selected.has(item.object_id))} disabled={locked} onChange={e => toggle(items.map(item => item.object_id), e.target.checked)}/><span><span className="block">{rule.name} · {formatCleanupBytes(bytes)} · выбрано {items.filter(item => selected.has(item.object_id)).length} из {items.length}</span><span className="block text-xs text-muted mt-1">{rule.description}</span></span></label></li>)}</ul>
      {confirmation && <div ref={confirmationPanel} className="scroll-mt-52 border border-accent rounded-xl p-5 space-y-3" aria-label="Подтверждение общей очистки"><h3 className="font-heading text-xl">Подтверждение выбранных файлов</h3><p className="text-sm">{confirmation.selected_count} файлов · примерно {formatCleanupBytes(confirmation.selected_bytes)}. Точный выбранный список — ниже.</p><p className="text-xs text-warn">Файлы будут удалены безвозвратно, без Корзины и восстановления. Перед каждым действием файл, его родители, возраст, исключения и состояние браузера проверяются повторно.</p><label className="flex gap-3 text-sm"><input type="checkbox" checked={acknowledged} disabled={busy} onChange={e => setAcknowledged(e.target.checked)}/>Подтверждаю безвозвратное удаление именно выбранных файлов.</label><div className="flex gap-3"><button disabled={busy} onClick={() => { setConfirmation(null); setAcknowledged(false); }} className="border border-line rounded-lg px-3 py-2 text-sm">Назад к выбору</button><button disabled={!acknowledged || busy || clock >= confirmation.expires_at_ms} onClick={() => void execute()} className="button-danger rounded-lg px-4 py-2 text-sm">Подтвердить и очистить</button></div>{clock >= confirmation.expires_at_ms && <p role="alert" className="text-xs text-warn">Подтверждение устарело. Вернитесь к выбору.</p>}</div>}
      <label className="block text-xs text-muted">Поиск файлов<input aria-label="Поиск файлов общей очистки" type="search" maxLength={128} value={query} onChange={e => { setQuery(e.target.value); setPage(0); }} className="block w-full mt-2 bg-shell border border-line rounded-lg px-3 py-2"/></label>
      {!confirmation && <label className="flex gap-2 text-xs text-muted"><input type="checkbox" checked={showSelected} onChange={e => { setShowSelected(e.target.checked); setPage(0); }}/>Показывать только выбранные</label>}
      <p className="text-xs text-muted">{confirmation ? "Точный выбранный список" : "Найденные файлы"} · страница {page + 1} · по запросу: {files?.total ?? 0}</p>
      {loading ? <p role="status" className="text-xs text-muted">Загрузка списка…</p> : <ul className="divide-y divide-line">{files?.items.map(item => <li key={item.object_id} className="py-2"><label className="flex items-start gap-3 text-xs"><input type="checkbox" aria-label={`Выбрать ${item.display_path}`} checked={selected.has(item.object_id)} disabled={locked} onChange={e => toggle([item.object_id], e.target.checked)}/><span className="break-all select-text">{item.display_path}<span className="block text-muted mt-1">{formatCleanupBytes(item.size_bytes)} · {rules.find(rule => rule.id === item.category_id)?.name}</span></span></label></li>)}</ul>}
      <div className="flex gap-3"><button disabled={loading || page === 0} onClick={() => setPage(value => value - 1)} className="border border-line rounded px-3 py-1 text-xs disabled:opacity-40">Предыдущие файлы</button><button disabled={loading || (page + 1) * 50 >= (files?.total ?? 0)} onClick={() => setPage(value => value + 1)} className="border border-line rounded px-3 py-1 text-xs disabled:opacity-40">Следующие файлы</button></div>
    </>}

  </section>;
}
