import { CleanupSteps } from "../../components/CleanupSteps";
import { useEffect, useRef, useState } from "react";
import { isTauriEnvironment, type CleanupRuleInfo } from "../../lib/ipc";
import { GeneralCleanupSelection } from "./GeneralCleanupSelection";
import { createGeneralCleanup, collectGeneralCleanup, finishGeneralCleanup, formatCleanupBytes as formatBytes, type GeneralIndex } from "./generalCleanupIpc";
import { scanReason } from "./diagnostics";
import { generalRules, runGeneralAnalysis, type GeneralResult } from "./generalAnalysis";

const stateLabels: Record<GeneralResult["state"], string> = {
  pending: "Ожидает", running: "Анализируется", complete: "Готово", partial: "Неполный результат", unavailable: "Недоступно", cancelled: "Остановлено",
};

export function GeneralCleanupAnalysis({ rules, onOpenCategory, onRunningChange, onOpenJournal }: {
  rules: CleanupRuleInfo[]; onOpenCategory: (id: string) => void; onRunningChange: (running: boolean) => void; onOpenJournal?: () => void;
}) {
  const [results, setResults] = useState<GeneralResult[]>([]);
  const [running, setRunning] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [index, setIndex] = useState<GeneralIndex | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [stopped, setStopped] = useState(false);
  const active = useRef<AbortController | null>(null);
  useEffect(() => () => { active.current?.abort(); active.current = null; }, []);
  useEffect(() => { onRunningChange(running || cleaning); return () => onRunningChange(false); }, [running, cleaning, onRunningChange]);
  async function start() {
    if (active.current || cleaning) return;
    const controller = new AbortController(); active.current = controller;
    setStopped(false); setRunning(true); setIndex(null); setError(null);
    setResults(generalRules(rules).map(rule => ({ rule, state: "pending", candidates: 0, bytes: "0", inspected: 0, diagnostics: {} })));
    try {
      const session = await createGeneralCleanup();
      const result = await runGeneralAnalysis(rules, controller.signal, row => {
        if (active.current === controller) setResults(old => old.map(item => item.rule.id === row.rule.id ? row : item));
      }, taskId => collectGeneralCleanup(session, taskId));
      const snapshot = await finishGeneralCleanup(session, result.cancelled, result.failedCategories);
      if (active.current === controller) setIndex(snapshot);
    } catch (e) { if (active.current === controller) setError(String(e)); } finally {
      if (active.current === controller) { setStopped(controller.signal.aborted); active.current = null; setRunning(false); }
    }
  }
  const finished = results.filter(row => row.state !== "pending" && row.state !== "running").length;
  return <div className="space-y-4">
    {!index && <CleanupSteps step={1}/>}
    <p className="text-xs text-muted">Последовательно проверит все {generalRules(rules).length} категорий без выбора папки, включая следующие партии. Кеши проектов проверяются отдельно. Запущенный браузер и недоступные каталоги будут отмечены в отчёте. Файлы не изменяются.</p>
    {running ? <button className="border border-line rounded-lg px-4 py-2 text-sm" onClick={() => active.current?.abort()}>Остановить общий анализ</button> : <button disabled={!isTauriEnvironment() || !rules.length || cleaning} onClick={() => void start()} className="bg-accent text-accent-ink rounded-lg px-4 py-2 text-sm disabled:opacity-40">{results.length ? "Повторить общий анализ" : "Запустить общий анализ"}</button>}
    {error && <p role="alert" className="text-xs text-warn">{error}</p>}
    <GeneralCleanupSelection index={index} rules={rules} onRunningChange={setCleaning} onOpenJournal={onOpenJournal}/>
    {!!results.length && <details open={running} className="border border-line rounded-xl p-4"><summary>Отчёт по категориям · обработано {finished} из {results.length}</summary>
      <p role="status" className="text-sm text-muted">{running ? "Идёт общий анализ" : stopped ? "Общий анализ остановлен" : "Общий анализ завершён"} · категорий обработано: {finished} из {results.length}</p>
      <p className="text-xs text-muted">Подробности анализа по категориям. Единый список для очистки выше исключает повторы и категории только для анализа. «Посмотреть файлы» отдельно повторяет анализ категории.</p>
      <ul className="divide-y divide-line">{results.map(row => <li key={row.rule.id} className="py-3 space-y-2">
        <div className="flex flex-wrap items-center justify-between gap-3"><div><h3 className="text-sm">{row.rule.name}{row.rule.preview_only ? " · только анализ" : ""}</h3><p className="text-xs text-muted mt-1">{stateLabels[row.state]} · найдено: {row.candidates} · {formatBytes(row.bytes)} · проверено объектов: {row.inspected}</p></div>
          {!running && !cleaning && row.candidates > 0 && <button onClick={() => onOpenCategory(row.rule.id)} className="border border-line rounded-lg px-3 py-2 text-xs">Посмотреть файлы: {row.rule.name}</button>}</div>
        {row.error && <p className="text-xs text-warn break-words">{row.error}</p>}
        {!!Object.keys(row.diagnostics).length && <details className="text-xs text-muted"><summary>Исключения и ограничения: {row.rule.name}</summary><ul>{Object.entries(row.diagnostics).map(([code, count]) => <li key={code}>{scanReason(code)}: {count}</li>)}</ul></details>}
      </li>)}</ul>
    </details>}
  </div>;
}
