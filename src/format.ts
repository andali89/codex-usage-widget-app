export function percent(value: number | null): string {
  if (value === null) return "Unavailable";
  if (value > 0 && value < 1) return "<1%";
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(value)}%`;
}

export function countdown(deadline: number, now: number): string {
  const minutes = Math.ceil(Math.max(0, deadline - now) / 60000);
  if (!minutes) return "Reset due — unconfirmed";
  const days = Math.floor(minutes / 1440), hours = Math.floor(minutes % 1440 / 60), rest = minutes % 60;
  return `Expected reset in ${days ? `${days}d ` : ""}${hours ? `${hours}h ` : ""}${rest || (!days && !hours) ? `${rest}m` : ""}`.trim();
}

/** Compact expected reset forecast; an elapsed deadline never confirms recovery. */
export function compactReset(deadline: number | null, now: number, _durationMinutes: number | null): string {
  if (deadline === null) return "—";
  const remaining = deadline - now;
  if (remaining <= 0) return "due?";
  const minutes = Math.ceil(remaining / 60_000);
  const days = Math.floor(minutes / 1440), hours = Math.floor(minutes % 1440 / 60), rest = minutes % 60;
  if (days) return `${days}d${hours ? `${hours}h` : ""}`;
  return `${hours ? `${hours}h` : ""}${rest || !hours ? `${rest}m` : ""}`;
}

/** A forecast of time remaining, independent of quota percentage or recovery. */
export function resetSegments(deadline: number | null, now: number, durationMinutes: number | null): number | null {
  if (deadline === null || durationMinutes === null || !Number.isFinite(deadline) || !Number.isFinite(durationMinutes) || durationMinutes <= 0) return null;
  return Math.ceil(Math.min(1, Math.max(0, (deadline - now) / (durationMinutes * 60_000))) * 10);
}

export function resetTimestamp(deadline: number): string {
  return new Date(deadline).toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit", timeZoneName: "shortOffset" });
}
