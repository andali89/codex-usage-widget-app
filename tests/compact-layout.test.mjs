import test from "node:test";
import assert from "node:assert/strict";
import { compactLayout } from "../src/compactLayout.ts";
import { hudQuotaValues } from "../src/compactSummary.ts";

test("normal compact readings keep their footprint, with space added only for a visible warning", () => {
  for (const style of ["words", "badges"]) {
    for (const fiveHour of [false, true]) for (const weekly of [false, true]) {
      const resets = {fiveHour, weekly}, count = Number(fiveHour) + Number(weekly);
      for (const warning of [false,true]) {
        assert.deepEqual(compactLayout("screen-edge", style, resets,warning), {width: 54, height: (style === "badges" ? 150 : 190) + count * 24 + (warning ? 18 : 0)});
        assert.deepEqual(compactLayout("compact-hud", style, resets,warning), {width:(style === "badges" ? 168 : 204) + count * 58 + (warning ? 28 : 0), height:38});
      }
    }
  }
  for (const periods of ["both", "five-hour", "weekly"]) {
    const single = periods !== "both";
    for (const showBars of [true, false]) {
      for (const warning of [false, true]) {
        assert.deepEqual(
          compactLayout("taskbar-strip", "words", { fiveHour: true, weekly: true }, warning, periods, showBars),
          { width: 128 + (warning ? 16 : 0), height: single ? (showBars ? 28 : 24) : (showBars ? 42 : 36) },
        );
      }
    }
  }
  assert.equal(compactLayout("floating", "words"), null);
  assert.equal(compactLayout("system-tray", "badges"), null);
});

test("HUD retains two slots without inventing unknown period durations or quota values", () => {
  const empty = hudQuotaValues([]);
  assert.equal(empty.length, 2);
  assert.ok(empty.every(quota => quota.durationMinutes === null && quota.remainingPercent === null));
  const partial = hudQuotaValues([{key:"primary",label:"5-hour",durationMinutes:300,remainingPercent:0.5}]);
  assert.equal(partial[0].label, "5H");
  assert.equal(partial[0].remainingPercent, 0.5);
  assert.equal(partial[1].remainingPercent, null);
  const unusual = hudQuotaValues([{key:"primary",label:"17-minute",durationMinutes:17,remainingPercent:82}]);
  assert.equal(unusual[0].label, "5H");
  assert.equal(unusual[0].remainingPercent, null);
  const weeklyOnly = hudQuotaValues([{key:"primary",label:"Weekly",durationMinutes:10080,remainingPercent:42}]);
  assert.equal(weeklyOnly[0].durationMinutes, null);
  assert.equal(weeklyOnly[1].label, "WEEK");
  assert.equal(weeklyOnly[1].remainingPercent, 42);
  const reversed = hudQuotaValues([{key:"primary",label:"Weekly",durationMinutes:10080,remainingPercent:42},{key:"secondary",label:"5-hour",durationMinutes:300,remainingPercent:68}]);
  assert.deepEqual(reversed.map(quota => quota.label), ["5H", "WEEK"]);
});
