import { invoke } from "@tauri-apps/api/core";
import { isTauriEnvironment } from "../../lib/ipc";
export interface GeneralItem { object_id: string; category_id: string; size_bytes: string }
export interface GeneralIndex { session_id: string; items: GeneralItem[]; total_bytes: string; partial: boolean; truncated: boolean; consumed: boolean; expires_at_ms: number }
export interface GeneralFile extends GeneralItem { display_path: string }
export interface GeneralPage { items: GeneralFile[]; total: number }
export interface GeneralConfirmation { confirmation_id: string; selected_count: number; selected_bytes: string; expires_at_ms: number }
export interface GeneralJob { job_id: string; status: "Running" | "Completed" | "Cancelled" | "Interrupted"; selected_count: number; processed: number; deleted: number; skipped: number; failed: number; deleted_bytes: string; operation_ids: string[]; message: string | null }
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriEnvironment()) throw new Error("Очистка доступна только в приложении CleanCore");
  try { return await invoke<T>(command, args); }
  catch (e) { throw new Error(e && typeof e === "object" && "message" in e ? String(e.message) : String(e)); }
}
export const createGeneralCleanup = () => call<string>("create_general_cleanup");
export const collectGeneralCleanup = (sessionId: string, taskId: string) => call<void>("collect_general_cleanup", { sessionId, taskId });
export const finishGeneralCleanup = (sessionId: string, cancelled: boolean, failedCategories: string[]) => call<GeneralIndex>("finish_general_cleanup", { sessionId, cancelled, failedCategories });
export const getGeneralCleanupPage = (sessionId: string, offset: number, query: string, selectedIds: string[] | null) => call<GeneralPage>("get_general_cleanup_page", { sessionId, offset, limit: 50, query, selectedIds });
export const prepareGeneralCleanup = (sessionId: string, selectedIds: string[], acceptPartial: boolean) => call<GeneralConfirmation>("prepare_general_cleanup", { sessionId, selectedIds, acceptPartial });
export const executeGeneralCleanup = (confirmationId: string, acknowledged: boolean) => call<GeneralJob>("execute_general_cleanup", { confirmationId, acknowledged });
export const getGeneralCleanupJob = (jobId: string) => call<GeneralJob>("get_general_cleanup_job", { jobId });
export const cancelGeneralCleanupJob = (jobId: string) => call<boolean>("cancel_general_cleanup_job", { jobId });

export function formatCleanupBytes(value: string): string {
  try {
    const n = BigInt(value); const units = ["Б", "КБ", "МБ", "ГБ", "ТБ", "ПБ", "ЭБ"];
    let divisor = 1n, unit = 0;
    while (unit < units.length - 1 && n >= divisor * 1024n) { divisor *= 1024n; unit++; }
    return `${(Number((n * 100n + divisor / 2n) / divisor) / 100).toLocaleString("ru-RU", { maximumFractionDigits: 2 })} ${units[unit]}`;
  } catch { return "нет данных"; }
}
