# Widget appearance and snapping — implementation evidence

## Optional outer border and opaque backgrounds — 27 October 2026

Implemented in source at application version **0.1.2**, settings schema **10**. This section supersedes the earlier borderless-Light and transparency policies; dated entries below retain their historical evidence.

- **Show widget border** appears in Settings → Display with the widget appearance controls. It defaults to On for new and migrated settings and applies across all paired themes, Floating, Screen Edge, HUD, desktop previews, and tray summaries. On restores the theme-specific frame colors from before the two border-removal commits; Off paints only the outer frame transparent, retaining its 1 px layout space. Internal dividers, quota graphics, rounded corners, and widget dimensions remain unchanged. Windows high contrast and forced colors retain a visible system border.
- Border edits use the existing native display-preview session: the active widget updates temporarily, Save persists the choice, and Cancel restores the saved value. Changing preset or Light/Dark theme retains the shared border choice.
- Transparency controls are removed from Settings. Both Rust and TypeScript resolution force background/backing alpha to 1, even when saved settings or preview commands request 20/50/80% transparency. Existing percentages and their validation are retained for compatibility, but inactive. The ordinary effective reason is transparency-disabled; existing accessibility/platform reasons keep precedence. Native corner cutouts and rounded regions remain supported.
- Schema-9 migration adds the default border preference while preserving the complete format-4 settings/reminder document, revision, profile metadata, theme/custom colors, positions, and saved transparency. Newer schemas remain protected from overwrite. No dependencies, permissions, application version, usage reads, scheduler, or reminder policy changed.

### Validation

| Check | Actual result |
| --- | --- |
| `npm.cmd test`, `npm.cmd run build` | 143 tests passed; production build and TypeScript check passed. Tests cover historical frame colors and Off for all 22 preset/theme combinations, unchanged non-frame paint, high-contrast precedence, opacity, and absent transparency controls. |
| `npm.cmd run rust:test` | 157 application tests and 3 maintained dependency tests passed; 1 explicit live probe ignored. Process-local TEMP/TMP were set to src-tauri/target/validation/border-opacity-test-temp. Tests include schema-9 full-document preservation, native storage reopen with border Off, strict IPC field types, shared paint fixtures, and display-only draft isolation. |
| `npm.cmd run rust:fmt`, `rust:check`, `rust:clippy` | Passed. Cache hard-link fallback warnings are environmental. |
| `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol` | Passed. The existing `npm.cmd run tauri -- --version` probe fails with `ERR_DLOPEN_FAILED` (invalid Win32 native binding); used the established direct Cargo release build without installing or changing tools. |
| Release EXE `--check-native-appearance` | **Passed**, explicit `native-check result=pass`. 132 shell border checks (11 presets × 2 themes × 3 modes × On/Off), 88 desktop-preview border checks, 44 tray-summary border checks, 132 stable-dimension checks, 3 synthetic high-contrast checks, inactive saved 0/20/50/80% transparency, Settings live preview/Cancel/Save, hidden controls, and disk reopen with border Off. Includes existing quota/sizing/navigation checks. |
| Release EXE `--check-native-appearance --probe-native-composition` | **Unverified pixels:** restricted and normal-access runs both failed with `native-capture-unavailable` (screen DC clip was 0×0; no pixel copied). No new pixel-opacity pass is claimed. Runtime state/CSS opacity checks passed in the complete run above. |
| Documentation/diff/process cleanup | Five maintained Markdown files have valid local links; `git diff --check` passed. All 9 observed processes from the final validation instance were no longer running after cleanup. |

Initial Rust tests used the default Windows temp location and failed filesystem commit/locator cases; rerunning with the project-owned temp directory passed. An initial new migration assertion compared internal snake-case metadata names to the storage envelope's camel-case names; it was corrected to compare the actual storage contract. No storage behavior was changed as a workaround.

Final executable: `src-tauri/target/release/codex-usage-widget.exe`, **13,681,664 bytes**, version **0.1.2**, SHA-256 `13FE97E329508ECA4A704C391C40E3DF74CBF476F465CA8A22BF29416BA4986D`.

Validation artifacts/logs are ignored under `src-tauri/target/validation/border-opacity-*`; the final explicit native result is in `src-tauri/target/validation/native-appearance/result.log`. The restricted runner also lacked CIM process-tree access; the final appearance run used approved normal Windows access. Existing tray-removal/WebView2 class-unregistration shutdown diagnostics remain visible. Windows 10, real pointer/focus interactions, mixed-DPI transitions, and actual Windows high-contrast/text-scale transitions remain manual checks. No live Codex probe, inference, credential access, notification submission, installer, publication, or push is part of this change.

