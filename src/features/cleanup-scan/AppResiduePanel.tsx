import { useState } from "react";
import { inspectAppResidue } from "../../lib/ipc";
import type { AppResidueReport } from "../../types";
import { formatBytes } from "../cleanup-execution/OutcomeView";
export function AppResiduePanel({ contextId, appName, onDismiss }: { contextId: string; appName: string; onDismiss: () => void }) {
  const [report, setReport] = useState<AppResidueReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function inspect() { if (busy) return; setBusy(true); setError(null); try { setReport(await inspectAppResidue(contextId)); } catch(e) { setError(e instanceof Error ? e.message : String(e)); } finally { setBusy(false); } }
  return <section aria-label="Возможные остатки приложения" className="bg-surface border border-line rounded-xl p-5 space-y-3 text-xs">
    <h3 className="text-sm font-semibold">После удаления: {appName}</h3>
    <p className="text-muted">Дождитесь завершения штатного деинсталлятора. Проверка ищет совпадения имён в LocalAppData/Roaming и оценивает логический объём. Профили и личные данные могут быть нужны; список не является разрешением удаления. Контекст действует 30 минут в текущем запуске.</p>
    <div className="flex gap-3"><button disabled={busy} onClick={() => void inspect()} className="border border-line rounded px-3 py-2 disabled:opacity-40">{busy ? "Проверяем…" : "Проверить возможные остатки"}</button><button disabled={busy} onClick={onDismiss} className="border border-line rounded px-3 py-2 disabled:opacity-40">Скрыть</button></div>
    {error && <p role="alert" className="text-warn">{error}</p>}
    {report && <>
      <p role="status" className="text-muted">{report.still_registered ? "Программа ещё зарегистрирована: показаны данные приложения, а не подтверждённые остатки." : "Запись программы не найдена. Это не доказывает, что найденные данные больше не нужны."} Проверено записей: {report.inspected_entries}. {!report.complete && "Анализ неполный."}</p>
      {report.warnings.map((warning,i) => <p key={i} className="text-warn">{warning}</p>)}
      <ul className="divide-y divide-line">{report.candidates.map(candidate => <li key={candidate.path} className="py-3 space-y-2"><input aria-label="Путь возможного остатка" readOnly value={candidate.path} onFocus={e => e.target.select()} className="w-full bg-shell border border-line rounded px-2 py-1"/><p>{candidate.reason} · Уверенность: {candidate.confidence}.</p><p className="text-muted">{candidate.logical_bytes == null ? "Объём недоступен" : formatBytes(candidate.logical_bytes)} · {candidate.files} файлов{candidate.complete ? "" : " · неполная оценка"}</p></li>)}</ul>
      {!report.candidates.length && <p>В поддерживаемых расположениях совпадений не найдено. Это не полный поиск по диску и реестру.</p>}
    </>}
  </section>;
}
