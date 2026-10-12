// Browser check: the 0.36 skyline scenarios and the lunar reference-station study with the Studio's 0.35 package (lib/catalog.mjs NEEDS_ENGINE). Each opens
// in the Simple and Advanced views, shows its recording with the "needs engine 0.36" notice, and is never run live
// (an older engine refuses its fields, or runs without them). Once pkg/ is 0.36 this check no longer applies.
//   python3 -m http.server 8871 --bind 127.0.0.1   (from the Studio folder)
//   node REVIEW/newer-engine-check.mjs [base-url] [--capable]
// --capable: the served pkg/ has the 0.36 skyline functions and the lunar kind, so each must run live with no notice.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { chromium } = require("/opt/node-tools/node_modules/playwright");
const base = (process.argv.slice(2).find((a) => /^https?:/.test(a)) || "http://127.0.0.1:8871/").replace(/\/?$/, "/");
const capable = process.argv.includes("--capable");
const b = await chromium.launch(); let fails = 0;
for (const view of ["", "advanced/"]) for (const n of ["orbit-urban-canyon-skyline", "jamming-harbour-approach-skyline", "integrity-harbour-approach", "leo-pvt-fjord-skyline", "gnss-sim-container-terminal-skyline", "leo-vertical-autonomous-vehicle-skyline", "lunar-station-coverage", "lunar-station-coverage-public-sources", "lunar-station-time-lab"]) {
  const p = await (await b.newContext({ viewport: { width: 1360, height: 900 } })).newPage();
  const errs = []; p.on("pageerror", (e) => errs.push(String(e)));
  await p.route(/fonts\.(googleapis|gstatic)\.com/, (r) => r.abort());
  await p.goto(`${base}${view}?scenario=${n}`, { waitUntil: "load" });
  await p.waitForTimeout(12000);
  if (capable) await p.waitForFunction(() => !/Recorded result|Running/.test(document.body.innerText.slice(0, 4000)), null, { timeout: 180000 }).catch(() => {});
  const t = (await p.evaluate(() => document.body.innerText)).replace(/\s+/g, " ");
  const title = { "orbit-urban-canyon-skyline": "City street availability", "jamming-harbour-approach-skyline": "Ship jammed entering harbour", "integrity-harbour-approach": "ARAIM entering harbour", "leo-pvt-fjord-skyline": "Ferry deep in a fjord", "gnss-sim-container-terminal-skyline": "Straddle carrier on a container quay", "leo-vertical-autonomous-vehicle-skyline": "Vehicle in a street, not a flat cut-off", "lunar-station-coverage": "Lunar reference station coverage", "lunar-station-coverage-public-sources": "Lunar reference station, public assumptions", "lunar-station-time-lab": "Lunar station time laboratory" }[n];
  const i = t.indexOf(title);
  const bad = /TOML parse error|invalid [a-z-]+ scenario|Ran locally/i.exec(t);
  const said = /needs engine 0\.36 or later; the engine running here is v0\.35\.0\. What you see is the run recorded with an engine build at commit (7b5cbf96|3bbc4657)/.test(t);
  const ranHere = /Ran locally|Showing a run|ran in this browser/i.test(t) || !/Recorded result/.test(t.slice(i, i + 1500));
  const ok = capable ? !errs.length && i >= 0 && !/needs engine 0\.36/.test(t) && !/TOML parse error|invalid [a-z-]+ scenario/i.test(t) && ranHere : !errs.length && i >= 0 && !bad && said; if (bad && !capable) console.log("   MATCH:", t.slice(i + bad.index - 200, i + bad.index + 200));
  if (!ok) fails++;
  console.log(ok ? "ok  " : "FAIL", view || "simple", n, errs.slice(0, 2).join(" | "), t.slice(0, 160));
}
await b.close();
console.log(fails ? `NEWER-ENGINE CHECK: ${fails} failure(s)` : "NEWER-ENGINE CHECK: all checks passed");
process.exit(fails ? 1 : 0);
