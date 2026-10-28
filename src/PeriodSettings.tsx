import type { DisplayMode, ResetCountdowns, Settings, VisiblePeriods } from "./state";
import { showsPeriod } from "./periods";

export default function PeriodSettings({value,onChange}:{value:Settings["windows"];onChange:(value:Settings["windows"])=>void}) {
  return <fieldset><legend>Widget periods and reset countdowns</legend><p className="help">HUD and Edge can show both periods or just one. Hover previews and Overview still show both. A countdown appears only when its period is visible. Choose 2–12 remaining-time bars for each HUD and Edge period; the default is 10.</p>
    {([["floating","Floating"],["screen-edge","Screen Edge"],["compact-hud","Compact HUD"]] as const).map(([mode,label])=>{
      const selection:VisiblePeriods = mode === "floating" ? "both" : value.visiblePeriods?.[mode] ?? "both";
      const resetControls = ([["fiveHour","5-hour reset",300],["weekly","Weekly reset",10080]] as const).map(([key,text,duration])=>{
        const hidden = !showsPeriod(selection,duration);
        const enabled = value.resetCountdowns[mode]?.[key] ?? false;
        const toggle = <label className="check-field"><input type="checkbox" aria-label={`${label}: ${text}`} checked={enabled} disabled={hidden} onChange={event=>onChange({...value,resetCountdowns:{...value.resetCountdowns,[mode]:{fiveHour:false,weekly:false,...value.resetCountdowns[mode],[key]:event.target.checked}} as Partial<Record<DisplayMode,ResetCountdowns>>})}/><span>{text}{hidden && <span className="muted"> (period hidden)</span>}</span></label>;
        return <div className="reset-period-options" key={key}>{toggle}{mode !== "floating" && <label className="field-label">Remaining-time bars<select aria-label={`${label}: ${text} bars`} disabled={hidden || !enabled} value={value.resetSegmentCounts?.[mode]?.[key] ?? 10} onChange={event=>onChange({...value,resetSegmentCounts:{...value.resetSegmentCounts,[mode]:{fiveHour:10,weekly:10,...value.resetSegmentCounts?.[mode],[key]:Number(event.target.value)}}})}>{Array.from({length:11},(_,index)=>index+2).map(count=><option key={count} value={count}>{count}</option>)}</select></label>}</div>;
      });
      return <div className={`countdown-option-row${mode !== "floating" ? " grouped-period-options" : ""}`} key={mode}><h2>{label}</h2>
        {mode !== "floating" && <label className="field-label">Displayed periods<select aria-label={`${label}: displayed periods`} value={selection} onChange={event=>onChange({...value,visiblePeriods:{...value.visiblePeriods,[mode]:event.target.value as VisiblePeriods}})}><option value="both">Both</option><option value="five-hour">5-hour only</option><option value="weekly">Weekly only</option></select></label>}
        {mode === "floating" ? resetControls : <div className="countdown-reset-options">{resetControls}</div>}
      </div>;
    })}
  </fieldset>;
}
