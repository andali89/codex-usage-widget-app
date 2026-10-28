import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {budgetFor,fittingScale,scaleIssues} from "../src/compactSizing.ts";
import {compactLayout} from "../src/compactLayout.ts";
import {defaultWidgetAppearance,appearanceIssues,resolveWidgetAppearance,presetIds,contrast} from "../src/widgetAppearance.ts";

const layouts=JSON.parse(readFileSync(new URL("./fixtures/compact-scaling.json",import.meta.url),"utf8"));
const draftFor=f=>({widgetAppearance:defaultWidgetAppearance(),windows:{snapGapLogicalPx:8,placements:{},quotaLabelStyle:f.labels,visiblePeriods:{[f.mode]:f.periods},resetCountdowns:{[f.mode]:f.resets}}});
test("shared native/layout fixtures preserve current Edge/HUD geometry, including warning space",()=>{
  for(const f of layouts)assert.deepEqual(compactLayout(f.mode,f.labels,f.resets,true,f.periods),{width:f.width,height:f.height});
});
test("80–300 percent custom sizing composes with text scaling and exact monitor limits",()=>{
  for(const f of layouts)for(const percent of [80,100,125,137,150,200,300])for(const text of [100,150,225]) {
    const draft=draftFor(f),budget={availableWidth:f.width*text/100*percent/100+16,availableHeight:f.height*text/100*percent/100+16};
    assert.equal(fittingScale(draft,f.mode,budget,text),percent);
    draft.widgetAppearance[f.mode==="screen-edge"?"edgeScalePercent":"hudScalePercent"]=percent;
    assert.deepEqual(scaleIssues(draft,draftFor(f),{[f.mode]:budget},text),[]);
    budget.availableWidth-=1;
    if(percent!==100)assert.equal(scaleIssues(draft,draftFor(f),{[f.mode]:budget},text).length,1);
  }
});
test("saved oversize settings survive a smaller monitor, while new sizes are rejected",()=>{
  const f=layouts[0],saved=draftFor(f);saved.widgetAppearance.edgeScalePercent=200;
  const budgets={"screen-edge":{availableWidth:70,availableHeight:500}};
  assert.deepEqual(scaleIssues(structuredClone(saved),saved,budgets,100),[]);
  const draft=structuredClone(saved);draft.widgetAppearance.edgeScalePercent=150;
  assert.match(scaleIssues(draft,saved,budgets,100)[0].message,/fits up to 100%/);
  draft.widgetAppearance.edgeScalePercent=80;
  assert.deepEqual(scaleIssues(draft,saved,budgets,100),[]);
  assert.equal(draft.widgetAppearance.hudScalePercent,100);
});
test("unsaved monitor selections use their native work-area budget, including primary fallback",()=>{
  const draft=draftFor(layouts[0]);
  draft.windows.placements={"screen-edge":{monitor:"large"}};
  const monitors={"":{availableWidth:100,availableHeight:300},large:{availableWidth:1000,availableHeight:2000}};
  assert.equal(budgetFor(draft,"screen-edge",{},monitors),monitors.large);
  draft.windows.placements["screen-edge"].monitor="removed";
  assert.equal(budgetFor(draft,"screen-edge",{},monitors),monitors[""]);
});
test("readability remains independent across themes, with stronger built-in text contrast",()=>{
  for(const presetId of presetIds)for(const theme of ["light","dark"])for(const bold of [false,true])for(const high of [false,true]) {
    const settings={...defaultWidgetAppearance(),presetId,compactCountdownBold:bold,compactCountdownHighContrast:high};
    const resolved=resolveWidgetAppearance(settings,theme);
    assert.equal(resolved.compactCountdownBold,bold);assert.equal(resolved.compactCountdownHighContrast,high);
    for(const bg of [resolved.palette.background,resolved.palette.contentSurface]) {
      assert.ok(contrast(resolved.palette.text,bg)>=7);
      assert.ok(contrast(resolved.palette.text,bg)>contrast(resolved.palette.secondaryText,bg));
    }
    assert.equal(resolveWidgetAppearance(settings,theme,true).compactCountdownBold,bold);
  }
  for(const value of [79,301,80.5,NaN,Infinity,"150",null]) {
    const settings={...defaultWidgetAppearance(),edgeScalePercent:value};
    assert.ok(appearanceIssues(settings).some(i=>i.field.endsWith("edgeScalePercent")));
  }
  for(const value of [1,"true",null])assert.ok(appearanceIssues({...defaultWidgetAppearance(),compactCountdownBold:value}).length);
});
