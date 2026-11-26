# Taskbar Strip mode and two-row tray hover — implementation evidence

Date: 2026-11-26  
Specification: [Two-line taskbar strip design](taskbar-two-line-display-design.md) (Phase 1, §§1–6 & §8, plus §7.1 two-row tray hover preview and native tooltip; §5.2 experimental in-taskbar overlay and §7.2 dynamic RGBA tray icon remain deferred).  
Application version baseline: **0.1.5** (settings schema **13 → 14**). No dependencies, lockfiles, Tauri capabilities, Codex protocol reads, or installer/release artifacts were changed.

## Summary of implemented behavior

### 1. Settings schema v14, migration, and Rust policy (`src-tauri/src/`)

- **Fifth display mode (`DisplayMode::TaskbarStrip` / `"taskbar-strip"`):** Added to `model.rs`, `windows.rs`, `compact_sizing.rs`, `interaction.rs`, `lib.rs` (tray *Display mode* submenu), and `Settings::validate()` allowlists (`placements`, `snap_anchors`, `visible_periods`, `reset_countdowns`).
- **Schema 13 → 14 migration (`storage.rs`):** Preserves existing settings, reminders, revisions, placements, and legacy unanchored modes while injecting the default `"taskbar-strip"` placement `(0, 0)` and bottom-right work-area snap anchor (`HorizontalAnchor::Right`, `VerticalAnchor::Bottom`). Newer schemas (`> 14`) remain protected from overwrite.
- **Taskbar-strip appearance controls (`widget_appearance.rs`, `display_preview.rs`, `compact_sizing.rs`):**
  - `widgetAppearance.taskbarStripShowBars` (default `true`): propagated into `EffectiveAppearance` so live temporary display previews update immediately and revert cleanly on Cancel.
  - `widgetAppearance.taskbarStripScalePercent` (default `100`, validated `80..=300`): shares the per-monitor client-space fitting policy and fallback-to-100% logic with Edge and HUD.
  - Base logical dimensions at 100% text/scale: `122 × 42` (both periods, bars On), `122 × 36` (both periods, bars Off), `122 × 28` (single period, bars On), `122 × 24` (single period, bars Off), plus `+16 px` width (`138 px`) when a status warning or critical `!` indicator is active.
- **Windows taskbar polarity (`platform.rs`, `service.rs`, `model.rs`):**
  - Reads `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\SystemUsesLightTheme` into `Appearance.system_resolved_theme` (falling back to `AppsUseLightTheme` when absent; overridden by explicit Light or Dark theme preference).
  - Computes `Appearance.taskbar_widget` from `system_resolved_theme` while keeping `Appearance.resolved_theme` and `Appearance.widget` on `AppsUseLightTheme` for all other surfaces (including Tray Hover Preview, Overview, Floating, Screen Edge, and Compact HUD).
- **Window coordination & Explorer recovery (`windows.rs`, `native_windows.rs`, `service.rs`):**
  - Defaults `TaskbarStrip` to bottom-right work-area docking (`default_anchors()`), `8 px` scaled corner rounding, centered hover preview, and click-to-Overview popup above/below the strip.
  - Registers `RegisterWindowMessageW("TaskbarCreated")` in `native_windows::initialize()` and handles it in `control_proc` by dispatching `Action::Appearance` and `Input::Layout` to re-query polarity and re-clamp/re-dock after an Explorer restart.
  - Fixed a pre-existing bug in `windows.rs` where foreground `activation_denied` dispatched `Input::Reconcile` (which runs sleep/clock-resume stale-marking in `service.rs`) instead of `Input::Environment`, and guarded `freshness_deadline` expiration during synthetic presentation checks so static fixtures do not expire mid-validation.

### 2. Frontend Taskbar Strip surface, Settings UI, and §7.1 Tray Hover / Tooltip (`src/`)

