import type { CleanupOutcome, ItemStatus, OperationStatus } from "./types";
const statuses: Record<ItemStatus, string> = { Pending: "Ожидает", Recycled: "В Корзине", Skipped: "Пропущен", Failed: "Ошибка", Cancelled: "Отменён", Unknown: "Исход неизвестен" };
const operations: Record<OperationStatus, string> = { Running: "Выполняется", Completed: "Завершено", Cancelled: "Отменено", Interrupted: "Прервано", JournalFailed: "Сбой журнала" };
export function formatBytes(value: string): string {
  try { return `${BigInt(value).toLocaleString("ru-RU")} байт`; } catch { return "нет данных"; }
}
export function OutcomeView({ outcome }: { outcome: CleanupOutcome }) {
  return <section className="space-y-3" aria-label="Результаты очистки">
    <h3 className="font-heading text-lg text-txt">{operations[outcome.status]}</h3>
    <p className="text-xs text-muted">В Корзину: {formatBytes(outcome.recycled_bytes)}. Это объём обработанных файлов, а не освобождённое место.</p>
    {!outcome.journal_ok && <p role="alert" className="text-xs text-warn">Результат не полностью записан. Некоторые исходы неизвестны; повторная очистка автоматически не запускается.</p>}
    {outcome.status === "Interrupted" && <p role="alert" className="text-xs text-warn">Операция прервана. Неизвестные исходы требуют проверки; автоматического повтора нет.</p>}
    <ul className="divide-y divide-line max-h-80 overflow-y-auto">
      {outcome.items.map(item => <li key={item.object_id} className="py-3 text-xs space-y-1">
        <p className="text-txt break-all select-text">{item.display_path}</p>
        <p className="text-muted">{statuses[item.status]} · {formatBytes(item.size_bytes)} · <span className="font-mono">{item.reason_code}</span></p>
      </li>)}
    </ul>
  </section>;
}
