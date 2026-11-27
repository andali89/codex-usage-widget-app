# Windows notification investigation

Updated: 2026-11-04. **Native delivery remains unresolved.** This report covers the current 0.1.5 source, locked notification dependencies, and portable/installed packaging. No application code, notification registrations, or Windows settings were changed during this investigation. Earlier installed-runtime evidence is preserved below and was not reproduced in this pass.

## Summary and implementation

**Portable notifications are explicitly blocked by the application. Installed notifications have a separate unresolved Windows availability-query failure.** Removing the portable gate alone would not complete the feature.

Rust owns reminder evaluation, suppression, scheduling, and persistence. It evaluates low-quota thresholds, approaching resets, confirmed recovery, and scheduled check-ins; applies freshness/account identity, quiet hours, snooze, session-lock/fullscreen, coalescing, and cooldown rules; and persists claims before submission. React controls this shared service through narrow commands. Defaults are reminders off, silent sound, and generic text.

The delivery path is `reminders.rs` → durable claim in `service.rs` → `notification.rs` → Tauri notification plugin 2.7.0 → `notify-rust` 4.18.2 → `tauri-winrt-notification` 0.8.1 → Windows WinRT. These are ordinary local notifications, not cloud push or browser notifications.

## Confirmed findings and root causes

| Finding | Evidence and consequence |
| --- | --- |
| Portable submission is disabled in code | [`notification.rs`](../src-tauri/src/notification.rs) requires an NSIS bundle marker, a matching HKCU installation path/running executable, and a Start menu shortcut. An extracted or relocated copy fails this gate and reports `notifications-install-required`. |
| Portable packaging performs no registration | The release workflow archives `Usage.exe`, `notification.png`, and `LICENSE`. The image alone does not establish a notification identity. |
| Installed availability previously failed | The October 23 installed debug check created a notifier successfully, then `ToastNotifier.Setting()` threw `0x80070490` (`ERROR_NOT_FOUND`). The adapter maps this to Unknown and stops before claiming/submitting. This remains historical runtime evidence, not a new reproduction. |
| Framework permission checks cannot establish availability | Cached locked plugin source confirms that desktop permission methods always return Granted. Missing JavaScript notification permissions are not the blocker: submission originates in Rust. |
| The plugin hides submission errors | Its ordinary send path prepares the payload, queues background work, and discards the eventual Windows result. The application correctly records queued attempts as Outcome unknown and does not blindly resend. |

The process already sets explicit AppUserModelID `dev.codexusagewidget.desktop`. NSIS creates the installed shortcut, and installer hooks add current-user display/icon metadata. Earlier checks verified these elements; they did not resolve the installed query failure. NSIS installation does not provide MSIX package identity.

