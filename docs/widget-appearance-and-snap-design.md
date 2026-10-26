# Widget appearance and consistent snapping — design report

**Date:** 25 October 2026\
**Status:** Design specification. Implementation status, passed checks, and the disabled native-transparency gate are recorded in the [implementation report](widget-appearance-and-snap-implementation-report.md). The acceptance matrix below defines requirements, not a claim that every scenario has passed.\
**Baseline:** Source version 0.1.1, reviewed at `f18449e`.\
**Relationship:** Focused follow-up to the [canonical UI/UX design](codex-widget-design-report.md). This report specifies the next increment without replacing that report or its implementation evidence.

## Current settings contract — v0.1.2

This revision supersedes the eight-role editor, embedded sample previews, and save-only display behavior described in the original proposal below. Geometry, accessibility and native-transparency acceptance requirements remain applicable.

Display offers Follow Windows/Light/Dark and one paired preset (Original, Ocean, Forest, Plum, Slate, Amber). Each preset remembers its own Light and Dark 5-hour/weekly quota choices and opt-in switch. Show only the active effective theme; reveal safe boxed color choices only when custom quota colors are enabled. Load preset colors replaces both choices for that preset/theme while keeping customization enabled. Dashboard structure and semantic/focus colors remain independent of presets. Original uses the v0.1.0 frame/divider, raised surface and wash styling, including when its quota colors are customized.

Remove embedded Settings previews. Theme, preset, quota colors and snap gap temporarily update the actual active widget without saving. Rust owns a display-only preview lease with session ownership, ordered updates and timeout. Save persists the whole draft once; Cancel, leaving/hiding Settings, reload/destruction, a settings conflict or quit retires the preview. Failed saves retain the draft and temporary appearance. Preview must not reveal hidden widgets or change Codex polling/reminders. Freeze an anchored Settings popup during gap dragging and reanchor it on release.

Merge snapping into Placement: position lock, compact gap slider and numeric field, 0–48 logical px, Reset to 8. Keep gap/lock available in tray mode and show monitor/offset/edge placement only for desktop modes. Native transparency remains unavailable until its validation gate passes.

Schema 7 stores selected preset and per-preset/per-theme custom quota choices. Schema-6 defaults become Original; quota-only overrides are retained; unmatched structural customizations retain the legacy appearance. Preserve schema-6 gap/anchors, revisions, connection/storage preferences and reminder ledger. Validate dormant custom colors against their own preset/theme.

## Original detailed proposal

## 1. Recommended design

Add three related controls to **Settings → Display**: background transparency, widget element colors, and a shared snap gap. Preserve the existing compact shapes, typography, period selection, and interaction routes. The visual improvement should come from controlled surface treatment and predictable alignment.

| Request | Proposed control | Default and limits | Applies to |
| --- | --- | --- | --- |
| Adjust widget background transparency | **Background transparency** slider and numeric percentage | **0%**; whole numbers **0–80%**, where 0% is opaque | Floating, Screen Edge, Compact HUD backgrounds |
| Customize widget element colors | **Widget colors**, with color swatches, hex inputs, and separate Light/Dark palettes | **Theme defaults**; validated optional overrides | Shared widget palette, with surface-specific scope in Section 5 |
| Make snap spacing consistent and configurable | **Snap gap** slider and numeric input | **8 logical px**; whole numbers **0–48** | All three desktop modes; one shared value |

The default appearance remains opaque and uses the existing palettes. The new default gap intentionally changes snapped Floating/HUD placement from flush to 8 logical pixels, matching Edge. Users can choose 0 for flush alignment where the taskbar safety rule permits it.

**Meaning of “snap gap”:** the distance from a desktop widget's visible outer boundary to the monitor's usable work-area boundary after snapping. It is distinct from the distance between a widget and its preview or clicked popup. Keep that popup spacing at the existing 8 logical pixels for this increment; changing it would also change hover travel behavior.

The four modes, tray recovery route, quota semantics, read-only Codex integration, and Rust-owned state remain the product foundation. Acrylic/Mica effects, wallpaper sampling, arbitrary CSS themes, custom artwork, and another display mode are outside this increment.

## 2. Current source baseline

The following findings come from code inspection, not a new runtime validation. Earlier test results remain in the [implementation report](implementation-report.md) and [layout-polish report](layout-polish-implementation-report.md).

