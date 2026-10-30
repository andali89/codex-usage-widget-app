import { useEffect, useState } from "react";
import { command, native } from "./state";
import type { DiagnosticStatus } from "./useDiagnostics";

export default function DiagnosticSettings({value,onChange,active,saved}:{value:boolean;onChange:(value:boolean)=>void;active:boolean;saved:boolean}) {
  const [status,setStatus]=useState<DiagnosticStatus|null>(null),[error,setError]=useState(false);
  useEffect(()=>{
    if(!active||!native)return;
    let disposed=false;
    const update=()=>{void command<DiagnosticStatus>("diagnostics_status").then(value=>{if(!disposed)setStatus(value);}).catch(()=>{if(!disposed)setError(true);});};
    update();const timer=saved?setInterval(update,5000):undefined;
    return()=>{disposed=true;clearInterval(timer);};
  },[active,saved]);
  return <fieldset><legend>Diagnostic logging</legend>
    <label className="check-field"><input type="checkbox" checked={value} onChange={event=>onChange(event.target.checked)}/>Enable local diagnostic logging</label>
    <p className="help">Off by default. Save to apply. Records loading, stale states, refresh timing and view activity locally. No credentials, account identifiers, quota values or raw responses are recorded.</p>
    {status&&<><p role="status">{status.error?`Recording suspended: ${status.error}. Save with logging enabled to retry.`:status.recording?"Recording":status.enabled?"Starting recording…":"Logging is off"}{status.dropped>0?` · ${status.dropped} events dropped`:""}</p><p className="storage-path">{status.directory}</p></>}
    <p className="help">Up to 5 MiB per configuration folder: one active file and four rotated files. Disabling keeps existing logs. Configuration moves leave old logs in place.</p>
    <button type="button" disabled={!native} onClick={()=>{void command("open_diagnostics_folder").then(()=>setError(false)).catch(()=>setError(true));}}>Open logs folder</button>
    {error&&<p role="alert">Diagnostic status or folder access is unavailable.</p>}
  </fieldset>;
}
