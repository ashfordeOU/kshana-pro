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

// 1. The masked-sky demo (recorded with the 0.36 geometry): both strips, the panel, the marks, the settings.
{
  const { ctx, page, log } = await open("trust/?demo=geometry");
  await page.waitForSelector("#mt-run:not([hidden])", { timeout: 60000 });
  const fills = await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg rect")].map((r) => r.getAttribute("fill")));
  ok(fills.includes("var(--g-sky)"), "geometry demo: a cause strip in the neutral sky blue");
  ok(fills.includes("var(--b-deg)") && fills.includes("var(--b-nom)"), "geometry demo: the sky strip, good then degraded");
  ok(await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg text")].some((t) => t.textContent === "sky") && [...document.querySelectorAll("#mt-time-svg text")].some((t) => t.textContent === "cause")), "geometry demo: strips labelled sky and cause");
  // the sky blue never marks an attack: every rect titled inconsistency is red or amber
  const attackFills = await page.evaluate(() => [...document.querySelectorAll("#mt-time-svg rect")].filter((r) => /^inconsistency|^geometry and inconsistency/.test(r.querySelector("title")?.textContent || "")).map((r) => r.getAttribute("fill")));
  ok(attackFills.length > 0 && attackFills.every((f) => f === "var(--b-unt)" || f === "var(--b-deg)"), `geometry demo: attack-like cause drawn red or amber, never the sky blue (${[...new Set(attackFills)]})`);
  await cursorTo(page, 700);
  ok(/^Geometry degraded: .*one side of the sky blocked/.test(await page.textContent("#mt-geo-now")), `t=700: the panel says the sky is blocked (${await page.textContent("#mt-geo-now")})`);
  ok(/HDOP/.test(await page.textContent("#mt-geo-rows")) && /230°/.test(await page.textContent("#mt-geo-rows")), "t=700: DOPs and the azimuth gap are shown");
  ok(await page.locator("#mt-reasons .mt-explained").count() >= 1, "t=700: loss-of-lock is marked explained by geometry");
  ok(/25\.0 pts/.test(await page.textContent("#mt-reasons")), "t=700: and it still costs its 25 points");
  ok(/geometry degraded/.test(await page.getAttribute("#mt-time-svg", "aria-valuetext")), "t=700: the spoken description names the geometry");
  await cursorTo(page, 2000);
  ok(/inconsistency/.test(await page.textContent("#mt-geo-rows")), "t=2000: the drag-off is put down to inconsistency");
  const set = await page.textContent("#mt-set");
  ok(/hdop_max/.test(set) && /pdop_max/.test(set) && /azimuth_gap_deg/.test(set), "settings: the geometry thresholds sit beside the score settings");
  ok(/IEC 61108-1/.test(set) && /Geometry never changes the score|never part of it/.test(set), "settings: their provenance, and that the geometry never changes the score");
  ok(/Advisory/.test(await page.textContent("#mt-adv")) && /IEC 61108, IEC 61162/.test(await page.textContent("main")), "the advisory statement stays");
  ok(/geometry unknown \(no GSV or GSA; not good\)/.test(await page.textContent("#mt-legend")), "legend: unknown is never good");
  clean(log, "geometry demo");
  if (shots) { await cursorTo(page, 700); await page.locator("#mt-run").screenshot({ path: shotDir + "trust-geometry.png" }); }
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
  ok(/^Geometry good\./.test(await page.textContent("#mt-geo-now")), "scored here: the demo's open sky reads good");
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
