# Implementation handoff

Updated: 2026-10-23. The earlier work session stopped at the user's request; implementation remains incomplete. The interaction-consistency follow-up is now implemented in this worktree and its validation is recorded in the [implementation report](implementation-report.md). Resume from this worktree, not from the original scaffold.

Start with [the notification error investigation](notification-error-investigation.md) and [the maintained implementation report / AC-01–AC-40 matrix](implementation-report.md). The canonical specification remains [the design report](codex-widget-design-report.md); setup and build commands are in [README](../README.md).

This is the historical stopping record. For later widget interaction/countdown changes, see [the implementation report](implementation-report.md); for layout polish, the compact client-area sizing correction, current checks and latest executable, see [the layout-polish report](layout-polish-implementation-report.md).

## Current outcome

The main usage-monitoring path is implemented and a real read-only Codex 0.163.0 handshake/account/quota read previously passed. Rust owns process supervision, normalization, shared refresh scheduling, authoritative snapshots, settings, appearance, reminder policy and storage. React consumes that shared state. Missing/stale data is distinguished, reset expiry does not prove recovery, and no inference requests or credit redemption are used.

All four modes are implemented: Floating, Screen Edge, System Tray (default) and Compact HUD. The tray remains the control/recovery surface. Compact tray hover, full detail, window promotion, shared temporary preview for Floating/Edge/HUD, both quota values in the Edge glance, dragging/placement, fullscreen suppression, Light/Dark/Follow Windows, lazy webviews, single instance and opt-in autostart are present. Widget and tray activation select Overview; the explicit Settings action selects Settings. Policy/native coordinator checks cover parts of these behaviors; actual input, accessibility and supported-Windows coverage remain incomplete.

Reminder thresholds, reset/recovery policy, quiet hours, snooze, schedules, account isolation, durable claims/deduplication and history are implemented. **Native delivery does not work in the tested installed build**: `ToastNotifier.Setting()` returns `0x80070490`, availability stays Unknown and no test toast is claimed or queued. Four installed attempts reproduced it after identity/registration fixes. The investigation report contains exact commands, evidence and unproven hypotheses. Do not describe notifications, any full phase, or the release as complete.

## Repository state

Latest commit: **`4f8fcaa build(windows): package and validate per-user NSIS upgrades`**. Recent prior commits:

- `283f942 feat(lifecycle): guard launches and reconcile opt-in autostart`
- `06224b3 test(windows): measure repeated native mode transitions`
- `148877a fix(core): reconcile freshness and episode reset deadlines`

The current notification increment and this handoff are **uncommitted**. No commit was made while stopping. Existing changes have been preserved. Run `git status --short` and inspect diffs before editing or staging.

Current application changes include the Rust notification adapter/permission diagnostics, explicit process AppUserModelID, installer-owned display/icon registration, isolated installed native check, package-check extensions, durable manual Test category, storage envelope format 3 and corresponding frontend diagnostics. Settings schema remains 4. The adapter reuses the already-resolved Windows bindings; it adds no JavaScript plugin permissions.

Changed implementation paths:

- `.gitignore`, `scripts/check-package.ps1`
- `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`
- `src-tauri/src/lib.rs`, `native_windows.rs`, `native_check.rs`, `notification.rs`, `service.rs`, `reminders.rs`, `storage.rs`
- New `src-tauri/installer/hooks.nsh` and `src-tauri/icons/notification.png`
- `src/ReminderSettings.tsx`, `src/state.ts`
- Documentation listed in `git status`, including the implementation report and these two handoff/investigation documents

No touch-option, global-shortcut or network-recovery implementation was started before stopping. The PNG is an export of the existing scaffold icon, not final product artwork.

## Checks actually completed

The following were rerun successfully on the **final current application source** before stopping; only documentation changed afterward:

| Command | Result |
| --- | --- |
| `npm.cmd run build` | Passed TypeScript checking and Vite production build |
| `npm.cmd test` | Passed all 10 Node tests |
| `npm.cmd run rust:fmt` | Passed, including modified vendored autostart policy files |
| `npm.cmd run rust:check` | Passed with locked dependencies |
| `npm.cmd run rust:clippy` | Passed all targets with warnings denied |
| `npm.cmd run rust:test` | 73 app tests passed, 1 explicit live probe ignored; 3 vendored dependency policy tests passed |
| `npm.cmd run desktop:package -- --debug` | Passed after the current process-identity and installer-hook additions |
| Installed `package:check ... -NativeCheck Notifications` | **Failed** at availability query; install/identity/state preservation/uninstall cleanup passed; preceding coordinator assertions passed |

Rust tests ran with normal Windows filesystem access because the restricted overlay cannot validate atomic replacement correctly. The live probe was not rerun in this stopping pass; its earlier positive evidence is in the implementation report. Source tests do not establish Windows toast, input, focus, theme or accessibility acceptance.

