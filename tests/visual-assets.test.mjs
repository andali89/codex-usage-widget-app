import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

test("bundled artwork has alpha, appropriate dimensions, and bounded file sizes", () => {
  for (const [path, size, budget] of [["../src/assets/brand-ring.png", 128, 65536], ["../src/assets/usage-empty.png", 512, 163840], ["../src-tauri/icons/notification.png", 128, 65536]]) {
    const image = readFileSync(new URL(path, import.meta.url));
    assert.equal(image.subarray(1, 4).toString(), "PNG");
    assert.equal(image.readUInt32BE(16), size);
    assert.equal(image.readUInt32BE(20), size);
    assert.equal(image[25], 6, "RGBA transparency is preserved");
    assert.ok(image.length < budget, `${path} is small enough for compact views`);
  }
});

test("Windows icon includes small tray frames and a high-resolution app frame", () => {
  const icon = readFileSync(new URL("../src-tauri/icons/icon.ico", import.meta.url));
  assert.equal(icon.readUInt16LE(2), 1);
  const sizes = Array.from({length:icon.readUInt16LE(4)}, (_, i) => icon[6 + i * 16] || 256);
  for (const size of [16, 32, 48, 256]) assert.ok(sizes.includes(size), `contains ${size}px frame`);
});
