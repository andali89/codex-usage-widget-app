import type { QuotaLabelStyle, QuotaWindow, ResetCountdowns, VisiblePeriods } from "./state";
import { percent } from "./format";
import { showsPeriod, effectiveResets } from "./periods";
import { PeriodBadge, QuotaRing } from "./visuals";
import ResetIndicator from "./ResetIndicator";

export default function CompactQuotaReadings({mode,windows,selection="both",resets,labelStyle,retained,now}:{mode:"screen-edge"|"compact-hud";windows:QuotaWindow[];selection?:VisiblePeriods;resets?:ResetCountdowns;labelStyle:QuotaLabelStyle;retained:boolean;now:number}) {
  const effective = effectiveResets(selection,resets), badges=labelStyle === "badges";
  return <>{([300,10080] as const).filter(duration=>showsPeriod(selection,duration)).map(duration=>{
    const quota=windows.find(window=>window.durationMinutes === duration), weekly=duration === 10080;
    const value=quota?.remainingPercent ?? null, label=weekly ? "WEEK" : "5H";
    const resetEnabled=weekly ? effective.weekly : effective.fiveHour;
    const reset=resetEnabled && <ResetIndicator deadline={quota?.resetsAtMs ?? null} durationMinutes={quota?.durationMinutes ?? null} now={now} retained={retained || !!quota?.lastKnown}/>;
    return mode === "screen-edge" ? <span className={`edge-value${weekly ? " weekly" : ""}`} key={duration}>
      {!badges && <span className="short-period">{label}</span>}<QuotaRing value={value} weekly={weekly} retained={retained} badge={badges}/>{reset}
    </span> : <span className={`hud-quota${weekly ? " weekly" : ""}${resetEnabled ? " with-reset" : ""}`} key={duration}>
      {badges ? <PeriodBadge weekly={weekly}/> : <span className="short-period" title={weekly ? "Weekly" : "5-hour"}>{label}</span>}
      <strong>{value === null ? "—" : percent(value)}</strong>
      <span className={`hud-track${value === null ? " unknown" : ""}`} aria-hidden="true">{value !== null && <i style={{width:`${value}%`}}/>}</span>
      {resetEnabled && <span className="hud-reset-slot">{reset}</span>}
    </span>;
  })}</>;
}
