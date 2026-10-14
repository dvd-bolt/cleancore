import { invoke } from "@tauri-apps/api/core";
import { isTauriEnvironment } from "../../lib/ipc";
import type { AuditPage, CleanupAvailability, CleanupConfirmation, CleanupOutcome, ExecutionPreview } from "./types";

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriEnvironment()) throw new Error("Исполнение доступно только в desktop-приложении.");
  try { return await invoke<T>(command, args); }
  catch (e) { throw new Error(e && typeof e === "object" && "message" in e ? String(e.message) : String(e)); }
}
export async function getAvailability(): Promise<CleanupAvailability> {
  if (!isTauriEnvironment()) return { execution_enabled: false, persistent_journal: false, reason: "Предпросмотр: сканер, исполнитель и постоянный журнал не подключены." };
  return call("get_cleanup_availability");
}
export async function getExecutionPreview(previewId: string | null = null): Promise<ExecutionPreview | null> {
  if (!isTauriEnvironment()) return null;
  return call("get_execution_preview", { previewId });
}
export const prepareConfirmation = (previewId: string, selectedIds: string[]) => call<CleanupConfirmation>("prepare_cleanup_confirmation", { previewId, selectedIds });
export const executeCleanup = (confirmationId: string, acknowledged: boolean) => call<CleanupOutcome>("start_confirmed_cleanup", { confirmationId, acknowledged });
export const getOperation = (operationId: string) => call<CleanupOutcome | null>("get_cleanup_operation", { operationId });
export const cancelCleanup = (operationId: string) => call<boolean>("cancel_cleanup", { operationId });
export async function getJournal(before: number | null = null): Promise<AuditPage> {
  if (!isTauriEnvironment()) return { operations: [], next_cursor: null };
  return call("get_cleanup_journal", { before, limit: 10 });
}
