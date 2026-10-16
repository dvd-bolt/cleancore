import { useEffect, useRef, useState } from "react";
import type { ActivePage } from "../types";
import { isTauriEnvironment } from "../lib/ipc";
import { getJournal } from "../features/cleanup-execution/ipc";
import type { CleanupOutcome } from "../features/cleanup-execution/types";
import { askAssistant, cancelAssistant, chooseAssistantModel, disableAssistant, enableAssistant, getAssistantStatus, prepareAssistantContext } from "../features/assistant/ipc";
import type { AssistantStatus, ContextSummary, PreparedContext } from "../features/assistant/types";

export function ContextReview({ summary }: { summary: ContextSummary }) {
  return <section aria-label="Сводка для локальной модели" className="bg-shell border border-line rounded-xl p-4 space-y-2 text-sm">
    <h3 className="font-semibold">Модель увидит только эту сводку</h3>
    {summary.metrics ? <p>CPU: {summary.metrics.cpu_percent?.toFixed(1) ?? "нет данных"}% · RAM: {summary.metrics.memory_percent ?? "нет данных"}% · свободно на диске: {summary.metrics.disk_free_bytes === null ? "нет данных" : `${(Number(summary.metrics.disk_free_bytes) / 1024 ** 3).toFixed(1)} ГБ`}</p> : <p>Метрики не включены.</p>}
    {summary.scan ? <p>Анализ {summary.scan.category}: {summary.scan.candidates} кандидатов; {(Number(summary.scan.bytes) / 1024 ** 2).toFixed(1)} МБ; {summary.scan.complete ? "полный снимок" : "неполный снимок — очистка недоступна"}.</p> : <p>Анализ файлов не включён.</p>}
    {summary.journal.map((entry,index) => <p key={index}>Операция {index+1}: {entry.status}, объектов {entry.item_count}, ошибок {entry.failed_count}, неизвестных исходов {entry.unknown_count}.</p>)}
    {!summary.journal.length && <p>Записи журнала не включены.</p>}
    <p className="text-xs text-muted">Без путей, имён процессов, содержимого файлов и командных строк.</p>
  </section>;
}
const EMPTY: AssistantStatus = { enabled:false, runtime_present:false, model:null, task:null };
export function AssistantPage({onNavigate}: {onNavigate: (page: ActivePage) => void}) {
  const [status,setStatus]=useState<AssistantStatus>(EMPTY);
  const [pending,setPending]=useState(false); const guard=useRef(false);
  const [error,setError]=useState<string | null>(null);
  const [licenseSource,setLicenseSource]=useState(""); const [licenseAccepted,setLicenseAccepted]=useState(false);
  const [includeMetrics,setIncludeMetrics]=useState(false); const [includeScan,setIncludeScan]=useState(false);
  const [journal,setJournal]=useState<CleanupOutcome[]>([]); const [journalIds,setJournalIds]=useState<string[]>([]);
  const [prepared,setPrepared]=useState<PreparedContext | null>(null);
  const [question,setQuestion]=useState("Объясни выбранные данные и предложи, что проверить вручную.");
  const running=status.task?.state==="running";
  const desktop=isTauriEnvironment();
  const latestScan=desktop ? sessionStorage.getItem("cleancore-scan-task") : null;
  useEffect(()=>{
    if(!desktop)return; let disposed=false;
    Promise.all([getAssistantStatus(),getJournal(null)]).then(([value,page])=>{if(!disposed){setStatus(value);setJournal(page.operations.slice(0,5));}}).catch(error=>{if(!disposed)setError(String(error));});
    return()=>{disposed=true;};
  },[desktop]);
  useEffect(()=>{
    if(!running)return; let disposed=false;let busy=false;
    const poll=async()=>{if(busy)return;busy=true;try{const value=await getAssistantStatus();if(!disposed)setStatus(value);}catch(error){if(!disposed)setError(String(error));}finally{busy=false;}};
    const timer=window.setInterval(poll,500);return()=>{disposed=true;window.clearInterval(timer);};
  },[running]);
  async function action(task:()=>Promise<void>){if(guard.current)return;guard.current=true;setPending(true);setError(null);try{await task();}catch(error){setError(error instanceof Error ? error.message : String(error));}finally{guard.current=false;setPending(false);}}
  const busy=pending||running;
  return <div className="space-y-6">
    <header className="border-b border-line pb-4"><p className="text-xs text-muted font-mono">ЛОКАЛЬНЫЙ ПОМОЩНИК</p><h2 className="font-heading text-2xl font-bold mt-1">Локальный помощник</h2><p className="text-sm text-muted mt-2">Объяснения на вашем компьютере. Модель даёт текстовые советы, решение остаётся за вами.</p></header>
    {!desktop && <p role="status" className="text-sm text-muted">Предпросмотр: локальная модель и системные данные недоступны.</p>}
    {error && <p role="alert" className="text-sm text-red-300">{error}</p>}
    <section aria-label="Локальная модель" className="bg-surface border border-line rounded-xl p-5 space-y-4">
      <div className="flex flex-wrap justify-between gap-3"><div><h3 className="font-semibold">Модель GGUF</h3><p className="text-xs text-muted mt-1">{status.runtime_present ? "Среда локальной модели установлена." : "Среда локальной модели недоступна. Основные функции CleanCore доступны."} Модель не загружается из интернета.</p></div>
        <button disabled={busy||!desktop} onClick={()=>action(async()=>{const model=await chooseAssistantModel();if(model){setLicenseSource("");setLicenseAccepted(false);setPrepared(null);}setStatus(await getAssistantStatus());})} className="px-4 py-2 border border-line rounded-lg text-sm disabled:opacity-40">Выбрать файл GGUF</button></div>
      {status.model ? <><p className="text-sm select-text">{status.model.name} · {status.model.architecture} · {(Number(status.model.size_bytes)/1024**3).toFixed(2)} ГБ</p><p className="text-xs text-muted">Лицензия в метаданных: {status.model.declared_license || "не указана"}. Проверьте условия на странице автора модели.</p>
        {!status.enabled && <><label className="block text-sm">Источник условий использования модели<input disabled={busy} value={licenseSource} maxLength={512} onChange={e=>setLicenseSource(e.target.value)} className="block w-full mt-2 bg-shell border border-line rounded-lg px-3 py-2" placeholder="Название лицензии и адрес страницы автора или локального файла"/></label><label className="flex items-start gap-2 text-sm"><input type="checkbox" disabled={busy} checked={licenseAccepted} onChange={e=>setLicenseAccepted(e.target.checked)}/>Я ознакомился с лицензией весов и имею право использовать эту модель.</label><button disabled={busy||!licenseAccepted||licenseSource.trim().length<4||!status.runtime_present} onClick={()=>action(async()=>{await enableAssistant(status.model!.id,licenseSource,true);setStatus(await getAssistantStatus());})} className="bg-accent text-accent-ink rounded-lg px-4 py-2 text-sm disabled:opacity-40">Включить помощника</button></>}
      </> : <p className="text-sm text-muted">Модель не выбрана. Выберите совместимый GGUF до 2 ГБ с известными вам условиями использования.</p>}
      <button disabled={pending||!desktop||(!status.model&&!running)} onClick={()=>action(async()=>{await disableAssistant();setStatus(await getAssistantStatus());setPrepared(null);setLicenseAccepted(false);})} className="border border-line rounded-lg px-4 py-2 text-sm disabled:opacity-40">Отключить и освободить модель</button>
      <p className="text-xs text-muted">Один запрос, 2 CPU-потока, до 90 секунд. Модель и ответы не сохраняются между запусками; память процесса освобождается после ответа или отмены.</p>
    </section>
    {status.enabled ? <>
    <section aria-label="Выбор данных" className="bg-surface border border-line rounded-xl p-5 space-y-4">
      <h3 className="font-semibold">Какие данные объяснить</h3>
      <label className="flex gap-2 text-sm"><input type="checkbox" disabled={busy} checked={includeMetrics} onChange={e=>{setIncludeMetrics(e.target.checked);setPrepared(null);}}/>CPU, RAM и свободное место</label>
      <label className="flex gap-2 text-sm"><input type="checkbox" disabled={busy||!latestScan} checked={includeScan} onChange={e=>{setIncludeScan(e.target.checked);setPrepared(null);}}/>Итоги последнего анализа Temp{!latestScan ? " — анализ ещё не запускался" : ""}</label>
      {journal.map(operation=><label className="flex gap-2 text-sm" key={operation.operation_id}><input type="checkbox" disabled={busy} checked={journalIds.includes(operation.operation_id)} onChange={e=>{setJournalIds(ids=>e.target.checked?[...ids,operation.operation_id]:ids.filter(id=>id!==operation.operation_id));setPrepared(null);}}/>Операция {new Date(operation.started_at_ms).toLocaleString("ru-RU")} · {operation.items.length} объектов</label>)}
      <button disabled={busy||!desktop} onClick={()=>action(async()=>{setPrepared(await prepareAssistantContext({metrics:includeMetrics,preview_id:includeScan?latestScan:null,journal_ids:journalIds}));setStatus(await getAssistantStatus());})} className="border border-line rounded-lg px-4 py-2 text-sm disabled:opacity-40">Подготовить сводку</button>
      {prepared && <ContextReview summary={prepared.summary}/>}
    </section>
    <section className="bg-surface border border-line rounded-xl p-5 space-y-4" aria-label="Вопрос помощнику">
      <label className="block text-sm">Вопрос<textarea rows={3} maxLength={512} value={question} disabled={busy} onChange={e=>setQuestion(e.target.value)} className="block w-full mt-2 bg-shell border border-line rounded-lg px-3 py-2"/></label>
      <div className="flex flex-wrap gap-3"><button disabled={busy||!status.enabled||!prepared||!question.trim()} onClick={()=>action(async()=>{await askAssistant(prepared!.id,question);setStatus(await getAssistantStatus());})} className="bg-accent text-accent-ink rounded-lg px-4 py-2 text-sm disabled:opacity-40">Объяснить локально</button><button disabled={pending||!running} onClick={()=>action(async()=>{await cancelAssistant();setStatus(await getAssistantStatus());})} className="border border-line rounded-lg px-4 py-2 text-sm disabled:opacity-40">Остановить</button></div>
      {running && <p role="status" className="text-sm text-muted">Модель обрабатывает сводку…</p>}
      {status.task?.error && <p role="alert" className="text-sm text-red-300">{status.task.error}</p>}
      {status.task?.state==="cancelled" && <p role="status" className="text-sm text-muted">Запрос остановлен.</p>}
      {status.task?.answer && <div className="bg-shell border border-line rounded-xl p-4"><h3 className="font-semibold mb-2">Ответ локальной модели</h3><p className="text-sm whitespace-pre-wrap break-words select-text">{status.task.answer}</p><p className="text-xs text-muted mt-4">Модель может ошибаться. Советы не выполняются автоматически.</p></div>}
      {(status.task?.recommendations ?? prepared?.recommendations ?? []).map(item=><button key={item.page} onClick={()=>onNavigate(item.page)} className="border border-line rounded-lg px-4 py-2 mr-2 text-sm">{item.label}</button>)}
    </section>
    </> : <p className="text-sm text-muted border border-line rounded-xl p-4">Сначала выберите модель и включите помощника. После подключения появятся выбор данных и поле вопроса.</p>}
  </div>;
}
