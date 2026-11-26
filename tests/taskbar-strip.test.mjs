import test, { before, after } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { taskbarPeriodLabel, taskbarStripRows } from "../src/taskbarStrip.ts";
import { defaultWidgetAppearance, resolveWidgetAppearance } from "../src/widgetAppearance.ts";

let server, TaskbarStripReadings, WidgetContents, PeriodSettings, WidgetAppearanceSettings;
before(async () => {
  server = await createServer({ server: { middlewareMode: true, hmr: false }, appType: "custom" });
  TaskbarStripReadings = (await server.ssrLoadModule("/src/TaskbarStripReadings.tsx")).default;
  WidgetContents = (await server.ssrLoadModule("/src/WidgetContents.tsx")).default;
  PeriodSettings = (await server.ssrLoadModule("/src/PeriodSettings.tsx")).default;
  WidgetAppearanceSettings = (await server.ssrLoadModule("/src/WidgetAppearanceSettings.tsx")).default;
});
after(async () => { await server?.close(); });

const hour = 3_600_000;
const day = 24 * hour;

test("taskbarPeriodLabel maps standard, custom, and unknown durations honestly", () => {
  assert.equal(taskbarPeriodLabel(300, 0), "5h:");
  assert.equal(taskbarPeriodLabel(10080, 1), "Wk:");
  assert.equal(taskbarPeriodLabel(60, 0), "1h:");
  assert.equal(taskbarPeriodLabel(1440, 1), "1d:");
  assert.equal(taskbarPeriodLabel(17, 0), "17m:");
  assert.equal(taskbarPeriodLabel(null, 0), "P1:");
  assert.equal(taskbarPeriodLabel(null, 1), "P2:");
});

test("taskbarStripRows preserves unknown durations, handles reversed/partial/empty windows, and filters single periods", () => {
  const empty = taskbarStripRows([], "both");
  assert.deepEqual(empty.map(r => [r.label, r.remainingPercent]), [["5h:", null], ["Wk:", null]]);

  const reversed = taskbarStripRows([
    { key: "codex/secondary/10080", durationMinutes: 10080, remainingPercent: 60, resetsAtMs: 3 * day, lastKnown: false },
    { key: "codex/primary/300", durationMinutes: 300, remainingPercent: 50, resetsAtMs: 2 * hour, lastKnown: false },
  ], "both");
  assert.deepEqual(reversed.map(r => [r.label, r.remainingPercent]), [["5h:", 50], ["Wk:", 60]]);

  const customUnknown = taskbarStripRows([
    { key: "codex/primary/unknown", durationMinutes: null, remainingPercent: 75, resetsAtMs: hour, lastKnown: false },
    { key: "codex/secondary/1440", durationMinutes: 1440, remainingPercent: 25, resetsAtMs: day, lastKnown: false },
  ], "both");
  assert.deepEqual(customUnknown.map(r => [r.label, r.remainingPercent]), [["P1:", 75], ["1d:", 25]]);

  const singleWeekly = taskbarStripRows(reversed, "weekly");
  assert.equal(singleWeekly.length, 1);
  assert.equal(singleWeekly[0].label, "Wk:");
  assert.equal(singleWeekly[0].remainingPercent, 60);
});

