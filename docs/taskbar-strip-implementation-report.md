# Taskbar Strip mode and two-row tray hover — implementation evidence

Date: 2026-11-27
Specification: [Two-line taskbar strip design](taskbar-two-line-display-design.md) (Phase 1 §§1–6 & §8, §5.2 opt-in unparented in-taskbar transparent overlay, and §7.1 two-row tray hover preview and native tooltip; §7.2 dynamic RGBA tray icon remains deferred).
Application version baseline: **0.1.5** (settings schema **13 → 14**). No dependencies, lockfiles, Tauri capabilities, Codex protocol reads, or installer/release artifacts were changed.

## Neutral text, aligned readings, and border controls — 2026-11-27

The strip uses the same neutral color and regular-weight Consolas for period labels, remaining percentages, and reset times. Words mode displays `5h   57% · 2h24m` and `7d   42% · 3d11h` in fixed columns. The shared **Quota labels** setting selects words or circular badges on Save. The existing bold and higher-contrast options apply to every text column, and the quota bars retain the active Light or Dark swatches.

The base width is 128 logical pixels at 100% scale, with 16 additional pixels when the status indicator is visible. The existing single- and two-row heights are preserved. Each row uses the same fixed label and percentage columns and an aligned reset column. The native and frontend size rules use the new width.

**Display → Taskbar Strip** groups placement, transparency, the outline, and micro bars. The border preference is saved in schema 14 as `taskbarStripBorder`. It defaults to Automatic when absent. Automatic follows the shared border preference for opaque cards and hides the outline when transparent. Show and Hide apply independently of background paint. Windows high contrast keeps its system outline and opaque background. The Taskbar Strip appearance settings use the existing live preview, Save, and Cancel flow.

Rust publishes the active transparency state after placing the window and configuring its native region. That state drives webview paint. The strip stays clear under the pointer when transparent. Moving above the taskbar, turning off Windows transparency effects, turning on Windows high contrast, or losing native composition uses an opaque fallback while preserving the preference. Inside the taskbar the strip follows the Windows taskbar theme. Above it the strip follows the app theme.

### Validation for the neutral-text and border update

| Check | Result |
| --- | --- |
| `npm.cmd test` | Passed, 171 tests. Includes both label styles, unknown periods, border policy, the Light/Dark swatch editor, and fallback messaging. |
| `npm.cmd run build` | Passed, including TypeScript checking and the production Vite build. |
| `npm.cmd run rust:fmt`, `npm.cmd run rust:check`, `npm.cmd run rust:clippy` | Passed. |
| `npm.cmd run rust:test` | Passed, 184 application tests and 3 autostart tests. The live protocol probe remained ignored. |
| `npm.cmd run desktop:build` | The installed Tauri CLI binding failed to load with `ERR_DLOPEN_FAILED`. |
| `cargo build --release --manifest-path src-tauri/Cargo.toml --locked --features tauri/custom-protocol` | Passed. Built the executable used for isolated Windows validation. |
| `Usage.exe --check-native-compact-options` | Passed, 194 assertions. Covered all-column readability controls, equal widths, fitting wide readings, app and taskbar theme selection, native and webview transparency, independent border paint, widget scale, and live preview, Cancel, and Save. See `src-tauri/target/validation/native-compact-options/result.log`. |
| `Usage.exe --check-native-appearance` | Passed, 871 assertions. Includes Taskbar Strip and existing compact widget and dashboard appearance checks. See `src-tauri/target/validation/native-appearance/result.log`. |

The later typography and label changes passed `node --test tests/taskbar-strip.test.mjs` (5 tests), `npm.cmd run build`, `npm.cmd run rust:fmt`, and `npm.cmd run rust:check`.

Manual checks remain for the physical pointer over the transparent strip, the effect of toggling Windows transparency and high contrast live, mixed-DPI monitor movement, and an actual Explorer restart on interactive Windows 10 and 11 desktops. The isolated Windows session exercised taskbar-band geometry and webview rendering using synthetic data.

## Phase 2 increment — Unparented in-taskbar overlay & transparent text-only style (2026-11-27)

Added user-requested in-taskbar placement and transparent background styling for **Taskbar Strip** without `SetParent` shell injection or DeskBand COM extensions:

- **Settings (`widget_appearance.rs`, `widgetAppearance.ts`, `WidgetAppearanceSettings.tsx`):**
  - `widgetAppearance.taskbarStripInTaskbar` (default `true`, serde-default backward-compatible with schema v14): places the unparented `TaskbarStrip` shell window inside the horizontal Windows taskbar band flush left of `TrayNotifyWnd` (or the system tray icon / right-edge fallback), vertically centered inside the taskbar height. Automatically falls back to Phase 1 bottom-right work-area docking above the taskbar when unchecked, auto-hide (`ABS_AUTOHIDE`) is enabled, the taskbar is vertical/missing, or the scaled strip exceeds taskbar height (`bar.height + 4 * scale`).
  - `widgetAppearance.taskbarStripTransparent` (default `true`): hides the card background gradient and outer border while placed inside the taskbar (`appearance.taskbarOverlayActive === true`), leaving crisp color-coded quota text (and optional 2 px micro bars via `taskbarStripShowBars`) over the native Windows taskbar chrome with a subtle native hover tint. Automatically stays opaque when fallen back above the taskbar, when Windows High Contrast is active, or when DWM composition is unavailable.
  - Both controls participate in live temporary preview via `display_preview::Patch` and revert cleanly on Cancel.
