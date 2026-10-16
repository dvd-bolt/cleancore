import { useState } from "react";
import type { DuplicateGroup } from "../../types";
import { prepareDuplicateCleanup, invalidateDiskReport } from "../../lib/ipc";
import { AccessibleDialog } from "../../components/AccessibleDialog";
import { CleanupExecutionPanel } from "../cleanup-execution/CleanupExecutionPanel";
import type { ExecutionPreview } from "../cleanup-execution/types";

export function DuplicateActions({ group, reportId }: { group: DuplicateGroup; reportId: string | null }) {
  const [keeper, setKeeper] = useState<number | null>(null);
  const [selected, setSelected] = useState<number[]>([]);
  const [preview, setPreview] = useState<ExecutionPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);
  const [stale, setStale] = useState(false);
  async function prepare() {
    if (!reportId || keeper == null || !selected.length || busy) return;
    setBusy(true); setError(null);
    try { setPreview(await prepareDuplicateCleanup(reportId, group.group_id, keeper, selected)); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }
  return <div className="border-t border-line pt-3 space-y-3 text-xs">
    <label className="block">Сохраняемый оригинал<select aria-label="Сохраняемый оригинал" disabled={busy || stale} value={keeper ?? ""} onChange={e => { setKeeper(e.target.value === "" ? null : Number(e.target.value)); setSelected([]); }} className="block w-full bg-shell border border-line rounded px-3 py-2 mt-2"><option value="">Выберите явно — автоматического выбора нет</option>{group.files.map((file, i) => <option key={file.path} value={i}>{file.path}</option>)}</select></label>
    {keeper != null && <fieldset className="space-y-2"><legend className="text-muted mb-2">Выберите ненужные копии. Оригинал будет проверен и сохранён.</legend>{group.files.map((file, i) => i === keeper ? <p key={file.path} className="text-accent break-all">Оставить: {file.path}</p> : <label key={file.path} className="flex gap-2 break-all"><input type="checkbox" disabled={busy || stale} checked={selected.includes(i)} onChange={e => setSelected(old => e.target.checked ? [...old, i] : old.filter(n => n !== i))}/>{file.path}</label>)}</fieldset>}
    <p className="text-muted">Удаление безвозвратное. Полный SHA-256 обеих копий проверяется перед каждым удалением; оригинал удерживается от изменения. Одинаковое содержимое не означает, что копия не нужна.</p>
    {error && <p role="alert" className="text-warn">{error}</p>}
    {stale ? <p role="status" className="text-warn">Группа устарела после операции. Повторите поиск дубликатов.</p> : <button disabled={!reportId || keeper == null || !selected.length || busy} onClick={() => void prepare()} className="border border-line rounded px-3 py-2 disabled:opacity-40">Проверить выбранные копии ({selected.length})</button>}
    {preview && <AccessibleDialog title="Подтверждение удаления копий" busy={running} onClose={() => { if (!running) setPreview(null); }}><div className="w-full max-w-4xl max-h-[90vh] overflow-y-auto rounded-xl bg-surface p-5"><button disabled={running} onClick={() => setPreview(null)} className="mb-4 border border-line rounded px-3 py-2 text-xs disabled:opacity-40">Закрыть предпросмотр</button><p className="text-xs mb-3 break-all">Сохранить: {keeper == null ? "" : group.files[keeper].path}</p><CleanupExecutionPanel previewId={preview.preview_id} selectedIds={preview.candidates.map(c => c.object_id)} onRunningChange={setRunning} onCompleted={() => { setStale(true); if (reportId) void invalidateDiskReport(reportId).catch(e => setError(String(e))); }}/></div></AccessibleDialog>}
  </div>;
}