---

## Borderless new Light themes — 26 October 2026

Extended the Sunrise preview treatment to all ten new Light presets: their outer widget/preview frames are transparent, with the existing 1 px layout space retained. Original Light, all Dark themes, high-contrast borders, internal borders and the 5H/weekly middle divider are unchanged. Shared Rust/TypeScript resolution and the existing fixtures were updated; settings remain schema 9.

Requested validation: builds only, no tests or runtime checks. `npm.cmd run build` and `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol` passed. The requested release EXE was rebuilt. No tests or runtime checks were run.

---

## Sunrise Light border preview — 26 October 2026

Sunrise in Light mode now paints the outer widget/preview frame transparent while retaining its 1 px layout space. The 5H/weekly middle divider, internal borders, Original and all other themes, Sunrise Dark, and high-contrast borders are unchanged. Shared Rust/TypeScript frame resolution and the existing expected fixture values were updated.

Requested validation: builds only, no tests or runtime checks. `npm.cmd run build` and `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol` passed. The release EXE was rebuilt for the user’s manual review. No tests or runtime checks were run.

---

## Quota outline removal — 26 October 2026

Removed the extra contrast stroke beneath rings and the inset outlines on progress bars, HUD bars and reset segments. All selected colors now use the same indicator treatment. Percentage text retains its readable related shade; stale/unavailable/focus markings, tracks, dimensions, frames, transparency and saved choices remain unchanged.

Removed unused derived `quotaBoundary` snapshot fields and CSS variables from Rust/TypeScript, and updated the existing fixtures. Settings remain schema 9; no migration is required. The earlier outline behavior and extensive validation below describe the preceding artifact.

Validation was deliberately limited at the user's request: `npm.cmd run build`, `npm.cmd run rust:check`, and the embedded Cargo release build all passed. One focused SSR check confirmed yellow/orange 5H/weekly rings in Light/Dark render only the normal track and value strokes, preserve selected fills, and contain no contrast-outline styling. Full frontend/Rust suites, broad native checks, and desktop visual review were not run.

Rebuilt `src-tauri/target/release/codex-usage-widget.exe`: version **0.1.2**, **13,617,152 bytes**, SHA-256 `BE31B43C435AEDCE78E445C9E53DCF79FC09DDBA07D6AED7F2D3CAC0D79B8F08`.

---

## Content fitting and uniform surface transparency — 26 October 2026 follow-up

This section supersedes the initial 26 October sizing/backing policy below. Application version remains **0.1.2**; settings schema advances to **9**. No dependencies, permissions, Codex reads, scheduling, or reminder policies change.

### Causes and implementation

- **Overview height:** the previous fit-once cache could retain a short loading/empty-state height indefinitely. Meaningful Overview measurements now grow or shrink either presentation to natural content height, capped at **85% of the relevant monitor work-area height, including native frame insets**. Settings never drives content fitting. It retains current bounds and scrolls; direct Settings opens use capped initial defaults. Countdown-only reports and unchanged acknowledgements do not refit. React coalesces resize reports per animation frame. Returning from Settings rechecks natural Overview height even when a work-area change has altered its cached bounds. Hidden/closed views cannot update sizing, and readiness requires the actual target width, including widths reduced to fit a narrow work area.
- **Independent popup:** standard and single-click popup dimensions remain in separate session slots. Genuine standard resizing/maximizing preserves the manual choice through close/reopen and popup promotion. Moving uses layout reconciliation and does not disable content fitting. Removed the move-as-resize heuristic and height floors that could defeat natural sizing/small-monitor caps. Automatic limits use the current standard monitor or popup anchor monitor; physical/logical conversions and frame insets occur once.
- **Focus preservation:** the broad runtime check exposed pending native/DOM heading focus outliving a close. Close now cancels pending native focus; heading callbacks require an active document and are retired with the view request. Native Windows activation still owns focus, including denial/taskbar recovery.
- **Frames:** an independent frame token blends 65% existing frame with 35% background in every Light preset and the ten new Dark presets. Original Dark and saved legacy Dark structural frames retain their colors. The 5H/weekly separator, internal dividers, widths, corner radii, shadows, focus and unavailable-data outlines are unchanged. Dashboard/Settings frame styles are outside this scope.
- **Transparency:** `backingAlpha=1` plus the protective content-group fills made nested surfaces opaque. Desktop shells now flatten their existing opaque gradient/panel/badge/control-hover backgrounds into SVG shapes, positioned from current layout bounds, then apply the requested alpha once to the background group. Native/React root opacity stays 1. Text, rings, percentage/bar fills, tracks, segmented reset indicators, icons, borders and focus indicators stay outside that group and remain opaque. State/layout/hover/focus events update the layer without polling. At 0%, the existing CSS paint path is retained. Previews, tray summaries, dashboard and Settings stay opaque; Windows accessibility/native-availability overrides still force opaque surfaces. Uniform transparency intentionally allows desktop content to affect perceived contrast.
- **Quota choices:** 26 named Light and 24 named Dark colors plus preset defaults/retained values, with explicit yellow, lemon, orange, tangerine, gold, coral and red. Exact selected colors paint indicators. A related percentage shade meets 4.5:1 on known opaque app surfaces; a thin related graphic boundary meets 3:1 when raw fills need it. Existing tracks and geometry remain. Default paint remains unchanged where already sufficient; unknown/stale labels and outlines keep their existing semantics. Structural palette validation remains strict. Schema-8 migration changes only the version and preserves all settings values; earlier migrations and durable reminder data remain intact.

