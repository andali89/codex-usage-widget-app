// Run from the repository root: node docs/assets/hud-warning-preview/generate.mjs
// Review-only snapshots rendered from production components; no app files change.
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createElement as h } from "react";
import { renderToStaticMarkup as render } from "react-dom/server";
import { build, createServer } from "vite";

const candidates = [
  { id: "current", name: "Current", extra: 0 },
  { id: "a", name: "A · +4px", extra: 4 },
  { id: "b", name: "B · +8px", extra: 8 },
  { id: "c", name: "C · +12px", extra: 12 },
];
const readings = ["stale", "long", "blocked", "missing"];
const now = Date.UTC(2026, 9, 28, 12);
const server = await createServer({ server: { middlewareMode: true, hmr: false }, appType: "custom" });
let Contents, compactLayout, appearance;
try {
  Contents = (await server.ssrLoadModule("/src/WidgetContents.tsx")).default;
  ({ compactLayout } = await server.ssrLoadModule("/src/compactLayout.ts"));
  appearance = await server.ssrLoadModule("/src/widgetAppearance.ts");
} finally {
  await server.close();
}
const bundled = await build({ logLevel: "error", build: { write: false, rollupOptions: { input: "src/styles.css" } } });
const css = bundled.output.find(asset => asset.type === "asset" && asset.fileName.endsWith(".css")).source;
const brand = `data:image/png;base64,${(await readFile("src/assets/brand-ring.png")).toString("base64")}`;
const { defaultWidgetAppearance, resolveWidgetAppearance, widgetStyle } = appearance;

function widget(candidate, labels, theme, reading) {
  const retained = reading === "stale" || reading === "long", missing = reading === "missing", long = reading === "long";
  const windows = missing ? [] : [300, 10080].map((durationMinutes, i) => ({
    key: `codex/${i ? "secondary" : "primary"}/${durationMinutes}`, limitId: "codex", label: i ? "Weekly" : "5-hour",
    durationMinutes, remainingPercent: long ? 99.9 : i ? 42 : 68, usedPercent: long ? .1 : i ? 58 : 32,
    resetsAtMs: now + (long ? i ? 86_340_000 : 17_940_000 : i ? 390_000_000 : 8_220_000),
    validatedAtMs: now, quality: "valid", lastKnown: retained,
  }));
  const resets = { fiveHour: true, weekly: true };
  const state = {
    settings: { mode: "compact-hud", windows: { visiblePeriods: { "compact-hud": "both" }, resetCountdowns: { "compact-hud": resets }, quotaLabelStyle: labels, lockPosition: false } },
    usage: { windows, freshness: retained ? "stale" : missing ? "empty" : "fresh", connection: missing ? "auth-required" : retained ? "offline" : "ready", resetCreditsAvailable: null, resetCredits: null, ordinaryUsageAllowed: reading === "blocked" ? false : null, refreshInFlight: false },
    presentation: { previewSource: "shell" }, trayAvailable: true,
  };
  // Preserve the selection snapshot's original 20px warning allowance.
  const normal = compactLayout("compact-hud", labels, resets, false, "both");
  const base = { ...normal, width: normal.width + 20 };
  const paint = resolveWidgetAppearance(defaultWidgetAppearance(), theme, false, false);
  return render(h("div", {
    className: `native-surface shell stable-compact mode-compact-hud option-${candidate.id}`,
    "data-label-style": labels, "data-paint": theme, "data-reading": reading, "data-extra": candidate.extra,
    "aria-label": `${candidate.name}, ${labels}, synthetic ${reading} example`,
    style: { ...widgetStyle(paint), "--compact-width": `${(base.width + candidate.extra) / 16}rem`, "--compact-height": `${base.height / 16}rem`, "--warning-extra-gap": `${candidate.extra / 16}rem` },
  }, h(Contents, { state, surface: "shell", now }))).replaceAll('src="/src/assets/brand-ring.png"', 'data-brand-image="true"');
}

