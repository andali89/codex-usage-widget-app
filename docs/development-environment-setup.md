# Development environment setup and validation

Date: 2026-10-22. Historical baseline: setup complete at `51f9436`, before product implementation. For current functionality and acceptance evidence, see [the maintained implementation report](implementation-report.md). The scaffold checks below do not validate later product changes.

## Prepared environment

- Reused the installed Rust/MSVC x64 toolchain, Windows SDK, WebView2, Node.js, npm, Git, and Codex CLI. The tested machine versions are recorded in the [root README](../README.md#verified-machine-baseline).
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
