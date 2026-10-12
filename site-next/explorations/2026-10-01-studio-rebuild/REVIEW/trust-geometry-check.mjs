// Browser check of the Maritime trust view's geometry lane (0.36), against the real WebAssembly engine.
//   python3 -m http.server 8871 --bind 127.0.0.1   (from the Studio folder, or a copy whose pkg/ has the 0.36 geometry)
//   node REVIEW/trust-geometry-check.mjs [base-url] [--engine-geometry] [--shots]
// --engine-geometry: the served pkg/ has the 0.36 geometry, so "Score the synthetic demo log" must also give a lane.
// Exit 1 on any failure.
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
const require = createRequire(import.meta.url);
let chromium;
try { ({ chromium } = require(process.env.KSHANA_PLAYWRIGHT ? `${process.env.KSHANA_PLAYWRIGHT}/playwright` : "playwright")); } catch { ({ chromium } = require("/opt/node-tools/node_modules/playwright")); }

const args = process.argv.slice(2);
const base = (args.find((a) => /^https?:/.test(a)) || "http://127.0.0.1:8871/").replace(/\/?$/, "/");
const engineGeo = args.includes("--engine-geometry");
const shots = args.includes("--shots");
const shotDir = new URL("./shots-geometry/", import.meta.url).pathname;
if (shots) mkdirSync(shotDir, { recursive: true });
const origin = new URL(base).origin;
let fails = 0;
const ok = (c, m) => { console.log(`${c ? "ok  " : "FAIL"} ${m}`); if (!c) fails++; };
const browser = await chromium.launch(process.env.KSHANA_CHROME ? { executablePath: process.env.KSHANA_CHROME } : {});

async function open(path, viewport = { width: 1360, height: 1000 }, scheme = "light") {
  const ctx = await browser.newContext({ viewport, colorScheme: scheme });
  const page = await ctx.newPage();
  const log = { console: [], errors: [], bad: [], foreign: [] };
  page.on("console", (m) => { if (m.type() === "error" && !/fonts\.(googleapis|gstatic)|ERR_FAILED/.test(m.text())) log.console.push(m.text()); });
  page.on("pageerror", (e) => log.errors.push(String(e)));
  page.on("request", (r) => { const u = new URL(r.url()); if (!["data:", "blob:"].includes(u.protocol) && u.origin !== origin && !/^fonts\.(googleapis|gstatic)\.com$/.test(u.hostname)) log.foreign.push(r.url()); });
  page.on("response", (r) => { if (r.status() >= 400) log.bad.push(`${r.status()} ${r.url()}`); });
  await page.route(/fonts\.(googleapis|gstatic)\.com/, (r) => r.abort());
  await page.goto(base + path, { waitUntil: "load" });
  return { ctx, page, log };
}
const clean = (log, what) => {
  ok(!log.console.length, `${what}: no console errors ${JSON.stringify(log.console)}`);
  ok(!log.errors.length, `${what}: no page errors ${JSON.stringify(log.errors)}`);
  ok(!log.bad.length, `${what}: no 4xx/5xx ${JSON.stringify(log.bad)}`);
  ok(!log.foreign.length, `${what}: no request to another host ${JSON.stringify(log.foreign)}`);
};
const cursorTo = (page, t) => page.evaluate((t) => {
  const s = document.getElementById("mt-slider"); s.value = String(t); s.dispatchEvent(new Event("input", { bubbles: true }));
}, t);

