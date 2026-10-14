import { isTauriEnvironment } from "../../lib/ipc";
import { useEffect, useRef, useState } from "react";
import { getAvailability, getExecutionPreview, prepareConfirmation, executeCleanup, getOperation, cancelCleanup } from "./ipc";
import type { CleanupAvailability, CleanupConfirmation, CleanupOutcome, ExecutionPreview } from "./types";
import { OutcomeView, formatBytes } from "./OutcomeView";
import { ConfirmationReview } from "./ConfirmationReview";

export function CleanupExecutionPanel() {
  const [preview, setPreview] = useState<ExecutionPreview | null>(null);
  const [availability, setAvailability] = useState<CleanupAvailability | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [confirmation, setConfirmation] = useState<CleanupConfirmation | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);
  const [outcome, setOutcome] = useState<CleanupOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);
  const [clock, setClock] = useState(Date.now());
  const guard = useRef(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; let disposed = false;
    Promise.all([getAvailability(), getExecutionPreview(), isTauriEnvironment() && sessionStorage.getItem("cleancore-cleanup-operation") ? getOperation(sessionStorage.getItem("cleancore-cleanup-operation")!) : Promise.resolve(null)]).then(([a,p,o]) => { if (!disposed) { setAvailability(a); setPreview(p); if (o) { setOutcome(o); setRunning(o.status === "Running"); } } }).catch(e => { if (!disposed) setError(String(e)); });
    const timer = window.setInterval(() => setClock(Date.now()), 1000);
    return () => { disposed = true; mounted.current = false; window.clearInterval(timer); };
  }, []);
  useEffect(() => {
    if (!running || !outcome?.operation_id) return;
    const operationId = outcome.operation_id; let disposed = false; let pending = false;
    const timer = window.setInterval(async () => { if (pending) return; pending = true;
      try { const value = await getOperation(operationId); if (!disposed && value) { setOutcome(value); if (value.status !== "Running") { setRunning(false); setBusy(false); guard.current = false; } } }
      catch (e) { if (!disposed) setError(String(e)); } finally { pending = false; }
    }, 500);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [running, outcome?.operation_id]);
  async function refresh() { if (guard.current || running) return; guard.current = true; setBusy(true); setError(null);
    try { const [a,p] = await Promise.all([getAvailability(), getExecutionPreview()]); if (mounted.current) { setAvailability(a); setPreview(p); setSelected([]); setConfirmation(null); setAcknowledged(false); } }
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
  return <section className="bg-surface border border-line rounded-xl p-5 space-y-4" aria-label="Исполнение очистки">
    <div className="flex flex-wrap items-start justify-between gap-3"><div><p className="font-mono text-[10px] text-dim tracking-widest">ЭТАП 4 · ПОДТВЕРЖДЕНИЕ И РЕЗУЛЬТАТ</p><h2 className="font-heading text-xl mt-1">Только выбранный список.</h2></div>
      <button disabled={busy || running} onClick={refresh} className="border border-line rounded-lg px-3 py-2 text-xs disabled:opacity-40">Обновить preview</button></div>
    <p role="status" className="text-xs text-muted">{availability?.reason ?? "Проверяем доступность исполнителя…"}</p>
    {error && <p role="alert" className="text-xs text-warn break-words">{error}</p>}
    {!preview && !outcome && <p className="text-sm text-muted">Нет preview с идентичностью объектов. Завершите сканирование этапа 3 и подключите его снимок к исполнителю.</p>}
    {preview && !confirmation && <><ul className="divide-y divide-line max-h-80 overflow-y-auto">{preview.candidates.map(c => <li key={c.object_id} className="py-3">
      <label className="flex gap-3 items-start text-xs"><input type="checkbox" disabled={busy || clock >= preview.expires_at_ms} checked={selected.includes(c.object_id)} onChange={e => { setSelected(old => e.target.checked ? [...old,c.object_id] : old.filter(id => id !== c.object_id)); setAcknowledged(false); }} /><span className="min-w-0"><span className="block break-all text-txt select-text">{c.display_path}</span><span className="block text-muted mt-1">{formatBytes(c.size_bytes)} · {c.reason}</span></span></label>
    </li>)}</ul><button onClick={prepare} disabled={busy || !selected.length || clock >= preview.expires_at_ms} className="bg-accent text-accent-ink rounded-lg px-4 py-2 text-sm disabled:opacity-40">Проверить выбранные ({selected.length})</button></>}
    {confirmation && <ConfirmationReview confirmation={confirmation} acknowledged={acknowledged} allowed={!!availability?.execution_enabled} busy={busy} expired={clock >= confirmation.expires_at_ms} onAcknowledge={setAcknowledged} onExecute={execute} onBack={() => { setConfirmation(null); setAcknowledged(false); }} />}
    {running && <p role="status" className="text-xs text-muted">Операция выполняется. Повторный запуск заблокирован.</p>}
    {running && outcome && <button onClick={cancel} className="border border-line rounded-lg px-4 py-2 text-sm">Остановить следующие объекты</button>}
    {outcome && <OutcomeView outcome={outcome} />}
  </section>;
}
