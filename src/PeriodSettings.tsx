import type { DisplayMode, ResetCountdowns, Settings, VisiblePeriods } from "./state";
import { showsPeriod } from "./periods";

export default function PeriodSettings({value,onChange}:{value:Settings["windows"];onChange:(value:Settings["windows"])=>void}) {
  return <fieldset><legend>Widget periods and reset countdowns</legend><p className="help">HUD and Edge can show both periods or just one. Hover previews and Overview still show both. A countdown appears only when its period is visible.</p>
    {([["floating","Floating"],["screen-edge","Screen Edge"],["compact-hud","Compact HUD"]] as const).map(([mode,label])=>{
      const selection:VisiblePeriods = mode === "floating" ? "both" : value.visiblePeriods?.[mode] ?? "both";
      const resetControls = ([["fiveHour","5-hour reset",300],["weekly","Weekly reset",10080]] as const).map(([key,text,duration])=><label className="check-field" key={key}><input type="checkbox" aria-label={`${label}: ${text}`} checked={value.resetCountdowns[mode]?.[key] ?? false} disabled={!showsPeriod(selection,duration)} onChange={event=>onChange({...value,resetCountdowns:{...value.resetCountdowns,[mode]:{fiveHour:false,weekly:false,...value.resetCountdowns[mode],[key]:event.target.checked}} as Partial<Record<DisplayMode,ResetCountdowns>>})}/><span>{text}{!showsPeriod(selection,duration) && <span className="muted"> (period hidden)</span>}</span></label>);
      return <div className={`countdown-option-row${mode !== "floating" ? " grouped-period-options" : ""}`} key={mode}><h2>{label}</h2>
        {mode !== "floating" && <label className="field-label">Displayed periods<select aria-label={`${label}: displayed periods`} value={selection} onChange={event=>onChange({...value,visiblePeriods:{...value.visiblePeriods,[mode]:event.target.value as VisiblePeriods}})}><option value="both">Both</option><option value="five-hour">5-hour only</option><option value="weekly">Weekly only</option></select></label>}
        {mode === "floating" ? resetControls : <div className="countdown-reset-options">{resetControls}</div>}
      </div>;
    })}
  </fieldset>;
}
