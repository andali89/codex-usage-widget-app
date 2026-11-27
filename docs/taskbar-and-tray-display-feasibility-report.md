# Taskbar & System Tray Usage Display — Technical Feasibility Report

**Status:** Investigation & design proposal (no source code modified)

**Date:** 2026-11-26

**Target platforms:** Windows 10 and Windows 11 (x64)

**Scope:** Evaluating display approaches that surface Codex usage status, remaining quota, and reset countdowns directly in the Windows taskbar or system tray, aligned with the existing Tauri 2 / Rust / React architecture.

---

## 1. Current Implementation Review

### 1.1 Architecture & Packaging
* **Stack:** Tauri 2 (`tauri 2.12.5`, `tao 0.37.1`, `wry 0.57.0`, `tray-icon 0.25.1`), Rust application service (`Usage.exe`), Win32 bindings (`windows-sys 0.61.2`, `windows 0.62.2`), and React 19 + TypeScript + Tailwind CSS v4 ([Cargo.toml](../src-tauri/Cargo.toml), [package.json](../package.json)).
* **Distribution & Identity:** Unsigned **per-user NSIS installer** and **portable ZIP** ([tauri.conf.json](../src-tauri/tauri.conf.json)). The application runs as an unpackaged Win32 desktop process with an explicit AUMID (`dev.codexusagewidget.desktop`) and **no MSIX package identity**.
* **Authoritative Rust Service:** A single Tokio actor in [service.rs](../src-tauri/src/service.rs) owns `AppState`, the local Codex app-server reader, refresh cadence, reminder evaluation, and native window coordination, emitting `widget-state` snapshots to webviews. Data normalization in [model.rs](../src-tauri/src/model.rs) explicitly distinguishes `remaining_percent`, `resets_at_ms`, `freshness`, `last_known`, and `ordinary_usage_allowed`.
* **Display Modes & Lazy WebView2 Surfaces:** Four `DisplayMode` variants (`Floating`, `ScreenEdge`, `SystemTray`, `CompactHud`) ([model.rs](../src-tauri/src/model.rs)) are rendered across three WebView2 windows ([windows.rs](../src-tauri/src/windows.rs), [main.tsx](../src/main.tsx)) that idle-retire after 5 minutes when hidden ([view_lifecycle.rs](../src-tauri/src/view_lifecycle.rs)):
  * `main`: Overview/Settings dashboard — frameless anchored popup (`DetailState::Popup`, 480 logical px) or standard taskbar-visible window (`DetailState::Window`, 620 logical px).
  * `shell`: Frameless desktop widget (`WS_EX_TOOLWINDOW`, `skip_taskbar(true)`, GDI `CreateRoundRectRgn` clipping, optional topmost, drag/snap anchoring via [geometry.rs](../src-tauri/src/geometry.rs)).
  * `preview`: Non-activating (`WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST`) hover summary (~260 × 76 logical px two-row box for tray hover, or quota cards for Edge/HUD).
* **Existing Windows Integration:**
  * **System tray ([lib.rs](../src-tauri/src/lib.rs)):** Single static bundled icon (`TrayIconBuilder::with_id("main")`), shared native context menu (`TrayControls`), `TrayIconEvent` geometry tracking, custom hover preview, and dynamic text tooltip fallback ([service.rs](../src-tauri/src/service.rs)).
  * **Win32 hooks & coordinator ([native_windows.rs](../src-tauri/src/native_windows.rs)):** Low-level mouse hook (`WH_MOUSE_LL`), `SetWinEventHook` (`EVENT_SYSTEM_FOREGROUND`, `EVENT_OBJECT_LOCATIONCHANGE`), window subclassing (`SetWindowSubclass`), hidden top-level broadcast receiver (`CodexUsageWidget.Coordinator`), and `SHAppBarMessage` auto-hide detection.
  * **Interaction state machine ([interaction.rs](../src-tauri/src/interaction.rs)):** 200 ms hover dwell, 350 ms leave grace, logical pointer corridor, OS double-click disambiguation (`ClickActivation`), fullscreen/lock suppression, and `tooltip_fallback` degradation.

