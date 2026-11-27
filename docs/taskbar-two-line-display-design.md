# Two-Line Taskbar Strip Display Mode — UI/UX & Implementation Design Report

**Status:** Original design proposal. See the [implementation report](taskbar-strip-implementation-report.md) for current styling and behavior.

**Date:** 2026-11-26

**Target platforms:** Windows 10 and Windows 11 (x64)

**Selected layout basis:** Option C · Tight Label + Configurable Micro Quota Bar (with `5h:` / `Wk:` colon labels)

**Related documents:** [UI/UX design report](codex-widget-design-report.md), [Taskbar & system tray display feasibility](taskbar-and-tray-display-feasibility-report.md), [Two-line preview comparison](taskbar-two-line-preview.html)

---

## 1. Summary & Scope

This report specifies a new compact display mode — **Taskbar Strip** (`DisplayMode::TaskbarStrip`, serialized key `"taskbar-strip"`) — that presents both **5-hour** and **Weekly** remaining quota percentages and expected reset countdowns on **two color-coded lines**, proportioned to harmonize with the Windows two-line taskbar clock (`16:37` / `2026/11/26`).

### Core Design Decisions
1. **Selected layout (Option C with colon labels):** Two stacked rows (`5h:` on row 1, `Wk:` on row 2), each colored with the active theme's contrast-checked quota text color (`--quota-five-hour-text` and `--quota-weekly-text`), paired with a **configurable 2 px micro quota bar** beneath each row.
2. **Toggleable micro quota bar:** Users can turn the micro quota bar **On** (default, ~40 logical px tall) or **Off** (~34 logical px tall) in Settings, with live temporary preview via the existing display-preview session.
3. **Strict column width & alignment discipline:** Explicit, predetermined column tracks for `5h:` / `Wk:`, remaining `%`, and reset countdown prevent awkward gaps between `5h:` and `%`, prevent `Wk:` collisions, and prevent layout jitter or clipping across worst-case strings (`99.9%`, `100%`, `<1%`, `23h59m`, `6d23h`, `due?`).
4. **Officially supported placement (Phase 1 default):** Docked flush above the Windows taskbar clock/tray (bottom-right work-area anchor) using standard Tauri/Win32 windowing — 100% reliable across Windows 10 and Windows 11 with zero shell injection.
5. **Architecture alignment:** Reuses the single authoritative Rust `Service`, `Interaction` state machine, `shell` / `preview` / `main` WebView2 surfaces, `WidgetAppearance` themes, `display_preview`, and `compact_sizing` monitor-fit validation. System Tray remains the always-available recovery and control surface.

---

## 2. Visual & Typographic Specification

### 2.1 Row Anatomy

With micro quota bars **On** (default):

```text
┌─────────────────────────┐
│  5h: 50%        2h17m   │  ← Row 1: 5-hour quota text color (--quota-five-hour-text)
│  ██████████░░░░░░░░░░   │  ← 2 px micro bar (--quota-five-hour on --track), toggleable
│  Wk: 60%        3d11h   │  ← Row 2: Weekly quota text color (--quota-weekly-text)
│  ████████████░░░░░░░░   │  ← 2 px micro bar (--quota-weekly on --track), toggleable
└─────────────────────────┘
```

With micro quota bars **Off**:

```text
┌─────────────────────────┐
│  5h: 50%        2h17m   │  ← Row 1: 5-hour quota text color
│  Wk: 60%        3d11h   │  ← Row 2: Weekly quota text color
└─────────────────────────┘
```

### 2.2 Column Width & Alignment Engineering

To satisfy both tight visual coupling (`5h: 50%` / `Wk: 60%`) and the project's **stable predetermined compact-shell sizing policy** ([codex-widget-design-report.md §4.7](codex-widget-design-report.md)), each row uses a **fixed 3-column CSS Grid** with `font-variant-numeric: tabular-nums` and `white-space: nowrap`:

