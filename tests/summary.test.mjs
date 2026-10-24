import test from "node:test";
import assert from "node:assert/strict";
import { usageSummary } from "../src/summary.ts";

const window = (label, remaining, resetsAtMs, durationMinutes = 300) => ({ key: label, label, remainingPercent:remaining, resetsAtMs, durationMinutes, quality: remaining === null ? "missing" : "valid", lastKnown:false });
const usage = windows => ({ windows, freshness:"fresh", connection:"ready", ordinaryUsageAllowed:null });
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
