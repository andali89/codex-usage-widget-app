import type { CSSProperties } from "react";
import ring from "./assets/brand-ring.png";
import empty from "./assets/usage-empty.png";
import type { DisplayMode } from "./state";
import { percent } from "./format";

export type IconName = "settings" | "refresh" | "clock" | "close" | "expand" | "chevron" | "credits" | "bell" | "monitor" | "sun" | "moon" | "check" | "activity" | "warning";
const paths: Record<IconName, string> = {
  settings: "M9.5 3.5 10 2h4l.5 1.5 1.7 1 1.6-.3 2 3.5-1.1 1.2v2l1.1 1.2-2 3.5-1.6-.3-1.7 1L14 18h-4l-.5-1.7-1.7-1-1.6.3-2-3.5 1.1-1.2v-2L4.2 7.7l2-3.5 1.6.3 1.7-1Z M15 10a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
  refresh: "M19 8a7 7 0 0 0-12-3L4 8m0-5v5h5 M5 12a7 7 0 0 0 12 3l3-3m0 5v-5h-5",
  clock: "M20 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z M12 5v5l3 2",
  close: "m6 4 12 12M18 4 6 16",
  expand: "M14 3h6v6m0-6-8 8 M10 3H4v14h14v-6",
  chevron: "m9 4 6 6-6 6",
  credits: "M5 6a7 7 0 1 1-1 7M5 2v5h5",
  bell: "M5 14h14l-2-3V7a5 5 0 0 0-10 0v4l-2 3Zm5 3h4",
  monitor: "M3 2h18v12H3V2Zm9 12v4m-5 0h10",
  sun: "M16 10a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM12 0v2m0 16v2M2 10h2m16 0h2M5 3l1 1m12 12 1 1M5 17l1-1M18 4l1-1",
  moon: "M19 13A8 8 0 0 1 9 2a8 8 0 1 0 10 11Z",
  check: "m5 10 4 4L19 4",
  activity: "M2 10h5l3-7 4 14 3-7h5",
  warning: "m12 2 10 16H2L12 2Zm0 5v5m0 3h.01",
};
export function Icon({ name, className = "" }: { name: IconName; className?: string }) {
  return <svg className={`icon ${className}`} viewBox="0 0 24 20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false"><path d={paths[name]} /></svg>;
}
export function BrandMark({ small = false }: { small?: boolean }) {
  return <span className={`brand-mark${small ? " small" : ""}`} aria-hidden="true"><img src={ring} alt="" draggable={false} /><span className="contrast-mark">C</span></span>;
}
export function EmptyArt() { return <img className="empty-art" src={empty} alt="" draggable={false} />; }

export function ModePreview({ mode }: { mode: DisplayMode }) {
  return <span className={`mode-preview mode-preview-${mode}`} aria-hidden="true"><span className="mode-mini"><BrandMark small /><span className="mode-mini-bars"><i /><i /></span></span>{mode === "system-tray" && <span className="mode-taskbar"><i /><i /><i /></span>}</span>;
}

export function PeriodBadge({ weekly = false }: { weekly?: boolean }) {
  return <span className={`period-badge${weekly ? " weekly" : ""}`} aria-hidden="true">{weekly ? "W" : "5"}</span>;
}

export function QuotaRing({ value, weekly = false, retained = false, badge = false }: { value: number | null; weekly?: boolean; retained?: boolean; badge?: boolean }) {
  const amount = value === null ? 0 : Math.min(100, Math.max(0, value));
  return <span className={`quota-ring${weekly ? " weekly" : ""}${value === null ? " unknown" : ""}${retained ? " retained" : ""}${badge ? " has-badge" : ""}`} style={{ "--quota-amount": amount } as CSSProperties} aria-hidden="true"><svg viewBox="0 0 44 44"><circle className="ring-track" cx="22" cy="22" r="19" /><circle className="ring-value" cx="22" cy="22" r="19" pathLength="100" strokeDasharray={`${amount} 100`} /></svg><span className="ring-reading">{badge && <PeriodBadge weekly={weekly} />}<strong>{value === null ? "—" : percent(value)}</strong></span></span>;
}