| Column | Content Examples | Font (at 100% text scale) | Track Width | Alignment | Design Rationale |
|---|---|---|---:|---|---|
| **Col 1 — Period Label** | `5h:`, `Wk:` *(fallback: `1h:`, `1d:`, `P1:`, `P2:`)* | `10.5 px` (`0.65625rem`), weight `500`, `78%` row color opacity | **`22 px`** (`1.375rem`) | **Right** | `Wk:` (~19.5 px in Segoe UI) is ~4 px wider than `5h:` (~15.5 px). Right-aligning Col 1 locks both `:` colons on one vertical axis so the gap from `:` to the first digit is identical on both rows, while reserving headroom for `Wk:` and custom durations without collision. |
| **Gap 1** | Label `:` → Percentage | — | **`3.5 px`** (`0.21875rem`) | — | Tightly pairs `5h:` with `50%` and `Wk:` with `60%` — eliminates the excessive whitespace seen with right-aligned wide `%` tracks. |
| **Col 2 — Remaining `%`** | `8%`, `50%`, `<1%`, `100%`, `99.9%`, `—` | `12 px` (`0.75rem`), weight `650`, `100%` row color | **`35 px`** (`2.1875rem`) | **Left** | Left-aligning Col 2 keeps the `3.5 px` gap after `:` constant whether the reading is `8%`, `50%`, `100%`, or `99.9%`. Reserving `35 px` fits `percent()` worst-case `99.9%` (~33 px in Segoe UI Semibold tabular) with zero clipping or window resize. Extra room for short values (`8%`, `50%`) falls naturally *between* `%` and reset time where separation aids scannability. |
| **Gap 2** | Percentage → Reset time | — | **`6 px`** (`0.375rem`) | — | Ensures `99.9%` and `23h59m` never touch even in simultaneous worst-case readings. |
| **Col 3 — Remaining Reset Time** | `45m`, `2h17m`, `3d11h`, `23h59m`, `due?`, `—` | `11 px` (`0.6875rem`), weight `450`, `82%` row color *(respects Bold / Higher contrast countdown switches)* | **`40 px`** (`2.5rem`) | **Right** | Fits `compactReset()` worst-case `23h59m` (~38 px) and `14d23h` cleanly without truncation. Right-aligning Col 3 keeps both rows flush against the right edge, visually echoing Windows' right-aligned two-line clock. |

#### Alignment Verification Across Edge Widths

```text
Normal (2-digit %):         Short + 3-digit %:          Worst-case fractional + long reset:
┌──────────────────────┐    ┌──────────────────────┐    ┌──────────────────────┐
│  5h: 50%      2h17m  │    │  5h: 8%         45m  │    │  5h: 99.9%   23h59m  │
│  Wk: 60%      3d11h  │    │  Wk: 100%     6d23h  │    │  Wk: 99.9%    6d23h  │
└──────────────────────┘    └──────────────────────┘    └──────────────────────┘
```

* **Alternative label alignment note:** If user testing prefers flush-left `5h:` / `Wk:` edges over aligned colons, switching Col 1 to `text-align: left` keeps the exact same `22 px` track and outer dimensions, at the cost of ~4 px extra space after `5h:`. **Aligned colons (`text-align: right`) is the recommended default** because it guarantees uniform tightness after `5h:` and `Wk:`.
* **Non-standard / unknown period durations:** Per [codex-widget-design-report.md §3](codex-widget-design-report.md), never label an unknown duration as `5h:`. Map durations as:
  * `300` min → `5h:`
  * `10080` min → `Wk:`
  * `n % 1440 == 0` → `${n / 1440}d:`
  * `n % 60 == 0` → `${n / 60}h:`
  * other positive `n` → `${n}m:`
  * `null` (unknown duration) → `P1:` / `P2:` (with full `Primary/Secondary window (duration unavailable)` in `aria-label`, hover preview, and Overview).

### 2.3 Micro Quota Bar Specification (Configurable On/Off)

