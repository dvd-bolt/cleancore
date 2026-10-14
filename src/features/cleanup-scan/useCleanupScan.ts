import { useCallback, useEffect, useRef, useState } from "react";
import { cancelCleanupScan, getCleanupPreview, getScanProgress, isTauriEnvironment, startCleanupScan } from "../../lib/ipc";
import type { CleanupPreviewPage, TaskStatus } from "../../types";
export const PAGE_SIZE = 15;
const SESSION_KEY = "cleancore-scan-task";
export function useCleanupScan() {
  const [taskId,setTaskId]=useState<string | null>(() => isTauriEnvironment() ? sessionStorage.getItem(SESSION_KEY) : null);
  const [status,setStatus]=useState<TaskStatus | null>(null);
  const [scanning,setScanning]=useState(!!taskId);
  const [preview,setPreview]=useState<CleanupPreviewPage | null>(null);
  const [loading,setLoading]=useState(false);
  const [page,setPage]=useState(0);
  const [query,setQuery]=useState("");
  const [error,setError]=useState<string | null>(null);
  const mounted=useRef(true); const startPending=useRef(false); const request=useRef(0);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;request.current++;};},[]);
  useEffect(()=>{
    if(!taskId || !scanning)return;
    let disposed=false;let pending=false;
    const poll=async()=>{if(pending)return;pending=true;
      try{const value=await getScanProgress(taskId);if(disposed)return;setStatus(value);
        if(value.state!=="Running" && value.state!=="Pending"){setScanning(false);if(value.state==="Failed")setError(value.error_message ?? "Сканирование не завершено");}
      }catch(e){if(!disposed){setError(e instanceof Error?e.message:String(e));setScanning(false);}}
      finally{pending=false;}
    };
    void poll();const timer=window.setInterval(poll,500);
    return()=>{disposed=true;window.clearInterval(timer);};
  },[taskId,scanning]);
  useEffect(()=>{
    if(!taskId || !status || scanning || status.state==="Failed")return;
    let disposed=false;const serial=++request.current;setLoading(true);
    const timer=window.setTimeout(()=>{
      getCleanupPreview(taskId,page*PAGE_SIZE,PAGE_SIZE,query).then(value=>{if(!disposed && serial===request.current)setPreview(value);})
        .catch(e=>{if(!disposed && serial===request.current)setError(e instanceof Error?e.message:String(e));})
        .finally(()=>{if(!disposed && serial===request.current)setLoading(false);});
    },200);
    return()=>{disposed=true;window.clearTimeout(timer);};
  },[taskId,status,scanning,page,query]);
  const start=useCallback(async()=>{
    if(startPending.current || scanning)return;startPending.current=true;setError(null);setScanning(true);setPreview(null);setStatus(null);setTaskId(null);setPage(0);setQuery("");request.current++;
    try{sessionStorage.removeItem(SESSION_KEY);const id=await startCleanupScan();sessionStorage.setItem(SESSION_KEY,id);if(mounted.current){setTaskId(id);setStatus({id,kind:"scan:user-temp-v1",state:"Running",processed_items:0});}}
    catch(e){if(mounted.current){setError(e instanceof Error?e.message:String(e));setScanning(false);}}
    finally{startPending.current=false;}
  },[scanning]);
  const cancel=async()=>{if(!taskId)return;try{await cancelCleanupScan(taskId);}catch(e){if(mounted.current)setError(e instanceof Error?e.message:String(e));}};
  const search=(value:string)=>{setQuery(value);setPage(0);};
  return {taskId,status,scanning,preview,loading,page,setPage,query,search,error,start,cancel};
}