- **Two-line Taskbar Strip (`src/taskbarStrip.ts`, `src/TaskbarStripReadings.tsx`, `src/WidgetContents.tsx`, `src/SurfaceView.tsx`, `src/styles.css`):**
  - Renders strict CSS Grid rows (`22 px` colon label `5h:` / `Wk:` + `35 px` right-aligned tabular remaining `%` + `1fr` right-aligned reset countdown) using the active theme's contrast-checked `--quota-five-hour-text` and `--quota-weekly-text` tokens and exact `122 px` / `138 px` border-box geometry.
  - Formats non-standard window durations honestly (`1h:`, `1d:`, `17m:`) and unknown durations as `P1:` / `P2:`; displays `<1%` with a `2 px` (`0.125rem`) minimum visible sliver, `due?` for elapsed unconfirmed resets, hollow dashed tracks for unavailable values, muted italic styling for retained/stale values, and a critical `!` glyph when `ordinaryUsageAllowed === false`.
  - Omits `.micro-bar` DOM elements completely when `taskbarStripShowBars` is `false`.
  - Applies `systemResolvedTheme` and `taskbarWidget` **only** to the docked `shell` surface when `mode === "taskbar-strip"`. Windows High Contrast / `forced-colors` rules retain system colors.
- **Settings UI (`src/App.tsx`, `src/WidgetAppearanceSettings.tsx`, `src/PeriodSettings.tsx`, `src/visuals.tsx`):**
  - Adds the *Taskbar Strip* mode card and miniature preview (`ModePreview`), *Show micro quota bars in Taskbar Strip* checkbox, *Taskbar Strip size (%)* slider/input (80–300% with per-monitor cap), *Taskbar Strip* period selector (`Both` / `5-hour only` / `Weekly only`), and a taskbar-matched theme editor label when `Follow Windows` has mixed app/taskbar polarity.
- **§7.1 Two-row Tray Hover Preview & native tooltip (`src/summary.ts`, `src/WidgetContents.tsx`, `src-tauri/src/service.rs`):**
  - Upgrades `trayPreview` to two aligned, read-only color-coded rows (`5h:` and `Wk:` with `%` and reset countdowns) plus an optional concise status line for stale, blocked, partial, verifying, or unavailable states. Continues honoring `windows.hoverEnabled` and the standard `resolvedTheme` / `state.appearance.widget` appearance logic.
  - Upgrades the Win32 native tray tooltip (`service::tooltip_at`) to a two-row fresh layout (`5h: 68% · 2h17m\nWk: 42% · 4d12h`) with honest stale/blocked/verifying/partial status priority and strict UTF-16 length capping (`<= 127` UTF-16 code units).

## Validation performed

All checks below ran against the final working tree after the last code edit unless noted. No live Codex CLI probe, inference request, credential access, installer build, or remote push was performed.

