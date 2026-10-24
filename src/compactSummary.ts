export interface CompactQuotaInput {
  durationMinutes: number | null;
  remainingPercent: number | null;
}

export interface EdgeQuotaValue {
  label: "5H" | "WEEK";
  remainingPercent: number | null;
}

export function edgeQuotaValues(windows: readonly CompactQuotaInput[]): EdgeQuotaValue[] {
  return ([
    { label: "5H", durationMinutes: 300 },
    { label: "WEEK", durationMinutes: 10080 },
  ] as const).map(({ label, durationMinutes }) => ({
    label,
    remainingPercent: windows.find(window => window.durationMinutes === durationMinutes)?.remainingPercent ?? null,
  }));
}

/** Keep both HUD positions present even before the first validated observation. */
export function hudQuotaValues(windows: readonly (CompactQuotaInput & { key: string; label: string })[]) {
  const slots = [300, 10080].map(duration => windows.find(quota => quota.durationMinutes === duration));
  return slots.map((known, index) => {
    const quota = known;
    return {
      key: quota?.key ?? `missing-${index}`,
      label: quota?.durationMinutes === 300 ? "5H" : quota?.durationMinutes === 10080 ? "WEEK" : quota?.label ?? (index === 0 ? "5H" : "WEEK"),
      durationMinutes: quota?.durationMinutes ?? null,
      remainingPercent: quota?.remainingPercent ?? null,
    };
  });
}
