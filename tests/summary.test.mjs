import test from "node:test";
import assert from "node:assert/strict";
import { trayHoverSummary, usageSummary } from "../src/summary.ts";

const window = (label, remaining, resetsAtMs, durationMinutes = 300) => ({ key: label, label, remainingPercent:remaining, resetsAtMs, durationMinutes, quality: remaining === null ? "missing" : "valid", lastKnown:false });
const usage = windows => ({ windows, freshness:"fresh", connection:"ready", ordinaryUsageAllowed:null });
test("account verification is distinct from transport status and quota estimates", () => {
  const pending = {...usage([]),connection:"verifying",nextRefreshAtMs:6000};
  assert.deepEqual(usageSummary(pending,0),{first:"Account verification pending",second:"Waiting to retry…"});
  assert.deepEqual(usageSummary({...pending,refreshInFlight:true},0),{first:"Verifying account…",second:null});
});
test("compact summary labels missing windows and omits absent reset rows", () => {
  const result = usageSummary(usage([window("5-hour",68,null),window("Weekly",null,null,10080)]),0);
  assert.match(result.first,/5-hour 68% left/); assert.match(result.first,/Weekly unavailable/); assert.equal(result.second,null);
});
test("stale and blocked states take priority over optimistic forecasts", () => {
  const value = {...usage([window("5-hour",74,60000)]),ordinaryUsageAllowed:false};
  assert.equal(usageSummary(value,0).second,"Ordinary usage blocked");
  const stale = usageSummary({...value,freshness:"stale",connection:"offline"},0);
  assert.match(stale.first,/Last known:/); assert.match(stale.second,/Offline/); assert.doesNotMatch(stale.second,/reset/);
  assert.equal(usageSummary({...usage([]),ordinaryUsageAllowed:false},0).second,"Ordinary usage blocked");
});
test("reset ordering uses time then shorter duration; due remains unconfirmed", () => {
  const value = usage([window("Weekly",42,60000,10080),window("5-hour",68,60000)]);
  assert.match(usageSummary(value,0).second,/^5-hour:/);
  assert.equal(usageSummary(value,60001).second,"Reset due — unconfirmed");
});
test("a retained invalid field is visibly last known and fractional data is not exhausted", () => {
  const result = usageSummary(usage([{...window("5-hour",0.5,60000),lastKnown:true,quality:"invalid"}]),0);
  assert.match(result.first,/Last known:.*<1% left/); assert.equal(result.second,"Some data out of date");
});

test("missing usage waits for three reads while authentication is immediate", () => {
  for (const consecutiveUnavailableReads of [0, 1, 2]) {
    assert.equal(usageSummary({...usage([]), consecutiveUnavailableReads}, 0).first, "Checking usage…");
  }
  assert.equal(usageSummary({...usage([]), consecutiveUnavailableReads:3}, 0).first, "Usage unavailable");
  for (const connection of ["auth-required", "unsupported", "paused"]) {
    assert.equal(usageSummary({...usage([]), connection, consecutiveUnavailableReads:1}, 0).first, "Usage unavailable");
  }
});

test("an absent window announces checking before its third missing read", () => {
  const data = usage([window("5-hour",68,null), {...window("Weekly",null,null,10080), key:"codex/secondary/10080"}]);
  assert.match(usageSummary({...data, missingWindowReads:{"codex/secondary":2}}, 0).first, /Weekly checking/);
  assert.match(usageSummary({...data, missingWindowReads:{"codex/secondary":3}}, 0).first, /Weekly unavailable/);
});

test("trayHoverSummary returns two aligned rows with both resets and respects honest edge states", () => {
  const hour = 3_600_000, day = 24 * hour;
  const fresh = trayHoverSummary(usage([
    window("5-hour", 68, 2 * hour + 17 * 60_000, 300),
    window("Weekly", 42, 4 * day + 12 * hour, 10080),
  ]), 0);
  assert.equal(fresh.fallback, null);
  assert.equal(fresh.status, null);
  assert.equal(fresh.rows.length, 2);
  assert.deepEqual(fresh.rows.map(r => [r.label, r.value, r.reset, r.due]), [
    ["5-hour", "68% left", "2h17m", false],
    ["Weekly", "42% left", "4d12h", false],
  ]);

  const due = trayHoverSummary(usage([window("5-hour", 0, 1000, 300), window("Weekly", 50, day, 10080)]), 1000);
  assert.equal(due.rows[0].reset, "due?");
  assert.equal(due.rows[0].due, true);
  assert.equal(due.rows[1].reset, "1d");

  const stale = trayHoverSummary({ ...usage([window("5-hour", 68, hour, 300), window("Weekly", 42, day, 10080)]), freshness: "stale", connection: "offline" }, 0);
  assert.equal(stale.rows.every(r => r.retained && r.reset === null), true);
  assert.match(stale.status, /Last known.*Offline/);

  const blocked = trayHoverSummary({ ...usage([window("5-hour", 68, hour, 300)]), ordinaryUsageAllowed: false }, 0);
  assert.equal(blocked.critical, true);
  assert.equal(blocked.status, "Ordinary usage blocked");

  const verifying = trayHoverSummary({ ...usage([]), connection: "verifying", refreshInFlight: true }, 0);
  assert.equal(verifying.rows.length, 0);
  assert.equal(verifying.fallback?.first, "Verifying account…");

  const unknownDur = trayHoverSummary(usage([window("Primary window (duration unavailable)", 55, hour, null)]), 0);
  assert.equal(unknownDur.rows[0].label, "Primary (period unknown)");
});