* **Setting:** `widgetAppearance.taskbarStripShowBars` (boolean, default `true`, propagated into `EffectiveAppearance` for live preview in Settings).
* **Geometry when enabled:**
  * One full-row horizontal bar directly underneath each row's text (`margin-top: 1.5 px`).
  * Height: `2 logical px` (`0.125rem`), `border-radius: 2 px`.
  * Width: Spans the full inner row content width (`Col 1 + Gap 1 + Col 2 + Gap 2 + Col 3 = 106.5 px` at 100% scale).
  * Fill width: `clamp(0%, remainingPercent, 100%)` in `--quota-five-hour` (row 1) or `--quota-weekly` (row 2) over `--track`.
  * **Value `0 < p < 1` (`<1%`):** Render at least `2 px` minimum visible fill sliver so `<1%` remains distinguishable from `0%`.
  * **Unavailable (`remainingPercent === null`):** Render a hollow dashed track (`1 px dashed var(--border)`, transparent fill) — never an empty solid track indistinguishable from `0%`.
  * **Stale / `last_known`:** Reduce bar fill opacity (`0.65`) and outline or subtly dash the track to mirror existing `.retained` semantics.
* **Geometry when disabled:** Omit bar DOM nodes completely and shrink shell height cleanly to the text-only baseline (do **not** leave dead vertical space).

### 2.4 Predetermined Stable Dimensions (100% Text & 100% Widget Scale)

Outer padding: `7 px` horizontal (`0.4375rem`), `3 px` top/bottom (`0.1875rem`). Border radius: `8 logical px` (matching native `CreateRoundRectRgn` clipping scaled by `widget_scale`).

| Visible Periods | Micro Bars | Warning Visible | Logical Width | Logical Height | Notes |
|---|---|---|---:|---:|---|
| **Both** (`5h` + `Wk`) | **On** (default) | No | **`122 px`** | **`42 px`** | Fits cleanly within Windows 11's 48 px taskbar height profile |
| **Both** (`5h` + `Wk`) | **On** | Yes | **`138 px`** | **`42 px`** | Adds `+16 px` right slot only while warning is active |
| **Both** (`5h` + `Wk`) | **Off** | No | **`122 px`** | **`36 px`** | Same width; collapses height by 6 px |
| **Both** (`5h` + `Wk`) | **Off** | Yes | **`138 px`** | **`36 px`** | Adds `+16 px` right slot only while warning is active |
| **Single** (`5h` or `Wk`) | **On** | No | **`122 px`** | **`28 px`** | Keeps comfortable click height (`≥ 28 px`) |
| **Single** (`5h` or `Wk`) | **Off** | No | **`122 px`** | **`24 px`** | Single-line compact pill (+16 px width if warning visible) |

* **Uniform scaling:** Supports independent `taskbarStripScalePercent` (`80%–300%`, default `100%`) validated by [compact_sizing.rs](../src-tauri/src/compact_sizing.rs) using monitor work area, native frame insets, `TextScaleFactor`, snap gap, taskbar safety, and visible-warning allowance (identical to `ScreenEdge` and `CompactHud`).
* **No layout jump on value changes:** Changing from `8%` to `100%` or `45m` to `23h59m` never changes the widget's outer pixel dimensions.

---

## 3. Honest Data Semantics & Edge States

Consistent with [codex-widget-design-report.md §3 & §4.7](codex-widget-design-report.md):

| State | Row Rendering | Micro Bar (if On) | Warning Indicator (+16 px right slot) |
|---|---|---|---|
| **Fresh valid data** | Full color `%` + softer countdown (`5h: 50%  2h17m`) | Solid fill (`remainingPercent%`) | None (slot omitted) |
| **Reset deadline elapsed** (`resetsAt <= now`) | `%` retains observed value; countdown shows `due?` in `--warning` semibold | Unchanged (never locally resets to `100%`) | None unless stale/incomplete |
| **Stale / `last_known`** | Both rows dimmed (`opacity: 0.75`) | Dimmed fill + subtle dashed border | Amber warning icon (`status-warning`, tooltip explains *Last known · out of date*) |
| **Single window missing (1–2 reads)** | Last-known value retained with dimmed styling; or `—` / `—` if no prior value | Dashed track if `null` | Amber warning icon |
| **Unavailable (`remainingPercent === null`)** | Label stays row-colored; `%` and countdown show `—` in `--secondary` (never `0%`) | Hollow `1 px dashed` track | Amber warning icon |
| **Checking / Verifying** | Labels + `—` / `—` (or retained last-known if verified ordinary retry) | Hollow track | Amber warning icon with status tooltip (*Checking usage…* / *Verifying account…*) |
| **Ordinary usage blocked** (`ordinaryUsageAllowed === false`) | Values stay visible; blocked status takes priority | Unchanged | Critical red `!` / warning mark taking priority |

