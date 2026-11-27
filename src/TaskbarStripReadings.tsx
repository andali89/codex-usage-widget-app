import type { QuotaLabelStyle, QuotaWindow, VisiblePeriods } from "./state";
import { compactReset, percent } from "./format";
import { taskbarStripRows } from "./taskbarStrip";

export default function TaskbarStripReadings({
  windows,
  selection = "both",
  labelStyle = "words",
  showBars = true,
  retained = false,
  now,
}: {
  windows: QuotaWindow[];
  selection?: VisiblePeriods;
  labelStyle?: QuotaLabelStyle;
  showBars?: boolean;
  retained?: boolean;
  now: number;
}) {
  const rows = taskbarStripRows(windows, selection);
  return <>
    {rows.map(row => {
      const value = row.remainingPercent;
      const clamped = value === null ? 0 : Math.min(100, Math.max(0, value));
      const rowRetained = retained || row.lastKnown;
      const resetText = compactReset(row.resetsAtMs, now, row.durationMinutes);
      const period = row.durationMinutes === 300 ? "5-hour" : row.durationMinutes === 10080 ? "Weekly" : row.label;
      const badge = labelStyle === "badges" && (row.durationMinutes === 300 ? "5" : row.durationMinutes === 10080 ? "W" : null);
      const due = row.resetsAtMs !== null && row.resetsAtMs <= now;
      const resetUnavail = row.resetsAtMs === null;
      return <div
        key={row.key}
        className={`taskbar-strip-row-wrap ${row.role === "weekly" ? "role-wk" : "role-5h"}${rowRetained ? " retained" : ""}`}
      >
        <div className="taskbar-strip-row">
          <span className={badge ? "ts-lbl ts-badge" : "ts-lbl"} title={period}>{badge || row.label}</span>
          <span className={`ts-pct${value === null ? " ts-unavail" : ""}`}>{value === null ? "—" : percent(value)}</span>
          <span className={`ts-rst${due ? " ts-due" : resetUnavail ? " ts-unavail" : ""}`} title={resetText}>{resetText}</span>
        </div>
        {showBars && (
          <div className={`micro-bar${value === null ? " unknown" : ""}${rowRetained ? " retained" : ""}`} aria-hidden="true">
            {value !== null && (
              <i style={{ width: `${clamped}%`, minWidth: value > 0 && value < 1 ? "0.125rem" : undefined }} />
            )}
          </div>
        )}
      </div>;
    })}
  </>;
}
