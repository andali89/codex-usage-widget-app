import type { VisiblePeriods } from "./state";
export function showsPeriod(selection: VisiblePeriods = "both", duration: number | null): boolean {
  return duration === 300 ? selection !== "weekly" : duration === 10080 ? selection !== "five-hour" : false;
}
export function effectiveResets(selection: VisiblePeriods, resets?: {fiveHour:boolean;weekly:boolean}) {
  return {fiveHour:showsPeriod(selection,300) && !!resets?.fiveHour,weekly:showsPeriod(selection,10080) && !!resets?.weekly};
}
