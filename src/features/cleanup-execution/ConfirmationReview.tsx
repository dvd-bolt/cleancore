import type { CleanupConfirmation } from "./types";
import { formatBytes } from "./OutcomeView";
export function ConfirmationReview({ confirmation, acknowledged, allowed, busy, expired, onAcknowledge, onExecute, onBack }: {
  confirmation: CleanupConfirmation; acknowledged: boolean; allowed: boolean; busy: boolean; expired: boolean;
  onAcknowledge: (value: boolean) => void; onExecute: () => void; onBack: () => void;
}) {
  return <section className="border border-accent rounded-xl p-5 space-y-4" aria-label="Подтверждение выбранного списка">
    <h3 className="font-heading text-xl text-txt">Проверь точный список</h3>
    <p className="text-xs text-muted">Выбрано: {confirmation.selected.length} · {formatBytes(confirmation.selected_bytes)}</p>
    <ul className="max-h-60 overflow-y-auto space-y-2 text-xs text-txt select-text">
      {confirmation.selected.map(item => <li key={item.object_id} className="break-all">{item.display_path} · {formatBytes(item.size_bytes)}</li>)}
    </ul>
    <p className="text-xs text-muted leading-relaxed">{confirmation.consequences}</p>
    <label className="flex gap-3 items-start text-sm text-txt"><input type="checkbox" checked={acknowledged} disabled={busy || expired} onChange={e => onAcknowledge(e.target.checked)} />Я подтверждаю обработку именно этого списка.</label>
    {expired && <p role="alert" className="text-xs text-warn">Подтверждение устарело. Вернитесь к выбору и подготовьте новое.</p>}
    <div className="flex flex-wrap gap-3">
      <button disabled={busy} onClick={onBack} className="px-4 py-2 rounded-lg border border-line text-sm disabled:opacity-40">Вернуться к выбору</button>
      <button disabled={!allowed || !acknowledged || busy || expired} onClick={onExecute} className="px-4 py-2 rounded-lg bg-accent text-accent-ink text-sm disabled:opacity-40">Подтвердить и отправить в Корзину</button>
    </div>
  </section>;
}