### Validation

| Check | Actual result |
| --- | --- |
| `npm.cmd test` | **120 passed**. All offered colors are checked for derived text/graphic contrast; 44 shared derived-paint cases agree with Rust. Legacy validation and Original v0.1.0 palette fixtures remain covered. |
| `npm.cmd run build` | Passed; includes TypeScript checking and the production frontend. |
| `npm.cmd run rust:fmt`, `rust:check`, `rust:clippy` | Passed; final Clippy covers all targets with `-D warnings`. The incremental-cache hard-link fallback warning is environmental. |
| `npm.cmd run rust:test` | **155 application tests and 3 maintained dependency tests passed; 1 explicit live probe ignored**. Process-local `TEMP`/`TMP` used `src-tauri/target/validation/showcase-test-temp`. Includes natural heights below/at/above the cap, 100/150/250% physical sizing math, small work areas, natural-content changes, width-clamped wrapping, unchanged acknowledgements, hidden-view and target-width readiness guards, independent popup/manual sizing, schema migration and shared derived paint. |
| `npm.cmd run desktop:build` | Still blocked by the existing Tauri CLI native binding (`ERR_DLOPEN_FAILED`, invalid Win32 application). No installation, dependency or global machine changes were made. |
| `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol` | Passed after the final frontend build; embeds production assets in the requested release executable. No installer/bundler validation is claimed. |
| Release `--check-native-appearance` | **Passed**. 88 preset/theme/mode combinations (66 shells + 22 synthetic tray previews), plus 66 yellow/orange shell combinations; separate frame/middle-divider assertions; Original Dark/base cascades; aligned 0/20/50/80% SVG background groups with opaque foreground information; natural-height/cap fitting; move/manual/popup/reopen policies; Settings navigation; 150% synthetic selector layout; warm-color selection, keyboard tab stops and Save/Cancel. |
| Release `--check-native-sizing` | **Passed: 104 compact combinations**, followed by the appearance/navigation/settings checks, docking, placement/stacking, tray recovery, and foreground preservation. This broad run preceded the final return-to-Overview refit condition; the final executable reran the complete appearance/navigation/settings check afterward. |
| Release desktop composition pixel probe | **Passed in the actual release EXE** at 0/20/50/80%: calibrated outer samples across Floating/Edge/HUD plus inner Floating quota-card and credit-panel samples over app-owned black/white backdrops. Inner RGB deltas were `[0,0,0]`, `[51,51,51]`, `[127,127,127]`, `[204,204,204]`, matching requested alpha without opaque nested backing. Capture was available in this final run (1920×1080 screen DC); this supersedes the earlier empty-region limitation below. |

The popup cap test initially omitted its native frame inset; correcting the expected outer height yielded the intended 300-client/309-outer, capped 877-outer, then 400-client/409-outer sequence on the 1.0-scale validation monitor. The reopened-popup assertion compares its current natural capped height rather than freezing an earlier Overview measurement. Settings tests serialize synthetic selections as distinct user actions. The broad run exposed an invalid activation-denial test precondition after keyboard selection had focused Main: the real activation helper returns success when Main already owns foreground. The corrected test releases only its own window focus, restores an inactive popup, verifies that precondition, then injects denial and checks foreground preservation. The focused reproduction is `--check-native-sizing --probe-native-activation`; its before/after records are retained under the validation directory. Heading focus now waits until Windows has actually focused the dashboard document, and closes cancel pending native focus. This prevents a late heading callback from a hidden view taking focus during hover validation while retaining heading focus after actual activation. Keyboard selection/tab-stop assertions run inside the app; real Windows pointer/focus behavior and mixed-DPI monitor moves remain manual checks.

