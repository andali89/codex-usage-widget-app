import { useId, useState } from "react";
import type { Usage } from "./state";
import { resetTimestamp } from "./format";
import { Icon } from "./visuals";

const INITIAL_ROWS = 3;
const statusLabels: Record<string, string> = { available: "Available", redeeming: "Redeeming", redeemed: "Redeemed", unknown: "Unknown" };

export default function ResetCreditsCard({ available, credits, stale, onLayoutChange }: {
  available: number; credits: Usage["resetCredits"]; stale: boolean; onLayoutChange: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const listId = useId();
  const rows = credits?.slice(0, expanded ? undefined : INITIAL_ROWS);
  const hidden = Math.max(0, (credits?.length ?? 0) - INITIAL_ROWS);
  return <section className="credit-card">
    <span className="icon-badge"><Icon name="credits" /></span>
    <div><h2>{stale ? "Last known reset credits" : "Reset credits"}</h2><p>Review eligibility in Codex.</p></div>
    <strong>{available}</strong>
    {rows && <div className="credit-details">
      <ul id={listId} className="credit-list" role="list">{rows.map((credit, index) => <li className="credit-row" role="listitem" key={index}>
        <Icon name="credits" /><span className="credit-status">{statusLabels[credit.status] ?? "Unknown"}</span>
        <span className="credit-expiry"><Icon name="clock" />{credit.expiresAtMs === null ? "No expiry supplied" : <time dateTime={new Date(credit.expiresAtMs).toISOString()}>Expires {resetTimestamp(credit.expiresAtMs)}</time>}</span>
      </li>)}</ul>
      {!rows.length && <p>No credit details returned.</p>}
      {hidden > 0 && <button className="text-button credit-expand" type="button" aria-expanded={expanded} aria-controls={listId} onClick={() => { setExpanded(value => !value); onLayoutChange(); }}>
        {expanded ? "Show less" : `Show more (${hidden})`}<Icon name="chevron" />
      </button>}
    </div>}
  </section>;
}
