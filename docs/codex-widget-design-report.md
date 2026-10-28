# Codex Usage Widget — UI/UX Design Report

**Design specification**  
**Version:** 1.13 — shared countdown readability and independent compact-widget scaling
**Date:** 28 October 2026
**Target platform:** Windows 10/11  
**Proposed stack:** Tauri 2 · TypeScript · React · Tailwind CSS · Rust application services

**Proposed follow-up:** [Widget appearance and consistent snapping](widget-appearance-and-snap-design.md) specifies configurable background transparency, element colors, and a shared desktop snap gap. It is a design proposal; the current implementation and validation status remain in the implementation reports.

---

## 1. Executive summary

This report specifies the user experience and interaction model for a lightweight Windows desktop widget that presents Codex usage limits, reset times, and available rate-limit reset credits. The product supports four interchangeable display modes: **Floating Widget**, **Screen-Edge Widget**, **System-Tray Mode**, and **Compact HUD**. All modes share one rate-limit data source, consistent terminology, and complete light and dark presentations. System-tray mode offers a compact read-only box on hover and a larger dashboard popup on click.

The central principle is **progressive disclosure**: **glance** at essential values in the collapsed state; **hover** to obtain a temporary preview; **click** to open or raise the Overview dashboard; and **dismiss** to restore the compact presentation. Screen Edge and HUD share one hover preview behavior; Floating opens details only on click; tray hover remains a smaller read-only summary. A configurable reminder service adds Windows desktop notifications for low remaining usage, expected resets, confirmed recovery, and user-defined schedules.

The recommended implementation is **Tauri 2 + TypeScript/React**, with Rust owning process supervision, authoritative state, refresh scheduling, persistence, windows, and reminders. Quota information will be requested from the local Codex app-server using `account/rateLimits/read`; visible interfaces render local countdowns between requests. Hidden or destroyed webviews must not stop monitoring. API and operating-system constraints are distinguished from proposed product behavior throughout this report.

### 1.1 At a glance

- **Primary interaction:** Hover for a temporary preview; left-click for the Overview popup; left-double-click for the standard dashboard window; right-click for the shared native menu.
- **Default mode:** Tray-only; retain the tray as a recovery/control surface in every mode. Select at most one additional Floating, Edge, or HUD shell.
- **Tray hover:** A two-row box, approximately 260 × 76 logical pixels, shows quota percentages and the next expected reset; click opens full detail.
- **Appearance:** Follow Windows by default, with persistent Light and Dark overrides across all four modes. Windows high contrast takes precedence.
- **Information levels:** Glance → Preview → Full Dashboard.
- **Refresh:** One shared scheduler: 120 seconds by default for expanded detail, compact shells, and tray-only background operation; configurable bounds and exceptions are in Section 7.5.
- **Reminders:** Off until enabled; configurable per quota window, with quiet hours, snooze, durable deduplication, and a local reminder history.
- **Lifecycle:** Closing a window keeps monitoring in the tray; explicit Quit stops monitoring and reminders. Launch at sign-in is opt-in.
- **Critical safeguard:** Mouse movement from trigger to popup must not cause accidental dismissal.

## 2. Objectives and scope

### 2.1 Goals

1. Show remaining usage in both the five-hour and weekly quota windows with unambiguous labels.
2. Allow the user to reveal additional information without invoking a full application window.
3. Support several Windows desktop placements without duplicating quota-fetching logic.
4. Provide a visually refined interface using responsive layout, restrained motion, and clear typographic hierarchy.
5. Remain reliable when the Codex service is disconnected, a limit is absent, or the user works across multiple monitors.
6. Give timely, configurable reminders without duplicate alerts or claims based on stale data.
7. Provide keyboard and touch routes, predictable focus, and consistent settings across all modes.

### 2.2 Non-goals for the first release

The initial version does not redeem reset credits, embed a live quota chip in the Windows taskbar, estimate remaining tokens or hours of usage from percentages, or display long-term usage analytics. Historical charts require a separately designed sampling and retention policy. A short local reminder history is in scope. The API's consumed percentage is **not** a precise count of remaining requests, tokens, or usable hours. Taskbar compatibility and the reference image's excluded chip are explained in Section 4.6.

The first release monitors one local Codex account context and the default Codex limit family. It does not provide account switching or guaranteed alerts while the application is quit, Windows is asleep, or the user is signed out. It runs as a normal per-user desktop process, without a Windows service, wake timer, or scheduled task. Other limit families must be identified as outside the selected scope rather than aggregated into one percentage.

## 3. Information architecture

The user interface has three information levels. Each level should add useful information rather than simply enlarge the same content.

| Level | Trigger and persistence | Content | Purpose |
|---|---|---|---|
| **L1 — Glance** | Visible in the idle/compact mode | Main remaining percentage, optional second quota summary, status indicator | Immediate awareness |
| **L2 — Preview** | Hover in Edge/HUD; tray hover | Tray: compact quota summary and next expected reset, with stale/unavailable status when needed. Other modes share the same quota bars, countdowns, optional reset credits, freshness and reminder status | Rapid inspection without context switching |
| **L3 — Dashboard** | Click any widget or tray icon; choose Details in a desktop preview | Detailed quotas, exact reset times, optional credits and plan, connection state, reminders, settings | Full inspection and control |

The quota interface must consistently distinguish **used** from **remaining**. If the service reports `usedPercent = 26`, the primary user-facing value is **74% remaining**, using `remainingPercent = 100 - usedPercent`.

When available, `windowDurationMins` should be used to label periods instead of inferring them from the `primary` and `secondary` object positions. A 300-minute window corresponds to five hours; 10,080 minutes correspond to seven days.

Use **5-hour**, **Weekly**, **N-minute/hour/day**, or **Primary/Secondary window (duration unavailable)** as appropriate. Never label an unknown duration as five hours. The Screen Edge tab stacks the five-hour and weekly remaining values; unavailable values remain labeled unavailable and stale values retain an out-of-date marker. This summary is not an account-wide guarantee of availability. A fresh explicit `ordinaryUsageAllowed = false`, where supported, takes precedence over an optimistic percentage; an unknown flag does not mean usage is allowed.

## 4. Layout modes

### 4.1 Floating widget

A small, frameless, draggable card appears at a user-selected desktop position and can optionally remain always on top.

**Idle:** A compact branded mini-card with two remaining-quota bars, independent optional reset countdowns, known reset credits and concise connection status. Its header provides Settings and a close control that saves System Tray mode; those controls are excluded from drag and Overview activation. Disable close-to-tray when no recovery tray exists. The native menu retains temporary Hide/show.
**Hover:** No expanded view. Preserve the existing compact card and open detail only on explicit activation.
**Click:** Opens the dashboard as an anchored popup; Open in window promotes it to a standard dashboard window.
**Dismiss:** Outside click or Escape closes a popup; a standard dashboard uses its Close control. Floating does not create a hover-only preview.

Size the compact card to its contents. The entire compact card is the drag target; moving at least 6 logical pixels starts a drag and cancels click/hover activation. Left-click opens the Overview popup after Windows' double-click interval expires. A double-click cancels that pending popup and opens the standard dashboard window directly at Overview. Right-click opens the shared native widget menu. Keep the shared Lock position setting. Snap near work-area edges without trapping the user there.

For Floating, evaluate above, below, right and left popup positions with an 8-logical-pixel gap. Clamp each to the monitor work area and choose the least overlap, using that order for ties. Keep the popup above its trigger when overlap cannot be avoided; apply the same ordering to Settings. Explicit detail follows the widget's topmost preference while it is a popup; promotion returns to a normal standard window.

### 4.2 Screen-edge widget

A compact tab is anchored to the left or right screen edge and slides outward when the user hovers over it.

**Idle:** Retain the application icon and stack the existing 44px percentage ring, optional period label or an internal circular badge, readable reset countdown and segmented time-remaining indicator for each window, separated by a subtle divider. Missing values are shown as unavailable; stale values retain a status marker.
**Hover:** The same read-only preview used in HUD slides into view from the edge, with the shared dwell and leave timing. Compact quota cards also show percentage used and the exact expected reset date/time with timezone, matching dashboard details. These details do not depend on the collapsed-widget countdown switches.
**Drag:** Move freely in both axes, including across monitors. On release, use the monitor under the pointer (nearest available monitor if outside the displays), dock to the left or right side nearest the final widget center, and clamp the vertical position within the work area. An exact center tie retains the previous side. Persist monitor, side and logical position as one settings revision; a failed save restores the previous saved dock. An unchanged or cancelled drag retains its prior placement. The shared Lock position setting disables movement.
**Click:** Left-click opens or raises the Overview popup beside the tab, using the same selected-side anchoring and work-area clamping as hover. Left-double-click opens the standard dashboard window at Overview. Right-click opens the shared native widget menu.
**Dismiss:** The preview closes after the pointer leaves the trigger and preview for the shared grace period. The dashboard follows the normal popup or window rules.

The entire exposed tab is a drag and activation target and must have at least a 32 × 48 logical-pixel hit area. Keep an 8-logical-pixel gap between the tab and its hover preview, matching HUD. The logical pointer corridor preserves travel across that gap; it owns no native window. Clicked Edge detail retains its adjacent selected-side anchor. Anchor within the chosen monitor's work area, leaving an 8-pixel inset from a taskbar auto-hide activation edge. Always on top is a separate preference. Do not add an Edge-only pin setting; the saved side controls docking and detail anchoring after release. Edge and HUD omit inline Settings controls; their context menu and tray remain the Settings routes.

### 4.3 System-tray mode

An icon resides in the **Windows notification area** (system tray), normally at the taskbar's right side. The app does not assume it can insert an icon at an arbitrary taskbar position.

