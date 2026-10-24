import test from "node:test";
import assert from "node:assert/strict";
import { shouldActivateWidgetClick, shouldStartWidgetDrag } from "../src/widgetGestures.ts";

test("a six-pixel move starts a drag, while shorter moves remain clicks", () => {
  const origin = { x: 20, y: 30 };
  assert.equal(shouldStartWidgetDrag(origin, { x: 25.9, y: 30 }, false, false), false);
  assert.equal(shouldStartWidgetDrag(origin, { x: 24, y: 35 }, false, false), true);
});

test("position lock and an active drag prevent a second drag start", () => {
  const origin = { x: 0, y: 0 };
  const moved = { x: 10, y: 0 };
  assert.equal(shouldStartWidgetDrag(origin, moved, true, false), false);
  assert.equal(shouldStartWidgetDrag(origin, moved, false, true), false);
});

test("single and keyboard clicks activate, but double-clicks and drag releases do not", () => {
  assert.equal(shouldActivateWidgetClick(1, false), true);
  assert.equal(shouldActivateWidgetClick(0, false), true);
  assert.equal(shouldActivateWidgetClick(2, false), false);
  assert.equal(shouldActivateWidgetClick(1, true), false);
});