Earlier committed packaging validation passed real per-user install, same-version reinstall, 0.1.0→0.1.1 fixture upgrade, downgrade rejection and uninstall while preserving existing state. Earlier native stress passed 100 mode transitions, one reused shell and final zero webviews. These are historical milestone results with the limitations recorded in the main report, not proof of current notification delivery or required idle budgets.

All checks started for this stopping pass have finished. No validation session remains active. The last installed fixture was uninstalled, its owned identity metadata removed, existing state preserved and zero owned processes observed. A final name-based process query also found no widget. The WebView2 shutdown class-unregistration warning 1412 remains unresolved.

## Build artifacts

Artifacts remain under ignored `src-tauri/target/`; do not commit or distribute them as a finished release.

| Artifact | Meaning |
| --- | --- |
| `src-tauri/target/debug/bundle/nsis/Codex Usage Widget_0.1.0_x64-setup.exe` | Current unsigned debug installer containing the unresolved notification increment |
| `src-tauri/target/debug/codex-usage-widget.exe` | Last native/debug build; Cargo test executables are separate. Rebuild before native checks if source changes |
| `src-tauri/target/validation/package-fixtures/notification-debug-setup.exe` | Copy used for the installed notification reproduction |
| `src-tauri/target/release/codex-usage-widget.exe` | Earlier initial-test build, before the interaction-consistency follow-up; the current standalone executable and hash are in the main report |
| `src-tauri/target/release/bundle/nsis/Codex Usage Widget_0.1.0_x64-setup.exe` | Current unsigned release-profile installer rebuilt for initial testing on 2026-10-23; not installed or runtime-validated in that build task |
| Version 0.1.1 installers/executables under target/validation or bundle output | Upgrade-test fixtures only; not a product release and not current notification/metadata upgrade evidence |

The main report separates historical artifact hashes from the current initial-test build hashes. `desktop:build` deliberately creates no installer; `desktop:package` is separate. Rebuild release and fresh upgrade fixtures after fixing notifications. Older uninstallers lack the new metadata hook, so do not reuse old fixtures to claim current metadata upgrade/cleanup correctness.

## Remaining work and suggested ownership

1. **Notification investigator:** use the focused error report. Establish supported classic desktop availability/registration semantics, fix the smallest proven cause, rerun the guarded installed check, then validate actual toast privacy/silence/sound/focus/denial/re-enable and conservative outcomes. Do not silently treat unknown as enabled or add fake COM activation.
2. **Core reliability:** add injected actor/transport overlap, generation and crash/save-boundary tests; validate account/workspace switches, missing stable ID behavior, network recovery, clock/timezone/resume and any actual protocol Retry-After representation. The current schema does not establish a retry-delay field.
3. **Accessibility/product gaps:** implement the required 44-pixel touch option and optional configurable global Show dashboard shortcut (disabled by default, conflicts reported), and audit the remaining specification against source. The requested local-data reset flow, distinct from Clear history, also needs implementation/verification. Validate keyboard focus/dialog drafts, screen readers and polite status announcements, text scaling/high contrast/reduced motion and every mode/state in both themes.
4. **Native/lifecycle:** real pointer hover/corridor/menu timing and tooltip fallback, dragging, mixed DPI/missing monitors, fullscreen/lock/resume, Explorer restart/overflow/auto-hide, real-time five-minute recycling, actual autostart/external disablement/login, and shutdown warning investigation. Coordinator injection is insufficient for these gates.
5. **Release:** required ten-minute idle tray/detail process-tree CPU/memory measurements, listener accounting and plateau checks; current release/upgrade hydration and storage-capacity failures; installer UI, absent-WebView2/offline path, signing/branding and Windows 10 x64 coverage. ARM64 is unclaimed.

The full acceptance matrix remains the source of truth. No whole phase is complete. The current Windows Computer Use helper failed with native pipe unavailable (`os error 2`), so rendered/native input checks are blocked until that tool works or a human supplies the required evidence. Do not bypass it with shell UI automation or change machine configuration.

## Constraints for resumption

Read `AGENTS.md`, README, the canonical design report, setup and relevant pinned schemas before changing a subsystem. User approval already covers the pinned Tauri single-instance/autostart/notification plugins and Tauri-managed NSIS tools; do not request that same approval again. Other installations still follow README approval rules. No global tooling, shell-profile, security or OS-notification-setting workaround was applied.

Keep four modes, the Rust state owner and narrow IPC. Preserve unrelated work. Never read/copy credential files, log account identifiers/private usage responses, run inference, redeem credits, or automatically replay uncertain notification claims. No agent was spawned; this handoff is ready for the user's assigned agents.
