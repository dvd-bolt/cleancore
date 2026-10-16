import { expect, it, vi, beforeEach } from "vitest";
import * as ipc from "../../lib/ipc";
import { runGeneralAnalysis, type GeneralResult } from "./generalAnalysis";
import type { CleanupPreviewPage, TaskStatus } from "../../types";

vi.mock("../../lib/ipc", () => ({ startCleanupScan: vi.fn(), getScanProgress: vi.fn(), getCleanupPreview: vi.fn(), resumeCleanupScan: vi.fn(), cancelCleanupScan: vi.fn() }));
const rule = (id: string): ipc.CleanupRuleInfo => ({ id, name: id, description: "fixture", min_age_days: 7, preview_only: false, risk_level: "Low" });
const status = (state: TaskStatus["state"] = "Completed"): TaskStatus => ({ id: "task", kind: "scan", state, processed_items: 1300 });
const preview = (count: number, bytes: string, resumable = false, batch_ready = false): CleanupPreviewPage => ({ preview_id: "task", category_id: "temp", offset: 0, limit: 1, total_candidates: count, unfiltered_candidates: count, total_bytes: bytes, complete: !resumable, cancelled: false, resumable, batch_ready, diagnostics: {}, execution_preview_id: null, items: [], rule_version: "1" });
beforeEach(() => { vi.resetAllMocks(); vi.mocked(ipc.startCleanupScan).mockResolvedValue("task"); vi.mocked(ipc.getScanProgress).mockResolvedValue(status()); vi.mocked(ipc.cancelCleanupScan).mockResolvedValue(true); });

it("visits all batches, does not double-count a paused batch, and excludes projects", async () => {
  vi.mocked(ipc.getCleanupPreview)
    .mockResolvedValueOnce(preview(300, "3000", true))
    .mockResolvedValueOnce(preview(1000, "10000", true, true))
    .mockResolvedValueOnce(preview(205, "2050"))
    .mockResolvedValueOnce(preview(3, "9007199254740993"));
  const rows = new Map<string, GeneralResult>();
  const collect = vi.fn(async () => {});
  await runGeneralAnalysis([rule("temp"), rule("project-rust-target-v1"), rule("browser")], new AbortController().signal, row => rows.set(row.rule.id, row), collect);
  expect(collect).toHaveBeenCalledTimes(3);
  expect(vi.mocked(ipc.startCleanupScan).mock.calls.map(args => args[0])).toEqual(["temp", "browser"]);
  expect(ipc.resumeCleanupScan).toHaveBeenCalledTimes(2);
  expect(rows.get("temp")).toMatchObject({ state: "complete", candidates: 1205, bytes: "12050" });
  expect(rows.get("browser")?.bytes).toBe("9007199254740993");
});

it("marks a category failed when continuation fails so its earlier retained batches can be withdrawn", async () => {
  vi.mocked(ipc.getCleanupPreview).mockResolvedValue(preview(1000, "10000", true, true));
  vi.mocked(ipc.resumeCleanupScan).mockRejectedValue(new Error("continuation failed"));
  const result = await runGeneralAnalysis([rule("temp")], new AbortController().signal, () => {}, vi.fn(async () => {}));
  expect(result.failedCategories).toEqual(["temp"]);
  expect(ipc.cancelCleanupScan).toHaveBeenCalledWith("task");
});

it("reports unavailable categories and continues without pretending they are empty", async () => {
  vi.mocked(ipc.startCleanupScan).mockRejectedValueOnce(new Error("APPLICATION_RUNNING"));
  vi.mocked(ipc.getCleanupPreview).mockResolvedValue({ ...preview(2, "20"), complete: false, diagnostics: { ACCESS_DENIED: 1 } });
  const rows = new Map<string, GeneralResult>();
  await runGeneralAnalysis([rule("browser"), rule("wer")], new AbortController().signal, row => rows.set(row.rule.id, row));
  expect(rows.get("browser")).toMatchObject({ state: "unavailable", error: "APPLICATION_RUNNING" });
  expect(rows.get("wer")).toMatchObject({ state: "partial", candidates: 2, diagnostics: { ACCESS_DENIED: 1 } });
});

it("cancels a task whose start reply arrives after stopping, and does not start another category", async () => {
  const controller = new AbortController();
  vi.mocked(ipc.startCleanupScan).mockImplementation(async () => { controller.abort(); return "late-task"; });
  await runGeneralAnalysis([rule("temp"), rule("browser")], controller.signal, () => {});
  expect(ipc.cancelCleanupScan).toHaveBeenCalledWith("late-task");
  expect(ipc.getScanProgress).not.toHaveBeenCalled();
  expect(ipc.startCleanupScan).toHaveBeenCalledTimes(1);
});