### 1.2 Reusable Components & Technical Constraints
* **Reusable directly:** `Service` actor and scheduler, `Usage` normalization and honesty semantics, `Interaction` state machine, `TrayControls` menu, `preview` / `main` webviews, [WidgetContents.tsx](../src/WidgetContents.tsx) / [summary.ts](../src/summary.ts), and `WidgetAppearance` themes and swatches ([widget_appearance.rs](../src-tauri/src/widget_appearance.rs)).
* **Constraints and current gaps:**
  1. **Taskbar theme detection gap:** [platform.rs](../src-tauri/src/platform.rs) currently reads `AppsUseLightTheme` only. On Windows 10/11, taskbar and system tray chrome follow `SystemUsesLightTheme` (commonly Dark taskbar with Light apps).
  2. **Explorer restart sync:** `control_proc` in [native_windows.rs](../src-tauri/src/native_windows.rs) does not yet listen for `TaskbarCreated` to invalidate cached tray `Rect` bounds or re-sync dynamic icon/taskbar state after `explorer.exe` restarts.
  3. **Unpackaged Win32 distribution:** Lack of MSIX identity precludes Windows 11 Widgets Board providers (`IWidgetProvider`) and MSIX-only badge APIs.
  4. **Memory budget:** Keeping an additional persistent WebView2 alive adds ~30–60 MB RAM, whereas native Rust icon rendering or `ITaskbarList3` updates add negligible memory and let all webviews remain idle-retired.
  5. **Prior design scope:** [codex-widget-design-report.md §4.6](codex-widget-design-report.md) previously excluded an inline taskbar chip specifically because Windows 11 removed DeskBand support.

---

## 2. Taskbar Application Area Integration

### T1. Standard Taskbar Button + `ITaskbarList3` & DWM Thumbnail *(Official API)*
Host a taskbar-visible window (either the existing `main` window when open, or a dedicated minimized/taskbar-hosted mode) and drive documented Shell/DWM interfaces:
* **`ITaskbarList3::SetProgressValue` / `SetProgressState`:** Draws a remaining-quota progress bar directly inside the taskbar button (`TBPF_NORMAL` green, `TBPF_PAUSED` amber for low/stale, `TBPF_ERROR` red for 0%/blocked, `TBPF_INDETERMINATE` while verifying).
* **`ITaskbarList3::SetOverlayIcon` + `WM_SETICON`:** Displays a DPI-scaled dynamic app icon and/or a 16×16 logical px overlay badge (percentage or warning/blocked glyph).
* **`SetWindowTextW` (dynamic window title):** When taskbar button combining is disabled ("Never combine"), Windows renders the live title text (e.g., `5H 68% · W 42% · 2h17m`) directly inside the taskbar button.
* **`DwmSetIconicThumbnail` + `ITaskbarList3::ThumbBarAddButtons` + `SetThumbnailTooltip`:** Renders a custom bitmap summary card (both quotas + reset countdowns) with native Refresh/Settings buttons inside Windows' taskbar button hover preview.
* **Limitations:** Windows removes taskbar buttons for hidden windows (`SW_HIDE`), so persistent display requires keeping a window visible/minimized (occupying a taskbar and Alt+Tab slot). Under Windows 11's default **combined/icon-only** taskbar, button width is fixed (~40–44 logical px), so **reset countdown strings cannot be displayed inline** without hovering.

### T2. Legacy DeskBand (`IDeskBand` / `IDeskBand2` COM Shell Extension) *(Deprecated — Win10 Only)*
In-process COM DLL loaded by `explorer.exe` inside `ReBarWindow32`.
* **Assessment:** **Unviable.** Officially removed in Windows 11's XAML/WinUI taskbar rewrite; also requires COM registration, breaks portable ZIP distribution, complicates upgrades while Explorer holds the DLL, and exposes `explorer.exe` to crash risk.

### T3. Taskbar Child Injection (`SetParent` into `Shell_TrayWnd` / `ReBarWindow32`) *(Unsupported Workaround)*
Injects an external HWND into Explorer's taskbar hierarchy (the legacy TrafficMonitor pattern).
* **Assessment:** **Not recommended.** Windows 11's XAML Islands taskbar (22H2–24H2+) virtualizes/removes classic `ReBarWindow32` structures. `SetParent` causes Z-order fighting, DPI drift, overlap with centered icons/Widgets/Search, and `explorer.exe` instability across Windows updates.

