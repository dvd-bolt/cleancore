import { invoke } from "@tauri-apps/api/core";
import { isTauriEnvironment } from "../../lib/ipc";
import type { AssistantStatus, ContextSelection, ModelInfo, PreparedContext } from "./types";
async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriEnvironment()) throw new Error("Локальный помощник доступен только в desktop-приложении.");
  try { return await invoke<T>(name,args); }
  catch (error) { throw new Error(error && typeof error === "object" && "message" in error ? String(error.message) : String(error)); }
}
export const getAssistantStatus = () => command<AssistantStatus>("assistant_status");
export const chooseAssistantModel = () => command<ModelInfo | null>("assistant_choose_model");
export const enableAssistant = (modelId: string, licenseSource: string, accepted: boolean) => command<void>("assistant_enable", { modelId, licenseSource, accepted });
export const disableAssistant = () => command<void>("assistant_disable");
export const cancelAssistant = () => command<void>("assistant_cancel");
export const prepareAssistantContext = (selection: ContextSelection) => command<PreparedContext>("assistant_prepare_context", {selection});
export const askAssistant = (contextId: string, question: string) => command<string>("assistant_ask", {contextId, question});
