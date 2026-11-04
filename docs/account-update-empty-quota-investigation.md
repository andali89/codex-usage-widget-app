# Account-update notifications and empty quota values

Updated: 2026-11-04. Status: notification timing bug reproduced with synthetic protocol fixtures and corrected in source. Frontend and Rust checks pass; Windows runtime validation is recorded below. No new release package has been produced.

## Implemented correction — 2026-11-04

A late `account/updated` notification no longer immediately rejects the whole refresh and empties the widget. The candidate is discarded and the same owned app-server performs **one** additional account-before → quota → account-after pass, without repeating initialization. This pass has a shared **15-second** reading budget and must be notification-free from its start; a further hint or failure ends verification rather than starting another pass. Existing per-request timeouts, cancellation, process cleanup, and shared scheduler spacing/backoff remain in place.

Only the complete accepted observation is published. Its validated quota `accountId` participates in the existing salted opaque context calculation. A different context normalizes from an empty baseline, preventing old windows, credits or missing-field grace from crossing account/workspace boundaries. Missing quota identity stays session-scoped with reminders/reset retries suspended. Email/plan equality does not establish cross-read continuity.

Read failures now carry a backend-only account-verification classification. Confirmed changes, sign-out, permanent errors, failed reconciliation and repeated hints clear the visible cache and invalidate reminder/reset context. An ordinary transient failure retains values as explicitly stale only when the previous binding had a stable quota identity. This evidence flag is backend-only, excluded from serialized shared state and disk settings; no public IPC field or settings migration was added.

Overview, Edge/HUD warning text, previews and native tray fallback distinguish **Verifying account…**, **Account verification pending**, and **Waiting to retry…**. Unresolved verification explains why old values are hidden instead of showing generic startup wording. Protocol diagnostics add pass numbers and safe account-field comparison outcomes; identifiers, percentages and raw responses remain excluded. Explicit stage names replace the request-ID heuristic so the second account-before check is labeled correctly.

This is a focused correction: the process-per-refresh lifecycle remains. A late hint can add three read-only RPCs in the same process. There is no additional scheduler, background retry loop, dependency, permission, authentication operation or inference request.

## Additional baseline evidence — 2026-11-04

Read-only analysis of the local `scripts/widget.2.jsonl` followed by `scripts/widget.1.jsonl` found **8,611 valid records** spanning approximately **13 h 45 min**, from November 3 at 20:28 to November 4 at 10:13 UTC+08:00, in one process/recording segment. The user confirmed no intentional account/workspace changes or sign-out during this interval.

| File | Successful results | Account-change rejections | Runtime empty episodes |
| --- | --- | --- | --- |
| `widget.2.jsonl` | 214 | 22 | 20 |
| `widget.1.jsonl` | 187 | 35 | 30 |

All **57** rejected reads received an account notification after the initial account response. All **400** successful reads with complete transaction history received it before that response; one other successful result begins at a file boundary without its earlier stages. No stage errors, malformed lines or reported drops were found. Six resume reconciliations and two generation discards appear but do not explain the 50 empty episodes: every entry follows an account-change rejection and has offline/empty usage with zero present windows.

The **50** completed runtime episodes total **505.552 s**, with median **8.1665 s**, 95th percentile **26.381 s**, and maximum **27.937 s**. Together with the source branch and deterministic fixture reproduction, these establish a coding bug in account-notification handling. They do not identify Codex's notification emission cause or prove identity equality from redacted logs. The supplied files remain untracked; do not commit them. The older `widget.jsonl` is an October 30 collection, not the chronological tail of these November backups.

## Original evidence — 2026-10-30

The local-only `scripts/widget.jsonl` confirms that the app repeatedly clears previously usable quota values after receiving `account/updated` during a read. All nine observed runtime empty-data episodes follow `account-changed-during-read`. Every recorded initialize/account/quota protocol stage completed without an error; the transaction rejected the observation afterward. This is an application account-verification/clearing path, rather than evidence that the quota endpoint returned no values or that a network timeout occurred.

Notification timing perfectly separates the recorded outcomes: all 63 successful transactions received the notification before the first account response, while all 10 rejected transactions received it after that response. The flag is reset at that boundary in the current code. This strongly supports a delayed notification causing unnecessary invalidation. The privacy-preserving log cannot prove that the underlying identities matched or establish why Codex emitted each notification; an actual account change remains indistinguishable in these records.

The `read_cycle` lifecycle launches a fresh app-server for every refresh and shuts it down afterward. Every recorded read therefore includes process initialization. The one-notification-per-read pattern is consistent with a startup-related notification, but the log does not establish its origin.

### Collection and measurements

The supplied file contains 1,958 valid format-1 records across two processes, covering 2026-10-30 approximately **15:44:31–17:33:59 UTC+08:00**. Recording metadata identifies application version **0.1.5**. No malformed/truncated lines or reported native/ingress/frontend-buffer drops were found. The CLI version and account identities are absent, and collection begins after the first process already had fresh data. The log remains untracked and is not included in this report or a commit.

