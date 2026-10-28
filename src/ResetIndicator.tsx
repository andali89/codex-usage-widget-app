import type { CSSProperties } from "react";
import { compactReset, resetSegments } from "./format";
import { Icon } from "./visuals";

export default function ResetIndicator({ deadline, durationMinutes, now, retained, segmentCount = 10 }: {
  deadline: number | null; durationMinutes: number | null; now: number; retained: boolean; segmentCount?: number;
}) {
  const filled = resetSegments(deadline, now, durationMinutes, segmentCount);
  const text = compactReset(deadline, now, durationMinutes);
  const label = `${retained ? "Last known expected" : "Expected"} reset${deadline === null ? " time unavailable" : deadline <= now ? " due — unconfirmed" : ` in ${text}`}`;
  return <span className={`reset-indicator${retained ? " retained" : ""}`} aria-label={label} title={`${label}${deadline === null ? "" : ` · ${new Date(deadline).toLocaleString()}`}`}>
    <span className="reset-countdown"><Icon name="clock" /><span className="reset-time">{retained ? "~" : ""}{text}</span></span>
    <span className={`reset-segments${filled === null ? " unknown" : ""}`} style={{"--reset-segment-count":segmentCount} as CSSProperties} aria-hidden="true">{Array.from({ length: segmentCount }, (_, i) => <i key={i} className={filled !== null && i < filled ? "filled" : undefined} />)}</span>
  </span>;
}