const cards = candidates.map(candidate => `<article data-option="${candidate.id}">
<header><h2>${candidate.name}</h2><p>${candidate.extra ? `Current spacing + ${candidate.extra} logical px` : "Unmodified production spacing"}</p></header>
<div class="stage">${["words", "badges"].flatMap(labels => ["light", "dark"].flatMap(theme => readings.map(reading => widget(candidate, labels, theme, reading)))).join("")}</div>
<p class="size" data-extra="${candidate.extra}"></p>
</article>`).join("");

const html = `<!doctype html><html lang="en" data-theme="dark"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>HUD warning gap candidates</title>
<style>${css}
body { padding:24px; min-width:320px; }
main { max-width:920px; margin:auto; }
h1 { font-size:25px; margin-bottom:7px; }
.intro { color:var(--secondary); margin-bottom:18px; font-size:13px; }
.controls { display:flex; flex-wrap:wrap; gap:14px; margin-bottom:20px; }
.controls label { display:grid; gap:5px; font-size:12px; }
.controls select { background:var(--surface); color:var(--text); border:1px solid var(--divider); border-radius:6px; padding:6px 9px; min-height:34px; }
.comparison { display:grid; gap:12px; }
article { padding:13px 16px 10px; background:var(--surface); border:1px solid var(--divider); border-radius:10px; min-width:0; }
article header { display:flex; gap:14px; align-items:baseline; margin-bottom:13px; }
article h2 { font-size:17px; font-weight:650; }
article header p { font-size:12px; color:var(--secondary); }
.stage { overflow-x:auto; overflow-y:hidden; padding:3px 0; }
.stage .shell { cursor:default; zoom:2; }
.mode-compact-hud .status-warning { margin-left:var(--warning-extra-gap); }
.size { margin-top:7px; font-size:11px; color:var(--secondary); }
.note { margin-top:14px; font-size:12px; color:var(--secondary); }
@media(max-width:560px) { body { padding:14px; } article { padding-inline:10px; } article header { flex-wrap:wrap; gap:5px 12px; } }
</style></head><body><main><h1>HUD · warning icon spacing</h1>
<p class="intro">Current + three wider gaps. Quota content stays in place; width grows only by the extra gap.</p>
<div class="controls">
<label>Appearance<select id="theme"><option value="dark">Dark</option><option value="light">Light</option></select></label>
<label>Quota labels<select id="labels"><option value="words">Words</option><option value="badges">Badges</option></select></label>
<label>Magnification<select id="zoom"><option value="2">2× comparison</option><option value="1">1× logical size</option></select></label>
<label>Warning example<select id="reading"><option value="stale">Last known values</option><option value="long">Long last known countdowns</option><option value="blocked">Usage blocked</option><option value="missing">Usage unavailable</option></select></label>
</div><div class="comparison">${cards}</div>
<p class="note">Synthetic data · both periods and countdowns enabled · preview only; application code is unchanged.</p>
</main><script>
document.querySelectorAll('[data-brand-image]').forEach(image => { image.src = ${JSON.stringify(brand)}; });
const theme=document.getElementById('theme'), labels=document.getElementById('labels'), zoom=document.getElementById('zoom'), reading=document.getElementById('reading');
function update() {
  document.documentElement.dataset.theme=theme.value;
  document.querySelectorAll('.stage .shell').forEach(widget => {
    widget.style.display=widget.dataset.labelStyle===labels.value && widget.dataset.paint===theme.value && widget.dataset.reading===reading.value ? 'flex' : 'none';
    widget.style.zoom=zoom.value;
  });
  document.querySelectorAll('.size').forEach(size => { const extra=Number(size.dataset.extra), base=labels.value==='words'?340:304; size.textContent=(base+extra)+' × 38 logical px'+(extra?' · width +'+extra+'px':' · current size'); });
}
[theme,labels,zoom,reading].forEach(control=>control.addEventListener('change',update));update();
</script></body></html>`;
await mkdir("docs/assets/hud-warning-preview", { recursive: true });
await writeFile("docs/hud-warning-spacing-comparison.html", html);
console.log("Generated docs/hud-warning-spacing-comparison.html; no application source changed.");