| Observation | Result |
| --- | --- |
| Completed refresh results | 73: 63 successful; 10 rejected with `account-changed-during-read` |
| `account_updated` hints | 73, one per completed read |
| Successful reads | All 63 hints precede the `account_before` stage result |
| Rejected reads | All 10 hints follow `account_before` and precede the `quota_read` result |
| Errors on individual protocol stages | None |
| Runtime native `quota_loading` episodes | 9 completed episodes; all begin with offline/empty usage, no present quota windows, and the account-change error |
| Runtime empty duration | Median **8.294 s**, maximum **27.039 s**, total **92.216 s** over **6,559.403 s** of observed native spans |
| Separate recorded startup loading | One incomplete observation, at least **5.066 s**, excluded from runtime duration statistics |
| Freshness-expiry, generation-discard, clock-change or resume-reconciliation events | None recorded |

Runtime episodes begin at **15:56:59, 16:19:52, 16:24:09, 16:40:35, 16:57:11, 17:22:05, 17:27:47, 17:28:24, and 17:28:48**, all UTC+08:00. There are ten rejected reads but nine episodes because the first episode contains two consecutive rejections before recovery.

The first episode makes the sequence particularly clear:

| Local time (UTC+08:00) | Recorded event and consequence |
| --- | --- |
| 15:56:59.384 | Read 8 returns `account-changed-during-read` after all protocol stages completed successfully. The service clears usage and schedules a retry about 4.987 s later. |
| 15:56:59.387 | Native `quota_loading` begins; both quota windows are absent. |
| 15:57:07.383 | Retry read 9 encounters the same ordering and error; the next retry delay grows to about 15.956 s. |
| 15:57:26.423 | Read 10 succeeds and rebinds a stable context. |
| 15:57:26.427 | The empty-data episode ends after 27.039 s. |

The shell records the same nine runtime quota-loading episodes. Seven webview creations and four idle destructions also appear, but recreating a webview is not necessary to explain these native quota losses. No native stale episode was recorded: the error path removes the retained values instead of marking them stale.

## Reported symptom

During normal use, Overview sometimes shows **Rechecking local usage…** while Screen Edge or Compact HUD shows empty quota placeholders instead of previous percentages marked **Last known · out of date**.

The initial source-only investigation had no occurrence log. The subsequently supplied 0.1.5 diagnostic file now establishes the clearing path and its timing; the executable hash and CLI version are still unknown.

## Pre-fix behavior found in source

The following describes the source before the November 4 correction. The supplied logs match this path; it does not establish behavior in every released build.

1. In [`protocol.rs`](../src-tauri/src/protocol.rs), receiving `account/updated` sets `account_changed = true`. During a read transaction, this flag is cleared after the first `account/read`, before reading quota values.
2. After the quota read and a second `account/read`, the transaction returns `account-changed-during-read` if the account comparison differs **or** the flag is set. The flag alone rejects the result even when the compared account information matches.
3. In [`service.rs`](../src-tauri/src/service.rs), that error invalidates reminder context, increments the connection generation, and replaces usage with an empty default. It then sets the connection to `offline` and schedules a retry.
4. In [`summary.ts`](../src/summary.ts), `usagePending` is true while the consecutive unavailable-read count is below three, unless the connection is `auth-required`, `unsupported`, or `paused`.
5. In [`UsagePage.tsx`](../src/UsagePage.tsx), an offline state with no refresh running, no overall stale flag, and `usagePending = true` produces **Rechecking local usage…**. The prominent error notice is suppressed during this pending/offline state; the diagnostic explanation remains in Read details.
6. [`CompactQuotaReadings.tsx`](../src/CompactQuotaReadings.tsx) renders empty placeholders when the selected periods have no retained percentage values.

The supplied log confirms this sequence immediately after previously visible quota values. Replacing usage with its default also removes the last-success timestamp and account binding, which makes an established process appear to be waiting for its first data. The scheduler adds jittered retry backoff (initial targets 5 s, then 15 s); the next read adds its own execution time. This explains the usual roughly eight-second gap and the longer two-failure episode.

A temporary transport failure can also produce the reported combination when values were already empty, such as after startup or a connection reset. A transport failure with existing quota windows ordinarily preserves stale values. Neither a transport failure nor a three-missing-observation sequence explains the nine runtime episodes in this file.

The third consecutive missing/invalid quota observation is a separate clearing policy. Three consecutive unavailable reads generally end the pending wording, so that policy alone does not explain this exact combination.

## Meaning of `account/updated`

