export function CleanupProgress({ running, processed, total, deleted, skipped, failed, bytes, message, onCancel, onOpenJournal }: {
  running: boolean; processed: number; total: number; deleted: number; skipped: number; failed: number;
  bytes: string; message?: string | null; onCancel: () => void; onOpenJournal?: () => void;
}) {
  return <div className="space-y-2" aria-label="Ход и результат очистки">
    <h3 role="status" className="font-heading text-lg font-semibold">{running ? "Очистка выполняется" : "Обработка завершена"}</h3>
    <p className="text-sm">Обработано: {processed} из {total} · удалено: {deleted} · пропущено: {skipped} · ошибок: {failed}</p>
    {running && <progress aria-label="Прогресс очистки" value={processed} max={Math.max(1, total)} className="w-full h-2 accent-slate-200" />}
    <p className="text-xs text-muted">Удалено файлов на {bytes}. Это не замер свободного места.</p>
    {message && <p role="status" className="text-xs text-warn">{message}</p>}
    <div className="flex flex-wrap gap-3">
      {running && <button onClick={onCancel} className="border border-line rounded-lg px-3 py-2 text-sm">Остановить очистку</button>}
      {!running && onOpenJournal && <button onClick={onOpenJournal} className="border border-line rounded-lg px-3 py-2 text-sm">Открыть журнал</button>}
    </div>
  </div>;
}