### T4. Topmost Floating Strip: Overlapping vs. Taskbar-Adjacent
* **T4a — Overlapping inside the taskbar (`HWND_TOPMOST` over `Shell_TrayWnd`) *(Fragile Workaround)*:** Does not reserve taskbar layout space (covers Start/apps/tray icons), fights Explorer for Z-order when the taskbar or Start menu is clicked, and degrades with auto-hide taskbars and Win+D.
* **T4b — Taskbar-Adjacent Docked Strip *(Official / Safe)*:** Reuses the existing `CompactHud` shell anchored flush to the taskbar work-area edge (e.g., bottom-right, `snap_gap = 0–4`). Officially supported, renders both quotas, bars, and reset countdowns cleanly, respects auto-hide safety insets ([native_windows.rs](../src-tauri/src/native_windows.rs)), and requires minimal code — though it sits *above* rather than *inside* the taskbar.

### T5. Windows 11 Widgets Board Provider (`IWidgetProvider`) *(Win11 + MSIX Only)*
* **Assessment:** **Unviable** for Windows 10/11 parity or unpackaged NSIS/portable builds; taskbar display is OS-controlled and transient rather than a persistent usage strip.

---

## 3. System Tray (Notification Area) Integration

### S1. Dynamic Single System Tray Icon *(Official `Shell_NotifyIconW` / Tauri `TrayIcon::set_icon`)*
Replace the static bundled icon with a Rust-rendered RGBA bitmap pushed via `TrayIcon::set_icon(Some(Image::new_owned(rgba, w, h)))` whenever quota, freshness, theme, or DPI changes.
* **DPI-exact rendering:** Query `GetSystemMetricsForDpi(SM_CXSMICON, dpi)` for the tray monitor (`16×16` @ 100%, `20×20` @ 125%, `24×24` @ 150%, `32×32` @ 200%, `40×40` @ 250%) to avoid blurry OS resampling.
* **Effective 16×16–32×32 px visual encodings:**
  * *Dual Gauge (vertical bars or split ring):* Shows both 5-hour and Weekly remaining quota simultaneously in theme quota colors, with distinct stale/blocked/unavailable markers. Legible even at 100% DPI (16×16 px).
  * *Numeric Remaining %:* Crisp pixel-aligned 2-digit percentage (lowest remaining or selected period) + thin bottom progress strip; displays `--` for unavailable (never `00`), `!` for blocked, and muted/outlined styling for `last_known`/stale.
* **Limitations:** A single 1:1 square icon cannot legibly fit two numeric percentages *and* a reset countdown simultaneously. Windows 11 places newly created tray icons in the overflow flyout (`^`) by default until pinned by the user (no official API can force-pin).

### S2. Multi-Icon System Tray Strip (2–3 `Shell_NotifyIconW` Icons) *(Official API, Mixed UX)*
Register two adjacent tray icons via Tauri (`TrayIconBuilder::with_id`), e.g., `5H` numeric + `Weekly` numeric.
* **Trade-offs:** Officially supported on both Windows 10 and 11 and displays both percentages side by side in the notification area. However, Windows 11 requires the user to pin *each* icon individually in Taskbar settings, icon ordering can shift across Explorer restarts, and countdown text still belongs in hover/tooltip surfaces. Suitable only as an explicit opt-in variant.

### S3. Native Tooltips & Interactive Hover/Click Popups *(Official / Already Implemented)*
* **Native Tooltip (`NOTIFYICONDATAW.szTip`):** Officially supported everywhere (127 UTF-16 code units + NUL). [service.rs](../src-tauri/src/service.rs) already builds a two-line quota + earliest-reset summary; adding a 127-char truncation guard hardens edge cases with long period names.
* **Hover & Click Popups:** Already implemented via `preview` (non-activating ~260 × 76 two-row box) and `main` (480 px Overview popup / 620 px standard window), with `tooltip_fallback` when native geometry/hooks are unavailable.

