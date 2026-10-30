import type { WidgetAppearanceSettings, EffectiveWidgetAppearance } from "./widgetAppearance";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { diagnosticEvent, setDiagnosticsEnabled } from "./useDiagnostics";
import { Replica } from "./replica";
export { percent, countdown } from "./format";

export type ThemePreference = "system" | "light" | "dark";
export type DisplayMode = "floating" | "screen-edge" | "system-tray" | "compact-hud";
export type QuotaLabelStyle = "words" | "badges";
export type VisiblePeriods = "both" | "five-hour" | "weekly";
export type StorageLocation = { kind: "executable" | "app-data" } | { kind: "custom"; directory: string };
export interface StorageStatus { preferred: StorageLocation; activeDirectory: string; fallback: boolean; diagnostic: string | null; portableLocator: boolean }
export interface ResetCountdowns { fiveHour: boolean; weekly: boolean }
export interface ResetSegmentCounts { fiveHour: number; weekly: number }
export interface ReminderRule { low: boolean; thresholds: number[]; approaching: boolean; leadMinutes: number; recovery: boolean }
export interface ReminderSchedule { id:string; revision:number; enabled:boolean; label:string; timing:{kind:"once";atMs:number}|{kind:"weekly";minute:number;weekdays:number[]} }
export interface ReminderPreferences { enabled:boolean; defaults:ReminderRule; windowRules:Record<string,ReminderRule>; quietHours:{enabled:boolean;startMinute:number;endMinute:number;weekdays:number[]}; schedules:ReminderSchedule[]; silent:boolean; detailed:boolean; suppressFullscreen:boolean }
export interface ReminderEvent { ruleKey:string; category:string; label:string; remaining:number|null; observedAt:number; eventKeys:string[] }
export interface ReminderDelivery { id:number; atMs:number; status:string; reason:string|null; attempts:number; events:ReminderEvent[] }
export const defaultReminders = ():ReminderPreferences => ({enabled:false,defaults:{low:true,thresholds:[20,10,0],approaching:false,leadMinutes:5,recovery:true},windowRules:{},quietHours:{enabled:false,startMinute:1320,endMinute:420,weekdays:[0,1,2,3,4,5,6]},schedules:[],silent:true,detailed:false,suppressFullscreen:true});
export interface Settings {
  schemaVersion: number; diagnosticLoggingEnabled: boolean; revision: number; themePreference: ThemePreference;
  mode: DisplayMode; codexPath: string | null; proxyUrl: string | null; storageLocation: StorageLocation;
  foregroundSeconds: number; compactSeconds: number; backgroundSeconds: number;
  widgetAppearance: WidgetAppearanceSettings;
  windows: { snapGapLogicalPx:number; snapAnchors:Partial<Record<DisplayMode,{horizontal:"none"|"left"|"right";vertical:"none"|"top"|"bottom"}>>; hoverEnabled: boolean; lockPosition: boolean; alwaysOnTop: boolean; suppressFullscreen: boolean; edgeSide: "left" | "right"; quotaLabelStyle: QuotaLabelStyle; placements: Partial<Record<DisplayMode, { monitor: string | null; x: number; y: number }>>; resetCountdowns: Partial<Record<DisplayMode, ResetCountdowns>>; resetSegmentCounts: Partial<Record<"screen-edge" | "compact-hud", ResetSegmentCounts>>; visiblePeriods: Partial<Record<"screen-edge" | "compact-hud", VisiblePeriods>> };
  reminders: ReminderPreferences;
  autostart: {requested:boolean;pending:boolean;effective:boolean|null;diagnostic:string|null};
}
export interface QuotaWindow {
  key: string; limitId: string; label: string; usedPercent: number | null;
  remainingPercent: number | null; durationMinutes: number | null;
  resetsAtMs: number | null; validatedAtMs: number | null;
  quality: "valid" | "missing" | "invalid"; lastKnown: boolean;
}
export interface Usage {
  accountContextKey: string | null; connectionGeneration: number;
  selectedLimitId: string | null; windows: QuotaWindow[];
  resetCreditsAvailable: number | null;
  resetCredits: { status: string; expiresAtMs: number | null }[] | null;
  planType: string | null; ordinaryUsageAllowed: boolean | null;
  additionalLimits: boolean; lastAttemptAtMs: number | null;
  lastSuccessAtMs: number | null; nextRefreshAtMs: number | null;
  refreshInFlight: boolean; connection: string; freshness: "empty" | "fresh" | "stale";
  errorCode: string | null;
  codexCliVersion: string | null;
  consecutiveUnavailableReads: number;
  missingWindowReads: Record<string, number>;
}
export interface AppState {
  diagnosticSegment: number;
  processEpoch: string; revision: number; settings: Settings;
  dashboardRequest:{sequence:number;page:"overview"|"settings"}|null;
  firstLaunch:boolean;
  appearance: {
    compactMonitorBudgets?:Record<string,import("./compactSizing").MonitorBudget>; compactSizing:Partial<Record<"screen-edge"|"compact-hud",import("./compactSizing").ScaleState>>; widget: EffectiveWidgetAppearance; placementConstraints:{margins:number[];taskbarUncertain:boolean;reducedToFit:boolean;contentExceedsWorkArea:boolean};
    themePreference: ThemePreference; resolvedTheme: "light" | "dark";
    highContrast: boolean; transparencyAllowed: boolean; reducedMotion: boolean; textScale: number;
  };
  usage: Usage; storage: StorageStatus; storageDiagnostic: string | null;
  trayAvailable: boolean; dashboardVisible: boolean;
  presentation: { generation: number; shell: "compact" | "preview"; detail: "closed" | "popup" | "window"; hidden: boolean; suppressed: boolean; traySuppressed: boolean; previewSource: "tray" | "shell"; menuOpen: boolean; dragging: boolean };
  windowDiagnostic: string | null; tooltipFallback: boolean; effectiveRefreshSeconds: number;
  monitors: { name: string; width: number; height: number; scale: number }[];
  reminders:{deliveryPaused:boolean;snoozedUntil:number|null;pendingCount:number;history:ReminderDelivery[];diagnostic:string|null;identityScope:string};
}

