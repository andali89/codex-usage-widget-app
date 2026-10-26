# Widget appearance and snapping — implementation evidence

Date: 25 October 2026. Source version: 0.1.2. Design: [appearance and consistent snapping](widget-appearance-and-snap-design.md).

## Delivered behavior

- App version is **0.1.2** in npm, Cargo, Tauri, lockfiles and package-check defaults. No dependency versions changed.
- Settings schema **7** stores one paired preset (Original, Ocean, Forest, Plum, Slate, Amber) and independent per-preset Light/Dark custom quota switches and values. The checksummed storage envelope remains format 4, with settings and reminder bookkeeping in one atomic transaction.
- Display shows only the active effective theme’s 5-hour and weekly controls. Custom color rows appear only after opt-in. Boxed swatches are filtered against the active preset’s content/track plus dashboard surfaces; names, checkmarks and arrow-key selection support keyboard use. Load preset colors restores both active preset/theme values and keeps opt-in enabled. Turning customization off retains its colors.
- The separate synthetic development fixture also understands the display-preview commands; it remains a simulation with no native effects.
- Settings contains no sample widget previews or gap diagram. Placement combines position lock, monitor/offset/edge placement and a compact gap slider/numeric field with Reset to 8. Lock and gap remain available in tray mode. Incomplete numeric input remains invalid rather than becoming zero.
- Theme, preset, quota colors and gap temporarily affect the actual current widget. Rust owns a display-only preview session with owner ID, monotonic sequence, saved-revision guard and a 10-second lease renewed every 2 seconds. Updates are coalesced during slider drags. Invalid drafts leave the last valid preview. Save persists the whole draft once; Cancel/Discard, leaving/hiding Settings, reload/destruction, revision conflicts and quit retire temporary styling. Failed saves retain the draft. Preview does not change Codex configuration, scheduling, reminders or hidden-widget intent.
- An anchored Settings popup stays in place during a gap drag and reanchors on release. Preview geometry uses a clone of saved settings and does not save placement. Other persisted changes retire the preview and use the existing draft conflict flow.
- Original bypasses structural palette overrides, restoring v0.1.0’s 1 px divider frame, distinct surface/raised/wash colors, Floating/Edge/HUD gradients, credits, badges, hover and tray surfaces. Custom quota colors remain independent. The HUD weekly percentage now uses the same custom weekly token as its track. Dashboard structure, brand, focus and semantic status colors remain unchanged.
- Rust validates all stored quota customizations, including disabled/inactive preset/theme entries, allowed role names and hex colors. The shared JSON catalog and defaults feed both Rust and TypeScript. Schema-6 defaults migrate to Original without activating hidden overrides; quota-only customizations retain both themes; unmatched structural customizations retain their legacy appearance as Saved custom appearance. Schema-6 gaps/anchors are preserved. The older pre-6 anchor migration remains separate.
- Floating, HUD and Edge share release/restore geometry with a 12 logical px attraction distance. Destination DPI is applied once. Anchor intent survives gap/size changes; free axes remain free; nudges clear affected anchors; explicit Reset position works while locked. Popup spacing remains 8 logical px. Auto-hide taskbar safety floors and insufficient-space constraints remain enforced.

## Native transparency gate

**Desktop transparency is unavailable in the shipped path.** A compact advanced row explains the disabled numeric control; stored preferences, preset colors and gap editing remain available.

The shell prototype uses Tauri transparent creation, an optional no-redirection bitmap, CSS background alpha, and the existing rounded native region. Main/preview reading windows remain opaque. Failed transparent creation retries an opaque shell and records a bounded diagnostic.

An isolated composition probe confirmed that React emits the requested RGBA gradient stops, but its paired native backdrop sampling could not establish a trustworthy composition result: the backdrop validation failed. Earlier samples reported no black/white difference, which is not sufficient evidence of a compositor defect once the backdrop is unverified. Neither native translucency nor its compatibility with rounded clipping is declared passed. The release capability gate therefore stays false, including in ordinary debug sessions. Only the explicit debug composition probe bypasses it.

Do not enable the release gate until actual composition, startup/recreation flashes, rounded hit bounds, real pointer/focus behavior, and accessibility transitions have passed on the intended Windows/WebView2 matrix. The inactive control is the design's disclosed fallback, not a claimed transparency implementation success.

## Current v0.1.2 validation

