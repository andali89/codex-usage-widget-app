import defaults from "./appearance-defaults.json" with { type: "json" };
import presets from "./appearance-presets.json" with { type: "json" };
import swatches from "./quota-swatches.json" with { type: "json" };
import type { CSSProperties } from "react";

export const colorRoles = ["background", "contentSurface", "text", "secondaryText", "fiveHour", "weekly", "track", "border"] as const;
export type ColorRole = typeof colorRoles[number];
export type Palette = Record<ColorRole, string>;
export type PaletteTheme = "light" | "dark";
export type PresetId = keyof typeof presets;
export const presetIds = Object.keys(presets) as PresetId[];
export interface QuotaCustomization { enabled:boolean; fiveHour?:string; weekly?:string }
export interface WidgetAppearanceSettings { presetId?:PresetId|null; quotaCustomizations?:Partial<Record<PresetId,Partial<Record<PaletteTheme,QuotaCustomization>>>>; backgroundTransparencyPercent:number; colorMode:"theme-defaults"|"custom"; palettes:Record<PaletteTheme,Partial<Palette>> }
export interface AppearanceIssue { field:string; message:string }
export const colorRoleLabels:Record<ColorRole,string>={background:"Widget background",contentSurface:"Content surface",text:"Main text",secondaryText:"Secondary text",fiveHour:"5-hour quota",weekly:"Weekly quota",track:"Track",border:"Border and divider"};
export interface AppearanceIssueGroup { field:string; label:string; message:string; details:string[] }

export function groupAppearanceIssues(issues:AppearanceIssue[]):AppearanceIssueGroup[] {
  const grouped=new Map<string,AppearanceIssue[]>();
  for(const issue of issues) {const existing=grouped.get(issue.field)??[];existing.push(issue);grouped.set(issue.field,existing);}
  return [...grouped].map(([field,items])=>{
    const [, , theme, role]=field.split(".");
    const label=field==="windows.snapGapLogicalPx"?"Snap gap":field==="widgetAppearance.backgroundTransparencyPercent"?"Background transparency":role && role in colorRoleLabels?`${colorRoleLabels[role as ColorRole]} (${theme==="dark"?"Dark":"Light"})`:"Widget colors";
    const details=[...new Set(items.map(issue=>issue.message.replace(/\b(fiveHour|contentSurface|secondaryText)\b/g,role=>colorRoleLabels[role as ColorRole].toLowerCase())))];
    const message=details.length>1?(role==="fiveHour"||role==="weekly"?"Choose a color with stronger contrast, or use the theme default. The selected color must keep percentage text readable on every surface.":"Choose colors that meet the required contrast, or use the theme default."):details[0];
    return {field,label,message,details};
  });
}
export const summarizeAppearanceIssues=(issues:AppearanceIssue[])=>groupAppearanceIssues(issues).map(group=>`${group.label}: ${group.message}`).join(" ");