| Area | Observed implementation | Design implication |
| --- | --- | --- |
| Settings | [App.tsx](../src/App.tsx), `SettingsPage`, offers Follow Windows/Light/Dark, placement, and desktop behavior; changes use a draft and explicit Save/Cancel | Extend the Display page and its existing revision/conflict handling |
| Color tokens | [styles.css](../src/styles.css) defines fixed Light/Dark semantic tokens; `--accent` also supplies quota, controls, and focus | Introduce explicit widget and quota roles before accepting custom colors |
| Backgrounds | Shells use opaque solid/gradient tokens; Floating also has opaque nested quota cards | Changing only an outer CSS background will leave most of Floating opaque |
| Accessibility state | [platform.rs](../src-tauri/src/platform.rs), `appearance`, reads high contrast, transparency preference, motion, and text scale; [state.ts](../src/state.ts), `applyAppearance`, does not apply a transparency setting | Add requested/effective appearance resolution; the existing OS flag alone does not implement translucency |
| Native surfaces | [windows.rs](../src-tauri/src/windows.rs), `ensure`, does not request a transparent webview window; [native_windows.rs](../src-tauri/src/native_windows.rs), `round_corners`, clips native paint/hit bounds | Native composition and rounded-region behavior need validation together |
| Floating/HUD snapping | `windows.rs`, `current_placement`, uses a 12-logical-pixel attraction distance and snaps to **0 gap** | Replace the flush coordinates with shared gap-aware geometry |
| Edge docking | [geometry.rs](../src-tauri/src/geometry.rs), `dock_edge`, and `windows.rs`, `synchronize`, each contain an **8-logical-pixel inset** | Consolidate release and restore paths so the gap cannot diverge |
| Persistence | [model.rs](../src-tauri/src/model.rs) uses settings schema **5**; [storage.rs](../src-tauri/src/storage.rs) writes envelope format **4** with validated migration and atomic storage | Version settings explicitly and preserve the existing storage/ledger transaction |
| Validation | [contrast tests](../tests/contrast.test.mjs) cover fixed opaque palettes; geometry/native checks cover existing placement | Extend behavioral coverage to custom palettes, composition, and gap changes |

These observations establish the missing controls. They do not establish that native translucency works on the supported Windows/WebView2 matrix.

## 3. Settings layout and editing behavior

Keep the four existing Settings pages. Within Display, group the new controls under **Widget appearance** and **Placement & snapping**. Keep the shared appearance/gap controls available in System Tray mode so users can prepare their desktop widgets before switching modes.

```text
Display
  Theme                 [Follow Windows] [Light] [Dark]
  Display mode          [Floating] [Screen Edge] [System Tray] [Compact HUD]

  Widget appearance
    Background transparency   Opaque ----o---- More transparent   [20] %
    Applies to Floating, Screen Edge, and Compact HUD.
    Text and essential indicators stay readable.

    Widget colors             [Theme defaults] [Custom]
    Edit palette              [Light] [Dark]
    5-hour quota              [swatch] [#005FCC] [Reset]
    Weekly quota              [swatch] [#6D28D9] [Reset]
    More element colors       [expand]

    Preview                   [Floating] [Screen Edge] [Compact HUD]
    Sample background         [Light] [Dark] [Pattern]
    Example data · Unsaved preview
    [Reset widget appearance]

  Placement & snapping
    Snap gap                  [8] logical px    [0 ----o---- 48]
    One gap for Floating, Screen Edge, and Compact HUD.
    [small work-area diagram with the selected gap]
    Monitor / position / Edge side / Lock position

  [Cancel]                                      [Save settings]
```

The illustrated 20% transparency is an editing example, not the default. Use real rendering components inside a bounded preview, with fixed synthetic values clearly labeled **Example data**. Do not read desktop pixels or start a Codex request for preview data. At narrow widths or increased text size, stack controls and provide a preview viewport without shrinking actual widget text.

### Draft and save contract

