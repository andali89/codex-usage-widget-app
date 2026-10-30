# Optional diagnostic logging

Implemented 2026-10-30 for investigating intermittent loading and stale displays. This change instruments existing behavior; it does not alter loading policy, quota normalization, refresh timing, or widget geometry. No root cause for the original intermittent issue has been established.

## Enable and collect

1. Open **Settings → Connection & diagnostics**. Enable **local diagnostic logging** and Save. The saved preference defaults to off and survives restart. Cancel leaves the saved preference unchanged.
2. Leave the widget running until the issue occurs. For startup evidence, enable logging before restarting. Logging cannot reconstruct earlier events.
3. Use **Open logs folder** on the same Settings page. Recording status, a safe failure code, and the active directory are shown there. If recording is suspended, correct the folder/disk problem and Save with logging enabled to retry.
4. Disable and Save when collection is complete. Existing files remain available.

Logs are in **`<active configuration directory>\diagnostics\widget.jsonl`**, with `widget.1.jsonl` through `widget.4.jsonl` as backups. Default portable configuration means the folder beside `Usage.exe`; AppData, custom configuration, and configuration fallback use their actual active directory. Automatic storage fallback follows that directory at state publication. Changing configuration locations leaves earlier logs in their earlier folders.

Summarize a folder from the repository root:

```powershell
powershell.exe -NoProfile -File scripts/summarize-diagnostics.ps1 -Directory 'C:\path\to\diagnostics'
# Structured output, including individual episodes:
powershell.exe -NoProfile -File scripts/summarize-diagnostics.ps1 -Directory 'C:\path\to\diagnostics' -AsJson
```

The script reads only the five known log filenames and does not modify them. It reports episode counts, observed entries per hour, exact completed duration median/95th percentile/maximum, observed duration totals, incomplete intervals, triggers, error codes/stages, event counts, and detected drop/truncation information. Native and individual view statistics overlap: do not add their durations together. Rate denominators are recorded source spans, not a guarantee that every moment was visible or observable. Compare sessions of similar length and conditions.

## Evidence and interpretation

Records use versioned JSON Lines with Unix timestamps in milliseconds, process/session and recording-segment identifiers, sequence numbers, and process-monotonic timing. Frontend reports also contain elapsed time within their webview. Wall-clock changes are recorded; durations use monotonic clocks rather than differences between wall timestamps. Control records have no producer sequence number.

| Evidence | What it helps distinguish |
| --- | --- |
| `scheduler_request`, `scheduler_start`, `scheduler_finish`, `refresh_request`, `refresh_schedule` | Startup, scheduled reads, reset deadlines, backoff, manual/detail requests, coalescing, paused reads and next retry delay. A nearby request is context, not proof of causation. |
| `refresh_start`, `protocol_stage`, `read_complete`, `refresh_result` | Read IDs link discovery, launch, initialize, account-before/after, quota read and cleanup timing to safe failures/timeouts. Early discovery/launch failures still have stage and final refresh results. |
| `usage_state`, `state_enter`, `state_exit`, `recovery` | Connection/freshness, generation, quota presence/quality, retained fields and missing-read counters, observation age, and overlapping loading/refresh/stale/unavailable episodes. Recovery requires usable fresh data, not merely leaving a state or reaching a reset deadline. |
| `context_rebound`, `protocol_hint`, `result_discarded`, `freshness_expired`, `clock_change`, `timezone_change`, `resume_reconcile`, `settings_saved` | Context changes, invalidated reads, freshness expiration, time/resume effects and connection reconfiguration. Account-context contents are excluded. |
| `frontend` snapshot/listen/gap/rejected/epoch events | Shared-state loading versus quota loading; snapshot latency, failed subscriptions, revision gaps and reconciliation reasons. Reports bypass the state actor so a blocked snapshot does not block reporting. |
| `webview_create`, `webview_page`, `webview_idle_destroy`, `webview_destroyed`, frontend mount/unmount/display | Expected five-minute idle destruction/recreation versus initial hydration and loading during an established process. Page/lifecycle records provide context when a recreated view starts with no shared state. |
| Frontend `render_summary` | Once-per-minute commit counts, changed revision inputs and clock inputs, separated by root/dashboard usage view. Clock and revision counts can overlap. Other commits can be ordinary local UI activity; these counts do not prove unnecessary rendering or measure full React render cost. |

Native loading entries are labeled startup until usable quota data has been observed during the process, then runtime. Explicit settings/context/lifecycle events distinguish expected runtime reinitialization from unexplained episodes. Frontend display flags independently represent shared-state loading, quota checking, refreshing, stale/retained data, partial data, unavailable data, blocked usage and visibility; compact period selection is respected. Normal refresh activity is recorded separately from quota loss.

Start with a runtime `state_enter` for `quota_loading` or `stale`; inspect the preceding trigger, matching read/stage events, final result, quality flags and view snapshot events. A successful native read with a delayed/failed snapshot points to a different investigation than a protocol timeout. Repeated `context_rebound` with `stable:false`, generation discards, freshness expirations or high revision/render counts are observations to investigate, not automatic diagnoses.

