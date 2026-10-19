// Browser smoke test of the observatory viewer. Run: node observatory-smoke.mjs http://localhost:PORT FIXTURE_DIR OUT_DIR  (FIXTURE_DIR from observatory-fixture/mk.py)
import { createRequire } from "node:module";
import { readdirSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
const require = createRequire("/opt/node22/lib/node_modules/");
const { chromium } = require("playwright");
const [base, dir, out] = process.argv.slice(2);
const all = readdirSync(dir).map((f) => `${dir}/${f}`);
const b = await chromium.launch({ executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args: ["--no-sandbox"] });
let fails = 0; const ok = (c, m) => { console.log(c ? "ok  " : "FAIL", m); if (!c) fails++; };
const requests = [];
for (const [w, h, scheme] of [[1440, 1000, "light"], [1440, 1000, "dark"], [390, 900, "light"], [390, 900, "dark"]]) {
  const ctx = await b.newContext({ viewport: { width: w, height: h }, colorScheme: scheme });
  const p = await ctx.newPage(); const errs = []; p.on("pageerror", (e) => errs.push(e.message)); p.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
  p.on("request", (r) => { const u = new URL(r.url()); if (u.origin !== new URL(base).origin && !/fonts\.(googleapis|gstatic)\.com$/.test(u.host) && u.protocol !== "data:" && u.protocol !== "blob:") requests.push(r.url()); });
  const main = w === 1440 && scheme === "light";
  await p.goto(`${base}/observatory/index.html`); await p.waitForSelector("#ko-files", { state: "attached" });
  ok(await p.$eval("#ko-body", (e) => e.hidden), "nothing is shown before a file is opened");
  await p.setInputFiles("#ko-files", all); await p.waitForSelector("#ko-body:not([hidden])"); await p.waitForTimeout(600);
  if (main) {
    const day = await p.$eval("#ko-day-h", (e) => e.textContent); ok(/2026-03-03/.test(day), "opens the newest day: " + day);
    ok((await p.$eval("#ko-sum", (e) => e.textContent)).match(/^\d+ cells degraded, \d+ not degraded, \d+ not observed, on 2026-03-03\.$/), "the summary line is built from the file");
    ok(/No degraded cells this day\./.test(await p.$eval("#ko-daynotes", (e) => e.textContent)), "a day with no degraded cells says only that");
    const body = await p.$eval("body", (e) => e.innerText);
    ok(/Not observed is not clear\./.test(body), "the legend sentence is on the page");
    ok(!/\bnow\b|jamming detected|spoof/i.test(body.replace(/Not observed is not clear\./, "").replace(/never says .now./, "")), "no 'now', no 'jamming detected', no spoofing words");
    ok(/at least 24 hours/.test(await p.$eval("#ko-release", (e) => e.textContent)), "release rule is stated");
    ok(/Advisory software/.test(body) && /not type-approved navigation equipment/.test(body), "advisory statement present");
    ok(/ODbL-1\.0/.test(await p.$eval("#ko-data", (e) => e.textContent)) && (await p.$$eval("#ko-data a", (a) => a.map((x) => x.href))).every((h) => h.startsWith("https://")), "licence shown with an https link");
    const hashes = await p.$$eval("#ko-files-t tbody tr td:last-child", (t) => t.map((x) => x.textContent)); ok(hashes.length === 3 && hashes.every((x) => x === "matches"), "all three files match the manifest: " + hashes);
    await p.click("#ko-prev"); await p.waitForTimeout(300);
    ok(/2026-03-02/.test(await p.$eval("#ko-day-h", (e) => e.textContent)) && /no cell could be called that day/.test(await p.$eval("#ko-daynotes", (e) => e.textContent)), "day-wide day: says no cell could be called, not that the sky was clean");
    await p.click("#ko-prev"); await p.waitForTimeout(300);
    ok(/2026-03-01/.test(await p.$eval("#ko-day-h", (e) => e.textContent)) && (await p.$$("#ko-inc tbody .ko-rowbtn")).length >= 1, "day 1 has an incident row");
    ok(await p.$eval("#ko-prev", (e) => e.disabled), "previous is disabled on the first day");
    await p.click("#ko-inc tbody .ko-rowbtn"); await p.waitForTimeout(200);
    ok((await p.$$("#ko-map .ko-sel")).length >= 1, "selecting an incident outlines its cells");
    ok((await p.$$eval("#ko-inc tbody td:last-child", (t) => t.map((x) => x.textContent))).every((x) => x === "not attributed"), "every incident's cause reads: not attributed");
    await p.selectOption("#ko-flt", "not_observed"); await p.waitForTimeout(200);
    ok((await p.$$eval("#ko-cells tbody tr td:first-of-type", (t) => t.map((x) => x.textContent))).every((x) => x === "Not observed"), "filter shows only not observed cells");
    await p.selectOption("#ko-flt", "all");
    ok((await p.$$eval("#ko-cells tbody td:nth-child(5)", (t) => t.map((x) => x.textContent))).includes("withheld"), "withheld counts read 'withheld'");
    ok((await p.$$("#ko-map .ko-not_observed")).length > 0 && (await p.$eval("#ko-map defs pattern", (e) => !!e)), "not observed cells carry the hatch pattern");
    ok((await p.$eval("#ko-img", (e) => e.complete && e.naturalWidth > 0)), "the PNG opened is shown");
    // a tampered file is caught
    const t = `${out}/tamper`; mkdirSync(t, { recursive: true });
    const g = readFileSync(`${dir}/observatory-adsb-synthetic-2026-03-01.geojson`, "utf8").replace('"cell_i": 0,', '"cell_i": 0 ,');
    writeFileSync(`${t}/observatory-adsb-synthetic-2026-03-01.geojson`, g);
    await p.setInputFiles("#ko-files", [`${t}/observatory-adsb-synthetic-2026-03-01.geojson`, `${dir}/observatory-adsb-synthetic-2026-03-01.manifest.json`]); await p.waitForTimeout(600);
    ok((await p.$$eval("#ko-files-t tbody tr", (r) => r.map((x) => x.textContent))).some((x) => /does not match/.test(x)), "an altered file 'does not match' its manifest hash");
    await p.reload(); await p.waitForSelector("#ko-files", { state: "attached" });
    await p.setInputFiles("#ko-files", [`${dir}/observatory-adsb-synthetic-2026-03-01.geojson`, `${dir}/observatory-adsb-synthetic-2026-03-01.manifest.json`]); await p.waitForSelector("#ko-body:not([hidden])"); await p.waitForTimeout(600);
    ok((await p.$$eval("#ko-files-t tbody tr td:last-child", (r) => r.map((x) => x.textContent))).join() === "matches,not opened,not opened", "files not opened say 'not opened'");
    // refusals
    const bad = `${out}/tamper/observatory-adsb-synthetic-2026-03-09.incidents.json`;
    writeFileSync(bad, readFileSync(`${dir}/observatory-adsb-synthetic-2026-03-01.incidents.json`, "utf8").replace('"not attributed"', '"hostile jamming"'));
    await p.setInputFiles("#ko-files", [bad]); await p.waitForTimeout(400);
    ok(/states a cause other than "not attributed"/.test(await p.$eval("#ko-notes", (e) => e.textContent)), "an incident file that names a cause is refused");
    await p.setInputFiles("#ko-files", all); await p.waitForTimeout(600);
  }
  ok(await p.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1), `no horizontal page scroll at ${w} ${scheme}`);
  ok(errs.length === 0, `no page errors ${w} ${scheme} ${errs.join("|").slice(0, 200)}`);
  await p.screenshot({ path: `${out}/observatory-synthetic-${w}-${scheme}.png`, fullPage: true, clip: { x: 0, y: 0, width: w, height: Math.min(await p.evaluate(() => document.documentElement.scrollHeight), w > 600 ? 2300 : 3600) } });
  await ctx.close();
}
ok(requests.length === 0, "no request to any other address than the Studio and its fonts: " + requests.join(" "));
await b.close(); process.exit(fails ? 1 : 0);