Final executable: `src-tauri/target/release/codex-usage-widget.exe`, **13,678,080 bytes**, file/product version **0.1.2**, SHA-256 `98E32AC9EE944B70219C96F5C35B7A0E302461080889AF0FD6E18FAA967B5038`.

Ignored evidence is under `src-tauri/target/validation/refinement-*` (frontend/Rust test logs and release appearance/sizing/composition records). Result markers are checked explicitly; process exit alone does not establish a pass. The previously recorded tray-removal/WebView2 class-unregistration shutdown diagnostics still occur. All validation widget processes exited; no development server was started.

The final screen-pixel check establishes actual composition over controlled solid desktop backdrops, including inner surfaces. Visual review over patterned/moving desktop content, badges and hover fills remains manual. Windows 10, real mixed-DPI pointer drags, OS text/high-contrast transitions and visual screenshot review remain unverified.

The supplied showcase remains local/private and ignored. No installer, public release, push, credential access, inference request, or reset-credit action is part of this follow-up.

---

## Initial sizing, showcase themes, and release transparency — 26 October 2026 (historical)

The following records describe the first implementation; the follow-up above replaces its fit-once and opaque-content-backing decisions.

Source/application version remains **0.1.2**; settings schema is now **8**. No dependencies, permission capabilities, protocol behavior, refresh scheduling, or reminder policies changed. The supplied theme showcase remains a private, ignored local reference; no copy is distributed.

### Causes and changes

- **Height changes:** React reports natural page height, and the native coordinator previously treated page changes and subsequent fit requests as permission to resize. Settings could therefore expand the dashboard or anchored single-click popup. Each presentation now fits once at its preferred width, then keeps independent logical client bounds during navigation, disclosures, and refreshes. Longer content scrolls. Direct Settings opens use the existing 640/600 logical-pixel initial heights; manually resized standard windows and reopened popups keep their own sizes. Existing placement, work-area clamping, and native frame accounting remain in use.
- **Theme similarity:** partial presets inherited Original’s surfaces/dividers, and quota choices were harvested from that small catalog. Ten complete Light/Dark palettes now follow the showcase: Graphite, Glacier, Matcha, Rosewood, Terracotta, Sapphire, Sunrise, Mint, Orchid, and Slate. Each supplies all eight color roles, coordinated gradients and themed hover surfaces. Widget hover selections use the theme’s track color. Layouts, typography, artwork, semantic status/focus colors, and dashboard/Settings structure remain intact.
- **More quota colors:** an independent catalog provides **16 named choices** in each theme, plus the selected preset’s defaults and retained saved colors. All offered choices pass the existing 4.5:1 text and 3:1 quota-track rules across widget and dashboard surfaces. The active palette retains separate 5-hour/weekly opt-in controls and keyboard selection.
- **Migration:** schema 7’s Ocean → Glacier, Forest → Matcha, Plum → Orchid, and Amber → Terracotta; Original and Slate retain identifiers. Both enabled and inactive Light/Dark customization entries move with their selections. The format-4 checksummed settings/reminder transaction, revisions, placement, and other preferences are preserved. Legacy structural customizations remain supported.
- **Transparency:** a hard-coded false capability gate disabled ordinary debug and release shells. Original additionally bypassed the RGBA paint class. Both builds now use the same DWM capability check and transparent shell creation; Original takes the alpha paint path only when needed, preserving its 0% appearance. The optional no-redirection bitmap flag was removed: pinned Tao 0.37.1 skips its normal DWM transparent-window setup when that flag is enabled. Windows high contrast, disabled/unknown transparency, and native creation failures retain opaque fallbacks; Settings explains the effective Windows fallback. Main/preview reading windows remain opaque, and foreground elements keep protected backing.
- **Original comparison:** the base Light/Dark token blocks are identical to the actual v0.1.0 tag. Native assertions retain its frame, gradients, raised/wash surfaces, credits, badges, hover, synthetic tray-preview and dashboard colors. No speculative recoloring was applied to Original.
- **Release checks:** the ordinary executable accepts the existing isolated appearance/sizing flags. They use a separate validation identity and EXE-relative configuration under target/validation, synthetic usage, disabled reminders, and no Codex worker. Diagnostic frontend IPC is rejected outside these sessions. Normal startup and narrow capabilities are unchanged.

### Verification

Environment: Windows **26200.9711 (25H2)**, WebView2 **155.0.4283.62**. The current native sizing run reports scale **1.0**; it does not establish a multi-DPI matrix.