| Check | Current result |
| --- | --- |
| `npm.cmd test` | Passed: 65 tests, including all 12 preset palettes, every offered quota swatch, independent preset/theme memory, dormant validation and concise opt-in rendering. |
| `npm.cmd run build` | Passed, including TypeScript checking; repeated after the final save-error wording update. |
| `npm.cmd run rust:test` | Passed: 147 app tests, one explicit live probe ignored, plus 3 autostart tests. Process-local TEMP/TMP used the workspace validation directory. |
| `npm.cmd run rust:fmt` | Passed. |
| `npm.cmd run rust:clippy` | Passed with all targets and warnings denied. |
| Embedded debug build | Passed: `cargo build --manifest-path src-tauri/Cargo.toml --locked --features tauri/custom-protocol`. |
| `--check-native-appearance` | Passed with `native-check result=pass`. Actual WebView styles match the source-derived v0.1.0 fixture for Original Light/Dark shell frames, gradients, credits, badges, hover and dashboard surfaces. Custom weekly percentage text uses the selected color in all three desktop modes. Real Settings DOM interactions cover preset/theme/custom opt-in, temporary gap movement, popup freezing/release, incomplete numeric input, Cancel rollback, no saved revision during preview, one revision on Save and session retirement on close. |
| `--check-native-sizing` | Passed on retry with `native-check result=pass`: Light/Dark, quota labels, visible periods/countdowns, client/hover sizing and placement, stacking, recovery and the appearance/Settings assertions. The first run stopped at `hover-preserves-foreground`; its cause was not established. The retry retained that assertion and passed. |
| Embedded release build | Passed: `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol`. File and product version both report **0.1.2**. This is an executable build, not installer/bundler validation. |
| Documentation | `git diff --check` and local-link/code-fence checks passed. |

Final local executable: `src-tauri/target/release/codex-usage-widget.exe`, **12,616,704 bytes**, file/product version **0.1.2**, SHA-256 `70EC7ECFECE1F6DF84A29371F95CBCF7D2CA2431147DA234F6EF5763F3F4ED5D`.

Current logs: `src-tauri/target/validation/widget-presets-native-final.log`, `widget-presets-native-sizing-retry.log`, `widget-presets-rust-tests-final.log`, `widget-presets-clippy-final.log` and `widget-presets-frontend-tests-final.log`. The failed first broad run is retained separately as `widget-presets-native-sizing.log`. The final release adds only explicit temporary-display wording to Save failures after the native interaction checks; the frontend production build passed afterward.

Computed border-width assertions account for Chromium rounding physical pixels at the available 250% scale; they also assert the original divider color and baseline gradient/surface tokens. This is no claim of a completed multi-DPI OS matrix. Native runs still emit tray-removal and WebView2 class-unregistration (1412) shutdown diagnostics. No validation app remained running after completion. End-to-end injected storage failure, settings-conflict/reload races, timed lease expiry and screen-reader/visual review remain manually unverified; focused pure tests cover lease expiry, ownership, ordering, invalid input and revision guards.

## Earlier appearance/snap validation

The table below records the earlier eight-role editor iteration. It is historical evidence for shared geometry and the transparency gate; its old UI-specific assertions do not establish the replacement preset controls.

Environment: Windows build **26200.9711 (25H2)**, installed WebView2 **155.0.4283.62**. This does not establish Windows 10 coverage.

Frontend tests/build, Rust checks, native checks, and remaining limitations are recorded below. Logs and other generated validation files belong under the ignored `src-tauri/target/validation/` directory.