---

## 4. Theming, Taskbar Surface Matching & Accessibility

1. **Taskbar-aware system polarity ([platform.rs](../src-tauri/src/platform.rs)):**
   * Extend `platform::appearance()` to also read `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\SystemUsesLightTheme` (exposed as `system_resolved_theme: "light" | "dark"`), alongside `AppsUseLightTheme`.
   * When `themePreference == System`, `TaskbarStrip` may follow `system_resolved_theme` (so users running Light Apps on a Dark Windows taskbar get a dark strip that blends naturally beside the clock), while explicit `Light` or `Dark` overrides take precedence.
2. **Presets & custom quota colors:**
   * Automatically inherits all 11 paired Light/Dark presets and custom 5-hour / Weekly quota swatches via `--quota-five-hour-text`, `--quota-weekly-text`, `--quota-five-hour`, and `--quota-weekly`.
   * Respects `showBorder` (turning border off preserves outer dimensions), `compactCountdownBold` (weight `700` on Col 3), and `compactCountdownHighContrast` (`100%` opacity / main text contrast on Col 3).
3. **Windows High Contrast (`forced-colors`):**
   * System colors (`Canvas`, `CanvasText`, `LinkText`, `Highlight`) take precedence; visible outer border is enforced regardless of `showBorder`; `5h:` / `Wk:` labels remain mandatory so both rows remain distinguishable without custom hue differences; micro bars use `1 px solid CanvasText` frame with `Highlight` fill.
4. **Reduced Motion & Text Scaling:**
   * No animated bars or cycling transitions under `prefers-reduced-motion` / `SPI_GETCLIENTAREAANIMATION == 0`.
   * Uses `rem`-based sizing so Windows `TextScaleFactor` (`100%–225%`) scales the strip cleanly within `compact_sizing` monitor budgets.
5. **Keyboard & Screen Reader Support:**
   * `role="button"`, `tabIndex={0}`, full accessible `aria-label` announcing both periods, remaining percentages, reset forecasts, freshness status, and interaction hints; supports `Enter`/`Space` (open Overview), `Escape` (close popup), and arrow-key nudging when unlocked.

---

## 5. Placement, Snapping & Interaction Model

### 5.1 Phase 1 Default Placement (Officially Supported — Windows 10 & Windows 11)
* **Default anchor:** Bottom-right of the primary monitor's work area (`HorizontalAnchor::Right`, `VerticalAnchor::Bottom`), placing the strip **flush above the Windows taskbar clock/tray** respecting the shared `snap_gap_logical_px` (with a recommended default of `4–8 px` or existing user snap gap) and auto-hide taskbar safety inset (`native_windows::taskbar_safety`).
* **Drag, snap & lock:** Entire strip is draggable (`Surface::Shell`, 6 px drag threshold via [widgetGestures.ts](../src/widgetGestures.ts)), supports multi-monitor dragging and shared work-area edge snapping (`geometry::release_anchors` / `resolve_anchors`), respects `lockPosition`, and **Reset position** restores bottom-right anchoring above the taskbar clock.
* **Always on top & fullscreen suppression:** Honors existing `alwaysOnTop` and `suppressFullscreen` settings (hidden over fullscreen apps and on session lock).

### 5.2 Phase 2 Optional Placement (Explicitly Deferred — Experimental In-Taskbar Overlay)
* **Not included in Phase 1** to keep initial delivery 100% officially supported and zero-risk across Windows 10 and Windows 11 builds.
* If pursued later as an explicit opt-in experimental toggle, it **must not** use `SetParent` injection into `Shell_TrayWnd` or DeskBands; it must remain an unparented `WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` window positioned relative to taskbar/tray geometry with automatic fallback to Phase 1 work-area docking whenever taskbar geometry is uncertain, auto-hide is active, or Explorer restarts.

