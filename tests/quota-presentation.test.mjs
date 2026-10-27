import test, { before, after } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";

let server, QuotaCard, ResetCreditsCard, UsagePage, CompactQuotaReadings, PeriodSettings, StorageSettings, WidgetAppearanceSettings;
before(async () => {
  server = await createServer({server:{middlewareMode:true,hmr:false},appType:"custom"});
  QuotaCard = (await server.ssrLoadModule("/src/QuotaCard.tsx")).default;
  ResetCreditsCard = (await server.ssrLoadModule("/src/ResetCreditsCard.tsx")).default;
  UsagePage = (await server.ssrLoadModule("/src/UsagePage.tsx")).default;
  CompactQuotaReadings = (await server.ssrLoadModule("/src/CompactQuotaReadings.tsx")).default;
  PeriodSettings = (await server.ssrLoadModule("/src/PeriodSettings.tsx")).default;
  StorageSettings = (await server.ssrLoadModule("/src/StorageSettings.tsx")).default;
  WidgetAppearanceSettings = (await server.ssrLoadModule("/src/WidgetAppearanceSettings.tsx")).default;
});
after(async () => { await server?.close(); });
const quota = {label:"5-hour",durationMinutes:300,remainingPercent:68,usedPercent:32,resetsAtMs:3600000,lastKnown:false};
const card = props => renderToStaticMarkup(createElement(QuotaCard,{quota,stale:false,now:0,...props}));

test("appearance controls show only active-theme custom quotas and no embedded previews",async()=>{
  const {defaultWidgetAppearance,resolveWidgetAppearance}=await import("../src/widgetAppearance.ts");
  const appearance=defaultWidgetAppearance();appearance.backgroundTransparencyPercent=80;
  const draft={mode:"system-tray",widgetAppearance:appearance,windows:{snapGapLogicalPx:8,resetCountdowns:{},visiblePeriods:{},quotaLabelStyle:"words",lockPosition:false}};
  const state={settings:draft,appearance:{resolvedTheme:"light",highContrast:false,transparencyAllowed:false,widget:resolveWidgetAppearance(appearance,"light",false,false),placementConstraints:{}},usage:{windows:[{...quota,label:"PRIVATE_REAL_VALUE"}],freshness:"stale"},presentation:{previewSource:"shell"},trayAvailable:true};
  const before=structuredClone(state);
  const html=renderToStaticMarkup(createElement(WidgetAppearanceSettings,{state,draft,onChange:()=>{throw new Error("Rendering must not save");}}));
  assert.doesNotMatch(html,/transparency|Transparency/);assert.match(html,/<input id="show-widget-border" type="checkbox" checked=""/);assert.match(html,/Show widget border/);assert.match(html,/Editing Light colors for Original/);assert.match(html,/Use custom quota colors/);
  assert.doesNotMatch(html,/Requested style|Effective style|PRIVATE_REAL_VALUE|radiogroup|Load preset colors|type="color"/);
  draft.widgetAppearance.quotaCustomizations={original:{light:{enabled:true},dark:{enabled:false}}};
  const enabled=renderToStaticMarkup(createElement(WidgetAppearanceSettings,{state,draft,onChange:()=>{}}));
  assert.match(enabled,/Load preset colors/);assert.match(enabled,/5-hour quota/);assert.match(enabled,/Weekly quota/);assert.equal((enabled.match(/role="radiogroup"/g)??[]).length,2);
  state.appearance.resolvedTheme="dark";
  const dark=renderToStaticMarkup(createElement(WidgetAppearanceSettings,{state,draft,onChange:()=>{}}));
  assert.match(dark,/Editing Dark colors/);assert.doesNotMatch(dark,/Load preset colors|radiogroup/);
  draft.widgetAppearance.showBorder=false;
  const borderless=renderToStaticMarkup(createElement(WidgetAppearanceSettings,{state,draft,onChange:()=>{}}));
  assert.ok(borderless.includes('<input id="show-widget-border" type="checkbox"/>'));
  assert.doesNotMatch(borderless,/transparency|Transparency/);
  assert.deepEqual(before.usage,state.usage);

});

