import { countdown, percent } from "./format.ts";
import type { Usage } from "./state";

export function usagePending(usage: Usage): boolean {
  return (usage.consecutiveUnavailableReads ?? 3) < 3 && !["auth-required", "unsupported", "paused"].includes(usage.connection);
}

export function quotaPending(usage: Usage, key: string): boolean {
  const slot = key.slice(0, key.lastIndexOf("/"));
  return (usage.missingWindowReads?.[slot] ?? 3) < 3 && !["auth-required", "unsupported", "paused"].includes(usage.connection);
}

export function usageSummary(usage: Usage, now: number): { first: string; second: string | null } {
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
