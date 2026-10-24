import type { QuotaWindow } from "./state";
import { compactReset, countdown, percent, resetTimestamp } from "./format";
import { Icon } from "./visuals";

export default function QuotaCard({ quota, stale, now, index = 0, pending = false, compact = false, showReset = true, showDetails = !compact }: { quota: QuotaWindow; stale: boolean; now: number; index?: number; pending?: boolean; compact?: boolean; showReset?: boolean; showDetails?: boolean }) {
  const retained = stale || quota.lastKnown;
  const known = quota.remainingPercent !== null;
  const period = quota.durationMinutes === 10080 ? "weekly" : quota.durationMinutes === 300 ? "five-hour" : String(index);
  const shortReset = compactReset(quota.resetsAtMs, now, quota.durationMinutes);
  const resetText = quota.resetsAtMs === null ? "Reset time not supplied" : !compact ? countdown(quota.resetsAtMs, now) : shortReset === "due?" ? "Reset due — unconfirmed" : `Resets in ${shortReset}`;
  return <section className={`quota-card period-${period}${compact ? " compact-card" : ""}${retained ? " retained" : ""}`} aria-label={quota.label}>
    <div className="quota-title"><h2>{quota.label}{compact && " limit"}</h2><span className={`quota-reading${known ? "" : " muted"}`}><strong>{!known && pending ? "—" : known ? percent(quota.remainingPercent) : "Unavailable"}</strong>{known && <span> remaining</span>}</span></div>
    <div className="quota-quality">{retained ? "Last known · out of date" : !known ? pending ? "Checking quota…" : "No validated quota value" : ""}</div>
    <div className={`progress${known ? "" : " unknown"}`} role="progressbar" aria-label={`${quota.label} remaining${retained ? ", last known" : ""}`} aria-valuenow={known ? quota.remainingPercent! : undefined} aria-valuemin={0} aria-valuemax={100} aria-valuetext={known ? `${percent(quota.remainingPercent)} remaining${retained ? ", out of date" : ""}` : "Unknown"}>{known && <span style={{ width: `${Math.min(100, Math.max(0, quota.remainingPercent!))}%` }} />}</div>
    {showReset && <div className="quota-meta"><span className="reset-summary"><Icon name="clock" />{retained ? "Last known: " : ""}{resetText}</span></div>}
    {showDetails && <div className="quota-details"><span className="used-reading"><Icon name="activity" />{quota.usedPercent === null ? "Used percentage unavailable" : `${percent(quota.usedPercent)} used`}{retained ? " · last known" : ""}</span><span className="exact-reset"><Icon name="clock" />{quota.resetsAtMs === null ? "Reset time not supplied" : <time aria-label={`${retained ? "Last known expected" : "Expected"} reset: ${resetTimestamp(quota.resetsAtMs)}`} dateTime={new Date(quota.resetsAtMs).toISOString()}>{retained ? "Last known expected reset: " : "Expected reset: "}{resetTimestamp(quota.resetsAtMs)}</time>}</span></div>}
  </section>;
}