test("HUD and Edge hide the whole unselected period including an enabled countdown", () => {
  const windows=[{...quota,key:"five",durationMinutes:300},{...quota,key:"week",label:"Weekly",durationMinutes:10080,remainingPercent:42,resetsAtMs:86400000}];
  for(const mode of ["screen-edge","compact-hud"]) for(const labelStyle of ["words","badges"]) for(const selection of ["both","five-hour","weekly"]) {
    for(const fiveHour of [false,true]) for(const weekly of [false,true]) {
      const html=renderToStaticMarkup(createElement(CompactQuotaReadings,{mode,labelStyle,selection,windows,resets:{fiveHour,weekly},retained:false,now:0}));
      assert.equal(html.includes("68%"),selection!=="weekly");
      assert.equal(html.includes("42%"),selection!=="five-hour");
      assert.equal((html.match(/class="reset-indicator/g)??[]).length,Number(selection!=="weekly"&&fiveHour)+Number(selection!=="five-hour"&&weekly));
      if(selection==="weekly")assert.match(html,/weekly/);
    }
  }
});

test("selected missing data is unavailable and hidden countdown preferences remain checked", () => {
  const html=renderToStaticMarkup(createElement(CompactQuotaReadings,{mode:"compact-hud",labelStyle:"words",selection:"weekly",windows:[],resets:{fiveHour:true,weekly:true},retained:false,now:0}));
  assert.match(html,/WEEK/);assert.match(html,/—/);assert.doesNotMatch(html,/0%|5H/);assert.match(html,/reset time unavailable/);
  const value={visiblePeriods:{"screen-edge":"five-hour","compact-hud":"weekly"},resetCountdowns:{"screen-edge":{fiveHour:true,weekly:true},"compact-hud":{fiveHour:true,weekly:true}}};
  const controls=renderToStaticMarkup(createElement(PeriodSettings,{value,onChange:()=>{}}));
  assert.match(controls,/aria-label="Screen Edge: Weekly reset"[^>]*disabled=""[^>]*checked=""/);
  assert.match(controls,/aria-label="Compact HUD: 5-hour reset"[^>]*disabled=""[^>]*checked=""/);
  assert.match(controls,/Hover previews and Overview still show both/);
});

test("storage controls show preferred versus effective location and fallback", () => {
  const html=renderToStaticMarkup(createElement(StorageSettings,{value:{kind:"custom",directory:"D:\\Widget config"},status:{activeDirectory:"C:\\Widget",fallback:true,diagnostic:"storage-fallback"},onChange:()=>{}}));
  assert.match(html,/Custom folder/);assert.match(html,/Choose folder/);assert.match(html,/C:\\Widget/);assert.match(html,/Using fallback/);assert.match(html,/AppData/);
});

test("Overview renders native CLI version diagnostics and clears the warning after recovery", () => {
  const usage = {windows:[], freshness:"empty", connection:"unsupported", errorCode:"incompatible-codex-protocol", codexCliVersion:"codex-cli 0.164.0", resetCreditsAvailable:null, consecutiveUnavailableReads:1};
  const state = {usage, dashboardVisible:false, firstLaunch:false, settings:{reminders:{enabled:false}}, reminders:{deliveryPaused:false}};
  const render = value => renderToStaticMarkup(createElement(UsagePage,{state:{...state,usage:value},onSettings:()=>{},onReminders:()=>{},onLayoutChange:()=>{}}));
  const warning = render(usage);
  assert.match(warning, /This Codex installation \(codex-cli 0\.164\.0\) is incompatible/);
  assert.match(warning, /Connection settings/);
  assert.match(warning, /Automatic reads paused/);
  assert.match(render({...usage,codexCliVersion:null}), /CLI version unavailable/);
  const recovered = render({...usage,connection:"ready",errorCode:null,codexCliVersion:null});
  assert.doesNotMatch(recovered, /is incompatible|CLI version unavailable|Connection settings/);
});

test("compact desktop hover independently exposes used percentage and an exact reset", () => {
  const hover = card({compact:true,showDetails:true});
  assert.match(hover,/32% used/);
  assert.match(hover,/Expected reset:/);
  assert.match(hover,/dateTime="1970-01-01T01:00:00.000Z"/);
  assert.doesNotMatch(card({compact:true}),/used-reading|exact-reset/);
  assert.match(card({}),/32% used/);
});
test("hover details preserve unavailable and last-known states", () => {
  const missing = card({compact:true,showDetails:true,quota:{...quota,usedPercent:null,resetsAtMs:null}});
  assert.match(missing,/Used percentage unavailable/);
  assert.match(missing,/Reset time not supplied/);
  const stale = card({compact:true,showDetails:true,stale:true});
  assert.match(stale,/32% used · last known/);
  assert.match(stale,/Last known expected reset:/);
  assert.match(card({compact:true,showDetails:true,now:3600001}),/Reset due — unconfirmed/);
});
test("credit rows disclose only after three and keep the reported available count", () => {
  for(const count of [0,1,3,4,30]) {
    const credits = Array.from({length:count},(_,index)=>({status:"available",expiresAtMs:index%2?null:0}));
    const html = renderToStaticMarkup(createElement(ResetCreditsCard,{available:7,credits,stale:false,onLayoutChange:()=>{}}));
    assert.equal((html.match(/class="credit-row"/g) ?? []).length,Math.min(count,3));
    assert.match(html,/<strong>7<\/strong>/);
    assert.equal(html.includes('aria-expanded="false"'),count>3);
    if(count>3) assert.match(html,new RegExp(`Show more \\(${count-3}\\)`));
    if(count===0) assert.match(html,/No credit details returned/);
    if(count>1) assert.match(html,/No expiry supplied/);
    if(count>0) assert.match(html,/1970-01-01T00:00:00.000Z/);
  }
});
