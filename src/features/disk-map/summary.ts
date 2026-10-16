import type { DiskAnalysisReport, TreemapData, TreemapNode } from "../../types";

/** Ordinary reports retain direct folders and top files only; never invent descendants. */
export function summaryMap(report: DiskAnalysisReport): TreemapData {
  const normalize = (s: string) => s.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
  const folder = normalize(report.root_path);
  const entries = [...report.subdirs, ...report.largest_files.filter(file => {
    const path = normalize(file.path); return path.slice(0, path.lastIndexOf("\\")) === folder;
  })];
  const children: TreemapNode[] = entries.map(entry => ({ id: `summary:${entry.path}`, name: entry.name, path: entry.path,
    size_bytes: entry.size_bytes, logical_bytes: entry.size_bytes, allocated_bytes: null, is_dir: entry.is_dir,
    is_aggregate: false, file_count: entry.file_count, extension: entry.is_dir ? "" : entry.name.split(".").slice(1).pop()?.toLowerCase() ?? "", children: [] }));
  const remainder = Math.max(0, report.total_bytes - children.reduce((sum, n) => sum + n.size_bytes, 0));
  if (remainder) children.push({ id: "summary:other", name: "Остальные файлы", path: report.root_path, size_bytes: remainder, logical_bytes: remainder,
    allocated_bytes: null, is_dir: true, is_aggregate: true, file_count: 0, extension: "", children: [] });
  const root: TreemapNode = { id: "summary:root", name: report.root_path, path: report.root_path, size_bytes: report.total_bytes,
    logical_bytes: report.total_bytes, allocated_bytes: null, is_dir: true, is_aggregate: false, file_count: report.total_files, extension: "", children };
  return { root, extensions: [], empty_files: [], empty_file_count: 0, parent_path: null, indexed: false };
}
