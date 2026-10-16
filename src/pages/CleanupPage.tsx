import type { ActivePage } from "../types";
import { useEffect, useRef, useState } from "react";
import { scanReason } from "../features/cleanup-scan/diagnostics";
import { getCleanupRules, getCleanupProfileContext, launchServiceTool, type CleanupRuleInfo, isTauriEnvironment } from "../lib/ipc";
import { useCleanupScan, PAGE_SIZE } from "../features/cleanup-scan/useCleanupScan";
import { CleanupExecutionPanel } from "../features/cleanup-execution/CleanupExecutionPanel";
import { getExecutionPreview } from "../features/cleanup-execution/ipc";
import { formatBytes } from "../features/cleanup-execution/OutcomeView";

import { GeneralCleanupAnalysis } from "../features/cleanup-scan/GeneralCleanupAnalysis";

export const CleanupPage = ({ onNavigate }: { onNavigate?: (page: ActivePage) => void }) => {
  const scan=useCleanupScan();
  const [rules,setRules]=useState<CleanupRuleInfo[]>([]);
  const [profileContext,setProfileContext]=useState<string | null>(null);
  const [systemNotice,setSystemNotice]=useState<string | null>(null);
  const [rulesError,setRulesError]=useState<string | null>(null);
  useEffect(()=>{let live=true;getCleanupRules().then(r=>{if(live)setRules(r);}).catch(e=>{if(live)setRulesError(String(e));});return()=>{live=false;};},[]);
  useEffect(()=>{let live=true;if(isTauriEnvironment())getCleanupProfileContext().then(path=>{if(live)setProfileContext(path);}).catch(()=>{if(live)setProfileContext(null);});return()=>{live=false;};},[]);
  const [projectPath,setProjectPath]=useState("");
  const [category,setCategory]=useState(scan.preview?.category_id ?? "general");
  const [acceptPartial,setAcceptPartial]=useState(false);
  const [selected,setSelected]=useState<string[]>([]);
  const [batchConsumed,setBatchConsumed]=useState(false);
  const [selectingBatch,setSelectingBatch]=useState(false);
  const [selectionError,setSelectionError]=useState<string | null>(null);
  const selectionGeneration=useRef(0);
  const [executing,setExecuting]=useState(false);
  const [generalRunning,setGeneralRunning]=useState(false);
  const isGeneral=category==="general";
  useEffect(()=>{selectionGeneration.current++;setSelected([]);setAcceptPartial(false);setBatchConsumed(false);setSelectionError(null);},[scan.taskId,scan.preview?.execution_preview_id,scan.scanning]);
  const preview=scan.preview;
  const pages=preview ? Math.ceil(preview.total_candidates/PAGE_SIZE) : 0;
  const canSelect=category===preview?.category_id && (!!preview?.complete || acceptPartial) && !!preview?.execution_preview_id && !executing && !batchConsumed && !selectingBatch;
  async function selectBatch(){
    if(!canSelect || !preview?.execution_preview_id)return;
    const generation=selectionGeneration.current;setSelectingBatch(true);setSelectionError(null);
    try{const result=await getExecutionPreview(preview.execution_preview_id);if(generation===selectionGeneration.current){if(result)setSelected(result.candidates.map(c=>c.object_id));else setSelectionError("Партия устарела или уже обработана. Продолжите или повторите анализ.");}}
    catch(e){if(generation===selectionGeneration.current)setSelectionError(String(e));}
    finally{setSelectingBatch(false);}
  }
  return <div className="space-y-6">
    <header className="pb-3 border-b border-line/40">
      <p className="text-[10px] font-mono text-dim tracking-widest">АНАЛИЗ И КОНТРОЛИРУЕМАЯ ОЧИСТКА</p>
      <h2 className="font-heading text-2xl font-bold mt-1">Очистка файлов</h2>
      <p className="text-xs text-muted mt-2">Один снимок от анализа до результата. Сканирование не изменяет файлы.</p>
    </header>
    <p className="text-xs text-muted break-all">Профиль запущенной учётной записи: {profileContext ?? "нет данных"}. Если в UAC введены данные другого администратора, персональные категории относятся к его профилю.</p>
    {!isGeneral && <CleanupExecutionPanel previewId={category===preview?.category_id ? preview?.execution_preview_id ?? null : null} selectedIds={selected} onOpenJournal={onNavigate ? () => onNavigate("journal") : undefined} onRunningChange={setExecuting} onCompleted={()=>{setSelected([]);setBatchConsumed(true);}}/>}
    <section className="bg-surface border border-line rounded-xl p-5 space-y-4" aria-label="Анализ временных файлов">
      <div className="flex flex-wrap justify-between items-start gap-4">
        <div><label className="block text-sm">Категория анализа<select aria-label="Категория анализа" value={category} disabled={scan.scanning||executing||selectingBatch||generalRunning} onChange={e=>{setCategory(e.target.value);setSelected([]);}} className="block w-full bg-shell border border-line rounded-lg px-3 py-2 mt-2"><option value="general">Общий анализ · все доступные категории</option>{rules.map(rule=><option key={rule.id} value={rule.id}>{rule.name} · от {rule.min_age_days} суток{rule.preview_only ? " · только анализ" : ""}</option>)}</select></label><p className="text-xs text-muted mt-2">{rules.find(rule=>rule.id===category)?.description ?? "Каталоги определяет Windows для учётной записи, под которой запущен CleanCore."}</p></div>
        {!isGeneral && (scan.scanning ? <button onClick={scan.cancel} className="border border-line rounded-lg px-4 py-2 text-sm">Остановить анализ</button> : <button disabled={!isTauriEnvironment() || executing || selectingBatch || !rules.length} onClick={()=>scan.start(category,category.startsWith("project-") ? projectPath : undefined)} className="bg-accent text-accent-ink rounded-lg px-4 py-2 text-sm disabled:opacity-40">{preview ? "Повторить анализ" : "Запустить анализ"}</button>)}
      </div>
      {isGeneral && <GeneralCleanupAnalysis rules={rules} onOpenJournal={onNavigate ? () => onNavigate("journal") : undefined} onRunningChange={setGeneralRunning} onOpenCategory={id=>{setCategory(id);void scan.start(id);}}/>}
      {category.startsWith("project-") && <label className="block text-xs text-muted">Полный путь к выбранному проекту<input aria-label="Путь к проекту" disabled={scan.scanning || executing} value={projectPath} onChange={e=>setProjectPath(e.target.value)} maxLength={4096} placeholder={"C:\\Users\\имя\\Desktop\\Projects\\проект"} className="block w-full mt-2 bg-shell border border-line rounded-lg px-3 py-2 text-txt"/></label>}
      {!isTauriEnvironment() && <p role="status" className="text-xs text-muted">В браузере сканирование недоступно. Демонстрационные файлы не подставляются.</p>}
      {scan.scanning && <p role="status" className="text-sm text-muted">Идёт анализ · проверено объектов: {scan.status?.processed_items ?? 0}. Общий объём работы ещё неизвестен.</p>}
      {rulesError && <p role="alert" className="text-sm text-warn">Не удалось загрузить правила: {rulesError}</p>}
      {scan.error && <p role="alert" className="text-sm text-warn break-words">{scan.error}</p>}
      {!isGeneral && preview && <>
        {category!==preview.category_id && <p role="status" className="text-sm text-muted">Показан предыдущий снимок ({preview.category_id}). Для выбранной категории запустите новый анализ.</p>}
        <div className="flex flex-wrap gap-6 text-sm"><span>Кандидатов в партии: <strong>{preview.unfiltered_candidates}</strong></span><span>Объём партии: <strong>{formatBytes(preview.total_bytes)}</strong></span><span>Выбрано: <strong>{selected.length}</strong></span></div>
        {!preview.complete && <p role="alert" className="text-sm text-warn">{preview.cancelled ? "Анализ отменён." : preview.batch_ready ? "Подготовлена партия файлов. Остальную часть каталога можно проверить следующим проходом." : preview.resumable ? "Анализ приостановлен на границе порции. Можно продолжить с сохранённого места." : "Анализ неполный: часть каталогов недоступна."} {preview.execution_preview_id ? "Можно отдельно разрешить выбор уже проверенных файлов." : "Исполнение по этому снимку отключено."}</p>}
        {preview.resumable && <div className="space-y-2"><p className="text-xs text-muted">Продолжение сохранит позицию обхода. Текущая партия и её подтверждение будут закрыты; невыбранные файлы этой партии останутся на диске. Повторный полный анализ снова покажет их.</p><button disabled={executing || scan.loading || selectingBatch} onClick={()=>void scan.resume()} className="border border-line rounded-lg px-4 py-2 text-sm disabled:opacity-40">{preview.batch_ready ? "Перейти к следующей партии" : "Продолжить анализ"}</button></div>}
        {!!Object.keys(preview.diagnostics).length && <details className="text-xs text-muted"><summary className="cursor-pointer">Исключения и ограничения анализа</summary><ul className="mt-2 space-y-1">{Object.entries(preview.diagnostics).map(([code,count])=><li key={code}>{scanReason(code)}: {count}</li>)}</ul></details>}
        {!preview.complete && preview.execution_preview_id && <label className="flex gap-3 text-sm text-amber-200"><input type="checkbox" checked={acceptPartial} disabled={executing} onChange={e=>{setAcceptPartial(e.target.checked);setSelected([]);}}/>Понимаю, что список неполный. Разрешаю выбор только проверенных файлов из этого списка.</label>}
        <label className="block text-xs text-muted">Поиск по всему снимку<input type="search" maxLength={128} value={scan.query} onChange={e=>scan.search(e.target.value)} className="block w-full max-w-md mt-2 bg-shell border border-line rounded-lg px-3 py-2 text-txt" placeholder="Путь или причина включения"/></label>
        <div className="flex flex-wrap justify-between gap-3 text-xs text-muted"><span>По поиску: {preview.total_candidates} · страница {pages ? scan.page+1 : 0} из {pages}</span><div className="flex gap-3"><button disabled={scan.loading || scan.page===0} onClick={()=>scan.setPage(p=>p-1)} className="border border-line rounded px-3 py-1 disabled:opacity-40">Назад</button><button disabled={scan.loading || scan.page+1>=pages} onClick={()=>scan.setPage(p=>p+1)} className="border border-line rounded px-3 py-1 disabled:opacity-40">Далее</button></div></div>
        {scan.loading ? <p role="status" className="text-sm text-muted">Читаем страницу…</p> : <ul className="divide-y divide-line">
          {preview.items.map(item=><li key={item.obj_id} className="py-3"><label className="flex items-start gap-3 text-sm"><input type="checkbox" aria-label={`Выбрать ${item.display_path}`} disabled={!canSelect} checked={selected.includes(item.obj_id)} onChange={e=>setSelected(old=>e.target.checked ? [...old,item.obj_id] : old.filter(id=>id!==item.obj_id))}/><span className="min-w-0"><span className="block break-all select-text text-txt">{item.display_path}</span><span className="block text-xs text-muted mt-1">{formatBytes(item.size_bytes)} · {item.reason}</span></span></label></li>)}
          {!preview.items.length && <li className="py-5 text-sm text-muted">{scan.query ? "По этому запросу ничего не найдено." : "Подходящих файлов не найдено."}</li>}
        </ul>}
        {batchConsumed && <p role="status" className="text-xs text-muted">Партия обработана. Перейдите к следующей партии или повторите анализ для нового выбора.</p>}
        {selectionError && <p role="alert" className="text-xs text-warn">{selectionError}</p>}
        <button disabled={!canSelect} onClick={()=>void selectBatch()} className="mr-3 text-xs border border-line rounded-lg px-3 py-2 disabled:opacity-40">Выбрать всю партию ({preview.unfiltered_candidates})</button>
        <button disabled={!selected.length || executing} onClick={()=>setSelected([])} className="text-xs border border-line rounded-lg px-3 py-2 disabled:opacity-40">Снять весь выбор</button>
      </>}
    </section>

    <section className="bg-surface border border-line rounded-xl p-5 space-y-3"><h3 className="text-sm font-semibold">Системное обслуживание</h3><p className="text-xs text-muted">Обслуживание компонентов Windows и поддерживаемые Windows системные категории открываются в штатной утилите. Состав и подтверждение выбираются в её окне; CleanCore не удаляет WinSxS и Installer обычным обходом.</p><button disabled={!isTauriEnvironment() || executing || scan.scanning || generalRunning} className="border border-line rounded-lg px-3 py-2 text-xs disabled:opacity-40" onClick={()=>{setSystemNotice(null);void launchServiceTool("cleanmgr").then(result=>setSystemNotice(result.message)).catch(e=>setSystemNotice(String(e)));}}>Открыть очистку диска Windows</button>{systemNotice && <p role="status" className="text-xs text-muted">{systemNotice}</p>}</section>
    <p className="text-xs text-muted">Очистка выбранных файлов выполняется безвозвратно, без Корзины. Выберите только ненужные файлы и внимательно проверьте список перед подтверждением.</p>
  </div>;
};
