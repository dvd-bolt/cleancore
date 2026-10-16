import { cancelCleanupScan, getCleanupPreview, getScanProgress, resumeCleanupScan, startCleanupScan, type CleanupRuleInfo } from "../../lib/ipc";

export type GeneralResult = {
  rule: CleanupRuleInfo;
  state: "pending" | "running" | "complete" | "partial" | "unavailable" | "cancelled";
  candidates: number;
  bytes: string;
  inspected: number;
  diagnostics: Record<string, number>;
  error?: string;
};

export function generalRules(rules: CleanupRuleInfo[]) {
  return rules.filter(rule => !rule.id.startsWith("project-"));
}

function pause(signal: AbortSignal) {
  return new Promise<void>(resolve => {
    const done = () => { clearTimeout(timer); signal.removeEventListener("abort", done); resolve(); };
    const timer = setTimeout(done, 500);
    signal.addEventListener("abort", done, { once: true });
    if (signal.aborted) done();
  });
}

// Read-only scanning; optionally retain verified batches in a backend snapshot.
export async function runGeneralAnalysis(
  rules: CleanupRuleInfo[], signal: AbortSignal, update: (row: GeneralResult) => void,
  collect?: (taskId: string) => Promise<void>,
) {
  let activeTask: string | null = null;
  const failedCategories: string[] = [];
  try {
    for (const rule of generalRules(rules)) {
      if (signal.aborted) break;
      let row: GeneralResult = { rule, state: "running", candidates: 0, bytes: "0", inspected: 0, diagnostics: {} };
      update(row);
      try {
        activeTask = await startCleanupScan(rule.id);
        while (!signal.aborted) {
          const status = await getScanProgress(activeTask);
          if (signal.aborted) break;
          row = { ...row, inspected: status.processed_items };
          update(row);
          if (status.state === "Pending" || status.state === "Running") { await pause(signal); continue; }
          if (status.state === "Failed") throw new Error(status.error_message ?? "Анализ категории не завершён");
          const preview = await getCleanupPreview(activeTask, 0, 1);
          if (signal.aborted) break;
          // A time slice can retain the same unfinished batch: count it only once.
          if (preview.batch_ready || !preview.resumable) {
            row = { ...row, candidates: row.candidates + preview.unfiltered_candidates, bytes: (BigInt(row.bytes) + BigInt(preview.total_bytes)).toString() };
          }
          row = { ...row, diagnostics: preview.diagnostics };
          if (preview.cancelled || status.state === "Cancelled") {
            row = { ...row, state: "cancelled" }; update(row); activeTask = null; return { failedCategories, cancelled: true };
          }
          if (preview.batch_ready || !preview.resumable) await collect?.(activeTask);
          if (!preview.resumable) {
            row = { ...row, state: preview.complete ? "complete" : "partial" }; update(row); activeTask = null; break;
          }
          update(row);
          await resumeCleanupScan(activeTask);
        }
      } catch (error) {
        failedCategories.push(rule.id);
        if (!signal.aborted) update({ ...row, state: "unavailable", error: error instanceof Error ? error.message : String(error) });
        // A failed preview/resume can leave a live task. Stop before the next category.
        if (activeTask) await cancelCleanupScan(activeTask).catch(() => {});
        activeTask = null;
      }
      if (signal.aborted) { update({ ...row, state: "cancelled" }); break; }
    }
  } finally {
    if (activeTask) await cancelCleanupScan(activeTask).catch(() => {});
  }
  return { failedCategories, cancelled: signal.aborted };
}
