import { compactLayout } from "./compactLayout.ts";
import type { Settings } from "./state.ts";
import type { AppearanceIssue } from "./widgetAppearance.ts";

export type CompactMode = "screen-edge" | "compact-hud";
export interface MonitorBudget { availableWidth:number; availableHeight:number }
export interface ScaleState extends MonitorBudget {
  requestedPercent:number; effectivePercent:number; maxPercent:number;
  availableWidth:number; availableHeight:number; fallbackReason:string|null;
}
export const scaleKey = (mode:CompactMode) => mode === "screen-edge" ? "edgeScalePercent" : "hudScalePercent";
export function budgetFor(draft:Settings,mode:CompactMode,budgets:Partial<Record<CompactMode,ScaleState>>,monitors:Record<string,MonitorBudget>={}) {
  return monitors[draft.windows.placements?.[mode]?.monitor??""]??monitors[""]??budgets[mode];
}
export function fittingScale(draft:Settings, mode:CompactMode, budget:MonitorBudget|undefined, textScale:number):number {
  if(!budget)return 300; // Browser fixtures have no native monitor constraint.
  const size=compactLayout(mode,draft.windows.quotaLabelStyle,draft.windows.resetCountdowns[mode],true,draft.windows.visiblePeriods?.[mode]??"both")!;
  const text=Math.max(1,textScale)/100,gap=2*draft.windows.snapGapLogicalPx;
  return Math.max(0,Math.min(300,Math.floor(Math.min((budget.availableWidth-gap)/(size.width*text),(budget.availableHeight-gap)/(size.height*text))*100+1e-8)));
}
export function scaleIssues(draft:Settings,saved:Settings,budgets:Partial<Record<CompactMode,ScaleState>>,textScale:number,monitors:Record<string,MonitorBudget>={}):AppearanceIssue[] {
  return (["screen-edge","compact-hud"] as const).flatMap(mode=>{
    const key=scaleKey(mode),value=draft.widgetAppearance[key],max=fittingScale(draft,mode,budgetFor(draft,mode,budgets,monitors),textScale);
    const layout=(s:Settings)=>compactLayout(mode,s.windows.quotaLabelStyle,s.windows.resetCountdowns[mode],true,s.windows.visiblePeriods?.[mode]??"both");
    const changed=value!==saved.widgetAppearance[key]||JSON.stringify(layout(draft))!==JSON.stringify(layout(saved))||draft.windows.snapGapLogicalPx!==saved.windows.snapGapLogicalPx||draft.windows.placements?.[mode]?.monitor!==saved.windows.placements?.[mode]?.monitor;
    return changed&&Number.isInteger(value)&&value>=80&&value<=300&&value>max?[{field:"widgetAppearance."+key,message:`This layout fits up to ${max}% on its monitor. Choose a smaller size or another monitor.`}]:[];
  });
}
