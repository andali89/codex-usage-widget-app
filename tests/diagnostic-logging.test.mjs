import test from "node:test";
import assert from "node:assert/strict";
import { diagnosticDisplay, displayFlags, RenderCounts } from "../src/diagnosticEvents.ts";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";

const quota = { key:"codex/primary/300",durationMinutes:300,remainingPercent:60,quality:"valid",lastKnown:false };
const state = () => ({settings:{mode:"compact-hud",windows:{visiblePeriods:{}}},usage:{windows:[{...quota}],connection:"ready",freshness:"fresh",refreshInFlight:false,consecutiveUnavailableReads:0,missingWindowReads:{},ordinaryUsageAllowed:null}});
test("diagnostics distinguish hydration, quota checking, stale refresh and unavailable without changing state",()=>{
  assert.equal(diagnosticDisplay(null,"shell",true),129);
  const s=state(),before=structuredClone(s);
  assert.equal(diagnosticDisplay(s,"shell",true),128);assert.deepEqual(s,before);
  s.usage.freshness="stale";s.usage.refreshInFlight=true;
  assert.equal(diagnosticDisplay(s,"shell",true),128|4|8);
  s.usage.windows=[];assert.ok(diagnosticDisplay(s,"shell",true)&displayFlags.quotaLoading);
  s.usage.consecutiveUnavailableReads=3;
  assert.ok(diagnosticDisplay(s,"shell",true)&displayFlags.unavailable);
  assert.equal(diagnosticDisplay(s,"shell",false)&128,0);
});
test("compact diagnostics honor selected periods and partial field checking",()=>{
  const s=state();s.settings.windows.visiblePeriods["compact-hud"]="five-hour";
  s.usage.windows.push({...quota,key:"codex/secondary/10080",durationMinutes:10080,remainingPercent:null,quality:"missing"});
  s.usage.missingWindowReads["codex/secondary"]=1;
  assert.equal(diagnosticDisplay(s,"shell",true),128);
  assert.equal(diagnosticDisplay(s,"preview",true),128|2|16);
});
test("render summaries count clock and state inputs independently and reset counts",()=>{
  const counts=new RenderCounts();counts.commit(1,1000);counts.commit(1,2000);counts.commit(1,2000);counts.commit(2,2000);
  assert.deepEqual(counts.take(),{count:4,clockCount:2,stateCount:2});
  assert.deepEqual(counts.take(),{count:0,clockCount:0,stateCount:0});
  counts.commit(2,3000);assert.deepEqual(counts.take(),{count:1,clockCount:1,stateCount:0});
});
test("PowerShell summary separates sources and excludes censored durations from percentiles",{skip:process.platform!=="win32"},()=>{
  const dir=mkdtempSync(join(tmpdir(),"widget-diagnostic-summary-"));
  const event=(event,at,data)=>({format:1,session:"synthetic",segment:1,event,elapsed_ms:at,data});
  const records=[event("state_enter",100,{condition:"quota_loading",origin:"startup",left_censored:true}),event("state_exit",600,{condition:"quota_loading",duration_ms:500}),event("state_enter",1000,{condition:"stale",origin:"runtime",left_censored:false}),event("state_exit",2000,{condition:"stale",duration_ms:1000}),event("state_enter",3000,{condition:"stale",origin:"runtime",left_censored:false}),event("state_exit",6000,{condition:"stale",duration_ms:3000}),event("state_enter",7000,{condition:"stale",origin:"runtime",left_censored:false}),event("refresh_result",8000,{error:"protocol-timeout"}),event("segment_end",9000,{dropped_total:0})];
  try {
    writeFileSync(join(dir,"widget.jsonl"),records.map(r=>JSON.stringify(r)).join("\n")+"\n{truncated");
    const result=JSON.parse(execFileSync("powershell.exe",["-NoProfile","-File","scripts/summarize-diagnostics.ps1","-Directory",dir,"-AsJson"],{encoding:"utf8"}));
    const stale=result.summary.find(s=>s.condition==="stale");
    assert.equal(stale.episodes,3);assert.equal(stale.exact_completed,2);assert.equal(stale.median_ms,2000);assert.equal(stale.p95_ms,3000);assert.equal(stale.incomplete,1);assert.equal(stale.observed_total_ms,6000);
    assert.equal(result.summary.find(s=>s.condition==="quota_loading").exact_completed,0);assert.equal(result.malformed_lines,1);assert.equal(result.dropped_native_or_ingress,0);
    assert.equal(stale.entries_per_observed_hour,1213.483);
    records.push(event("events_dropped",9100,{total:2}));
    writeFileSync(join(dir,"widget.jsonl"),records.map(r=>JSON.stringify(r)).join("\n"));
    const gap=JSON.parse(execFileSync("powershell.exe",["-NoProfile","-File","scripts/summarize-diagnostics.ps1","-Directory",dir,"-AsJson"],{encoding:"utf8"}));
    assert.equal(gap.summary.find(s=>s.condition==="stale").exact_completed,0);
    assert.ok(gap.episodes.every(e=>!e.complete&&e.duration_ms===null));
  } finally {rmSync(dir,{recursive:true,force:true});}
});