| Check | Command | Actual result |
| --- | --- | --- |
| Frontend type check | `npm.cmd run typecheck` | **Passed** (0 errors). |
| Frontend policy & layout tests | `npm.cmd test` | **Passed — 170 / 170 tests**, including new `tests/taskbar-strip.test.mjs`, updated `tests/compact-layout.test.mjs`, `tests/compact-scaling.test.mjs` (6 Taskbar Strip fixtures in `tests/fixtures/compact-scaling.json`), `tests/summary.test.mjs` (`trayHoverSummary`), and `tests/quota-presentation.test.mjs`. |
| Frontend production build | `npm.cmd run build` | **Passed** (`dist/` bundled with TypeScript check). |
| Rust formatting check | `npm.cmd run rust:fmt` | **Passed** (root crate and vendored `auto-launch`). |
| Rust compilation check | `npm.cmd run rust:check` | **Passed**. |
| Rust behavioral tests | `npm.cmd run rust:test` | **Passed — 183 app tests + 3 `auto-launch` tests passed; 1 explicit live probe ignored**. Covers schema-14 defaults/migration, `SystemUsesLightTheme` fallback, `taskbar_strip_show_bars` & `taskbar_strip_scale_percent` validation/preview, shared layout fixtures, bottom-right default anchor, `TaskbarStrip` interaction state machine, and two-row 127-UTF16 tooltip capping. |
| Rust lint checks | `npm.cmd run rust:clippy` | **Passed** (`--all-targets -- -D warnings`). |
| Tauri CLI wrapper build | `npm.cmd run desktop:build` | **Blocked by pre-existing environment issue:** installed `@tauri-apps/cli-win32-x64-msvc` native binding fails with `ERR_DLOPEN_FAILED` (consistent with earlier entries in [implementation-report.md](implementation-report.md)). No tooling or `node_modules` changes were made. |
| Embedded release executable build | `cargo build --release --manifest-path src-tauri/Cargo.toml --locked --features tauri/custom-protocol` | **Passed**. Produced `src-tauri/target/release/Usage.exe` (**14,106,624 bytes**, SHA-256 `82EDA8169A0BA40E2928CBCCA67A49B290E459E3BD89B2DF8FAB5352F432C701`). |
| Isolated native compact-options check | `Usage.exe --check-native-compact-options` | **Passed — `native-check result=pass` (182 / 182 pass assertions)** in `src-tauri/target/validation/native-compact-options/result.log`. Verifies Taskbar Strip bold/high-contrast countdowns, `taskbar-strip-micro-bars-off`, uniform scaling (80–300%), rounded regions, oversized rejection, and Settings preview/Cancel/Save reopen. |
| Isolated native appearance check | `Usage.exe --check-native-appearance` | **Passed — `native-check result=pass` (859 pass assertions)** in `src-tauri/target/validation/native-appearance/result.log`. Verifies Taskbar Strip bottom-right `ResetPosition` across `0/8/16/48 px` snap gaps, all preset/theme/border checks, two-row `TrayPreview` showcase styles, `compact_options_check`, dashboard navigation, and Settings preview/Cancel/Save. |
| Isolated native sizing check | `Usage.exe --check-native-sizing` | **Partial (environmental stop at final probe) — 1,711 pass assertions** covering all Taskbar Strip `both`/`five-hour`/`weekly` × `showBars: true/false` layouts (`122×42`, `122×36`, `122×28`, `122×24`), hover/tray preview sizing, `edge_docking`, full `appearance_check`, `reveal_check`, `popup_stacking` (including Taskbar Strip), and `floating_close_to_tray`. Stopped only on the final `activation_fallback` precondition (`activation-denial-precondition-inactive-popup`) because `GetForegroundWindow()` returned `0` after hiding `main` in an unattended desktop session. |
| Whitespace & process cleanup | `git diff --check` and `Get-Process -Name Usage` | **Passed**; zero lingering `Usage.exe` processes after validation. |

## Remaining manual Windows verification

Automated synthetic checks do not replace manual verification on real Windows 10 and Windows 11 desktops:

- [ ] Visual inspection of Taskbar Strip docked above the Windows 10 and Windows 11 taskbar clock with micro quota bars **On** (`122 × 42`) and **Off** (`122 × 36`).
- [ ] **Mixed OS polarity:** Windows *Custom* theme with Light Apps (`AppsUseLightTheme = 1`) + Dark Taskbar (`SystemUsesLightTheme = 0`) under *Follow Windows* — confirm Taskbar Strip renders Dark while Overview, Settings, and Tray Hover Preview render Light, and that explicit Light/Dark overrides both.
- [ ] **DPI & Text Scaling:** Verify crisp alignment and zero clipping at 100%, 125%, 150%, 175%, and 200% DPI and Windows Text Scale 100%–225%, plus worst-case strings (`100%`, `99.9%`, `<1%`, `23h59m`, `6d23h`, `due?`).
- [ ] **Windows High Contrast / Contrast Themes:** Confirm forced-colors legibility on Taskbar Strip and two-row Tray Hover Preview.
- [ ] **Multi-monitor & Taskbar edge cases:** Drag across mixed-DPI monitors, snap/release, *Reset position*, auto-hide taskbar behavior, and fullscreen video/game suppression.
- [ ] **Explorer restart recovery:** Restart `explorer.exe` via Task Manager while Taskbar Strip is active and verify automatic re-docking and polarity refresh on `TaskbarCreated`.
- [ ] **Two-row Tray Hover Preview & native tooltip:** Verify real pointer hover timing, `windows.hoverEnabled` toggle, click-to-Overview, and native Win32 tooltip display when tooltip fallback is active.