test("TaskbarStripReadings renders micro bars on/off, <1% sliver, worst-case widths, due?, stale, and hollow unavailable tracks", () => {
  const windows = [
    { key: "codex/primary/300", label: "5-hour", durationMinutes: 300, remainingPercent: 0.4, resetsAtMs: 0, quality: "valid", lastKnown: false },
    { key: "codex/secondary/10080", label: "Weekly", durationMinutes: 10080, remainingPercent: 99.9, resetsAtMs: 23 * hour + 59 * 60_000, quality: "valid", lastKnown: false },
  ];
  const withBars = renderToStaticMarkup(createElement(TaskbarStripReadings, { windows, selection: "both", showBars: true, retained: false, now: 0 }));
  assert.match(withBars, />5h:</);
  assert.match(withBars, />&lt;1%</);
  assert.match(withBars, /min-width:0\.125rem/);
  assert.match(withBars, /class="ts-rst ts-due">due\?</);
  assert.match(withBars, />Wk:</);
  assert.match(withBars, />99\.9%</);
  assert.match(withBars, />23h59m</);
  assert.equal((withBars.match(/class="micro-bar/g) ?? []).length, 2);

  const withoutBars = renderToStaticMarkup(createElement(TaskbarStripReadings, { windows, selection: "both", showBars: false, retained: false, now: 0 }));
  assert.equal((withoutBars.match(/class="micro-bar/g) ?? []).length, 0);

  const unavailable = renderToStaticMarkup(createElement(TaskbarStripReadings, { windows: [], selection: "both", showBars: true, retained: true, now: 0 }));
  assert.equal((unavailable.match(/class="micro-bar unknown retained"/g) ?? []).length, 2);
  assert.doesNotMatch(unavailable, /0%/);
  assert.match(unavailable, />—</);
});

test("WidgetContents renders Taskbar Strip blocked/warning indicators and two-row read-only Tray hover preview", () => {
  const appearance = defaultWidgetAppearance();
  const makeState = (mode, usageOverrides = {}, previewSource = "shell") => ({
    settings: {
      mode,
      themePreference: "system",
      widgetAppearance: appearance,
      windows: { quotaLabelStyle: "words", lockPosition: false, visiblePeriods: {}, resetCountdowns: {} },
    },
    appearance: {
      resolvedTheme: "light",
      systemResolvedTheme: "dark",
      highContrast: false,
      transparencyAllowed: false,
      widget: resolveWidgetAppearance(appearance, "light"),
      taskbarWidget: resolveWidgetAppearance(appearance, "dark"),
    },
    usage: {
      windows: [
        { key: "codex/primary/300", label: "5-hour", durationMinutes: 300, remainingPercent: 50, usedPercent: 50, resetsAtMs: 2 * hour + 17 * 60_000, quality: "valid", lastKnown: false },
        { key: "codex/secondary/10080", label: "Weekly", durationMinutes: 10080, remainingPercent: 60, usedPercent: 40, resetsAtMs: 3 * day + 11 * hour, quality: "valid", lastKnown: false },
      ],
      freshness: "fresh",
      connection: "ready",
      ordinaryUsageAllowed: null,
      resetCreditsAvailable: null,
      refreshInFlight: false,
      consecutiveUnavailableReads: 0,
      missingWindowReads: {},
      ...usageOverrides,
    },
    presentation: { previewSource },
    trayAvailable: true,
  });

  const freshStrip = renderToStaticMarkup(createElement(WidgetContents, { state: makeState("taskbar-strip"), surface: "shell", now: 0 }));
  assert.match(freshStrip, /taskbar-strip-rows/);
  assert.match(freshStrip, />50%</);
  assert.match(freshStrip, />2h17m</);
  assert.match(freshStrip, />60%</);
  assert.match(freshStrip, />3d11h</);
  assert.doesNotMatch(freshStrip, /status-warning/);

  const blockedStrip = renderToStaticMarkup(createElement(WidgetContents, { state: makeState("taskbar-strip", { ordinaryUsageAllowed: false }), surface: "shell", now: 0 }));
  assert.match(blockedStrip, /class="status-warning critical"[^>]*>!</);

  const trayHover = renderToStaticMarkup(createElement(WidgetContents, { state: makeState("system-tray", {}, "tray"), surface: "preview", now: 0 }));
  assert.match(trayHover, /tray-summary-rows/);
  assert.match(trayHover, />5-hour</);
  assert.match(trayHover, />50% left</);
  assert.match(trayHover, />2h17m</);
  assert.match(trayHover, />Weekly</);
  assert.match(trayHover, />60% left</);
  assert.match(trayHover, />3d11h</);
  assert.doesNotMatch(trayHover, /<button/);

  const staleTray = renderToStaticMarkup(createElement(WidgetContents, { state: makeState("system-tray", { freshness: "stale", connection: "offline" }, "tray"), surface: "preview", now: 0 }));
  assert.match(staleTray, /Last known · Offline · out of date/);
  assert.doesNotMatch(staleTray, /2h17m|3d11h/);

  const periodHtml = renderToStaticMarkup(createElement(PeriodSettings, { value: { visiblePeriods: { "taskbar-strip": "five-hour" }, resetCountdowns: {} }, onChange: () => {} }));
  assert.match(periodHtml, /aria-label="Taskbar Strip: displayed periods"/);
  assert.doesNotMatch(periodHtml, /aria-label="Taskbar Strip: 5-hour reset"/);

  const mixedThemeState = makeState("taskbar-strip");
  const settingsHtml = renderToStaticMarkup(createElement(WidgetAppearanceSettings, { state: mixedThemeState, draft: mixedThemeState.settings, onChange: () => {} }));
  assert.match(settingsHtml, /Editing Dark colors \(matching Windows taskbar\) for Original/);
});
