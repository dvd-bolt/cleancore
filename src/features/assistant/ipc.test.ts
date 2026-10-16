import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
import { askAssistant, chooseAssistantModel, getAssistantStatus, prepareAssistantContext } from "./ipc";
afterEach(()=>{vi.unstubAllGlobals();vi.clearAllMocks();});
describe("local assistant boundary",()=>{
  it("does not invent a model or answer outside desktop",async()=>{
    await expect(getAssistantStatus()).rejects.toThrow("desktop");
    await expect(chooseAssistantModel()).rejects.toThrow("desktop");
    await expect(askAssistant("fake","question")).rejects.toThrow("desktop");expect(invoke).not.toHaveBeenCalled();
  });
  it("sends only context IDs and bounded selections, preserving backend failures",async()=>{
    vi.stubGlobal("window",{__TAURI_INTERNALS__:{}});
    vi.mocked(invoke).mockResolvedValueOnce({id:"snapshot",summary:{},recommendations:[]});
    await prepareAssistantContext({metrics:true,preview_id:null,journal_ids:[]});
    expect(invoke).toHaveBeenCalledWith("assistant_prepare_context",{selection:{metrics:true,preview_id:null,journal_ids:[]}});
    vi.mocked(invoke).mockRejectedValueOnce({code:"MODEL_MISSING",message:"Выберите модель"});
    await expect(askAssistant("snapshot","Вопрос")).rejects.toThrow("Выберите модель");
    expect(invoke).toHaveBeenLastCalledWith("assistant_ask",{contextId:"snapshot",question:"Вопрос"});
  });
});
