// @vitest-environment jsdom
import {act} from "react";
import {createRoot, type Root} from "react-dom/client";
import {beforeEach,afterEach,describe,it,expect,vi} from "vitest";
import {DiskPage} from "./DiskPage";
import * as ipc from "../lib/ipc";
vi.mock("../features/disk-map/DiskTreemap",()=>({DiskTreemap:()=>null}));
vi.mock("../lib/ipc",()=>({
 getDiskTreemap:vi.fn().mockResolvedValue({root:{id:"root",size_bytes:0,children:[],is_dir:true},extensions:[],empty_files:[],empty_file_count:0,parent_path:null}),revealDiskEntry:vi.fn(),
 chooseAnalysisFolder:vi.fn(),isTauriEnvironment:()=>true,
 cancelDiskTask:vi.fn(),fetchQuickDirectories:vi.fn(),fetchResourceHistory:vi.fn(),
 fetchSpaceHogs:vi.fn().mockResolvedValue([{path:"C:/test_large.iso",size_bytes:5368709120,category:"DiskImage",extension:"iso",is_system_locked:false}]),
 fetchStaleFiles:vi.fn().mockResolvedValue([{path:"C:/Downloads/old_archive.zip",name:"old_archive.zip",size_bytes:104857600,last_modified_utc:"2023-01-01T00:00:00Z",days_old:650,extension:"zip"}]),
 revealPathInExplorer:vi.fn().mockResolvedValue(undefined),
 getDiskAnalysisProgress:vi.fn(),getDiskAnalysisReport:vi.fn(),getDuplicateReport:vi.fn(),getDuplicateScanProgress:vi.fn(),startDiskAnalysis:vi.fn(),startDuplicateScan:vi.fn()
}));
let container:HTMLDivElement;let root:Root;
beforeEach(()=>{
 vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT",true);
 sessionStorage.clear();vi.clearAllMocks();
 vi.mocked(ipc.fetchQuickDirectories).mockResolvedValue([{id:"drive-2",name:"Диск C:",path:"C:/"}]);
 vi.mocked(ipc.getDiskAnalysisProgress).mockResolvedValue({task_id:"disk-task",kind:"disk_analysis",state:"Completed",progress_message:"частично",scanned_items:1,scanned_bytes:42,error_message:null});
 vi.mocked(ipc.getDiskAnalysisReport).mockResolvedValue({root_path:"C:/",total_bytes:42,total_files:1,total_dirs:0,complete:false,diagnostics:{ACCESS_DENIED:1},categories:[],largest_dirs:[],subdirs:[],largest_files:[{path:"C:/known.txt",name:"known.txt",size_bytes:42,is_dir:false,file_count:1,percentage:100,category:"Documents"}]});
 container=document.createElement("div");document.body.append(container);root=createRoot(container);
});
afterEach(async()=>{await act(async()=>root.unmount());container.remove();vi.unstubAllGlobals();});
describe("disk report lifecycle",()=>{
 it("browses the MFT snapshot without starting another scan",async()=>{
  sessionStorage.setItem("cleancore-disk-task","disk-task");
  const initial = await ipc.getDiskAnalysisReport("fixture");
  vi.mocked(ipc.getDiskAnalysisReport).mockClear();
  vi.mocked(ipc.getDiskAnalysisReport).mockResolvedValueOnce({...initial,engine:"ntfs-mft",elapsed_ms:1250,subdirs:[{path:"C:/folder",name:"folder",size_bytes:42,is_dir:true,file_count:1,percentage:100,category:"Other"}]}).mockResolvedValueOnce({...initial,engine:"ntfs-mft",root_path:"C:/folder"});
  await act(async()=>root.render(<DiskPage/>));
  expect(container.textContent).toContain("Быстрый анализ NTFS");
  const button=Array.from(container.querySelectorAll("button")).find(b=>b.textContent?.startsWith("folder"));
  expect(button).toBeDefined();
  await act(async()=>button!.click());
  expect(ipc.getDiskAnalysisReport).toHaveBeenLastCalledWith("disk-task","C:/folder");
  expect(ipc.startDiskAnalysis).not.toHaveBeenCalled();
  expect(container.textContent).toContain("На уровень выше");
 });
 it("restores a completed task across navigation and shows readable partial results",async()=>{
  sessionStorage.setItem("cleancore-disk-task","disk-task");
  await act(async()=>root.render(<DiskPage/>));
  expect(container.textContent).toContain("known.txt");
  expect(container.textContent).toContain("Частичный результат");
  expect(container.textContent).toContain("Нет доступа");
  await act(async()=>root.unmount());root=createRoot(container);
  await act(async()=>root.render(<DiskPage/>));
  expect(container.textContent).toContain("known.txt");
  expect(ipc.getDiskAnalysisReport).toHaveBeenCalledTimes(2);
 });
 it("preserves the cancelled partial report across navigation",async()=>{
  sessionStorage.setItem("cleancore-disk-task","disk-task");
  vi.mocked(ipc.getDiskAnalysisProgress).mockResolvedValue({task_id:"disk-task",kind:"disk_analysis",state:"Cancelled",progress_message:"",scanned_items:1,scanned_bytes:42,error_message:null});
  await act(async()=>root.render(<DiskPage/>));
  expect(container.textContent).toContain("Ниже сохранена прочитанная часть");
  expect(sessionStorage.getItem("cleancore-disk-task")).toBe("disk-task");
  await act(async()=>root.unmount());root=createRoot(container);
  await act(async()=>root.render(<DiskPage/>));
  expect(container.textContent).toContain("known.txt");
 });
 it("surfaces terminal backend errors instead of returning an empty page",async()=>{
  sessionStorage.setItem("cleancore-disk-task","disk-task");
  vi.mocked(ipc.getDiskAnalysisProgress).mockResolvedValue({task_id:"disk-task",kind:"disk_analysis",state:"Failed",progress_message:"",scanned_items:0,scanned_bytes:0,error_message:"Недоступен корень"});
  await act(async()=>root.render(<DiskPage/>));
  expect(container.textContent).toContain("Недоступен корень");
  expect(sessionStorage.getItem("cleancore-disk-task")).toBeNull();
 });
 it("switches to the space hogs tab and displays large files",async()=>{
  await act(async()=>root.render(<DiskPage/>));
  const tabBtn = Array.from(container.querySelectorAll("button")).find(b=>b.textContent?.includes("Крупные файлы"));
  expect(tabBtn).toBeDefined();
  await act(async()=>tabBtn!.click());
  expect(container.textContent).toContain("Space Hogs");
  expect(container.textContent).toContain("test_large.iso");
  expect(ipc.fetchSpaceHogs).toHaveBeenCalled();
 });
 it("switches to the stale files tab and displays stale items",async()=>{
  await act(async()=>root.render(<DiskPage/>));
  const tabBtn = Array.from(container.querySelectorAll("button")).find(b=>b.textContent?.includes("Забытые файлы"));
  expect(tabBtn).toBeDefined();
  await act(async()=>tabBtn!.click());
  expect(container.textContent).toContain("Забытые файлы");
  expect(container.textContent).toContain("old_archive.zip");
  expect(ipc.fetchStaleFiles).toHaveBeenCalled();
 });
});

it("chooses a folder without starting analysis and preserves the path on cancellation",async()=>{
 vi.mocked(ipc.chooseAnalysisFolder).mockResolvedValueOnce("C:/chosen").mockResolvedValueOnce(null);
 await act(async()=>root.render(<DiskPage/>));
 const pick=()=>Array.from(container.querySelectorAll("button")).find(b=>b.textContent==="Выбрать папку…")!;
 await act(async()=>pick().click());
 expect((container.querySelector('[aria-label="Путь к папке или диску для анализа"]') as HTMLInputElement).value).toBe("C:/chosen");
 expect(ipc.startDiskAnalysis).not.toHaveBeenCalled();
 expect(ipc.startDuplicateScan).not.toHaveBeenCalled();
 await act(async()=>pick().click());
 expect((container.querySelector('[aria-label="Путь к папке или диску для анализа"]') as HTMLInputElement).value).toBe("C:/chosen");
});
