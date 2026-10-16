import type { CleanupOutcome, ItemStatus, OperationStatus } from "./types";
const statuses: Record<ItemStatus, string> = { Pending: "Ожидает", Recycled: "В Корзине", Deleted: "Удалён безвозвратно", Skipped: "Пропущен", Failed: "Ошибка", Cancelled: "Отменён", Unknown: "Исход неизвестен" };
const operations: Record<OperationStatus, string> = { Running: "Выполняется", Completed: "Завершено", Cancelled: "Отменено", Interrupted: "Прервано", JournalFailed: "Сбой журнала" };
export function formatBytes(value: string): string {
  try { return `${BigInt(value).toLocaleString("ru-RU")} байт`; } catch { return "нет данных"; }
}
const reasons: Record<string,string> = {
  USER_EXCLUDED: "Путь находится в исключениях", PROTECTED_ORIGINAL: "Папка защищена от удаления дубликатов",
  APPLICATION_RUNNING: "Приложение запущено: закройте его и повторите анализ", PROCESS_CHECK_FAILED: "Не удалось проверить запущенные приложения",
  KEEPER_CHANGED: "Сохраняемый оригинал изменился", DUPLICATE_CHANGED: "Содержимое больше не соответствует группе дубликатов",
  FILE_IN_USE: "Файл занят", ACCESS_DENIED: "Доступ запрещён", NOT_FOUND: "Объект уже отсутствует", OBJECT_CHANGED: "Объект изменился после анализа",
  DELETE_REFUSED: "Windows отказала в удалении: возможны занятость, ограничения или новые файлы в папке",
  PREFERENCES_UNAVAILABLE: "Настройки защиты недоступны", TOO_RECENT: "Файл не соответствует возрасту правила", CANCELLED: "Отменено пользователем",
  DELETED_PERMANENTLY: "Удаление выполнено без Корзины", RECYCLED: "Перемещено в Корзину", STOPPED: "Следующие действия остановлены",
};
export function OutcomeView({ outcome }: { outcome: CleanupOutcome }) {
  return <section className="space-y-3" aria-label="Результаты очистки">
    <h3 className="font-heading text-lg text-txt">{operations[outcome.status]}</h3>
    <p className="text-xs text-muted">Удалено безвозвратно: {formatBytes(outcome.deleted_bytes || "0")}. В Корзину: {formatBytes(outcome.recycled_bytes)}. Это объём обработанных файлов, а не освобождённое место.</p>
    {!outcome.journal_ok && <p role="alert" className="text-xs text-warn">Результат не полностью записан. Некоторые исходы неизвестны; повторная очистка автоматически не запускается.</p>}
    {outcome.status === "Interrupted" && <p role="alert" className="text-xs text-warn">Операция прервана. Неизвестные исходы требуют проверки; автоматического повтора нет.</p>}
    <ul className="divide-y divide-line max-h-80 overflow-y-auto">
      {outcome.items.map(item => <li key={item.object_id} className="py-3 text-xs space-y-1">
        <p className="text-txt break-all select-text">{item.display_path}</p>
        <p className="text-muted">{statuses[item.status]} · {formatBytes(item.size_bytes)} · <span className="font-mono">{reasons[item.reason_code] ?? item.reason_code}</span></p>
      </li>)}
    </ul>
  </section>;
}
