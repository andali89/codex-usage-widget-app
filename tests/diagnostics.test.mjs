import test from "node:test";
import assert from "node:assert/strict";
import { diagnostic } from "../src/diagnostics.ts";

test("protocol incompatibility warning includes arbitrary CLI version text", () => {
  for (const version of ["codex-cli 0.163.0", "codex-cli 0.164.0", "custom nightly / future build"]) {
    const message = diagnostic("incompatible-codex-protocol", version);
    assert.ok(message.includes(`This Codex installation (${version}) is incompatible`));
    assert.match(message, /Update Codex or the widget, then Refresh\./);
  }
});

test("unavailable version information still produces an actionable warning", () => {
  for (const version of [null, undefined, ""]) {
    assert.match(diagnostic("incompatible-codex-protocol", version), /\(CLI version unavailable\).*incompatible/);
  }
});

test("authentication and transient diagnostics do not blame the CLI version", () => {
  for (const code of ["auth-required", "unsupported-auth-mode", "protocol-timeout", "usage-read-failed", "invalid-quota-field"]) {
    const message = diagnostic(code, "custom future version");
    assert.doesNotMatch(message, /incompatible|custom future version/);
  }
});
