// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, afterEach, describe, it, expect, vi } from "vitest";
import { DuplicateActions } from "./DuplicateActions";
import * as ipc from "../../lib/ipc";
import type { DuplicateGroup } from "../../types";
vi.mock("../../lib/ipc", () => ({ prepareDuplicateCleanup: vi.fn() }));
vi.mock("../cleanup-execution/CleanupExecutionPanel", () => ({ CleanupExecutionPanel: () => <div>Точный предпросмотр</div> }));
const group: DuplicateGroup = { group_id: "group-1", file_size_bytes: 10, full_hash: "ab".repeat(32), reclaimable_bytes: 20, files: ["original", "copy-1", "copy-2"].map(name => ({ path: `C:\\Users\\owned\\Documents\\${name}`, size_bytes: 10, modified_utc: "2026-10-15T00:00:00Z" })) };
let container: HTMLDivElement, root: Root;
beforeEach(() => { vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true); vi.clearAllMocks(); container = document.createElement("div"); document.body.append(container); root = createRoot(container); });
afterEach(async () => { await act(async () => root.unmount()); container.remove(); vi.unstubAllGlobals(); });
describe("manual duplicate selection", () => {
  it("requires an explicit keeper and sends only selected copy indices", async () => {
    vi.mocked(ipc.prepareDuplicateCleanup).mockResolvedValue({ preview_id: "preview", category: "duplicates-v1", rule_version: 1, expires_at_ms: Date.now() + 10000, candidates: [] });
    await act(async () => root.render(<DuplicateActions group={group} reportId="report-1"/>));
    const prepare = () => container.querySelector("button")!;
    expect(prepare().disabled).toBe(true);
    await act(async () => { const select = container.querySelector("select")!; select.value = "0"; select.dispatchEvent(new Event("change", { bubbles: true })); });
    expect(container.querySelectorAll('input[type="checkbox"]')).toHaveLength(2);
    expect(prepare().disabled).toBe(true);
    await act(async () => (container.querySelector('input[type="checkbox"]') as HTMLInputElement).click());
    await act(async () => prepare().click());
    expect(ipc.prepareDuplicateCleanup).toHaveBeenCalledWith("report-1", "group-1", 0, [1]);
    expect(container.textContent).toContain("Сохранить:");
  });
  it("surfaces protected-original refusal without opening a success view", async () => {
    vi.mocked(ipc.prepareDuplicateCleanup).mockRejectedValue(new Error("Защищённый оригинал"));
    await act(async () => root.render(<DuplicateActions group={group} reportId="report-1"/>));
    await act(async () => { const select = container.querySelector("select")!; select.value = "1"; select.dispatchEvent(new Event("change", { bubbles: true })); });
    await act(async () => (container.querySelector('input[type="checkbox"]') as HTMLInputElement).click());
    await act(async () => container.querySelector("button")!.click());
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("Защищённый оригинал");
    expect(container.textContent).not.toContain("Точный предпросмотр");
  });
});
