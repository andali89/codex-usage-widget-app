import type { AppState } from "./state";
import { quotaPending, usageSummary } from "./summary";
import { showsPeriod, effectiveResets } from "./periods";
import CompactQuotaReadings from "./CompactQuotaReadings";
import QuotaCard from "./QuotaCard";
import { BrandMark, Icon } from "./visuals";

export default function WidgetContents({state,surface,now,onAction=()=>{}}:{state:AppState;surface:"shell"|"preview";now:number;onAction?:(request:Record<string,unknown>)=>void}) {
  const sourceUsage = state.usage, mode = state.settings.mode;
  const selection = (mode === "screen-edge" || mode === "compact-hud") ? state.settings.windows.visiblePeriods?.[mode] ?? "both" : "both";
  const filtered = surface === "shell" && (mode === "screen-edge" || mode === "compact-hud");
  const u = filtered ? {...sourceUsage, windows:sourceUsage.windows.filter(w => showsPeriod(selection,w.durationMinutes))} : sourceUsage;
  const summary = usageSummary(u, now);
  const retained = u.freshness === "stale" || u.windows.some(w => w.lastKnown);
  const incomplete = u.windows.some(w => w.quality !== "valid" || w.remainingPercent === null);
  const status = u.ordinaryUsageAllowed === false ? `${retained ? "Last known: " : ""}Ordinary usage blocked` : retained ? "Last known · out of date" : u.refreshInFlight ? "Checking usage…" : incomplete ? u.windows.some(w => quotaPending(u, w.key)) ? "Confirming quota data…" : "Some quota data unavailable" : u.windows.some(w => w.remainingPercent !== null) ? "Latest local read" : summary.first;
  const warning = retained || incomplete || u.ordinaryUsageAllowed === false || !u.windows.some(w => w.remainingPercent !== null);
  const savedResets = state.settings.windows.resetCountdowns[mode];
  const resets = filtered ? effectiveResets(selection,savedResets) : savedResets;
  const trayPreview = surface === "preview" && state.presentation.previewSource === "tray";
  const labelStyle = state.settings.windows.quotaLabelStyle;
  const activationLabel = `${summary.first}. ${status}. ${state.settings.windows.lockPosition ? "Position locked." : "Drag to move."} Click for Overview; double-click for the app window.`;
  const settingsButton = <button type="button" data-widget-control className="icon-button" aria-label="Widget settings" title="Settings" onClick={() => onAction({ type: "settings" })}><Icon name="settings" /></button>;
  const credits = u.resetCreditsAvailable !== null && <div className="widget-credits"><Icon name="credits" /><span>{retained ? "Last known credits" : "Reset credits"}</span><strong>{u.resetCreditsAvailable}</strong><Icon name="chevron" /></div>;
  const warningMark = warning && <span className="status-warning" title={status} aria-label={status}><Icon name="warning" /></span>;
  return <>
    {surface === "shell" ? mode === "floating" ? <>
      <header className="widget-header"><BrandMark /><strong>Codex</strong><span className="spacer" />{settingsButton}<button type="button" data-widget-control className="icon-button" disabled={!state.trayAvailable} title={state.trayAvailable ? "Switch to System Tray mode" : "System tray unavailable"} aria-label="Switch to System Tray mode" onClick={() => onAction({ type: "close-to-tray" })}><Icon name="close" /></button></header>
      <div className="glance-target floating-content" role="button" tabIndex={0} aria-label={activationLabel}>
        <div className="widget-quotas">{u.windows.length ? u.windows.map((quota, index) => <QuotaCard key={quota.key} quota={quota} index={index} compact stale={u.freshness === "stale"} now={now} pending={quotaPending(u, quota.key)} showReset={quota.durationMinutes === 300 ? !!resets?.fiveHour : quota.durationMinutes === 10080 ? !!resets?.weekly : false} />) : <div className="widget-empty"><Icon name="activity" /><span>{summary.first}</span></div>}</div>
        {credits}<p className="widget-status"><span className={`status-dot ${warning ? "warning" : "healthy"}`} />{status}</p>
      </div>
    </> : mode === "screen-edge" ? <>
      <BrandMark small /><div className="glance-target edge-values" role="button" tabIndex={0} aria-label={activationLabel}>
        <CompactQuotaReadings mode="screen-edge" segmentCounts={state.settings.windows.resetSegmentCounts?.["screen-edge"]} windows={u.windows} selection={selection} resets={savedResets} labelStyle={labelStyle} retained={retained} now={now}/>
      </div>{warningMark}
    </> : <>
      <BrandMark small /><div className="glance-target hud-values" role="button" tabIndex={0} aria-label={activationLabel}>
        <CompactQuotaReadings mode="compact-hud" segmentCounts={state.settings.windows.resetSegmentCounts?.["compact-hud"]} windows={u.windows} selection={selection} resets={savedResets} labelStyle={labelStyle} retained={retained} now={now}/>
      </div>{warningMark}
    </> : trayPreview ? <><BrandMark small /><div className="tray-summary-content"><p>{summary.first}</p>{summary.second && <p className="muted">{summary.second}</p>}</div></> : <>
      <header className="widget-header"><BrandMark /><strong>Codex <span className="muted">usage</span></strong></header>
      {u.ordinaryUsageAllowed === false && <p className="notice critical">{retained ? "Last known: " : ""}Ordinary usage blocked</p>}
      <div className="preview-quotas">{u.windows.length ? u.windows.map((quota, index) => <QuotaCard key={quota.key} quota={quota} index={index} compact showDetails stale={u.freshness === "stale"} now={now} pending={quotaPending(u, quota.key)} />) : <p className="widget-empty">{summary.first}</p>}</div>
      {credits}<footer className="preview-footer"><span className="widget-status"><span className={`status-dot ${warning ? "warning" : "healthy"}`} />{status}</span><button className="text-button" onClick={() => onAction({ type: "details" })}>Details <Icon name="chevron" /></button></footer>
    </>}
  </>;
}