export interface EffectiveWidgetAppearance { originalStructure?:boolean; palette:Palette; divider:string; frameBorder:string; quotaText:Record<"fiveHour"|"weekly",string>; backgroundAlpha:number; backingAlpha:number; reason:string|null; compositionAvailable:boolean; protectContent:boolean }
export const defaultWidgetAppearance = ():WidgetAppearanceSettings => ({presetId:"original",quotaCustomizations:{},backgroundTransparencyPercent:0,colorMode:"theme-defaults",palettes:{light:{},dark:{}}});
export const themePalette = (theme:PaletteTheme):Palette => ({...defaults[theme]});
export const validHex = (value:string) => /^#[0-9a-f]{6}$/i.test(value);
export function paletteFor(settings:WidgetAppearanceSettings, theme:PaletteTheme):Palette {
  const palette=settings.presetId?presetPalette(settings.presetId,theme):themePalette(theme);
  if(settings.presetId) {const custom=settings.quotaCustomizations?.[settings.presetId]?.[theme]; if(custom?.enabled) for(const role of ["fiveHour","weekly"] as const) if(custom[role]&&validHex(custom[role]!))palette[role]=custom[role]!.toUpperCase(); return palette;}
  if(settings.colorMode==="custom") for(const role of colorRoles) { const color=settings.palettes[theme][role]; if(color && validHex(color)) palette[role]=color.toUpperCase(); }
  return palette;
}
function rgb(hex:string) { return [1,3,5].map(offset=>parseInt(hex.slice(offset,offset+2),16)/255); }
function luminance(channels:number[]) { return channels.reduce((sum,c,i)=>sum+[0.2126,0.7152,0.0722][i]*(c<=0.04045?c/12.92:((c+0.055)/1.055)**2.4),0); }
const ratio=(a:number,b:number)=>(Math.max(a,b)+0.05)/(Math.min(a,b)+0.05);
export const contrast=(a:string,b:string)=>ratio(luminance(rgb(a)),luminance(rgb(b)));
export function worstContrast(foreground:string, backing:string, alpha:number) {
  const color=rgb(backing), low=luminance(color.map(c=>c*alpha)), high=luminance(color.map(c=>c*alpha+1-alpha)), fg=luminance(rgb(foreground));
  return fg>=low && fg<=high ? 1 : Math.min(ratio(fg,low),ratio(fg,high));
}
/** Mix in sRGB, with identical integer rounding in the native resolver. */
export function mixColor(a:string,b:string,amount:number):string {
  const aa=rgb(a),bb=rgb(b);
  return "#"+aa.map((c,i)=>Math.round(255*(c*(1-amount)+bb[i]*amount)).toString(16).padStart(2,"0")).join("").toUpperCase();
}
export function quotaPaint(palette:Palette,theme:PaletteTheme) {
  const surfaces=[palette.contentSurface,...(theme==="dark"?["#081426","#10213A","#193452","#10233E"]:["#F3F6FB","#FFFFFF","#E8EFF8","#F6FAFF"])];
  const readable=(color:string,backgrounds:string[],minimum:number)=> {
    const target=theme==="dark"?"#FFFFFF":"#000000";
    for(let step=0;step<=100;step++) {const shade=mixColor(color,target,step/100);if(backgrounds.every(bg=>contrast(shade,bg)+1e-8>=minimum))return shade;}
    return target;
  };
  const text={} as Record<"fiveHour"|"weekly",string>;
  for(const role of ["fiveHour","weekly"] as const) {
    text[role]=readable(palette[role],surfaces,4.5);
  }
  return {text};
}
function legacyIssues(settings:WidgetAppearanceSettings):AppearanceIssue[] {
  const issues:AppearanceIssue[]=[];
  if(!Number.isInteger(settings.backgroundTransparencyPercent)||settings.backgroundTransparencyPercent<0||settings.backgroundTransparencyPercent>80) issues.push({field:"widgetAppearance.backgroundTransparencyPercent",message:"Use a whole percentage from 0 to 80."});
  for(const theme of ["light","dark"] as const) {
    const base=themePalette(theme), overrides=settings.palettes[theme], palette={...base};
    const field=(role:string)=>`widgetAppearance.palettes.${theme}.${role}`;
    if(Object.keys(overrides).length>colorRoles.length) {issues.push({field:`widgetAppearance.palettes.${theme}`,message:"Only the eight supported color roles are allowed."});continue;}
    for(const [role,value] of Object.entries(overrides)) {
      if(!colorRoles.includes(role as ColorRole)||!validHex(value)) issues.push({field:field(role),message:"Choose a supported color role and a #RRGGBB color without alpha."});
      else palette[role as ColorRole]=value;
    }
    const pairs:[ColorRole,ColorRole,number][]=[...["text","secondaryText","fiveHour","weekly"].map(fg=>[fg,"contentSurface",4.5] as [ColorRole,ColorRole,number]),["fiveHour","track",3],["weekly","track",3],["border","contentSurface",3]];
    for(const [fg,bg,minimum] of pairs) { const r=contrast(palette[fg],palette[bg]); if(r+1e-8<minimum) issues.push({field:field(overrides[fg]?fg:bg),message:`${fg} on ${bg}: ${r.toFixed(2)}:1; needs ${minimum}:1. Use theme default or adjust this pair.`}); }
    for(const role of ["fiveHour","weekly"] as const) {
      for(const background of theme==="dark"?["#081426","#10213A","#193452","#10233E"]:["#F3F6FB","#FFFFFF","#E8EFF8","#F6FAFF"]) { const r=contrast(palette[role],background); if(r+1e-8<4.5) issues.push({field:field(role),message:`${role} on dashboard ${background}: ${r.toFixed(2)}:1; needs 4.5:1.`}); }
      const r=contrast(palette[role],base.track); if(r+1e-8<3) issues.push({field:field(role),message:`${role} on dashboard track: ${r.toFixed(2)}:1; needs 3:1.`});
    }
    for(const foreground of theme==="dark"?["#FBBF24","#FCA5A5","#34D399","#60A5FA"]:["#92400E","#B91C1C","#047857","#005FCC"]) if(contrast(foreground,palette.contentSurface)+1e-8<4.5) issues.push({field:field("contentSurface"),message:"Content surface must retain readable status text and keyboard focus. Use theme default."});
  }
  return issues;
}
export const presetPalette=(id:PresetId,theme:PaletteTheme):Palette=>({...themePalette(theme),...presets[id][theme]});
export function appearanceIssues(settings:WidgetAppearanceSettings):AppearanceIssue[] {
  if(!settings.presetId)return legacyIssues(settings);
  if(!presetIds.includes(settings.presetId))return [{field:"widgetAppearance.presetId",message:"Choose an available preset."}];
  const issues:AppearanceIssue[]=legacyIssues(settings);
  const maps=settings.quotaCustomizations??{};
  for(const id of new Set<PresetId>([settings.presetId,...Object.keys(maps) as PresetId[]])) {
    if(!presetIds.includes(id)) {issues.push({field:"widgetAppearance.quotaCustomizations",message:"Choose an available preset."});continue;}
    for(const theme of ["light","dark"] as const) {
      const custom=maps[id]?.[theme];
      const palette={...presetPalette(id,theme)};
      for(const role of ["fiveHour","weekly"] as const)if(custom?.[role]!=null)palette[role]=custom[role];
      for(const role of ["fiveHour","weekly"] as const) if(!validHex(palette[role])) issues.push({field:`widgetAppearance.quotaCustomizations.${id}.${theme}.${role}`,message:"Choose a #RRGGBB color without alpha."});

    }
  }
  return issues;
}
export function quotaSwatches(settings:WidgetAppearanceSettings,theme:PaletteTheme,role:"fiveHour"|"weekly"):string[] {
  const id=settings.presetId;if(!id)return [];
  const selected=settings.quotaCustomizations?.[id]?.[theme]??{enabled:false};
  const colors=[...swatches[theme].map(([,color])=>color),...(["fiveHour","weekly"] as const).map(role=>presetPalette(id,theme)[role])];
  if(selected[role])colors.push(selected[role]!);
  return [...new Set(colors.map(color=>color.toUpperCase()))].filter(color=>!appearanceIssues({...settings,quotaCustomizations:{[id]:{[theme]:{...selected,enabled:true,[role]:color}}}}).length);
}
export function resolveWidgetAppearance(settings:WidgetAppearanceSettings, theme:PaletteTheme, highContrast=false, transparencyAllowed=true, compositionAvailable=true):EffectiveWidgetAppearance {
  const palette=highContrast?themePalette(theme):paletteFor(settings,theme);
  const reason=highContrast?"high-contrast":!transparencyAllowed?"windows-transparency-off-or-unknown":!compositionAvailable?"native-composition-unverified":null;
  const backgroundAlpha=reason?1:1-Math.min(80,Math.max(0,settings.backgroundTransparencyPercent||0))/100;
  const originalStructure=settings.presetId==="original"||(!settings.presetId&&settings.colorMode==="theme-defaults");
  const divider=!originalStructure || (!highContrast && settings.colorMode==="custom" && settings.palettes[theme].border) ? palette.border : theme==="dark"?"#334B6B":"#CBD5E1";
  const frameBase=originalStructure?divider:palette.border;
  const frameBorder=theme==="light"&&!highContrast&&!!settings.presetId&&settings.presetId!=="original"?"transparent":theme==="dark"&&(!settings.presetId||settings.presetId==="original")?frameBase:mixColor(frameBase,palette.background,0.35);
  const quota=quotaPaint(palette,theme);
  return {frameBorder,quotaText:quota.text,originalStructure:settings.presetId==="original"||(!settings.presetId&&settings.colorMode==="theme-defaults"),palette,divider,backgroundAlpha,backingAlpha:backgroundAlpha,reason,compositionAvailable,protectContent:false};
}
export function widgetStyle(effective:EffectiveWidgetAppearance, _shell=true):CSSProperties {
  const p=effective.palette;
  const paint=(color:string,alpha:number)=>`rgba(${rgb(color).map(c=>Math.round(c*255)).join(",")},${alpha})`;
  const dark=p.background==="#0A1B32";
  const tint=effective.originalStructure?(dark?"#10233E":"#F6FAFF"):p.contentSurface;
  return {
    "--widget-background":p.background,"--widget-content":p.contentSurface,"--widget-text":p.text,"--widget-secondary":p.secondaryText,"--widget-track":p.track,"--widget-border":p.border,"--widget-divider":effective.divider,"--widget-frame":effective.frameBorder,
    "--widget-base-paint":paint(p.background,1),"--widget-surface-paint":paint(p.contentSurface,1),
    "--widget-wash-paint":paint(tint,1),"--widget-decorative-raised-paint":paint(p.contentSurface,1),"--widget-group-paint":"transparent",
    "--widget-reading-paint":effective.originalStructure?(dark?"#10213A":"#FFFFFF"):p.contentSurface,
    "--quota-five-hour":p.fiveHour,"--quota-weekly":p.weekly,
    "--quota-five-hour-text":effective.quotaText.fiveHour,"--quota-weekly-text":effective.quotaText.weekly,
  } as CSSProperties;
}
export const quotaColorName=(color:string)=>[...swatches.light,...swatches.dark].find(([,hex])=>hex===color)?.[0]??"Theme color";
export const appearanceReason=(reason:string|null)=> reason==="high-contrast"?"Currently opaque because Windows high contrast is on.":reason==="windows-transparency-off-or-unknown"?"Currently opaque because Windows transparency is off or unavailable.":reason?"Native transparency is unavailable; the widget remains opaque.":"";
