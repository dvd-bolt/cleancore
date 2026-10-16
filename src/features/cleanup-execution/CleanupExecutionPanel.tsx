import { CleanupSteps } from "../../components/CleanupSteps";
import { CleanupProgress } from "../../components/CleanupProgress";
import { isTauriEnvironment } from "../../lib/ipc";
import { useEffect, useRef, useState } from "react";
import { getAvailability, getExecutionPreview, prepareConfirmation, executeCleanup, getOperation, cancelCleanup } from "./ipc";
import type { CleanupAvailability, CleanupConfirmation, CleanupOutcome, ExecutionPreview } from "./types";
import { OutcomeView, formatBytes } from "./OutcomeView";
import { ConfirmationReview } from "./ConfirmationReview";

export function CleanupExecutionPanel({ previewId = null, selectedIds, onRunningChange, onCompleted, onOpenJournal }: { previewId?: string | null; selectedIds?: string[]; onRunningChange?: (running: boolean) => void; onCompleted?: (outcome: CleanupOutcome) => void; onOpenJournal?: () => void }) {
  const panel = useRef<HTMLElement>(null);
  const [preview, setPreview] = useState<ExecutionPreview | null>(null);
  const [availability, setAvailability] = useState<CleanupAvailability | null>(null);
  const [internalSelected, setSelected] = useState<string[]>([]);
  const selected = selectedIds ?? internalSelected;
  const [confirmation, setConfirmation] = useState<CleanupConfirmation | null>(null);
  useEffect(() => { if (confirmation) panel.current?.scrollIntoView?.({ block: "start" }); }, [confirmation]);
  const [acknowledged, setAcknowledged] = useState(false);
  const [outcome, setOutcome] = useState<CleanupOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);
  const [clock, setClock] = useState(Date.now());
  const guard = useRef(false);
  const notified = useRef<string | null>(null);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; let disposed = false;
    setPreview(null); setConfirmation(null); setAcknowledged(false); setSelected([]); setOutcome(null);
    Promise.all([getAvailability(), getExecutionPreview(previewId), isTauriEnvironment() && sessionStorage.getItem("cleancore-cleanup-operation") ? getOperation(sessionStorage.getItem("cleancore-cleanup-operation")!) : Promise.resolve(null)]).then(([a,p,o]) => { if (!disposed) { setAvailability(a); setPreview(p); if (o && (o.status === "Running" || !previewId || o.preview_id === previewId)) { setOutcome(o); setRunning(o.status === "Running"); } else { sessionStorage.removeItem("cleancore-cleanup-operation"); setRunning(false); } } }).catch(e => { if (!disposed) setError(String(e)); });
    const timer = window.setInterval(() => setClock(Date.now()), 1000);
    return () => { disposed = true; mounted.current = false; window.clearInterval(timer); };
  }, [previewId]);
  useEffect(() => { setConfirmation(null); setAcknowledged(false); }, [selectedIds]);
  useEffect(() => { onRunningChange?.(running); }, [running, onRunningChange]);
  useEffect(() => {
    if (outcome && outcome.status !== "Running" && outcome.preview_id === previewId && notified.current !== outcome.operation_id) {
      notified.current = outcome.operation_id; onCompleted?.(outcome);
    }
  }, [outcome, previewId, onCompleted]);
  useEffect(() => {
    if (!running || !outcome?.operation_id) return;
    const operationId = outcome.operation_id; let disposed = false; let pending = false;
    const timer = window.setInterval(async () => { if (pending) return; pending = true;
      try { const value = await getOperation(operationId); if (!disposed && value) { setOutcome(value); if (value.status !== "Running") { setRunning(false); setBusy(false); guard.current = false; } } }
      catch (e) { if (!disposed) setError(String(e)); } finally { pending = false; }
    }, 500);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [running, outcome?.operation_id]);
  const selectedBytes = preview ? preview.candidates.filter(item => selected.includes(item.object_id)).reduce((sum, item) => sum + BigInt(item.size_bytes), 0n).toString() : null;
  async function refresh() { if (guard.current || running) return; guard.current = true; setBusy(true); setError(null);
    try { const [a,p] = await Promise.all([getAvailability(), getExecutionPreview(previewId)]); if (mounted.current) { setAvailability(a); setPreview(p); setSelected([]); setConfirmation(null); setAcknowledged(false); } }
    catch (e) { if (mounted.current) setError(String(e)); } finally { guard.current = false; if (mounted.current) setBusy(false); }
  }
  async function prepare() { if (!preview || !selected.length || guard.current) return; guard.current = true; setBusy(true); setError(null);
    try { const c = await prepareConfirmation(preview.preview_id, selected); if (mounted.current) { setConfirmation(c); setAcknowledged(false); } }
    catch (e) { if (mounted.current) setError(String(e)); } finally { guard.current = false; if (mounted.current) setBusy(false); }
  }
  async function execute() { if (!confirmation || !acknowledged || !availability?.execution_enabled || clock >= confirmation.expires_at_ms || guard.current) return;
    guard.current = true; setBusy(true); setRunning(true); setError(null); setOutcome(null);
    try { const result = await executeCleanup(confirmation.confirmation_id, acknowledged); sessionStorage.setItem("cleancore-cleanup-operation", result.operation_id); if (mounted.current) { setOutcome(result); setRunning(result.status === "Running"); setConfirmation(null); setPreview(null); setSelected([]); } }
    catch (e) { if (mounted.current) { setError(String(e)); setRunning(false); setConfirmation(null); setAcknowledged(false); } }
    finally { if (mounted.current) setBusy(false); guard.current = false; }
  }
  async function cancel() { if (!outcome) return; try { const accepted = await cancelCleanup(outcome.operation_id); if (mounted.current) setError(accepted ? "Запрошена отмена. Уже обработанные файлы не откатываются." : "Операция уже завершена."); } catch(e) { if (mounted.current) setError(String(e)); } }
  return <section ref={panel} className={`${selectedIds !== undefined && !confirmation ? "action-panel" : "bg-surface border border-line rounded-xl p-5 space-y-4"}`} aria-label="Исполнение очистки">
    <CleanupSteps step={outcome || running ? 4 : confirmation ? 3 : preview ? 2 : 1}/>
    <div className="flex flex-wrap items-start justify-between gap-3"><div><p className="font-mono text-[10px] text-dim tracking-widest">УПРАВЛЕНИЕ ОЧИСТКОЙ</p><h2 className="font-heading text-xl mt-1">{outcome ? "Результат обработки" : `Выбранные файлы: ${selected.length}`}</h2></div>
      <button disabled={busy || running} onClick={refresh} className="border border-line rounded-lg px-3 py-2 text-xs disabled:opacity-40">Обновить список</button></div>
    {selectedBytes && <p className="text-sm font-semibold">Объём выбранных файлов: {formatBytes(selectedBytes)}</p>}
    {!outcome && <p role="status" className="text-xs text-muted">{availability?.reason ?? "Проверяем доступность исполнителя…"}</p>}
    {error && <p role="alert" className="text-xs text-warn break-words">{error}</p>}
    {!preview && !outcome && <p className="text-sm text-muted">Запустите анализ и выберите проверенные файлы. Отмена, ошибка корня или достижение лимита запрещают очистку.</p>}
    {preview && !confirmation && <>{selectedIds === undefined && <ul className="divide-y divide-line max-h-80 overflow-y-auto">{preview.candidates.map(c => <li key={c.object_id} className="py-3">
      <label className="flex gap-3 items-start text-xs"><input type="checkbox" disabled={busy || clock >= preview.expires_at_ms} checked={selected.includes(c.object_id)} onChange={e => { setSelected(old => e.target.checked ? [...old,c.object_id] : old.filter(id => id !== c.object_id)); setAcknowledged(false); }} /><span className="min-w-0"><span className="block break-all text-txt select-text">{c.display_path}</span><span className="block text-muted mt-1">{formatBytes(c.size_bytes)} · {c.reason}</span></span></label>
    </li>)}</ul>}<button onClick={prepare} disabled={busy || !selected.length || clock >= preview.expires_at_ms} className="bg-accent text-accent-ink rounded-lg px-4 py-2 text-sm disabled:opacity-40">Проверить выбранные ({selected.length})</button></>}
    {confirmation && <ConfirmationReview confirmation={confirmation} acknowledged={acknowledged} allowed={!!availability?.execution_enabled} busy={busy} expired={clock >= confirmation.expires_at_ms} onAcknowledge={setAcknowledged} onExecute={execute} onBack={() => { setConfirmation(null); setAcknowledged(false); }} />}
    {running && <p role="status" className="text-xs text-muted">Операция выполняется. Повторный запуск заблокирован.</p>}

    {outcome && <>
      <CleanupProgress running={running} total={outcome.items.length} processed={outcome.items.filter(item => item.status !== "Pending" && item.status !== "Cancelled").length} deleted={outcome.items.filter(item => item.status === "Deleted").length} skipped={outcome.items.filter(item => item.status === "Skipped").length} failed={outcome.items.filter(item => item.status === "Failed").length} bytes={formatBytes(outcome.deleted_bytes ?? "0")} onCancel={cancel} onOpenJournal={onOpenJournal} message={outcome.items.some(item => item.status === "Unknown") || !outcome.journal_ok ? "Есть неизвестные исходы или сбой журнала. Проверьте подробности." : outcome.status === "Cancelled" || outcome.status === "Interrupted" ? "Очистка остановлена. Уже обработанные файлы не восстанавливаются." : null}/>
      <details><summary>Подробные результаты по файлам</summary><OutcomeView outcome={outcome}/></details>
    </>}
  </section>;
}
