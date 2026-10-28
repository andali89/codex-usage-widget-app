import { useLayoutEffect, useRef, type CSSProperties, type MouseEvent, type KeyboardEvent, type PointerEvent } from "react";
import { applyAppearance, command, useSharedState, useVisibleClock } from "./state";
import WidgetBackground from "./WidgetBackground";
import WidgetContents from "./WidgetContents";
import { widgetStyle } from "./widgetAppearance";
import { showsPeriod, effectiveResets } from "./periods";
import { compactLayout } from "./compactLayout";
import { shouldActivateWidgetClick, shouldStartWidgetDrag } from "./widgetGestures";

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
  useLayoutEffect(() => { if(state) applyAppearance(state); }, [state?.appearance, state?.settings.widgetAppearance]);
  useLayoutEffect(() => {
    const element = root.current;
    if (!element || surface !== "shell" || (state?.settings.mode !== "screen-edge" && state?.settings.mode !== "compact-hud")) return;
    // Capture Tab before it moves focus into the widget from the document body.
    const showKeyboardFocus = () => { delete element.dataset.pointerFocus; };
    document.addEventListener("keydown", showKeyboardFocus, true);
    return () => {
      document.removeEventListener("keydown", showKeyboardFocus, true);
      delete element.dataset.pointerFocus;
    };
  }, [surface, state?.settings.mode]);
  if (!state) return <div className="native-loading">{error ?? "Loading usage…"}</div>;
  const sourceUsage = state.usage, mode = state.settings.mode;
  const selection = (mode === "screen-edge" || mode === "compact-hud") ? state.settings.windows.visiblePeriods?.[mode] ?? "both" : "both";
  const filtered = surface === "shell" && (mode === "screen-edge" || mode === "compact-hud");
  const u = filtered ? {...sourceUsage, windows:sourceUsage.windows.filter(w => showsPeriod(selection,w.durationMinutes))} : sourceUsage;
  const retained = u.freshness === "stale" || u.windows.some(w => w.lastKnown);
  const incomplete = u.windows.some(w => w.quality !== "valid" || w.remainingPercent === null);
  const warning = retained || incomplete || u.ordinaryUsageAllowed === false || !u.windows.some(w => w.remainingPercent !== null);
  const savedResets = state.settings.windows.resetCountdowns[mode];
  const resets = filtered ? effectiveResets(selection,savedResets) : savedResets;
  const trayPreview = surface === "preview" && state.presentation.previewSource === "tray";
  const labelStyle = state.settings.windows.quotaLabelStyle;
  const layout = surface === "shell" ? compactLayout(mode, labelStyle, resets, warning, selection) : null;
  const layoutStyle = layout ? { "--compact-width": `${layout.width / 16}rem`, "--compact-height": `${layout.height / 16}rem` } as CSSProperties : undefined;
  const contextMenu = (event: MouseEvent<HTMLDivElement>) => { event.preventDefault(); void surfaceAction({ type: "context-menu", x: event.clientX, y: event.clientY }); };
  const pointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (surface === "shell" && (mode === "screen-edge" || mode === "compact-hud")) event.currentTarget.dataset.pointerFocus = "true";
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
  const palettePaint=!state.appearance.highContrast && (!state.appearance.widget.originalStructure || surface==="shell" && state.appearance.widget.backgroundAlpha<1);
  const compose=surface==="shell"&&!state.appearance.highContrast&&state.appearance.widget.backgroundAlpha<1;
  return <div ref={root} style={{...layoutStyle,...(!state.appearance.highContrast ? widgetStyle(state.appearance.widget,surface==="shell") : {})}} data-label-style={labelStyle} className={`native-surface ${compose?"surface-composition":""} ${palettePaint ? "widget-palette" : ""} ${palettePaint && state.appearance.widget.originalStructure ? "widget-original" : ""} ${surface} mode-${mode}${layout ? " stable-compact" : ""}${trayPreview ? " tray-summary" : ""}${state.settings.windows.lockPosition ? " position-locked" : ""}`} onContextMenu={contextMenu} onKeyDown={keyDown}
    onPointerDown={pointerDown} onPointerUp={pointerEnd} onPointerCancel={pointerEnd}
    onPointerMove={event => { const origin = dragOrigin.current; if (origin && origin.pointerId === event.pointerId && shouldStartWidgetDrag(origin, { x: event.clientX, y: event.clientY }, state.settings.windows.lockPosition, dragging.current)) { dragging.current = true; suppressClick.current = true; void surfaceAction({ type: "drag" }); } }}
    onClick={event => { if (surface !== "shell" || isControl(event.target)) return; if (!shouldActivateWidgetClick(event.detail, suppressClick.current)) { event.preventDefault(); return; } void surfaceAction({ type: event.detail === 0 ? "details" : "activate" }); }}
    onDoubleClick={event => { if (surface === "shell" && !isControl(event.target) && !suppressClick.current) { event.preventDefault(); void surfaceAction({ type: "open-window" }); } }}>
    {compose&&<WidgetBackground root={root} alpha={state.appearance.widget.backgroundAlpha}/>}
    <WidgetContents state={state} surface={surface} now={now} onAction={request=>void surfaceAction(request)} />
  </div>;
}
