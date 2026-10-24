import { useLayoutEffect, useRef, type CSSProperties, type MouseEvent, type KeyboardEvent, type PointerEvent } from "react";
import { applyAppearance, command, useSharedState, useVisibleClock } from "./state";
import { quotaPending, usageSummary } from "./summary";
import CompactQuotaReadings from "./CompactQuotaReadings";
import { showsPeriod, effectiveResets } from "./periods";
import { compactLayout } from "./compactLayout";
import { shouldActivateWidgetClick, shouldStartWidgetDrag } from "./widgetGestures";
import { BrandMark, Icon } from "./visuals";
import QuotaCard from "./QuotaCard";

export const surfaceAction = (request: Record<string, unknown>) => command("surface_action", { request });
const isControl = (target: EventTarget | null) => target instanceof Element && !!target.closest("[data-widget-control]");

export default function SurfaceView({ surface }: { surface: "shell" | "preview" }) {
  const { state, error } = useSharedState();
  const visible = !!state && (surface === "shell" ? !state.presentation.suppressed && !state.presentation.hidden && state.settings.mode !== "system-tray" : !(state.presentation.previewSource === "tray" ? state.presentation.traySuppressed : state.presentation.suppressed) && state.presentation.shell !== "compact");
  const now = useVisibleClock(visible);
  const root = useRef<HTMLDivElement>(null);
  const dragOrigin = useRef<{ pointerId: number; x: number; y: number } | null>(null);
  const dragging = useRef(false), suppressClick = useRef(false), lastMeasurement = useRef("");
  useLayoutEffect(() => {
    if (!state || !root.current) return;
    applyAppearance(state);
    document.body.dataset.surface = surface;
    document.documentElement.dataset.mode = state.settings.mode;
    const measure = () => {
      if (!root.current) return;
      const rect = root.current.getBoundingClientRect();
      const fixed = root.current.classList.contains("stable-compact");
      const width = Math.ceil(fixed ? rect.width : Math.max(rect.width, root.current.scrollWidth)), height = Math.ceil(fixed ? rect.height : Math.max(rect.height, root.current.scrollHeight));
      const key = `${state.presentation.generation}/${width}/${height}`;
      if (lastMeasurement.current === key) return;
      lastMeasurement.current = key;
      void surfaceAction({ type: "ready", generation: state.presentation.generation, width, height }).catch(() => { lastMeasurement.current = ""; });
    };
    measure();
    const observer = new ResizeObserver(measure); observer.observe(root.current);
    return () => observer.disconnect();
  }, [surface, state?.presentation.generation, state?.presentation.previewSource, state?.settings.mode, state?.appearance.resolvedTheme, state?.appearance.highContrast, state?.appearance.reducedMotion, state?.appearance.textScale]);
  if (!state) return <div className="native-loading">{error ?? "Loading usage…"}</div>;
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
  const layout = surface === "shell" ? compactLayout(mode, labelStyle, resets, warning, selection) : null;
  const layoutStyle = layout ? { "--compact-width": `${layout.width / 16}rem`, "--compact-height": `${layout.height / 16}rem` } as CSSProperties : undefined;
  const contextMenu = (event: MouseEvent<HTMLDivElement>) => { event.preventDefault(); void surfaceAction({ type: "context-menu", x: event.clientX, y: event.clientY }); };
  const pointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (surface !== "shell" || event.button !== 0 || state.settings.windows.lockPosition || isControl(event.target)) return;
    dragOrigin.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY };
    dragging.current = false; suppressClick.current = false; event.currentTarget.setPointerCapture(event.pointerId);
  };
  const pointerEnd = (event: PointerEvent<HTMLDivElement>) => {
    if (dragOrigin.current?.pointerId === event.pointerId) dragOrigin.current = null;
    window.setTimeout(() => { suppressClick.current = false; }, 0);
  };
  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") { event.preventDefault(); void surfaceAction({ type: "escape" }); }
    if (surface !== "shell" || isControl(event.target)) return;
    if (event.key === "Enter" || event.key === " ") { event.preventDefault(); void surfaceAction({ type: "details" }); }
    const delta = ({ ArrowLeft: [-10, 0], ArrowRight: [10, 0], ArrowUp: [0, -10], ArrowDown: [0, 10] } as Record<string, number[]>)[event.key];
    if (delta && !state.settings.windows.lockPosition) { event.preventDefault(); void surfaceAction({ type: "nudge", dx: delta[0], dy: delta[1] }); }
  };
  const activationLabel = `${summary.first}. ${status}. ${state.settings.windows.lockPosition ? "Position locked." : "Drag to move."} Click for Overview; double-click for the app window.`;
  const settingsButton = <button type="button" data-widget-control className="icon-button" aria-label="Widget settings" title="Settings" onClick={() => void surfaceAction({ type: "settings" })}><Icon name="settings" /></button>;
  const credits = u.resetCreditsAvailable !== null && <div className="widget-credits"><Icon name="credits" /><span>{retained ? "Last known credits" : "Reset credits"}</span><strong>{u.resetCreditsAvailable}</strong><Icon name="chevron" /></div>;
  const warningMark = warning && <span className="status-warning" title={status} aria-label={status}><Icon name="warning" /></span>;
  return <div ref={root} style={layoutStyle} data-label-style={labelStyle} className={`native-surface ${surface} mode-${mode}${layout ? " stable-compact" : ""}${trayPreview ? " tray-summary" : ""}${state.settings.windows.lockPosition ? " position-locked" : ""}`} onContextMenu={contextMenu} onKeyDown={keyDown}
    onPointerDown={pointerDown} onPointerUp={pointerEnd} onPointerCancel={pointerEnd}
    onPointerMove={event => { const origin = dragOrigin.current; if (origin && origin.pointerId === event.pointerId && shouldStartWidgetDrag(origin, { x: event.clientX, y: event.clientY }, state.settings.windows.lockPosition, dragging.current)) { dragging.current = true; suppressClick.current = true; void surfaceAction({ type: "drag" }); } }}
    onClick={event => { if (surface !== "shell" || isControl(event.target)) return; if (!shouldActivateWidgetClick(event.detail, suppressClick.current)) { event.preventDefault(); return; } void surfaceAction({ type: event.detail === 0 ? "details" : "activate" }); }}
    onDoubleClick={event => { if (surface === "shell" && !isControl(event.target) && !suppressClick.current) { event.preventDefault(); void surfaceAction({ type: "open-window" }); } }}>
    {surface === "shell" ? mode === "floating" ? <>
      <header className="widget-header"><BrandMark /><strong>Codex</strong><span className="spacer" />{settingsButton}<button type="button" data-widget-control className="icon-button" disabled={!state.trayAvailable} title={state.trayAvailable ? "Switch to System Tray mode" : "System tray unavailable"} aria-label="Switch to System Tray mode" onClick={() => void surfaceAction({ type: "close-to-tray" })}><Icon name="close" /></button></header>
      <div className="glance-target floating-content" role="button" tabIndex={0} aria-label={activationLabel}>
        <div className="widget-quotas">{u.windows.length ? u.windows.map((quota, index) => <QuotaCard key={quota.key} quota={quota} index={index} compact stale={u.freshness === "stale"} now={now} pending={quotaPending(u, quota.key)} showReset={quota.durationMinutes === 300 ? !!resets?.fiveHour : quota.durationMinutes === 10080 ? !!resets?.weekly : false} />) : <div className="widget-empty"><Icon name="activity" /><span>{summary.first}</span></div>}</div>
        {credits}<p className="widget-status"><span className={`status-dot ${warning ? "warning" : "healthy"}`} />{status}</p>
      </div>
    </> : mode === "screen-edge" ? <>
      <BrandMark small /><div className="glance-target edge-values" role="button" tabIndex={0} aria-label={activationLabel}>
        <CompactQuotaReadings mode="screen-edge" windows={u.windows} selection={selection} resets={savedResets} labelStyle={labelStyle} retained={retained} now={now}/>
      </div>{warningMark}
    </> : <>
      <BrandMark small /><div className="glance-target hud-values" role="button" tabIndex={0} aria-label={activationLabel}>
        <CompactQuotaReadings mode="compact-hud" windows={u.windows} selection={selection} resets={savedResets} labelStyle={labelStyle} retained={retained} now={now}/>
      </div>{warningMark}
    </> : trayPreview ? <><BrandMark small /><div className="tray-summary-content"><p>{summary.first}</p>{summary.second && <p className="muted">{summary.second}</p>}</div></> : <>
      <header className="widget-header"><BrandMark /><strong>Codex <span className="muted">usage</span></strong></header>
      {u.ordinaryUsageAllowed === false && <p className="notice critical">{retained ? "Last known: " : ""}Ordinary usage blocked</p>}
      <div className="preview-quotas">{u.windows.length ? u.windows.map((quota, index) => <QuotaCard key={quota.key} quota={quota} index={index} compact showDetails stale={u.freshness === "stale"} now={now} pending={quotaPending(u, quota.key)} />) : <p className="widget-empty">{summary.first}</p>}</div>
      {credits}<footer className="preview-footer"><span className="widget-status"><span className={`status-dot ${warning ? "warning" : "healthy"}`} />{status}</span><button className="text-button" onClick={() => void surfaceAction({ type: "details" })}>Details <Icon name="chevron" /></button></footer>
    </>}
  </div>;
}