1. Sliders, color picking, palette switching, and gap editing immediately update the **Settings preview only**. Editing the Light/Dark palette tab does not change the saved theme preference or the live Windows theme.
2. Existing desktop widgets continue to render the saved state until **Save settings** succeeds. Avoid temporary movement of the live widget while its settings popup is anchored to it.
3. Save validates the entire draft in Rust, persists one settings revision, then broadcasts it. The active shell updates without restart; inactive modes use the same preferences when next opened. Reanchor any open popup after a saved placement change without changing its page or stealing focus.
4. A storage error leaves the previous saved appearance and placement active. Retain the draft and identify the failed operation. If persistence succeeds but native application fails, show **Settings saved; display update failed** and retain a usable opaque/recovery surface rather than claiming full success.
5. **Cancel** restores the latest saved settings in the editor and preview. Preserve the existing Save/Discard/Cancel navigation and revision-conflict flows. An incoming revision must not silently overwrite a dirty draft.
6. Per-color **Reset** removes that theme's override. **Reset widget appearance** resets both custom palettes and transparency in the draft, but preserves theme choice, mode, gap, and placement. Gap has its own **Reset to 8 px** action. Neither reset writes until Save.

Give every slider a labeled numeric alternative, unit, keyboard support, and inline error. Do not coerce an empty input to zero while typing. Reject out-of-range, fractional, non-finite, or malformed saved values with a field-specific message. Announce final slider values and validation results without flooding screen readers during pointer movement.

## 4. Background transparency

### Visual treatment

Store the setting as transparency so the displayed percentage has an unambiguous direction:

```text
requested background alpha = 1 - backgroundTransparencyPercent / 100
0% transparency  -> alpha 1.00 (opaque)
20% transparency -> alpha 0.80
80% transparency -> alpha 0.20
```

Use one setting for all three shells. Apply it to the background paint layers, including their gradient stops. For Floating, include the nested quota/credit surface treatment; otherwise the control would affect only the card's narrow outer padding. Preserve visual hierarchy through background tints and borders rather than accumulating several overlapping alpha layers that obscure the selected effect.

Text, percentages, icons, progress fills, reset segments, focus outlines, and required boundaries retain full foreground opacity. Do not apply CSS `opacity` to the shell/root or use whole-window alpha, which would also fade those elements. Do not alter hit targets, compact dimensions, or drag/click behavior as transparency changes.

The 80% upper limit retains some visible surface and avoids a completely invisible interactive rectangle. It is a proposed product limit, not a Windows limit. Hover previews, tray summaries, the dashboard, and Settings remain opaque reading/control surfaces. No hover-to-opaque or idle-fade animation is introduced.

### Readability and effective appearance

Opaque text can still become unreadable over arbitrary desktop content. Put readable backing surfaces behind text/control/indicator groups when the requested transparency would fail the canonical contrast targets: **4.5:1 for normal text** and **3:1 for essential graphics, controls, and focus**.

Resolve each backing surface to at least the opacity needed by its foreground roles. Evaluate the possible composited-background luminance range, including black, white, and intermediate backgrounds; checking only two endpoint ratios is insufficient when a foreground luminance lies between them. An opaque backing is the safe fallback. This computation needs no wallpaper capture. Keep the user's chosen transparency on decorative background areas and disclose **Content backing is more opaque for readability** in the preview when protection is active.

Appearance precedence is:

| Condition | Effective rendering | Saved preference |
| --- | --- | --- |
| Windows high contrast | System colors, opaque surfaces, no decorative gradients/shadows | Retained |
| Windows transparency disabled or its state unavailable | Opaque surfaces; validated custom colors can remain active | Retained |
| Native composition unavailable or failed | Opaque fallback with a Display-page explanation | Retained |
| Normal operation | Requested background transparency plus necessary readable backing | Retained |

Explain an override beside the control, for example **Currently opaque because Windows transparency is off**. Continue to allow editing the saved preference and a clearly labeled requested-style preview; also show the effective-style preview. Leaving an override restores the saved preference through shared state resolution. Reduced motion suppresses optional visual transitions; opacity changes must not cause resizing or recurring animation.

### Native implementation boundary

Tauri exposes transparent window creation, and its builder documentation describes CSS alpha rendering for that window. This is an available mechanism, not proof of compatibility with this app's existing native window styles and clipping. [Tauri WebviewWindowBuilder documentation](https://docs.rs/tauri/latest/x86_64-pc-windows-msvc/tauri/webview/struct.WebviewWindowBuilder.html#method.transparent).

