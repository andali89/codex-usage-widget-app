import type { QuotaWindow, VisiblePeriods } from "./state.ts";

export interface TaskbarStripRow {
  key: string;
  role: "five-hour" | "weekly";
  label: string;
  remainingPercent: number | null;
  resetsAtMs: number | null;
  durationMinutes: number | null;
  lastKnown: boolean;
}

/** Use compact duration labels and keep unknown periods distinct. */
export function taskbarPeriodLabel(durationMinutes: number | null, slotIndex: 0 | 1): string {
  if (durationMinutes !== null && Number.isFinite(durationMinutes) && durationMinutes > 0) {
    if (durationMinutes % 1440 === 0) return `${durationMinutes / 1440}d`;
    if (durationMinutes % 60 === 0) return `${durationMinutes / 60}h`;
    return `${durationMinutes}m`;
  }
  return slotIndex === 0 ? "P1" : "P2";
}

function resolveBothSlots(windows: readonly QuotaWindow[]): [QuotaWindow | undefined, QuotaWindow | undefined] {
  const standardPrimary = windows.find(w => w.durationMinutes === 300);
  const standardSecondary = windows.find(w => w.durationMinutes === 10080);
  const used = new Set<QuotaWindow>();
  if (standardPrimary) used.add(standardPrimary);
  if (standardSecondary) used.add(standardSecondary);
  const primary =
    standardPrimary ??
    windows.find(w => !used.has(w) && w.key.includes("/primary/")) ??
    windows.find(w => !used.has(w));
  if (primary) used.add(primary);
  const secondary =
    standardSecondary ??
    windows.find(w => !used.has(w) && w.key.includes("/secondary/")) ??
    windows.find(w => !used.has(w));
  return [primary, secondary];
}

/** Build 1 or 2 honest rows for Taskbar Strip without turning missing data into 0% or inventing durations. */
export function taskbarStripRows(windows: readonly QuotaWindow[], selection: VisiblePeriods = "both"): TaskbarStripRow[] {
  if (selection !== "both") {
    const duration = selection === "weekly" ? 10080 : 300;
    const role = selection === "weekly" ? "weekly" : "five-hour";
    const slotIndex: 0 | 1 = selection === "weekly" ? 1 : 0;
    const quota = windows.find(w => w.durationMinutes === duration);
    return [{
      key: quota?.key ?? `missing-${role}`,
      role,
      label: taskbarPeriodLabel(quota ? quota.durationMinutes : duration, slotIndex),
      remainingPercent: quota?.remainingPercent ?? null,
      resetsAtMs: quota?.resetsAtMs ?? null,
      durationMinutes: quota ? quota.durationMinutes : duration,
      lastKnown: !!quota?.lastKnown,
    }];
  }
  const [primary, secondary] = resolveBothSlots(windows);
  return ([
    [primary, "five-hour", 0, 300],
    [secondary, "weekly", 1, 10080],
  ] as const).map(([quota, role, slotIndex, duration]) => ({
    key: quota?.key ?? `missing-${role}`,
    role,
    label: taskbarPeriodLabel(quota ? quota.durationMinutes : duration, slotIndex),
    remainingPercent: quota?.remainingPercent ?? null,
    resetsAtMs: quota?.resetsAtMs ?? null,
    durationMinutes: quota ? quota.durationMinutes : duration,
    lastKnown: !!quota?.lastKnown,
  }));
}
