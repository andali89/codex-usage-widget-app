import type { DisplayMode, QuotaLabelStyle, ResetCountdowns, VisiblePeriods } from "./state";

/** Logical size at 100% text. Taskbar Strip uses the width as a cap for measured content. */
export function compactLayout(mode: DisplayMode, style: QuotaLabelStyle, resets?: ResetCountdowns, warning = false, periods: VisiblePeriods = "both", showBars = true) {
  const count = Number(periods !== "weekly" && !!resets?.fiveHour) + Number(periods !== "five-hour" && !!resets?.weekly);
  const single = periods !== "both";
  if (mode === "taskbar-strip") return { width: 112 + (warning ? 16 : 0), height: single ? (showBars ? 28 : 24) : (showBars ? 42 : 36) };
  // Four extra bottom pixels preserve the selected rings' content positions.
  if (mode === "screen-edge") return { width: 54, height: (style === "badges" ? single ? 100 : 150 : single ? 120 : 190) + 24 * count + (warning ? 18 : 0) };
  // Match the extra .25rem of right padding without moving the HUD's contents.
  if (mode === "compact-hud") return { width: (style === "badges" ? single ? 102 : 164 : single ? 120 : 200) + 4 + 58 * count + (warning ? 28 : 0), height: 38 };
  return null;
}
