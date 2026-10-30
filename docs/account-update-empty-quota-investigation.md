# Potential account-update clearing of quota values

Recorded: 2026-10-30. Status: suspected cause identified through source inspection; runtime cause unconfirmed; fix deferred.

## Reported symptom

During normal use, Overview sometimes shows **Rechecking local usage…** while Screen Edge or Compact HUD shows empty quota placeholders instead of previous percentages marked **Last known · out of date**.

The user reported this behavior. No reproduction, error detail, or diagnostic log from the affected occurrence was captured in this investigation. The exact application build involved has not been established.

## Current behavior found in source

The following describes the working tree inspected on the recorded date, including ongoing diagnostic-logging changes. It does not establish behavior in every released build.

1. In [`protocol.rs`](../src-tauri/src/protocol.rs), receiving `account/updated` sets `account_changed = true`. During a read transaction, this flag is cleared after the first `account/read`, before reading quota values.
2. After the quota read and a second `account/read`, the transaction returns `account-changed-during-read` if the account comparison differs **or** the flag is set. The flag alone rejects the result even when the compared account information matches.
3. In [`service.rs`](../src-tauri/src/service.rs), that error invalidates reminder context, increments the connection generation, and replaces usage with an empty default. It then sets the connection to `offline` and schedules a retry.
4. In [`summary.ts`](../src/summary.ts), `usagePending` is true while the consecutive unavailable-read count is below three, unless the connection is `auth-required`, `unsupported`, or `paused`.
5. In [`UsagePage.tsx`](../src/UsagePage.tsx), an offline state with no refresh running, no overall stale flag, and `usagePending = true` produces **Rechecking local usage…**. The prominent error notice is suppressed during this pending/offline state; the diagnostic explanation remains in Read details.
6. [`CompactQuotaReadings.tsx`](../src/CompactQuotaReadings.tsx) renders empty placeholders when the selected periods have no retained percentage values.

This sequence can produce the reported combination immediately after previously visible quota values. A temporary transport failure also produces this combination when values were already empty, such as after startup or a connection reset. A transport failure with existing quota windows ordinarily preserves stale values.

The third consecutive missing/invalid quota observation is a separate clearing policy. Three consecutive unavailable reads generally end the pending wording, so that policy alone does not explain this exact combination.

## Meaning of `account/updated`

The [official OpenAI app-server documentation](https://learn.chatgpt.com/docs/app-server) describes `account/updated` as a server notification about authentication-mode changes and illustrates it after login and logout. It carries the authentication mode and the ChatGPT plan when available. Quota changes have a separate notification, `account/rateLimits/updated`.

The project's [Codex 0.163.0 notification schema](../schemas/codex/0.163.0/v2/AccountUpdatedNotification.json) defines `authMode` and `planType`, with no account identifier. Receiving the notification alone therefore does not establish that the user switched to a different account. Its exact emission circumstances in the user's CLI version still need verification; this investigation does not establish that routine token refresh caused it.

## Potential problem

The widget may interpret an account-state notification as a confirmed account change and unnecessarily discard useful last-known values. The generic retry wording then makes the loss look like ordinary loading.

Account safety still matters: clearing or hiding old values is appropriate when the account changes or its binding cannot be verified. Matching the current before/after comparison is insufficient evidence by itself: `account_identity` currently compares account type eligibility and email, with a fallback when email is absent. It does not prove a stable account/workspace binding.

## Proposed future fix

- Separate an account-update hint from a confirmed account/binding change. Verify the selected CLI contract and actual notification timing before changing the policy.
- Revalidate the account context after a hint. Define how to establish continuity using supported stable identity fields, including workspace context where available; verify optional fields against the versioned schema. Do not rely solely on matching email or plan.
- When continuity is verified, preserve previous validated values as explicitly stale while a follow-up read reconciles the update. Keep Rust authoritative and use the existing shared scheduler and generation handling.
- When the binding changes, sign-out occurs, or continuity remains uncertain, hide or clear previous-account values and suspend account-bound reminders. Never expose old-account quota as belonging to a new account.
- Distinguish active checking, waiting for retry, and account verification in Overview. Make the reason for empty values discoverable without requiring the user to infer it from generic loading wording.
- Preserve the existing missing-field confirmation policy and bounded retry behavior. A notification must not create extra clients, overlapping reads, optimistic recovery claims, or a retry loop.

These are implementation directions, not completed changes or an approved identity-verification algorithm.

## Evidence and validation for the future fix

First capture the affected build/CLI versions and the diagnostic error code at the time placeholders appear. If the build includes optional local diagnostic logging, correlate `protocol_hint` (`account_updated`), `refresh_result` (`account-changed-during-read`), and subsequent usage/display state events. Use redacted event metadata; do not collect credentials, account identifiers, raw usage responses, or quota values in the report.

Add focused protocol/service tests using synthetic fixtures for:

- An update hint with a verified unchanged binding: retain stale values until a validated replacement arrives.
- A confirmed account/workspace change, sign-out, and an unverifiable binding: prevent previous-account values and reminders from crossing the boundary.
- Missing identifiers or email: avoid claiming identity continuity from the fallback comparison.
- A temporary transport failure with and without existing values: preserve stale data where safe and show an accurate retry state.
- Repeated update hints and overlapping refresh triggers: coalesce reads, bound retries, and reject obsolete-generation results.
- Missing/invalid quota fields: retain values for the established two-observation grace period and clear them on the third as specified in the [design report](codex-widget-design-report.md).

Run the frontend build/policy tests and applicable Rust formatting, compilation, and behavioral checks after implementation. Perform Windows runtime checks in Overview, Edge, and HUD for the visible transitions. Record results and remaining limitations here when the fix is implemented.

## Validation of this note

Source branches, the pinned notification schema, and official documentation were inspected. Local Markdown link targets and the documentation index entry were checked. No application code was changed, no application tests or build were run, and the reported runtime occurrence remains unverified.
