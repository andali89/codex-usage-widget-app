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
test("saved transparency preferences remain inactive under all effective overrides",()=>{
  for(const theme of ["light","dark"]) for(const transparency of [0,20,50,80]) {
    const value=defaultWidgetAppearance();value.backgroundTransparencyPercent=transparency;
    const original=structuredClone(value);
    for(const [contrast,allowed,composition,reason] of [[true,true,true,"high-contrast"],[false,false,true,"windows-transparency-off-or-unknown"],[false,true,false,"native-composition-unverified"]]) {
      const result=resolveWidgetAppearance(value,theme,contrast,allowed,composition);
      assert.equal(result.backgroundAlpha,1);assert.equal(result.reason,reason);
    }
    const restored=resolveWidgetAppearance(value,theme);
    assert.equal(restored.backgroundAlpha,1);assert.equal(restored.reason,"transparency-disabled");
    assert.equal(restored.backingAlpha,restored.backgroundAlpha);assert.deepEqual(value,original);
    assert.equal(restored.protectContent,false);
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
test("surface paints are opaque inputs to a single composited background layer",()=>{
  const value=defaultWidgetAppearance();value.backgroundTransparencyPercent=80;
  const resolved=resolveWidgetAppearance(value,"light");
  const style=widgetStyle(resolved);
  const decorativeAlpha=Number(style["--widget-decorative-raised-paint"].match(/,([\d.]+)\)$/)?.[1]);
  assert.equal(decorativeAlpha,1);
  assert.equal(style["--widget-surface-paint"],"rgba(255,255,255,1)");
  assert.equal(style["--widget-group-paint"],"transparent");
  assert.equal(style.opacity,undefined);
  const previewStyle=widgetStyle(resolved,false);
  assert.equal(previewStyle["--widget-decorative-raised-paint"],"rgba(255,255,255,1)");
});

for(const id of presetIds)for(const theme of ["light","dark"])test(id+" "+theme+" preset and every offered quota color meet contrast",()=>{
  const settings={...defaultWidgetAppearance(),presetId:id};
  assert.deepEqual(appearanceIssues(settings),[]);
  for(const role of ["fiveHour","weekly"]) {
    const choices=quotaSwatches(settings,theme,role);assert.ok(choices.includes(presetPalette(id,theme)[role]));assert.ok(choices.length>=24);
    for(const color of choices) {const customized={...settings,quotaCustomizations:{[id]:{[theme]:{enabled:true,[role]:color}}}};assert.deepEqual(appearanceIssues(customized),[]);
      const effective=resolveWidgetAppearance(customized,theme);
      for(const background of [effective.palette.contentSurface,...(theme==="dark"?["#081426","#10213A","#193452","#10233E"]:["#F3F6FB","#FFFFFF","#E8EFF8","#F6FAFF"])]) assert.ok(contrast(effective.quotaText[role],background)>=4.5);
    }
  }
});

test("showcase provides ten complete paired themes and keeps Original unchanged",()=>{
  assert.deepEqual(presetIds,["original","graphite","glacier","matcha","rosewood","terracotta","sapphire","sunrise","mint","orchid","slate"]);
  const reference=JSON.parse(readFileSync(new URL("./fixtures/original-v0.1.0-colors.json",import.meta.url),"utf8"));
  for(const theme of ["light","dark"]) {
    const original=presetPalette("original",theme);
    for(const [role,token] of Object.entries({background:"--shell",contentSurface:"--raised",text:"--text",secondaryText:"--secondary",fiveHour:"--accent",weekly:"--weekly",track:"--track",border:"--border"}))assert.equal(original[role],reference[theme][token]);
    const bodies=new Set();
    for(const id of presetIds.slice(1)) {
      const palette=presetPalette(id,theme);bodies.add(palette.background);
      for(const role of ["text","secondaryText","fiveHour","weekly"])assert.ok(contrast(palette[role],palette.contentSurface)>=4.5);
    }
    assert.equal(bodies.size,10,"each theme has its own body color");
  }
});
test("each preset and theme retains independent opt-in quota choices",()=>{
  const value=defaultWidgetAppearance();value.quotaCustomizations={original:{light:{enabled:true,fiveHour:"#6D28D9"},dark:{enabled:false,fiveHour:"#34D399"}},matcha:{light:{enabled:true,weekly:"#6D28D9"}}};
  assert.equal(paletteFor(value,"light").fiveHour,"#6D28D9");assert.equal(paletteFor(value,"dark").fiveHour,"#60A5FA");
  value.presetId="matcha";assert.equal(paletteFor(value,"light").weekly,"#6D28D9");assert.equal(paletteFor(value,"light").background,"#F0F6E4");
  value.presetId="original";value.quotaCustomizations.original.light.enabled=false;assert.equal(paletteFor(value,"light").fiveHour,"#005FCC");assert.equal(value.quotaCustomizations.original.light.fiveHour,"#6D28D9");
  value.quotaCustomizations.original.light.enabled=true;assert.equal(resolveWidgetAppearance(value,"light").originalStructure,true);
});
test("invalid dormant colors are rejected against their own preset and theme",()=>{
  const value=defaultWidgetAppearance();value.quotaCustomizations={matcha:{light:{enabled:false,fiveHour:"#FFF"}}};
  assert.ok(appearanceIssues(value).some(i=>i.field.includes("quotaCustomizations.matcha.light.fiveHour")));
});

const paintFixtures=JSON.parse(readFileSync(new URL("./fixtures/widget-derived-paint.json",import.meta.url),"utf8"));
for(const f of paintFixtures) test(`derived paint contract: ${f.id} ${f.theme} ${f.name}`,()=>{
  const value={...defaultWidgetAppearance(),presetId:f.id,showBorder:f.showBorder??true,quotaCustomizations:f.colors?{[f.id]:{[f.theme]:{enabled:true,...f.colors}}}:{}};
  const effective=resolveWidgetAppearance(value,f.theme);
  assert.equal(effective.frameBorder,f.frameBorder);assert.equal(effective.divider,f.divider);
  assert.deepEqual(effective.quotaText,f.quotaText);
  const surfaces=[effective.palette.contentSurface,...(f.theme==="dark"?["#081426","#10213A","#193452","#10233E"]:["#F3F6FB","#FFFFFF","#E8EFF8","#F6FAFF"])];
  for(const role of ["fiveHour","weekly"]) {
    if(f.colors)assert.equal(effective.palette[role],f.colors[role]);
    for(const bg of surfaces)assert.ok(contrast(effective.quotaText[role],bg)>=4.5);
  }
  if(f.id==="original"&&f.theme==="dark"&&f.showBorder!==false)assert.equal(effective.frameBorder,"#334B6B");
});

test("border preference defaults on and high contrast overrides off",()=>{
  assert.equal(defaultWidgetAppearance().showBorder,true);
  for(const id of presetIds) for(const theme of ["light","dark"]) {
    const value={...defaultWidgetAppearance(),presetId:id};
    const on=resolveWidgetAppearance(value,theme);
    assert.notEqual(on.frameBorder,"transparent");
    const off=resolveWidgetAppearance({...value,showBorder:false},theme);
    assert.equal(off.frameBorder,"transparent");
    assert.deepEqual({...off,frameBorder:on.frameBorder},on,"only the frame paint changes");
    const hc=resolveWidgetAppearance({...value,showBorder:false},theme,true);
    assert.notEqual(hc.frameBorder,"transparent");
    assert.equal(hc.backgroundAlpha,1);
  }
});
