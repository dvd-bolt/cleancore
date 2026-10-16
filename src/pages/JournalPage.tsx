import { useRef, useState } from "react";
import { AuditHistory } from "../features/cleanup-execution/AuditHistory";
import { exportCleanupJournal, isTauriEnvironment } from "../lib/ipc";
export const JournalPage = () => {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const guard = useRef(false);
  async function exportJournal() {
    if (guard.current) return;
    guard.current = true; setBusy(true); setMessage(null);
    try {
      const path = await exportCleanupJournal();
      if (path) setMessage(`CSV-журнал сохранён: ${path}`);
    } catch (error) { setMessage(`Экспорт не выполнен: ${String(error)}`); }
    finally { guard.current = false; setBusy(false); }
  }
  return <div className="space-y-6">
    <div className="pb-3 border-b border-line/40">
      <p className="font-mono text-[10px] tracking-widest text-dim">ЛОКАЛЬНАЯ ИСТОРИЯ</p>
      <h2 className="font-heading text-2xl font-bold text-txt">Журнал операций</h2>
    </div>
    <button onClick={exportJournal} disabled={busy || !isTauriEnvironment()} className="border border-line rounded-lg px-4 py-2 text-sm disabled:opacity-40">{busy ? "Сохранение…" : "Экспорт последних 200 операций в CSV"}</button>
    {message && <p role="status" className="text-xs text-muted select-text">{message}</p>}
    <AuditHistory />
  </div>;
};
