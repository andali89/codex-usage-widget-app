import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { appearanceIssues, defaultWidgetAppearance, resolveWidgetAppearance, worstContrast, contrast, paletteFor, widgetStyle, presetIds, presetPalette, quotaSwatches } from "../src/widgetAppearance.ts";

const fixtures=JSON.parse(readFileSync(new URL("./fixtures/widget-appearance.json",import.meta.url),"utf8"));
for(const fixture of fixtures.palettes) test(`palette contract: ${fixture.name}`,()=>{
  const appearance=defaultWidgetAppearance();
  appearance.palettes[fixture.theme]=fixture.roles;
  // Retained custom maps are validated even with Theme defaults selected.
  assert.equal(appearanceIssues(appearance).length===0,fixture.valid);
});
test("worst-case contrast includes intermediate desktop backgrounds",()=>{
  assert.ok(contrast("#757575","#000000")>=4.5);
  assert.ok(contrast("#757575","#FFFFFF")>=4.5);
  for(const fixture of fixtures.composition) assert.ok(Math.abs(worstContrast(fixture.foreground,fixture.backing,fixture.alpha)-fixture.worst)<1e-6);
});
test("requested preferences survive all effective overrides and regain transparency",()=>{
  for(const theme of ["light","dark"]) for(const transparency of [0,20,80]) {
    const value=defaultWidgetAppearance();value.backgroundTransparencyPercent=transparency;
    const original=structuredClone(value);
    for(const [contrast,allowed,composition,reason] of [[true,true,true,"high-contrast"],[false,false,true,"windows-transparency-off-or-unknown"],[false,true,false,"native-composition-unverified"]]) {
      const result=resolveWidgetAppearance(value,theme,contrast,allowed,composition);
      assert.equal(result.backgroundAlpha,1);assert.equal(result.reason,reason);
    }
    const restored=resolveWidgetAppearance(value,theme);
    assert.equal(restored.backgroundAlpha,1-transparency/100);assert.equal(restored.reason,null);
    assert.equal(restored.backingAlpha,1);assert.deepEqual(value,original);
    for(const role of ["text","secondaryText","fiveHour","weekly"]) assert.ok(worstContrast(restored.palette[role],restored.palette.contentSurface,restored.backingAlpha)>=4.5);
  }
});
test("custom period colors are scoped separately from interface accent and unknown data",()=>{
  const value=defaultWidgetAppearance();value.presetId=null;value.colorMode="custom";value.palettes.light.fiveHour="#6d28d9";
  assert.equal(paletteFor(value,"light").fiveHour,"#6D28D9");
  assert.equal(paletteFor(value,"dark").fiveHour,"#60A5FA");
  const style=widgetStyle(resolveWidgetAppearance(value,"light"));
  assert.equal(style["--quota-five-hour"],"#6D28D9");assert.equal(style["--accent"],undefined);
  value.colorMode="theme-defaults";assert.equal(paletteFor(value,"light").fiveHour,"#005FCC");assert.equal(value.palettes.light.fiveHour,"#6d28d9");
});
test("transparency rejects fractional, empty/non-finite, and out of range drafts",()=>{
  for(const value of [-1,81,1.5,NaN,Infinity]) { const draft=defaultWidgetAppearance();draft.backgroundTransparencyPercent=value;assert.ok(appearanceIssues(draft).some(issue=>issue.field.endsWith("backgroundTransparencyPercent"))); }
});
test("requested transparency reaches nested decorative paint while content remains protected",()=>{
  const value=defaultWidgetAppearance();value.backgroundTransparencyPercent=80;
  const resolved=resolveWidgetAppearance(value,"light");
  const style=widgetStyle(resolved);
  const decorativeAlpha=Number(style["--widget-decorative-raised-paint"].match(/,([\d.]+)\)$/)?.[1]);
  assert.ok(Math.abs(decorativeAlpha-0.2)<1e-6);
  assert.equal(style["--widget-surface-paint"],"rgba(255,255,255,1)");
  assert.equal(style["--widget-group-paint"],"rgba(255,255,255,1)");
  assert.equal(style.opacity,undefined);
  const previewStyle=widgetStyle(resolved,false);
  assert.equal(previewStyle["--widget-decorative-raised-paint"],"rgba(255,255,255,1)");
});

for(const id of presetIds)for(const theme of ["light","dark"])test(id+" "+theme+" preset and every offered quota color meet contrast",()=>{
  const settings={...defaultWidgetAppearance(),presetId:id};
  assert.deepEqual(appearanceIssues(settings),[]);
  for(const role of ["fiveHour","weekly"]) {
    const choices=quotaSwatches(settings,theme,role);assert.ok(choices.includes(presetPalette(id,theme)[role]));
    for(const color of choices) {const customized={...settings,quotaCustomizations:{[id]:{[theme]:{enabled:true,[role]:color}}}};assert.deepEqual(appearanceIssues(customized),[]);}
  }
});
test("each preset and theme retains independent opt-in quota choices",()=>{
  const value=defaultWidgetAppearance();value.quotaCustomizations={original:{light:{enabled:true,fiveHour:"#6D28D9"},dark:{enabled:false,fiveHour:"#34D399"}},forest:{light:{enabled:true,weekly:"#6D28D9"}}};
  assert.equal(paletteFor(value,"light").fiveHour,"#6D28D9");assert.equal(paletteFor(value,"dark").fiveHour,"#60A5FA");
  value.presetId="forest";assert.equal(paletteFor(value,"light").weekly,"#6D28D9");assert.equal(paletteFor(value,"light").background,"#EAF4EC");
  value.presetId="original";value.quotaCustomizations.original.light.enabled=false;assert.equal(paletteFor(value,"light").fiveHour,"#005FCC");assert.equal(value.quotaCustomizations.original.light.fiveHour,"#6D28D9");
  value.quotaCustomizations.original.light.enabled=true;assert.equal(resolveWidgetAppearance(value,"light").originalStructure,true);
});
test("invalid dormant colors are rejected against their own preset and theme",()=>{
  const value=defaultWidgetAppearance();value.quotaCustomizations={forest:{light:{enabled:false,fiveHour:"#FFFFFF"}}};
  assert.ok(appearanceIssues(value).some(i=>i.field.includes("quotaCustomizations.forest.light.fiveHour")));
});
