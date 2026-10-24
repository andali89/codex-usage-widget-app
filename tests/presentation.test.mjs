import test from "node:test";
import assert from "node:assert/strict";
import { percent, countdown, compactReset, resetSegments, resetTimestamp } from "../src/format.ts";
import { Replica } from "../src/replica.ts";

test("fractional quota and missing data never round into exhausted quota", () => {
  assert.equal(percent(null), "Unavailable");
  assert.equal(percent(0), "0%");
  assert.equal(percent(0.01), "<1%");
  assert.equal(percent(0.9), "<1%");
  assert.equal(percent(74), "74%");
});
test("elapsed countdown is unconfirmed and never negative or recovered", () => {
  assert.equal(countdown(100, 200), "Reset due — unconfirmed");
  assert.equal(countdown(3600001, 0), "Expected reset in 1h 1m");
  assert.equal(countdown(86400000, 0), "Expected reset in 1d");
});
test("compact reset units follow period thresholds and the local clock", () => {
  const hour = 3600000, day = 24 * hour;
  assert.equal(compactReset(2.5 * hour, 0, 300), "2h30m");
  assert.equal(compactReset(hour, 0, 300), "1h");
  assert.equal(compactReset(hour - 1, 0, 300), "1h");
  assert.equal(compactReset(1000, 0, 300), "1m");
  assert.equal(compactReset(2.5 * day, 0, 10080), "2d12h");
  assert.equal(compactReset(day, 0, 10080), "1d");
  assert.equal(compactReset(day - 1, 0, 10080), "1d");
  assert.equal(compactReset(1000, 0, 10080), "1m");
  assert.equal(compactReset(2.5 * hour, 0.5 * hour, 300), "2h");
  assert.equal(compactReset(null, 0, 300), "—");
  assert.equal(compactReset(1000, 1000, 300), "due?");
  assert.equal(compactReset(1000, 2000, 10080), "due?");
});
test("reset segments measure time remaining and never claim recovery", () => {
  const duration = 300 * 60000;
  assert.equal(resetSegments(duration, 0, 300), 10);
  assert.equal(resetSegments(duration, duration / 2, 300), 5);
  assert.equal(resetSegments(duration, duration - 1, 300), 1);
  assert.equal(resetSegments(duration, duration, 300), 0);
  assert.equal(resetSegments(duration, duration + 1, 300), 0);
  assert.equal(resetSegments(duration * 2, 0, 300), 10);
  for (const period of [null, 0, -1, Infinity]) assert.equal(resetSegments(duration, 0, period), null);
  assert.equal(resetSegments(null, 0, 300), null);
  assert.equal(resetSegments(NaN, 0, 300), null);
});
test("reset timestamp preserves the instant, date, minutes and numeric time-zone offset", () => {
  const instant = Date.UTC(2026, 9, 23, 17, 9);
  const expected = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit", timeZoneName: "shortOffset" }).format(instant);
  assert.equal(resetTimestamp(instant), expected);
  assert.match(resetTimestamp(instant), /GMT/);
});
test("subscription event wins against an older or duplicate snapshot", () => {
  const replica = new Replica();
  replica.apply({ processEpoch: "a", revision: 4, value: "event" });
  assert.equal(replica.apply({ processEpoch: "a", revision: 3, value: "snapshot" }).accepted, false);
  assert.equal(replica.apply({ processEpoch: "a", revision: 4 }).accepted, false);
  assert.equal(replica.current.value, "event");
  assert.equal(replica.apply({ processEpoch: "a", revision: 7 }).gap, true);
});
test("new process epoch replaces old state and rejects delayed retired events", () => {
  const replica = new Replica();
  replica.apply({ processEpoch: "a", revision: 500 });
  assert.equal(replica.apply({ processEpoch: "b", revision: 0 }).accepted, true);
  assert.equal(replica.apply({ processEpoch: "a", revision: 501 }).accepted, false);
  assert.equal(replica.current.processEpoch, "b");
});