**Hover:** After 200 ms over the icon, show a compact read-only box without taking focus. Enable this view by default when reliable tray enter/leave events and geometry are available.
**Click:** A single left-button release closes the hover box and schedules Overview after Windows' double-click interval. A double-click cancels that pending popup and opens the standard dashboard window directly at Overview. A trailing button-up must not schedule another popup. The explicit Settings menu command still opens Settings. Disable the tray menu's left-click behavior so it does not compete with this action.
**Mouse leave:** Dismiss the hover box 350 ms after the pointer leaves both the icon and box, including their connecting corridor. Entering either cancels dismissal. A clicked dashboard remains open under the normal popup rules.  
**Right-click:** Opens the shared native context menu: Open dashboard, Refresh, Display mode, Hide/show widget, Reset position, Snooze reminders, Resume reminders, Settings, and Quit. The same menu opens from desktop widgets and their previews. Hide/show and Reset position are disabled in tray-only mode.

The hover box targets **260 × 76 logical pixels**, with 12-pixel horizontal padding, 13-pixel text, and two compact rows at default text size. It uses the light/dark tokens in Section 6.4. This is a baseline size, not a clipping constraint: wrap and increase height for text scaling, localization, or longer actual period labels. Do not shrink text or truncate a percentage or status to preserve the baseline dimensions.

```text
5-hour 68% left · Weekly 42% left
Next reset: 5-hour in 2h 17m
```

The first row names the available quota windows and their remaining percentages; an expected but unavailable window is labeled **unavailable**, never zero. The second row identifies the earliest known future reset among fresh windows, with ties preferring the shorter period. Omit the reset row when no valid reset time exists. A due reset shows **Reset due — unconfirmed** until refreshed. Stale/offline status takes priority over reset forecasts: label retained values **Last known** and use the second row for **Out of date** or **Offline**. With no usable values, show **Usage unavailable** and the applicable connection status. A fresh explicit usage-blocked flag takes priority over reset text and must remain visible even when percentages look positive.

The box contains no settings, buttons, credit details, full timestamps, or detailed progress bars. These belong in the clicked dashboard. Its read-only role keeps it substantially smaller than that dashboard and requires no keyboard focus. Keyboard and touch users open the dashboard through normal tray activation or its menu.

Anchor the box near the tray icon, normally above a bottom taskbar with an 8-pixel gap; treat that gap as a logical hover corridor, without creating an invisible input-blocking window. Flip and clamp to the available monitor bounds for other taskbar positions and overflow geometry. Apply the same monitor-aware placement to the clicked popup. Suppress the hover box while an app dashboard or native menu is open, and require a fresh pointer entry after dismissal before reopening it. Fullscreen and lock suppression apply as specified for other automatic previews.

When custom hover events or positioning are unreliable, use a native tooltip with the same compact summary instead. Select one hover presentation at a time: suppress the native tooltip when the custom box is enabled; if that cannot be done reliably on a tested build, use tooltip-only behavior. Windows controls tooltip styling, size, and timing. Failure to position a hover box must never open a full dashboard automatically; only explicit activation may fall back to a clamped standard dashboard window.