| Check | Result |
| --- | --- |
| npm.cmd test | Passed: **76** frontend tests, including all 22 palettes, every offered quota swatch, Original fixtures and customization memory. |
| npm.cmd run build | Passed, including TypeScript checking. |
| npm.cmd run rust:test | Passed: **149 app tests**, one explicit live probe ignored, plus **3 autostart tests**. Includes schema-7 migration, inactive colors, one-time fitting, navigation, and separate popup bounds. |
| npm.cmd run rust:fmt | Passed. |
| npm.cmd run rust:check | Passed during implementation; final source is also compiled by release build and all-target Clippy. |
| npm.cmd run rust:clippy | Passed with all targets and warnings denied. |
| npm.cmd run desktop:build | Blocked by the existing local Tauri CLI binding: invalid Win32 application / ERR_DLOPEN_FAILED. No tooling was installed or changed. |
| cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol | Passed; embeds the production frontend in the actual requested release executable. No installer/bundler validation is claimed. |
| Release --check-native-appearance | Passed: **88 theme/mode combinations** (66 desktop shells plus 22 tray previews using an app-owned synthetic anchor), Original Light/Dark native cascades, 0/20/80 requested background alpha without whole-widget opacity, Settings sections, manual resize, popup reopening, draft Save/Cancel behavior, Original’s translucent credit tint, and a synthetic 150% text-size selector layout check. |
| Release --check-native-sizing | Passed: **104 compact combinations**, client/rounded-region sizing, period/countdown/label combinations, docking, popup placement/stacking, foreground preservation, tray recovery and the appearance/navigation assertions. |
| Release composition pixel probe | **Blocked by capture availability:** failed with native-capture-unavailable; artifact identity is recorded below. CSS/native capability checks do not establish actual desktop pixel blending. |

The final capture probe uses explicit app-owned black/white backdrops, verifies sample ownership, rejects invalid pixels, and calibrates opaque samples before accepting alpha deltas. Ordinary screen capture returns a valid DC with an **empty clipping region**, so the composed WebView sample is unavailable. Normal-desktop-access retries show the same limitation. This is not evidence that alpha blending fails, and no capture failure is treated as a pass. GDI child-window reads can produce blank pixels and are not accepted as proof of composition.

Validation logs are ignored under src-tauri/target/validation/: showcase-frontend-tests.log, showcase-frontend-build.log, showcase-rust-tests.log, showcase-clippy.log, showcase-release-appearance.log, and showcase-release-sizing.log. Native result.log markers are checked explicitly because the application exit code alone does not establish a passing check. Native runs still emit the previously observed tray-removal/WebView2 class-unregistration shutdown diagnostics.

Final executable: `src-tauri/target/release/codex-usage-widget.exe`, **13,530,112 bytes**, file/product version **0.1.2**, SHA-256 `C9AA8A6D503B6DE29E2847D8D649886DDC8A84587C7351ACEFD128122E0516CA`. Release appearance and broad sizing checks ended with `native-check result=pass`. The separate final release composition probe ended with `native-check result=failed code=native-capture-unavailable`; its record is `src-tauri/target/validation/showcase-release-composition.log`. The broad sizing run preceded the final credit-paint, responsive-selector and fallback-message refinement; the final appearance/navigation check passed afterward. Actual desktop blending still requires manual confirmation. All validation application processes exited.

### Reproduce current release checks

Run npm.cmd run build, then the embedded Cargo release command above. Execute src-tauri/target/release/codex-usage-widget.exe with --check-native-appearance or --check-native-sizing, one at a time. Add --probe-native-composition to the appearance command for the explicit desktop pixel test. Each exits automatically; require native-check result=pass in its EXE-relative validation result.log.

Manually confirm actual desktop blending at 0/20/80 over light, dark, patterned and moving backgrounds. Windows 10, mixed-DPI pointer drags, text scaling/high-contrast transitions, auto-hide taskbars, Explorer recovery, screen readers, startup flashes, and visual screenshot review remain unverified. No inference requests, credential access, reset-credit operations, notification submissions, installer, publication, or push formed part of this work.

---

The dated evidence below describes earlier iterations and is retained as history; it does not override the current status above.

Date: 25 October 2026. Source version: 0.1.2. Design: [appearance and consistent snapping](widget-appearance-and-snap-design.md).

## Earlier delivered behavior — 25 October

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

## Historical native transparency gate — 25 October

**In the 25 October implementation, desktop transparency was unavailable in the shipped path.** A compact advanced row explains the disabled numeric control; stored preferences, preset colors and gap editing remain available.

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
