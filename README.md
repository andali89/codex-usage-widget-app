# Codex Usage Widget

A Windows desktop utility for monitoring your Codex usage limits, remaining quota, expected reset times, and available reset credits.

**Public preview preparation — version 0.1.0.** Installed notifications remain unresolved, and Windows compatibility/accessibility validation is incomplete. See [known limitations](#known-limitations) before relying on reminders.

This is an unofficial project, not affiliated with or endorsed by OpenAI.

## Features

| Mode | At a glance |
| --- | --- |
| Floating | Quota bars, reset countdowns and available credits in a movable mini-card. |
| Screen Edge | Stacked percentage rings; drag anywhere, then release to dock on the nearest left or right edge of the drop monitor. |
| System Tray | Background monitoring with a compact hover summary and dashboard access. |
| Compact HUD | A movable horizontal strip with quota values, bars and optional reset countdowns. |

- Edge and HUD support Words or circular Badges, with stable dimensions during normal reading changes.
- Edge/HUD hover previews show percentage used and exact expected reset dates; click opens Overview.
- Dashboard details include freshness, expected resets, reset-credit metadata and connection diagnostics.
- Independent Both / 5-hour only / Weekly only choices for HUD and Edge; hover previews retain both periods.
- Optional HTTP/HTTPS proxy configuration for the owned Codex process, without proxy login.
- EXE-adjacent configuration by default, selectable AppData/custom folders and visible automatic fallback.
- Light, Dark and Follow Windows preferences, position lock and optional launch at sign-in.

## Requirements

- Windows x64. Windows 10/11 are the targets; the recorded runtime validation is on Windows 11, and Windows 10 remains unverified.
- A local, signed-in native **Codex CLI** installation with a supported ChatGPT account context. The widget tries the read-only app-server protocol without restricting version numbers or their format; incompatible protocol responses produce an actionable warning with best-effort CLI version information.
- Microsoft Edge WebView2 Runtime 120 or newer. The NSIS installer is configured to offer its download when needed; that path requires internet access and remains a compatibility check.

## Download and install

A GitHub release is being prepared. Once published, download the Windows x64 **setup executable** from this repository's **Releases** page. GitHub's source-code archives are for building the application, not installing it. Release downloads are not included in the source tree.

Run the installer, then open Codex Usage Widget. Authenticate through your local Codex installation first. If automatic executable discovery fails, select the native Codex executable in **Settings → Connection & diagnostics**. Do not paste authentication tokens into the widget.

Until a release is published, developers can build an installer using the instructions under [Check and build](#check-and-build).

The [Windows release workflow](.github/workflows/release.yml) is configured to build an installer, portable ZIP, and checksums into a draft prerelease on pushes to `release`. Its first GitHub-hosted run is pending; see the [release branch guide](docs/github-release-guide.md#github-actions-release-branch-current-automated-path).

To publish your own copy from a new folder, follow the [GitHub publication and release guide](docs/github-release-guide.md). The [publication audit](docs/publication-audit.md) records the source/privacy review and prepared artifacts. `npm.cmd run release:build` creates a public installer with local compiler source paths remapped and a checksum file in `src-tauri/target/release-assets/v<VERSION>/`.

## Using the widget

1. Choose a mode in Settings or through the tray menu. The tray remains the recovery/control surface in every mode.
2. Hover over Edge or HUD for usage/reset details. Floating opens details on click; tray hover uses a smaller summary.
3. Single-click a widget or tray icon for Overview; double-click opens the standard dashboard window. Right-click opens the shared native menu.
4. Drag Floating or HUD to move them. Drag Edge across the desktop and release to dock it; enable Lock position to prevent movement.
5. Closing the dashboard keeps monitoring in the tray. Floating's × switches to System Tray mode. Use **Quit** to stop monitoring.

Missing or stale usage is marked explicitly. A reset countdown reaching zero is **unconfirmed** until fresh usage data arrives; it does not prove quota recovery.

## Privacy and local data

The widget reads account/usage information through the local Codex app-server and keeps authentication with Codex. It does not copy credential files, issue inference requests, or redeem reset credits.

Settings and opaque reminder bookkeeping default to files beside the widget executable. General settings can select AppData or a custom folder; an unavailable preferred location falls back to the EXE folder, then AppData. The active path and recovery warnings are shown in Settings. Existing files are retained during migration, and recovery from an older copy pauses reminders. Displayed quota values are not restored from disk across restarts. The widget does not add a cloud telemetry or quota-upload service; Codex itself communicates with its services as needed.

## Known limitations

- Proxy settings are passed only to the owned Codex process. Child environment behavior is tested, and a live read with the user’s proxy configured passed. Actual traffic routing through that proxy remains independently unverified. See the [proxy/storage/period report](docs/proxy-storage-periods-implementation-report.md).

- **Installed Windows notifications are unresolved.** Do not rely on reminder delivery. See the [notification investigation](docs/notification-error-investigation.md).
- Builds are unsigned; downloaded Windows executables may display SmartScreen warnings. Code signing is not configured.
- Windows 10, real pointer/focus behavior, Explorer recovery, mixed-monitor DPI, accessibility and absent-WebView2 installation still require the documented runtime matrix.
- The current period-control layout passed 104 native sizing cases. Real pointer/focus and accessibility behavior still require interactive checks; earlier visual decisions are recorded in the [layout report](docs/layout-polish-implementation-report.md).
- Existing local installers may predate the latest executable. Build a fresh package from the chosen release commit.

The [implementation report](docs/implementation-report.md) distinguishes implemented features, recorded evidence and remaining work. The [canonical design](docs/codex-widget-design-report.md) includes planned requirements and is not a claim that every acceptance criterion has passed.

## Development

The project uses Tauri 2, Rust, React, TypeScript, Vite and Tailwind CSS. The following setup/build instructions are for contributors. Detailed design and validation records are indexed in [docs/](docs/README.md).

## Verified machine baseline

| Component | Detected version |
| --- | --- |
| Windows | x64, build 26200.9711, 25H2 |
| Rust / Cargo | 1.99.1, `x86_64-pc-windows-msvc` |
| Rust components | rustfmt and Clippy installed |
| Visual Studio Build Tools | 2026, 18.11.1 |
| MSVC | 14.51.36231, x64 compiler and linker |
| Windows SDK | 10.0.26100.0, headers, libraries, and resource compiler |
| Microsoft Edge WebView2 Runtime | 155.0.4283.62 |
| Node.js / npm | 24.17.0 / 11.13.0 |
| Git | 2.44.0.windows.1 |
| Codex CLI | 0.163.0 original baseline; 0.164.0 live protocol read validated on 2026-10-24, native Windows x64 |

The setup reuses the existing toolchain without changing the default Rust toolchain, installing global npm packages, or editing system configuration. `.node-version` records the tested Node version; it does not require a version manager. The tested Rust compiler is recorded here rather than forcing rustup to install another copy of the existing stable toolchain.

## Install dependencies

Run these commands from the repository root in PowerShell:

```powershell
npm.cmd ci
cargo fetch --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc --locked
```

Direct dependency versions are exact in the manifests; npm and Cargo lockfiles pin their resolved dependency trees. npm packages are installed in `node_modules`; Cargo uses its normal per-user registry cache and the project build directory. Registry access is required for dependencies that have not been cached.

Use the existing npm installation. No pnpm, Yarn, Bun, CMake, Ninja, SQLite server, or global Tauri CLI is required for this scaffold. Tauri's CLI is a project-local development dependency. Obtain confirmation before adding any additional software or development tools.

## Develop

Open the desktop application, including its Vite development server:

```powershell
npm.cmd run desktop:dev
```

The dashboard connects to the installed native Codex CLI using read-only account and rate-limit APIs. Authenticate using Codex itself. The widget never reads credential files, starts inference, or redeems credits. A protocol incompatibility or unsupported authentication mode shows a diagnostic; an optional full native `codex.exe` path can be saved in Settings. Changes to `src/App.tsx` update through Vite hot reload.

Closing the dashboard keeps monitoring in the notification area. Find the icon in the tray overflow if Windows hides it. Use **Quit** in the dashboard or tray menu, then stop the development command with Ctrl+C. A second manual launch opens the existing dashboard; a duplicate login launch exits quietly. Launch at sign-in is off by default; enable it in General settings. The saved request, pending changes and observed Windows registration are shown separately. Login launches restore only the selected compact mode and tray; first launch and tray failure retain the dashboard.

Choose Floating, Screen Edge, System Tray (default), or Compact HUD in Settings or the tray menu. Floating uses a branded mini-card with quota bars and credits; Edge uses stacked percentage rings; HUD uses paired two-row quota/countdown groups. Display settings offer a shared Words (default) / Badges choice: circular 5 / W badges sit inside Edge rings and beside HUD percentages. HUD and Edge retain their existing containers and application icon, with segmented expected-reset indicators. Neutral grey badges keep emphasis on the colored percentages. Edge reset strips are 41px wide to match the visible ring diameter within its 44px container and 54px shell; both compact modes use tighter padding. The shared ring identity also appears in the Windows app, tray, installer and notification artwork. Edge and HUD share a temporary read-only hover preview with used percentages and exact reset dates/times; Floating has no hover expansion; the compact Edge tab shows both 5-hour and weekly remaining quotas. Left-click a desktop widget or tray icon to open or raise Overview after the Windows double-click interval; double-click cancels that pending popup and opens the standard app window directly. Overview and Settings share the same popup area; navigating to Settings keeps it beside the widget. Right-click a widget or preview to open the same native menu as the tray icon. Floating and HUD are draggable across their full compact surfaces; Edge moves freely while dragging, then docks to the nearest left or right edge of the monitor under the release pointer; its monitor, side and vertical position are saved together. Display settings offer separate 5-hour and weekly reset countdown switches for Floating, Edge, and HUD. HUD and Edge each offer Both, 5-hour only or Weekly only. A hidden period and its countdown disappear from the compact widget even if its countdown preference is enabled; hover previews and Overview still show both periods. Visible countdowns update locally as readable durations such as 3h10m or 6d7h. Ten segments in HUD/Edge show time remaining relative to the reported window duration; an elapsed deadline remains unconfirmed. Floating sizes itself to its contents. Edge and HUD retain a fixed scalable footprint per saved layout during normal usage/countdown changes; a visible warning adds space only while present. Edge details open beside the selected edge. Missing fields retain marked Last known values for two checks and become unavailable on the third; account and authentication changes clear data immediately. Floating retains Settings and a close button that saves System Tray mode; the tray menu still offers temporary Hide/show; Edge and HUD use the context menu or tray for Settings. Selecting a desktop mode from the tray, including the current mode, or choosing Show widget reveals and raises it once without changing Always on top. Applying a different desktop mode in Settings also reveals it; unrelated saves preserve hidden state. Enabled Always on top keeps widgets above newly opened ordinary windows. The shared position-lock setting and arrow-key nudging remain available. Settings provides monitor/offset selection, Edge side, topmost and fullscreen behavior. The tray menu includes Hide/show and Reset position. HUD hover and its single-click popup are horizontally centered and clamped to the work area; Edge hover uses the same 8-logical-pixel gap. Overview and its single-click popup show used percentages, exact reset timestamps, credit details and read timing directly. Credit metadata uses one row per item, showing three initially with Show more / Show less. Reminder management remains in Settings. If Windows denies foreground activation, a standard dashboard provides taskbar/tray recovery without repeated focus attempts or a false coordination warning. Native pointer, Explorer, accessibility and mixed-DPI behavior still need the full documented runtime matrix.

To run the frontend alone:

```powershell
npm.cmd run dev
```

Open `http://127.0.0.1:1420`. The browser entry explicitly reports that it has no native connection; it does not substitute sample quotas. A separate development-only [synthetic presentation fixture](http://127.0.0.1:1420/src/preview.html) supports `?theme=dark&scenario=stale`, `scenario=missing|blocked|loading|partial|fractional`, `scale=200`, `contrast`, `surface=shell|preview`, `mode=floating|screen-edge|compact-hud`, `tray-hover`, `resets=both|five-hour|weekly`, `labels=words|badges`, `credits=0..100`, `scenario=max-width|long-reset`, `detail=popup`, `page=settings`, and `welcome`. It is labeled synthetic and excluded from the production entry. The server binds only to loopback. Vite and Tauri use the same fixed port and fail clearly if that port is occupied.

## Check and build

```powershell
npm.cmd run typecheck
npm.cmd test
npm.cmd run build
npm.cmd run rust:fmt
npm.cmd run rust:check
npm.cmd run rust:test
npm.cmd run rust:clippy
npm.cmd run desktop:build
```

The native build command runs the frontend build and creates `src-tauri/target/release/codex-usage-widget.exe`. It passes `--no-bundle` and does **not** create or install an installer. Obtain confirmation before adding further tools/plugins or installer tooling; the pinned plugins and Tauri-managed NSIS tools were explicitly approved on 2026-10-23.

To build the unsigned per-user NSIS package:

```powershell
npm.cmd run desktop:package
```

The artifact is `src-tauri/target/release/bundle/nsis/Codex Usage Widget_0.1.0_x64-setup.exe`. Tauri uses its cached NSIS 3.13 and helper 0.5.3; no global NSIS/WiX installation is needed. The installer checks for WebView2 (minimum 120) and can download the bootstrapper if required; that path needs internet access. Only the existing 155 runtime has been validated here. The package is a development milestone, not a signed release.

`npm.cmd run package:check` performs a **real, temporary per-user installation** under `src-tauri/target/validation/package install`, checks artifact bytes (allowing only Tauri's NSIS bundle marker), shortcut target/app identity and data preservation, reinstalls, and uninstalls. It refuses existing product registration/shortcuts/startup entries. The default does not launch the application; optional `-NativeCheck Windows` or `-NativeCheck Notifications` launches only a matching debug package with isolated native-check state. The latter currently fails as documented in the notification investigation. The checker temporarily creates and removes one labeled app-data sentinel. Close the widget first; use a clean validation user if the guards refuse the run. A real version-upgrade/downgrade test additionally uses isolated version fixtures; exact commands/results are in the implementation report. Notification behavior, installer UI, absent-runtime/offline failures, Windows 10 and signing remain release gates.

Frontend policy tests use Node's built-in runner; Rust logic uses Cargo's built-in test runner. No additional test framework is required. Atomic Win32 file replacement tests need normal filesystem access; the agent's restricted overlay may reject that operation. Tests do not establish native focus, tray, notification, DPI or accessibility behavior.

To run the explicit **live read-only** protocol gate against the local signed-in Codex installation:

```powershell
npm.cmd run protocol:probe
```

It outputs only structural pass/fail metadata and optional-field presence, never quota values or account identifiers. There is no version query on successful reads. On protocol incompatibility the widget queries the same executable with `--version` for display only (two-second timeout, 4 KiB output cap, 200 sanitized characters); a failed query shows CLI version unavailable. Automatic reads pause until Refresh or changing the executable path. Network, timeout and sign-in failures retain their separate diagnostics. It launches and stops its own app-server; no inference is issued. Regular `rust:test` skips this live test.

## Native window check

For the focused stacking check, build the debug executable and run `& .\src-tauri\target\debug\codex-usage-widget.exe --check-native-topmost`. It uses separate validation settings and app-owned ordinary/topmost blocker windows to check reveal, external-activation stacking repair, focus preservation, and disabled-topmost/hidden behavior across the three desktop modes. Require `native-check result=pass`. It quits automatically and does not reproduce Photos or test real foreground-hook delivery; see the [investigation and limitations](docs/implementation-report.md#external-activation-and-topmost-stacking--2026-10-24).

After building the debug executable below, use `& .\src-tauri\target\debug\codex-usage-widget.exe --check-native-sizing` for the isolated compact-widget sizing gate. It compares generation-matched React measurements against Windows client dimensions for Floating, Edge, HUD and Edge/HUD desktop previews in Light/Dark with both reset countdowns on/off, and checks tray preview sizing. It verifies rounded native regions, hover focus preservation, hide/reselect recovery, one-time raising, persistent topmost behavior against a new app-owned ordinary window, Float hover exclusion, HUD hover centering, the Edge hover gap, framework style rewrites, event-driven topmost repair, and Floating popup stacking. It also injects an activation-denial outcome to verify standard-window recovery without focus theft or warnings. The popup geometry check uses a debug-only coordinator input without requesting foreground activation; it does not establish real click/focus behavior. The gate uses isolated validation state and quits automatically. It also injects Edge drag completions to verify both-side docking on each available monitor, saved monitor/height, position lock and no accidental Overview. Require `native-check result=pass`; actual pointer, visual, mixed-DPI, enlarged Windows text and high-contrast behavior remain separate checks. The flag is debug-only.

For the debug-only native coordinator check, build the bundled debug executable and run it directly:

```powershell
npm.cmd run tauri -- build --debug --no-bundle
& .\src-tauri\target\debug\codex-usage-widget.exe --check-native-windows
```

Require the `native-check result=pass` marker; a process exit alone is not a passing result. This check opens its own windows, uses isolated validation settings and normal read-only Codex monitoring, then quits. It advances only idle-view lease time to test retirement, posts an app-local native reconciliation message, and verifies lazy recreation/minimize/restore; it does not change Windows time/settings. It injects coordinator events rather than OS input and cannot establish real pointer/DPI/Explorer or screen-reader behavior. It is excluded from release builds. Its current results and limitations are in the implementation report.

After the same debug build, run the 100-transition coordinator/process-tree sampler:

```powershell
npm.cmd run native:stress
```

Close any running widget first. The script requires successful native assertions and 100 mode transitions, verifies reuse of one shell and final webview retirement, samples owned processes, and checks that no observed owned process survives shutdown. CSV/logs go to ignored `src-tauri/target/validation/`. This is a debug transition-load measurement; its sampled CPU and memory do not establish the ten-minute idle/release performance budgets, actual input behavior, or listener stability.

## Codex protocol reference

The versioned JSON Schema reference and regeneration instructions are in [schemas/codex](schemas/codex/README.md). They were generated offline from the existing CLI; no server connection, account lookup, inference request, or credential-file access is needed for schema generation.

The application launches a validated native `codex.exe`, never the npm or PowerShell wrapper. Discovery checks PATH and the existing npm installation's native x64 package. The live positive read passed on this machine; account switching and other authentication variants remain compatibility gates. No displayed quota cache is restored across restarts. Opaque account-bound reminder episodes and history persist without raw account IDs, emails, credentials, or protocol responses; missing account identity suspends quota reminders.

## Permissions and local configuration

The bundled dashboard has only named IPC permissions for state, refresh, validated settings, configuration-folder selection/inspection, and lifecycle commands plus event subscription. Rust registers pinned single-instance, autostart and notification plugins; React has no direct plugin, shell, filesystem or network permissions. Notification delivery is still gated while its adapter is completed. The content security policy allows local assets; the development policy additionally permits loopback Vite/hot reload. Rust stores settings and the reminder ledger in an atomic checksummed document and last-good backup, beside the widget EXE by default. General settings also offer the existing per-user `dev.codexusagewidget.desktop` AppData directory or a custom folder. A portable location manifest, with an AppData fallback, records the selected and active locations. Settings schema 5 and envelope format 4 preserve migration and downgrade protection. Do not edit it while the application is running.

The approved autostart dependency includes a project-local patch under `src-tauri/vendor/auto-launch`, preserving its MIT license. It restricts registration to the current user, quotes the executable/arguments and verifies the registered command. `rust:test` also runs its three pure Windows policy tests; no test changes sign-in registration. Patch provenance and runtime limitations are recorded in the implementation report.

Keep credentials out of the project. Local `.env` files, dependencies, build outputs, and generated Tauri schemas are excluded from source control. The Codex protocol schema reference and both dependency lockfiles should remain in source control. The four supplied design/reference images remain local and are intentionally excluded from the public repository. Production artwork and its documented provenance remain tracked.

## Troubleshooting

- **PowerShell profile errors:** The existing PowerShell profile produced a Conda startup error during inspection. Use `pwsh -NoProfile` for a clean shell; setup does not modify that profile. Using `npm.cmd` also avoids PowerShell's npm script wrapper.
- **`cl` is absent from PATH:** This is normal outside a Visual Studio developer shell. Rust successfully discovered the installed MSVC tools and compiled, linked, and ran a temporary executable from the normal shell. Use the installed x64 Developer PowerShell if future native dependencies require a developer environment.
- **Registry DNS or access errors in an agent sandbox:** Package downloads require network access. Use an authorized shell with registry access. Do not change registry mirrors, TLS verification, or system proxy settings as a workaround.
- **Codex temporary-directory warnings in an agent sandbox:** CLI inspection and schema generation succeeded even though the restricted process could not maintain Codex's existing per-user temporary aliases. This is an access restriction, not a missing CLI. Do not change Codex configuration or authentication to suppress it.
- **Native window fails to open:** Confirm the detected WebView2 runtime is still installed, then run `npm.cmd run tauri -- info`. Report any missing prerequisite before installing it.

## Setup validation

The [environment setup record](docs/development-environment-setup.md) records the original scaffold checks. Current implementation commands, test results and open gates belong to [the implementation report](docs/implementation-report.md); the earlier scaffold smoke check is not evidence for the new product behavior.

Recorded native sizing/coordinator and stress results apply to the revisions identified in the [layout-polish report](docs/layout-polish-implementation-report.md). The latest spacing follow-up was compiled without rerunning tests or runtime checks. Scripted coordinator checks provide limited native style, geometry and focus evidence; the full acceptance matrix remains open.

## References

- [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri with Vite](https://v2.tauri.app/start/frontend/vite/)
- [Tailwind CSS with Vite](https://tailwindcss.com/docs/installation/using-vite)
- [Official Codex app-server documentation](https://learn.chatgpt.com/docs/app-server)

## License and acknowledgements

Original project code and documentation are licensed under the [MIT License](LICENSE), copyright © 2026 Anda Li H.

Third-party materials retain their applicable upstream terms. The maintained autostart patch preserves the upstream [auto-launch MIT license](src-tauri/vendor/auto-launch/LICENSE); its modifications are recorded in the [implementation report](docs/implementation-report.md). The versioned Codex schema references have separate [generation provenance](schemas/codex/README.md); the project's MIT license does not replace their applicable upstream terms. Other dependencies retain their own licenses and notices.

Production artwork provenance is recorded in the [layout report](docs/layout-polish-implementation-report.md). Supplied private design/reference images are not distributed with this repository. Before distributing binaries, collect and review the applicable third-party notices for the exact dependency versions included.