### 5.3 Interaction Semantics (Reuses Existing `Interaction` State Machine)
* **Hover (`200 ms` dwell / `350 ms` leave grace + logical pointer corridor):** Opens the existing `preview` window anchored cleanly above/centered on the strip (via `geometry::anchored` / `geometry::centered_panel`), showing full quota cards, used %, exact local reset timestamps with timezone, credits, and a *Details* button.
* **Left single-click:** After Windows' double-click interval (`ClickActivation`), opens the `main` Overview popup anchored cleanly above the strip.
* **Left double-click:** Cancels pending popup and opens/raises the standard `main` dashboard window at Overview.
* **Right-click:** Closes transient preview and opens the shared native `TrayControls` menu at the pointer.
* **Refresh cadence:** Counts as `compact_visible()` (`compact_seconds` cadence while visible, `foreground_seconds` when preview/dashboard is open, `background_seconds` when hidden/locked).

---

## 6. Architecture & Codebase Implementation Guide

No new WebView2 windows and **no Tauri capability/permission changes** (`src-tauri/capabilities/` stays unchanged) are needed.

### 6.1 Settings Schema & Migration (`schemaVersion`: `13` → `14`)

Extend `Settings` in [model.rs](../src-tauri/src/model.rs) and migrate in [storage.rs](../src-tauri/src/storage.rs):

| Field | Location | Type | Default | Validation |
|---|---|---|---|---|
| `DisplayMode::TaskbarStrip` | `model::DisplayMode` | enum variant (`"taskbar-strip"`) | — | Accepted in mode selection, tray menu, `placements`, `snapAnchors`, `visiblePeriods`, `resetCountdowns` |
| `taskbar_strip_show_bars` | `widget_appearance::WidgetAppearance` | `bool` | `true` | Always valid; included in `display_preview::Patch` for live preview in Settings |
| `taskbar_strip_scale_percent` | `widget_appearance::WidgetAppearance` | `u16` | `100` | `80..=300`; monitor-fit validated in `compact_sizing.rs` |
| Default `placements["taskbar-strip"]` | `WindowSettings::default()` | `Placement` | `{ monitor: None, x: 0.0, y: 0.0 }` | Clamped & resolved via bottom-right `snapAnchors` |
| Default `snap_anchors["taskbar-strip"]` | `WindowSettings::default()` | `SnapAnchors` | `{ horizontal: Right, vertical: Bottom }` | Anchors flush above taskbar clock on first selection and Reset Position |

* **Migration (`version < 14` in [storage.rs](../src-tauri/src/storage.rs)):**
  * Increment `schemaVersion` to `14` (update upper bound check `if version > 14`).
  * Insert default `placements["taskbar-strip"]` and `snapAnchors["taskbar-strip"] = { horizontal: "right", vertical: "bottom" }` if absent.
  * Rely on `#[serde(default)]` for `taskbarStripShowBars = true` and `taskbarStripScalePercent = 100`.
  * Update `Settings::validate()` allowlists in [model.rs](../src-tauri/src/model.rs):
    * `snap_anchors` max len `4`, allow `"taskbar-strip"`.
    * `placements` max len `4`, allow `"taskbar-strip"`.
    * `visible_periods` allow `"taskbar-strip"`.
    * `reset_countdowns` allow `"taskbar-strip"` (note: for `TaskbarStrip`, countdowns are always shown by design unless a future toggle is added; default both `true` or ignore per-mode countdown checkboxes cleanly so the two-line layout remains deterministic).

### 6.2 Rust Backend File-by-File Map

