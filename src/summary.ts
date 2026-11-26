import { compactReset, countdown, percent } from "./format.ts";
import type { Usage } from "./state";

export interface TrayHoverRow {
  key: string;
  role: "five-hour" | "weekly";
  label: string;
  value: string;
  reset: string | null;
  due: boolean;
  unavailable: boolean;
  retained: boolean;
}

export interface TrayHoverSummary {
  rows: TrayHoverRow[];
  status: string | null;
  critical: boolean;
  fallback: { first: string; second: string | null } | null;
}

export function usagePending(usage: Usage): boolean {
  return (usage.consecutiveUnavailableReads ?? 3) < 3 && !["auth-required", "unsupported", "paused"].includes(usage.connection);
}

export function quotaPending(usage: Usage, key: string): boolean {
  const slot = key.slice(0, key.lastIndexOf("/"));
  return (usage.missingWindowReads?.[slot] ?? 3) < 3 && !["auth-required", "unsupported", "paused"].includes(usage.connection);
}

export function usageSummary(usage: Usage, now: number): { first: string; second: string | null } {
  if (usage.connection === "verifying") return {
    first: usage.refreshInFlight ? "Verifying account…" : "Account verification pending",
    second: !usage.refreshInFlight && usage.nextRefreshAtMs ? "Waiting to retry…" : null,
  };
  const usable = usage.windows.some(w => w.remainingPercent !== null);
  const stale = usage.freshness === "stale" || usage.windows.some(w => w.lastKnown);
  const first = usable ? `${stale ? "Last known: " : ""}${usage.windows.map(w => `${w.label} ${w.remainingPercent === null ? quotaPending(usage, w.key) ? "checking…" : "unavailable" : `${percent(w.remainingPercent)} left`}`).join(" · ")}` : usagePending(usage) ? "Checking usage…" : "Usage unavailable";
  if (usage.freshness === "stale") return { first, second: usage.connection === "offline" ? "Offline · out of date" : "Out of date" };
  if (usage.ordinaryUsageAllowed === false) return { first, second: "Ordinary usage blocked" };
  if (stale) return { first, second: "Some data out of date" };
  if (!usable) return { first, second: usage.connection === "auth-required" ? "Sign-in required" : usage.connection === "ready" ? "No quota windows available" : usage.connection };
  const windows = usage.windows.filter(w => w.quality === "valid" && w.resetsAtMs !== null).sort((a,b) => a.resetsAtMs! - b.resetsAtMs! || (a.durationMinutes ?? Infinity) - (b.durationMinutes ?? Infinity));
  const earliest = windows[0];
  return { first, second: !earliest ? null : earliest.resetsAtMs! <= now ? "Reset due — unconfirmed" : `${earliest.label}: ${countdown(earliest.resetsAtMs!, now).toLowerCase()}` };
}

export function trayHoverSummary(usage: Usage, now: number): TrayHoverSummary {
  const usable = usage.windows.some(w => w.remainingPercent !== null);
  if (usage.connection === "verifying" || !usable) {
    return { rows: [], status: null, critical: false, fallback: usageSummary(usage, now) };
  }
  const stale = usage.freshness === "stale";
  const rows: TrayHoverRow[] = usage.windows.map((w, index) => {
    const role: "five-hour" | "weekly" = w.durationMinutes === 10080 || (w.durationMinutes !== 300 && index === 1) ? "weekly" : "five-hour";
    const label = w.durationMinutes === null ? (index === 0 ? "Primary (period unknown)" : "Secondary (period unknown)") : w.label;
    const unavailable = w.remainingPercent === null;
    const value = unavailable ? (quotaPending(usage, w.key) ? "checking…" : "unavailable") : `${percent(w.remainingPercent)} left`;
    const retained = stale || w.lastKnown;
    const showReset = !retained && w.quality === "valid" && w.resetsAtMs !== null;
    return {
      key: w.key,
      role,
      label,
      value,
      reset: showReset ? compactReset(w.resetsAtMs, now, w.durationMinutes) : null,
      due: showReset && w.resetsAtMs! <= now,
      unavailable,
      retained,
    };
  });
  if (stale) {
    return { rows, status: usage.connection === "offline" ? "Last known · Offline · out of date" : "Last known · Out of date", critical: false, fallback: null };
  }
  if (usage.ordinaryUsageAllowed === false) {
    return { rows, status: "Ordinary usage blocked", critical: true, fallback: null };
  }
  if (usage.windows.some(w => w.lastKnown)) {
    return { rows, status: "Some data out of date", critical: false, fallback: null };
  }
  return { rows, status: null, critical: false, fallback: null };
}
