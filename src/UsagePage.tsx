import { diagnostic, useVisibleClock, type AppState } from "./state";
import { quotaPending, usagePending } from "./summary";
import { useDiagnostics } from "./useDiagnostics";
import QuotaCard from "./QuotaCard";
import { EmptyArt, Icon } from "./visuals";
import ResetCreditsCard from "./ResetCreditsCard";

export default function UsagePage({ state, onSettings, onReminders, onLayoutChange }: { state: AppState; onSettings: () => void; onReminders: () => void; onLayoutChange: () => void }) {
  const now = useVisibleClock(state.dashboardVisible);
  useDiagnostics(state,"usage",now,state.dashboardVisible);
  const u = state.usage;
  const stale = u.freshness === "stale";
  const pending = usagePending(u);
  const connection = u.refreshInFlight ? "Checking usage…" : stale ? "Out of date · last known values" : u.connection === "ready" ? "Connected to local Codex" : u.connection === "starting" ? "Connecting to local Codex…" : pending ? "Rechecking local usage…" : u.connection === "auth-required" ? "Sign-in required" : "Usage connection unavailable";
  const actionableError = u.errorCode && !["session-scoped-identity", "reconciliation-pending"].includes(u.errorCode) && !(pending && u.connection === "offline");
  return <>
    <h1 className="sr-only" data-overview-heading tabIndex={-1}>Overview</h1>
    {state.firstLaunch && <section className="welcome-card"><EmptyArt /><div><h2>A quieter way to keep track.</h2><p>Keep your limits in view. Monitoring continues in the tray when you close this window.</p><button className="text-button" onClick={onSettings}>Make it yours <Icon name="chevron" /></button></div></section>}
    <div className="connection-line" role="status"><span className={`status-dot ${u.connection === "ready" && !stale ? "healthy" : "warning"}`} />{connection}<span className="spacer" />{u.planType && <span className="pill">{u.planType}</span>}</div>
    {u.ordinaryUsageAllowed === false && <div className="notice critical" role="status"><Icon name="warning" /><div><strong>{stale ? "Last known: ordinary usage was blocked." : "Ordinary usage is blocked."}</strong><p>A positive percentage does not confirm availability.</p></div></div>}
    {actionableError && <div className="notice warning"><Icon name="warning" /><div>{diagnostic(u.errorCode!, u.codexCliVersion)}{["codex-not-found", "select-native-codex-exe", "incompatible-codex-protocol"].includes(u.errorCode!) && <button className="text-button" onClick={onSettings}>Connection settings <Icon name="chevron" /></button>}</div></div>}
    {u.windows.some(w => w.remainingPercent !== null) ? <div className="quota-grid">{u.windows.map((quota, index) => <QuotaCard key={quota.key} quota={quota} stale={stale} now={now} index={index} pending={quotaPending(u, quota.key)} />)}</div> : <section className="empty-state"><EmptyArt /><h2>{pending ? "Getting things ready…" : "Usage unavailable"}</h2><p>{pending ? "Waiting for validated quota values. Automatic checks will continue." : u.connection === "auth-required" ? "Sign in through your local Codex installation, then refresh here." : "No validated quota values after repeated checks. Refresh to try again."}</p><span className="muted">Your data stays with your local Codex installation.</span></section>}
    {u.resetCreditsAvailable !== null && <ResetCreditsCard key={`${state.processEpoch}/${u.accountContextKey ?? "unknown"}/${state.dashboardRequest?.sequence ?? 0}`} available={u.resetCreditsAvailable} credits={u.resetCredits} stale={stale} onLayoutChange={onLayoutChange} />}
    {u.additionalLimits && <p className="notice">Additional limits available; not monitored in this version.</p>}
    <section className="reminder-summary"><Icon name="bell" /><div><h2>Reminders</h2><p>{state.reminders.deliveryPaused ? "Delivery paused" : !state.settings.reminders.enabled ? "Off" : "Monitoring enabled"}{state.reminders.diagnostic ? " · desktop delivery unavailable" : ""}{state.reminders.snoozedUntil && state.reminders.snoozedUntil > now ? ` · Snoozed until ${new Date(state.reminders.snoozedUntil).toLocaleString()}` : ""}</p></div><button className="text-button" onClick={onReminders}>Manage <Icon name="chevron" /></button></section>
    <section className="read-details" aria-label="Read details"><div className="footnote"><span>{u.lastSuccessAtMs ? `Last successful read ${new Date(u.lastSuccessAtMs).toLocaleTimeString()}` : "Waiting for the first successful read"}</span><span>{u.lastAttemptAtMs ? `Last attempt ${new Date(u.lastAttemptAtMs).toLocaleTimeString()}` : "No read attempted"}</span><span>{u.nextRefreshAtMs ? `Next check ${new Date(u.nextRefreshAtMs).toLocaleTimeString()}` : u.refreshInFlight ? "One shared read in progress" : "Automatic reads paused"}</span>{u.errorCode && !actionableError && <span>{diagnostic(u.errorCode, u.codexCliVersion)}</span>}</div></section>
  </>;
}