For this classic WinRT path, Windows requires a Start menu shortcut with the matching AppUserModelID; process identity alone does not satisfy that registration requirement. Microsoft supplies programmatic shortcut creation but recommends installer-managed registration. Tauri documents installed-app support on Windows and does not provide portable registration. [Microsoft registration requirements](https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid), [Tauri notification guide](https://v2.tauri.app/plugin/notification/).

## Unconfirmed causes

The underlying Windows cause of the installed `Setting()` exception is unresolved. Microsoft's documentation issue tracker reports the same HRESULT, including shortly after installation; a contributor describes it as a platform failure. This supports investigating initialization/registration timing, but does not prove that explanation here. [Microsoft issue #1431](https://github.com/MicrosoftDocs/windows-dev-docs/issues/1431).

Windows notification switches, policy, Focus Assist/Do Not Disturb, and banner suppression require runtime verification. Unsigned binaries and a missing COM activator have not been established as causes of basic banner failure. The earlier installed check ran in an interactive, non-elevated session, so elevation was not supported as its cause. No system configuration changes are recommended as an investigative workaround.

## Feasible solutions

| Approach | Compatibility, complexity, and maintenance |
| --- | --- |
| Retain Tauri plugin | Smallest installed-build change, but portable registration and hidden submission errors remain unresolved. |
| **Direct Rust WinRT adapter — recommended** | Reuses existing Windows bindings and suits ordinary reminders. Requires maintaining per-user registration, native callbacks, and owned-entry cleanup. |
| Windows App SDK `AppNotificationManager` | Microsoft's current recommended desktop API, including unpackaged apps. Adds runtime deployment and Rust projection/native-bridge integration work. |
| MSIX distribution | Provides managed package identity but changes distribution and does not preserve the current extracted-ZIP experience. |

Windows App SDK supports local notifications without package identity and supports self-contained deployment. It is a credible alternative if broader activation features become necessary. Its deployment requirements should be checked against the selected SDK version; the current deployment guide distinguishes local app notifications from push APIs requiring the Singleton package. [API guidance](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/), [self-contained deployment](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/self-contained-deploy/deploy-self-contained-apps).

## Recommended implementation steps

1. Replace the plugin submission boundary with direct WinRT `Show()` results and `Failed` callbacks. Preserve durable claims, conservative outcomes, and no automatic resend of uncertain submissions. A successful submission must never mean the user saw a banner.
2. Replace installation-only eligibility with validated notification registration. Preserve the installed identity; give portable registration a separate stable identity and owned per-user shortcut. Repair relocated executable paths and provide explicit cleanup without modifying installed-app registrations.
3. Separate registration readiness, permission status, and submission outcome. Preserve Unknown and known OS denial states. Do not simply turn Unknown into Enabled.
4. First validate a user-triggered Test submission after registration validation when `Setting()` throws. Establish first-send and subsequent-query behavior on Windows 10/11 before permitting automatic delivery under that condition. If unresolved, keep an explicit unavailable diagnostic.
5. Validate installed/portable first launch, relocation, upgrades, denial/re-enable, silent/opt-in sound, privacy, deduplication, focus behavior, and cleanup. Update this report and the [implementation report](implementation-report.md) with actual runtime evidence before claiming reliable delivery.

Portable support would leave a small per-user registration footprint outside the extracted folder. Windows controls banner visibility; reminders still require the app running and Windows awake. Notification Center activation and launching after Quit should remain outside this correction.

## Validation in this investigation

| Command | Result |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline notification::tests` | Passed: 4 tests covering permission mapping, generic/silent privacy defaults, formatting, and test content. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline reminders::tests` | Passed: 18 tests covering thresholds, suppression, schedules, persistence, recovery, and deduplication. |

Cargo reported incremental-cache hard-link warnings and copied files instead; both commands completed successfully. These tests establish policy/payload behavior, not native delivery. No notifications were sent, installers run, registrations changed, or live Codex probes executed in this pass. The final documentation-only save does not require an application rebuild.

<details>
<summary>Historical installed investigation and reproduction evidence — 23 October 2026</summary>

The following record preserves the earlier environment, commands, attempted fixes, and outcomes. Version numbers, executable names, worktree state, and validation counts describe that earlier milestone and must not be treated as current instructions. See the [implementation handoff](implementation-handoff.md) for its original context.

## Observed failure

The installed debug build cannot determine Windows notification availability. `ToastNotificationManager::CreateToastNotifierWithId` succeeds, but **`ToastNotifier.Setting()` throws HRESULT `0x80070490` (`ERROR_NOT_FOUND`)**. The adapter returns `Unknown`; the test stops before claiming or queuing a notification. No toast appearance, sound, OS acceptance, or Notification Center behavior has been validated.

Latest structural stderr:

```text
native-check notification-status stage=read-setting hresult=80070490
native-check notification-status stage=read-setting hresult=80070490
native-check result=failed code=notifications-status-unknown
[1023/113053.478:ERROR:ui\gfx\win\window_impl.cc:172] Failed to unregister class Chrome_WidgetWin_0. Error = 1412
```

The WebView2 shutdown warning is a separate unresolved observation. There is no evidence that it causes the earlier notification query failure. The wrapper observed zero owned processes before fixture cleanup.

## Historical reproduction

At that milestone, notification changes were uncommitted on top of packaging commit `4f8fcaa`; the earlier installer did not contain them. These historical artifact names must be updated before reproducing against a later version.

From the repository root, close the widget and run:

```powershell
npm.cmd run desktop:package -- --debug
Copy-Item -LiteralPath 'src-tauri/target/debug/bundle/nsis/Codex Usage Widget_0.1.0_x64-setup.exe' -Destination 'src-tauri/target/validation/package-fixtures/notification-debug-setup.exe'
npm.cmd run package:check -- -Installer src-tauri/target/validation/package-fixtures/notification-debug-setup.exe -Executable src-tauri/target/debug/codex-usage-widget.exe -NativeCheck Notifications
```

The fixture directory already exists in this workspace; create it under `src-tauri/target/validation/` if resuming in a clean checkout. These commands build an unsigned debug NSIS package and perform a **real temporary current-user install**, launch its debug check, and uninstall. Use normal Windows filesystem/session access; the restricted overlay has previously hidden existing app-data state. The checker refuses pre-existing installation/shortcut/startup conflicts. Do not bypass those guards or uninstall an unrelated installation.

Expected current result: package command exits 1 after the notification gate fails. The installer, matching executable bytes, shortcut target/AppUserModelID, current-user display/icon registration, production-state preservation, and uninstall cleanup checks pass. Native assertions before the failure include the process AppUserModelID, no-focus preview, four modes, zero-webview survival, duplicate-process routing and atomic snooze persistence. The last native stdout assertion is `notification-test-safe-preferences=pass`; **there is no overall native pass marker**.

Logs are ignored build outputs:

- `src-tauri/target/validation/package-native.stdout.log`
- `src-tauri/target/validation/package-native.stderr.log`

The debug check selects the isolated `native-window-check` data directory before service creation and resets only its reminder preferences/ledger to master off, generic text and silent sound. It uses normal read-only Codex monitoring. The package checker compares production state bytes without printing contents or digests. All observed owned processes were stopped and the fixture was uninstalled after the last run; the current-user identity metadata was removed and existing state bytes were preserved.

## Tested environment

| Component | Observed value |
| --- | --- |
| Windows | Windows 11 x64, 25H2, build 26200.9711 |
| Tauri | Rust 2.12.5; CLI/API 2.12.4 |
| Notification plugin | 2.7.0 |
| Windows Rust bindings | 0.62.2, already present transitively and now reused directly |
| WebView2 | 155.0.4283.62 |
| Toolchain | Rust/Cargo 1.99.1 MSVC; Node 24.17.0; npm 11.13.0 |
| Installer | Tauri-managed NSIS 3.13; helper 0.5.3; current-user mode |
| Shell session | `ElevatedAdministrator=false`, `SessionId=2`, `Interactive=true` |

The last row was measured with a read-only normal-access process-token/session check. Elevation is not supported as the cause by current evidence. No account name, SID, credential, private usage response, or notification-setting value was logged.

## Relevant implementation

| File | Responsibility / investigation point |
| --- | --- |
| `src-tauri/src/notification.rs` | Rust adapter, installed-app gate, WinRT initialization, notifier creation and failing `Setting()` query; bounded stage/HRESULT diagnostics only for the explicit debug gate |
| `src-tauri/src/native_windows.rs` | Sets the process explicit AppUserModelID; debug query verifies it matches configuration |
| `src-tauri/src/lib.rs` | Applies process identity before creating Tauri UI; attaches the authoritative service |
| `src-tauri/installer/hooks.nsh` | Current-user app display/icon metadata; owned-entry cleanup after uninstall |
| `src-tauri/tauri.conf.json` | Installer hook and installed notification icon resource |
| `scripts/check-package.ps1` | Guarded installed debug check, shortcut/identity checks, state preservation and cleanup |
| `src-tauri/src/native_check.rs` | Isolated safe preferences, installed test action and durable-result assertions; currently stops before the test claim |
| `src-tauri/src/service.rs` | Shared notification availability reconciliation, claim-before-submit transaction and conservative result storage |
| `src-tauri/src/reminders.rs`, `storage.rs` | Generic manual Test category, cooldown, no test retries; envelope format 3 migration |

The installed gate requires the NSIS bundle marker, a matching HKCU uninstall `InstallLocation`/running executable, and the installed Start menu shortcut. The package checker independently reads the shortcut's `System.AppUserModel.ID` and verifies the installer-owned display/icon metadata.

`RoInitialize(RO_INIT_MULTITHREADED)` succeeds or tolerates an existing STA (`RPC_E_CHANGED_MODE`). Successful initialization is paired with `RoUninitialize` after the notifier is dropped. Instrumentation distinguishes initialization, notifier creation and reading `Setting`; the failure is specifically the last stage.

The pinned Tauri plugin's desktop permission implementation unconditionally reports Granted. Its `show()` prepares work and spawns an asynchronous task without returning the eventual Windows result. Therefore the adapter queries native availability and records a successful plugin queue as **Outcome unknown**, consumes its keys and never blindly resends. A synchronous preparation failure is definitely not submitted. This distinction must survive any fix.

Inspect cached sources for `tauri-plugin-notification-2.7.0/src/desktop.rs`, `notify-rust-4.18.2/src/windows.rs`, and `tauri-winrt-notification-0.8.1/src/lib.rs` to verify these boundaries. Omitted sound reaches silent toast audio in the pinned backend; explicit sound uses Default. That is source evidence, not an audible/visual runtime pass.

## Attempts and results

| Attempt | Evidence / outcome |
| --- | --- |
| Installed NSIS app with matching shortcut identity | Adapter still returned unknown; no notification claimed |
| Stage-specific HRESULT instrumentation | Located `0x80070490` at `ToastNotifier.Setting()`; notifier creation succeeds |
| Explicit process AppUserModelID before UI creation | Native identity assertion passes; same query error persists |
| Installer-owned HKCU `Software\Classes\AppUserModelId` display name/icon metadata | Checker verifies metadata and installed icon; same query error persists |
| Check elevation/session rather than assume an elevation problem | Normal interactive, non-elevated session observed |

Current-user metadata consists only of `DisplayName`, `IconUri` and an ownership marker. It does not register a COM activator, alter notification permissions/policy, or enable launch-after-quit. The icon is a PNG export of the existing scaffold ICO; final product artwork/DPI review remains open. Uninstall deletes only matching installer-owned metadata, after cancellation checks.

Four installed attempts reproduced the unknown-availability failure. Repeating the same registration changes without new evidence is unlikely to resolve it.

## Evidence and questions for the next investigator

Use the canonical design report's ordinary-notification fallback and narrow Rust boundary. Investigate these hypotheses; **none is established**:

1. Is `ToastNotifier.Setting()` reliable for this classic unpackaged desktop/shortcut registration before the first toast? A mandatory availability check could be circular if Windows has not materialized its notification record. Establish supported behavior before changing the gate.
2. Is shell registration/indexing asynchronous after NSIS installation? A bounded wait/recheck has not been tested; avoid sleeps presented as proof of correct registration.
3. Does this API require additional supported classic-desktop registration on this Windows build? Modern Windows App SDK registration includes other machinery; copying internal registry keys or inventing an unimplemented COM activator is not validated compatibility work.
4. Is this user session's notification platform unavailable independently of the app? Read-only capability evidence is appropriate; do not change services, privacy/policy settings, UAC or machine configuration as a workaround.

Primary references already consulted:

- [Tauri notification guide](https://v2.tauri.app/plugin/notification/) — installed Windows app requirement.
- [Microsoft ToastNotifier.Setting](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotifier.setting?view=winrt-26100) and [NotificationSetting values](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.notificationsetting?view=winrt-26100) — availability API/enum, not proof this registration works.
- [Process explicit AppUserModelID](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-setcurrentprocessexplicitappusermodelid) and [desktop shortcut identity](https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid).
- [Classic desktop toast quickstart](https://learn.microsoft.com/en-us/windows/win32/shell/quickstart-sending-desktop-toast) — registration context; action/activation examples do not expand the product requirements.
- [Windows App SDK registration source](https://github.com/microsoft/WindowsAppSDK/blob/main/dev/AppNotifications/AppNotificationManager.cpp) — a different notification stack; comparison only, not instructions to install it or copy its internals.

Do not relax Unknown into Enabled merely to pass the check. If supported evidence justifies an attempted delivery with unknown permission, document the product/safety consequence and retain conservative outcomes and no automatic resend. Known application/user/policy/manifest denial must still suspend delivery without a prompt/retry storm. No toast actions, credit use, inference requests or credential access are authorized by this investigation.

## Passing checks and remaining work

Final current-source frontend build and ten Node tests passed. Rust format/check/Clippy and **73 app tests plus 3 dependency policy tests** passed, with the explicit live probe ignored; these were rerun after the final process-identity/installer-hook additions. Debug NSIS builds also pass after those additions. No passing source check substitutes for installed notification delivery. All checks started in this session have finished.

After fixing the query, rerun the guarded installed test and then validate actual toast appearance, generic privacy, silent default/opt-in sound, no focus theft, OS denial/re-enable, cooldown/deduplication, Notification Center limitations and cleanup. Rebuild current release artifacts and fresh version-upgrade fixtures; earlier packaging milestone artifacts predate this notification work. Windows 10, screen-reader/input/DPI, actual autostart and ten-minute resource gates remain in the main report.

Next concrete action for an assigned investigator: reproduce the failing `Setting()` call in the installed debug gate, determine supported classic-desktop availability semantics, and propose the smallest evidence-backed adapter/registration correction. Other implementation can proceed without modifying the notification boundary.

</details>
