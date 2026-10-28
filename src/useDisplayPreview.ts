import { useEffect, useRef, useState } from "react";
import { command, native, type AppState, type Settings } from "./state";
import { widgetDraftIssues } from "./WidgetAppearanceSettings";
interface PreviewPatch {
  themePreference:Settings["themePreference"];
  widgetAppearance:Settings["widgetAppearance"];
  snapGapLogicalPx:number;
  freezePopup:boolean;
}
interface Session {
  owner:string;
  sequence:number;
  queue:Promise<unknown>;
  stopped:boolean;
  timer:ReturnType<typeof setTimeout>|null;
  lastValid?:PreviewPatch;
}
function patchFor(session:Session,draft:Settings,freezePopup:boolean,state:AppState):PreviewPatch|undefined {
  if(widgetDraftIssues(draft,state).length) {
    // Release the popup even when a numeric field is temporarily incomplete.
    return session.lastValid?{...session.lastValid,freezePopup}:undefined;
  }
  const patch={themePreference:draft.themePreference,widgetAppearance:draft.widgetAppearance,snapGapLogicalPx:draft.windows.snapGapLogicalPx,freezePopup};
  session.lastValid=patch;
  return patch;
}
export function useDisplayPreview(state:AppState,draft:Settings,baseRevision:number,active:boolean,saving:boolean,freezePopup:boolean) {
  const session=useRef<Session|null>(null);
  const retiring=useRef<Promise<unknown>>(Promise.resolve());
  const latest=useRef({draft,saving,freezePopup,state});
  latest.current={draft,saving,freezePopup,state};
  const [cycle,setCycle]=useState(0),[error,setError]=useState("");
  const end=async()=>{
    const current=session.current;
    if(!current)return;
    current.stopped=true;
    if(current.timer!==null)clearTimeout(current.timer);
    session.current=null;
    current.queue=current.queue.catch(()=>{}).then(()=>command("end_display_preview",{session:current.owner}));
    retiring.current=current.queue.catch(()=>{});
    await current.queue;
  };
  const flush=async()=>{await session.current?.queue;};
  useEffect(()=>{
    if(!native||!active||baseRevision!==state.settings.revision)return;
    const current:Session={owner:crypto.randomUUID(),sequence:0,queue:Promise.resolve(),stopped:false,timer:null};
    session.current=current;
    setError("");
    // A remount or rapid navigation must retire the previous owner first.
    current.queue=retiring.current.then(()=>{if(!current.stopped)return command("begin_display_preview",{session:current.owner,expectedRevision:baseRevision});});
    current.queue=current.queue.then(async()=>{
      if(current.stopped||latest.current.saving)return;
      const patch=patchFor(current,latest.current.draft,latest.current.freezePopup,latest.current.state);
      if(patch)await command("update_display_preview",{session:current.owner,sequence:++current.sequence,patch});
    }).catch(error=>{if(!current.stopped)setError(String(error));});
    const heartbeat=window.setInterval(()=>{
      current.queue=current.queue.catch(()=>{}).then(async()=>{
        if(!current.stopped)await command("update_display_preview",{session:current.owner,sequence:++current.sequence,patch:null});
      }).catch(error=>{if(!current.stopped)setError(String(error));});
    },2000);
    const stop=()=>{if(session.current===current)void end().catch(()=>{});};
    window.addEventListener("pagehide",stop);
    return ()=>{
      window.clearInterval(heartbeat);
      window.removeEventListener("pagehide",stop);
      current.stopped=true;
      if(current.timer!==null)clearTimeout(current.timer);
      if(session.current===current)void end().catch(()=>{});
    };
  },[active,baseRevision,state.settings.revision,cycle]);
  const signature=JSON.stringify([draft.themePreference,draft.widgetAppearance,draft.windows.snapGapLogicalPx,freezePopup,state.appearance.compactSizing,state.appearance.compactMonitorBudgets]);
  useEffect(()=>{
    const current=session.current;
    if(!current)return;
    if(current.timer!==null) {
      if(freezePopup&&!saving)return;
      clearTimeout(current.timer);
      current.timer=null;
    }
    if(saving)return;
    // Throttle continuous dragging, always reading the newest draft at dispatch.
    current.timer=setTimeout(()=>{
      current.timer=null;
      current.queue=current.queue.catch(()=>{}).then(async()=>{
        if(current.stopped||latest.current.saving)return;
        const patch=patchFor(current,latest.current.draft,latest.current.freezePopup,latest.current.state);
        if(patch)await command("update_display_preview",{session:current.owner,sequence:++current.sequence,patch});
      }).catch(error=>{if(!current.stopped)setError(String(error));});
    },freezePopup?60:0);
  },[signature,saving,active,baseRevision,cycle]);
  return {end,flush,restart:()=>setCycle(c=>c+1),error,pending:native&&active&&state.appearance.themePreference!==draft.themePreference};
}