| File | Required Changes |
|---|---|
| [src-tauri/src/model.rs](../src-tauri/src/model.rs) | Add `DisplayMode::TaskbarStrip` (`key() => "taskbar-strip"`), bump `schema_version` to `14`, add default bottom-right placement & `snap_anchors` for `"taskbar-strip"`, update `Settings::validate()` allowlists, add `system_resolved_theme: String` to `Appearance`. |
| [src-tauri/src/storage.rs](../src-tauri/src/storage.rs) | Add `version < 14` migration step and update max supported version to `14`; add migration round-trip test. |
| [src-tauri/src/platform.rs](../src-tauri/src/platform.rs) | Read `SystemUsesLightTheme` into `Appearance.system_resolved_theme` (fallback to `resolved_theme` if registry key missing). |
| [src-tauri/src/widget_appearance.rs](../src-tauri/src/widget_appearance.rs) | Add `taskbar_strip_show_bars: bool` (default `true`) and `taskbar_strip_scale_percent: u16` (default `100`, validated `80..=300`) to `WidgetAppearance` and propagate `taskbar_strip_show_bars` into `EffectiveAppearance`. |
| [src-tauri/src/compact_sizing.rs](../src-tauri/src/compact_sizing.rs) | Include `DisplayMode::TaskbarStrip` in `requested()`, `base_size()` (mirroring §2.4 table including `taskbar_strip_show_bars` and `+16 px` warning width), `resolve_with_budgets()`, and `issues()`. |
| [src-tauri/src/windows.rs](../src-tauri/src/windows.rs) | Set default initial `shell_size` (`(122.0, 42.0)`), corner radius (`8.0 * widget_scale`), `max_width` allowance (`3000.0`), Reset Position default anchors (`HorizontalAnchor::Right`, `VerticalAnchor::Bottom` for `TaskbarStrip`), and preview/popup anchoring (`geometry::anchored` / `geometry::centered_panel`). |
| [src-tauri/src/native_windows.rs](../src-tauri/src/native_windows.rs) | Register `TaskbarCreated` message in `control_proc` to dispatch `Input::Layout` / `Input::Environment` so taskbar-docked shells and tray rect recover cleanly after Explorer restarts. |
| [src-tauri/src/lib.rs](../src-tauri/src/lib.rs) | Add `(DisplayMode::TaskbarStrip, "Taskbar Strip")` to `TrayControls` `Display mode` submenu and handle `"mode-taskbar-strip"` menu event. |
| [src-tauri/src/interaction.rs](../src-tauri/src/interaction.rs) | Verify `can_preview()` and `compact_visible()` automatically include `DisplayMode::TaskbarStrip` (already true since only `Floating` suppresses shell hover and only `SystemTray` hides `shell`); add unit test coverage. |

### 6.3 Frontend File-by-File Map

| File | Required Changes |
|---|---|
| [src/state.ts](../src/state.ts) & [src/widgetAppearance.ts](../src/widgetAppearance.ts) | Add `"taskbar-strip"` to `DisplayMode`, `taskbarStripShowBars` & `taskbarStripScalePercent` to `WidgetAppearanceSettings` / `EffectiveWidgetAppearance`, and `systemResolvedTheme` to `AppState.appearance`. |
| [src/compactLayout.ts](../src/compactLayout.ts) | Add deterministic `"taskbar-strip"` sizing branch matching [compact_sizing.rs](../src-tauri/src/compact_sizing.rs) (`width: 122 + (warning ? 16 : 0)`, `height` by `visiblePeriods` × `showBars`). |
| `src/TaskbarStripReadings.tsx` *(new)* | Focused presentation component rendering the 1 or 2 color-coded rows (`5h:` / `Wk:` right-aligned colon track, left-aligned `percent()`, right-aligned `compactReset()`, and optional 2 px micro quota bar), with honest `stale` / `due?` / `unavailable` / `blocked` styling. |
| [src/SurfaceView.tsx](../src/SurfaceView.tsx) & [src/WidgetContents.tsx](../src/WidgetContents.tsx) | Include `"taskbar-strip"` in compact shell scaling, visible-period filtering, stable-compact layout styling, and render `<TaskbarStripReadings />` for `mode === "taskbar-strip"`. |
| [src/App.tsx](../src/App.tsx), [src/WidgetAppearanceSettings.tsx](../src/WidgetAppearanceSettings.tsx), [src/PeriodSettings.tsx](../src/PeriodSettings.tsx), [src/visuals.tsx](../src/visuals.tsx) | Add **Taskbar Strip** radio card + miniature `ModePreview`, **Show micro quota bars** checkbox (live-previewed via `useDisplayPreview`), **Taskbar Strip size (80%–300%)** slider, and **Visible periods** selector for Taskbar Strip. |
| [src/styles.css](../src/styles.css) | Add `.mode-taskbar-strip` CSS Grid rules, `tabular-nums`, row color tokens, `.micro-bar` styling, High Contrast `@media (forced-colors: active)` overrides, and countdown bold/high-contrast rules. |

