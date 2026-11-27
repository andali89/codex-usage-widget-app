import { useEffect, useState } from "react";
import type { AppState, Settings } from "./state";
import { budgetFor, fittingScale, scaleIssues, scaleKey } from "./compactSizing";
import { appearanceIssues, contrast, paletteFor, presetIds, presetPalette, quotaSwatches, quotaColorName, type AppearanceIssue, type QuotaCustomization } from "./widgetAppearance";

export function widgetDraftIssues(draft:Settings,state?:AppState):AppearanceIssue[] {
  const issues=appearanceIssues(draft.widgetAppearance);
  if(!Number.isInteger(draft.windows.snapGapLogicalPx)||draft.windows.snapGapLogicalPx<0||draft.windows.snapGapLogicalPx>48)issues.push({field:"windows.snapGapLogicalPx",message:"Use a whole snap gap from 0 to 48 logical px."});
  if(state)issues.push(...scaleIssues(draft,state.settings,state.appearance.compactSizing??{},state.appearance.textScale??100,state.appearance.compactMonitorBudgets));
  return issues;
}
export function SnapGap({value,onChange,onDragging}:{value:number;onChange:(value:number)=>void;onDragging:(dragging:boolean)=>void}) {
  const [raw,setRaw]=useState(String(value));
  useEffect(()=>{if(Number.isFinite(value))setRaw(String(value));},[value]);
  const invalid=!Number.isInteger(value)||value<0||value>48;
  return <div className="snap-gap"><div className="snap-gap-heading"><label htmlFor="snap-gap">Snap gap <small>logical px</small></label><button type="button" className="text-button" onClick={()=>onChange(8)}>Reset to 8</button></div><div className="snap-gap-controls"><input type="range" aria-label="Snap gap" min="0" max="48" step="1" value={Number.isFinite(value)?value:8} onPointerDown={()=>onDragging(true)} onPointerUp={()=>onDragging(false)} onPointerCancel={()=>onDragging(false)} onBlur={()=>onDragging(false)} onChange={e=>onChange(Number(e.target.value))}/><input id="snap-gap" type="number" min="0" max="48" step="1" value={raw} aria-invalid={invalid} aria-describedby="snap-gap-help" onChange={e=>{setRaw(e.target.value);onChange(e.target.value.trim()===""?NaN:Number(e.target.value));}}/></div><p id="snap-gap-help" className={invalid?"field-error":"help"}>{invalid?"Enter a whole number from 0 to 48.":"Space between a snapped widget and the screen’s usable edge."}</p></div>;
}
export default function WidgetAppearanceSettings({state,draft,onChange,pending=false}:{state:AppState;draft:Settings;onChange:(next:Settings)=>void;pending?:boolean}) {
  const value=draft.widgetAppearance, id=value.presetId;
  const stripMode = draft.mode === "taskbar-strip";
  const stripVisible = stripMode && state.settings.mode === "taskbar-strip" && !state.presentation.hidden && !state.presentation.suppressed;
  const live = state.appearance.widget;
  const placementApplied = stripVisible && value.taskbarStripInTaskbar === live.taskbarStripInTaskbar;
  const insideTaskbar = value.taskbarStripInTaskbar && (!placementApplied || state.appearance.taskbarOverlayActive);
  const matchesTaskbar = stripMode && insideTaskbar;
  const appTheme = draft.themePreference === "light" || draft.themePreference === "dark"
    ? draft.themePreference : state.appearance.resolvedTheme;
  const theme = (matchesTaskbar ? state.appearance.systemResolvedTheme : undefined) ?? appTheme;
  const transparencyFallback = placementApplied && value.taskbarStripInTaskbar && value.taskbarStripTransparent
    && live.taskbarStripTransparent && !state.appearance.taskbarTransparencyActive
    ? !insideTaskbar ? "The strip is above the taskbar, so it uses an opaque card."
      : state.appearance.highContrast ? "Windows high contrast uses an opaque card."
      : !state.appearance.transparencyAllowed ? "Windows transparency effects are off."
      : "Native transparency is unavailable. The strip uses an opaque card."
    : null;
  const changeStrip = (patch: Partial<Settings["widgetAppearance"]>) => onChange({...draft,widgetAppearance:{...value,...patch}});
  const selected=id?value.quotaCustomizations?.[id]?.[theme]:undefined;
  const custom=selected??{enabled:false};
  const update=(patch:Partial<QuotaCustomization>)=>{if(id)onChange({...draft,widgetAppearance:{...value,quotaCustomizations:{...value.quotaCustomizations,[id]:{...value.quotaCustomizations?.[id],[theme]:{...custom,...patch}}}}});};
  const palette=paletteFor(value,theme);
  return <><fieldset><legend>Widget colors</legend><div className="preset-grid" aria-label="Color preset">{presetIds.map(preset=>{const p=presetPalette(preset,theme);return <button type="button" key={preset} data-preset={preset} aria-pressed={id===preset} onClick={()=>onChange({...draft,widgetAppearance:{...value,presetId:preset,colorMode:"theme-defaults",palettes:{light:{},dark:{}}}})}><span className="preset-colors" aria-hidden="true">{[p.background,p.fiveHour,p.weekly].map((color,index)=><i key={index} style={{backgroundColor:color}}/>)}</span><span>{preset[0].toUpperCase()+preset.slice(1)}</span></button>;})}</div>
  {!id?<p className="notice">Saved custom appearance. Choose a preset to use the simplified quota color controls.</p>:<><p className="help">Editing {theme=== "light"?"Light":"Dark"} colors{matchesTaskbar?" (matching Windows taskbar)":""} for {id[0].toUpperCase()+id.slice(1)}. Each preset remembers its own Light and Dark choices.</p><label className="check-field"><input type="checkbox" checked={custom.enabled} disabled={pending} onChange={e=>update({enabled:e.target.checked})}/>Use custom quota colors</label>{custom.enabled&&<div className="quota-custom-editor" aria-busy={pending}>{(["fiveHour","weekly"] as const).map(role=><div className="quota-swatch-field" key={role}><span id={role+"-label"}>{role==="fiveHour"?"5-hour quota":"Weekly quota"}</span><div className="quota-swatches" role="radiogroup" aria-labelledby={role+"-label"}>{quotaSwatches(value,theme,role).map(color=><button type="button" role="radio" key={color} aria-checked={palette[role]===color} data-color={color} aria-label={quotaColorName(color)+" ("+color+")"} title={quotaColorName(color)+" · "+color} tabIndex={palette[role]===color?0:-1} onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","ArrowUp","ArrowDown","Home","End"].includes(event.key))return;event.preventDefault();const choices=quotaSwatches(value,theme,role),index=choices.indexOf(color),next=event.key==="Home"?0:event.key==="End"?choices.length-1:(index+(["ArrowRight","ArrowDown"].includes(event.key)?1:choices.length-1))%choices.length;update({[role]:choices[next]});event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("button")[next]?.focus();}} disabled={pending} style={{backgroundColor:color,color:contrast(color,"#FFFFFF")>=4.5?"#FFFFFF":"#000000"}} onClick={()=>update({[role]:color})}>{palette[role]===color?"✓":""}</button>)}</div></div>)}<button type="button" disabled={pending} onClick={()=>{const p=presetPalette(id,theme);update({fiveHour:p.fiveHour,weekly:p.weekly});}}>Load preset colors</button><p className="help">{stripMode ? "Taskbar Strip uses these colors for its quota bars." : "Indicators use your selected color. Percentage text uses a related readable shade when needed."}</p></div>}</>}
  <label className="check-field"><input id="show-widget-border" type="checkbox" checked={value.showBorder} disabled={pending} onChange={e=>onChange({...draft,widgetAppearance:{...value,showBorder:e.target.checked}})}/>Show widget border</label><p className="help">Applies to widgets and hover summaries. Taskbar Strip uses this preference for its opaque card when Border is set to Automatic.</p>
  </fieldset>
  {stripMode && <fieldset><legend>Taskbar Strip</legend>
    <label className="check-field"><input id="taskbar-strip-in-taskbar" type="checkbox" checked={value.taskbarStripInTaskbar} disabled={pending} onChange={e=>changeStrip({taskbarStripInTaskbar:e.target.checked})}/>Place inside Windows taskbar (experimental)</label>
    <p className="help">Matches the Windows taskbar’s light or dark appearance inside it. The strip uses the app theme when placed above it, including when auto-hide is on, the taskbar is vertical, or the strip does not fit.</p>
    <label className="check-field"><input id="taskbar-strip-transparent" type="checkbox" checked={value.taskbarStripTransparent} disabled={pending || !value.taskbarStripInTaskbar} onChange={e=>changeStrip({taskbarStripTransparent:e.target.checked})}/>Transparent background inside taskbar</label>
    <p className="help">Lets the Windows taskbar show through when transparency is available.</p>
    {transparencyFallback && <p className="help" role="status">{transparencyFallback}</p>}
    <label className="field-label" htmlFor="taskbar-strip-border">Border</label>
    <select id="taskbar-strip-border" value={value.taskbarStripBorder ?? "automatic"} disabled={pending} aria-describedby="taskbar-strip-border-help" onChange={e=>changeStrip({taskbarStripBorder:e.target.value as Settings["widgetAppearance"]["taskbarStripBorder"]})}>
      <option value="automatic">Automatic</option><option value="show">Show</option><option value="hide">Hide</option>
    </select>
    <p id="taskbar-strip-border-help" className="help">Automatic follows the widget border preference on opaque cards and hides the outline on transparent backgrounds. Windows high contrast keeps an outline visible.</p>
    <label className="check-field"><input id="taskbar-strip-show-bars" type="checkbox" checked={value.taskbarStripShowBars} disabled={pending} onChange={e=>changeStrip({taskbarStripShowBars:e.target.checked})}/>Show micro quota bars</label>
    <p className="help">Adds a thin remaining-quota bar under each row. Widget colors controls the bar colors.</p>
  </fieldset>}
  <fieldset><legend>Compact countdown readability</legend>
    {([["compactCountdownBold","Bold countdown text"],["compactCountdownHighContrast","Higher contrast countdown text"]] as const).map(([key,label])=><label className="check-field" key={key}><input type="checkbox" checked={value[key]} disabled={pending} onChange={e=>onChange({...draft,widgetAppearance:{...value,[key]:e.target.checked}})}/>{label}</label>)}
    <p className="help">Applies to all Taskbar Strip text, including percentages, and to reset countdowns on Edge and HUD. Windows high contrast uses system colors.</p>
  </fieldset>
  <fieldset><legend>Compact widget size</legend><p className="help">Scale the whole Edge, HUD, or Taskbar Strip widget from 80% to 300%, keeping its layout. Size is additional to Windows scaling and applies across themes. Save keeps changes; Cancel restores saved sizes.</p>
    {(["screen-edge","compact-hud","taskbar-strip"] as const).map(mode=>{
      const key=scaleKey(mode),max=fittingScale(draft,mode,budgetFor(draft,mode,state.appearance.compactSizing??{},state.appearance.compactMonitorBudgets),state.appearance.textScale??100);
      const invalid=!Number.isInteger(value[key])||value[key]<80||value[key]>300||scaleIssues(draft,state.settings,state.appearance.compactSizing??{},state.appearance.textScale??100,state.appearance.compactMonitorBudgets).some(issue=>issue.field==="widgetAppearance."+key);
      const budget=state.appearance.compactSizing?.[mode],label=mode==="screen-edge"?"Edge":mode==="compact-hud"?"HUD":"Taskbar Strip";
      const update=(percent:number)=>onChange({...draft,widgetAppearance:{...value,[key]:percent}});
      return <div className="widget-size-option" key={mode}><div className="snap-gap-heading"><label htmlFor={key}>{label} size (%)</label><button type="button" className="text-button" disabled={pending} onClick={()=>update(100)}>Reset to 100%</button></div>
        <div className="snap-gap-controls"><input type="range" aria-label={label+" size"} min={80} max={Math.max(80,max)} step={1} disabled={pending||max<80} value={Number.isFinite(value[key])?Math.max(80,Math.min(max,value[key])):100} onChange={e=>update(Number(e.target.value))}/><input id={key} type="number" min={80} max={Math.min(300,max)} step={1} disabled={pending} value={Number.isFinite(value[key])?value[key]:""} aria-invalid={invalid} aria-describedby={key+"-help"} onChange={e=>update(e.target.value===""?NaN:Number(e.target.value))}/></div>
        <p id={key+"-help"} className={invalid?"field-error":"help"}>{invalid?(Number.isInteger(value[key])&&value[key]>=80&&value[key]<=300?"This layout does not fit. ":"Enter a whole percentage from 80 to 300. "):""}{max<80?"This layout cannot fit at the minimum 80% on its monitor.":"Maximum for this layout and monitor: "+max+"%."}</p>
        {budget?.fallbackReason&&<p className="notice" role="status">{label} saved size {budget.requestedPercent}% does not fit this monitor. Temporarily using {budget.effectivePercent}%; the saved size will return when it fits.{budget.effectivePercent>budget.maxPercent&&" Even the fallback size exceeds the available space."}</p>}
      </div>;
    })}
  </fieldset></>;
}
