// Studio inventory: domains, scenarios, inputs per scenario, by importing Studio's own modules (read-only).
// Run: node inventory.mjs [studioDir]  -> writes inventory.json
import fs from "node:fs"; import path from "node:path"; import { pathToFileURL } from "node:url";
const dir = process.argv[2] || path.resolve(process.env.HOME, "Code/kshana-site-next/explorations/2026-09-30-studio-dash");
const imp = (p) => import(pathToFileURL(path.join(dir, p)).href);
const C = await imp("lib/catalog.mjs"), Gd = await imp("lib/guided.mjs"), P = await imp("lib/params.mjs"), K = await imp("lib/kinds.mjs");
const groups = JSON.parse(fs.readFileSync(path.join(dir, "groups.json"))).groups || [];
for (const g of groups) { const gi = JSON.parse(fs.readFileSync(path.join(dir, g, "index.json"))); C.registerGroup({ ...gi, dir: g }); }
const rows = [];
for (const s of C.SCENARIOS) {
  const file = s[0]; const e = C.entryFor(file); const dirp = C.dirOf(file);
  const p = path.join(dir, dirp, "scenarios", path.basename(file));
  const alt = path.join(dir, dirp, path.basename(file));
  const tp = fs.existsSync(p) ? p : (fs.existsSync(alt) ? alt : null);
  if (!tp) { rows.push({ file, domain: s[1], missing: true }); continue; }
  const toml = fs.readFileSync(tp, "utf8");
  const kind = (toml.match(/^\s*kind\s*=\s*"([^"]+)"/m) || [])[1];
  const fields = P.numericFields(toml);
  const guided = K.hasCapability(kind) ? K.guidedFor(kind, fields).length : Gd.knobsForToml(toml).length;
  const lines = toml.split("\n");
  rows.push({ file, domain: s[1], title: e && e.title, kind, guided, allFields: fields.length, tomlLines: lines.length,
    commentLines: lines.filter((l) => /^\s*#/.test(l)).length, notInBrowser: !!C.NOT_IN_BROWSER[file], questionWords: (e && e.question || "").split(/\s+/).length });
}
const ok = rows.filter((r) => !r.missing);
const stat = (k) => { const v = ok.map((r) => r[k]).sort((a, b) => a - b); return { min: v[0], median: v[Math.floor(v.length / 2)], max: v[v.length - 1], mean: +(v.reduce((a, b) => a + b, 0) / v.length).toFixed(1) }; };
const out = { studioDir: dir, domains: C.DOMAINS.length + groups.length, domainsWithScenarios: new Set(C.SCENARIOS.map((s) => s[1])).size,
  scenarios: C.SCENARIOS.length, missing: rows.filter((r) => r.missing).length, notInBrowser: Object.keys(C.NOT_IN_BROWSER).length,
  defaultScenario: C.DEFAULT_SCENARIO, capabilityKinds: Object.keys(K.CAPABILITIES).length,
  guided: stat("guided"), allFields: stat("allFields"), tomlLines: stat("tomlLines"),
  scenariosOver5AllFields: ok.filter((r) => r.allFields > 5).length, scenariosWithZeroGuided: ok.filter((r) => r.guided === 0).length,
  perDomain: Object.fromEntries([...new Set(C.SCENARIOS.map((s) => s[1]))].map((d) => [d, C.SCENARIOS.filter((s) => s[1] === d).length])), rows };
fs.writeFileSync("inventory.json", JSON.stringify(out, null, 1));
const { rows: _r, ...summary } = out; console.log(JSON.stringify(summary, null, 1));
