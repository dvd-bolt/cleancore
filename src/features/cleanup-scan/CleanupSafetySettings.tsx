import { useEffect, useState } from "react";
import { getCleanupPreferences, saveCleanupPreferences, clearDuplicateHashCache, isTauriEnvironment } from "../../lib/ipc";
const lines = (text: string) => [...new Set(text.split(/\r?\n/).map(line => line.trim()).filter(Boolean))];
export function CleanupSafetySettings() {
  const [excluded, setExcluded] = useState("");
  const [protectedRoots, setProtectedRoots] = useState("");
  const [busy, setBusy] = useState(true);
  const [notice, setNotice] = useState<string | null>(null);
  useEffect(() => { let live = true; if (!isTauriEnvironment()) { setBusy(false); return; } getCleanupPreferences().then(p => { if (live) { setExcluded(p.excluded_paths.join("\n")); setProtectedRoots(p.protected_duplicate_roots.join("\n")); } }).catch(e => { if (live) setNotice(String(e)); }).finally(() => { if (live) setBusy(false); }); return () => { live = false; }; }, []);
  async function save() {
    if (busy) return; setBusy(true); setNotice(null);
    try { await saveCleanupPreferences({ excluded_paths: lines(excluded), protected_duplicate_roots: lines(protectedRoots) }); setNotice("Сохранено. Старые подтверждения отозваны; выполните новый анализ. Исключения также проверяются перед действием."); }
    catch (e) { setNotice(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }
  async function clear() { if (busy) return; setBusy(true); try { await clearDuplicateHashCache(); setNotice("Кеш хешей очищен. Следующий поиск заново прочитает файлы."); } catch(e) { setNotice(String(e)); } finally { setBusy(false); } }
  return <section aria-label="Защита файлов при очистке" className="bg-surface border border-line rounded-xl p-5 space-y-4">
    <h3 className="text-sm font-semibold">Исключения и защищённые оригиналы</h3>
    <p className="text-xs text-muted">Полный локальный путь на каждой строке, до 100 строк каждого вида. Папка защищает всё содержимое. Маски не поддерживаются. Настройки сохраняются локально.</p>
    <label className="block text-xs">Не удалять при очистке и ручных действиях<textarea aria-label="Исключения очистки" value={excluded} disabled={busy} onChange={e => setExcluded(e.target.value)} rows={4} maxLength={409600} placeholder={"C:\\Users\\имя\\Documents\\Важное"} className="block mt-2 w-full bg-shell border border-line rounded-lg p-3 font-mono"/></label>
    <label className="block text-xs">Никогда не выбирать для удаления дубликатов<textarea aria-label="Защищённые оригиналы" value={protectedRoots} disabled={busy} onChange={e => setProtectedRoots(e.target.value)} rows={4} maxLength={409600} placeholder={"D:\\Оригиналы"} className="block mt-2 w-full bg-shell border border-line rounded-lg p-3 font-mono"/></label>
    <div className="flex gap-3 flex-wrap"><button disabled={busy || !isTauriEnvironment()} onClick={() => void save()} className="border border-line rounded-lg px-3 py-2 text-xs disabled:opacity-40">Сохранить защиту</button><button disabled={busy || !isTauriEnvironment()} onClick={() => void clear()} className="border border-line rounded-lg px-3 py-2 text-xs disabled:opacity-40">Очистить кеш хешей</button></div>
    {notice && <p role="status" className="text-xs text-muted break-words">{notice}</p>}
  </section>;
}
