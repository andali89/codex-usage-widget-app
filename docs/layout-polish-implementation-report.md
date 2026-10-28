# Layout polish implementation and evidence

Updated: 2026-10-28. Reference: the [canonical design report](codex-widget-design-report.md) and its supplied private reference board (not distributed with this repository).

## HUD warning gap B (+8 px) — 2026-10-28

Applied the user's selected **B — Current spacing +8 logical px**. HUD warnings have an 8 px left margin; the warning-only width allowance increases from 20 to **28 px** to preserve the existing quota columns, countdowns, divider and right padding. Layouts without warnings keep their previous dimensions. Height stays **38 px**; both periods with both countdowns and a warning now use **348 px Words / 312 px Badges**. The native sizing check and existing frontend expectations match the new allowance. Edge, Floating, tray, preferences, IPC and dependencies are unchanged.

The [comparison page](hud-warning-spacing-comparison.html), [comparison capture](assets/hud-warning-preview/candidates-words-dark.jpg), and its source generator in `assets/hud-warning-preview/` preserve the selection record. The generator explicitly retains the original 20 px warning allowance for that historical comparison. [Implemented B](assets/hud-warning-preview/implemented-b.jpg) shows synthetic long retained readings at 200% text scale.

Validation: `npm.cmd run build` passed, including TypeScript and production bundling. `node --test tests/compact-layout.test.mjs tests/periods.test.mjs` passed **4 tests**. `npm.cmd run rust:fmt` and `npm.cmd run rust:check` passed; compilation emitted only the existing incremental-cache hard-link/copy warnings. Browser checks using production components and bundled CSS passed **256 presentations** across label styles, all effective period/countdown combinations, Light/Dark, 100/200% text scale and four reading scenarios. Widths, 38 px height, 8 px warning margin and overflow checks passed; warning clearance from painted quota content including outlines was at least **1.8 logical px**. The two-period long-countdown profile matched all measured positions and sizes of the selected B preview (11 elements Words / 13 Badges), within 0.05 px. Fixtures are under `src-tauri/target/validation/hud-warning-validation.html` and `generate-hud-warning-validation.mjs`.

The standard Cargo release build could not replace the existing executable (`Access is denied`). `cargo rustc --manifest-path src-tauri/Cargo.toml --release --locked --bin codex-usage-widget --features tauri/custom-protocol -- -C extra-filename=-hud-warning` passed, and the output was copied from `release/deps/codex_usage_widget-hud-warning.exe` to **`src-tauri/target/release/codex-usage-widget-hud-warning.exe`** (13,681,664 bytes). No user process was stopped; the standard-named executable was not replaced. `git diff --check`, documentation links and preview-script syntax passed. The temporary localhost server was stopped. No full test suites, installer build/installation, executable launch, or native pointer/focus/DPI checks were run in this increment.

## Edge warning icon overlap correction — 2026-10-28

Reproduced the warning triangle overlapping the lower reset strip in the Balanced Edge layout. At 200% text scale, the warning box overlapped the last strip by 2 px Words / 2.5 px Badges in logical units; the retained-data outline extended the visible conflict to 5 / 5.5 px. Earlier layout validation checked each element's containment within the shell, but did not check the warning against the painted bounds of neighboring quota content.

The Edge-only warning now sits **10 logical px lower** inside the existing bottom padding. The quota stack, rings, 42 px segmented strips, 54 px shell width, fixed heights and 18 px warning allowance are unchanged. HUD warnings are unaffected. [Before](assets/edge-sizing-preview/warning-before.jpg) and [after](assets/edge-sizing-preview/warning-after.jpg) show synthetic retained data at 200% text scale.

Validation: `npm.cmd run build` passed, including TypeScript and final frontend bundling. Browser checks passed **256 presentations**, including **128 warning presentations**, across Words/Badges, all effective visible-period/countdown combinations, Light/Dark, 100/200% text, and fresh/retained-long/unavailable/fractional-due readings. The warning cleared the painted quota bounds including dashed outlines by at least **3.4 logical px**, and the outer bottom border by at least **6.5 logical px**. Expected dimensions, overflow and countdown clipping checks also passed. Checks used the production component/CSS validation fixture under `src-tauri/target/validation/`; the local server was stopped afterward.

The standard `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol` could not replace the existing executable (`Access is denied`, including a normal-filesystem-access retry). No user process was stopped. The established alternate build, `cargo rustc --manifest-path src-tauri/Cargo.toml --release --locked --bin codex-usage-widget --features tauri/custom-protocol -- -C extra-filename=-edge-warning`, passed. Its binary was copied from `release/deps/codex_usage_widget-edge-warning.exe` to **`src-tauri/target/release/codex-usage-widget-edge-warning.exe`** (13,681,664 bytes). The standard-named executable was not updated. `git diff --check` and documentation links passed. No unit/Rust test suites, installer build, native pointer/focus/DPI check, or installation was run for this CSS-only correction; the alternate executable was not launched.

## Edge segmented indicator width — 2026-10-28

Latest follow-up: changed the Edge reset strip from **40 px to 42 logical px** (`2.5rem` → `2.625rem`) at default text size, after the earlier 38 → 40 px increment. It remains centered; the ten segments, segment gap, ring/text sizes, spacing and outer widget dimensions are unchanged. The canonical design specification now records the 42 px width. Historical previews and captures retain their prior width.

Validation of the 42 px follow-up: `npm.cmd run build` passed, including TypeScript and production bundling. `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol` passed using the established Tauri CLI fallback and rebuilt `src-tauri/target/release/codex-usage-widget.exe`. `git diff --check` passed. Per user request, no test suites, browser validation or native runtime checks were run. The executable was not launched or installed; no installer was rebuilt. The earlier 40 px increment used the same successful build commands and also skipped tests as requested.

## Confirmed Balanced Edge implementation — 2026-10-28

Applied the user's confirmed Balanced profile with revised top spacing and the additional 4 px bottom gap. Edge rings are **48 px**, percentage text **12.5 px**, and countdown text **11.5 px**. The quota stack sits 2.5 logical px lower; horizontal padding is 2 px and bottom padding is 16 px. Width remains **54 px**. Every Edge layout gains 4 px height: both-period bases are 190 px Words / 150 px Badges, single-period bases 120 / 100 px, plus the existing 24 px per visible countdown and 18 px per warning. Both periods with both countdowns therefore use **54 × 238 px Words / 54 × 198 px Badges**. Sizes scale with Windows text settings. HUD, Floating, tray, saved preferences, IPC and dependencies are unchanged.

Production CSS and `compactLayout` implement the profile; existing frontend layout expectations and the native sizing check's Edge constants match the new dimensions. The canonical design specification is aligned. Historical preview files below remain selection records and were not regenerated after the production change.

Validation:

- `npm.cmd run build`: passed, including TypeScript and production bundling. `npm.cmd test`: **143 passed**.
- `npm.cmd run rust:fmt` and `npm.cmd run rust:check`: passed. The latter emitted only incremental-cache hard-link warnings and used file copies.
- `npm.cmd run desktop:build`: blocked by the existing Tauri CLI native-binding load error. The established fallback, `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol`, passed and rebuilt `src-tauri/target/release/codex-usage-widget.exe` (13,681,664 bytes) with the final frontend. No tooling was reinstalled and no installer was built.
- Browser layout checks using production `WidgetContents`, `compactLayout` and bundled CSS passed **256 presentations**: 16 effective label/period/countdown profiles × Light/Dark × 100/200% text scale × fresh/retained-long/unavailable/fractional-due readings. Dimensions, ring diameter, brand separation, root overflow, countdown clipping, and ring/reset/warning containment passed. The confirmed two-period layout matched all measured positions and sizes of 11 elements per label style in the accepted preview, within 0.05 px. [Production rendering](assets/edge-sizing-preview/implemented-edge.jpg) shows both label styles at 200% text scale with synthetic data.
- `git diff --check`, documentation links and snapshot inline-script syntax passed. Validation HTML and its generator live under `src-tauri/target/validation/edge-balanced-validation.html` and `generate-edge-balanced-validation.mjs`; the temporary loopback server was stopped.

