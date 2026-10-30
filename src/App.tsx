import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { applyAppearance, command, diagnostic, native, useSharedState } from "./state";
import type { AppState, Settings } from "./state";
import DiagnosticSettings from "./DiagnosticSettings";
import { useDiagnostics } from "./useDiagnostics";
import StorageSettings from "./StorageSettings";
import PeriodSettings from "./PeriodSettings";
import { useDisplayPreview } from "./useDisplayPreview";
import WidgetAppearanceSettings, { SnapGap, widgetDraftIssues } from "./WidgetAppearanceSettings";
import { summarizeAppearanceIssues, type AppearanceIssue } from "./widgetAppearance";
import ReminderSettings from "./ReminderSettings";
import type { RefObject } from "react";
import { BrandMark, Icon, ModePreview } from "./visuals";
import UsagePage from "./UsagePage";

const modes = [
  ["floating", "Floating", "A movable desktop card"],
  ["screen-edge", "Screen Edge", "An anchored tab with a temporary preview"],
  ["system-tray", "System Tray", "Quiet monitoring in the notification area"],
  ["compact-hud", "Compact HUD", "A thin quota strip"],
] as const;

type SettingsSection = "display" | "reminders" | "general" | "connection";

interface DraftController { dirty: boolean; save: () => Promise<boolean>; discard: () => Promise<boolean> }

