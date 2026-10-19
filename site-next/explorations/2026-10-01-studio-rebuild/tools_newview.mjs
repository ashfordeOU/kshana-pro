// Stamps out the files of a new Studio kind view, so a page for a kind that has just arrived starts from a working,
// honest generic reader and grows its own charts afterwards. Run: node tools_newview.mjs <id> "<Title>" "<The question the page answers>" <kind> <scenario-id>[,<scenario-id>...] [--out <dir>]
//   <id>        folder and file stem, lower-case with hyphens (e.g. ssv-sky); the page is <id>/index.html
//   <kind>      the engine's scenario kind the page reads (the reader refuses a result of another kind)
// Writes <id>/index.html, <id>/<id>.js, <id>/<id>.css, lib/<id>.mjs and lib/<id>.test.mjs. Nothing is wired in: the checklist it
// prints (selfcheck's own list and base-href rule, the Studio menu, port_site.py STUDIO_PAGES, site.test.mjs VIEWS, ci.yml) is the
// rest. The generic reader shows only what a result holds: its label, advisory text, summary, scalar fields and the size of every
// list; it draws nothing and invents nothing. Replace `readView` and `chartsOf` with the kind's own once its result shape is known.
import { mkdirSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const oi = args.indexOf("--out"); const out = oi >= 0 ? args.splice(oi, 2)[1] : new URL(".", import.meta.url).pathname;
const [id, title, question, kind, scen] = args;
if (!id || !title || !question || !kind || !scen || !/^[a-z][a-z0-9-]*$/.test(id)) { console.error('usage: node tools_newview.mjs <id> "<Title>" "<Question>" <kind> <scenario-id[,..]> [--out <dir>]'); process.exit(2); }
const scenarios = scen.split(",").map((s) => s.trim()).filter(Boolean);
const P = id.slice(0, 2) + "k"; // a short prefix for the page's class names, e.g. "ssk"
const Name = id.split("-").map((w) => w[0].toUpperCase() + w.slice(1)).join("");
const here = new URL(".", import.meta.url).pathname;
const w = (rel, text) => { const p = join(out, rel); if (existsSync(p)) { console.error(`refusing to overwrite ${rel}`); process.exit(1); } mkdirSync(join(p, ".."), { recursive: true }); writeFileSync(p, text); console.log("wrote " + rel); };
const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/"/g, "&quot;");

// ---- page: the head, header and theme switch are those of the Optical PNT page, which every view shares
const opt = readFileSync(join(here, "optical/index.html"), "utf8");
const head = opt.slice(0, opt.indexOf('<main id="main"'))
  .replace(/Optical PNT: a page of the Studio\./, `${title}: a page of the Studio.`).replace(/Optical PNT · Kshana Studio/, `${esc(title)} · Kshana Studio`)
  .replace(/<meta name="description" content="[^"]*">/, `<meta name="description" content="${esc(question)} Read from the results the engine recorded; nothing is uploaded.">`)
  .replace(/optical\/optical\.css/, `${id}/${id}.css`).replace(/lib\/optical\.mjs/, `lib/${id}.mjs`).replace(/<span>Optical PNT<\/span>/, `<span>${esc(title)}</span>`);
w(`${id}/index.html`, head + `<main id="main" tabindex="-1">
<div class="wrap ${P}">
  <p class="kicker"><i style="--c:var(--orb)"></i>${esc(title)}</p>
  <h1 id="${P}-h">${esc(question)}</h1>
  <p class="lede">Every number below is copied from a result the engine recorded; the page scales it and draws it, and computes nothing else.</p>
  <p class="${P}-adv" role="note"><b>Advisory software.</b> Kshana is not type-approved navigation equipment (IEC 61108, IEC 61162) and has not been assessed against any performance standard. Every figure carries the label the ledger holds for it (<b>MODELLED</b> unless an external oracle was used); it is an aid to judgement, not a finding about any place, mission or event.</p>
  <section class="card ${P}-pick" aria-label="Scenario"><div class="${P}-tabs" id="${P}-tabs" role="tablist" aria-label="${esc(title)} scenarios"></div><p class="${P}-blurb" id="${P}-blurb"></p><p class="${P}-status" id="${P}-status" role="status" aria-live="polite">Opening the scenarios…</p></section>
  <div id="${P}-body" hidden>
    <section class="card" aria-labelledby="${P}-facts-h"><div class="card-head"><h2 id="${P}-facts-h" class="h3">What the result holds</h2><p class="card-note" id="${P}-summary"></p></div><dl class="${P}-kv" id="${P}-kv"></dl></section>
    <div class="${P}-grid" id="${P}-charts"></div>
    <p class="${P}-prov" id="${P}-prov"></p>
  </div>
</div>
</main>
<script type="module" src="${id}/${id}.js"></script>
</body>
</html>
`);

// ---- reader (generic) and charts (none yet)
w(`lib/${id}.mjs`, `// SPDX-License-Identifier: AGPL-3.0-only
// The ${title} page's reader. It reads the engine's recorded result of a \`${kind}\` scenario (the \`json\` field of recorded/<name>.json)
// and returns what it holds. GENERIC: it lists the result's label, advisory text, summary, scalar fields and the size of each list. The
// kind's own reader and charts replace \`readView\` and \`chartsOf\` once the result shape is known; until then nothing is drawn and nothing
// is made up. Pure; tested in ${id}.test.mjs.

export const KIND = ${JSON.stringify(kind)};
export const SCENARIOS = Object.freeze([
${scenarios.map((s) => `  { id: ${JSON.stringify(s)}, title: ${JSON.stringify(s)}, blurb: "" },`).join("\n")}
]);

const isNum = (x) => typeof x === "number" && Number.isFinite(x);
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

// Scalar fields of an object, one level of nesting, as [path, value] rows (numbers, strings and booleans only); lists are counted.
export function scalarRows(obj, max = 80) {
  const rows = [], lists = [];
  const walk = (o, path, depth) => {
    for (const [k, v] of Object.entries(o || {})) {
      const p = path ? \`\${path}.\${k}\` : k;
      if (Array.isArray(v)) lists.push([p, v.length]);
      else if (v && typeof v === "object") { if (depth < 2) walk(v, p, depth + 1); }
      else if (isNum(v) || typeof v === "string" || typeof v === "boolean") rows.push([p, typeof v === "string" ? v.slice(0, 160) : String(v)]);
    }
  };
  walk(obj, "", 0);
  return { rows: rows.slice(0, max), lists };
}

// The recorded document -> { model } or { error }. A result of another kind is refused.
export function readView(doc) {
  let j;
  try { j = typeof doc.json === "string" ? JSON.parse(doc.json) : doc.json; } catch (e) { return { error: \`the result is not JSON (\${e.message})\` }; }
  if (!j || typeof j !== "object" || j.kind !== KIND) return { error: \`this is not a \${KIND} result\` };
  const { rows, lists } = scalarRows(j);
  return { model: { label: String(j.label || ""), advisory: String(j.advisory || ""), summary: String(doc.summary || ""), engine: String(doc.engine_version || ""), file: String(doc.file || ""), rows, lists } };
}

export const factRows = (m) => [...m.rows.map(([k, v]) => [k, v]), ...m.lists.map(([k, n]) => [k, \`a list of \${n}\`])];
// The kind's charts: none yet. Each chart is { id, title, note, svg, wide }.
export function chartsOf(model, opts = {}) { void model; void opts; return []; }
export { esc };
`);
w(`lib/${id}.test.mjs`, `// SPDX-License-Identifier: AGPL-3.0-only
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { KIND, SCENARIOS, readView, scalarRows, factRows } from "./${id}.mjs";

const here = new URL("../", import.meta.url).pathname;
const load = (id) => JSON.parse(readFileSync(here + \`recorded/\${id}.json\`, "utf8"));

test("every scenario of the page has a recorded result of the page's kind, read without invention", () => {
  for (const sc of SCENARIOS) {
    assert.ok(existsSync(here + \`recorded/\${sc.id}.json\`), \`\${sc.id} is recorded\`);
    const doc = load(sc.id), raw = JSON.parse(doc.json), { model, error } = readView(doc);
    assert.equal(error, undefined, sc.id); assert.equal(raw.kind, KIND);
    assert.ok(model.rows.length > 0);
    const text = JSON.stringify(raw); // a value shown must occur in the result as it is (keys may themselves contain dots, so paths are not walked)
    for (const [k, v] of model.rows) assert.ok(text.includes(v.length === 160 ? v : v.replace(/"/g, '\\"')), \`\${k} = \${v} occurs in the result\`);
    assert.deepEqual(model.rows, scalarRows(raw).rows);
  }
});
test("a result of another kind, or not JSON, is refused with a reason", () => {
  assert.match(readView({ json: JSON.stringify({ kind: "other" }) }).error, /not a/);
  assert.match(readView({ json: "{nope" }).error, /not JSON/);
});
test("scalars and list sizes are listed, and nothing deeper than two levels", () => {
  const { rows, lists } = scalarRows({ a: 1, b: { c: "x", d: { e: 2, f: { g: 3 } } }, l: [1, 2] });
  assert.deepEqual(rows, [["a", "1"], ["b.c", "x"], ["b.d.e", "2"]]); assert.deepEqual(lists, [["l", 2]]);
  assert.equal(factRows({ rows, lists })[3][1], "a list of 2");
});
`);

// ---- page script and styles
w(`${id}/${id}.js`, `// SPDX-License-Identifier: AGPL-3.0-only
// ${title}: the DOM layer. What is read and drawn is in lib/${id}.mjs. The page fetches only this folder's own recorded results
// (recorded/<scenario>.json); nothing leaves the page.
import { SCENARIOS, readView, chartsOf, factRows } from "../lib/${id}.mjs";

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
let current = null, lastModel = null;
const setStatus = (msg, err) => { const s = $("${P}-status"); s.textContent = msg; s.classList.toggle("err", !!err); };

async function open(id) {
  const sc = SCENARIOS.find((s) => s.id === id) || SCENARIOS[0];
  current = sc.id;
  for (const b of $("${P}-tabs").children) { b.setAttribute("aria-selected", String(b.dataset.id === sc.id)); b.tabIndex = b.dataset.id === sc.id ? 0 : -1; }
  $("${P}-blurb").textContent = sc.blurb;
  setStatus(\`Opening \${sc.title}…\`);
  try {
    const r = await fetch(\`recorded/\${sc.id}.json\`);
    if (!r.ok) throw new Error("the recorded result did not load");
    const { model: m, error } = readView(await r.json());
    if (error) throw new Error(error);
    if (current !== sc.id) return;
    draw(m);
    setStatus(\`\${sc.title}: the engine's recorded result (version \${m.engine}).\`);
    try { const q = new URLSearchParams(location.search); q.set("scenario", sc.id); history.replaceState(null, "", \`\${location.pathname}?\${q}\`); } catch (e) { /* no history API */ }
  } catch (e) { setStatus(\`\${sc.title} could not be opened: \${e && e.message ? e.message : e}.\`, true); }
}
function draw(m) {
  lastModel = m;
  $("${P}-body").hidden = false;
  $("${P}-summary").textContent = m.summary;
  $("${P}-kv").innerHTML = factRows(m).map(([k, v]) => \`<div><dt>\${esc(k)}</dt><dd>\${esc(v)}</dd></div>\`).join("");
  $("${P}-charts").innerHTML = chartsOf(m, { wide: matchMedia("(min-width: 901px)").matches }).map((c) => \`<section class="card ${P}-card\${c.wide ? " ${P}-wide" : ""}" aria-labelledby="${P}-c-\${c.id}"><h3 id="${P}-c-\${c.id}">\${esc(c.title)}</h3><p class="card-note">\${esc(c.note || "")}</p>\${c.svg}</section>\`).join("");
  $("${P}-prov").textContent = \`\${m.label} \${m.advisory}\`.trim();
}
function wire() {
  try { matchMedia("(min-width: 901px)").addEventListener("change", () => { if (lastModel) draw(lastModel); }); } catch (e) { /* no matchMedia */ }
  $("${P}-tabs").replaceChildren(...SCENARIOS.map((s) => { const b = document.createElement("button"); b.type = "button"; b.setAttribute("role", "tab"); b.className = "${P}-tab"; b.dataset.id = s.id; b.textContent = s.title; b.addEventListener("click", () => open(s.id)); return b; }));
  $("btn-theme").addEventListener("click", () => {
    const root = document.documentElement;
    const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
    root.dataset.theme = dark ? "light" : "dark";
    try { localStorage.setItem("kshana-theme", root.dataset.theme); } catch (e) { /* storage blocked */ }
  });
}
wire();
open(new URLSearchParams(location.search).get("scenario") || SCENARIOS[0].id);
`);
w(`${id}/${id}.css`, `/* ${title} page. Colours, type and radii come from tokens.css; layout from app.css and studio.css. A series is never told apart by colour alone. */
.${P}{padding-bottom:56px}
.${P} h1{font:700 clamp(28px,4.4vw,46px)/1.05 var(--f-display);letter-spacing:-.02em;margin:10px 0 8px}
.${P} .lede{max-width:78ch;margin:0 0 14px;color:var(--ink-2);font-size:16px;line-height:1.5}
.${P} .h3{font:650 17px/1.2 var(--f-display)}
.${P} h3{margin:0 0 6px;font:650 15px/1.2 var(--f-display)}
.${P}-adv{margin:0 0 16px;padding:12px 14px;border-radius:14px;border:1px solid var(--line-2);border-left:4px solid var(--orb-card);background:var(--surface);font-size:13.5px;line-height:1.5;color:var(--ink-2);max-width:100ch}
.${P}-adv b{color:var(--ink)}
.${P}-pick{margin-bottom:14px}
.${P}-tabs{display:flex;flex-wrap:wrap;gap:8px}
.${P}-tab{min-height:44px;padding:0 16px;border-radius:999px;border:1px solid var(--line-2);background:var(--surface);font-weight:600;font-size:13.5px;color:var(--ink-2)}
.${P}-tab[aria-selected=true]{background:var(--ink);color:var(--bg);border-color:var(--ink)}
.${P}-tab:focus-visible{outline:2px solid var(--tim);outline-offset:2px}
.${P}-blurb{margin:10px 0 0;font-size:13.5px;color:var(--ink-2);max-width:90ch}
.${P}-status{margin:8px 0 0;font-size:13px;color:var(--ink-2)}.${P}-status.err{color:var(--int);font-weight:600}
.${P} .card{margin-bottom:14px;min-width:0}
.${P}-kv{display:grid;grid-template-columns:repeat(auto-fill,minmax(210px,1fr));gap:8px 20px;margin:12px 0 0}
.${P}-kv dt{font:600 11.5px var(--f-text);color:var(--ink-3);text-transform:uppercase;letter-spacing:.04em;overflow-wrap:anywhere}
.${P}-kv dd{margin:2px 0 0;font:500 13px var(--f-mono);color:var(--ink);overflow-wrap:anywhere}
.${P}-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px;margin-bottom:14px;align-items:start}
.${P}-card{min-width:0}.${P}-wide{grid-column:1/-1}
@media (max-width:900px){.${P}-grid{grid-template-columns:minmax(0,1fr)}}
.${P}-prov{margin:0 0 14px;font-size:12.5px;color:var(--ink-3);max-width:110ch}
`);

console.log(`
Wiring checklist for ${id} (${Name}), none of it done by this tool:
  1. selfcheck.mjs: add "lib/${id}.mjs", "${id}/index.html", "${id}/${id}.js", "${id}/${id}.css" to the own list; add the page to the base-href/advisory loop and the relative-fetch loop; "lib/${id}.mjs" to the no-request list.
  2. index.html: a link in the Views menu and in the list of pages.
  3. kshana web/tools/port_site.py STUDIO_PAGES ("${id}/index.html") and web/site.test.mjs VIEWS ("studio/${id}/index.html"); .github/workflows/ci.yml: a step for web/studio/lib/${id}.test.mjs.
  4. catalog / recorded: the scenarios ${scenarios.join(", ")} must be in scenarios/, lib/catalog.mjs and recorded/ (tools_record.mjs), then tools_areas, tools_bundle, tools_links.
  5. Replace readView/chartsOf with the kind's own once the result shape is known, and take screenshots at 1440 and 390 px, light and dark.`);