export function applyAppearance(state: AppState) {
  const root = document.documentElement;
  root.dataset.theme = state.appearance.resolvedTheme;
  root.style.setProperty("--quota-five-hour", state.appearance.highContrast ? "LinkText" : state.appearance.widget.palette.fiveHour);
  root.style.setProperty("--quota-weekly", state.appearance.highContrast ? "LinkText" : state.appearance.widget.palette.weekly);
  for(const [role,token] of [["fiveHour","five-hour"],["weekly","weekly"]] as const) {
    root.style.setProperty(`--quota-${token}-text`,state.appearance.highContrast?"LinkText":state.appearance.widget.quotaText[role]);
  }
  root.dataset.highContrast = String(state.appearance.highContrast);
  root.dataset.reducedMotion = String(state.appearance.reducedMotion);
  root.dataset.largeText = String(state.appearance.textScale >= 150);
  root.style.fontSize = `${state.appearance.textScale}%`;
}

export const native = isTauri();
export const command = <T,>(name: string, args?: Record<string, unknown>) => invoke<T>(name, args);

export function useSharedState() {
  const [state, setState] = useState<AppState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const replica = useRef(new Replica<AppState>());
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let reconciling = false;
    const accept = (next: AppState) => {
      if (disposed) return { accepted: false, gap: false };
      const old=replica.current.current;
      const result = replica.current.apply(next);
      if(result.accepted)setDiagnosticsEnabled(next.settings.diagnosticLoggingEnabled,next.diagnosticSegment);
      if(result.gap)diagnosticEvent("gap",{revision:next.revision});
      if(!result.accepted)diagnosticEvent("rejected",{revision:next.revision});
      if(old&&old.processEpoch!==next.processEpoch)diagnosticEvent("epoch_changed",{revision:next.revision});
      if (result.accepted) setState(next);
      return result;
    };
    const reconcile = async (reason:"initial"|"gap"|"focus"|"visible"="initial") => {
      if (reconciling || disposed) return;
      reconciling = true;
      const began=performance.now();
      diagnosticEvent("snapshot_start",{reason});
      try { accept(await command<AppState>("get_state")); diagnosticEvent("snapshot_end",{reason,durationMs:Math.round(performance.now()-began)}); if (!disposed) setError(null); }
      catch { diagnosticEvent("snapshot_failed",{reason,durationMs:Math.round(performance.now()-began)}); if (!disposed) setError("Shared state is unavailable. Reopen the dashboard from the tray."); }
      finally { reconciling = false; }
    };
    // Subscribe before snapshot. Events received during snapshot win by epoch/revision.
    void (async () => {
      try {
        const remove = await listen<AppState>("widget-state", event => {
          if (accept(event.payload).gap) void reconcile("gap");
        });
        if (disposed) { remove(); return; }
        unlisten = remove;
        diagnosticEvent("listen_ready");
        await reconcile();
      } catch { diagnosticEvent("listen_failed"); if (!disposed) setError("Cannot connect to the native state service."); }
    })();
    const resume = () => { if (!document.hidden) void reconcile("visible"); };
    const focus = () => { if (!document.hidden) void reconcile("focus"); };
    document.addEventListener("visibilitychange", resume);
    window.addEventListener("focus", focus);
    return () => { disposed = true; unlisten?.(); document.removeEventListener("visibilitychange", resume); window.removeEventListener("focus", focus); };
  }, []);
  return { state, error };
}

export function useVisibleClock(enabled = true) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = () => {
      clearTimeout(timer);
      if (enabled && !document.hidden) { setNow(Date.now()); timer = setTimeout(tick, 1000); }
    };
    tick(); document.addEventListener("visibilitychange", tick);
    return () => { clearTimeout(timer); document.removeEventListener("visibilitychange", tick); };
  }, [enabled]);
  return now;
}

export { diagnostic } from "./diagnostics";
