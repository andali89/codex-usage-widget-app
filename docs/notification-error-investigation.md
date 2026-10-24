# Installed Windows notification failure

Updated: 2026-10-23. **Unresolved; handoff for investigation.** Work is stopped at the user's request; see [the implementation handoff](implementation-handoff.md). This is a focused companion to the [implementation report](implementation-report.md), not a replacement for its checklist or AC-01–AC-39 matrix.

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

## Reproduce with the current worktree

The notification changes are **uncommitted**, on top of packaging commit `4f8fcaa`. Preserve the current worktree. Do not test the earlier release installer and assume it contains these changes.

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
