// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import { GeneralCleanupSelection } from "./GeneralCleanupSelection";
import * as ipc from "./generalCleanupIpc";
import type { CleanupRuleInfo } from "../../lib/ipc";
vi.mock("./generalCleanupIpc", async importOriginal => ({ ...await importOriginal<typeof import("./generalCleanupIpc")>(), getGeneralCleanupPage: vi.fn(), prepareGeneralCleanup: vi.fn(), executeGeneralCleanup: vi.fn(), getGeneralCleanupJob: vi.fn(), cancelGeneralCleanupJob: vi.fn() }));
const rules: CleanupRuleInfo[] = [{ id: "user-temp-v1", name: "Temp", description: "Старые временные файлы", min_age_days: 7, risk_level: "Low", preview_only: false }, { id: "crash-dumps-v1", name: "Дампы", description: "Диагностика будет потеряна", min_age_days: 30, risk_level: "Medium", preview_only: false }];
const index: ipc.GeneralIndex = { session_id: "snapshot", expires_at_ms: Date.now() + 600000, total_bytes: "60", partial: false, truncated: false, consumed: false, items: [{ object_id: "a", category_id: "user-temp-v1", size_bytes: "10" }, { object_id: "b", category_id: "user-temp-v1", size_bytes: "20" }, { object_id: "c", category_id: "crash-dumps-v1", size_bytes: "30" }] };
let container: HTMLDivElement, root: Root;
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true); vi.clearAllMocks(); sessionStorage.clear();
  vi.mocked(ipc.cancelGeneralCleanupJob).mockResolvedValue(true);
  vi.mocked(ipc.getGeneralCleanupPage).mockResolvedValue({ total: 3, items: index.items.map(item => ({ ...item, display_path: `${item.object_id}.tmp` })) });
  vi.mocked(ipc.prepareGeneralCleanup).mockResolvedValue({ confirmation_id: "token", selected_count: 1, selected_bytes: "10", expires_at_ms: Date.now() + 120000 });
  vi.mocked(ipc.executeGeneralCleanup).mockResolvedValue({ job_id: "job", status: "Completed", selected_count: 1, processed: 1, deleted: 1, skipped: 0, failed: 0, deleted_bytes: "10", operation_ids: ["journal"], message: null });
  container = document.createElement("div"); document.body.append(container); root = createRoot(container);
});
afterEach(async () => { await act(async () => root.unmount()); container.remove(); sessionStorage.clear(); vi.unstubAllGlobals(); });
const button = (text: string) => Array.from(container.querySelectorAll("button")).find(b => b.textContent?.startsWith(text))!;
async function render(value = index) { await act(async () => { root.render(<GeneralCleanupSelection index={value} rules={rules} onRunningChange={() => {}}/>); }); await act(async () => { await new Promise(resolve => setTimeout(resolve, 180)); }); }

it("shows a unique total, selects a category and individual files, then requires irreversible confirmation", async () => {
  await render();
  expect(container.textContent).toContain("Можно освободить до 60 Б");
  expect(button("Очистить выбранное").disabled).toBe(true);
  expect(ipc.executeGeneralCleanup).not.toHaveBeenCalled();
  await act(async () => (container.querySelector('[aria-label="Очистить категорию Temp"]') as HTMLInputElement).click());
  expect(button("Очистить выбранное").textContent).toContain("(2)");
  await act(async () => (container.querySelector('[aria-label="Выбрать b.tmp"]') as HTMLInputElement).click());
  expect(button("Очистить выбранное").textContent).toContain("(1)");
  await act(async () => button("Очистить выбранное").click());
  expect(ipc.prepareGeneralCleanup).toHaveBeenCalledWith("snapshot", ["a"], false);
  expect(button("Подтвердить и очистить").disabled).toBe(true);
  expect(ipc.executeGeneralCleanup).not.toHaveBeenCalled();
  const ack = Array.from(container.querySelectorAll("label")).find(label => label.textContent?.includes("Подтверждаю безвозвратное"))!.querySelector("input")!;
  await act(async () => ack.click());
  await act(async () => button("Подтвердить и очистить").click());
  expect(ipc.executeGeneralCleanup).toHaveBeenCalledWith("token", true);
  expect(container.textContent).toContain("Очистка завершена");
  expect(container.textContent).toContain("Список использован");
  expect(button("Очистить выбранное")).toBeUndefined();
  expect(container.querySelector(".action-panel")?.textContent).toContain("Очистка завершена");
  expect(container.querySelector(".action-panel")!.compareDocumentPosition(button("Следующие файлы")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

it("requires separate partial acknowledgement and does not enable an expired confirmation", async () => {
  await render({ ...index, partial: true });
  await act(async () => button("Выбрать всё").click());
  expect(button("Очистить выбранное").disabled).toBe(true);
  const partial = Array.from(container.querySelectorAll("label")).find(label => label.textContent?.includes("Разрешаю очистку"))!.querySelector("input")!;
  await act(async () => partial.click());
  expect(button("Очистить выбранное").disabled).toBe(false);
  vi.mocked(ipc.prepareGeneralCleanup).mockResolvedValue({ confirmation_id: "expired", selected_count: 3, selected_bytes: "60", expires_at_ms: Date.now() - 1 });
  await act(async () => button("Очистить выбранное").click());
  const ack = Array.from(container.querySelectorAll("label")).find(label => label.textContent?.includes("Подтверждаю безвозвратное"))!.querySelector("input")!;
  await act(async () => ack.click());
  expect(button("Подтвердить и очистить").disabled).toBe(true);
  expect(ipc.executeGeneralCleanup).not.toHaveBeenCalled();
});

it("formats gigabytes without exposing a raw byte-only total", () => {
  expect(ipc.formatCleanupBytes("1073741824")).toBe("1 ГБ");
  expect(ipc.formatCleanupBytes("1610612736")).toBe("1,5 ГБ");
});

it("restores a running cleanup with visible progress and cancellation above the file list", async () => {
  sessionStorage.setItem("cleancore-general-cleanup-job", "running-job");
  vi.mocked(ipc.getGeneralCleanupJob).mockResolvedValue({ job_id: "running-job", status: "Running", selected_count: 3, processed: 1, deleted: 1, skipped: 0, failed: 0, deleted_bytes: "10", operation_ids: [], message: null });
  await act(async () => root.render(<GeneralCleanupSelection index={null} rules={rules} onRunningChange={() => {}}/>));
  expect(container.querySelector(".action-panel")?.textContent).toContain("Очистка выполняется");
  expect(container.querySelector("progress")?.value).toBe(1);
  expect(container.querySelector("progress")?.max).toBe(3);
  await act(async () => button("Остановить очистку").click());
  expect(ipc.cancelGeneralCleanupJob).toHaveBeenCalledWith("running-job");
  expect(ipc.executeGeneralCleanup).not.toHaveBeenCalled();
});
