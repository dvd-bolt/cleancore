// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, afterEach, describe, it, expect, vi } from "vitest";
import { DiskTreemap } from "./DiskTreemap";
import * as ipc from "../../lib/ipc";
import type { DiskAnalysisReport, TreemapData, TreemapNode } from "../../types";
vi.mock("../../lib/ipc", () => ({ getDiskTreemap: vi.fn(), revealDiskEntry: vi.fn(), prepareDiskFileCleanup: vi.fn(), invalidateDiskReport: vi.fn() }));
vi.mock("../cleanup-execution/CleanupExecutionPanel", () => ({ CleanupExecutionPanel: () => <div>Предпросмотр подтверждения</div> }));
vi.mock("./MapCanvas", () => ({ bytes: (n: number) => `${n} Б`, MapCanvas: ({ root, onSelect, onOpen }: { root: TreemapNode; onSelect: (n: TreemapNode) => void; onOpen: (n: TreemapNode) => void }) => <div><button onClick={() => onSelect(root.children[0])}>Выбрать блок</button><button onClick={() => onOpen(root.children[0])}>Внутрь блока</button></div> }));
const file: TreemapNode = { id: "33", path: "C:\\folder", name: "folder", size_bytes: 100, logical_bytes: 200, allocated_bytes: 100, file_count: 1, is_dir: true, is_aggregate: false, extension: "", children: [] };
const data: TreemapData = { root: { ...file, id: "5", path: "C:\\", name: "C:\\", children: [file] }, parent_path: null, indexed: true, extensions: [{ extension: "bin", bytes: 100, files: 1 }], empty_file_count: 0, empty_files: [] };
const report = { root_path: "C:\\", engine: "ntfs-mft" } as DiskAnalysisReport;
let container: HTMLDivElement, root: Root;
beforeEach(() => { vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true); vi.clearAllMocks(); vi.mocked(ipc.getDiskTreemap).mockResolvedValue(data); container = document.createElement("div"); document.body.append(container); root = createRoot(container); });
afterEach(async () => { await act(async () => root.unmount()); container.remove(); vi.unstubAllGlobals(); });
const button = (text: string) => Array.from(container.querySelectorAll("button")).find(b => b.textContent === text)!;
describe("disk map interactions", () => {
  it("requests an ordinary folder scan instead of relabelling the summary as another path", async () => {
    const navigate=vi.fn();
    const ordinary={...report,engine:"directory",total_bytes:100,total_files:1,subdirs:[{path:"C:\\folder",name:"folder",size_bytes:100,is_dir:true,file_count:1,percentage:100,category:"Other"}],largest_files:[]} as DiskAnalysisReport;
    await act(async()=>root.render(<DiskTreemap report={ordinary} reportId="ordinary" onNavigate={navigate}/>));
    expect(ipc.getDiskTreemap).not.toHaveBeenCalled();
    await act(async()=>button("Внутрь блока").click());
    expect(navigate).toHaveBeenLastCalledWith("C:\\folder");
    expect(container.textContent).toContain("Сводная карта обычного обхода");
  });
  it("uses the current report and reuses its index for navigation and allocation mode", async () => {
    await act(async () => root.render(<DiskTreemap report={report} reportId="report-1" />));
    expect(ipc.getDiskTreemap).toHaveBeenLastCalledWith("report-1", "C:\\", "logical");
    await act(async () => button("Выбрать блок").click());
    expect((container.querySelector('input[readonly]') as HTMLInputElement).value).toBe("C:\\folder");
    await act(async () => button("Показать в Проводнике").click());
    expect(ipc.revealDiskEntry).toHaveBeenLastCalledWith("report-1", "33");
    await act(async () => button("Внутрь блока").click());
    expect(ipc.getDiskTreemap).toHaveBeenLastCalledWith("report-1", "C:\\folder", "logical");
    await act(async () => { const select = container.querySelector("select")!; select.value = "allocated"; select.dispatchEvent(new Event("change", { bubbles: true })); });
    expect(ipc.getDiskTreemap).toHaveBeenLastCalledWith("report-1", "C:\\folder", "allocated");
  });
  it("prepares exactly the selected indexed object and surfaces scope refusals", async () => {
    vi.mocked(ipc.prepareDiskFileCleanup).mockRejectedValue(new Error("MANUAL_SCOPE: системная область"));
    await act(async () => root.render(<DiskTreemap report={report} reportId="report-1"/>));
    await act(async () => button("Выбрать блок").click());
    await act(async () => button("Удалить…").click());
    expect(ipc.prepareDiskFileCleanup).toHaveBeenCalledWith("report-1", "33");
    expect(container.textContent).toContain("MANUAL_SCOPE");
    expect(container.textContent).not.toContain("Предпросмотр подтверждения");
  });
  it("reports stale-index failures and never treats them as an empty successful map", async () => {
    vi.mocked(ipc.getDiskTreemap).mockRejectedValue(new Error("Индекс заменён"));
    await act(async () => root.render(<DiskTreemap report={report} reportId="stale" />));
    expect(container.textContent).toContain("Индекс заменён");
    expect(container.textContent).not.toContain("Выбрать блок");
  });
});
