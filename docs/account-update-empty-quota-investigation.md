# Account-update notifications and empty quota values

Updated: 2026-10-30. Status: quota-clearing mechanism confirmed in the supplied diagnostic log; notification timing race strongly supported; fix not implemented.

## Finding from the supplied log

The local-only `scripts/widget.jsonl` confirms that the app repeatedly clears previously usable quota values after receiving `account/updated` during a read. All nine observed runtime empty-data episodes follow `account-changed-during-read`. Every recorded initialize/account/quota protocol stage completed without an error; the transaction rejected the observation afterward. This is an application account-verification/clearing path, rather than evidence that the quota endpoint returned no values or that a network timeout occurred.

Notification timing perfectly separates the recorded outcomes: all 63 successful transactions received the notification before the first account response, while all 10 rejected transactions received it after that response. The flag is reset at that boundary in the current code. This strongly supports a delayed notification causing unnecessary invalidation. The privacy-preserving log cannot prove that the underlying identities matched or establish why Codex emitted each notification; an actual account change remains indistinguishable in these records.

The current `read_cycle` launches a fresh app-server for every refresh and shuts it down afterward. Every recorded read therefore includes process initialization. The one-notification-per-read pattern is consistent with a startup-related notification, but the log does not establish its origin.

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

## Current behavior found in source

The following describes the working tree inspected on the recorded date. The supplied log matches this path; it does not establish behavior in every released build.

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

## Problem and remaining uncertainty

The widget treats an account-state notification received after the first account check as sufficient to reject the whole observation and clear useful last-known values. The exact same notification received earlier is forgotten by resetting the flag. The observed timing split strongly indicates this policy is causing avoidable empty views. Generic retry wording and suppression of the prominent error notice then make the loss look like ordinary loading.

Account safety still matters: clearing or hiding old values is appropriate when the account changes or its binding cannot be verified. Matching the current before/after comparison is insufficient evidence by itself: `account_identity` currently compares account type eligibility and email, with a fallback when email is absent. It does not prove a stable account/workspace binding.

## Recommended correction (not implemented)

- Separate an account-update hint from a confirmed account/binding change. The recorded before/after timing split supplies a concrete regression case; reproduce both notification orders with synthetic protocol fixtures before changing the policy.
- Revalidate the account context after a hint. Define how to establish continuity using supported stable identity fields, including workspace context where available; verify optional fields against the versioned schema. Do not rely solely on matching email or plan.
- When continuity is verified, preserve previous validated values as explicitly stale while a follow-up read reconciles the update. Keep Rust authoritative and use the existing shared scheduler and generation handling.
- When the binding changes, sign-out occurs, or continuity remains uncertain, hide or clear previous-account values and suspend account-bound reminders. Never expose old-account quota as belonging to a new account.
- Distinguish active checking, waiting for retry, and account verification in Overview. Make the reason for empty values discoverable without requiring the user to infer it from generic loading wording.
- Preserve the existing missing-field confirmation policy and bounded retry behavior. A notification must not create extra clients, overlapping reads, optimistic recovery claims, or a retry loop.

These are implementation directions, not completed changes or an approved identity-verification algorithm. Removing `|| self.account_changed` alone would bypass the current account-change protection without proving continuity and is not an adequate correction.

## Evidence and validation for the future fix

The supplied file establishes app version, error code, notification ordering and subsequent native/shell state transitions. For implementation, still establish the CLI contract and a supported way to verify account/workspace continuity. Any additional diagnostics should report only safe comparison outcomes (for example, hint observed versus identity mismatch), never credentials, identifiers or raw responses. The current error code combines these causes, so the log cannot establish identity equality.

Add focused protocol/service tests using synthetic fixtures for:

- An update hint with a verified unchanged binding: retain stale values until a validated replacement arrives.
- A confirmed account/workspace change, sign-out, and an unverifiable binding: prevent previous-account values and reminders from crossing the boundary.
- Missing identifiers or email: avoid claiming identity continuity from the fallback comparison.
- A temporary transport failure with and without existing values: preserve stale data where safe and show an accurate retry state.
- Repeated update hints and overlapping refresh triggers: coalesce reads, bound retries, and reject obsolete-generation results.
- Missing/invalid quota fields: retain values for the established two-observation grace period and clear them on the third as specified in the [design report](codex-widget-design-report.md).

Run the frontend build/policy tests and applicable Rust formatting, compilation, and behavioral checks after implementation. Perform Windows runtime checks in Overview, Edge, and HUD for the visible transitions. Record results and remaining limitations here when the fix is implemented.

## Validation of this note

The initial investigation inspected source branches, the pinned notification schema, and official documentation. This follow-up inspected the supplied log against the current protocol, service, scheduler and rendering code.

Commands run from the repository root:

- `powershell.exe -NoProfile -File scripts/summarize-diagnostics.ps1 -Directory scripts` — passed; 1,958 records, no malformed lines or reported drops.
- `powershell.exe -NoProfile -File scripts/summarize-diagnostics.ps1 -Directory scripts -AsJson` — passed; exact runtime durations above come from completed native episodes, without adding overlapping frontend durations.
- Read-only PowerShell correlations by process/read ID and sequence number — confirmed the 63-before/10-after notification split, no stage errors, and all nine runtime entries following an account-change error with empty quota windows.
- Local Markdown link-target checks and `git diff --check` — passed for the documentation update.

No application code was changed, no application tests/build or live Codex probe was run, and no new executable was launched. Runtime evidence comes from the supplied file. A corrected policy, synthetic race reproduction, and before/after Windows verification remain pending.
