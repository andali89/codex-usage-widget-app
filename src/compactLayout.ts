import type { DisplayMode, QuotaLabelStyle, ResetCountdowns, VisiblePeriods } from "./state";

/** Logical client size at 100% text. Only a warning may add space to a saved layout. */
export function compactLayout(mode: DisplayMode, style: QuotaLabelStyle, resets?: ResetCountdowns, warning = false, periods: VisiblePeriods = "both") {
  const count = Number(periods !== "weekly" && !!resets?.fiveHour) + Number(periods !== "five-hour" && !!resets?.weekly);
  const single = periods !== "both";
  // Four extra bottom pixels preserve the selected rings' content positions.
  if (mode === "screen-edge") return { width: 54, height: (style === "badges" ? single ? 100 : 150 : single ? 120 : 190) + 24 * count + (warning ? 18 : 0) };
  // Match the extra .25rem of right padding without moving the HUD's contents.
  if (mode === "compact-hud") return { width: (style === "badges" ? single ? 102 : 164 : single ? 120 : 200) + 4 + 58 * count + (warning ? 20 : 0), height: 38 };
  return null;
}
