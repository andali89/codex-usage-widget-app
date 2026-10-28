// Run from the repository root: node docs/assets/edge-sizing-preview/generate.mjs
// Generates a standalone review page from the real components and compiled CSS.
import { readFile, writeFile } from "node:fs/promises";
import { createElement as h } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { build, createServer } from "vite";

const candidates = [
  { id: "a", name: "A · Subtle reference", ring: 46, percent: 11.5, time: 11, padding: 3, note: "Spacing reference" },
  { id: "b", name: "B · Original Balanced", ring: 48, percent: 12.5, time: 11.5, padding: 2, note: "Previous preview" },
  { id: "b-spaced", name: "B · Revised spacing", ring: 48, percent: 12.5, time: 11.5, padding: 2, note: "+2.5px above the quota stack" },
];
const now = Date.UTC(2026, 9, 28, 12);
const samples = {
  normal: { values: [68, 42], offsets: [8_220_000, 390_000_000], retained: false },
  widest: { values: [99.9, 99.9], offsets: [17_940_000, 86_340_000], retained: true },
  fractional: { values: [0.5, 0], offsets: [30_000, -1_000], retained: false },
  missing: { values: [null, null], offsets: [null, null], retained: false },
};
const server = await createServer({ server: { middlewareMode: true }, appType: "custom" });
let readings, brand, compactLayout, appearance;
try {
  readings = (await server.ssrLoadModule("/src/CompactQuotaReadings.tsx")).default;
  brand = (await server.ssrLoadModule("/src/visuals.tsx")).BrandMark;
  ({ compactLayout } = await server.ssrLoadModule("/src/compactLayout.ts"));
  appearance = await server.ssrLoadModule("/src/widgetAppearance.ts");
} finally {
  await server.close();
}
const bundled = await build({
  logLevel: "error", build: { write: false, rollupOptions: { input: "src/styles.css" } },
});
const css = bundled.output.find(asset => asset.type === "asset" && asset.fileName.endsWith(".css")).source;
const brandImage = `data:image/png;base64,${(await readFile("src/assets/brand-ring.png")).toString("base64")}`;
const { widgetStyle, resolveWidgetAppearance, defaultWidgetAppearance } = appearance;
const widget = (candidate, labelStyle, sample, theme) => {
  const fixture = samples[sample];
  const windows = [300, 10080].map((durationMinutes, i) => ({
    key: `synthetic-${i}`, limitId: "codex", label: i ? "Weekly" : "5-hour",
    usedPercent: fixture.values[i] === null ? null : 100 - fixture.values[i],
    remainingPercent: fixture.values[i], durationMinutes,
    resetsAtMs: fixture.offsets[i] === null ? null : now + fixture.offsets[i],
    validatedAtMs: now, quality: fixture.values[i] === null ? "missing" : "valid", lastKnown: fixture.retained,
  }));
  const size = compactLayout("screen-edge", labelStyle, { fiveHour: true, weekly: true });
  const paint = resolveWidgetAppearance(defaultWidgetAppearance(), theme, false, false);
  return renderToStaticMarkup(h("div", {
    className: `native-surface shell stable-compact mode-screen-edge widget-palette widget-original option-${candidate.id}`,
    "data-label-style": labelStyle, "data-candidate": candidate.id,
    "data-sample": sample, "data-paint": theme,
    "aria-label": `${candidate.name}, ${labelStyle}, synthetic ${sample} example`,
    style: { ...widgetStyle(paint), "--compact-width": `${size.width / 16}rem`, "--compact-height": `${size.height / 16}rem` },
  }, h(brand, { small: true }), h("div", { className: "edge-values" }, h(readings, {
    mode: "screen-edge", windows, labelStyle, resets: { fiveHour: true, weekly: true },
    retained: fixture.retained, now,
  })))).replaceAll('src="/src/assets/brand-ring.png"', 'data-brand-image="true"');
};
const cards = candidates.map(candidate => `<article class="candidate" data-option="${candidate.id}">
  <h2>${candidate.name}</h2>
  <p class="spec">Ring <b>${candidate.ring}px</b> · % <b>${candidate.percent}px</b> · Time <b>${candidate.time}px</b></p>
  <div class="previews">${["words", "badges"].map(style => `<figure><figcaption>${style === "words" ? "Words" : "Badges"}</figcaption><div class="widget-stage">${Object.keys(samples).flatMap(sample => ["dark", "light"].map(theme => widget(candidate, style, sample, theme))).join("")}</div></figure>`).join("")}</div>
  <p class="footprint">54px wide · same outer height<br>${candidate.note}</p>
</article>`).join("");
const overrides = candidates.filter(c => c.id !== "current").map(c => `
.native-surface.shell.stable-compact.option-${c.id} { padding-inline:${c.padding / 16}rem; }
.option-${c.id} .quota-ring { width:${c.ring / 16}rem; height:${c.ring / 16}rem; flex:none; }
.option-${c.id} .ring-reading>strong { font-size:${c.percent / 16}rem; }
.option-${c.id} .reset-indicator { width:100%; }
.option-${c.id} .reset-countdown { width:${c.ring / 16}rem; font-size:${c.time / 16}rem; line-height:1.1; letter-spacing:-.02em; }
`).join("");
const html = `<!doctype html>
<html lang="en" data-theme="dark"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Balanced Edge spacing preview</title>
<style>${css}
${overrides}
/* Reuse the small gap seen above A's first ring, retaining B's sizes. */
.option-b-spaced .edge-values { transform:translateY(.15625rem); }
body { min-width:320px; padding:28px; }
main { max-width:880px; margin:auto; }
h1 { font-size:26px; margin-bottom:8px; }
.intro { color:var(--secondary); margin-bottom:18px; }
.controls { display:flex; flex-wrap:wrap; gap:16px; margin-bottom:24px; align-items:end; }
.controls label { display:grid; gap:6px; font-size:13px; }
.controls select { background:var(--surface); color:var(--text); border:1px solid var(--divider); border-radius:6px; padding:7px 10px; min-height:36px; }
.comparison { display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:14px; }
.candidate { padding:16px 10px 12px; border:1px solid var(--divider); border-radius:12px; background:var(--surface); }
.candidate h2 { text-align:center; font-size:18px; margin-bottom:6px; }
.spec { text-align:center; font-size:11px; color:var(--secondary); white-space:nowrap; }
.spec b { color:var(--text); font-weight:600; }
.previews { display:flex; justify-content:center; gap:20px; padding-top:18px; }
figure { margin:0; }
figcaption { text-align:center; font-size:12px; color:var(--secondary); margin-bottom:12px; }
.widget-stage { width:108px; height:468px; }
.widget-stage .shell { zoom:2; cursor:default; }
.widget-stage .shell:not([data-sample="normal"]) { display:none; }
html[data-theme="dark"] .shell[data-paint="light"], html[data-theme="light"] .shell[data-paint="dark"] { display:none; }
.footprint { margin-top:14px; font-size:11px; text-align:center; color:var(--secondary); }
.note { margin-top:18px; font-size:13px; color:var(--secondary); }
@media(max-width:880px) { .comparison { grid-template-columns:repeat(2,minmax(0,1fr)); } }
@media(max-width:580px) { body { padding:16px; } .comparison { grid-template-columns:1fr; } }
</style></head><body><main>
<h1>Balanced · a little more space above the rings</h1>
<p class="intro">Revised B keeps the Balanced ring and text sizes, with a top gap similar to Subtle.</p>
<div class="controls">
<label>Appearance<select id="theme"><option value="dark">Dark</option><option value="light">Light</option></select></label>
<label>Preview magnification<select id="zoom"><option value="2">2× · easier comparison</option><option value="1">1× · logical size</option></select></label>
<label>Example readings<select id="sample"><option value="normal">Typical values</option><option value="widest">Long countdowns / last known</option><option value="fractional">Less than 1% / reset due</option><option value="missing">Unavailable</option></select></label>
</div>
<div class="comparison">${cards}</div>
<p class="note">Synthetic examples · percentages show remaining quota; times show expected reset countdowns. Preview only; the application has not been changed.</p>
</main><script>
document.querySelectorAll("[data-brand-image]").forEach(image => { image.src = ${JSON.stringify(brandImage)}; });
const theme = document.getElementById("theme"), zoom = document.getElementById("zoom"), sample = document.getElementById("sample");
function update() {
  document.documentElement.dataset.theme = theme.value;
  document.querySelectorAll(".widget-stage").forEach(stage => { stage.style.width = 54 * Number(zoom.value) + "px"; stage.style.height = 234 * Number(zoom.value) + "px"; });
  document.querySelectorAll(".widget-stage .shell").forEach(widget => { widget.style.zoom = zoom.value; widget.style.display = widget.dataset.sample === sample.value && widget.dataset.paint === theme.value ? "flex" : "none"; });
}
[theme, zoom, sample].forEach(control => control.addEventListener("change", update));
update();
</script></body></html>`;
await writeFile("docs/edge-sizing-comparison.html", html);
console.log("Generated docs/edge-sizing-comparison.html from production components and CSS.");
