import { compactReset, resetSegments } from "./format";
import { Icon } from "./visuals";

export default function ResetIndicator({ deadline, durationMinutes, now, retained }: {
  deadline: number | null; durationMinutes: number | null; now: number; retained: boolean;
}) {
  const filled = resetSegments(deadline, now, durationMinutes);
  const text = compactReset(deadline, now, durationMinutes);
  const label = `${retained ? "Last known expected" : "Expected"} reset${deadline === null ? " time unavailable" : deadline <= now ? " due — unconfirmed" : ` in ${text}`}`;
  return <span className={`reset-indicator${retained ? " retained" : ""}`} aria-label={label} title={`${label}${deadline === null ? "" : ` · ${new Date(deadline).toLocaleString()}`}`}>
    <span className="reset-countdown"><Icon name="clock" /><span className="reset-time">{retained ? "~" : ""}{text}</span></span>
    <span className={`reset-segments${filled === null ? " unknown" : ""}`} aria-hidden="true">{Array.from({ length: 10 }, (_, i) => <i key={i} className={filled !== null && i < filled ? "filled" : undefined} />)}</span>
  </span>;
}
