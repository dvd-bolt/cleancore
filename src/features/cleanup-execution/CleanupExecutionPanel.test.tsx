// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CleanupExecutionPanel } from "./CleanupExecutionPanel";
import * as ipc from "./ipc";
import type { CleanupOutcome } from "./types";
vi.mock("../../lib/ipc", () => ({ isTauriEnvironment: () => true }));
vi.mock("./ipc", () => ({ getAvailability: vi.fn(), getExecutionPreview: vi.fn(), getOperation: vi.fn(), cancelCleanup: vi.fn(), prepareConfirmation: vi.fn(), executeCleanup: vi.fn() }));
let container: HTMLDivElement, root: Root;
const operation: CleanupOutcome = { operation_id: "last", preview_id: "preview", category: "user-temp-v1", started_at_ms: 1, status: "Running", journal_ok: true, deleted_bytes: "10", recycled_bytes: "0", items: [
  { object_id: "a", display_path: "a.tmp", size_bytes: "10", status: "Deleted", reason_code: "DELETED_PERMANENTLY" },
  { object_id: "b", display_path: "b.tmp", size_bytes: "10", status: "Pending", reason_code: "PENDING" },
] };
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true); vi.clearAllMocks(); sessionStorage.clear(); sessionStorage.setItem("cleancore-cleanup-operation", "last");
  vi.mocked(ipc.getAvailability).mockResolvedValue({ execution_enabled: true, persistent_journal: true, reason: "Доступно" });
  vi.mocked(ipc.getExecutionPreview).mockResolvedValue(null);
  vi.mocked(ipc.getOperation).mockResolvedValue(operation);
  vi.mocked(ipc.cancelCleanup).mockResolvedValue(true);
  container = document.createElement("div"); document.body.append(container); root = createRoot(container);
});
afterEach(async () => { await act(async () => root.unmount()); container.remove(); sessionStorage.clear(); vi.unstubAllGlobals(); });

it("restores category progress in the pinned controls and permits cancellation without re-executing", async () => {
  await act(async () => root.render(<CleanupExecutionPanel selectedIds={[]}/>));
  expect(container.querySelector(".action-panel")?.textContent).toContain("Очистка выполняется");
  expect(container.querySelector("progress")?.value).toBe(1);
  expect(container.querySelector("progress")?.max).toBe(2);
  await act(async () => [...container.querySelectorAll("button")].find(b => b.textContent === "Остановить очистку")!.click());
  expect(ipc.cancelCleanup).toHaveBeenCalledWith("last");
  expect(ipc.executeCleanup).not.toHaveBeenCalled();
});

it("retains the completed category result on navigation and offers the journal", async () => {
  const openJournal = vi.fn();
  vi.mocked(ipc.getOperation).mockResolvedValue({ ...operation, status: "Completed" });
  await act(async () => root.render(<CleanupExecutionPanel selectedIds={[]} onOpenJournal={openJournal}/>));
  expect(container.querySelector(".action-panel")?.textContent).toContain("Обработка завершена");
  await act(async () => [...container.querySelectorAll("button")].find(b => b.textContent === "Открыть журнал")!.click());
  expect(openJournal).toHaveBeenCalledOnce();
  expect(container.querySelector("progress")).toBeNull();
  expect(ipc.executeCleanup).not.toHaveBeenCalled();
});