WebView2's `DefaultBackgroundColor` accepts fully transparent or opaque alpha, not intermediate alpha values. Therefore the proposed approach is a transparent native/webview base with CSS controlling the surface alpha; do not send the slider value directly as the WebView2 default-background alpha. [Microsoft WebView2 controller reference](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller2?view=webview2-1.0.3537.50#put_defaultbackgroundcolor).

Prototype transparent creation for the reusable **shell** only, retaining opaque preview/main windows. Hydrate its effective appearance before showing it. Validate startup flashes, recreation, clipping, pointer input, and high-contrast transitions before enabling the control in a release. This proposal does not require blur libraries, new broad IPC permissions, global environment changes, or dependency upgrades. If the existing stack cannot provide reliable background-only transparency, keep the control visibly unavailable with the reason and record the limitation; an ineffective slider is not completion.

## 5. Custom widget colors

### Color roles and scope

Offer **Theme defaults** and **Custom**, with independent Light/Dark override maps. Initially expose the two quota colors; place secondary roles in **More element colors**. A swatch opens a picker and an adjacent input accepts `#RRGGBB`; normalize case on successful validation. Alpha belongs exclusively to the transparency setting.

| User-facing role | Visual elements | Scope |
| --- | --- | --- |
| Widget background | Shell base and derived gradient tints | Floating, Edge, HUD |
| Content surface | Quota/credit panels and readable backing | Desktop shells and app-rendered hover previews |
| Main text | Period labels and ordinary primary text | Desktop shells and app-rendered hover previews |
| Secondary text | Countdown metadata, neutral badges, freshness labels | Desktop shells and app-rendered hover previews |
| 5-hour quota | Corresponding rings, bars, known colored percentages, reset segments | All app-rendered quota views, including Overview |
| Weekly quota | Corresponding rings, bars, known colored percentages, reset segments | All app-rendered quota views, including Overview |
| Track | Unfilled progress/ring/reset tracks | Desktop shells and app-rendered hover previews |
| Border and divider | Widget outline, card boundaries, separators | Desktop shells and app-rendered hover previews |

“App-rendered hover previews” includes the compact tray summary where the role exists; it stays opaque, read-only, and approximately 260 × 76 logical pixels at default text size. Dashboard and Settings structural colors retain their theme defaults, providing a predictable editing/recovery surface. Their quota graphics use the saved period colors, so a period keeps its identity throughout the application.

Keep warning, critical/error, unavailable/stale styling, and keyboard focus under semantic/accessibility control. Brand artwork and Windows-owned tray backgrounds, menus, tooltips, and notifications retain their existing treatment. Color selection does not change quota values, remaining-versus-used meaning, reset semantics, or reminder thresholds.

### Palette resolution and validation

- Start from the selected theme's existing tokens, then apply only its saved overrides. Missing overrides mean **Use theme default**. Switching to Theme defaults retains the custom maps for later reuse; Reset removes them.
- Follow Windows selects the Light or Dark map automatically. Show both palette tabs even for an explicit theme, and validate both before Save so a later theme switch cannot reveal an unreadable palette. High contrast ignores custom maps without deleting them.
- Decouple period color tokens from `--accent`: changing 5-hour blue must not silently change Settings buttons or the focus ring. Prefer explicit `--quota-five-hour`, `--quota-weekly`, and scoped `--widget-*` tokens, resolved through the shared appearance path.
- Validate actual role pairs across their scopes, including quota fills against the dashboard's default tracks, colored percentage text, nested surfaces, hover/selected states, and protected backing. Syntax checks alone are insufficient.
- If a color fails even with opaque backing, show the failing pair and ratio beside the field and block Save. Offer **Use theme default** as a direct fix; do not silently replace a chosen color. A contrast warning about the inactive palette must link to that palette.
- Period labels/badges and geometry continue to identify each period if the user chooses similar colors. Unknown values remain neutral and visibly unavailable; custom fills cannot turn missing data into a healthy-looking value.

The preview should offer fresh, stale, unavailable, and warning examples. Validate the same palette in both Words/Badges and single/both-period configurations. Do not add decorative glow, moving gradients, or new image assets to achieve color customization.

## 6. One snap gap for Floating, HUD, and Edge

### Geometry contract

Use **logical pixels**, independent of Windows text scaling. Convert once using the destination monitor's DPI scale. At 200% DPI, an 8-logical-pixel gap is 16 physical pixels. Allow at most one physical pixel of rounding error.

