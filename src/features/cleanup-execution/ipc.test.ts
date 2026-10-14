import { describe,it,expect,vi,afterEach } from "vitest";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
import { invoke } from "@tauri-apps/api/core";
import { getAvailability,getExecutionPreview,prepareConfirmation,executeCleanup,getJournal } from "./ipc";
afterEach(()=>{vi.unstubAllGlobals();vi.mocked(invoke).mockReset();});
describe("cleanup IPC boundary",()=>{
  it("browser preview cannot execute or fake a journal",async()=>{
    expect((await getAvailability()).execution_enabled).toBe(false);expect(await getExecutionPreview()).toBeNull();expect((await getJournal()).operations).toEqual([]);
    await expect(executeCleanup("token",true)).rejects.toThrow("desktop");expect(invoke).not.toHaveBeenCalled();
  });
  it("confirmation sends identifiers only and surfaces backend errors",async()=>{
    vi.stubGlobal("window",{__TAURI_INTERNALS__:{}});vi.mocked(invoke).mockRejectedValue({code:"PREVIEW_NOT_FOUND",message:"Preview не найден"});
    await expect(prepareConfirmation("preview",["object"])).rejects.toThrow("Preview не найден");
    expect(invoke).toHaveBeenCalledWith("prepare_cleanup_confirmation",{previewId:"preview",selectedIds:["object"]});
  });
});
