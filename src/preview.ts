import { defaultWidgetAppearance, resolveWidgetAppearance } from "./widgetAppearance";
// This separate Vite development entry is never imported by the production application.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { AppState, Settings } from "./state";

if (!import.meta.env.DEV) throw new Error("Synthetic fixtures require the development server");
(globalThis as unknown as { isTauri: boolean }).isTauri = true;
// Initialize the native flag before state.ts captures it, so hooks use mocked IPC.
const { defaultReminders } = await import("./state");
const query = new URLSearchParams(location.search);
const at = Date.now();
const dark = query.get("theme") === "dark";
const scenario = query.get("scenario") ?? "fresh";
const state: AppState = {
  processEpoch: "synthetic-fixture", revision: 1, dashboardRequest:null, firstLaunch:query.has("welcome"),
  settings: { schemaVersion: 7, widgetAppearance:defaultWidgetAppearance(), autostart:{requested:false,pending:false,effective:false,diagnostic:null}, revision: 0, themePreference: "system", mode: query.get("mode") === "screen-edge" ? "screen-edge" : query.get("mode") === "compact-hud" ? "compact-hud" : query.get("mode") === "floating" ? "floating" : "system-tray", codexPath: null, proxyUrl:null, storageLocation:{kind:"executable"}, foregroundSeconds: 120, compactSeconds: 120, backgroundSeconds: 120, windows: { snapGapLogicalPx:8,snapAnchors:{}, hoverEnabled:true, lockPosition:false, alwaysOnTop:false, suppressFullscreen:true, edgeSide:"right", quotaLabelStyle:query.get("labels") === "badges" ? "badges" : "words", placements:{}, resetCountdowns:{}, visiblePeriods:{} }, reminders:defaultReminders() },
  appearance: { widget:resolveWidgetAppearance(defaultWidgetAppearance(),dark?"dark":"light",query.has("contrast"),false), placementConstraints:{margins:[8,8,8,8],taskbarUncertain:false,reducedToFit:false,contentExceedsWorkArea:false}, themePreference: "system", resolvedTheme: dark ? "dark" : "light", highContrast: query.has("contrast"), transparencyAllowed: false, reducedMotion: true, textScale: Number(query.get("scale") ?? 100) },
  usage: {
    accountContextKey: "synthetic-only", connectionGeneration: 1, selectedLimitId: "codex",
    windows: [
      { key: "codex/primary/300", limitId: "codex", label: "5-hour", usedPercent: 32, remainingPercent: 68, durationMinutes: 300, resetsAtMs: at + 8_220_000, validatedAtMs: at, quality: "valid", lastKnown: false },
      { key: "codex/secondary/10080", limitId: "codex", label: "Weekly", usedPercent: 58, remainingPercent: 42, durationMinutes: 10080, resetsAtMs: at + 390_000_000, validatedAtMs: at, quality: "valid", lastKnown: false },
    ],
    resetCreditsAvailable: 3, resetCredits: null, planType: "synthetic plan", ordinaryUsageAllowed: null,
    additionalLimits: false, lastAttemptAtMs: at, lastSuccessAtMs: at, nextRefreshAtMs: at + 120000,
    refreshInFlight: false, connection: "ready", freshness: "fresh", errorCode: null, codexCliVersion: null, consecutiveUnavailableReads:0, missingWindowReads:{},
  }, storage:{preferred:{kind:"executable"},activeDirectory:"Synthetic folder",fallback:false,diagnostic:null,portableLocator:true}, storageDiagnostic: null, trayAvailable: true, dashboardVisible: true,
  presentation: { generation:0, shell:"preview", detail:"window", hidden:false, suppressed:false, traySuppressed:false, previewSource:query.has("tray-hover") || !query.has("mode") ? "tray" : "shell", menuOpen:false, dragging:false },
  windowDiagnostic:null, tooltipFallback:false, effectiveRefreshSeconds:120,
  reminders:{deliveryPaused:false,snoozedUntil:null,pendingCount:0,history:[],diagnostic:"notification-adapter-unavailable",identityScope:"synthetic"},
  monitors:[{ name:"Synthetic display",width:1920,height:1080,scale:1 }],
};
const baseUsage = structuredClone(state.usage);
if (query.has("resets") && state.settings.mode !== "system-tray") {
  state.settings.windows.resetCountdowns[state.settings.mode] = {
    fiveHour: ["both", "five-hour"].includes(query.get("resets")!),
    weekly: ["both", "weekly"].includes(query.get("resets")!),
  };
}
if (query.get("page") === "settings") state.dashboardRequest = {sequence:1,page:"settings"};
if (query.get("detail") === "popup") state.presentation.detail = "popup";
if (scenario === "missing") { state.usage.windows = []; state.usage.resetCreditsAvailable = null; state.usage.freshness = "empty"; state.usage.connection = "auth-required"; state.usage.errorCode = "auth-required"; }
if (scenario === "loading") { state.usage.windows=[]; state.usage.resetCreditsAvailable=null; state.usage.freshness="empty"; state.usage.connection="starting"; state.usage.refreshInFlight=true; }
if (scenario === "partial") { state.usage.windows[1].remainingPercent=null; state.usage.windows[1].usedPercent=null; state.usage.windows[1].quality="missing"; }
if (scenario === "fractional") { state.usage.windows[0].remainingPercent=0.5; state.usage.windows[0].usedPercent=99.5; }
if (scenario === "stale") { state.usage.freshness = "stale"; state.usage.connection = "offline"; state.usage.errorCode = "usage-read-failed"; }
if (scenario === "blocked") state.usage.ordinaryUsageAllowed = false;
if (scenario === "due") state.usage.windows.forEach(window => { window.resetsAtMs = at - 1000; });
if (scenario === "no-reset") state.usage.windows.forEach(window => { window.resetsAtMs = null; });
if (scenario === "credit-details") state.usage.resetCredits = [{ status: "available", expiresAtMs: at + 86400000 }, { status: "available", expiresAtMs: null }];
if (query.has("credits")) {
  const count = Math.min(100, Math.max(0, Number(query.get("credits")) || 0));
  state.usage.resetCredits = Array.from({length:count}, (_, index) => ({status:["available", "redeeming", "redeemed", "unknown"][index % 4], expiresAtMs:index % 2 ? null : at + 86400000 * (index + 1)}));
}
if (["max-width", "long-reset"].includes(scenario)) state.usage.windows.forEach(window => { window.remainingPercent=99.9; window.usedPercent=0.1; window.resetsAtMs=at + (window.durationMinutes === 300 ? 17940000 : 602000000); window.lastKnown=true; });
if (scenario === "long-reset") state.usage.windows[1].resetsAtMs = at + 86340000;
mockWindows(query.get("surface") === "shell" ? "shell" : query.get("surface") === "preview" ? "preview" : "main");
const actionHistory: string[] = [];
let displayPreview:{session:string;sequence:number}|null=null;
const fixturePaint=(theme:Settings["themePreference"],appearance:Settings["widgetAppearance"],gap:number)=>{
  state.appearance.themePreference=theme;
  state.appearance.resolvedTheme=theme==="system"?(dark?"dark":"light"):theme;
  state.appearance.widget=resolveWidgetAppearance(appearance,state.appearance.resolvedTheme,state.appearance.highContrast,state.appearance.transparencyAllowed);
  state.appearance.placementConstraints.margins=[gap,gap,gap,gap];
};
const fixturePublish=async()=>{state.revision++;await emit("widget-state",structuredClone(state));return structuredClone(state);};
mockIPC(async (name, payload) => {
  if (name === "get_state") return structuredClone(state);
  if(name==="begin_display_preview") {
    const args=payload as {session:string;expectedRevision:number};
    if(args.expectedRevision!==state.settings.revision)throw "settings-conflict";
    if(displayPreview&&displayPreview.session!==args.session)throw "display-preview-owned";
    displayPreview??={session:args.session,sequence:0};return structuredClone(state);
  }
  if(name==="update_display_preview") {
    const args=payload as {session:string;sequence:number;patch:{themePreference:Settings["themePreference"];widgetAppearance:Settings["widgetAppearance"];snapGapLogicalPx:number}|null};
    if(!displayPreview||displayPreview.session!==args.session)throw "display-preview-ended";
    if(args.sequence<=displayPreview.sequence)throw "display-preview-outdated";
    displayPreview.sequence=args.sequence;
    if(args.patch)fixturePaint(args.patch.themePreference,args.patch.widgetAppearance,args.patch.snapGapLogicalPx);
    return fixturePublish();
  }
  if(name==="end_display_preview") {
    if(displayPreview?.session===(payload as {session:string}).session){displayPreview=null;fixturePaint(state.settings.themePreference,state.settings.widgetAppearance,state.settings.windows.snapGapLogicalPx);}
    return fixturePublish();
  }
  if (name === "save_settings") {
    const args = payload as { expectedRevision: number; settings: Settings };
    if (args.expectedRevision !== state.settings.revision) throw "settings-conflict";
    displayPreview=null;
    state.settings = { ...args.settings, revision: state.settings.revision + 1 };
    state.appearance.themePreference = state.settings.themePreference;
    state.appearance.resolvedTheme = state.settings.themePreference === "system" ? dark ? "dark" : "light" : state.settings.themePreference;
    state.appearance.widget=resolveWidgetAppearance(state.settings.widgetAppearance,state.appearance.resolvedTheme,state.appearance.highContrast,state.appearance.transparencyAllowed);
    state.revision++;
    await emit("widget-state", structuredClone(state));
    return structuredClone(state);
  }
  if (name === "surface_action") {
    const { request } = payload as { request: { type: string } };
    if (request.type !== "ready") { actionHistory.push(request.type); document.getElementById("fixture-banner")!.dataset.actions = actionHistory.join(","); }
    if (request.type === "settings") { state.dashboardRequest = {sequence:(state.dashboardRequest?.sequence ?? 0)+1,page:"settings"}; state.revision++; await emit("widget-state",structuredClone(state)); document.getElementById("fixture-banner")!.textContent="Synthetic fixture · anchored Settings requested · no native effects"; }
    if (request.type === "toggle-hidden") { state.presentation.hidden = !state.presentation.hidden; document.getElementById("fixture-banner")!.textContent="Synthetic fixture · hide widget requested · no native effects"; }
    if (["promote", "open-window"].includes(request.type)) {
      state.presentation.detail = "window";
      state.revision++;
      await emit("widget-state", structuredClone(state));
      document.getElementById("fixture-banner")!.textContent = "Synthetic fixture · window promotion simulated · no native effects";
    }
  }
  if (["refresh", "hide_dashboard", "quit"].includes(name)) document.getElementById("fixture-banner")!.textContent = `Synthetic fixture · ${name} simulated · no native effects`;
  return null;
}, { shouldMockEvents: true });
let fixtureStep = 0;
document.getElementById("fixture-next")!.addEventListener("click", () => {
  fixtureStep = (fixtureStep + 1) % 5;
  const credits = state.usage.resetCredits;
  const available = state.usage.resetCreditsAvailable;
  state.usage = structuredClone(baseUsage);
  state.usage.resetCredits = credits;
  state.usage.resetCreditsAvailable = available;
  if (fixtureStep === 1) state.usage.windows.forEach(quota => { quota.remainingPercent=99.9; quota.usedPercent=0.1; quota.resetsAtMs=Date.now()+(quota.durationMinutes === 300 ? 17940000 : 602000000); });
  if (fixtureStep === 2) state.usage.windows.forEach(quota => { quota.remainingPercent=0; quota.usedPercent=100; quota.resetsAtMs=Date.now()-1000; });
  if (fixtureStep === 3) { state.usage.freshness="stale"; state.usage.connection="offline"; state.usage.windows.forEach(quota => { quota.remainingPercent=0.5; quota.usedPercent=99.5; quota.resetsAtMs=Date.now()+30000; }); }
  if (fixtureStep === 4) { state.usage.windows=[]; state.usage.freshness="empty"; state.usage.connection="auth-required"; }
  state.revision++; void emit("widget-state", structuredClone(state));
});
document.getElementById("fixture-overview")!.addEventListener("click", () => {
  state.dashboardRequest={sequence:(state.dashboardRequest?.sequence ?? 0)+1,page:"overview"}; state.revision++;
  void emit("widget-state", structuredClone(state));
});
await import("./main");
