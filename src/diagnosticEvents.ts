import type { AppState } from "./state";
import { usagePending, quotaPending } from "./summary.ts";
import { showsPeriod } from "./periods.ts";

// Independent flags: loading and stale are not mutually exclusive.
export const displayFlags = { sharedLoading:1, quotaLoading:2, refreshing:4, stale:8, partial:16, unavailable:32, blocked:64, visible:128 } as const;
export function diagnosticDisplay(state:AppState|null,surface:"main"|"shell"|"preview"|"usage",visible:boolean):number {
  if(!state)return displayFlags.sharedLoading|(visible?128:0);
  const mode=state.settings.mode;
  const selection=mode==="screen-edge"||mode==="compact-hud"?state.settings.windows.visiblePeriods[mode]??"both":"both";
  const source=state.usage;
  const u=surface==="shell"?{...source,windows:source.windows.filter(w=>showsPeriod(selection,w.durationMinutes))}:source;
  const usable=u.windows.some(w=>w.remainingPercent!==null);
  return (visible?128:0)|(u.refreshInFlight?4:0)|
    (u.freshness==="stale"||u.windows.some(w=>w.lastKnown)?8:0)|
    ((!usable&&usagePending(u))||u.windows.some(w=>w.remainingPercent===null&&quotaPending(u,w.key))?2:0)|
    (usable&&u.windows.some(w=>w.remainingPercent===null||w.quality!=="valid")?16:0)|
    (!usable&&!usagePending(u)?32:0)|(u.ordinaryUsageAllowed===false?64:0);
}

export class RenderCounts {
  count=0;clockCount=0;stateCount=0;
  private previousRevision:number|undefined;
  private previousClock:number|undefined;
  commit(revision:number|undefined,clock:number|undefined) {
    this.count++;
    if(this.previousRevision!==revision)this.stateCount++;
    if(this.previousClock!==clock)this.clockCount++;
    this.previousRevision=revision;this.previousClock=clock;
  }
  take(){const result={count:this.count,clockCount:this.clockCount,stateCount:this.stateCount};this.count=0;this.clockCount=0;this.stateCount=0;return result;}
}