The [official OpenAI app-server documentation](https://learn.chatgpt.com/docs/app-server) describes `account/updated` as a server notification about authentication-mode changes and illustrates it after login and logout. It carries the authentication mode and the ChatGPT plan when available. Quota changes have a separate notification, `account/rateLimits/updated`.

The project's [Codex 0.163.0 notification schema](../schemas/codex/0.163.0/v2/AccountUpdatedNotification.json) defines `authMode` and `planType`, with no account identifier. Receiving the notification alone therefore does not establish that the user switched to a different account. Its exact emission circumstances in the user's CLI version still need verification; this investigation does not establish that routine token refresh caused it.

## Original problem and remaining uncertainty

Before the correction, the widget treated an account-state notification received after the first account check as sufficient to reject the whole observation and clear useful last-known values. The exact same notification received earlier was forgotten by resetting the flag. Generic retry wording and suppression of the prominent error notice then made the loss look like ordinary loading. The synthetic fixture now reproduces both timings and verifies that the corrected path accepts a fresh consistent replacement without empty publication.

Account safety still matters: clearing or hiding old values is appropriate when the account changes or its binding cannot be verified. Matching the current before/after comparison is insufficient evidence by itself: `account_identity` currently compares account type eligibility and email, with a fallback when email is absent. It does not prove a stable account/workspace binding.

## Correction validation — 2026-11-04

| Check | Result |
| --- | --- |
| `npm.cmd test` | Passed: 158 frontend tests, including rendered Overview/Edge/HUD account verification states and retry wording. |
| `npm.cmd run build` | Passed: TypeScript and Vite production build. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked protocol::tests -- --nocapture` | Passed: 13 protocol tests, live read-only gate ignored. Native PowerShell fixtures cover early/quota-stage/account-after hints, repeated hints, a hint during the reconciliation account-before call, same-email workspace changes, missing identity/email, sign-out, unsupported auth, malformed data, timeout and failed reconciliation. |
| `npm.cmd run rust:fmt` / `rust:check` / `rust:clippy` | Passed, including maintained dependency formatting and warnings-denied Clippy. Cargo emitted incremental-cache hard-link fallback warnings. |
| `npm.cmd run rust:test` | Passed: 179 application tests, one live gate ignored, plus 3 maintained dependency tests. Process-local `TEMP`/`TMP` were set to `src-tauri/target/validation/test-temp`; no machine/global environment changes. Includes protocol-to-service publication tests and verified/unverified transport retention/backoff tests; existing missing-field, scheduler, generation, reminder and storage tests also pass. |
| `npm.cmd run desktop:build` | Blocked by the previously recorded installed Tauri native binding: “not a valid Win32 application.” No tooling was installed or upgraded. |
| `cargo build --manifest-path src-tauri/Cargo.toml --locked --features tauri/custom-protocol --bin Usage` | Passed: debug `src-tauri/target/debug/Usage.exe` with the current production frontend embedded, bypassing the broken Tauri CLI binding. |
| Embedded-frontend debug `Usage.exe --check-native-diagnostics` | Passed with the isolated validation identifier/configuration and synthetic data. Verified pending/active account verification and recovery in HUD, Edge and Overview, plus prior loading/stale/recovery, diagnostics privacy/toggle and shell recreation checks. The process exited normally with code 0. Results are in `src-tauri/target/validation/native-diagnostics/result.log`; stdout/stderr are under `src-tauri/target/validation/account-recheck/`. |

An initial new fixture failed because an unlabeled PowerShell `continue` exited its `switch` rather than the input loop and emitted an unsolicited response. Labeling that fixture loop fixed the harness; the production response-ID validation was retained. The corrected fixture and full suite passed afterward. Native teardown printed a system-tray removal message and the previously observed WebView2 unregister-class error 1412 after the successful assertions; these remain separate lifecycle observations.

Remaining validation: a long-running live post-fix log comparison, installed/packaged execution, real account/workspace changes, sleep/resume and mixed-DPI testing. The provided logs are pre-fix evidence, not post-fix measurements. No live Codex probe, credential access, inference request, installer or release publication was performed.

## Validation of this note

The initial investigation inspected source branches, the pinned notification schema, and official documentation. This follow-up inspected the supplied log against the current protocol, service, scheduler and rendering code.

Commands run from the repository root:

- `powershell.exe -NoProfile -File scripts/summarize-diagnostics.ps1 -Directory scripts` — passed; 1,958 records, no malformed lines or reported drops.
- `powershell.exe -NoProfile -File scripts/summarize-diagnostics.ps1 -Directory scripts -AsJson` — passed; exact runtime durations above come from completed native episodes, without adding overlapping frontend durations.
- Read-only PowerShell correlations by process/read ID and sequence number — confirmed the 63-before/10-after notification split, no stage errors, and all nine runtime entries following an account-change error with empty quota windows.
- Local Markdown link-target checks and `git diff --check` — passed for the documentation update.

The October 30 investigation changed documentation only. Implementation and synthetic test evidence from November 4 are recorded separately above; supplied pre-fix runtime evidence must not be represented as a post-fix test.