---

## 4. Feasibility & Compatibility Comparison

| Approach | API Status | Win 10 / Win 11 | Inline Info Capacity | Visual Flexibility | Reliability (DPI / Explorer / Auto-hide) | Complexity & Maintenance |
|---|---|---|---|---|---|---|
| **S1. Dynamic Single Tray Icon** | Official (`Shell_NotifyIconW`) | Full / Full | Dual bars **or** 1 numeric % + status | Low-Medium (16–32 px square) | **High** (with DPI + `TaskbarCreated` sync) | **Low** / Very Low |
| **S2. Dual/Multi Tray Icons** | Official (`Shell_NotifyIconW`) | Full / Full | 2 numeric % (5H + Weekly) | Medium (2 square slots) | **Medium-High** (manual dual pinning; order drift) | **Low-Medium** / Low |
| **S3. Tray Hover + Click Popups** | Official + App HWND *(existing)* | Full / Full | Full %s + countdowns + status + actions | **High** (WebView2 React/CSS) | **High** (with native `tooltip_fallback`) | **Implemented** / Low |
| **T1. Taskbar Button (`ITaskbarList3`)** | Official (COM + DWM) | Full / Full | Progress fill + overlay badge (+ text if uncombined) | Medium (icon/bar + custom thumbnail) | **High** (occupies taskbar + Alt+Tab slot) | **Medium** / Low |
| **T4b. Taskbar-Adjacent HUD Preset** | Official App HWND *(existing)* | Full / Full | Full 5H + Weekly + bars + countdowns | **High** (full themes & scaling) | **High** (respects work area & auto-hide) | **Very Low** / Very Low |
| **T4a. Topmost Overlay on Taskbar** | Workaround (`HWND_TOPMOST`) | Fragile / Fragile | Full 5H + Weekly + countdowns | High | **Low** (Z-order wars, covers icons, auto-hide bugs) | **High** / High |
| **T3. `SetParent` Taskbar Injection** | Unsupported Hack | Fragile / Broken | Full | High | **Very Low** (breaks across Win11 builds; Explorer crashes) | **Very High** / Very High |
| **T2. DeskBand COM Extension** | Deprecated API | Win10 only / **None** | Full | Medium-High | **N/A on Win11** (breaks portable ZIP) | **Very High** / Unviable |
| **T5. Win11 Widgets Board** | Official (Win11 + MSIX) | None / Win11 only | Widgets Board only | Low (Adaptive Cards) | **N/A** (breaks NSIS/portable packaging) | **High** / Unviable |

---

## 5. Recommended Design & Implementation Strategy

Use **100% official Win32 APIs** with zero shell injection (`SetParent` / DeskBand / taskbar-overlapping hacks). Deliver value in two layers consistent with the app's existing L1 (Glance) → L2 (Preview) → L3 (Dashboard) progressive disclosure model:

### Phase 1 (Primary Recommendation): Dynamic System Tray Icon
Enhance `SystemTray` (and optionally allow dynamic tray icons in all modes since the tray is always active) with a configurable **Tray Icon Style** in Settings → Display:
1. **Dual Gauge (default dynamic style):** Two crisp vertical bars (or split ring) for 5-hour and Weekly remaining quota using active theme/swatch colors, plus explicit visual treatment for stale/blocked/unavailable states. Legible at 16×16 px (100% DPI).
2. **Numeric Remaining %:** Pixel-aligned 2-digit value (`Lowest remaining`, `5-hour`, or `Weekly`) + thin bottom fill strip:
   * `<1%` displays `<1`, missing/unavailable displays `--` (never `00`), `ordinaryUsageAllowed == false` displays `!`, and `stale`/`last_known` renders muted/outlined so stale or missing data is never misread as fresh or zero.
3. **Static Brand Icon:** Preserves current behavior.
4. **Hover & Click unchanged:** Existing ~260 × 76 hover summary (with 127-char-safe tooltip fallback) continues delivering exact percentages and reset countdowns; click opens Overview popup; double-click opens standard window.