Measure from the shell's visible rectangular bounding box, excluding shadows and any invisible native frame. Rounded corners do not redefine the gap. Reuse native client/frame measurements so a hidden margin cannot make two modes look differently spaced. Work-area boundaries account for a reserved taskbar; this setting is not spacing from the full-screen rectangle or from a neighboring application window.

For work area `(L, T, R, B)`, outer visible widget size `(w, h)`, and effective physical margins `(gL, gT, gR, gB)`, use:

```text
left anchor:   x = L + gL
right anchor:  x = R - gR - w
top anchor:    y = T + gT
bottom anchor: y = B - gB - h
```

Normally every margin is the same configured gap multiplied by monitor scale. A corner snap combines one horizontal and one vertical anchor.

### Mode and input behavior

| Behavior | Floating / Compact HUD | Screen Edge |
| --- | --- | --- |
| During drag | Follow the pointer freely | Follow the pointer freely |
| On release | Snap eligible axes to the nearest work-area side; otherwise retain free placement | Always dock left/right based on final center; vertical near-edge snapping uses the same gap |
| Monitor choice | Monitor under the release pointer; nearest available monitor outside displays | Same policy, preserving existing Edge behavior |
| Side tie | Retain prior anchor when applicable; otherwise choose left/top deterministically | Retain previous left/right side |
| Position lock | Blocks drag and keyboard nudges | Same |
| Explicit saved gap change | Reposition anchored axes; preserve free axes | Reposition the docked side and any vertical anchor |

Keep **12 logical pixels** as the fixed snap-attraction distance; it is not a second user setting. On each axis, a dropped widget edge is eligible when it lies within the configured inset band or within 12 logical pixels inward of that band's snap line. Thus dropping flush still snaps to the selected gap, including a 48-pixel gap. Larger gaps intentionally enlarge that outer attraction band. If both opposing sides qualify on a small work area, choose the nearest candidate, then the tie rule above.

Snap once on drag release; do not add magnetic motion, a screen-wide overlay, or an animation during drag. Preserve the existing drag threshold and suppression of click activation after a drag. Persist the monitor, coordinates, anchor metadata, and Edge side together. A failed placement save restores the previous saved placement.

Keyboard nudges and explicit position edits allow fine positioning without being repeatedly pulled back into the attraction band. For Floating/HUD, changing an axis clears its anchor and clamps to the work area, including the taskbar safety floor below but without imposing the configured snap gap on free axes. Edge retains its horizontal dock; vertical edits clear its vertical anchor and respect its effective vertical margins. Gap changes made through Settings apply even when position lock is on: the lock prevents direct movement, not an explicit saved placement preference.

### Anchors, resizing, and recovery

Persist per-mode horizontal/vertical anchor intent separately from free logical offsets. Coordinate equality alone is too ambiguous to preserve a right/bottom gap after content-size, countdown, text-scale, or DPI changes. Edge's horizontal anchor derives from its saved side; do not create two conflicting sources of truth.

- On Save, update every saved mode's anchored placement policy; physically move only the active shell. Free Floating/HUD placements remain unchanged unless work-area clamping is necessary.
- On mode switch, restart, monitor removal, DPI/work-area change, or content resizing, resolve saved anchors against the current visible shell size. Reposition dependent previews/popups through the existing coordinator.
- For an auto-hide taskbar activation edge, use `max(configured gap, 8 logical px)` on that edge in **all three modes**. This is an explicit safety exception. If taskbar-edge discovery is uncertain, conservatively apply the 8-pixel floor to all work-area sides and report the effective constraint in Settings.
- If opposing margins would leave insufficient room, reduce the effective margins on that axis before shrinking content; keep the requested gap saved. If content itself exceeds the work area, use accessible reflow/scroll or dashboard/tray recovery and report that exact gap placement is unavailable. Do not silently clip controls to satisfy the margin.
- Extend **Reset position** to use the shared policy: Floating/HUD reset to top-left anchors on the primary monitor; Edge keeps its selected side and resets vertically to the top margin. The resulting placement uses the current gap, including taskbar safety. This action remains an explicit recovery command even when locked.

