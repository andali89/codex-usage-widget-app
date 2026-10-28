# Development environment setup and validation

Date: 2026-10-22. Historical baseline: setup complete at `51f9436`, before product implementation. For current functionality and acceptance evidence, see [the maintained implementation report](implementation-report.md). The scaffold checks below do not validate later product changes.

## Prepared environment

- Reused the installed Rust/MSVC x64 toolchain, Windows SDK, WebView2, Node.js, npm, Git, and Codex CLI. The tested machine versions are recorded in the [verified machine baseline](#verified-machine-baseline).
- Installed the approved project-local Tauri 2, React, TypeScript, Vite, Tailwind CSS, and required build/type dependencies. Direct versions are exact; npm and Cargo lockfiles record their resolved trees. No global software or tooling was installed or upgraded.
- Created one standard Tauri window containing a React/Tailwind readiness message, with strict TypeScript, loopback development serving, frontend/native build commands, Rust checks, a development icon, and generated-file exclusions.
- Kept native permissions empty and enabled no application plugins or custom commands. The application identifier is `dev.codexusagewidget.desktop`; installer bundling is disabled.
- Generated and parsed all 318 JSON Schema files from native Codex CLI 0.163.0 without experimental fields. [Schema provenance and regeneration instructions](../schemas/codex/README.md) are maintained beside that reference.

The existing design report, reference image, and unrelated repository instructions were preserved. The scaffold contains no widget modes, tray behavior, quota client, refresh scheduler, reminders, database, or product theme implementation. No account requests or inference turns were issued.

## Validation performed

| Check / actual command | Result |
| --- | --- |
| Temporary Rust executable compiled from stdin, linked with MSVC, and executed | Passed; temporary artifacts removed |
| `npm.cmd list --depth=0` | Passed; all declared direct dependencies resolved |
| `npm.cmd run build` | Passed; strict TypeScript check and production Vite build |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Passed |
| `cargo check --manifest-path src-tauri/Cargo.toml --locked` | Passed |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings` | Passed |
| `npm.cmd run tauri -- info` | Detected the installed MSVC, WebView2, Rust, Node, and Tauri environment successfully |
| `npm.cmd run desktop:dev` | Started Vite and the native development executable; responsive Windows window observed |
| Edge at `http://127.0.0.1:1420/` | React message and Tailwind styling visually verified |
| Temporary readiness-heading source edit | Appeared through hot reload without navigation; source restored and restoration visually verified |
| `npm.cmd run desktop:build` | Passed; final release executable built without the earlier bundle-identifier warning |
| Final release executable with Vite stopped | Started and owned a responsive Windows main window; development port returned `ECONNREFUSED` |
| JSON parsing of generated protocol reference | Passed for all 318 files |

The native build produced `src-tauri/target/release/codex-usage-widget.exe`. No installer was built or installed. Development servers, native smoke-test processes, and the temporary browser tab were stopped or closed after verification.

## Limitations and follow-up boundaries

The implementation now includes four display modes, a native window coordinator, settings schema 4, reminder policy/ledger storage and single-instance/autostart integration. These are later milestones, not part of the original scaffold validation above. The user approved the pinned plugins and NSIS tooling on 2026-10-23; single-instance 2.5.4, autostart 2.7.0 and notification 2.7.0 are now downloaded and locked. A local patch to the transitive `auto-launch` 0.6.0 restricts Windows registration to the current user and quotes its command. See the [current implementation report](implementation-report.md) for provenance, actual checks and remaining gates. Approved Tauri-managed NSIS 3.13 and helper 0.5.3 are cached per user. Per-user install/upgrade/downgrade rejection/uninstall smoke checks now pass; notification delivery, installer UI/offline provisioning and supported-OS validation remain outstanding.

- The restricted shell could not access package registries or local network sockets. Authorized execution outside that sandbox completed dependency installation and runtime checks without changing machine networking or security settings.
- The Windows Computer Use helper pipe was unavailable. Native process/window responsiveness was verified, but the WebView2-rendered window was not visually inspected. Frontend appearance and hot reload were verified in the connected Edge browser. Native visual inspection remains a manual check.
- Codex emitted restricted temporary-alias maintenance warnings during inspection and schema generation; generation succeeded. Its authentication and configuration were not changed.
- The schema reference does not establish account-specific field availability or live quota compatibility. Those checks belong to implementation, along with the design report's native focus, tray, notification, appearance, persistence, and performance acceptance criteria.
- Additional development tools, plugins, installer tooling, or system software require reporting and user confirmation before installation.

The later debug coordinator stress command, `npm.cmd run native:stress`, uses only existing PowerShell/.NET and the locally built debug executable. Build it first using the instructions in the root README; samples/logs are generated under the ignored target directory. It does not replace ten-minute idle profiling or installed/manual Windows checks.

For everyday commands, use the [development setup guide](../README.md). Continue product work against the [canonical design report](codex-widget-design-report.md).

Implementation was stopped at the user's request on 2026-10-23 with final source checks passing and installed notification availability still failing. The [handoff](implementation-handoff.md) identifies the uncommitted worktree, build artifacts and remaining gates; the [notification investigation](notification-error-investigation.md) gives a reproducible failure for the user's assigned agents.

## Contributor reference — 2026-10-24

The sections below retain detailed development instructions moved from the root README after the v0.1.0 public preview was published. Earlier setup results above remain historical.

### Verified machine baseline

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

### Frontend presentation fixture

Run `npm.cmd run dev` from the repository root.

Open `http://127.0.0.1:1420`. The browser entry explicitly reports that it has no native connection; it does not substitute sample quotas. A separate development-only [synthetic presentation fixture](http://127.0.0.1:1420/src/preview.html) supports `?theme=dark&scenario=stale`, `scenario=missing|blocked|loading|partial|fractional`, `scale=200`, `contrast`, `surface=shell|preview`, `mode=floating|screen-edge|compact-hud`, `tray-hover`, `resets=both|five-hour|weekly`, `labels=words|badges`, `credits=0..100`, `scenario=max-width|long-reset`, `detail=popup`, `page=settings`, and `welcome`. It is labeled synthetic and excluded from the production entry. The server binds only to loopback. Vite and Tauri use the same fixed port and fail clearly if that port is occupied.

### Native window checks

For the focused stacking check, build the debug executable and run `& .\src-tauri\target\debug\codex-usage-widget.exe --check-native-topmost`. It uses separate validation settings and app-owned ordinary/topmost blocker windows to check reveal, external-activation stacking repair, focus preservation, and disabled-topmost/hidden behavior across the three desktop modes. Require `native-check result=pass`. It quits automatically and does not reproduce Photos or test real foreground-hook delivery; see the [investigation and limitations](implementation-report.md#external-activation-and-topmost-stacking--2026-10-24).

After building the debug executable, use `& .\src-tauri\target\debug\codex-usage-widget.exe --check-native-sizing` for the isolated compact-widget sizing gate. It compares generation-matched React measurements against Windows client dimensions for Floating, Edge, HUD and Edge/HUD desktop previews in Light/Dark with both reset countdowns on/off, and checks tray preview sizing. It verifies rounded native regions, hover focus preservation, hide/reselect recovery, one-time raising, persistent topmost behavior against a new app-owned ordinary window, Float hover exclusion, HUD hover centering, the Edge hover gap, framework style rewrites, event-driven topmost repair, and Floating popup stacking. It also injects an activation-denial outcome to verify standard-window recovery without focus theft or warnings. The popup geometry check uses a debug-only coordinator input without requesting foreground activation; it does not establish real click/focus behavior. The gate uses isolated validation state and quits automatically. It also injects Edge drag completions to verify both-side docking on each available monitor, saved monitor/height, position lock and no accidental Overview. Require `native-check result=pass`; actual pointer, visual, mixed-DPI, enlarged Windows text and high-contrast behavior remain separate checks. The flag is debug-only.

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

### Installer smoke check

`npm.cmd run package:check` performs a **real, temporary per-user installation** under `src-tauri/target/validation/package install`, checks artifact bytes (allowing only Tauri's NSIS bundle marker), shortcut target/app identity and data preservation, reinstalls, and uninstalls. It refuses existing product registration/shortcuts/startup entries. The default does not launch the application; optional `-NativeCheck Windows` or `-NativeCheck Notifications` launches only a matching debug package with isolated native-check state. The latter currently fails as documented in the notification investigation. The checker temporarily creates and removes one labeled app-data sentinel. Close the widget first; use a clean validation user if the guards refuse the run. A real version-upgrade/downgrade test additionally uses isolated version fixtures; exact commands/results are in the implementation report. Notification behavior, installer UI, absent-runtime/offline failures, Windows 10 and signing remain release gates.


### Compact readability and size validation — 2026-10-28

After building the current 0.1.3 production assets and native executable, run `& .\src-tauri\target\release\Usage.exe --check-native-compact-options` for the isolated synthetic Edge/HUD readability and 80–300% scale gate. It checks exact native/client bounds, rounded regions, independent countdown options, preserved reset-bar counts, fitting-limit rejection, and actual Settings preview/Cancel/Save/reopen. The complete `--check-native-appearance` gate includes the same styling/size cases. Both quit automatically; require `native-check result=pass` in the corresponding `src-tauri/target/validation/native-compact-options/result.log` or `native-appearance/result.log`. No live usage read is needed. See the [appearance report](widget-appearance-and-snap-implementation-report.md) for current results and remaining manual Windows checks.
