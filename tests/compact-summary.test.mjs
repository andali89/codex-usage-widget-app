import test from "node:test";
import assert from "node:assert/strict";
import { edgeQuotaValues } from "../src/compactSummary.ts";

test("Screen Edge compact summary includes five-hour and weekly remaining values", () => {
  assert.deepEqual(edgeQuotaValues([
    { durationMinutes: 300, remainingPercent: 68 },
    { durationMinutes: 10080, remainingPercent: 42 },
  ]), [
    { label: "5H", remainingPercent: 68 },
    { label: "WEEK", remainingPercent: 42 },
  ]);
});

test("missing Screen Edge quota windows stay unavailable instead of becoming zero", () => {
  assert.deepEqual(edgeQuotaValues([
    { durationMinutes: 300, remainingPercent: null },
  ]), [
    { label: "5H", remainingPercent: null },
    { label: "WEEK", remainingPercent: null },
  ]);
});