Windows native pointer/focus, docking, mixed DPI, OS high contrast, and notification behavior were not exercised in this increment; browser text scaling is not native DPI validation. Rust behavioral tests and native stress checks were not run because native runtime logic did not change; only native validation expectations changed. The rebuilt executable was not launched or installed.

## Edge ring and text sizing selection record — 2026-10-28

**Historical previews; the final bottom-gap version is now implemented above.** The preview phase changed no application source, saved settings, or native executable. The [standalone comparison](edge-sizing-comparison.html) renders the then-current `CompactQuotaReadings`, branding, Original appearance, and compiled CSS with candidate-only overrides. It opens locally without a server or external resources. Controls switch Light/Dark, 1×/2× magnification, and four synthetic reading scenarios; Words and Badges appear together.

Latest refinement: the comparison now shows A as a spacing reference, original B, and revised B. Revised B translates the quota stack down **2.5 logical px**, restoring the small gap below the logo without changing B's 48 px rings, 12.5 px percentages, 11.5 px countdowns, or outer dimensions. The measured logo-to-first-ring gap is 2.35 px for Badges / 22.35 px including the label for Words, within 0.06 px of A. Existing internal ring-to-countdown gaps remain intact. The [revised Dark](assets/edge-sizing-preview/balanced-spacing-dark.jpg) and [revised Light](assets/edge-sizing-preview/balanced-spacing-light.jpg) captures show the comparison at 2×.

Refinement validation: generation passed with `node docs/assets/edge-sizing-preview/generate.mjs`. Browser checks passed all **32 revised-B presentations** (Words/Badges × Light/Dark × 1×/2× magnification × four reading scenarios), checking 54 px width, 234/194 px height, 48 px ring diameter, root overflow, countdown clipping, brand overlap, and ring/reset containment. No application builds or native runtime checks were run; the temporary loopback preview server was stopped. `node --check docs/assets/edge-sizing-preview/generate.mjs`, generated inline-script syntax, local documentation links, and `git diff --check` also passed.

Bottom spacing follow-up: a separate [bottom-gap preview](edge-bottom-gap-preview.html), derived from the revised-B standalone HTML, adds **4 logical px** between the last segmented indicator and the outer bottom border. Its preview-only bottom padding and shell height both increase by 4 px, so the complete content retains its prior position. Width stays 54 px; Words height is 238 px and Badges height is 198 px. Existing application files and the preview generator were not edited. [Dark](assets/edge-sizing-preview/bottom-gap-dark.jpg) and [Light](assets/edge-sizing-preview/bottom-gap-light.jpg) captures compare the prior and proposed spacing at 2×.

Bottom-gap validation: all **32 before/after comparisons** passed across both label styles, themes, magnifications and four reading scenarios. Each showed exactly +4 px height and bottom gap, unchanged content positions and sizes, no root overflow, and no clipped countdowns. Generated inline-script syntax, local documentation links, and `git diff --check` passed; `git diff -- src src-tauri` remained empty. No application builds or native checks were run. The temporary preview server was stopped.

Initial sizing candidates (retained here as the selection record):

| Candidate | Ring diameter | Percentage font | Countdown font |
| --- | ---: | ---: | ---: |
| Current | 44 px | 10.4 px | 10 px |
| A — Subtle | 46 px | 11.5 px | 11 px |
| B — Balanced | 48 px | 12.5 px | 11.5 px |
| C — Largest | 50 px | 13.5 px | 12 px |

All sizes are logical pixels at default text scale. Each preview retains the current **54 × 234 px Words / 54 × 194 px Badges** rectangle with both periods/countdowns enabled. Candidates reduce horizontal padding, center reset indicators in the available width, and use 1.1 countdown line height with −.02em letter spacing to fit `~23h59m`. C additionally uses 8 px bottom padding and 2/4 px internal gaps. Warning icons and their existing height allowance are omitted from these size comparisons; other period/countdown configurations and Windows runtime behavior remain unverified for the candidates.

Generation: `node docs/assets/edge-sizing-preview/generate.mjs` passed using installed Vite/React; it compiles the production CSS and renders actual components to static markup. No dependencies were added. Browser validation passed **128 widget presentations**: Current/A/B/C × Words/Badges × Light/Dark × 1×/2× preview magnification × typical/long-retained/fractional-due/unavailable readings. All retained the expected outer dimensions, with no horizontal/vertical root overflow, clipped countdown, brand/content overlap, or ring/reset bounds outside the shell. Preview magnification is not a Windows DPI/text-scaling test. Final brand-image loading was checked after deduplicating the embedded image. The [Dark](assets/edge-sizing-preview/candidates-dark.jpg) and [Light](assets/edge-sizing-preview/candidates-light.jpg) captures show typical values at 2×. A temporary loopback server was used for browser validation and stopped afterward. Application builds, Rust checks, and native runtime checks were not needed or run for this preview-only increment.

## HUD additional right-side room — 2026-10-27

Applied the selected **+4 logical px** preview to the HUD. Right padding increases from 6 to 10 logical px at default text size; every HUD client width increases by 4 logical px so the existing content area, label/quota columns, countdowns, and segmented indicators retain their positions. The extra space scales with text size. Both-period base widths are now 204 px (Words) / 168 px (Badges); single-period bases are 124 / 106 px. Countdown and warning increments stay 58 / 20 px, and height stays 38 px. Screen Edge, Floating, previews, snap gap, border preference, and saved settings are unchanged. Existing layout expectations were updated to the new width contract.

Validation: `npm.cmd run build` (including TypeScript), `npm.cmd run rust:fmt`, `cargo build --manifest-path src-tauri/Cargo.toml --locked --release --features tauri/custom-protocol`, and `git diff --check` passed. Rebuilt `src-tauri/target/release/codex-usage-widget.exe`. The existing Tauri CLI native-binding limitation is avoided with the established Cargo build. Per user request, no test suites or native runtime/stress matrix were run; updated test expectations were not executed. Actual desktop appearance/DPI checks remain manual.

## Grouped period controls in Settings — 2026-10-24

Only the Screen Edge and Compact HUD groups in **Widget periods and reset countdowns** change layout. Each mode now has its own heading, a labeled selector beneath it, and a separate grid of reset checkboxes. The grid becomes one column when space is limited; hidden-period notes wrap with their checkbox text. Floating retains its inline row. Saved selections, countdown preferences, disabled-state rules, and accessible labels are unchanged.