---

## 7. Complementary System Tray Enhancements (Optional Follow-Up)

To maintain cohesion if the user switches to quiet `SystemTray` mode (or hides the Taskbar Strip):
1. **Two-row Tray Hover Preview & Tooltip:** Upgrade [summary.ts](../src/summary.ts) and [service.rs::tooltip](../src-tauri/src/service.rs) from "earliest reset only" to two aligned rows displaying **both** 5-hour and Weekly remaining `%` and **both** reset countdowns (with a safe 127 UTF-16 char cap on native tooltips).
2. **Dynamic System Tray Icon (`src-tauri/src/tray_icon.rs`):** Optional pure-Rust RGBA renderer for `TrayIcon::set_icon` (4-bar stack or numeric `%`) using `GetSystemMetricsForDpi(SM_CXSMICON, dpi)` and `SystemUsesLightTheme` — zero extra WebView2 memory.

---

## 8. Validation & Acceptance Checklist (For Implementation)

### 8.1 Automated Tests (Required Before Commit)
* **Rust (`npm.cmd run rust:test`, `rust:clippy`, `rust:fmt`):**
  * `model::tests`: `DisplayMode::TaskbarStrip` round-trip, `schemaVersion: 14` validation, allowlist checks for `placements`, `snap_anchors`, `visible_periods`.
  * `storage::tests`: v13 → v14 migration preserves all existing user settings and injects default bottom-right `"taskbar-strip"` anchor & `taskbarStripShowBars = true`.
  * `compact_sizing::tests`: `base_size()` parity against shared JSON fixture for all combinations (`both`/`five-hour`/`weekly` × `bars on/off` × `80–300%` × `100–225%` text scale), including monitor-fit rejection & fallback.
  * `display_preview::tests`: `taskbar_strip_show_bars` and `taskbar_strip_scale_percent` apply temporarily and revert cleanly on `EndPreview`.
  * `interaction::tests`: Hover dwell (`200 ms`), leave grace (`350 ms`), corridor, single-click popup, double-click window, and fullscreen suppression for `DisplayMode::TaskbarStrip`.
* **Frontend (`npm.cmd test`, `npm.cmd run build`):**
  * Policy tests verifying `compactLayout("taskbar-strip", ...)` matches Rust `compact_sizing::base_size` exactly.
  * Formatting & label mapping tests for `5h:`, `Wk:`, custom durations, `<1%`, `99.9%`, `100%`, `23h59m`, `due?`, and `—`.

### 8.2 Manual Windows 10 & Windows 11 Runtime Verification (Build Alone Is Insufficient)
* **Typography & alignment:** Verify `5h:` vs `Wk:` colon alignment and zero overlap/clipping across `8%`, `50%`, `99.9%`, `100%`, `45m`, `2h17m`, `23h59m`, `6d23h`, and `due?` at **100%, 125%, 150%, and 200% DPI** and **100%–225% Windows Text Scale**.
* **Micro bar toggle:** Toggle *Show micro quota bars* On/Off in Settings — verify instant temporary live preview, smooth height transition (`42 px` ↔ `36 px`) staying bottom-right anchored above the taskbar clock, Save persistence, and Cancel restoration.
* **Themes & High Contrast:** Verify Light Apps + Dark Taskbar (`SystemUsesLightTheme`), Light Taskbar, custom quota swatches, `showBorder` Off, and Windows High Contrast (labels visible, border visible, micro bars readable).
* **Windowing & recovery:** Verify drag + multi-monitor drop, Reset Position (returns to bottom-right above clock), auto-hide taskbar safety inset, fullscreen app suppression, hover preview anchoring, and Explorer restart (`taskkill /f /im explorer.exe && start explorer.exe`).
