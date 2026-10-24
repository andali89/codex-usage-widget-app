import { command, diagnostic, type StorageLocation, type StorageStatus } from "./state";
import { useState } from "react";

export default function StorageSettings({value,status,onChange}:{value:StorageLocation;status:StorageStatus;onChange:(value:StorageLocation)=>void}) {
  const [error,setError] = useState("");
  const choose = async () => {
    try { const directory = await command<string|null>("choose_storage_folder"); if(directory)onChange({kind:"custom",directory}); setError(""); }
    catch { setError(diagnostic("storage-folder-dialog")); }
  };
  return <fieldset><legend>Configuration location</legend>
    <label className="field-label">Preferred location<select value={value.kind} onChange={event=>onChange(event.target.value === "custom" ? {kind:"custom",directory:status.activeDirectory} : {kind:event.target.value as "executable"|"app-data"})}>
      <option value="executable">Beside widget EXE (default)</option><option value="app-data">AppData</option><option value="custom">Custom folder</option>
    </select></label>
    {value.kind === "custom" && <><label className="field-label" htmlFor="storage-directory">Custom folder</label><input id="storage-directory" type="text" autoComplete="off" spellCheck={false} value={value.directory} onChange={event=>onChange({kind:"custom",directory:event.target.value})}/><button type="button" onClick={()=>void choose()}>Choose folder…</button></>}
    <p className="help">Settings and reminder history stay together. If the preferred folder is unavailable, the app tries the widget EXE folder, then AppData.</p>
    <p className="help">Active folder: <span className="storage-path">{status.activeDirectory}</span>{status.fallback && " · Using fallback"}</p>
    {!status.portableLocator && <p className="help">The location record is stored in AppData because it could not be written beside the widget EXE.</p>}
    {status.diagnostic && <p className="notice warning" role="status">{diagnostic(status.diagnostic)}</p>}
    {error && <p className="notice warning" role="alert">{error}</p>}
  </fieldset>;
}