Validation: the frontend production build and all seven tests in `node --test tests/quota-presentation.test.mjs` passed. Interactive visual checks at normal/narrow widths were unavailable: Windows Computer Use could not connect to its native pipe (`os error 2`), and browser inventory was empty. No new layout-only tests or broad validation matrix were added. See the [implementation report](implementation-report.md#external-activation-and-topmost-stacking--2026-10-24) for the separate native stacking correction and Photos limitation.

## HUD percentage and divider spacing — 2026-10-24

Latest follow-up: restored end alignment for countdowns and segments. Each reset column decreases from 61px to 55px at default text size, narrowing each quota area with an enabled countdown by 6px. The fixed HUD width decreases by 6px per visible countdown: 200 + 58n px for Words or 164 + 58n px for Badges when both periods are visible. Existing single-period bases remain 120px/102px. The 6px spacing on each side of the divider and percentage-column width are retained. Layout assertions and the design specification reflect the new dimensions; assertions were not run.

Follow-up validation: `npm.cmd run desktop:build` passed frontend type checking, production bundling and native release compilation after a normal-filesystem-access retry resolved replacement permission failure. Updated executable: `src-tauri/target/release/codex-usage-widget.exe`. Tests and visual/native runtime checks were not run, as requested; no installer was rebuilt. The following paragraph describes the earlier spacing revision.

HUD reset countdowns and segments align to the start of their reserved column, bringing the segments closer to the percentage. Horizontal quota grid gaps decrease from 3px to 2px at default text size. The gap before the middle divider and padding after it increase from 4px to 6px; quota groups use start alignment to keep the inter-group gap predictable. The fixed window dimensions are unchanged.

Validation: `npm.cmd run desktop:build` passed frontend TypeScript checking, Vite production bundling and native release compilation after a normal-filesystem-access retry; the restricted attempt could not replace the existing executable. Output: `src-tauri/target/release/codex-usage-widget.exe`. No installer was rebuilt. Tests and visual/native runtime checks were skipped as requested.

## Edge reset indicator width — 2026-10-24

The centered Edge reset strip is now 2.5625rem (41px at default text size), matching the visible outer ring diameter: 19px radius × 2 + 3px stroke inside the 44px SVG. The ten grid segments, countdown text width and shell dimensions are unchanged. This supersedes the earlier 44px strip width below; README and design sizing descriptions reflect the correction.

Validation: `npm.cmd run desktop:build` passed frontend TypeScript checking and production bundling, then failed to replace the locked standard executable. `cargo rustc --manifest-path src-tauri/Cargo.toml --release --locked --bin codex-usage-widget --features tauri/custom-protocol -- -C extra-filename=-edge-indicator` passed. Its output was copied to `src-tauri/target/release/codex-usage-widget-edge-indicator.exe` (11,731,456 bytes). No installer was rebuilt. Tests and visual/native runtime checks were not run for this CSS sizing change.

## Edge spacing follow-up — 2026-10-24

Edge bottom padding increases from 6px to 12px while top padding remains 6px. Ring-to-reset spacing decreases from 3px to 1px, and spacing above/below the middle divider increases from 3px to 5px. Fixed height becomes 186 + 24n px (Words) or 146 + 24n px (Badges), preserving content room for each enabled-countdown combination; width remains 54px and a warning still adds 18px height. Frontend and native sizing expectations match the revised profile.

For the final 12px-bottom revision, `npm.cmd run desktop:build` passed frontend type checking and production bundling but could not replace the locked standard executable. A separate release compiled successfully using `cargo rustc --manifest-path src-tauri/Cargo.toml --release --locked --bin codex-usage-widget --features tauri/custom-protocol -- -C extra-filename=-edge-padding`; its output was copied to `src-tauri/target/release/codex-usage-widget-edge-padding.exe`. Tests and runtime/visual checks were not run, as requested. Earlier dimensions, validation and hashes below apply to earlier revisions.

## Balanced compact padding — 2026-10-24

HUD now uses 6px left/right padding; Edge uses 6px top/bottom padding. The other sides remain 4px. HUD grows 4px wider and Edge grows 4px taller to preserve content space. With n countdowns, HUD width is 200 + 64n px (Words) or 164 + 64n px (Badges), at 38px tall. Edge remains 54px wide and its height is 176 + 26n px (Words) or 136 + 26n px (Badges). Warning increments remain 20px HUD width and 18px Edge height. Padding scales with text size. This supersedes the prior dimensions below; ring/segment widths, badge placement and normal fixed-size behavior are retained.

Validation: `npm.cmd test` passed all 27 tests; `npm.cmd run build`, `npm.cmd run rust:fmt`, and `npm.cmd run rust:check` passed. Browser inspection passed 32 layouts (both modes, both labels, four countdown combinations and 100% light/200% dark text) with 64 readings covering retained long countdowns and normal readings. Computed padding and fixed dimensions matched the profiles, with no clipping or content outside widget bounds. `npm.cmd run desktop:build` and the isolated debug build passed. Final release: 11,731,456 bytes, SHA-256 `E4312AABDD0E289445B36D7AB16FE63BEEF5170FEC56F4490C9209A85554ACDE`. The updated executable is `src-tauri/target/release/codex-usage-widget.exe`; no installer was rebuilt.

The isolated `--check-native-sizing` gate passed all 40 shell cases at 250% monitor DPI and finished with `native-check result=pass`, including preview/popup geometry and stacking. Two earlier attempts failed the existing `preview-client-sizing-stable` and `hover-preserves-foreground` assertions; the final run completed with build/browser activity stopped. Logs: `src-tauri/target/validation/balanced-padding{,-retry,-final}.{stdout,stderr}.log`. The existing WebView2 shutdown warning (1412) remains. Actual mouse interaction and mixed-monitor DPI remain manual checks. Validation tabs and the local server were closed; no user process was stopped. Changed-document links (8) and `git diff --check` passed.

## Compact spacing and Edge docking — 2026-10-24

This increment supersedes the prior dimensions, badge colors and vertical-only Edge drag policy. Existing containers, branding, themes, hover content and saved Words / Badges choice remain.

- Edge rings and ten-segment reset strips now share a **44px width** inside a **54px-wide** shell with **4px side padding**. Edge hides the clock and uses centered 10px time text; retained `~23h59m` fits. HUD retains its countdown font, uses an 11px clock, 2px icon gap, a 14px badge column and aligned right-side countdowns.
- Grey badges use secondary text, divider borders and the existing wash. Internal Edge badges are 12px (HUD stays 14px), moved 4px upward from their centered layout position. Known Edge percentages use blue/purple quota colors; unavailable values are neutral.
- With n enabled countdowns, Edge height is **172 + 26n px (Words)** or **132 + 26n px (Badges)**. A visible warning adds 18px height. HUD is **196 + 64n px (Words)** or **160 + 64n px (Badges)** by **38px**; a visible warning adds 20px width. Normal readings never resize a saved layout. Text scaling applies to the full profile.
- The native fixed-X moving constraint is removed. Edge follows the mouse freely, then docks using the release-time native bounds and pointer. The pointer chooses the drop monitor (nearest monitor fallback); the final widget center chooses the nearest side, with ties retaining the saved side. Destination DPI and an 8-logical-pixel work-area inset are applied before vertical clamping.
- Drag sessions track their starting mode, native window and bounds; unchanged/cancelled drags, locked positions, mode changes and retired windows cannot save an old drop. Hover collapses and drag releases keep existing click suppression. Monitor, side and placement save in one revision; a failed save preserves the prior authoritative settings and restores that dock.
- No new frontend command, persisted field, schema version, dependency or permission is introduced. Settings placement help now describes release-to-edge docking. The development-only `scenario=long-reset` fixture exercises the longest normal countdown.

### Validation and artifacts

- `npm.cmd test`: **27 passed**. `npm.cmd run build`: passed, including TypeScript checking; the final bundled debug build also reran it successfully.
- `npm.cmd run rust:check`, `npm.cmd run rust:fmt`, and `npm.cmd run rust:clippy`: passed. `npm.cmd run rust:test`: **105 passed, 1 explicit live probe ignored**, plus **3 vendored tests passed**. Normal-access retry resolved the restricted overlay's three existing atomic storage replacement failures.
- New Rust tests cover both-side docking, exact-center ties, work-area clamping, negative coordinates, monitor gaps, 100/125/150/250% DPI, logical placement round trips, unchanged/cancelled bounds, lock, mode changes and retired windows. A failed docking save preserves the entire previous settings document; a successful save round-trips side, monitor and height in one revision.
- Browser checks: final Edge long-countdown layouts passed Light/Dark × Words/Badges × 100/200% text (8 cases); high-contrast Edge passed all four countdown combinations × both labels × both scales (16 cases). HUD passed the corresponding long-countdown cases and optional-countdown/high-contrast checks. No normal countdown clipping or root-bound overflow was found. Eight six-state sequences across both modes, labels and scales preserved the base rectangle through 99.9%, zero/due, stale fractional, missing and restored data; only warning presence changed the permitted dimension. A final 200% dark screenshot verified 12px logical Edge badges raised above the colored values without overlap.
- `npm.cmd run tauri -- build --debug --no-bundle --config src-tauri/target/validation/widget-check-config.json`: passed with isolated validation identity. The updated `--check-native-sizing` gate passed **all 40 shell cases** at this machine's 250% DPI, preview/popup geometry and stacking, close-to-tray, and the new injected left/right docking assertions on the available monitor, including saved side/monitor/height, 8px inset, no accidental Overview and position lock. The first run passed sizing/docking but timed out in the existing Floating popup hydration phase; a separate retry completed with `native-check result=pass`. Logs: `src-tauri/target/validation/compact-docking{,-retry}.{stdout,stderr}.log`. The existing WebView2 shutdown class-unregister warning (1412) remains.
- `npm.cmd run desktop:build` compiled release code but could not replace the running widget's locked executable. The running user instance was preserved. A separate release was successfully linked using `cargo rustc --manifest-path src-tauri/Cargo.toml --release --locked --bin codex-usage-widget --features tauri/custom-protocol -- -C extra-filename=-compact-docking`, then copied from `release/deps` to `src-tauri/target/release/codex-usage-widget-compact-docking.exe`. It embeds the final production frontend. After the existing instance exited independently, a retry of `npm.cmd run desktop:build` passed and produced the normal `src-tauri/target/release/codex-usage-widget.exe`. No installer was rebuilt or installed.
- Final standard release: **11,731,456 bytes**, SHA-256 `C5297E3B874A505232BB077AFE4F67DA6D52E36D99A732463DDDF096EE790981`. Separate release: **11,731,456 bytes**, SHA-256 `6C696846540D1BF2DD7FB1D10C0D63E8B148C8F2222EFA4BF671F6C1BF71FFBB`. Isolated debug: **18,049,536 bytes**, SHA-256 `EF23276457CB7C678813F503D66967F1EB6266B0C8FD8028E8DB2738B210A647`.
- Changed Markdown links: **38 checked, none broken**. `git diff --check` passed. Validation tabs were closed; the local Vite server and isolated native gate were stopped after verification. No user process was stopped; the existing release instance had exited by the final process check.

Real pointer movement/cancellation, restart with saved Edge placement, Windows 10 and mixed-monitor DPI drags remain manual runtime gates. Geometry and serialization tests plus injected coordinator events do not establish those OS interactions. Release artifacts are compiled but were not launched for manual pointer validation.

## Badges, stable widgets, and hover details — 2026-10-24

This increment uses the supplied `Modern Dual Progress Dashboard.png` image only for the circular period-badge concept. Existing app surfaces, colors, typography, branding, 44px Edge rings, bars and native corners are retained. This section supersedes earlier content-driven Edge/HUD sizing and artifact entries below.

### Changes and contracts

- Display settings has one shared **Words / Badges** choice for Edge and HUD. Words remains the default. Circular **5 / W** badges sit inside Edge rings above the percentages and beside HUD values. Badges use CSS and existing semantic colors; no new artwork, dependencies or permissions were added.
- `settings.windows.quotaLabelStyle` accepts only `words` or `badges`. Rust's defaulted enum preserves old settings under schema version 4; the existing revision-checked Save/Cancel flow remains authoritative.
- Normal quota, countdown and connection-data updates keep each compact widget's predetermined client rectangle. Only a **visible warning** adds space; no empty warning slot is reserved. Label/countdown settings and accessibility scaling can recalculate the rectangle. Missing HUD data retains two slots without inventing values or durations. Known five-hour/weekly windows occupy stable slots even if their source order changes; nonstandard periods keep their own text and full tooltip.
- Edge's reset strip is 36px wide with the same ten time-remaining segments. HUD retains its strip width. Countdown cells constrain exceptional strings with full tooltip/accessible text; percentages reserve room for `99.9%` and remain fully visible.
- The shared Edge/HUD **hover** preview now shows used percentage and exact expected reset date/time with timezone, using dashboard detail rows within compact card spacing. These details do not depend on compact countdown switches. Last-known/unavailable states and due-but-unconfirmed resets remain explicit. Tray hover stays compact; Floating has no hover expansion.
- HUD's single-click popup now shares centered horizontal placement with hover, including the existing 8-logical-pixel gap, above/below preference and work-area clamp. Settings navigation and content fitting retain the shell anchor. Standard-window promotion and foreground-denial recovery remain intact.
- Dashboard reset-credit details use divided icon-led rows with readable status and expiry. Three rows appear initially, followed by **Show more (N)** / **Show less** when needed. The reported available count is never inferred from row count or expiry. Expansion preserves keyboard focus and ordinary-refresh state, resets for a new Overview request or account context, and requests content fitting without overriding a manually resized standard window.
- Floating's **×** issues the narrow `close-to-tray` request, which is accepted only from its shell while Floating is selected and a recovery tray exists. Rust routes it through the normal persisted System Tray mode selection. The button is disabled without a tray; native-menu Hide/show remains temporary. No extra IPC capability was introduced.

Client dimensions at 100% text, where **n** is the number of enabled countdowns (0–2):

| Layout | Normal width | Normal height | Visible warning adds |
| --- | ---: | ---: | --- |
| Edge, Words | 60px without countdowns; otherwise 72px | 180 + 30n px | 20px height |
| Edge, Badges | 60px without countdowns; otherwise 72px | 140 + 30n px | 20px height |
| HUD, either style | 212 + 68n px | 44px | 20px width |

Dimensions use scalable units and continue through the existing logical-client/native-outer conversion. A browser check exposed that `99.9%` needed more width than the initial HUD value column; the final column is 5.5ch and the footprint includes that allowance. Genuine content/work-area limits still apply at extreme accessibility sizes.

### Validation

| Command/check | Result |
| --- | --- |
| `npm.cmd test` | Passed 27 tests. New coverage checks fixed layout/warning dimensions, missing/reordered/nonstandard HUD windows, compact hover detail visibility and stale/missing values, credit rows at 0/1/3/4/30 entries, expiry epoch zero, and preserving the reported available count. Palette checks include badge text colors/background. |
| `npm.cmd run build` | Passed TypeScript and Vite production builds, including the native builds' frontend steps. |
| `npm.cmd run rust:test` with normal filesystem access | Passed 103 application tests and 3 vendored auto-launch tests; 1 explicit live protocol test ignored. Added tests cover the label default/round trip/invalid enum and close-to-tray source/mode/tray/payload restrictions. The restricted run failed three pre-existing atomic-storage tests with `storage-commit`; the normal-access retry passed. |
| `npm.cmd run rust:check`, `npm.cmd run rust:fmt`, `npm.cmd run rust:clippy` | Passed. Cargo sometimes copied incremental artifacts when the restricted filesystem could not hard-link them. |
| `npm.cmd run desktop:build` | Passed; rebuilt the standalone release executable without an installer. |
| `npm.cmd run tauri -- build --debug --no-bundle --config src-tauri/target/validation/widget-check-config.json` | Passed using the existing isolated validation identity. |
| Isolated debug `--check-native-sizing` | Passed with `native-check result=pass` at the current Windows 250% DPI: 40 shell cases covering both themes, all countdown combinations, Floating and both Edge/HUD label styles. Preview sizing, rounded regions, centered HUD hover, tray baseline, reveal/topmost checks, HUD click-popup center/clamp with topmost off/on, Floating close-to-tray, and activation-denial recovery passed. |

The first native attempt passed its 40 sizing cases, then failed `reveal-preserves-foreground` while browser validation was also active. A later run without concurrent browser interaction passed the complete gate; the first attempt is not counted as a passing run. The existing WebView2 shutdown diagnostic (`Chrome_WidgetWin_0`, error 1412) still appears after the successful marker. Structural logs remain under ignored `src-tauri/target/validation/badges-stable-sizing*.stdout.log` and corresponding stderr logs, with no quota values or account identifiers.

Synthetic browser verification (existing development fixture only):

- 16 Light layout cases: Edge/HUD × Words/Badges × 100%/200% text × countdowns off/both, with widest normal/stale readings; no internal overflow or normal countdown ellipsis.
- Eight Dark layout sequences: both modes/styles at 100%/200%, each with six successive snapshots covering normal values, `99.9%`, zero/due, stale fractional, unavailable, and restored data. Normal snapshots retained identical bounds; warnings alone changed height/width and removed that space on recovery.
- Eight 200% high-contrast cases: both modes/styles with only five-hour or only weekly countdowns; no overflow and all indicators retained ten segments.
- Eight desktop-hover cases: both modes/themes at 100%/200%; each supplied quota showed used percentage and a reset timestamp even with collapsed countdowns off. Component tests cover missing/stale metadata.
- Eight credit-layout cases: 480×650 popup / 620×760 window × both themes × 100%/200%; initial three rows, expansion to eight, focus retention, and no horizontal overflow. Mouse and Enter/Space controls, ordinary-refresh retention and new-Overview collapse were verified. Badge-setting Save/Cancel and Floating's isolated `close-to-tray` action emission were checked.

The browser viewport override was reset and the active validation tab closed. An earlier temporary tab lost its debugger binding and could not be explicitly closed; it is left to automatic session cleanup. Screenshots were inspected in tool output and were not added as production assets.

### Artifacts and remaining limits

The final bundled-frontend native rerun also passed all 40 sizing cases and `native-check result=pass`, including both HUD popup assertions and Floating close-to-tray. Its structural output is `src-tauri/target/validation/badges-stable-sizing-verified.stdout.log`; the corresponding stderr retains only the noted WebView2 shutdown diagnostic.

| Final artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `src-tauri/target/release/codex-usage-widget.exe` | 11,769,344 | `2C2DFC59D27AB582B2B85EC32B5EB201FCBD0F68E4A1DB3C1BB5771C367F1D5E` |
| `src-tauri/target/debug/codex-usage-widget.exe` (isolated identity) | 18,000,384 | `ED0334EE3AE159640975A588A65E17D984DDA262BD08AD265FB2620E7FEFC5CC` |

The release notification PNG remains adjacent to the standalone executable. The existing installer and installed application were not replaced. All 38 local Markdown links in the five maintained documents resolve; `git diff --check` passed. Final normal-access cleanup found no widget process and no development listener on port 1420. Build outputs and structural logs remain ignored.

Real pointer travel/drag/focus, Explorer recovery, mixed-monitor DPI, Windows 10 and live OS accessibility changes remain outside this increment's verified coverage. The scripted native check exercises actual window geometry and injected app-local coordinator events; it does not establish real mouse/keyboard acceptance or notification delivery.

## Implemented

- Shared generated ring identity, SVG control icons, remaining-quota cards, percentage rings, and Light/Dark surface tokens across all four modes, desktop/tray previews, dashboard and Settings.
- Floating is a richer mini-card with a brand/action header, two quota bars, independent optional reset forecasts, known reset credits, and connection status. Edge stacks labeled quota rings; HUD uses inline bars and forecasts. Native bounds still follow measured contents.
- Overview and the single-click popup show used percentages and exact reset timestamps inside quota cards, with credit status/expiry and read timing visible directly. A compact reminder/snooze summary links to Settings. Blocking usage, storage and connection diagnostics stay visible.
- Settings has mode thumbnails, grouped preference cards, aligned countdown preferences and a persistent Save/Cancel bar. Popup/window navigation and revision-checked drafts are preserved.
- Floating's Settings control issues only `surface_action: {type:"settings"}`; Edge/HUD omit that button. Rust validates the source/payload, cancels pending Overview activation and opens Settings in the anchored popup, preserving an existing standard window. Toolbar buttons do not initiate whole-surface drag/activation. No additional capabilities, settings migration or quota client were introduced.
- Native app/tray/installer ICO and notification PNG derive from the ring master. Compact shells/previews use rounded native regions without native shadows; dashboard shadows follow high contrast. High contrast replaces decorative imagery with a system-color mark and suppresses decorative effects; reduced motion stops the refresh animation.

## Window coordination and direct usage details — 2026-10-23

This correction supersedes the historical hover, focus-warning, layout and artifact evidence below. The internal-layout references are the supplied private HUD and Edge images (not distributed with this repository). Their container artwork was not adopted.

### Root causes and changes

- Native `SetWindowPos`/style writes previously changed the real HWND without updating Tao's cached topmost/focusable flags. Tauri property setters were queued from the service thread and could subsequently rewrite extended styles from that cache. The old native flag repair did not prevent this later rewrite. Creation now sets topmost/focusability correctly, and one owning-thread operation applies changed framework properties, native tool styles and final HWND verification in order. Own-window style/position notifications coalesce mismatch repairs; no timer, foreground loop or repeated ordinary-window raising was added. Policy entries retire with their HWND.
- `native-foreground-denied` was a local diagnostic, produced by immediately comparing `GetForegroundWindow` after a queued Tauri focus call. Native show/hide also bypassed Tao's visibility cache, making that fallback unreliable; Tao 0.37.1's focus path can synthesize Alt on denial when its cached visibility permits the call. Early errors left pending activation unconsumed, and generic coordination handling marked custom hover unavailable and retained the warning. Activation now makes one native foreground request on the owning thread and returns a typed accepted/denied outcome. Denial consumes the request, promotes an anchored popup to a standard taskbar-accessible window, requests attention once and retains tray recovery. It neither enables tooltip fallback nor creates a failure warning. Genuine errors remain diagnostic; a later successful coordination clears its transient error. No synthesized input or foreground-lock workaround is used. [Windows activation rules](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow) and [Z-order semantics](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos) govern the implementation.
- Floating shell hover is excluded by the authoritative interaction policy, including pending dwell work. Its compact appearance, Settings/Hide, drag, single/double-click, context menu and tray hover remain. Edge/HUD retain their shared preview content and timing. Edge hover replaces the explicit zero gap with 8 logical pixels; HUD hover replaces right alignment with trigger-centered placement. Work-area clamping and pointer corridors include the new gap. Click-popup placement is unchanged.
- HUD retains the application icon and uses paired two-row quota/countdown groups. Edge retains the icon and 44px quota rings, with period groups separated by a subtle divider. Existing container backgrounds, corner radii, effects and padding/gap treatment remain. Independent reset preferences show/hide both countdown text and ten segments. Segments represent remaining time divided by the reported window duration, clamped to 0–100%, with positive partial segments rounded up. Unknown time stays unknown; stale forecasts are marked; elapsed time says `due?` and never confirms quota recovery. Duration examples are `3h10m` and `6d7h`.
- Dashboard and single-click Overview omit introductory marketing copy and usage/credit/read disclosure controls. Quota cards show used percentage and exact local reset time with small metadata icons and a numeric timezone offset. Optional credit status/expiry remains visible. Reminder controls/history stay under Settings → Reminders; Overview shows status/snooze and a direct Manage route. Draft protection, keyboard heading focus and scroll reset on navigation remain functional.
- The existing main-view readiness IPC now carries generation, page, measured client width/height and a fit hint. Stale/nonfinite/out-of-range measurements are rejected. Preferred client widths remain 620px for a standard window and 480px for a popup; native frame insets and monitor work areas constrain content fitting. New usage/page content can fit until the user moves/resizes/maximizes the standard window. Countdown-only measurements do not resize it. Hydration waits for the requested page; genuinely oversized/enlarged-text content scrolls. No dependency, capability, settings-schema or quota-service change was needed.

### Validation

| Check | Result |
| --- | --- |
| `npm.cmd test` | Passed 22 frontend policy tests, including readable duration boundaries, exact timestamp formatting and time-remaining segments. |
| `npm.cmd run build` | Passed TypeScript and Vite, also run by native builds. |
| `npm.cmd run rust:test` with normal filesystem access | Passed 101 application tests and 3 vendored auto-launch tests; 1 explicit live read-only protocol test remains ignored. New coverage includes Float shell/tray distinctions, hover geometry/corridors at fractional DPI, native policy mismatches, activation denial and content-fitting/manual-resize policy. The restricted filesystem overlay still fails three existing atomic-storage tests; normal-access final runs pass. |
| `npm.cmd run rust:fmt`, `npm.cmd run rust:check`, `npm.cmd run rust:clippy` | Passed, including all-target Clippy with warnings denied. The restricted build filesystem reports hard-link-cache fallbacks; compilation completes. |
| `npm.cmd run tauri -- build --debug --no-bundle` | Passed. Debug native checks use the existing validation-only settings subdirectory. |
| Debug executable `--check-native-sizing` | Passed with `native-check result=pass`, exit 0, at current 250% DPI. Checks 12 shell cases, 8 Edge/HUD desktop-hover cases, tray preview baseline, rounded regions and Float hover exclusion. Native assertions verify HUD center/clamp, Edge 8px hover gap, topmost off/on, framework style rewrites, event-driven native-demotion repair and foreground preservation. |
| Injected native activation denial, inside sizing gate | Passed actual standard-window/taskbar recovery, normal topmost state, no coordination warning/tooltip fallback and unchanged foreground. This injects the native outcome; it does not establish every real foreground-lock condition. |
| Debug executable `--check-native-windows` | Passed with `native-check result=pass`. Covers deferred click/double-click, popup and Settings coordination, promotion, Edge detail placement, tray recovery, minimize/restore, idle retirement, native reconciliation and single-instance recreation. Debug readiness assertions wait for activation completion before inspecting presentation. |
| `npm.cmd run native:stress` | Final rerun passed 100 transitions, `native-stress result=pass cycles=100`, `native-check result=pass`, exit 0 and `remainingOwnedProcessesBeforeCleanup=0`. The 44.619-second debug run sampled 314.367–368.027 MiB across plateau samples; sampled machine CPU was 1.914% across 24 logical processors. These are transition-load measurements, not an idle/release performance claim. |
| `npm.cmd run desktop:build` | Passed the final release build. No installer build or installation is part of this correction. |

Synthetic browser review used the development-only fixture, with no live account data:

- All three compact modes × Light/Dark × 100%/200% text: 12 cases, no internal horizontal overflow. Rounded default-text dimensions with both forecasts selected: Floating 274 × 287, Edge 64 × 237, HUD 299 × 42 logical pixels. Container sizing remains content-based.
- Three compact modes × loading/missing/partial/stale/blocked/fractional/due/no-reset: 24 cases, no internal horizontal overflow. Due indicators show no lit segments with `due?`; unknown timestamps show `—` and unavailable tracks.
- Dashboard/popup × Light/Dark × 100%/200% text: 8 cases, visible used/reset metadata and no usage disclosures or internal horizontal overflow. Enlarged text scrolls vertically rather than shrinking type.
- High-contrast 200% text: all three compact modes and the popup with explicit credit metadata fit horizontally. System-color branding and visible segment borders remain. The direct reminder route opens the correct Settings page; a dirty reminder draft triggers Save/Discard/Cancel, and Discard returns to Overview. Credit status/expiry spans its own compact row.

Logs remain under ignored `src-tauri/target/validation/window-layout-*.stdout.log`, corresponding stderr logs and the native stress sampler outputs. They contain structural validation evidence, not usage responses or account identifiers. The first direct shell invocation produced no marker and was not counted as a passing gate; final invocations use `Start-Process -WindowStyle Hidden` with explicit stdout/stderr capture and process exit/marker checks.

### Current artifacts

- Standalone executable: `src-tauri/target/release/codex-usage-widget.exe`, version 0.1.0, **11,753,472 bytes**, SHA-256 `727B3E7962BE1756C8AE177CC4E62D4C79B43B676D0C3FADE19FF5A17A4CE341`.
- Final debug executable used by the final coordinator/stress run: `src-tauri/target/debug/codex-usage-widget.exe`, SHA-256 `8B7B72BD05772F17A5B9939C2AA567BA94ED147986574CF25C7C757AA5185855`.
- Generated artifacts and structural validation logs remain ignored. The installed application/installer was not replaced. No new tools or dependencies were installed.

### Remaining limits

The Windows Computer Use helper was checked and remains unavailable (`os error 2`). Actual pointer travel, real application creation/activation/switching, Explorer recovery, mixed-monitor DPI, Windows 10, OS accessibility changes and native visual quality remain manually unverified. Scripted ordinary-window probes and injected coordinator events are limited evidence, not substitutes for that matrix. Foreground denial remains subject to Windows policy; the supported recovery respects it. The existing WebView2 shutdown diagnostic (`Chrome_WidgetWin_0`, error 1412) appears after passing markers. Notification availability/release gates are unchanged. The final stress sampler confirms no owned native process survived shutdown. The late extra browser-preview batch timed out before returning evidence and is not counted above; browser cleanup lost its debugger binding, leaving that temporary fixture tab to session cleanup.

## Widget recovery, rounded boundaries and popup stacking — 2026-10-23

This correction supersedes the earlier compact sizing implementation below. Floating retains Settings and Hide; Edge and HUD use the shared context menu/tray for Settings. Tighter padding/gaps preserve existing text sizes and the 44px default quota rings. Synthetic default-text measurements with both countdowns enabled changed to approximately Floating **274 × 287**, Edge **58 × 212**, and HUD **381 × 38** logical pixels.

Native changes:

- Shell/preview creation disables native shadows. Win32 rounded window regions match the CSS radii (Floating/desktop preview 14px, Edge 16px, HUD 12px, tray preview 10px), scaled to actual DPI and refreshed after geometry changes. Successful region ownership is transferred to Windows; unchanged regions are not reapplied. The shape excludes corner paint and hit testing while card interiors stay opaque.
- Preview sizing now uses the same logical-client to physical-outer conversion as compact shells. The CSS viewport-width cap was removed from previews to prevent a constrained native viewport feeding smaller measurements back into placement. Current-generation readiness gates preview display. Real content larger than the work area remains clamped and can scroll.
- State publication no longer changes native window styles. Its previous all-window shadow setter contradicted compact shadow policy; the first native run showed content-ready messages but a hidden shell. Centralizing shadow application before native placement/show resolved that regression. Dashboard shadows still honor high contrast.
- Explicit tray mode selection, including the selected mode, restores a hidden desktop widget and raises it once after current-generation hydration. Show widget and actual Settings mode changes also reveal it. Unrelated settings edits preserve hidden state. Hide, tray-only selection, a superseding mode, and suppression cancel pending raises.
- Persistent topmost handling reads and verifies the real HWND flag and synchronously repairs it when necessary. This avoids relying solely on Tao's cached asynchronous setter. One-time raising leaves Always on top unchanged; ordinary reconciliation does not raise a non-topmost widget.
- Floating previews and clicked detail evaluate above/below/right/left placements, select the least overlap after work-area clamping, and maintain detail above the trigger. Popup topmost matches the widget preference and resets on standard-window promotion. Edge selected-side behavior remains intact.

Validation of the final native source:

| Command/check | Result |
| --- | --- |
| `npm.cmd test` | Passed all 20 frontend policy tests. |
| `npm.cmd run build` | Passed TypeScript and Vite production build, including the final debug build's frontend step. |
| `npm.cmd run rust:test` with normal filesystem access | Passed 95 application tests and 3 vendored auto-launch tests; 1 explicit live protocol test ignored. Seven new tests cover four-side placement, negative/fractional-DPI geometry, unavoidable overlap, reveal readiness/cancellation, settings behavior and stale preview measurements. The initial restricted run failed three existing atomic-persistence tests; normal-access runs passed. |
| `npm.cmd run rust:fmt`, `npm.cmd run rust:check`, `npm.cmd run rust:clippy` | Passed. |
| `npm.cmd run tauri -- build --debug --no-bundle --config src-tauri/target/validation/widget-check-config.json` | Passed with the existing isolated validation identity. No dependencies or tools installed. |
| Isolated `--check-native-sizing` | Passed with `native-check result=pass` at current **250% DPI**. All 12 shell cases and 12 desktop preview cases passed stable client sizing and rounded-region checks; native frame insets are now **0 × 0**. Both tray theme cases passed the 260 × 76 baseline. Hover checks preserved foreground and placed previews above their shells. |
| Native reveal/new-window check, within sizing gate | Passed Floating/Edge/HUD with topmost off/on. Hidden selection and same-mode reselection restore visibility; explicit selection raises above an app-owned ordinary native test window. A newly opened ordinary test window stays below topmost widgets and above non-topmost widgets; routine reconciliation preserves that order. No OS mouse/keyboard input is injected. |
| Native Floating popup check, within sizing gate | Passed actual popup creation/hydration, popup-above-Floating order, and matching topmost flags with the preference off/on. A debug-only coordinator input opens the real popup without requesting foreground activation; this establishes geometry/stacking, not real click/focus behavior. |
| Full `--check-native-windows` | Did not pass: standard style and first shell sizing/rounded-region checks passed, then the existing `native-foreground-denied` condition stopped hover hydration. This is not claimed as a passing interaction gate. |
| `npm.cmd run desktop:build` | Passed; rebuilt the standalone release executable without an installer. |
| Documentation/diff checks and cleanup | All 33 local Markdown links in the five changed documents resolve; `git diff --check` passed. Normal-access queries found zero widget processes and zero development listeners on port 1420; the temporary browser tab was closed. |

Synthetic Edge-browser checks used only the existing development fixture:

- Three widget modes × Light/Dark × 100%/200% text (12 cases): no internal horizontal overflow; Settings only on Floating. Default Edge rings remain 44px.
- Three modes × loading/missing/partial/stale/blocked/fractional data (18 cases): no internal horizontal overflow; control visibility correct.
- Desktop/tray previews × Light/Dark × 100%/200% text (8 cases): no internal horizontal overflow; desktop retains Details and tray has no controls. Default tray is 260 × 76; at 200% text it grows to approximately 260 × 169.
- Three modes at 200% text/high contrast (3 cases): no horizontal overflow and decorative brand images hidden. Floating, Edge and HUD screenshots were inspected through browser tools; no screenshots with account data were created.

The native Computer Use helper remains unavailable (`os error 2`), so real pointer travel, click/drag focus, native visual quality, Windows 10, mixed-monitor DPI, Explorer recovery and OS accessibility changes remain manually unverified. The sizing run still emits the pre-existing WebView2 shutdown diagnostic (`Chrome_WidgetWin_0`, error 1412) after its successful marker. Structural logs are under ignored `src-tauri/target/validation/widget-fixes-*.log`; they contain no quota values or account identifiers. Installer rebuild/installation are outside this correction.

Current standalone artifact: `src-tauri/target/release/codex-usage-widget.exe`, version 0.1.0, **11,700,736 bytes**, SHA-256 `E0444D1079E69F1397D434A1DF083735036F1B94D7E76E8C7F3C321B641B7302`. The adjacent `notification.png` is present. Isolated debug executable: **17,763,328 bytes**, SHA-256 `71EEFA71F3A515B7C6034CF0E575FB3980D4C0CFB2A8CDA6C1C2B771779658C5`. Older artifact entries below are historical.

## Compact client-area sizing correction — 2026-10-23

The persistent compact-widget scrollbars came from treating React's content measurements as native **outer** dimensions. The pinned Tao 0.37.1 Windows implementation reserves hidden non-client insets for borderless windows with shadows. At the tested 250% DPI those insets measured **32 × 19 physical pixels**, reducing the available client area under the old placement code. Earlier browser-only content-fit checks did not exercise that native boundary.

Implemented:

- Floating, Edge, and HUD now scale their measured logical client dimensions upward to whole physical pixels and add the actual outer/client size difference before monitor clamping and Edge placement. Current shadow/style settings are applied before measuring the insets.
- Shadow/high-contrast and DPI changes reconcile native sizing; corrected outer bounds drive drag constraints and popup anchors. A changed shell rectangle re-anchors an existing widget popup. Unchanged native bounds/styles skip redundant frame changes.
- `ready.width`/`height` retain their existing IPC shape and explicitly mean logical client-area size. No layout redesign, scrollbar-hiding CSS, quota logic, dependencies, permission changes, or settings migration was introduced. Genuine content exceeding a work area remains clamped and is not claimed to fit.
- A debug-only `--check-native-sizing` gate compares generation-matched React measurements with actual Windows client dimensions across all three widgets × Light/Dark × both countdowns off/on. It needs no foreground activation, uses the existing isolated validation profile, restores its test preferences on success, and quits. The same sizing cases precede the interaction assertions in the broader native check.

Validation on the final source:

| Command/check | Result |
| --- | --- |
| `npm.cmd test` / `npm.cmd run build` | Passed 20 frontend policy tests and TypeScript/Vite production build. |
| `npm.cmd run rust:fmt`, `npm.cmd run rust:check`, `npm.cmd run rust:clippy` | Passed. Cargo used copies when incremental hard links were unavailable. |
| `npm.cmd run rust:test` with normal filesystem access | Passed 88 application tests and 3 vendored auto-launch tests; 1 explicit live protocol test ignored. New geometry tests cover zero/nonzero insets, 100–250% scale including fractional scales, upward rounding, work-area clamping and Edge popup anchoring. |
| `npm.cmd run tauri -- build --debug --no-bundle --config src-tauri/target/validation/widget-check-config.json` | Passed using the existing `dev.codexusagewidget.validation` identity overlay. |
| Debug executable `--check-native-sizing` | Passed all 12 cases at the current Windows 250% DPI. Actual client width/height equaled the scaled content requirement for three consecutive samples in each case; native frame insets were 32 × 19 physical pixels. Required `native-check result=pass` marker present. |
| Debug executable `--check-native-windows` | Did not pass: identity, standard style and the first Floating sizing case passed, then Windows reported `native-foreground-denied` during sizing hydration. Later popup/interaction assertions were not reached. The independent sizing gate above avoids this pre-existing focus limitation. |
| `npm.cmd run desktop:build` | Passed; rebuilt the standalone release executable without an installer. |
| Documentation and diff checks | All 38 local Markdown links in changed documents resolve; `git diff --check` passed. |
| Cleanup | Normal-access process/listener queries found no widget process and no listener on port 1420 after validation. |

The native runs recorded the existing WebView2 shutdown class-unregistration diagnostic (`Chrome_WidgetWin_0`, error 1412). The successful sizing marker establishes client geometry, not visual/input acceptance. Real pointer/focus behavior, mixed-DPI transitions, enlarged Windows text, OS high-contrast switching and native missing/stale-state visual inspection remain unverified in this correction. No machine settings were changed or new tools installed; normal read-only monitoring remained active in the isolated debug app.

Previous standalone artifact (superseded by the widget recovery correction above): `src-tauri/target/release/codex-usage-widget.exe`, version 0.1.0, **11,671,040 bytes**, SHA-256 `AE5C31DE74E726625707C60DFAA696681B603477F07D3796185E52A87BCB0A67`. Its adjacent `notification.png` is present. Final isolated debug SHA-256: `EC017B4052CC875411BC67D46D03C29A4C12C5020791E3CFAEABA231D0E2E34C`. The installer was not rebuilt or installed for this correction. Structural runtime logs remain under ignored `src-tauri/target/validation/compact-sizing.*.log` and `compact-window-check.*.log`.

## Generated artwork and provenance

Generated with the built-in imagegen tool; no API-key/CLI generation path or additional software was used. The approved assets are stored locally:

| Purpose | Master | Runtime asset |
| --- | --- | --- |
| Brand and Windows identity | [Ring master](assets/widget-ring-master.png) | `src/assets/brand-ring.png` (128px), `src-tauri/icons/icon.ico`, `src-tauri/icons/notification.png` (128px) |
| Welcome and empty state | [Illustration master](assets/widget-empty-master.png) | `src/assets/usage-empty.png` (512px) |

The installed Tauri CLI resized the assets with preserved alpha and produced the multi-size Windows ICO. Intermediate icon sets remain in ignored `src-tauri/target/validation/`. Existing file names/configuration were retained for Windows resource consumers. The reference board was not modified or duplicated.

Exact generation prompts:

> Use case: logo-brand. Asset type: production Windows desktop utility icon and in-app brand mark for Codex Usage Widget. Create a single original sculpted open circular C-shaped ring, rich cobalt blue on the right flowing into luminous cyan on the left, rounded substantial ends opening at lower right, clean circular negative center. Straight-on orthographic view, centered and occupying 84% of a square canvas. Polished but restrained Fluent-style dimensional material, crisp silhouette, broad simple color fields that remain recognizable at 16 pixels. No lettering, no text, no other objects, no plate or rounded-square backing, no cast shadow, no surrounding glow, no watermark. Genuine transparent background. One mark only.

> Use case: stylized-concept. Asset type: small decorative welcome/empty-state illustration for a polished Windows quota monitoring utility. A quiet isometric composition of two overlapping rounded rectangular interface cards floating together: one pale icy white-blue card in front, one deeper blue card behind, with two simple blue and violet horizontal inset tracks and a small circular cobalt/cyan ring emblem. A tiny detached clock outline beside them. Carefully composed sculpted Fluent-style illustration, soft material depth, restrained highlights, cool blue/cyan/violet palette. Readable miniature illustration, not a screenshot. Centered compact composition, ample transparent margin, no text, no letters, no numbers, no percentages, no people, no wallpaper or scene background, no watermark. Genuine transparent background.

Derivation commands:

```powershell
npm.cmd run tauri -- icon docs/assets/widget-ring-master.png --output src-tauri/target/validation/polish-icons
npm.cmd run tauri -- icon docs/assets/widget-empty-master.png --fit contain --png 512 --output src-tauri/target/validation/polish-empty
```

Only the selected ICO/PNGs are retained as source assets. The in-app ring is 16,893 bytes and the empty-state illustration is 107,935 bytes.

## Validation

- `npm.cmd test`: passed 20 tests, including existing quota/countdown/gesture/contrast policies and new alpha/dimensions/file-budget/ICO-frame checks.
- `npm.cmd run build`: passed TypeScript and Vite; generated artwork is bundled as local assets.
- `npm.cmd run rust:check`, `npm.cmd run rust:fmt`, `npm.cmd run rust:clippy`: passed for the native changes.
- `npm.cmd run rust:test` with normal filesystem access: passed 86 application tests and 3 auto-launch tests; one explicit live probe ignored. A new regression test exposed that a Serde unit enum variant accepted extra fields; the Settings request now uses an empty struct variant and the rejection test passes.
- `npm.cmd run tauri -- build --debug --no-bundle --config src-tauri/target/validation/widget-check-config.json`: passed with the existing isolated validation identifier. Cargo fell back from incremental hard links to copies; no compiler error.
- `git diff --check`: passed. No dependencies, lockfiles or IPC capabilities changed.

### Synthetic browser inspection

The development-only `src/preview.html` fixture was exercised in Edge. Screenshots were inspected in the tool output; no browser screenshots are stored as repository assets. All quotas are synthetic. Browser results do not establish native pointer transport, focus, monitor geometry or notification delivery.

| Coverage | Result |
| --- | --- |
| Floating / Edge / HUD × Light / Dark × 100% / 200% text (12 cases, repeated after final icon sizing) | No internal horizontal overflow. Both selected countdowns visible, artwork loaded. At 100%: Floating 282×327, Edge 62×278, HUD 462×47 logical px. At 200%: 538×608, 122×554, 920×92 respectively. Contents and text scale determine the measured bounds. |
| Three widgets × loading / missing / partial / stale / blocked / fractional (18 cases) | No internal horizontal overflow; pending, last-known and blocked labels retained; fractional quota remains `<1%`. |
| Desktop / tray previews × Light / Dark × 100% / 200% (8 cases) | No internal horizontal overflow. Desktop has a Details control; tray has no interactive controls. Tray is 260×76 at default scale and grows vertically to 260×169 at 200%. |
| Settings at popup 480×650 / window 620×760 × Light / Dark × 100% / 200% (8 cases) | No horizontal overflow. Large-text narrow headers use labeled icon controls and a shorter footer. Save/Cancel remain inside the scroll area; the final popup save bar is approximately 104px high at 200%. |
| High contrast Floating, Edge at 200%, HUD and popup Settings at 200% | Decorative PNGs hidden, system-color mark visible; no internal overflow after fixing thumbnail lettering. Native gradients suppressed. |
| Welcome and empty Overview | Generated illustration loads; empty state remains explicit and does not substitute zero quota. Both normal and enlarged text inspected. |
| Settings flow | All four sections reachable. Saving countdown edits succeeds and preserves popup presentation. Settings/Hide widget controls issue only their own actions. Keyboard activation requests Details; double-click requests deferred activation followed by Open Window for native cancellation. |

Browser findings fixed during implementation: placement labels and invisible theme radios causing overflow, large-text icon sizing, large-text popup header/save-bar space, and high-contrast thumbnail lettering. The temporary viewport override was reset, the active task tab was closed and the development server was stopped. An earlier detached fixture tab could not be closed through browser control and is left to session cleanup.

### Native integration and package

The debug check ran with identifier `dev.codexusagewidget.validation` and isolated validation settings. Identity, standard/Floating styles, preview hydration, preview foreground preservation and preview native styles passed. It then failed at preview dismissal with `native-foreground-denied`; the added anchored Settings runtime assertion was not reached. A WebView2 shutdown class-unregistration diagnostic followed. The process exited; no validation widget process remained. Unit tests cover the Settings request, cancellation, retained popup/window presentation and rejected extra payload fields.

The default `npm.cmd run desktop:package` could not replace an executable locked by a running instance. The final package succeeded using the existing alternate build directory:

```powershell
$env:CARGO_TARGET_DIR=Join-Path (Get-Location) 'src-tauri/target/interaction-gestures'
npm.cmd run desktop:package
```

Restricted bundling initially could not reach the already approved official NSIS download/cache. The normal-access retry succeeded with the existing Tauri-managed NSIS tooling; no new global software was installed. The installer was rebuilt, not installed or smoke-tested in this increment. The standalone executable's adjacent notification PNG was updated from the selected source asset.

| Final artifact under `src-tauri/target/interaction-gestures/release/` | Bytes | SHA-256 |
| --- | ---: | --- |
| `codex-usage-widget.exe` | 11,670,016 | `E096AEFA4E30EB6059FDE7D8186B5B0B61BB32B9EE5EA6451C34436E3779D542` |
| `bundle/nsis/Codex Usage Widget_0.1.0_x64-setup.exe` | 2,818,515 | `8E2ADA3598A0852A1CABF5D074E1FAC4D3515AC9005CE5B4810F8357BE91C2F5` |
| `notification.png` | 16,893 | `0025D7EE2171AC3A5A1464480B356A1BA1C744ADC1F06B4B773527A9A7CEF66E` |

## Remaining verification

The Windows Computer Use helper reports an unavailable native pipe (`os error 2`). Actual pointer/focus, native icon appearance, DPI/monitor-edge placement and Windows accessibility still require runtime verification. Browser fixtures use synthetic data and do not establish native behavior or notification delivery. Existing notification/release gates remain open.

Local Markdown links in the maintained documents resolve. The source/assets/docs change is committed locally; generated executables, installer, caches and validation logs remain outside version control.