// 1. The masked-sky demo (recorded with the 0.36 geometry, final contract): sky and class strips, the panel, the
//    class chips, the settings.
{
  const { ctx, page, log } = await open("trust/?demo=geometry");
  await page.waitForSelector("#mt-run:not([hidden])", { timeout: 60000 });
  const fills = await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg rect")].map((r) => r.getAttribute("fill")));
  ok(fills.includes("var(--b-deg)") && fills.includes("var(--b-nom)"), "geometry demo: the sky strip, good then degraded");
  ok(fills.includes("var(--g-env)") && fills.includes("var(--g-sky)") && fills.includes("var(--b-unt)"), "geometry demo: all three classes in the class strip (environment, geometry, inconsistency)");
  const labels = await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg text")].map((t) => t.textContent));
  ok(labels.includes("sky") && labels.includes("class"), "geometry demo: strips labelled sky and class");
  // an attack-like class is red, never a sky or environment colour
  const attackFills = await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg rect")].filter((r) => /^inconsistency/.test(r.querySelector("title")?.textContent || "")).map((r) => r.getAttribute("fill")));
  ok(attackFills.length > 0 && attackFills.every((f) => f === "var(--b-unt)"), `geometry demo: inconsistency drawn red (${[...new Set(attackFills)]})`);
  await cursorTo(page, 700);
  const now = await page.textContent("#mt-geo-now");
  ok(/^geometry fair: HDOP 2\.870/.test(now), `t=700: the heading is the engine's note, label and metrics first (${now.slice(0, 60)})`);
  const rows = await page.textContent("#mt-geo-rows");
  ok(/fair \(not scored\)/.test(rows) && /HDOP/.test(rows) && /230°/.test(rows) && /6 of 11 in view/.test(rows), "t=700: label, DOPs, gap and satellites are shown");
  ok(!/Geometry deduction/.test(rows), "t=700: no geometry deduction row while the deduction is zero");
  ok((await page.locator("#mt-geo-flags li").count()) === 0, "t=700: no flag while none is set");
  ok(await page.locator("#mt-reasons .mt-class.environment").count() >= 1, "t=700: the loss-of-lock deduction carries its class, environment");
  ok(/25\.0 pts/.test(await page.textContent("#mt-reasons")), "t=700: and it costs its 25 points");
  ok(/geometry fair/.test(await page.getAttribute("#mt-time-svg", "aria-valuetext")), "t=700: the spoken description names the geometry");
  await cursorTo(page, 1000);
  ok(/^geometry poor: HDOP/.test(await page.textContent("#mt-geo-now")), "t=1000: the narrow berth reads poor");
  ok(/Geometry deduction[\d.]+ points \(geometry\)/.test(await page.textContent("#mt-geo-rows")), "t=1000: a geometry deduction classed geometry");
  ok((await page.locator("#mt-geo-flags li").count()) === 0, "t=1000: no mismatch and no signal loss");
  ok(await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg rect.mt-geo-under")].some((r) => r.getAttribute("fill") === "var(--g-sky)")), "t=1000: the class strip underlines the geometry deduction under environment");
  ok(await page.locator("#mt-reasons .mt-class.geometry").count() >= 1, "t=1000: the geometry class chip on the deduction");
  if (shots) await page.locator("#mt-run").screenshot({ path: shotDir + "trust-geometry-class.png" });
  await cursorTo(page, 2000);
  ok(await page.locator("#mt-reasons .mt-class.inconsistency").count() >= 1, "t=2000: the drag-off deductions are classed inconsistency");
  const set = await page.textContent("#mt-set");
  ok(/hdop_max/.test(set) && /pdop_max/.test(set) && /azimuth_gap_deg/.test(set) && /cap_in_force/.test(set) && /hold_s/.test(set), "settings: the [geometry] fields sit beside the score settings");
  ok(/the degraded edge/.test(set) && /0\.25/.test(set), "settings: an empty floor reads as the degraded edge; 0.25 is not rounded");
  ok(/founder decision B1|design note/.test(set) && /Label: \d+ good, \d+ fair, 300 poor/.test(set), "settings: provenance, and the counts by label");
  ok(/Advisory software, not type-approved navigation equipment \(IEC 61108, IEC 61162\): the operator remains responsible for the navigation of the vessel\./.test(await page.textContent("main")), "the full advisory statement stays");
  const legend = await page.textContent("#mt-legend");
  ok(/geometry unknown \(no GSV or GSA; not good\)/.test(legend) && /Deduction class:/.test(legend), "legend: unknown is never good; the deduction classes");
  clean(log, "geometry demo");
  if (shots) { await cursorTo(page, 700); await page.locator("#mt-run").screenshot({ path: shotDir + "trust-geometry.png" }); }
  await ctx.close();
}

// 1b. The review fixture (poor geometry, a stated HDOP that disagrees): the deduction row and the flag appear.
{
  const { ctx, page, log } = await open("trust/");
  await page.setInputFiles("#mt-files", new URL("./fixtures/poor-geometry.result.json", import.meta.url).pathname);
  await page.waitForSelector("#mt-run:not([hidden])", { timeout: 60000 });
  await cursorTo(page, 700);
  ok(/^geometry poor: HDOP 10\.836/.test(await page.textContent("#mt-geo-now")), "poor fixture: the heading reads poor");
  const rows = await page.textContent("#mt-geo-rows");
  ok(/Geometry deduction15\.0 points \(inconsistency\)/.test(rows), `poor fixture: the geometry deduction row, 15 points, inconsistency`);
  ok(/HDOP the receiver states0\.9/.test(rows), "poor fixture: the stated HDOP is shown");
  const flags = await page.locator("#mt-geo-flags li").allTextContents();
  ok(flags.some((f) => /disagrees/.test(f)) && flags.some((f) => /Signal loss/.test(f)), `poor fixture: mismatch and signal-loss flags (${flags.length})`);
  ok(await page.locator("#mt-reasons .mt-class.inconsistency").count() >= 1, "poor fixture: the geometry deduction's class chip");
  ok(!/Not run[\s\S]*\bgeometry\b/.test(await page.textContent("#mt-reasons")), "poor fixture: the geometry check is never listed as not run");
  ok(await page.evaluate(() => { const g = document.querySelector(".mt-setgrid dt"); return [...document.querySelectorAll(".mt-setgrid div")].every((d) => d.scrollWidth <= d.clientWidth + 1); }), "poor fixture: no setting name overflows its cell");
  await cursorTo(page, 400);
  ok(!/Geometry deduction/.test(await page.textContent("#mt-geo-rows")) && (await page.locator("#mt-geo-flags li").count()) === 0, "poor fixture: before the mask, no deduction row and no flag");
  if (shots) { await cursorTo(page, 700); await page.locator("#mt-run").screenshot({ path: shotDir + "trust-geometry-poor.png" }); }
  clean(log, "poor fixture");
  await ctx.close();
}

// 2. The 0.35 demo (no geometry): no lane, and nothing shown as good.
{
  const { ctx, page, log } = await open("trust/?demo=1");
  await page.waitForSelector("#mt-run:not([hidden])", { timeout: 60000 });
  ok(!(await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg text")].some((t) => t.textContent === "sky"))), "0.35 demo: no sky strip");
  ok(/No geometry in this run/.test(await page.textContent("#mt-geo-now")), "0.35 demo: the panel says the run has no geometry");
  ok(/carries no geometry/.test(await page.textContent("#mt-set")), "0.35 demo: the settings say so too");
  clean(log, "0.35 demo");
  await ctx.close();
}

// 3. Scoring the demo log in this browser (live lines): with a 0.36 engine, schema 1.3 brings the lane.
if (engineGeo) {
  const { ctx, page, log } = await open("trust/");
  await page.click("#mt-score-demo");
  await page.waitForSelector("#mt-run:not([hidden])", { timeout: 180000 });
  ok(await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg text")].some((t) => t.textContent === "sky")), "scored here: the 1.3 lines carry the lane");
  await cursorTo(page, 400);
  ok(/^geometry good: HDOP/.test(await page.textContent("#mt-geo-now")), "scored here: the demo's open sky reads good");
  ok(/do(es)? not state the geometry thresholds/.test(await page.textContent("#mt-set")), "scored here: a stream states no thresholds, and the page says so");
  clean(log, "scored here");
  await ctx.close();
}

// 4. Phone width.
{
  const { ctx, page, log } = await open("trust/?demo=geometry", { width: 375, height: 812 });
  await page.waitForSelector("#mt-run:not([hidden])", { timeout: 60000 });
  const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  ok(over <= 0, `phone: no sideways scroll (${over} px)`);
  clean(log, "phone");
  await ctx.close();
}

await browser.close();
console.log(fails ? `TRUST GEOMETRY CHECK: ${fails} failure(s)` : "TRUST GEOMETRY CHECK: all checks passed");
process.exit(fails ? 1 : 0);
