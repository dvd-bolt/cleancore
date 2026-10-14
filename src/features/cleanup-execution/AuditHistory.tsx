import { useEffect, useRef, useState } from "react";
import { getJournal } from "./ipc";
import type { CleanupOutcome } from "./types";
import { OutcomeView } from "./OutcomeView";
import { isTauriEnvironment } from "../../lib/ipc";
export function AuditHistory() {
  const [operations,setOperations] = useState<CleanupOutcome[]>([]);
  const [cursor,setCursor] = useState<number | null>(null);
  const [loading,setLoading] = useState(true);
  const [error,setError] = useState<string | null>(null);
  const busy = useRef(false); const mounted = useRef(true);
  async function load(append: boolean) {
    if (busy.current) return; busy.current = true; setLoading(true); setError(null);
    try { const page = await getJournal(append ? cursor : null); if (mounted.current) { setOperations(old => append ? [...old,...page.operations] : page.operations); setCursor(page.next_cursor); } }
    catch(e) { if (mounted.current) setError(String(e)); }
    finally { busy.current = false; if (mounted.current) setLoading(false); }
  }
  useEffect(() => { mounted.current = true; void load(false); return () => { mounted.current = false; }; }, []);
  return <section className="space-y-4">
    <div className="flex justify-between items-center gap-3"><p className="text-xs text-muted">Намерения, результаты и причины пропуска. Автоматического повтора после сбоя нет.</p>
      <button onClick={() => load(false)} disabled={loading} className="border border-line rounded-lg px-3 py-2 text-xs disabled:opacity-40">Обновить журнал</button></div>
    {!isTauriEnvironment() && <p role="status" className="text-xs text-muted">Предпросмотр: постоянный журнал не подключён.</p>}
    {error && <p role="alert" className="text-xs text-warn">Журнал недоступен: {error}</p>}
    {loading && <p role="status" className="text-sm text-muted">Читаем журнал…</p>}
    {!loading && !error && !operations.length && <p className="bg-surface border border-line rounded-xl p-8 text-center text-muted">История пока пуста. Сюда попадут подтверждённые операции очистки.</p>}
    {operations.map(operation => <details key={operation.operation_id} className="bg-surface border border-line rounded-xl p-5">
      <summary className="cursor-pointer text-sm text-txt">{new Date(operation.started_at_ms).toLocaleString("ru-RU")} · {operation.items.length} объектов · {operation.category}</summary>
      <div className="pt-4"><OutcomeView outcome={operation}/></div>
    </details>)}
    {cursor !== null && <button onClick={() => load(true)} disabled={loading || operations.length >= 200} className="border border-line rounded-lg px-4 py-2 text-sm disabled:opacity-40">Показать ещё</button>}
    {operations.length >= 200 && <p className="text-xs text-muted">Показаны 200 операций. Обновите журнал, чтобы вернуться к последним.</p>}
  </section>;
}