The settings preview labels the configured gap and any effective exception. No spacing change may add a taskbar button, steal foreground focus, open a preview, or change always-on-top policy.

### Migration

At upgrade, add transparency 0%, Theme defaults, empty color maps, and gap 8. Preserve the user's theme, mode, period/countdown choices, monitor references, coordinates, proxy/storage configuration, and reminder ledger.

Legacy Floating/HUD placements have no anchor intent. At their first geometry resolution, infer an anchor only for a coordinate flush to the previous work-area boundary within one logical pixel; otherwise keep it free. If the old monitor/size is unavailable, do not guess: clamp as a free placement. Edge retains its explicit side; infer top/bottom anchors only at its previous 8-pixel inset. Persist inferred metadata in the next normal placement/settings transaction, not during repeated rendering.

The first display of an inferred flush Floating/HUD anchor moves it to the new 8-pixel gap. Record this deliberate migration behavior in release notes, including the option to choose 0. Subsequent updates use stored anchor intent rather than re-inferring it.

## 7. State and implementation plan

The following is a proposed contract sketch, not an existing API. Reserve the next settings schema version during implementation; with today's schema 5, that would be **6**. The envelope format need not change solely because settings gain fields.

```ts
type ColorRole =
  | "background" | "contentSurface" | "text" | "secondaryText"
  | "fiveHour" | "weekly" | "track" | "border";

interface WidgetAppearanceSettings {
  backgroundTransparencyPercent: number; // integer 0..80; default 0
  colorMode: "theme-defaults" | "custom";
  palettes: {
    light: Partial<Record<ColorRole, string>>; // normalized #RRGGBB
    dark: Partial<Record<ColorRole, string>>;
  };
}

// Add to Settings:
// widgetAppearance: WidgetAppearanceSettings
// windows.snapGapLogicalPx: number                 // integer 0..48; default 8
// windows.snapAnchors: per-mode { horizontal, vertical }
//   Floating/HUD horizontal: "none" | "left" | "right"
//   All desktop modes vertical: "none" | "top" | "bottom"
//   Edge horizontal: derived from windows.edgeSide
```

Rust validates these fields, derives the effective palette/opacity and placement constraints, and owns persistence and native coordination. Publish requested/effective differences and a bounded reason code in shared state. React renders that state; the Settings-only draft preview can use a matching pure resolver. Share fixtures between TypeScript and Rust to catch differences in normalization, contrast, rounding, and fallback behavior.

Use the existing `save_settings` revision contract. Extend strict allowlists for roles and mode keys; reject arbitrary CSS, URLs, oversized maps, and future settings versions. Keep full settings validation on the native side, including when a color picker or browser validation is bypassed. No separate appearance store, quota client, timer, or scheduler is needed. Appearance-only saves must not restart Codex or trigger usage reads.

| Stage | Work | Exit evidence |
| --- | --- | --- |
| 1. Native feasibility | Prototype background-only transparency with existing shell creation, rounded regions, and appearance observation | Real Windows evidence for composition, readable content, no startup flash, and unchanged hit/focus behavior; document fallback |
| 2. Shared contracts | Add validated settings, migration, effective appearance, shared geometry, and anchor intent | Behavioral tests for defaults, invalid inputs, revision conflicts, migration, and atomic persistence |
| 3. Settings and rendering | Add controls/preview/reset flows; split quota/widget tokens; use resolved paint layers | Production frontend build plus preview checks across themes, modes, text scale, and data states |
| 4. Native integration | Use one gap policy in drag-end, synchronize/restore, keyboard placement, reset, and resize paths | Native placement checks at multiple DPI scales; failure/rollback checks |
| 5. Release verification | Run the matrix below, update implementation evidence and user-facing documentation | Record tested platforms and remaining limitations; expose only functional controls |

Primary code touchpoints: `src/App.tsx`, `src/state.ts`, `src/SurfaceView.tsx`, `src/styles.css`, shared quota components, and Rust `model.rs`, `storage.rs`, `service.rs`, `geometry.rs`, `windows.rs`, `native_windows.rs`, and `platform.rs`. Extract focused appearance/settings helpers where needed instead of embedding all policy in `App.tsx` or CSS selectors.

## 8. Acceptance and validation plan

Every item below is **planned, not passed**. Builds and browser fixtures alone cannot validate native desktop transparency, taskbar placement, focus, or input.