| Check | Result |
| --- | --- |
| `npm.cmd test` | Passed: 51 tests, including shared palette/compositing fixtures, nested decorative paint versus protected backing, and synthetic Settings rendering without real usage values. |
| `npm.cmd run build` | Passed, including TypeScript checking. |
| `npm.cmd run rust:fmt` | Passed. |
| `npm.cmd run rust:check` | Passed. |
| `npm.cmd run rust:test` | Passed on final source: 143 app tests, one live probe ignored, and 3 autostart tests. |
| `npm.cmd run rust:clippy` | Passed on final source with `--all-targets -- -D warnings`. |
| Embedded debug build | Passed using `cargo build --manifest-path src-tauri/Cargo.toml --locked --features tauri/custom-protocol` after the frontend build. |
| `--check-native-appearance` | Passed with `native-check result=pass`: all three modes at gaps 0/8/16/48, locked resets, saved anchor metadata, rounded regions, retained 0/20/80 preferences with opaque fallback, foreground preservation, and no quota read in flight. The focused check was repeated successfully after the final paint-only CSS refinement. |
| Composition probe | Not passed; see the native gate above. |
| `--check-native-sizing` | Passed with `native-check result=pass`: Light/Dark, Words/Badges, period/countdown combinations, client sizing, popup/hover placement and foreground preservation, both-side Edge docking, stacking/recovery, and the appearance/fallback assertions. This full matrix preceded the final paint-only CSS refinement; the focused appearance check passed afterward. The available monitor scale was 250%; this does not establish actual cross-DPI drags. |
| `npm.cmd run desktop:build` | Blocked by the installed Tauri CLI native binding; see below. |
| Native release compilation | Passed: `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol` after the frontend build. This is local compilation evidence, not a packaged, path-remapped public release or a substitute for Tauri bundler validation. |
| Documentation checks | Passed: `git diff --check`; inline Node checker resolved 52 local links and balanced code fences across README, the documentation index, design/implementation report, and v0.1.1 notes. |

Rust filesystem tests require process-local `TEMP` and `TMP` pointing to `src-tauri/target/validation/appearance-baseline-temp` in this session. The ordinary temporary directory initially produced storage commit/future-version failures; the workspace-local run passed. Incremental-cache hard-link warnings fell back to copying.

The project-local Tauri CLI fails to load its existing Windows native binding (`ERR_DLOPEN_FAILED`, invalid Win32 application). No dependencies or global tools were installed/upgraded. Missing local npm executable shims were regenerated offline with `npm.cmd rebuild --bin-links --ignore-scripts --offline`. Direct Cargo embedded builds reuse the existing lockfile/toolchain. Plain `cargo build` uses the development URL and did not hydrate in this socket-restricted session; those timeout runs are not passing native evidence.

The Windows computer-use helper could not connect to its native pipe. Browser inventory was empty and the in-app browser was unavailable. Visual screenshots and computer-use checks could not be performed. Native WebView DOM/IPC interaction is covered by the isolated checks described above. The synthetic markup tests do not substitute for those checks.

The passing native run still emitted tray-removal and WebView2 class-unregistration (1412) shutdown diagnostics. Its pass marker establishes the assertions above, not a claim that native shutdown is warning-free. No test application remained running after exit. WebView2 child-process enumeration could not be independently verified because CIM access was unavailable; a failed enumeration is not evidence of zero children.

## Reproduce the isolated checks

From the repository root, build the frontend and an embedded debug executable:

```powershell
npm.cmd run build
cargo build --manifest-path src-tauri/Cargo.toml --locked --features tauri/custom-protocol
& .\src-tauri\target\debug\codex-usage-widget.exe --check-native-appearance
```

This check uses separate configuration under `target/validation/native-appearance`, synthetic usage, disabled reminders, and no Codex worker. It quits automatically. Require the final `native-check result=pass` marker; the process exit code alone is insufficient.

The explicit experimental composition gate is:

```powershell
& .\src-tauri\target\debug\codex-usage-widget.exe --check-native-appearance --probe-native-composition
```

It temporarily places app-owned solid-color backdrops behind the synthetic shell and samples only verified app-owned regions. It retains no screenshots, wallpaper, user data, or actual quota values. A failed/occluded backdrop invalidates the result. This probe is currently unverified and does not enable the release gate.

`--check-native-sizing` also runs the appearance/fallback assertions and uses isolated synthetic presentation data under `target/validation/native-sizing`. Other protocol/notification check modes retain their existing explicit behavior.

## Remaining manual validation

Windows 10, actual mixed-DPI pointer drags and monitor removal, auto-hide taskbars on every edge, Explorer restart, text scaling/high-contrast/transparency changes, screen readers, screenshots over moving backgrounds, and native startup flashes remain unverified. Rust tests cover negative origins, 100/125/150/200/250% DPI calculations, ties, attraction boundaries, fit exhaustion, conservative migration, and right/bottom anchors after resize; they do not establish those OS interactions.

Schema migration intentionally moves a reliably inferred legacy flush anchor to the default 8 logical px gap. Choose 0 for flush placement where the taskbar safety floor permits it. Uncertain legacy placements remain free. No installer, release, push, inference request, credential copy, or reset-credit operation is part of this change.