function SettingsPage({ state, controller, section, setSection, active }: { state: AppState; controller: RefObject<DraftController | null>; active:boolean; section: SettingsSection; setSection: (section: SettingsSection) => void }) {
  const [draft, setDraft] = useState<Settings>(() => structuredClone(state.settings));
  const [baseRevision, setBaseRevision] = useState(state.settings.revision);
  const baseline = useRef(structuredClone(state.settings));
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  const [storageConflict, setStorageConflict] = useState<{token:string}|null>(null);
  const storageDialog = useRef<HTMLDialogElement>(null);
  useEffect(()=>{if(storageConflict)storageDialog.current?.showModal();else storageDialog.current?.close();},[storageConflict]);
  const conflict = state.settings.revision !== baseRevision;
  const [gapDragging,setGapDragging]=useState(false);
  const preview=useDisplayPreview(state,draft,baseRevision,active&&state.dashboardVisible&&!conflict,saving,gapDragging);
  const change = <K extends keyof Settings>(key: K, value: Settings[K]) => { setDraft(s => ({ ...s, [key]: value })); setMessage(""); };
  const save = async (storageResolution?: "copy-current"|"use-destination", confirmedToken?:string) => {
    const issues=widgetDraftIssues(draft,state); if(issues.length) { setMessage(summarizeAppearanceIssues(issues)); return false; }
    setSaving(true); setMessage("");
    try {
      await preview.flush();
      let storageToken:string|undefined;
      if(JSON.stringify(draft.storageLocation)!==JSON.stringify(state.settings.storageLocation)) {
        const inspection = await command<{token:string;conflict:boolean}>("inspect_storage_location",{location:draft.storageLocation});
        if(confirmedToken && confirmedToken!==inspection.token) {setStorageConflict(null);throw "storage-destination-changed";}
        if(inspection.conflict && !storageResolution) {setStorageConflict(inspection);return false;}
        storageToken = inspection.token;
      }
      const result = await command<AppState>("save_settings", { expectedRevision: baseRevision, settings: draft, storageResolution:storageResolution??null,storageToken:storageToken??null });
      setDraft(structuredClone(result.settings)); baseline.current = structuredClone(result.settings); setBaseRevision(result.settings.revision); setMessage(result.windowDiagnostic ? "Settings saved; display update failed. " + result.windowDiagnostic : "Settings saved.");
      return true;
    } catch (error) { let parsed:unknown=error; try { if(typeof error==="string" && error.startsWith("{")) parsed=JSON.parse(error); } catch { /* ordinary diagnostic */ } const failure = typeof parsed==="object" && parsed && "issues" in parsed ? summarizeAppearanceIssues((parsed as {issues:AppearanceIssue[]}).issues) : diagnostic(String(error)); setMessage(failure + " Display edits remain temporary until saved."); return false; }
    finally { setSaving(false); }
  };
  const discard = async () => { try { await preview.end(); } catch {setMessage("Cannot restore the saved display yet. Please try Cancel again.");return false;} setGapDragging(false); setDraft(structuredClone(state.settings)); baseline.current = structuredClone(state.settings); setBaseRevision(state.settings.revision); setMessage("Draft discarded."); preview.restart(); return true; };
  useEffect(() => {
    if (!saving && baseRevision !== state.settings.revision && JSON.stringify(draft) === JSON.stringify(baseline.current)) {
      baseline.current = structuredClone(state.settings); setDraft(structuredClone(state.settings)); setBaseRevision(state.settings.revision);
    }
  }, [state.settings, baseRevision, draft, saving]);
  useLayoutEffect(() => {
    controller.current = { dirty: JSON.stringify(draft) !== JSON.stringify(baseline.current), save, discard };
  });
  return <form className="settings-form" noValidate onSubmit={e => { e.preventDefault(); void save(); }}>
    <div className="page-intro short"><p className="eyebrow">MAKE IT YOURS</p><h1 tabIndex={-1}>Settings</h1><p className="muted">One set of preferences for every view.</p></div>
    {conflict && <div className="notice warning" role="alert"><p>A newer settings revision is saved. Your draft is retained.</p><details><summary>Compare saved values</summary><pre>{JSON.stringify(state.settings, null, 2)}</pre></details><div className="button-row"><button type="button" onClick={() => { void discard(); }}>Reload saved</button><button type="button" onClick={() => { baseline.current = structuredClone(state.settings); setBaseRevision(state.settings.revision); }}>Reapply this draft</button></div></div>}
    <nav className="tabs settings-tabs" aria-label="Settings pages">{([["display","Display"],["reminders","Reminders"],["general","General"],["connection","Connection & diagnostics"]] as const).map(([key,label])=><button type="button" key={key} aria-current={section===key?"page":undefined} onClick={()=>setSection(key)}>{label}</button>)}</nav><section hidden={section!=="display"}><fieldset><legend>Appearance</legend><div className="segmented">{([["system", "Follow Windows"], ["light", "Light"], ["dark", "Dark"]] as const).map(([value, label]) => <label key={value}><input type="radio" name="theme" value={value} checked={draft.themePreference === value} onChange={() => change("themePreference", value)} /><span>{label}</span></label>)}</div><p className="help">Try changes on the current widget. Save to keep them; Cancel restores saved settings. Windows high contrast takes precedence.</p></fieldset>
    <fieldset><legend>Display mode</legend><div className="mode-list">{modes.map(([value, label, description]) => <label key={value} className={value === draft.mode ? "selected" : ""}><input type="radio" name="mode" checked={draft.mode === value} onChange={() => change("mode", value)} /><ModePreview mode={value} /><span><strong>{label}</strong><small>{description}{value === "system-tray" ? " · Default" : ""}</small></span></label>)}</div><p className="help">The tray remains available in every mode. Edge and HUD show a temporary preview on hover; tray hover stays compact. Floating opens detail only on click. Click for Overview, double-click for the app window, or right-click for the tray menu.</p></fieldset>
    <fieldset><legend>Quota labels</legend><div className="segmented">{([["words", "Words"], ["badges", "Badges"]] as const).map(([value, label]) => <label key={value}><input type="radio" name="quota-labels" checked={draft.windows.quotaLabelStyle === value} onChange={() => change("windows", {...draft.windows, quotaLabelStyle:value})} /><span>{label}</span></label>)}</div><p className="help">Choose 5H / WEEK words or circular 5 / W badges for Screen Edge and Compact HUD. Changes apply when saved.</p></fieldset>
    <WidgetAppearanceSettings state={state} draft={draft} pending={preview.pending} onChange={next=>{setDraft(next);setMessage("");}} />
    <PeriodSettings value={draft.windows} onChange={windows=>change("windows",windows)} />
    <fieldset><legend>Desktop behavior</legend>{([["hoverEnabled","Show previews on hover"],["alwaysOnTop","Keep desktop widget on top"],["suppressFullscreen","Hide automatic previews and widgets over fullscreen apps"]] as const).map(([key,label]) => <label className="check-field" key={key}><input type="checkbox" checked={draft.windows[key]} onChange={event => change("windows",{...draft.windows,[key]:event.target.checked})} />{label}</label>)}<p className="help">Always on top is separate from hover behavior and does not override fullscreen suppression.</p></fieldset>
    <fieldset><legend>Placement</legend><label className="check-field"><input type="checkbox" checked={draft.windows.lockPosition} onChange={event=>change("windows",{...draft.windows,lockPosition:event.target.checked})}/>Lock widget position</label><SnapGap value={draft.windows.snapGapLogicalPx} onChange={snapGapLogicalPx=>change("windows",{...draft.windows,snapGapLogicalPx})} onDragging={setGapDragging}/>{draft.mode!=="system-tray"&&<><div className="placement-fields"><label>Monitor<select value={draft.windows.placements[draft.mode]?.monitor ?? ""} onChange={event => change("windows",{...draft.windows,placements:{...draft.windows.placements,[draft.mode]:{x:24,y:120,...draft.windows.placements[draft.mode],monitor:event.target.value || null}}})}><option value="">Primary monitor</option>{state.monitors.map(m => <option value={m.name} key={m.name}>{m.name} · {m.width} × {m.height} · {Math.round(m.scale*100)}%</option>)}</select></label>{(["x","y"] as const).map(key => <label key={key}>{key === "x" ? "Horizontal offset" : "Vertical offset"} (logical px)<input type="number" min={-100000} max={100000} disabled={draft.mode === "screen-edge" && key === "x"} value={draft.windows.placements[draft.mode]?.[key] ?? 24} onChange={event => change("windows",{...draft.windows,placements:{...draft.windows.placements,[draft.mode]:{monitor:null,x:24,y:24,...draft.windows.placements[draft.mode],[key]:Number(event.target.value)}}})} /></label>)}</div>{draft.mode === "screen-edge" && <label className="field-label">Screen edge<select value={draft.windows.edgeSide} onChange={event => change("windows",{...draft.windows,edgeSide:event.target.value as "left"|"right"})}><option value="left">Left</option><option value="right">Right</option></select></label>}<p className="help">Drag anywhere on Floating or HUD to move them; drag Edge anywhere to dock it to the nearest left or right edge on release. Positions are clamped to the selected monitor's work area, and a missing monitor falls back to the primary display.</p></>}</fieldset>
    </section><section hidden={section!=="reminders"}><ReminderSettings value={draft.reminders} onChange={reminders=>change("reminders",reminders)} state={state}/></section><section hidden={section!=="connection"}><fieldset><legend>Local Codex connection</legend><label className="field-label" htmlFor="codex-path">Native executable path <span className="muted">(optional)</span></label><input id="codex-path" type="text" autoComplete="off" spellCheck={false} value={draft.codexPath ?? ""} onChange={e => change("codexPath", e.target.value || null)} placeholder="Automatic discovery of codex.exe" /><p className="help">Uses the local Codex app-server to check compatibility, without restricting CLI versions. Enter the full executable path, without arguments. Authentication stays with Codex.</p><label className="field-label" htmlFor="proxy-url">HTTP/HTTPS proxy <span className="muted">(optional)</span></label><input id="proxy-url" type="text" autoComplete="off" spellCheck={false} value={draft.proxyUrl ?? ""} onChange={event=>change("proxyUrl",event.target.value.trim() || null)} placeholder="Example: http://127.0.0.1:7890"/><p className="help">Used for this widget’s Codex connection. HTTP and HTTPS proxies without a username/password are supported. Leave blank to use existing network settings. Saving retries the connection.</p></fieldset><DiagnosticSettings saved={state.settings.diagnosticLoggingEnabled} value={draft.diagnosticLoggingEnabled} onChange={value=>change("diagnosticLoggingEnabled",value)} active={active&&section==="connection"}/>
    </section><section hidden={section!=="general"}><StorageSettings value={draft.storageLocation} status={state.storage} onChange={location=>change("storageLocation",location)}/><fieldset><legend>Refresh cadence</legend><div className="interval-grid">{([["foregroundSeconds", "Detail", 15, 120], ["compactSeconds", "Compact", 30, 300], ["backgroundSeconds", "Background", 60, 600]] as const).map(([key, label, min, max]) => <label key={key}><span>{label} (seconds)</span><input type="number" min={min} max={max} step="1" required value={draft[key]} onChange={e => change(key, Number(e.target.value))} /></label>)}</div><p className="help">Effective interval: {state.effectiveRefreshSeconds} seconds, with cadence jitter. Detection can lag by that interval plus request latency. Battery saver doubles intervals up to 600 seconds.</p></fieldset>
    <fieldset><legend>Launch at sign-in</legend><label className="check-field"><input type="checkbox" checked={draft.autostart.requested} onChange={e=>change("autostart",{...draft.autostart,requested:e.target.checked})}/>Start after I sign in to Windows</label><p className="help">Off by default. Login launches stay in the tray and restore only your selected compact mode. Manual launches open the dashboard.</p><p role="status">Saved request: {state.settings.autostart.requested?"On":"Off"} · Observed in Windows: {state.settings.autostart.effective===null?"Unknown":state.settings.autostart.effective?"On":"Off"}{state.settings.autostart.pending?" · Change pending":""}</p>{state.settings.autostart.diagnostic && <p className="notice">{diagnostic(state.settings.autostart.diagnostic)}</p>}<button type="button" disabled={JSON.stringify(draft) !== JSON.stringify(baseline.current) || saving} onClick={()=>{void command<AppState>("retry_autostart").then(()=>setMessage("Sign-in registration rechecked. Review its observed status.")).catch(error=>setMessage(diagnostic(String(error))));}}>Reapply saved sign-in preference</button></fieldset>
    <section className="notice"><h2>Tray and reminder availability</h2><p>{state.trayAvailable ? "Closing this window keeps monitoring in the tray. Find the icon in the notification area or its overflow menu; choose Quit to stop." : "The tray icon could not be created. Keep this dashboard open to retain access."}</p><p>{state.tooltipFallback ? "Hover uses the Windows native tooltip because the custom preview adapter is unavailable. Windows controls its styling and timing." : "Tray hover uses a compact read-only preview. Click the icon for full detail."}</p></section></section>
    <dialog className="draft-dialog storage-conflict-dialog" aria-labelledby="storage-conflict-title" onKeyDown={event=>event.stopPropagation()} ref={storageDialog} onCancel={()=>setStorageConflict(null)}><h2 id="storage-conflict-title">Configuration already exists</h2><p>Use that folder’s settings and reminder history, or copy your current settings there? Copying preserves the destination files as dated backups. Loading replaces this settings draft.</p><div className="button-row"><button type="button" disabled={saving} onClick={()=>setStorageConflict(null)}>Cancel</button><button type="button" disabled={saving} onClick={()=>{const token=storageConflict?.token;setStorageConflict(null);void save("use-destination",token);}}>Use destination configuration</button><button type="button" disabled={saving} onClick={()=>{const token=storageConflict?.token;setStorageConflict(null);void save("copy-current",token);}}>Copy current configuration</button></div></dialog>
    <div className="save-bar"><span role="status">{message || (preview.error?"Temporary preview unavailable. Your draft is retained.":"Appearance and snap gap changes are temporary until saved.")}</span><div className="button-row"><button type="button" onClick={discard}>Cancel</button><button className="primary" type="submit" disabled={saving || conflict || widgetDraftIssues(draft,state).length>0}>{saving ? "Saving…" : "Save settings"}</button></div></div>
  </form>;
}

export default function App() {
  const { state, error } = useSharedState();
  useDiagnostics(state,"main",undefined,state?.dashboardVisible??true);
  const [page, setPage] = useState<"usage" | "settings">("usage");
  const [actionError, setActionError] = useState("");
  const ready = useRef("");
  const [contentLayoutRevision, setContentLayoutRevision] = useState(0);
  const contentLayoutChanged = useCallback(() => setContentLayoutRevision(value => value + 1), []);
  const [settingsSection, setSettingsSection] = useState<SettingsSection>("display");
  const dashboardRequest=useRef<number|null>(null);
  const focusedDashboardRequest=useRef<number|null>(null);
  const hydrationPage = useRef<"usage" | "settings" | null>(null);
  const main = useRef<HTMLElement>(null);
  const draft = useRef<DraftController | null>(null);
  const [settingsVisited, setSettingsVisited] = useState(false);
  const [pending, setPending] = useState<"usage" | "hide_dashboard" | "quit" | null>(null);
  const [dialogBusy, setDialogBusy] = useState(false);
  const modal = useRef<HTMLDivElement>(null);
  const initiatingControl = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    if (!state) return;
    applyAppearance(state);
    if(state.dashboardRequest!==null && state.dashboardRequest.sequence!==dashboardRequest.current) {
      dashboardRequest.current=state.dashboardRequest.sequence;
      const nextPage=state.dashboardRequest.page === "settings" ? "settings" : "usage";
      hydrationPage.current = nextPage;
      if(nextPage === "settings") setSettingsVisited(true);
      if(nextPage !== page) setPage(nextPage);
      if(nextPage !== page) return;
    }
    if (hydrationPage.current === page) hydrationPage.current = null;
  }, [state,page,settingsVisited]);
  useLayoutEffect(() => {
    const request=state?.dashboardRequest;
    if(!request||!state.dashboardVisible||state.presentation.detail==="closed"||page!==(request.page==="settings"?"settings":"usage"))return;
    let frame=0;
    const focusHeading=()=>{
      if(!document.hasFocus()||focusedDashboardRequest.current===request.sequence)return;
      const heading=main.current?.querySelector<HTMLElement>(page==="settings"?"[data-settings] h1":"[data-overview-heading]");
      if(heading){heading.focus({preventScroll:true});focusedDashboardRequest.current=request.sequence;}
    };
    const schedule=()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(focusHeading);};
    // Native coordination owns window activation. Focus the heading only after
    // Windows has actually focused this document, including a later user click.
    schedule();window.addEventListener("focus",schedule);
    return ()=>{cancelAnimationFrame(frame);window.removeEventListener("focus",schedule);};
  },[state?.dashboardRequest?.sequence,state?.dashboardVisible,state?.presentation.detail,page]);
  useLayoutEffect(() => { main.current?.scrollTo({ top: 0 }); }, [page, state?.dashboardRequest?.sequence]);
  useLayoutEffect(() => {
    if (!state || state.presentation.detail === "closed" || !main.current || (hydrationPage.current !== null && hydrationPage.current !== page)) return;
    const content = main.current.querySelector<HTMLElement>(".content-body");
    const frame = main.current.closest<HTMLElement>(".app-frame");
    if (!content || !frame) return;
    const measure = (fit = false) => {
      const viewport = main.current;
      if (!viewport) return;
      const style = getComputedStyle(viewport);
      const chrome = Array.from(frame.children).filter(node => node.matches(".app-header,.tabs,.app-footer")).reduce((sum, node) => sum + node.getBoundingClientRect().height, 0);
      const width = Math.ceil(frame.getBoundingClientRect().width);
      const height = Math.ceil(content.getBoundingClientRect().height + parseFloat(style.paddingTop) + parseFloat(style.paddingBottom) + chrome);
      const key = [state.presentation.generation, page, width, height, fit].join("/");
      if (ready.current === key) return;
      ready.current = key;
      void command("view_ready", { generation: state.presentation.generation, page: page === "usage" ? "overview" : "settings", width, height, fit }).catch(() => { ready.current = ""; });
    };
    measure(true);
    let pendingFrame = 0;
    const observer = new ResizeObserver(() => { cancelAnimationFrame(pendingFrame); pendingFrame = requestAnimationFrame(() => measure()); });
    observer.observe(content); observer.observe(frame);
    return () => { observer.disconnect(); cancelAnimationFrame(pendingFrame); };
  }, [state?.presentation.generation, page, contentLayoutRevision, state?.dashboardRequest?.sequence, state?.usage.accountContextKey, state?.appearance.textScale, state?.presentation.detail,
    state?.usage.lastSuccessAtMs, state?.usage.connection, state?.usage.freshness, state?.usage.refreshInFlight,
    state?.usage.errorCode, state?.usage.codexCliVersion, state?.usage.resetCreditsAvailable, state?.usage.resetCredits?.length,
    state?.reminders.diagnostic, state?.reminders.deliveryPaused, state?.settings.reminders.enabled, state?.reminders.snoozedUntil]);
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const removers: (() => void)[] = [];
    for (const [event, handler] of [
      ["open-settings", () => { setSettingsVisited(true); setPage("settings"); }],
      ["request-close", () => requestAction("hide_dashboard")],
      ["request-quit", () => requestAction("quit")],
    ] as const) {
      void listen(event, handler).then(fn => { if (disposed) fn(); else removers.push(fn); });
    }
    return () => { disposed = true; removers.forEach(fn => fn()); };
  }, []);
  useEffect(() => { if (pending) modal.current?.querySelector<HTMLButtonElement>("button")?.focus(); }, [pending]);
  const completeAction = (next: "usage" | "hide_dashboard" | "quit") => {
    if (next === "usage") { setPage("usage"); requestAnimationFrame(() => main.current?.querySelector<HTMLElement>("h1")?.focus({ preventScroll: true })); }
    else void act(next);
  };
  const requestAction = (next: "usage" | "hide_dashboard" | "quit") => {
    if (draft.current?.dirty) { initiatingControl.current = document.activeElement as HTMLElement | null; setPending(next); }
    else completeAction(next);
  };
  const cancelDialog = () => { setPending(null); initiatingControl.current?.focus(); };
  const resolveDraft = async (save: boolean) => {
    if (!pending || dialogBusy) return;
    setDialogBusy(true);
    const next = pending;
    if (save && !await draft.current?.save()) { setDialogBusy(false); setPending(null); setPage("settings"); return; }
    if (!save && !await draft.current?.discard()) { setDialogBusy(false); setPending(null); setPage("settings"); return; }
    setPending(null); setDialogBusy(false); completeAction(next);
  };
  const navigate = (next: "usage" | "settings", section?: SettingsSection) => {
    if (section) setSettingsSection(section);
    if (next === "usage") requestAction("usage");
    else { setSettingsVisited(true); setPage(next); requestAnimationFrame(() => { if(document.hasFocus()) main.current?.querySelector<HTMLElement>("[data-settings] h1")?.focus({ preventScroll: true }); }); }
  };
  const act = async (name: string) => { try { await command(name); setActionError(""); } catch { setActionError("The native service did not respond. Reopen from the tray and try again."); } };
  if (!native) return <main className="browser-message"><BrandMark /><h1>Codex Usage Widget</h1><p>This browser page has no connection to local usage.</p><p>Run the desktop application to monitor Codex. No sample quota values are substituted.</p></main>;
  return <div className={`app-frame detail-${state?.presentation.detail ?? "window"}`} onKeyDown={event => { if (event.key === "Escape" && !pending && state?.presentation.detail === "popup") { event.preventDefault(); void command("surface_action", { request: { type: "escape" } }); } }}>
    <header className="app-header"><div className="brand"><BrandMark /><span>Codex <small>USAGE WIDGET</small></span></div><div className="button-row"><button disabled={!state || state.usage.refreshInFlight} onClick={() => void act("refresh")} aria-label="Refresh Codex usage"><Icon name="refresh" className={state?.usage.refreshInFlight ? "refreshing" : ""} /><span>Refresh</span></button><button className="icon-button" aria-label="Hide dashboard to tray" title="Hide to tray" disabled={!state?.trayAvailable} onClick={() => requestAction("hide_dashboard")}><Icon name="close" /></button></div></header>
    <nav className="tabs" aria-label="Dashboard"><button aria-current={page === "usage" ? "page" : undefined} aria-label="Overview" title="Overview" onClick={() => navigate("usage")}><Icon name="activity" /><span className="nav-label">Overview</span></button><button aria-current={page === "settings" ? "page" : undefined} aria-label="Settings" title="Settings" onClick={() => navigate("settings")}><Icon name="settings" /><span className="nav-label">Settings</span></button><span className="spacer" />{state?.presentation.detail === "popup" && <button onClick={() => void command("surface_action", { request: { type: "promote" } })} aria-label="Open in window" title="Open in window"><Icon name="expand" /><span className="expand-label">Open in window</span></button>}</nav>
    <main className="content" ref={main}><div className="content-body">{(error || actionError) && <div className="notice critical" role="alert">{error || actionError}</div>}{state?.windowDiagnostic && <div className="notice warning" role="status">Window coordination needs attention: {state.windowDiagnostic}. The tray remains the recovery route.</div>}{state?.storage.fallback && <div className="notice warning" role="status">Configuration fallback: <span className="storage-path">{state.storage.activeDirectory}</span>. Your preferred location is retained.</div>}{state?.storageDiagnostic && <div className="notice warning" role="alert">{diagnostic(state.storageDiagnostic)}</div>}{state ? <>{page === "usage" && <UsagePage state={state} onLayoutChange={contentLayoutChanged} onSettings={() => navigate("settings")} onReminders={() => navigate("settings", "reminders")} />}{settingsVisited && <section data-settings hidden={page !== "settings"}><SettingsPage state={state} controller={draft} active={page==="settings"} section={settingsSection} setSection={setSettingsSection} /></section>}</> : <div className="empty-state"><h1>Connecting…</h1><p>Loading the shared application state.</p></div>}</div></main>
    <footer className="app-footer"><span><span className="status-dot healthy" /><span className="footer-status-long">Monitoring continues in the tray</span><span className="footer-status-short">Monitoring active</span></span><button className="text-button" onClick={() => requestAction("quit")}>Quit widget</button></footer>
    {pending && <div className="modal-backdrop"><div ref={modal} role="dialog" aria-modal="true" aria-labelledby="draft-title" className="draft-dialog" onKeyDown={event => {
      if (event.key === "Escape" && !dialogBusy) { event.preventDefault(); cancelDialog(); }
      if (event.key === "Tab") { const buttons = modal.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"); if (buttons?.length) { const first = buttons[0], last = buttons[buttons.length - 1]; if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); } else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); } } }
    }}><h2 id="draft-title">Save your settings changes?</h2><p>Your draft has not been saved.</p><div className="button-row"><button className="primary" disabled={dialogBusy} onClick={() => void resolveDraft(true)}>Save</button><button disabled={dialogBusy} onClick={() => void resolveDraft(false)}>Discard</button><button disabled={dialogBusy} onClick={cancelDialog}>Cancel</button></div></div></div>}
  </div>;
}
