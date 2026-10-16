// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, afterEach, it, expect, vi } from "vitest";
import { CleanupSafetySettings } from "./CleanupSafetySettings";
import * as ipc from "../../lib/ipc";
vi.mock("../../lib/ipc", () => ({ getCleanupPreferences: vi.fn(), saveCleanupPreferences: vi.fn(), clearDuplicateHashCache: vi.fn(), isTauriEnvironment: () => true }));
let container: HTMLDivElement, root: Root;
beforeEach(() => { vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true); vi.clearAllMocks(); vi.mocked(ipc.getCleanupPreferences).mockResolvedValue({ excluded_paths: ["C:\\Keep"], protected_duplicate_roots: ["D:\\Originals"] }); container = document.createElement("div"); document.body.append(container); root = createRoot(container); });
afterEach(async () => { await act(async () => root.unmount()); container.remove(); vi.unstubAllGlobals(); });
it("loads persistent protection and submits both lists without silently dropping originals", async () => {
  vi.mocked(ipc.saveCleanupPreferences).mockResolvedValue(undefined);
  await act(async () => root.render(<CleanupSafetySettings/>));
  expect(Array.from(container.querySelectorAll("textarea")).map(t => t.value)).toEqual(["C:\\Keep", "D:\\Originals"]);
  await act(async () => container.querySelector("button")!.click());
  expect(ipc.saveCleanupPreferences).toHaveBeenCalledWith({ excluded_paths: ["C:\\Keep"], protected_duplicate_roots: ["D:\\Originals"] });
  expect(container.textContent).toContain("Старые подтверждения отозваны");
});
it("shows a failed save and never presents it as successful", async () => {
  vi.mocked(ipc.saveCleanupPreferences).mockRejectedValue(new Error("Дождитесь завершения очистки"));
  await act(async () => root.render(<CleanupSafetySettings/>));
  await act(async () => container.querySelector("button")!.click());
  expect(container.textContent).toContain("Дождитесь завершения очистки");
  expect(container.textContent).not.toContain("Сохранено.");
});
