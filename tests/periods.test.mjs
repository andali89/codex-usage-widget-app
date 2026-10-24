import test from "node:test";
import assert from "node:assert/strict";
import { showsPeriod, effectiveResets } from "../src/periods.ts";
import { compactLayout } from "../src/compactLayout.ts";

test("period visibility overrides saved countdowns without changing those preferences", () => {
  for (const selection of ["both","five-hour","weekly"]) {
    for(const fiveHour of [false,true]) for(const weekly of [false,true]) {
      const saved={fiveHour,weekly}, copy={...saved};
      assert.deepEqual(effectiveResets(selection,saved),{fiveHour:selection!=="weekly"&&fiveHour,weekly:selection!=="five-hour"&&weekly});
      assert.deepEqual(saved,copy);
      assert.equal(showsPeriod(selection,300),selection!=="weekly");
      assert.equal(showsPeriod(selection,10080),selection!=="five-hour");
      for(const unknown of [null,17,60]) assert.equal(showsPeriod(selection,unknown),false);
    }
  }
});

test("single-period rectangles exclude hidden countdown space for both label styles", () => {
  for(const mode of ["screen-edge","compact-hud"]) for(const style of ["words","badges"]) {
    for(const selection of ["five-hour","weekly"]) {
      const hiddenOnly=selection==="five-hour"?{fiveHour:false,weekly:true}:{fiveHour:true,weekly:false};
      const noResets=compactLayout(mode,style,undefined,false,selection);
      assert.deepEqual(compactLayout(mode,style,hiddenOnly,false,selection),noResets);
      const both=compactLayout(mode,style,undefined,false,"both");
      assert.ok(mode==="screen-edge"?noResets.height<both.height:noResets.width<both.width);
    }
  }
});
