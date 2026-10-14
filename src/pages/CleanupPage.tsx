import { useEffect, useState } from "react";
import { scanReason } from "../features/cleanup-scan/diagnostics";
import { isTauriEnvironment } from "../lib/ipc";
import { useCleanupScan, PAGE_SIZE } from "../features/cleanup-scan/useCleanupScan";
import { CleanupExecutionPanel } from "../features/cleanup-execution/CleanupExecutionPanel";
import { formatBytes } from "../features/cleanup-execution/OutcomeView";

export const CleanupPage = () => {
  const scan=useCleanupScan();
  const [selected,setSelected]=useState<string[]>([]);
  const [executing,setExecuting]=useState(false);
  useEffect(()=>{setSelected([]);},[scan.taskId]);
  const preview=scan.preview;
  const pages=preview ? Math.ceil(preview.total_candidates/PAGE_SIZE) : 0;
  const canSelect=!!preview?.complete && !!preview.execution_preview_id && !executing;
  return <div className="space-y-6">
    <header className="pb-3 border-b border-line/40">
      <p className="text-[10px] font-mono text-dim tracking-widest">АНАЛИЗ И КОНТРОЛИРУЕМАЯ ОЧИСТКА</p>
      <h2 className="font-heading text-2xl font-bold mt-1">Посмотреть. Выбрать. Подтвердить.</h2>
      <p className="text-xs text-muted mt-2">Один снимок от анализа до результата. Сканирование не изменяет файлы.</p>
    </header>
    <section className="bg-surface border border-line rounded-xl p-5 space-y-4" aria-label="Анализ временных файлов">
      <div className="flex flex-wrap justify-between items-start gap-4">
        <div><h3 className="font-heading text-lg">Временные файлы пользователя</h3><p className="text-xs text-muted mt-1">user-temp-v1 · не моложе 7 суток · корень определяет Windows</p></div>
        {scan.scanning ? <button onClick={scan.cancel} className="border border-line rounded-lg px-4 py-2 text-sm">Остановить анализ</button> : <button disabled={!isTauriEnvironment() || executing} onClick={scan.start} className="bg-accent text-accent-ink rounded-lg px-4 py-2 text-sm disabled:opacity-40">{preview ? "Повторить анализ" : "Запустить анализ"}</button>}
      </div>
      {!isTauriEnvironment() && <p role="status" className="text-xs text-muted">В браузере сканирование недоступно. Демонстрационные файлы не подставляются.</p>}
      {scan.scanning && <p role="status" className="text-sm text-muted">Идёт анализ · проверено объектов: {scan.status?.processed_items ?? 0}. Общий объём работы ещё неизвестен.</p>}
      {scan.error && <p role="alert" className="text-sm text-warn break-words">{scan.error}</p>}
      {preview && <>
        <div className="flex flex-wrap gap-6 text-sm"><span>Кандидатов в снимке: <strong>{preview.unfiltered_candidates}</strong></span><span>Объём снимка: <strong>{formatBytes(preview.total_bytes)}</strong></span><span>Выбрано: <strong>{selected.length}</strong></span></div>
        {!preview.complete && <p role="alert" className="text-sm text-warn">{preview.cancelled ? "Анализ отменён." : "Анализ неполный: достигнут лимит или часть каталогов недоступна."} Исполнение по этому снимку отключено.</p>}
        {!!Object.keys(preview.diagnostics).length && <details className="text-xs text-muted"><summary className="cursor-pointer">Исключения и ограничения анализа</summary><ul className="mt-2 space-y-1">{Object.entries(preview.diagnostics).map(([code,count])=><li key={code}>{scanReason(code)}: {count}</li>)}</ul></details>}
        <label className="block text-xs text-muted">Поиск по всему снимку<input type="search" maxLength={128} value={scan.query} onChange={e=>scan.search(e.target.value)} className="block w-full max-w-md mt-2 bg-shell border border-line rounded-lg px-3 py-2 text-txt" placeholder="Путь или причина включения"/></label>
        <div className="flex flex-wrap justify-between gap-3 text-xs text-muted"><span>По поиску: {preview.total_candidates} · страница {pages ? scan.page+1 : 0} из {pages}</span><div className="flex gap-3"><button disabled={scan.loading || scan.page===0} onClick={()=>scan.setPage(p=>p-1)} className="border border-line rounded px-3 py-1 disabled:opacity-40">Назад</button><button disabled={scan.loading || scan.page+1>=pages} onClick={()=>scan.setPage(p=>p+1)} className="border border-line rounded px-3 py-1 disabled:opacity-40">Далее</button></div></div>
        {scan.loading ? <p role="status" className="text-sm text-muted">Читаем страницу…</p> : <ul className="divide-y divide-line">
          {preview.items.map(item=><li key={item.obj_id} className="py-3"><label className="flex items-start gap-3 text-sm"><input type="checkbox" aria-label={`Выбрать ${item.display_path}`} disabled={!canSelect} checked={selected.includes(item.obj_id)} onChange={e=>setSelected(old=>e.target.checked ? [...old,item.obj_id] : old.filter(id=>id!==item.obj_id))}/><span className="min-w-0"><span className="block break-all select-text text-txt">{item.display_path}</span><span className="block text-xs text-muted mt-1">{formatBytes(item.size_bytes)} · {item.reason}</span></span></label></li>)}
          {!preview.items.length && <li className="py-5 text-sm text-muted">{scan.query ? "По этому запросу ничего не найдено." : "Подходящих файлов не найдено."}</li>}
        </ul>}
        <button disabled={!selected.length || executing} onClick={()=>setSelected([])} className="text-xs border border-line rounded-lg px-3 py-2 disabled:opacity-40">Снять весь выбор</button>
      </>}
    </section>
    <CleanupExecutionPanel previewId={preview?.execution_preview_id ?? null} selectedIds={selected} onRunningChange={setExecuting}/>
    <p className="text-xs text-muted">Корзина пока заблокирована до допуска Windows-адаптера. Повторная проверка сама по себе не разрешает удаление по пути.</p>
  </div>;
};