| ID | Scenario | Required result |
| --- | --- | --- |
| VIS-01 | Set transparency to 0, 20, and 80 in every desktop mode | Visible background response; foregrounds remain opaque; protected backing stays readable; geometry unchanged |
| VIS-02 | Place shell over light, dark, patterned, and moving content | No disappearing labels/focus or misleading quota graphics; backing meets contrast policy |
| VIS-03 | High contrast, transparency disabled, and native fallback; then restore normal state | Correct effective appearance and reason; saved preference survives; all new/recreated shells hydrate before showing |
| VIS-04 | Customize each role in both themes; switch Follow Windows | Correct role/surface scope; palettes retained; no accidental focus/button recoloring |
| VIS-05 | Invalid hex, out-of-range numbers, failing contrast, malformed IPC, future schema | Field-specific rejection; no partial save or corrupted configuration |
| VIS-06 | Edit, Cancel, Reset, Save, navigate away, and receive a competing revision | Preview-only drafts, existing conflict handling, and truthful success/failure state |
| SNAP-01 | Gaps 0, 8, 16, 48; all modes and supported sides/corners | Same configured logical gap within one physical pixel, except disclosed safety/fit constraints |
| SNAP-02 | Drop inside/outside the attraction band, ties, and cancelled drags | Deterministic release behavior; no accidental click, unwanted persistence, or repeated attraction during fine nudging |
| SNAP-03 | Cross monitors at 100%, 125%, 150%, 200%, and 250% DPI, including negative origins | Destination-monitor conversion once; no double scaling, drift, or lost widget |
| SNAP-04 | Change gap while anchored/free/locked; switch modes; resize periods/countdowns/text | Anchored axes follow the gap; free axes stay free; Edge keeps its side; popup follows without focus loss |
| SNAP-05 | Reserved/auto-hide taskbars on relevant edges, missing monitor, tiny work area | Correct work-area use and safety floor; usable recovery; constrained gap explained |
| SAVE-01 | Upgrade schema 5, restart, recreate shell, restore backup, and simulate write failure | Defaults and anchors migrate as specified; preferences/ledger retained; failed writes restore previous live state |
| REG-01 | Click/double-click/drag/right-click, hover corridor, tray recovery, always-on-top, fullscreen, sleep/resume, Explorer restart | Existing interaction policies preserved; no new polling client, scheduler, or taskbar surface |

Automated work should extend the existing Node built-in tests and Rust/native checks. Test resolvers and geometry with expected outcomes, including right/bottom anchors after size changes, safe-area exhaustion, unknown system appearance, and palette pairs that fail only in the other theme. Do not rely on CSS string checks as proof that custom colors or transparency are accessible.

After implementation, run from the repository root:

```powershell
npm.cmd test
npm.cmd run build
npm.cmd run rust:fmt
npm.cmd run rust:check
npm.cmd run rust:test
npm.cmd run rust:clippy
npm.cmd run desktop:build
```

Extend the isolated `--check-native-sizing` scenarios for gap/anchor assertions and composition checks. Use synthetic presentation fixtures for screenshots and contrast inspection. Exercise real pointer/focus behavior on Windows 10 and 11 and record actual OS/WebView2 versions; unsupported or unavailable combinations remain explicitly unverified. Stop any validation processes started for this work. A live Codex protocol probe is unnecessary for appearance and geometry changes.

## 9. Report review and remaining decisions

The proposed defaults, scope, interaction contract, and safety exceptions above are ready for implementation planning. Native background composition remains the principal feasibility gate; numerical gap geometry and palette validation can proceed independently.

For this documentation change, source files and existing evidence were inspected, and the cited official Tauri/WebView2 references were checked. No application code, dependencies, settings, credentials, or running desktop behavior were changed. No application build, tests, screenshots, or Windows runtime checks were performed.

Documentation validation on 25 October 2026:

- `git diff --check` — passed for the tracked documentation edits.
- `node --input-type=module` with an inline Markdown checker — passed: 29 local links resolve across this report and `docs/README.md`; code fences are balanced and whitespace is valid in both files; the canonical report contains the new cross-reference. The check includes the newly created report.
- Source/design consistency review — completed; new behaviors are labeled proposed, and native composition, input, accessibility, and mixed-DPI outcomes remain unverified.