- **Win32 coordination (`native_windows.rs`, `geometry.rs`, `windows.rs`, `service.rs`):**
  - Added `native_windows::taskbar_band(monitor, work)` inspecting `Shell_TrayWnd` / `Shell_SecondaryTrayWnd` + `TrayNotifyWnd` (with OS work-area band fallback when `Shell_TrayWnd` is not exposed in an isolated desktop session) and pure `geometry::taskbar_overlay_rect(...)`.
  - Added `native_windows::is_above_taskbar(...)`, `raise_above_taskbar(...)`, taskbar mouse-up Z-order repair in `WH_MOUSE_LL`, and `clear_window_region(...)` (executed on the owning UI thread via `on_window_thread`) to remove GDI corner clipping while transparent.
  - Preserves horizontal dragging and Left/Right arrow nudging inside the taskbar band, with **Reset position** snapping back flush left of `TrayNotifyWnd`.
  - Forces `topmost: true` on `shell`, `preview`, and `main` popup while `taskbar_overlay_active` is active so the strip stays above `Shell_TrayWnd` and popups stay above the strip (while fullscreen suppression still hides it cleanly).
  - Replaced full `state.appearance = platform::appearance(...)` overwrites in `service.rs` with `refresh_platform_appearance(...)` so coordinator-owned fields (`taskbar_overlay_active`, `placement_constraints`) are never clobbered on settings save or 60-second reconciliation.

### Phase 2 validation (2026-11-27)

| Check | Actual result |
| --- | --- |
| `npm.cmd run typecheck` | **Passed** (0 errors). |
| `npm.cmd test` | **Passed — 170 / 170 tests** (including new `taskbarStripInTaskbar` / `taskbarStripTransparent` default, resolution, and disabled-state Settings assertions in `tests/taskbar-strip.test.mjs`). |
| `npm.cmd run build` | **Passed**. |
| `npm.cmd run rust:fmt`, `rust:check`, `rust:clippy` | **Passed** (`-D warnings`). |
| `npm.cmd run rust:test` | **Passed — 184 app tests + 3 `auto-launch` tests passed; 1 live probe ignored**, including new `geometry::tests::taskbar_overlay_places_flush_left_of_tray_and_falls_back_cleanly` and extended `widget_appearance` / `display_preview` tests. |
| Release executable build (`cargo rustc --release --locked --bin Usage --features tauri/custom-protocol -- -C extra-filename=-taskbar`) | **Passed** (`src-tauri/target/release/Usage-taskbar.exe`, **14,134,272 bytes**, SHA-256 `80FD874E3E49F6D838A3CD142BBA3EEB79F5B4347CF376BA8875D17FD82BC2CD`; built side-by-side because `Usage.exe` was locked by an active instance). |
| `Usage-taskbar.exe --check-native-compact-options` | **Passed — `native-check result=pass` (188 / 188 pass assertions)**, including `taskbar-strip-positioned-inside-taskbar-band=pass`, `taskbar-strip-transparent-clears-window-region=pass`, and `taskbar-strip-in-taskbar-transparent=pass` in both Light and Dark themes. |
| `Usage-taskbar.exe --check-native-appearance` | **Passed — `native-check result=pass` (865 pass assertions)**. |

Manual Windows 10/11 verification remains required for real `Shell_TrayWnd` / `TrayNotifyWnd` placement, taskbar click Z-order recovery, Win+D, Light/Dark acrylic readability, auto-hide fallback, and Explorer restart recovery.

---

## Summary of implemented behavior (Phase 1 + §7.1)

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

- [ ] Inspect Taskbar Strip inside and above the taskbar clock on Windows 10 and Windows 11 with micro bars **On** (`128 × 42`) and **Off** (`128 × 36`).
- [ ] Check live mixed Windows polarity with light apps and a dark taskbar. Confirm the strip uses the taskbar theme inside it and the app theme above it, including when the app theme is set explicitly.
- [ ] **DPI & Text Scaling:** Verify crisp alignment and zero clipping at 100%, 125%, 150%, 175%, and 200% DPI and Windows Text Scale 100%–225%, plus worst-case strings (`100%`, `99.9%`, `<1%`, `23h59m`, `6d23h`, `due?`).
- [ ] **Windows High Contrast / Contrast Themes:** Confirm forced-colors legibility on Taskbar Strip and two-row Tray Hover Preview.
- [ ] **Multi-monitor & Taskbar edge cases:** Drag across mixed-DPI monitors, snap/release, *Reset position*, auto-hide taskbar behavior, and fullscreen video/game suppression.
- [ ] **Explorer restart recovery:** Restart `explorer.exe` via Task Manager while Taskbar Strip is active and verify automatic re-docking and polarity refresh on `TaskbarCreated`.
- [ ] **Two-row Tray Hover Preview & native tooltip:** Verify real pointer hover timing, `windows.hoverEnabled` toggle, click-to-Overview, and native Win32 tooltip display when tooltip fallback is active.
