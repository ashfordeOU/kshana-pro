// Browser check: the 0.36 skyline scenarios with the Studio's 0.35 package (lib/catalog.mjs NEEDS_ENGINE). Each opens
// in the Simple and Advanced views, shows its recording with the "needs engine 0.36" notice, and is never run live
// (an older engine refuses its fields, or runs without them). Once pkg/ is 0.36 this check no longer applies.
//   python3 -m http.server 8871 --bind 127.0.0.1   (from the Studio folder)
//   node REVIEW/newer-engine-check.mjs [base-url]
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { chromium } = require("/opt/node-tools/node_modules/playwright");
const base = (process.argv.slice(2).find((a) => /^https?:/.test(a)) || "http://127.0.0.1:8871/").replace(/\/?$/, "/");
const b = await chromium.launch(); let fails = 0;
for (const view of ["", "advanced/"]) for (const n of ["orbit-urban-canyon-skyline", "jamming-harbour-approach-skyline", "integrity-harbour-approach", "leo-pvt-fjord-skyline", "gnss-sim-container-terminal-skyline", "leo-vertical-autonomous-vehicle-skyline"]) {
  const p = await (await b.newContext({ viewport: { width: 1360, height: 900 } })).newPage();
  const errs = []; p.on("pageerror", (e) => errs.push(String(e)));
  await p.route(/fonts\.(googleapis|gstatic)\.com/, (r) => r.abort());
  await p.goto(`${base}${view}?scenario=${n}`, { waitUntil: "load" });
  await p.waitForTimeout(12000);
  const t = (await p.evaluate(() => document.body.innerText)).replace(/\s+/g, " ");
  const title = { "orbit-urban-canyon-skyline": "City street availability", "jamming-harbour-approach-skyline": "Ship jammed entering harbour", "integrity-harbour-approach": "ARAIM entering harbour", "leo-pvt-fjord-skyline": "Ferry deep in a fjord", "gnss-sim-container-terminal-skyline": "Straddle carrier on a container quay", "leo-vertical-autonomous-vehicle-skyline": "Vehicle in a street, not a flat cut-off" }[n];
  const i = t.indexOf(title);
  const bad = /TOML parse error|invalid [a-z-]+ scenario|Ran locally/i.exec(t);
  const said = /needs engine 0\.36 or later; the engine running here is v0\.35\.0\. What you see is the run recorded with an engine build at commit 7b5cbf96/.test(t);
  const ok = !errs.length && i >= 0 && !bad && said; if (bad) console.log("   MATCH:", t.slice(i + bad.index - 200, i + bad.index + 200));
  if (!ok) fails++;
  console.log(ok ? "ok  " : "FAIL", view || "simple", n, errs.slice(0, 2).join(" | "), t.slice(0, 160));
}
await b.close();
console.log(fails ? `NEWER-ENGINE CHECK: ${fails} failure(s)` : "NEWER-ENGINE CHECK: all checks passed");
process.exit(fails ? 1 : 0);