### Phase 2 (Optional Complementary Enhancements)
* **`ITaskbarList3` Taskbar Button Telemetry:** Apply `SetProgressValue` / `SetProgressState` and `SetOverlayIcon` to the `main` window whenever `DetailState::Window` is visible; optionally add a **"Taskbar Button"** mode (keep `main` minimized on the taskbar instead of hiding) with dynamic window title (automatic inline text for uncombined taskbars) and `DwmSetIconicThumbnail` + `ThumbBar` actions.
* **Taskbar-Adjacent HUD Dock Preset:** Add a one-click **"Dock above taskbar"** placement preset for `CompactHud` (bottom-right work-area anchor, small gap) for users who want always-visible reset countdowns right beside the taskbar without Explorer fragility.
* **Opt-In Dual Tray Icons:** Optional power-user toggle splitting 5-hour and Weekly into two adjacent numeric tray icons, paired with concise Settings guidance on pinning both icons in Windows 11.

### Technical Implementation Plan
1. **Pure-Rust Dynamic Icon Renderer (`src-tauri/src/tray_icon.rs`):**
   * Render DPI-matched RGBA buffers (`GetSystemMetricsForDpi(SM_CXSMICON, dpi)`) using integer/AA primitives and an embedded crisp bitmap font — **zero WebView2 overhead** and no heavy graphics dependencies.
   * Memoize a compact render key `(style, metric, five_hour_bucket, weekly_bucket, freshness, blocked, system_dark, high_contrast, dpi_px, palette)` so `TrayIcon::set_icon` is called only on visual changes (avoiding GDI handle churn).
   * Update strictly on state/theme/DPI events, never on a 1-second timer.
2. **Taskbar-Accurate Theme Detection ([platform.rs](../src-tauri/src/platform.rs)):**
   * Read `SystemUsesLightTheme` alongside `AppsUseLightTheme` (with `SPI_GETHIGHCONTRAST` precedence) so tray iconography contrasts reliably against the actual taskbar surface.
3. **Explorer Restart & DPI Recovery ([native_windows.rs](../src-tauri/src/native_windows.rs), [windows.rs](../src-tauri/src/windows.rs)):**
   * Register `RegisterWindowMessageW("TaskbarCreated")` in `control_proc` to invalidate cached `self.tray` geometry and re-apply dynamic icon state after Explorer restarts.
   * Re-query tray monitor DPI on `WM_DISPLAYCHANGE` / `WM_DPICHANGED`, and cap `service::tooltip` to 127 UTF-16 code units.
4. **Settings Schema & UI ([model.rs](../src-tauri/src/model.rs), [App.tsx](../src/App.tsx)):**
   * Add validated, backward-compatible settings fields (`trayIconStyle`, `trayIconMetric`, and optional Phase 2 taskbar flags) with defaults preserving existing behavior.
   * Keep Tauri IPC capabilities unchanged; all Win32 icon and COM logic stays inside Rust.
5. **Optional `ITaskbarList3` Adapter (`src-tauri/src/taskbar.rs`):**
   * Reuse `windows-bindings` (`Win32_UI_Shell` and `Win32_System_Com` are already enabled in [Cargo.toml](../src-tauri/Cargo.toml)).
   * Listen for `TaskbarButtonCreated` in `main`'s window subclass ([native_windows.rs](../src-tauri/src/native_windows.rs)) and run COM calls on the owning UI thread via `on_window_thread`.

---

## 6. Planned Validation Checklist (For Future Implementation)

* **Automated Rust unit tests (not yet implemented):** Verify RGBA icon generation at `16/20/24/32 px` across `fresh`, `stale`/`last_known`, `<1%`, `0%`, `100%`, `unavailable`, `ordinary_usage_allowed = false`, `verifying`, Light/Dark taskbar themes, and High Contrast; verify 127-char tooltip truncation and settings schema validation.
* **Manual Windows 10 & Windows 11 runtime checks (required):** Mixed-DPI monitors (`100%`, `125%`, `150%`, `200%`), Light Apps + Dark Taskbar, Explorer restart recovery, Windows 11 tray overflow pinning, auto-hide taskbar behavior, and combined vs. uncombined taskbar labels (if Phase 2 `ITaskbarList3` is implemented).