The first observation after enabling, page creation, hidden-to-visible transitions, unmount, disable, abrupt exit, or missing rotated history may censor an interval. The summary excludes censored durations from exact percentiles and treats their measured durations as partial observations. Frontend episodes are counted only while the view is visible. If a session reports dropped events, its episode durations are withheld from exact statistics; gaps can hide intermediate transitions. An abruptly stopped process or crash can lose the buffered tail, so absence of a final event is not proof of a specific failure.

## Efficiency, reliability and privacy

- Rust owns the recorder. Producers enqueue small allowlisted records without waiting for disk. A 256-record bounded queue, a 4 KiB serialized-record limit, and dropped-event counters bound work and expose overload. Frontend batches contain at most 32 events; native ingress is limited to 240 events per window per minute. Browser-side buffer overflow has its own counter.
- A dedicated writer buffers disk output, flushes at least once per second while active, and rotates before a file exceeds **1 MiB**. Four backups plus the active file cap retained log data at **5 MiB per configuration folder**. There is no age-based retention; old folders are not purged or globally capped. Known log filenames are ignored by Git if configuration is placed in this repository.
- Unchanged usage/display states are deduplicated. Render commits are counters, not per-render disk records. Disabled logging creates no diagnostic directory/file or recording/render-summary timers. One status bootstrap per webview is independent of shared-state acquisition. After first activation, the writer thread parks when disabled; Settings polls recording status every five seconds only while its diagnostic section is active and the saved preference is enabled.
- Disk/open/rotation failure suspends the recorder without changing quota state or returning failure from quota operations. Shutdown requests a bounded flush (500 ms worker wait within a 600 ms application bound). A stalled disk can still delay that worker or lose its tail; it must not block normal service work.
- Logs exclude tokens, credentials, account identifiers and opaque account keys, raw protocol responses/stderr, quota percentages, plan/credit values, paths, proxy URLs, and arbitrary error messages/stacks. Safe static codes, fixed categories, timing and numeric flags/counters are recorded. Timing records remain local user activity information; inspect a collection before sharing it.
- Frontend diagnostic IPC accepts typed event/reason enums and bounded numbers; it rejects unknown fields/text. The native window supplies the source. Shared state carries a diagnostic segment number; views re-baseline when recording moves/restarts and reports from superseded segments are counted as dropped. Directory opening accepts no caller-provided path and is dashboard-only. No broad shell/filesystem/network capability, dependency, installation or telemetry upload was added.
- Settings schema 13 migrates older documents with logging disabled and preserves opt-in on current-schema round trips. An older build may reject the newer settings schema through the existing future-version protection.

## Validation

Commands run from the repository root during implementation:

| Check | Result |
| --- | --- |
| `npm.cmd test` | Passed: 156 frontend/policy tests, including diagnostic flags, period filtering, render counters and the real PowerShell summary with censored durations, frequency and dropped-event handling. |
| `npm.cmd run build` | Passed: TypeScript and Vite production build. |
| `npm.cmd run rust:fmt` | Passed, including the maintained autostart patch. |
| `npm.cmd run rust:check` | Passed. |
| `npm.cmd run rust:clippy` | Passed with warnings denied. Cargo separately reported incremental-cache hard-link fallback warnings. |
| `npm.cmd run rust:test` | Passed outside the filesystem sandbox: 172 application tests, one explicit live probe ignored, and 3 dependency tests. Covers rotation/size limits, disabled behavior, queue saturation, failure/retry/directory routing, rotation failure, state timing/deduplication, IPC text rejection and schema migration. Sandbox attempts failed Windows rename/atomic storage operations; the unrestricted run passed. |
| `cargo build --manifest-path src-tauri/Cargo.toml --locked` | Passed: debug native executable. |
| Debug `Usage.exe --check-native-diagnostics`, with `npm.cmd run dev -- --strictPort` | Passed using the isolated synthetic validation profile and a development server outside sandbox loopback restrictions. Verified HUD fresh/loading/stale/recovery, recording, native/frontend records, payload exclusions, Settings control, disabling without changing usage, and shell recreation into Edge. It does not access live Codex or send inference requests. |
| PowerShell summary on native validation logs | Passed; parsed synthetic native/frontend records without malformed lines or reported dropped events. Generated artifacts remain under `src-tauri/target/validation/native-diagnostics/`. |
| `npm.cmd run desktop:build` | Blocked: installed `@tauri-apps/cli-win32-x64-msvc` native binding reports “not a valid Win32 application” both inside and outside the sandbox. No dependencies were reinstalled or upgraded. |

Initial native attempts timed out before hydration because the sandbox-started development server was unreachable from the validation process. Running both outside that restriction passed. WebView2 printed an unregister-class teardown message after the successful check; no quota behavior change was made in response.

Not established: a packaged/installed runtime test, Explorer opening via the new button, long-running CPU/memory comparison, real sleep/resume and mixed-DPI behavior, or reproduction/root cause of the user's intermittent issue. The native test exercises explicit recreation rather than waiting five minutes for the unchanged idle lease. Live Codex probing was intentionally not performed.