Windows controls tray overflow and icon visibility; explain how to find the icon without promising it will always be visible. Validate hover behavior after Explorer restart, with auto-hide, in overflow, and at mixed DPI. Tray events and menu behavior use [Tauri's system tray APIs](https://v2.tauri.app/learn/system-tray/).

### 4.4 Compact HUD / top-bar mode

A compact horizontal indicator retains the application icon and groups each period as label/value above its quota bar, beside countdown text above segmented time-remaining indicators. Preserve the existing container, corners and surface effects, with tighter internal spacing. Use the stable layout sizing policy in Section 4.7. A shared Words / Badges preference replaces 5H / WEEK with circular 5 / W badges beside HUD percentages. Hover shows percentage used and exact expected reset date/time with timezone in a preview horizontally centered on the strip, flipping above/below and clamping to the monitor work area; left-click opens a similarly centered Overview popup; left-double-click opens the standard dashboard window; and right-click opens the shared native widget menu. The entire compact strip is draggable and shares Floating mode's position lock. Automatic previews and desktop shells are hidden over a detected fullscreen application on their monitor by default; explicit user-invoked detail remains available. Do not implement an invisible click-through overlay that leaves no reliable activation target.

### 4.5 Mode comparison

| Mode | Collapsed form | Hover | Activation and menu | Default dismissal |
|---|---|---|---|---|
| Floating | Compact card; entire card draggable | None | Left-click: Overview popup; double-click: standard window; right-click: shared menu | Outside click or Escape |
| Screen edge | Edge tab with five-hour and weekly values; vertical drag along selected edge | Shared desktop preview, anchored to the edge | Left-click: Overview popup; double-click: standard window; right-click: shared menu | Shared hover exit delay / dashboard outside click or Escape |
| System tray | Notification icon | Two-row read-only box (~260 × 76); tooltip fallback | Left-click: Overview popup; double-click: standard window; right-click: shared menu | Hover: 350 ms exit grace; popup: outside click or Escape |
| HUD | Thin quota strip; entire strip draggable | Drop-down preview | Left-click: Overview popup; double-click: standard window; right-click: shared menu | Hover exit delay / click outside or Escape |

### 4.6 Taskbar and standard dashboard

The reference board's **Task Bar Chip Mode** is excluded from this Windows 10/11 design. Microsoft documents that Windows 11 apps can no longer customize taskbar areas; Windows 10's legacy desk-band capability does not establish a supported cross-version inline chip. Retain the existing tray and HUD modes without adding a taskbar-adjacent substitute, a Windows 10-only chip, or shell modifications. Sources: [Windows 11 feature deprecations](https://www.microsoft.com/en-us/windows/windows-11-specifications), [Microsoft's Windows 10 desk-band distinction](https://blogs.windows.com/windows-insider/2025/06/02/updates-to-windows-for-the-digital-markets-act/).

Compact shells, previews, and popup dashboards do not create taskbar buttons or Alt+Tab entries. Open in window creates one resizable standard dashboard with a normal taskbar button and Alt+Tab entry, and closes the popup. Its Close button hides it to the tray; minimize uses normal Windows behavior. Overview and Settings reuse the same dashboard components in both popup and standard presentations. Navigating to Settings keeps the current popup presentation and anchor; promotion requires Open in window or double-click. Do not auto-pin the application to the taskbar or embed a live quota strip there. A user-pinned launcher reopens the existing instance. Keep only one L3 presentation open at a time, including Settings, so edits cannot compete across duplicate dashboards.

Overview omits introductory marketing copy. Each quota card directly shows remaining percentage, used percentage, relative expected reset and the exact local reset timestamp with numeric timezone offset. A small clock icon identifies reset metadata. Read timing remains directly visible. Reset-credit details use separated icon-led status/expiry rows; show the first three in returned order, with Show more (N) / Show less for the remainder. Expansion is temporary view state and never changes the reported available count. A compact reminder/snooze summary links to Settings → Reminders, preserving management actions and history there.

The standard dashboard prefers a 620-logical-pixel client width; the single-click popup prefers 480. Fit natural Overview content and chrome up to 85% of the relevant monitor’s work-area height, including the native frame. Continue fitting meaningful Overview content changes until the user resizes or maximizes a standard window; moving does not disable fitting. Remember manual standard dimensions for the session, independently of the single-click popup. Settings keeps the current dimensions and scrolls; direct Settings opens use capped initial defaults. Countdown-only measurement changes do not trigger fitting. Enlarged text or oversized content scrolls at the cap. Both presentations use the same responsive quota cards and hierarchy.

Native property changes run in order on the owning UI thread and keep Tauri/Tao's cached topmost/focusable state coherent with the HWND. Verify actual flags and coalesce own-window style/Z-order mismatch repairs without activation. One-time reveal remains distinct from persistent topmost. Attempt foreground activation once for explicit detail. If Windows denies it, expose a standard taskbar-accessible dashboard, request attention once and retain tray recovery; do not enable tooltip fallback or display a coordination-failure warning for an ordinary foreground restriction. See [Windows foreground rules](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow).

### 4.7 Optional compact reset countdowns and missing data

Display settings provide independent five-hour and weekly reset switches for Floating, Screen Edge, and HUD. Defaults are off. Each visible shell calculates expected reset time from the most recently observed deadline between shared usage checks. Use hours and minutes below one day, and days plus hours for longer durations. Round positive durations up to the next minute before decomposition. Examples: `3h10m`, `45m`, `6d7h`. HUD and Edge add configurable segments representing remaining time divided by the reported window duration, clamped to 0–100%; missing timestamps/durations stay unavailable. Choose an integer count from 2–12 independently for Edge five-hour, Edge weekly, HUD five-hour and HUD weekly; all default to 10, including migrated settings. Counts apply on Save and Cancel restores saved values. Hidden periods or disabled countdowns retain their counts while disabling the selector. Keep the strip width at 42 logical px for Edge and 48 for HUD at default text size, dividing it into equal-width bars without resizing either shell. Positive partial segments round up; elapsed forecasts show zero lit segments with the unconfirmed label. Units use compact lowercase text. An elapsed deadline says `due?`, and retained forecasts carry a last-known marker; elapsed time never proves quota recovery.

Floating and preview dimensions follow rendered contents. Edge and HUD use predetermined scalable client dimensions for each saved layout, independent of changing percentages, countdown strings and data availability. Independent Edge size and HUD size settings accept whole percentages from 80% to 300%, defaulting to 100%. Uniformly scale the whole shell, including text, graphics, spacing, borders and corners; keep the current layout and combine this with Windows text/DPI scaling. Floating, previews, tray summaries and dashboard keep their sizes. Size edits use the existing temporary display-preview session; Save persists and Cancel restores. Shared Bold countdown text and Higher contrast countdown text switches default Off and affect only Edge/HUD countdowns across themes. Bold uses weight 700; stronger contrast uses the theme main-text color, with Windows system colors retaining precedence. Only a visible warning adds space; do not reserve an empty warning slot. Label/countdown settings and accessibility scaling can recalculate size. Keep minimum pointer targets and clamp native bounds to the work area. Widget-anchored popups re-anchor when the shell size or placement changes.

At default text size, with n enabled countdowns: Edge is 54px wide; height is 190 + 24n px for Words or 150 + 24n px for Badges. Single-period base heights are 120px for Words and 100px for Badges. A warning adds 18px height. HUD is 204 + 58n px wide for Words or 168 + 58n px for Badges, and 38px tall; a warning adds 28px width, including an extra 8px gap before the warning icon. HUD uses 6px left, 10px right and 4px top/bottom padding; Edge uses 2px left/right, 6px top and 16px bottom padding. Edge's quota stack sits 2.5px lower to preserve the selected gap below the logo; the 4px height increase supplies extra room below the last segmented indicator. Edge uses a 1px ring-to-reset gap when a countdown is enabled and 5px spacing above and below the middle divider. HUD's value column accommodates 99.9%. Both modes retain two quota positions when data is absent, without inventing period durations. The shared label preference defaults to Words; badges use only the reference image's badge concept and retain all current theme/surface styling. Edge uses the selected A · Subtle reference profile: ring containers are 46px wide with 11.5px percentage text; centered reset strips are 42px wide. Its centered time-only countdown uses 11px text, 1.1 line height and −.02em letter spacing to fit retained ~23h59m without clipping; HUD keeps 11px countdown text and a small clock icon. Internal Edge badges are 12px and sit 4px above their centered layout position; HUD badges remain 14px. Badges use neutral secondary text and divider borders; known Edge percentages use the corresponding blue or purple quota color, while unavailable values stay neutral.

Rust determines fitting size limits for each selected monitor using work area, native frame insets, Windows text/DPI scaling, snap gap, taskbar safety, and space for a visible warning. Reject new sizes above the fitting maximum and retain the last valid preview. When an environment change makes a saved enlargement stop fitting, temporarily display at 100%, retain its requested value, and restore it when it fits again. Never enlarge a saved value below 100% as a fallback. If even that fallback cannot fit, retain the existing work-area constraints and explain the limitation in Settings. Snap gaps, popup gaps and drag thresholds remain at their existing logical sizes; popups re-anchor to the scaled shell.

Keep the Edge warning icon clear of quota graphics and their retained-data outlines. Its placement uses a 10px downward offset within the existing bottom padding, preserving the shell dimensions and the quota stack's selected spacing.

Use tight padding and gaps without reducing quota ring or text sizes. Compact shells and hover previews have rounded native boundaries matching their CSS corners, with opaque readable interiors and no rectangular outer frame. Native placement treats frontend measurements as logical client dimensions and accounts for any actual frame insets before work-area clamping.

Hiding a widget is temporary. Explicitly selecting any desktop mode from the tray, including the current mode, or choosing Show widget reveals and raises it once without taking keyboard focus. Applying a different desktop mode in Settings does the same. Unrelated settings saves and automatic updates preserve hidden state and window order. Keep the Always on top preference unchanged: when enabled, widgets remain above newly opened ordinary windows; when disabled, later window activation can cover them. Lock/fullscreen suppression still applies and cancels a pending raise.

For a missing or invalid quota field in the same account and mapping, retain the validated value for two unsuccessful observations and mark it Last known. The third consecutive missing observation displays unavailable; a validated observation resets that counter. With no prior data, show Checking during the first two checks. Transport failures retain stale values. Authentication, unsupported configurations, or account changes remain immediate diagnostics and never retain data across an account boundary.

## 5. Interaction specification

### 5.1 State model

Use explicit interaction states rather than a single `isExpanded` boolean. Rust owns a window coordinator; React renders its decisions:

```text
Shell state:   COMPACT -> PREVIEW
                         |
                  leave + grace
                         +-----> COMPACT

Detail state:  CLOSED -> POPUP -> WINDOW
                          |         |
                    outside/Esc   Close
                          +-----> CLOSED

Left-click widget or tray: open/raise POPUP (or focus existing WINDOW), at Overview
Left-double-click widget or tray: open/raise WINDOW at Overview
Right-click widget, preview, or tray: close transient preview and open the shared native menu
Explicit Settings command: open WINDOW at Settings
Open in window: POPUP -> WINDOW, retaining the current page
```

Only one transient preview may exist. Opening L3 closes PREVIEW and suppresses hover reopening until L3 closes and the pointer leaves the trigger. Opening L3 never starts another data client. Keep interaction state separate from connection status, freshness, and reminder enablement.

### 5.2 Event behavior

| Event | Hover preview | Dashboard |
|---|---|---|
| Pointer enters trigger | Open after short dwell, if configured | No change |
| Pointer leaves trigger | Schedule dismissal, unless moving into preview | No change |
| Pointer enters preview | Cancel pending dismissal | No change |
| Left-click trigger or tray icon | Cancel all hover timers | Open or raise the Overview popup at Overview and focus it |
| Left-double-click trigger or tray icon | Cancel all hover timers | Open or raise the standard dashboard window at Overview |
| Right-click widget, preview, or tray icon | Close preview; cancel dwell and leave timers | Open the shared native menu at the pointer; keep a standard dashboard open |
| Escape | Close only when received by the app | Close topmost dialog/menu first, then popup; standard dashboard remains open |
| Outside click | Close | Close popup; standard dashboard remains open |
| Window loses focus | Pointer rules govern the unfocused preview | Close popup only after focus leaves the entire owned window/menu/dialog group |
| Menu closes | Require fresh pointer entry before preview can reopen | Restore normal pointer behavior |

A hover preview is informational. Floating/Edge/HUD previews can provide Details, which explicitly transfers focus to L3; the compact tray preview is read-only and contains no focusable controls. Clicking the tray icon transitions directly from its small preview to L3. Touch and keyboard activation use the same click actions. Escape is not a global shortcut: an unfocused preview must not intercept another application's keys.

Treat trigger and preview bounds, including their connecting corridor, as one hover region. Cancel collapse on entry to either; native pointer events must drive the decision across window boundaries. During menu/dialog interaction, dragging, or a mode change, cancel pending hover tasks. Associate timers with a view-generation token so old callbacks cannot hide newly opened windows. Repeated clicks keep the dashboard open and raise it; a drag release is never a click. The whole compact shell is draggable in Floating and HUD; Edge can move freely while dragging and docks to the nearest side on the release monitor. Position lock disables drag and keyboard nudging without disabling activation or the context menu.

Mode changes save the outgoing placement, close transient views, and create the incoming compact shell; a standard dashboard stays open. Do not persist transient preview state across restart. Screen locking hides desktop surfaces without changing the selected mode or stopping the Rust service.

### 5.3 Motion and timing guidelines

The following are initial design targets to validate in usability tests, rather than requirements imposed by Tauri:

| Parameter | Proposed target | Rationale |
|---|---:|---|
| Hover activation | Tray, Floating, Edge, and HUD: 200 ms | Avoid activation from fleeting pointer movement; native tooltip timing is OS-controlled |
| Expand animation | 150–200 ms | Responsive, subtle transition |
| Collapse grace period | Tray: 350 ms; other previews: 350 ms (tune within 250–500 ms) | Allows travel from trigger to card |
| Re-enter during grace period | Cancel collapse immediately | Prevent flicker |
| Countdown tick | 1 second only for visible second-resolution text; otherwise 1 minute | No per-second IPC, network traffic, or hidden-webview timer |
| Quota refresh | 120 seconds by default for all visibility states | One scheduler; see Section 7.5 |

Support reduced-motion settings by disabling or minimizing translation and scale transitions. Never require a hover interaction as the only means of accessing the data.

### 5.4 Reminder rules and defaults

Reminders are opt-in. Enabling the master switch offers low-usage presets and a separate Test notification button. All numeric defaults below are product choices, not service guarantees. Threshold rules bind to an account context, limit family, and stable window key, never to the currently visible shell or a translated label.

| Rule | Default after reminders are enabled | Trigger and message |
|---|---|---|
| Low remaining usage | Enabled at 20%, 10%, and 0% for each supported window | A fresh valid sample reaches or crosses a configured remaining threshold; name the period and remaining amount |
| Reset approaching | Off; lead time 5 minutes, configurable 1–60 minutes | Enter the lead-time interval for a future known reset, while that window is at/below its highest enabled usage threshold (20% fallback); say **Expected reset in…** |
| Quota recovered | Enabled for windows that entered a low-usage episode while monitored | Confirm recovery using Section 5.5; name the recovered window and report any other limiting window |
| Scheduled check-in | Off | One-time date/time or recurring selected weekdays and local time; remind the user to check Codex usage |

Allow up to five unique whole-number remaining thresholds from 0–99 per window, sorted from high to low; show the corresponding used percentage in Settings to prevent inversion. Evaluate unrounded values, not display text. A value between 0% and 1% displays **<1% remaining**, not **0%**; only a validated zero permits the zero-threshold alert. Thresholds and reminder categories are independent of visual severity colors.

On first enablement or creation of a rule, establish a baseline from a fresh sample. If already low, send at most one **Currently low** summary at the most severe satisfied threshold; consume the less severe thresholds for that episode. Editing an existing threshold rebases it without an immediate alert: mark newly satisfied levels as handled and retain prior suppression history. Crossing several thresholds between samples also produces one alert, consuming all crossed levels. Repeated polls, percentage rounding, view changes, and refresh clicks never create another event for the same threshold and episode.

Scheduled reminders are allowed without quota data. Include percentages only after a successful refresh within the last 60 seconds; otherwise send a neutral reminder with **Usage data unavailable/out of date**. Schedule evaluation must not wait indefinitely for the network. Support a maximum of 20 schedules, each with a stable ID, revision, enabled flag, label, local date/time or weekday set, and time-zone policy. Default recurring schedules follow the Windows time zone; optionally allow a fixed IANA time zone. Store one-time due instants in UTC and show their local equivalent.

For daylight-saving gaps, run a recurring occurrence at the first valid time after the gap; during a repeated hour, run only at the first occurrence. Deduplicate by schedule ID/revision plus intended local date and time, not UTC alone. Zone changes recompute future recurring occurrences without replaying an already handled local occurrence. Clock changes never replay consumed occurrences. Edits cancel pending occurrences of the previous revision; past-due one-time entries require the user to choose a future time.

### 5.5 Reset semantics and episode identity

A countdown reaching zero is a prompt to refresh, **not proof of restored quota**. Display **Reset due — checking**; never change remaining usage to 100% locally. An approaching-reset reminder is a forecast. A recovery reminder requires a fresh service observation and uses **Quota increased** or **Usage available again** only when the corresponding evidence exists.

Maintain a persisted episode ID per account/limit/window. Keep a proposed reset deadline separately; changing a timestamp or duration does not by itself create an episode. For a known deadline, confirm a new cycle only when a fresh sample received after the prior deadline has a later future deadline and a higher remaining percentage. An increase after a credit redemption or rolling-window recovery can also end a low episode: require remaining usage at or above `min(100, highest configured threshold + 5)` on two successful reads at least 30 seconds apart; use 20% as the threshold when none is configured. This sustained-recovery rule also covers missing reset timestamps; an authoritative reset identifier, if a validated API version supplies one, may confirm a cycle directly. If evidence is ambiguous, keep the episode and suppress repeat alerts. Do not attribute an increase to credit redemption unless the service explicitly supplies that cause.

Create at most one recovery event for a previously low episode, then rearm thresholds for the new episode. A brief upward correction followed by another threshold crossing in the same episode does not rearm them. With no earlier low observation, a new cycle silently establishes a baseline. A fresh explicit availability change from false to true can support **Usage available again**; a single window's reset cannot establish account-wide availability, especially if another window is exhausted or unavailable. Combine simultaneous window recoveries and the availability transition into one summary.

On deadline correction, reschedule the pending approaching-reset event. Once submitted, do not send another approaching event in that episode. A missed approaching reminder expires at its deadline. On restart, reuse the persisted episode only after matching the account and validating new data; missing windows suspend their rules without erasing history. If identity or window mapping changes, establish a new baseline without fabricating a reset.

### 5.6 Notification preferences, snooze, and suppression

Settings provides a master switch, per-category/window switches, thresholds, schedules, quiet hours, sound preference, privacy preference, and delivery diagnostics. Default to silent notifications with generic content that omits account identifiers; detailed quota text is opt-in. Quiet hours default off and use a local start/end time with weekday selection; an interval spanning midnight belongs to its start day. Reject identical start/end times and offer Pause reminders for an all-day pause.

Provide **Snooze all for 15 minutes / 1 hour / Until tomorrow 09:00**, plus **Resume now**, in the tray and dashboard. Compute tomorrow in local calendar time. A reminder-history item also offers Snooze this rule with the same choices. Persist deadlines and show **Snoozed until…** in all relevant views. Snooze suppresses newly eligible events; it does not replay an already submitted event when it ends. Thresholds reached during snooze remain eligible for one current-state summary on resumption.

Evaluate delivery in this order: enabled rule and master switch → valid event and identity → quiet hours/snooze/session lock/fullscreen preference → coalescing and cooldown → native permission/transport → durable submission. Disablement cancels pending events, preserving consumed history; re-enablement does not replay old events. If delivery is blocked by a known OS denial, retain history with a reason and offer Windows notification settings and Test notification; do not repeatedly request permission. A permission grant alone does not replay the backlog.

App-controlled suppression retains at most one pending candidate per rule/window. When suppression ends, request a shared refresh, discard obsolete candidates, and deliver at most one digest: current low-usage conditions, still-future resets within their lead interval, recovery observed within 15 minutes, and scheduled occurrences missed by at most 15 minutes. Later scheduled occurrences are recorded as missed; never replay every missed occurrence. If quota refresh fails, leave quota candidates pending for normal recovery; neutral scheduled reminders can still be delivered within their grace period. Sleep, restart, and reconnect use the same catch-up rules. A pending low-usage event expires when its episode ends or its condition ceases to apply; a recovery candidate expires 15 minutes after observation.

Coalesce eligible events over 5 seconds. Permit at most one automatic desktop notification per 60 seconds; higher-severity/newer state replaces a queued lower-severity candidate and all merged event IDs are consumed together. Snooze, quiet hours, and OS suppression are never bypassed by a zero-percent alert. Do not show a competing overlay to work around Windows Do Not Disturb or Focus Assist; Windows controls whether a toast appears as a banner or in Notification Center. Submission success is not proof the user saw it. See [Windows notification preferences](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows).

### 5.7 Durable deduplication and Windows delivery

Persist an event ledger keyed by account context, limit/window, episode ID, rule ID, and threshold/category; scheduled events use the occurrence key above. Store eligibility time, expiry, suppression reason, delivery-attempt status, and source snapshot revision. Serialize evaluation, settings changes, and submission in the Rust service. Never deduplicate by notification text or a frontend boolean.

Before handing a notification to Windows, atomically persist a claimed delivery attempt and consume its event keys. Then record **Submitted to Windows**, **Failed**, or **Outcome unknown**. A crash between claiming and recording the result must not automatically resend: the design favors avoiding duplicates over guaranteed delivery. Retry automatically only when the transport definitively confirms that submission never occurred, with at most two retries at 30 and 120 seconds, revalidating policy and data each time. A retry advances the existing event's attempt state; it never recreates its trigger or clears consumed keys. Ambiguous results remain in history for inspection. Persistence failure pauses desktop delivery and exposes an error; it must not silently discard deduplication state.

The baseline implementation sends ordinary notifications from Rust through a desktop notification adapter. Tauri documents an installed-app requirement on Windows, so validate identity/icon/permission behavior in the actual installer build. Its notification guide and current API reference differ in desktop action coverage; the reference limits Windows click reporting to the visible toast. Do not promise Notification Center activation, custom action buttons, or launch-after-quit through the generic plugin. Snooze and Details remain available through tray/dashboard controls in every build. Add toast actions only after the selected version passes packaged Windows tests; otherwise use text directing the user to the tray. Sources: [Tauri notification guide](https://v2.tauri.app/plugin/notification/), [notification API](https://v2.tauri.app/reference/javascript/notification/).

Scheduling runs in Rust while the app is running. Windows has a separate scheduled-toast API, but it has its own delivery limits and is not assumed to be implemented by the selected Tauri plugin. Delivery after Quit is deferred; Settings must plainly state **Reminders require the app to be running and Windows awake**. See [Windows scheduled notifications](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-scheduled).

## 6. Visual presentation

### 6.1 Design direction

The visual-polish increment uses an original generated cyan/cobalt ring across app-rendered and native identity surfaces, with a restrained generated illustration for welcome/empty states. Critical labels, control icons, quota bars and rings remain scalable HTML/SVG. Floating uses the richer mini-card; Edge stacks percentage rings and HUD uses inline quota bars. Settings uses mode thumbnails, grouped controls and a persistent save bar. See [asset provenance and validation](layout-polish-implementation-report.md).

Use restrained surfaces, soft shadows, rounded corners, clear typography, and minimal decorative movement. The supplied private design board (not distributed with this repository) informs the compact proportions, blue accents, card hierarchy, and spacing. Its Floating, Edge, Tray, and HUD concepts map to the four supported modes; its taskbar chip is excluded under Section 4.6. The board's decorative backgrounds and differing example values are visual references, not application assets or live data.

Light and dark presentations share geometry, typography, and information hierarchy. Use Segoe UI Variable with Segoe UI and system sans-serif fallbacks; reserve smaller 13-pixel text for compact summaries and use 14-pixel body text in detail views. Use 12-pixel corner radii, 12–16-pixel card padding, and a 4-pixel spacing grid. Keep shadows subtle and avoid glow behind small text. Translucency is optional; opaque surfaces are the baseline for legibility and the fallback when transparency is disabled.

Progress bars consistently represent **remaining** usage; label used percentages separately in detail. Status text and icons supplement color. Do not copy the board's `3h 24m / 5h` or `37h of 50h` usage estimates: a quota percentage and window duration do not establish remaining working time. Time labels describe the quota period or expected reset only.

### 6.2 Proposed information card

```text
╭────────────────────────────────╮
│ Codex Usage                ↗ ⚙ │
│                                │
│ 5-hour quota        100% left  │
│ ██████████████████████████████ │
│ Resets in 03h 42m              │
│                                │
│ Weekly quota        74% left   │
│ ██████████████████████░░░░░░░░ │
│ Resets in 5d 12h               │
│                                │
│ Reset credits: 3 available     │
│ Reminders: On                  │
│ Updated 18 seconds ago      ↻  │
╰────────────────────────────────╯
```

The card should indicate unavailable or stale values rather than displaying `0%` as a placeholder. A disconnected state preserves the most recent valid values for the verified account with an explicit **Last successful refresh** timestamp and Retry. A successful transport call with invalid quota fields does not make those fields fresh. Distinguish **Connecting**, **Sign-in required**, **Unsupported account/version**, **Offline**, and **Refresh failed**. Past reset dates show **Reset due — unconfirmed**, not a negative countdown or a fresh quota. Cached values from an unverified account remain hidden.

HUD and Edge independently offer Both (default), 5-hour only or Weekly only for their compact shells. Hide the entire unselected period, including its enabled reset countdown and accessible reading. Keep reset preferences saved while hidden. Hover previews, Overview, tray summaries, polling and reminder rules retain all periods. Single-period shells shrink along their existing axis but keep a stable footprint while readings change.

### 6.3 Dashboard content

The full dashboard adds exact reset dates/times and time-zone labels, used and remaining percentages, optional plan and credit details, connection state, last attempt/success times, next retry, and a reminder history. Refresh shows one shared in-progress state and cannot enqueue duplicate requests. Credit expiry alone does not change the reported available count locally; refresh and mark outdated information. Reset credits are informational and clearly distinguished from a monetary/API balance.

Settings uses four pages: **Display** (mode, placement, hover, theme, topmost, fullscreen behavior); **Reminders** (rules, schedules, quiet hours, snooze, privacy, sound, test); **General** (launch at sign-in, refresh intervals, background behavior); **Connection & diagnostics** (Codex location/version, account support, errors, retry). Validate before Save and commit atomically; show failures instead of optimistic success. Cancel discards drafts. Closing or navigating away from dirty settings offers Save, Discard, or Cancel; mode changes preserve the draft. Reminder history shows trigger, period, observation time, and status/reason, with Clear history affecting display records only, never deduplication tombstones. An optional diagnostic export is user initiated and redacted.

### 6.4 Light, dark, and system appearance

Display settings offers **Follow Windows** (default), **Light**, and **Dark**. Follow Windows tracks the Windows **app** theme, which may differ from the taskbar's theme. Explicit Light/Dark overrides remain in effect until changed. Windows high contrast takes precedence over all three choices without overwriting the saved preference; leaving high contrast restores its resolved appearance.

Use these semantic tokens as the initial opaque palette. Reuse them across components rather than maintaining separate colors for each mode.

| Token / role | Light | Dark |
|---|---|---|
| Canvas | `#F3F6FB` | `#081426` |
| Card / preview surface | `#FFFFFF` | `#10213A` |
| Hover / selected surface | `#E8EFF8` | `#193452` |
| Primary text | `#12233F` | `#F4F7FC` |
| Secondary text | `#475569` | `#B8C7DC` |
| Decorative divider | `#CBD5E1` | `#334B6B` |
| Essential control border | `#64748B` | `#94A3B8` |
| Accent / 5-hour progress / focus ring | `#005FCC` | `#60A5FA` |
| Weekly progress | `#6D28D9` | `#A78BFA` |
| Progress track | `#E2E8F0` | `#253B57` |
| Healthy status | `#047857` | `#34D399` |
| Warning status | `#92400E` | `#FBBF24` |
| Critical / error status | `#B91C1C` | `#FCA5A5` |
| Card shadow | `0 4px 16px rgba(15, 23, 42, 0.12)` | `0 4px 16px rgba(0, 0, 0, 0.28)` |

Use period-specific colors for bars and labeled status icons/text for severity, so color never changes a quota's meaning. Status colors do not define notification thresholds. Decorative dividers cannot serve as the sole boundary of a required control. Focus uses a visible 2-pixel ring with a 2-pixel offset. Normal text must reach **4.5:1** contrast; essential control boundaries, focus indicators, and graphical values must reach **3:1** against adjacent colors. Validate actual rendered combinations, including hover/selected states and any optional transparency, before release.

| Surface | Light presentation | Dark presentation |
|---|---|---|
| Floating widget and preview | White card, dark labels, restrained blue/purple bars | Navy card, pale labels, bright blue/purple bars |
| Screen-edge tab and expanded card | Pale tab with dark quota text; white expanded card | Navy tab with pale quota text; matching expanded card |
| System-tray hover box and clicked popup | White compact box and detail card, dark text, subtle outline/shadow | Navy compact box and detail card, pale text, subtle outline/shadow |
| Compact HUD and drop-down | Pale strip with dark summaries; white preview | Navy strip with pale summaries; navy preview |
| Dashboard, settings, and state messages | Pale canvas and white sections; readable labeled status | Dark canvas and navy sections; readable labeled status |

Theme changes affect every app-rendered surface, including empty, loading, stale, offline, error, disabled, hover, and keyboard-focus states. Keep content and dimensions unchanged except when accessibility settings require reflow. Windows-owned context menus, native tooltips, notification surfaces, and tray backgrounds follow OS styling; an app override does not recolor them. Supply tray icon artwork that remains legible against the actual Windows notification-area background independently of the app theme.

In high contrast, use Windows system colors for surfaces, text, controls, and focus; suppress translucency, shadows, and decorative gradients. Retain labels and visible boundaries so all quota and connection states remain understandable. Disabled transparency uses the opaque tokens above without removing status information.

## 7. Technical design

### 7.1 Architecture

```text
React / TypeScript: compact shell, preview, dashboard/settings
          | commands                  ^ versioned snapshots
          +----------- Tauri IPC -----+
                         |
Rust application services (one authoritative state owner)
  ├─ window coordinator: tray, placement, focus, presentation state
  ├─ quota service: normalization, freshness, refresh scheduler
  ├─ reminder engine: rules, due times, suppression, deduplication
  ├─ Windows notification adapter
  ├─ persistence: settings, account-scoped cache, event ledger
  └─ process supervisor / Codex JSON-RPC client
                         | stdio JSONL
                  owned codex app-server
                         |
                account/rateLimits/read
```

Keep one owned app-server process per widget instance. Use `codex app-server` (default stdio), or the verified explicit form `codex app-server --listen stdio://`; `--stdio` is not the documented form. Send `initialize`, await success, then `initialized`, before `account/rateLimits/read`. The transport is newline-delimited JSON with request IDs. Treat `account/rateLimits/updated` as a refresh hint. Use versioned schemas as implementation and regression references; do not restrict runtime reads by CLI version or version format. Attempt the existing read-only sequence and determine compatibility from its operation. Required-method or malformed-response incompatibilities pause automatic reads and show best-effort `--version` display text (two-second timeout, 4 KiB cap, 200 sanitized characters), with CLI version unavailable as fallback. Version information never gates a read. Network, timeout, authentication and absent optional-data failures remain distinct. The reviewed documentation labels app-server experimental, so successful reads establish compatibility for the tested installation rather than a promise for every future release. These protocol facts come from the [official Codex app-server documentation](https://learn.chatgpt.com/docs/app-server).

Application requirements: spawn without a console using a validated executable path and argument array; pipe stdout/stdin and drain bounded stderr continuously. Do not start a thread, inference turn, or interactive terminal. Resolve the installed native executable rather than blindly launching a PowerShell/npm shim; if only an unsupported wrapper or WSL installation is found, show a path-selection/help state. Do not execute user-supplied shell command strings. Initialization and reads have a widget-side 15-second timeout. Limit each JSONL frame to 1 MiB; reject malformed or oversized frames with a redacted compatibility error. Correlate requests by ID and connection generation; discard late results from previous processes or account contexts.

Connection settings may supply an HTTP/HTTPS proxy URL without authentication to the owned Codex app-server process only. Blank preserves inherited network settings. Rust validates the address and sets child proxy variables without changing global proxy settings, Codex files, TLS trust or frontend permissions. Saving a changed proxy invalidates old reads and retries the connection. Proxy failures remain network diagnostics, separate from protocol incompatibility.

All frontend windows are subscribers. Commands include Get state, Refresh, Save settings(expected revision), Set mode, Snooze, and Quit. Rust serializes state changes, increments a process-epoch/revision pair, and broadcasts the resulting state. On mount, a view subscribes first, buffers events, obtains Get state, then applies only newer revisions; on a gap, reconnect, or resume, retrieve a full snapshot. A new process epoch replaces the old replica. Reject conflicting settings revisions, retain the local draft, and offer a reload/reapply comparison. No window may independently fetch quotas, mutate the ledger, or schedule notifications.

Extend the existing settings and presentation snapshots with the following appearance contract; do not add a separate settings store or quota client:

```ts
type ThemePreference = 'system' | 'light' | 'dark';

interface AppearanceSettings {
  themePreference: ThemePreference; // persisted; default 'system'
}

interface AppearanceState {
  themePreference: ThemePreference;
  resolvedTheme: 'light' | 'dark'; // Windows app theme when preference is 'system'
  highContrast: boolean;         // system-color rendering takes precedence
  transparencyAllowed: boolean; // false requires opaque surfaces
}
```

Rust observes Windows appearance/accessibility changes and broadcasts the resolved state using the existing revision mechanism. CSS semantic tokens render that state consistently in all webviews. A successful settings Save applies the theme immediately to every existing window without restart; Cancel discards a draft without changing the active theme. New or recreated windows obtain the current appearance before becoming visible to avoid a flash of the wrong theme. System-theme changes update views only when Follow Windows is selected; high contrast and transparency accessibility changes apply regardless of that selection. Reconcile on resume/unlock as well as native change events. Theme updates preserve placement, preview/popup state, settings drafts, quota data, and reminder state, and do not initiate quota reads.

On settings migration, preserve a recognized existing theme choice; use `system` when none is stored. Persist only the user's preference, not the observed Windows theme or accessibility state. Leaving high contrast recomputes the effective theme from the retained preference and current Windows app theme.

### 7.2 Data normalization

The frontend should consume a stable application-level model rather than relying directly on the raw Codex response shape:

```ts
interface QuotaWindow {
  key: string;             // adapter-defined stable key; never array index alone
  limitId: string;
  label: string;
  usedPercent: number | null;
  remainingPercent: number | null;
  durationMinutes: number | null;
  resetsAtMs: number | null; // normalized UTC epoch milliseconds
  validatedAtMs: number | null;
  quality: 'valid' | 'missing' | 'invalid';
  // Cached values, if displayed, retain their original validation timestamp.
}

interface UsageSnapshot {
  processEpoch: string;
  revision: number;
  accountContextKey: string | null; // opaque local key; not an access token
  connectionGeneration: number;
  selectedLimitId: string | null;
  windows: QuotaWindow[];
  resetCreditsAvailable: number | null;
  resetCredits: Array<{
    key: string;
    status: string;
    expiresAtMs: number | null;
  }> | null;
  planType: string | null;
  ordinaryUsageAllowed: boolean | null;
  lastAttemptAtMs: number | null;
  lastSuccessAtMs: number | null;
  nextRefreshAtMs: number | null;
  refreshInFlight: boolean;
  connection: 'starting' | 'ready' | 'offline' | 'auth-required'
    | 'unsupported' | 'error';
  freshness: 'empty' | 'fresh' | 'stale';
  errorCode: string | null; // safe diagnostic code, not raw server output
}
```

This is the shared presentation contract, not the raw API schema. Persist reminder state separately, including rule revisions, episode IDs, consumed thresholds, candidate deadlines, occurrence keys, and delivery claims. Settings also has a schema version and revision. Unknown fields in upstream responses are ignored safely; missing documented optional fields are not errors.

Validate finite numbers before arithmetic. Accept percentage values in 0–100 and derive remaining exactly once; invalid/out-of-range values become unknown with a diagnostic, rather than silently clamping them into a false 0%/100% alert. Clamp only visual bar width defensively. Convert valid source Unix seconds to epoch milliseconds at the adapter boundary and reject impossible/non-finite dates; absence is null. Validate positive durations and nonnegative integer credit counts. Preserve a last-good field separately if a new response is invalid, but mark it stale and ineligible for reminders. Missing values are never zero. Optional credits and account fields need their own validation timestamps internally.

Use top-level `rateLimits` as the default when present. If it is absent, select the `codex` entry from `rateLimitsByLimitId` only when the adapter recognizes it; do not select an arbitrary map entry. Deduplicate the top-level and map representations of the same family. Expose other families as **Additional limits available; not monitored in this version**. Stable window keys combine the family and schema-defined slot, checked against duration; a changed duration/mapping suspends old rules pending a new baseline. Never combine unrelated families into a minimum or average. Support one window, null windows, and entirely unavailable quotas. Treat monetary credits/unlimited flags as separate concepts from quota percentages and reset-credit availability.

Account binding is a prerequisite for publishing fresh data. At startup/reconnect, establish the local account context through the supported account interface without reading credential files; confirm it around each polling cycle if there is no reliable account-change signal. Account changes increment the connection generation, clear the visible cache, cancel old quota candidates, and rebind rules after a successful read. General scheduled check-ins remain global and include quota detail only for the newly verified context. If a tested API version establishes only a session-scoped identity, disable cross-restart quota caching and use a silent baseline-only first sample after restart; do not guess identity from plan type. If account changes cannot be distinguished even within a session, suspend account-bound reminders until the adapter can safely rebind. Record this reduced capability in diagnostics.

### 7.3 Native-window considerations

A hover card uses a **separate native window** from its compact trigger. CSS cannot expose content beyond native bounds. Reuse a single preview window across modes and create the dashboard lazily; avoid one webview per mode. Before showing a tray hover box, render its compact two-row layout and resize the native window to the measured content; do not retain a larger mode's invisible bounds. The icon remains the activation target for the full dashboard. Tauri handles windows, tray events, and positioning; React handles content and animation inside the allocated bounds. Where Tauri cannot provide reliable work-area, fullscreen, or no-activation behavior, isolate a small Win32 adapter behind the window coordinator and test it on Windows 10/11.

Store placement in logical coordinates with monitor identity, anchor, and offset. Convert native physical coordinates exactly once at the boundary, including negative virtual-screen coordinates. Recompute placement on DPI/work-area/display changes; if the monitor disappears, move to the primary monitor and clamp the entire interactive surface onscreen. Flip previews to the side with space; if the card cannot fit, use a scrollable dashboard. Provide Reset position in the tray menu. Fullscreen detection uses foreground-window bounds on the relevant monitor and a conservative fallback to tray-only display when uncertain.

Use tray-event geometry with the [Tauri positioner](https://v2.tauri.app/plugin/positioner/) as an aid, not a substitute for work-area clamping; forward tray events to the plugin when required. Handle Explorer restarts by recreating the tray icon without spawning another quota service. A hover window must not activate or appear in Alt+Tab. User clicks on a widget or tray icon open or raise the dashboard and request foreground activation; never request focus from a reminder or background refresh. Windows may deny foreground activation under its foreground-lock rules, so report that outcome and retain the visible dashboard. Owned dialogs and native menus belong to the same focus group. Always on top defaults off and does not override fullscreen suppression. Retain the tray recovery route even when shells are hidden.

### 7.4 Resilience and security

Keep authentication within the local Codex installation; do not copy secrets into settings or logs. An unsupported account/authentication mode shows an explanatory state and guidance to authenticate using Codex. Avoid repeated authentication attempts. API-key accounts must not be represented as having the ChatGPT quota windows. The widget reads usage only and never redeems credits, issues inference requests, or handles arbitrary approval requests.

Expose narrow Tauri commands to local bundled UI only; restrict capabilities by window, apply a content security policy, and render server strings as text. Do not allow arbitrary filesystem/shell/network access from React. Validate all IPC input in Rust. Log bounded diagnostic codes and timing, excluding tokens, raw responses, paths containing secrets, and full account identifiers. Notification payloads carry opaque event IDs only; if actions are enabled later, validate the ID against the ledger and refresh before showing detail.

### 7.5 Refresh, freshness, and ordering

Use one scheduler and at most one in-flight quota read. Periodic ticks, manual refresh, push hints, window-open refreshes, wake events, and reset deadlines all request work through it. The most active visible surface selects the interval; opening a second surface cannot add another polling loop.

| Condition | Policy |
|---|---|
| Dashboard or hover preview (including the compact tray box) visible | Every 120 seconds by default |
| Compact Floating/Edge/HUD visible | Every 120 seconds by default |
| Tray-only, hidden shells, or locked session | Every 120 seconds by default |
| User configuration | Foreground 15–120 seconds; compact 30–300; background 60–600; validate foreground ≤ compact ≤ background |
| Battery saver | Double intervals, cap at 600 seconds; manual refresh and one-shot reset checks still work |
| Push hint / opening detail | Coalesce; refresh if the relevant observation is older than 15 seconds or the hint invalidates it |
| Manual Refresh | Join current work; enforce a 5-second minimum between read starts and honor server Retry-After |
| Reset deadline | One refresh at deadline + 2 seconds; if unchanged, at most two follow-ups after 15 and 60 seconds, then normal polling |
| Transient read/network/process failure | Retry after 5, 15, 30, 60, 120, then 300 seconds with ±20% jitter; honor longer Retry-After when provided |
| Authentication/unsupported schema | Stop automatic quota polling until sign-in/context/version changes or explicit Retry |

Normal cadence has ±10% jitter; one-shot due times do not. On success reset transient backoff. Settings show effective cadence and warn that low-usage detection can lag by that interval plus request latency; scheduled reminders are evaluated independently of polling. A user-triggered Retry may bypass transient backoff once per 5 seconds, never a server-imposed delay.

Requests record their connection/account generation and invalidation revision. An update hint arriving during a read marks the result dirty: do not use it for reminder triggers; queue one coalesced follow-up, with the same minimum spacing. Repeated hints collapse into one dirty flag, not an unbounded queue. If hints continually invalidate reads, preserve last-good data as stale and expose that reconciliation is pending. Discard results for superseded generations. Never merge partial push payloads into a full response unless the versioned adapter explicitly defines merge semantics.

Freshness is independent of transport health: a running process may have stale data, and cached data may exist while offline. A failed read immediately marks the affected data stale. Without failures, mark a field stale when its successful observation age exceeds `max(90 seconds, 2 × current effective interval + 15 seconds)`; after restart, resume, or a major clock change, treat cached data as stale until revalidated. Notification evaluation requires a newly validated observation, and any deferred quota notification older than 60 seconds requests revalidation before submission. Transport receipt time is not proof that upstream quota accounting is instantaneous; expose any source timestamp supplied by the adapter.

Compute countdowns as `max(0, resetsAtMs - currentUtcMs)` on render, never by decrementing a stored counter. Use monotonic time for in-process timeouts/backoff and wall-clock UTC for persisted due instants. On sleep/resume, network recovery, lock/unlock, time-zone change, or detected clock jump over 60 seconds, recompute deadlines and coalesce one reconciliation read. Do not replay elapsed timer ticks. One bounded reset-check sequence belongs to one episode; an unchanged past deadline cannot retrigger it every second. A timezone change affects labels and recurring schedules, not quota UTC deadlines.

### 7.6 Persistence, startup, and shutdown

Store widget settings and reminder state beside the widget executable by default. General settings offer that location, the existing per-user AppData directory, or an absolute custom directory with a native folder chooser. Resolve unavailable locations in preferred → EXE folder → AppData order, including runtime fallback, while showing preferred/effective locations. Keep the portable location manifest beside the EXE when writable and use a per-EXE-directory AppData manifest otherwise; newest valid revision wins. Preserve settings and the entire ledger during journaled relocation, retain original/previous files, request a choice for conflicting destination configurations, lock writers, and pause reminders when only an older recovery copy is available. Newer-format or unrelated data must never be overwritten silently. Codex authentication and Codex configuration remain outside this widget storage. Use Rust-owned transactional storage (SQLite is the proposed default) for versioned settings, account-scoped snapshots, reminder episodes, scheduled occurrences, and delivery claims. Commit a settings revision together with cancellation/recalculation of its pending reminders. Debounce placement saves until drag-end; do not write countdown ticks. Retain user-visible history for 30 days with a 1,000-entry cap; compact older records into consumed-state tombstones. Keep current-episode thresholds and schedule high-water marks independently of history retention so clearing history, restart, and a backward clock change cannot replay events. Delete retired-account caches after 30 days; require a fresh baseline when that context returns.

Validate migrations and recover from a last-known-good backup after corruption. Any rollback or loss of the delivery ledger pauses desktop reminders because prior submissions may no longer be recorded. Show the recovery error and require explicit re-enablement with a fresh silent quota baseline and scheduled high-water marks advanced to the recovery time; do not replay events from the missing interval. If restoration is unavailable, load safe display defaults with reminders disabled. Do not claim settings were saved when storage is unwritable or full. Bound and rotate diagnostics; provide a user-initiated local-data reset separate from Clear history.

Local settings transactions cannot atomically change Windows registration or permission state. Persist requested preferences first, apply external effects idempotently, then persist observed success/failure. On startup reconcile any interrupted operation. Keep requested and effective state visible until reconciliation completes; never report autostart or native notifications as working solely because the local preference was saved.

Acquire the single-instance guard before spawning Codex or creating schedulers. A second manual launch forwards Show dashboard to the existing process; a duplicate login/background launch exits without stealing focus. Use [Tauri's single-instance plugin](https://v2.tauri.app/plugin/single-instance/). First launch opens a small onboarding/dashboard window explaining the tray, connection setup, and reminder opt-in; subsequent manual launches show the Overview dashboard, while login launches start hidden in the tray and restore a configured compact shell without stealing focus. Never restore a hover preview, popup, or modal dialog automatically.

Launch at sign-in is off by default. Apply changes through the [Tauri autostart plugin](https://v2.tauri.app/plugin/autostart/), check the resulting registration, and display failures; distinguish the requested preference from observed startup status when Windows disables it externally. It starts after user sign-in, not at system boot. Closing the last window leaves the tray process alive and explains that behavior once. If tray creation fails, keep a standard dashboard accessible; do not hide the only control surface.

Quit cancels schedulers and pending submissions, persists committed state, closes child stdin, and waits up to 2 seconds for the owned app-server before terminating that process tree. Never kill unrelated Codex processes. Use a Windows Job Object or equivalent ownership mechanism to prevent orphaned children after a crash. Reconnection disposes of the previous child before spawning another; after five child exits in five minutes, open the circuit and require Retry. Hidden operation keeps the Rust event loop alive even with no webview. Tray tooltip and reminder status must remain usable in this state.

### 7.7 Packaging and resource budgets

Ship an installed Windows build with a stable application identifier and notification identity. Tauri uses WebView2 on Windows; the installer must detect/provision a compatible runtime and report offline-install failures clearly. Pin tested Tauri/plugin/Rust versions and record Windows versions and architectures actually validated; ARM64 support must not be assumed from an x64 build. See [Tauri Windows installer guidance](https://v2.tauri.app/distribute/windows-installer/).

Provisional performance budgets on a documented reference Windows 11 x64 machine: average idle CPU below 1% of total machine capacity over 10 minutes, widget-owned process-tree private memory below 200 MiB after warm-up, and combined memory including the owned Codex process below 400 MiB. These are measurement targets, not guarantees supplied by Tauri. Record both idle tray-only and visible-dashboard results, with versions and polling settings. Confirm memory plateaus after 100 open/close or mode switches, and that no child processes or listeners accumulate.

Avoid hidden animations, per-second backend broadcasts, idle polling of cursor position, and always-live unused dashboards. Keep one compact webview, one reusable preview, and at most one dashboard. Allow hidden detail webviews to be destroyed after 5 minutes; reopening hydrates state through IPC. Use an event-driven next-deadline timer in Rust for reminders, with a low-frequency wall-clock reconciliation check (at most once per minute). Resource measurements include the Codex sidecar, WebView2 children, timers, and notification/persistence work.

## 8. Accessibility and interaction quality

- Support keyboard activation and closing with Enter/Space and Escape where appropriate; a mouse hover must never be the exclusive path to detail.
- Provide accessible names for tray menu items, mode switches, percentages, and refresh actions.
- Ensure progress bars have text labels and do not communicate state through color alone.
- Apply Section 6.4's 4.5:1 normal-text and 3:1 essential-graphics/control contrast targets in both themes; use system colors in high contrast.
- Respect system text scaling and reduced-motion preferences; avoid tiny click targets in edge mode.
- Preserve focus when hover previews appear; move focus deliberately only when opening a keyboard-invoked dashboard.
- HUD and Edge suppress the main-content focus outline during pointer interaction, including click and drag. Keyboard input restores the normal visible focus cue. This does not hide the widget frame, period dividers or retained-data outlines.
- Avoid automatic overlays in fullscreen or presentation contexts unless explicitly enabled.
- Use minimum 32-pixel desktop controls and a touch-friendly 44-pixel option; text scaling must reflow cards instead of clipping controls.
- Allow the compact tray hover box to grow beyond its baseline size for text scaling and long period labels. Keep its content read-only; provide the same information through the keyboard-accessible dashboard.
- Expose progress values and freshness to screen readers, but do not announce countdowns each second. Announce status changes politely and once.
- On user-invoked detail, focus its heading or first control and keep a logical tab order; restore focus to the initiating app control when it still exists. Hover never changes focus.
- Offer an optional configurable global Show dashboard shortcut, disabled by default; report registration conflicts and retain the tray route.
- Reminders never require sound, color, or toast action buttons to be understood. Their history and snooze controls are keyboard accessible.

## 9. Acceptance criteria

| ID | Scenario | Pass condition |
|---|---|---|
| AC-01 | Compact widget is visible | Actual window durations are labeled correctly; Edge shows five-hour and weekly remaining; unknown/invalid values are never zero placeholders |
| AC-02 | Hover over Floating, Edge, or HUD | The same read-only preview appears after 200 ms, without stealing focus or noticeable flicker |
| AC-03 | Move cursor from trigger to preview | Preview remains open and is usable |
| AC-04 | Leave both trigger and preview | Only transient view dismisses after configured grace period |
| AC-05 | Drag in Edge mode | Widget moves freely, docks to the nearest left/right side on the release monitor with work-area clamping, persists side/monitor/height atomically, and respects the shared position lock |
| AC-06 | Click or double-click tray icon or desktop widget | Single-click opens Overview popup; double-click opens the standard Overview window; repeated activation raises it when Windows permits |
| AC-07 | Press Escape / click outside | Popup closes after owned menus/dialogs; standard window survives focus loss; no Edge-only pin setting exists |
| AC-08 | Codex is unavailable | Stale or unavailable status appears; prior percentages are not misrepresented as current |
| AC-09 | Reset timestamp advances/passes | Countdown never goes negative; bounded reconciliation runs once per episode; a changed timestamp alone never creates a recovery notification |
| AC-10 | Switch monitor / scaling | Windows remain onscreen, and tray/edge placement is recalculated |
| AC-11 | Accessibility | Keyboard route, readable labels, and reduced-motion behavior work |
| AC-12 | Multiple layout modes | All views use the same underlying normalized quota state |
| AC-13 | 25% → 9% → 8% remaining, then restart | One 10%-severity summary consumes 20%/10% thresholds; no duplicate after restart; 0% can trigger once later |
| AC-14 | First enablement at 9%; threshold edit; 9% → 11% → 9% | One initial summary; editing rebases silently; correction jitter does not rearm consumed levels |
| AC-15 | Two windows become low in one refresh | One coalesced notification names both; dashboard and history retain individual event identities |
| AC-16 | Deadline passes while offline or weekly limit stays exhausted | No fabricated 100% value or account-wide recovery claim; retry remains bounded; fresh evidence governs recovery |
| AC-17 | Quiet hours, snooze, lock, fullscreen, and OS denial | App suppression retains bounded candidates; resume sends at most one eligible digest; denial causes no prompt/retry storm |
| AC-18 | Scheduled reminder across sleep, restart, DST gap/fold, timezone/clock change | Within 15 minutes catch up once; older occurrences are missed; repeated local times/clock rollback do not replay consumed occurrences |
| AC-19 | Crash immediately before/after native submission | Claimed event is never blindly replayed; uncertain outcome is visible; definitive failure permits bounded retry only |
| AC-20 | Poll, manual Refresh, push hint, and reset timer overlap | At most one read is in flight; redundant triggers coalesce; old-generation/dirty results cannot trigger reminders |
| AC-21 | Open a view during an update; reload a webview | Subscribe/snapshot ordering yields the latest revision; no duplicate scheduler or notification |
| AC-22 | Account changes, field disappears, malformed or out-of-range data arrives | Old-account values/candidates are cleared; missing/invalid fields are unknown or explicitly stale; rule scope remains correct |
| AC-23 | Close all windows; launch twice; sign in; Quit | Monitoring survives hidden views; one process owner and child exist; autostart respects preference; Quit stops both |
| AC-24 | Storage full/corrupt, schema upgrade, Clear history | Writes fail visibly; notification deduplication remains safe; migration preserves settings; history clearing does not rearm events |
| AC-25 | Installed Windows 10/11 notification tests | Correct app identity/icon; silent/privacy preferences verified; OS-blocked delivery explained; unsupported actions omitted |
| AC-26 | Explorer restart, tray overflow, absent monitor, 100–200% mixed DPI | Control surface remains reachable; icon recovers; windows clamp correctly without invisible input interception |
| AC-27 | Drag anywhere on a compact shell; double-click; open a context menu; change modes quickly | Drag release never activates; the whole Floating/HUD shell moves; Edge remains constrained; menus and dashboard activation cancel stale hover work |
| AC-28 | Idle operation and 100 mode/window cycles | Section 7.7 budgets measured; memory/listeners stabilize; hidden views do not run countdown updates |
| AC-29 | Supported and unsupported Codex CLI/account variants | Version/schema checks gate optional fields; unsupported mode has an actionable status and no fabricated quota |
| AC-30 | Slow/failed refresh during scheduled check-in; long snooze ends | Neutral scheduled reminder can fire on time; deferred quota detail requires fresh validation; no backlog burst |
| AC-31 | Hover tray icon with fresh quota data | After 200 ms, a read-only box near 260 × 76 logical pixels shows period-labeled percentages and the next known expected reset without taking focus; no detailed bars or controls |
| AC-32 | Move between tray icon and box, leave, then re-enter | Connecting corridor prevents flicker; entry cancels dismissal; leaving both dismisses after 350 ms; no invisible window blocks unrelated input |
| AC-33 | Click, double-click, or right-click while a widget or tray preview is visible | Single-click opens Overview popup; double-click opens the standard window; right-click shows the shared tray menu; no simultaneous preview or hover reopening until fresh entry |
| AC-34 | Tray preview with missing/stale data, expired deadline, or blocked usage | Unknown values are labeled; retained values say Last known; status takes priority over forecasts; no inferred reset, usable hours, zero placeholder, or optimistic availability claim |
| AC-35 | Tray hover with overflow, auto-hide, Explorer restart, monitor edges, or mixed DPI | Box remains correctly sized and positioned; unsupported custom hover uses one native tooltip; failure never opens a dashboard without explicit activation |
| AC-36 | All four modes in Light and Dark | Compact surfaces, previews, dashboard, settings, and loading/stale/offline/error/focus states use the shared tokens and identical information hierarchy; tray hover remains smaller than clicked detail |
| AC-37 | Change Windows app theme or save an explicit override; restart/recreate windows | Follow Windows tracks changes live; explicit overrides persist; all app windows agree before display; theme changes preserve placement, drafts, interaction and quota state without a new read |
| AC-38 | Enable/disable high contrast or transparency; increase text size at 100–200% DPI | System colors take precedence and the saved theme returns afterward; opaque fallback remains readable; text reflows without clipping; measured text/control/graphic contrast meets Section 6.4 |
| AC-39 | Inspect display-mode choices and quota examples | Exactly four modes are offered; no inline chip or taskbar-adjacent substitute; time values describe periods or resets, never usage hours inferred from percentages |
| AC-40 | Reopen dashboard after visiting Settings | Widget and tray activation select Overview; the explicit Settings command still opens Settings |

Validate reminder, refresh, and tray hover timing with a fake clock, recorded redacted protocol fixtures, and injected native/notification adapters. Inspect all four layouts in both themes with fresh, missing, stale, and failed data. Native focus, tooltip suppression, installation identity, tray geometry, screen-reader behavior, Windows theme/accessibility changes, and OS suppression require installed-build/manual Windows checks. This revision defines those checks; document validation does not establish that an application implementation has passed them.

## 10. Recommended delivery sequence

**Phase 1 — Feasibility and shared core:** Pin a Codex schema and Tauri/plugin versions; validate account binding, native executable discovery, installed notifications, no-focus preview, tray geometry, and native-tooltip suppression. Build the Rust state owner, transactional persistence, process supervision, refresh scheduling, appearance settings/system observers, and a basic dashboard with missing/stale states. Establish shared light/dark tokens and high-contrast support alongside the first UI. Verify protocol fixtures before visual polish.

**Phase 2 — Tray and reminders:** Deliver the default tray mode with its compact hover box, larger clicked dashboard, tooltip fallback, close-to-tray lifecycle, single instance, opt-in autostart, reminder rules/schedules, durable ledger, snooze/quiet hours, and history. Validate both themes, pointer travel, 200/350 ms hover timing, focus preservation, and menu transitions. Test hidden operation, restart, crash boundaries, and actual installed Windows notifications.

**Phase 3 — Multiple layouts:** Add Floating, Edge, and HUD adapters in both themes, shared Edge/HUD hover and Float click-only detail, drag/lock, standard dashboard promotion, and monitor/DPI/fullscreen handling. All layouts consume the existing service, preview window, and appearance/settings contract. Keep the mode list at four; no taskbar chip is included.

**Phase 4 — Release validation:** Complete the four-mode light/dark matrix, contrast and text-scaling checks, high-contrast/reduced-motion polish, keyboard/screen-reader checks, packaging and upgrade tests, resource profiling, and the acceptance matrix. Accessibility, offline behavior, and notification deduplication are baseline requirements in earlier phases, not optional finishing work. Gate optional toast buttons/click activation on the feasibility results.

## 11. Conclusion

The recommended design is a **single desktop utility with four display modes, shared light/dark themes, one authoritative Rust service, and one reminder policy**. The compact tray hover box provides a quick read; explicit activation opens or raises the Overview dashboard. Floating, Edge, and HUD share a temporary preview, while the Edge glance shows both quota windows. Fresh service observations govern quota claims; local clocks schedule checks and forecasts.

**Version 1.10:** Tightens Edge/HUD padding and profiles, makes Edge rings and reset strips equally wide, uses neutral grey badges and colored Edge percentages, and replaces vertical-only dragging with release-to-edge docking on the drop monitor.

**Version 1.9:** Adds shared optional 5 / W badges, stable Edge/HUD rectangles with space only for visible warnings, used/reset metadata in desktop hover, centered HUD click popups, three-row credit disclosure and Floating close-to-tray. The reference contributes only the badge concept.

**Version 1.7:** Keeps Settings on Floating only, tightens compact spacing, requires rounded native silhouettes and consistent preview sizing, restores hidden widgets on explicit selection, distinguishes a one-time raise from persistent topmost behavior, and prevents Floating from covering its popup.

**Version 1.5:** Defers single-click activation to prevent double-click popup flashes; keeps Settings in the shared popup; sizes desktop shells to contents; adds per-mode reset countdowns, repeated-missing-read confirmation, and Edge popup side anchoring.

**Version 1.4:** Makes compact shells fully draggable, constrains Edge movement to the selected side, opens the standard Overview window on left-double-click, and reuses the native tray menu for right-clicks on widgets and previews.

**Version 1.2:** Specifies the small tray hover box separately from the clicked dashboard; adds paired light/dark tokens, Follow Windows behavior, appearance persistence, accessibility precedence, and corresponding acceptance criteria. Explains the reference board's taskbar-chip exclusion and removes ambiguity about hour-based quota estimates. Updates delivery phases while preserving the shared service and reminder design.

**Version 1.1 foundation:** Defined reminders and schedules, durable deduplication, click/focus behavior, refresh ordering, account isolation, persistence, lifecycle, resource budgets, and protocol/notification compatibility gates. Remaining gates and their fallbacks are recorded in Appendix B.

---

## Appendix A. Source data and definitions

The original report describes a supplied local example containing `rateLimits.primary`, `rateLimits.secondary`, `rateLimitResetCredits`, and `ordinaryUsageAllowed`: a five-hour quota at **0% used / 100% remaining**, a seven-day quota at **26% used / 74% remaining**, and three available reset credits. The underlying response was not included with this revision, so those values remain illustrative rather than independently verified live data or interface defaults.

The official documentation describes rate-limit reads/updates, optional windows, multiple limit families, and reset-credit metadata. `ordinaryUsageAllowed` was not found in the reviewed public page; support it only if the selected local version's generated schema confirms it. Do not infer credit scope or full-reset behavior from a count. Source: [Codex app-server rate-limit contract](https://learn.chatgpt.com/docs/app-server).

| JSON field | Interpretation |
|---|---|
| `usedPercent` | Percentage consumed within a quota window |
| `windowDurationMins` | Quota-window duration in minutes |
| `resetsAt` | Unix timestamp in seconds for quota reset |
| `ordinaryUsageAllowed` | Optional version-dependent availability signal; unknown when absent or unsupported |
| `rateLimitResetCredits.availableCount` | Count of available reset credits, if provided |
| `planType` | Optional plan designation from the validated account/limit contract; not an identity key |

Source timestamps use seconds; the application model uses milliseconds with explicit `Ms` suffixes. A rate-limit window, a reset credit, and a monetary credit balance are separate objects. All quoted example values are fixtures for design illustration only.

## Appendix B. Remaining technical decisions and validation gates

Tray-only default, four display modes without an inline chip, transient Screen Edge hover, compact read-only tray hover, Follow Windows with Light/Dark overrides, Rust-owned scheduling, and no reminders after Quit are settled product decisions. The following require implementation spikes or version-specific evidence:

| Decision | Proposed default / fallback | Evidence needed before enabling broader behavior |
|---|---|---|
| Supported Codex versions and account identity | Explicitly tested native Windows CLI versions; unknown optional fields hidden; no persistent account cache without reliable binding | Generated schemas and redacted responses for signed-in, signed-out, changed-account, missing-window, and unsupported-auth cases; verify credential changes are observed by the owned server |
| Windows toast actions and activation | Ordinary installed desktop notifications; snooze/details in tray and dashboard | Pin plugin version; test visible-toast click, Notification Center click, process-not-running activation, silent mode, and identity across updates; use a dedicated WinRT adapter only if added scope requires it |
| Installer format and supported architectures | Evaluate per-user NSIS first; one stable app identity; x64 first | Confirm WebView2 provisioning, signing, notification registration, upgrade/uninstall cleanup, and Windows 10/11 build coverage; validate ARM64 separately |
| Tray hover and native focus/placement adapters | Compact custom box when reliable; exclusive native-tooltip fallback for hover; standard dashboard fallback only on explicit activation | Installed tests for enter/leave events, pointer corridor, no-focus behavior, native-tooltip suppression, overflow, Explorer restart, auto-hide, mixed DPI, monitor edges, owned dialogs, and fullscreen applications |
| Windows appearance and accessibility integration | Shared opaque light/dark tokens; high contrast overrides app theme using system colors | Verify app-theme observation, explicit overrides, tray-icon legibility against the OS theme, no initial theme flash, saved-preference migration, live accessibility changes, rendered contrast, and text scaling |
| Threshold/reset episode mapping for real accounts | Conservative confirmation rules in Section 5.5; suppress ambiguous recovery claims | Fixtures for deadline corrections, early credit use, rolling recovery, null timestamps, and shared/multiple limit families |
| Performance and interaction timing | Section 7.7 budgets; tray hover 200 ms, other previews 100–200 ms, tray exit grace 350 ms | Reference-hardware measurements including Codex/WebView2 processes, preview-window resizing/reuse, pointer/trackpad/touch sessions, and high text scaling |

If a capability fails its gate, ship the stated fallback and disclose it in Settings. Do not silently expand permissions, enable toast actions with unreliable activation, or replace missing quota data with estimates.
