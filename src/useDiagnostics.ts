import { useEffect, useRef } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { diagnosticDisplay, RenderCounts } from "./diagnosticEvents";
import type { AppState } from "./state";

export interface DiagnosticStatus {segment:number;enabled:boolean;recording:boolean;directory:string;error:string|null;dropped:number}
type Kind="mount"|"unmount"|"snapshot_start"|"snapshot_end"|"snapshot_failed"|"listen_ready"|"listen_failed"|"gap"|"rejected"|"epoch_changed"|"display"|"render_summary"|"visibility"|"frontend_error"|"frontend_rejection"|"client_dropped";
type Reason="initial"|"gap"|"focus"|"visible"|"changed"|"unmount"|"interval";
interface Entry {segment?:number;kind:Kind;view:number;elapsedMs:number;revision?:number;durationMs?:number;flags?:number;count?:number;clockCount?:number;stateCount?:number;reason?:Reason}
let enabled=false,known=false,started=false,pending:Entry[]=[],queued=false,dropped=0,segment:number|undefined;
const listeners=new Set<(enabled:boolean)=>void>();
const origin=performance.now();
const elapsed=()=>Math.round(performance.now()-origin);
export const diagnosticsEnabled=()=>enabled;
function flush() {
  queued=false;
  if(!known||!enabled){if(known)pending=[];return;}
  const events=pending.splice(0,dropped?31:32).map(entry=>({...entry,segment:entry.segment??segment}));
  if(dropped){events.push({segment,kind:"client_dropped",view:0,elapsedMs:elapsed(),count:dropped});dropped=0;}
  if(pending.length){queued=true;queueMicrotask(flush);}
  if(events.length)void invoke("diagnostics_report",{events}).catch(()=>{if(enabled)dropped=Math.min(1_000_000,dropped+events.length);});
}
export function diagnosticEvent(kind:Kind,detail:Omit<Partial<Entry>,"kind"|"elapsedMs">={}) {
  if(known&&!enabled)return;
  if(pending.length<32)pending.push({segment,kind,view:0,elapsedMs:elapsed(),...detail});else dropped=Math.min(1_000_000,dropped+1);
  if(!queued&&known){queued=true;queueMicrotask(flush);}
}
export function setDiagnosticsEnabled(next:boolean,nextSegment?:number) {
  if(!isTauri())return;
  const changed=next!==enabled||(next&&nextSegment!==undefined&&nextSegment!==segment);known=true;enabled=next;segment=nextSegment??segment;
  if(!enabled){pending=[];dropped=0;}else flush();
  if(changed)for(const listener of listeners)listener(next);
}
export function startDiagnostics() {
  if(started||!isTauri())return;started=true;
  diagnosticEvent("mount");
  // Independent of the state actor: diagnose a blocked initial snapshot.
  void invoke<DiagnosticStatus>("diagnostics_status").then(status=>{if(!known)setDiagnosticsEnabled(status.enabled,status.segment);}).catch(()=>{if(!known)setDiagnosticsEnabled(false);});
  window.addEventListener("error",()=>diagnosticEvent("frontend_error"));
  window.addEventListener("unhandledrejection",()=>diagnosticEvent("frontend_rejection"));
}

export function useDiagnostics(state:AppState|null,surface:"main"|"shell"|"preview"|"usage",clock?:number,visible=true) {
  const view={main:1,shell:2,preview:3,usage:4}[surface];
  const current=useRef({state,clock,visible});current.current={state,clock,visible};
  const counters=useRef(new RenderCounts());
  const previous=useRef<{flags:number;at:number}|null>(null);
  const report=useRef(()=>{});
  report.current=()=>{
    if(!enabled)return;
    const latest=current.current;
    const flags=diagnosticDisplay(latest.state,surface,latest.visible&&!document.hidden);
    if(previous.current?.flags!==flags){
      const at=elapsed();
      diagnosticEvent("display",{view,flags,revision:latest.state?.revision,durationMs:previous.current?at-previous.current.at:undefined,reason:previous.current?"changed":"initial"});
      previous.current={flags,at};
    }
  };
  useEffect(()=>{
    if(enabled){counters.current.commit(state?.revision,clock);report.current();}
  });
  useEffect(()=>{
    let timer:ReturnType<typeof setInterval>|undefined;
    const summary=(reason:Reason)=>{const counts=counters.current.take();if(counts.count)diagnosticEvent("render_summary",{view,...counts,reason});};
    const configure=(active:boolean)=>{
      clearInterval(timer);previous.current=null;counters.current.take();
      if(active){report.current();timer=setInterval(()=>summary("interval"),60000);}
    };
    const visibility=()=>{diagnosticEvent("visibility",{view,flags:document.hidden?0:128});report.current();};
    listeners.add(configure);configure(enabled);document.addEventListener("visibilitychange",visibility);
    return()=>{summary("unmount");diagnosticEvent("unmount",{view,flags:previous.current?.flags});clearInterval(timer);listeners.delete(configure);document.removeEventListener("visibilitychange",visibility);};
  },[view]);
}
