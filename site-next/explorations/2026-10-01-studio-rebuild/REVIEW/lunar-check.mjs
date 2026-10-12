// Browser check of the lunar station view (0.36).
//   python3 -m http.server 8871 --bind 127.0.0.1   (from the Studio folder)
//   node REVIEW/make-lunar-fixture.mjs && node REVIEW/lunar-check.mjs [base-url] [--engine-kind] [--shots]
// Before the engine kind ships there is no recording and no bundled scenario: the page must say so, and a result
// file (the synthetic review fixture) must draw. --engine-kind: the served folder has the scenario and a pkg/ that
// runs it, so a small run here must draw too.
// Exit 1 on any failure.
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
const require = createRequire(import.meta.url);
let chromium;
try { ({ chromium } = require(process.env.KSHANA_PLAYWRIGHT ? `${process.env.KSHANA_PLAYWRIGHT}/playwright` : "playwright")); } catch { ({ chromium } = require("/opt/node-tools/node_modules/playwright")); }

const args = process.argv.slice(2);
const base = (args.find((a) => /^https?:/.test(a)) || "http://127.0.0.1:8871/").replace(/\/?$/, "/");
const engineKind = args.includes("--engine-kind");
const shots = args.includes("--shots");
const shotDir = new URL("./shots-lunar/", import.meta.url).pathname;
if (shots) mkdirSync(shotDir, { recursive: true });
const fixture = new URL("./fixtures/lunar-station.synthetic.json", import.meta.url).pathname;
const named = new URL("./fixtures/lunar-station.named-note.json", import.meta.url).pathname;
const origin = new URL(base).origin;
let fails = 0;
const ok = (c, m) => { console.log(`${c ? "ok  " : "FAIL"} ${m}`); if (!c) fails++; };
const browser = await chromium.launch(process.env.KSHANA_CHROME ? { executablePath: process.env.KSHANA_CHROME } : {});

async function open(path, viewport = { width: 1360, height: 1000 }, scheme = "light") {
  const ctx = await browser.newContext({ viewport, colorScheme: scheme });
  const page = await ctx.newPage();
  const log = { console: [], errors: [], bad: [], foreign: [], reqs: [] };
  page.on("console", (m) => { if (m.type() === "error" && !/fonts\.(googleapis|gstatic)|ERR_FAILED|404 \(File not found\)|Failed to load resource/.test(m.text())) log.console.push(m.text()); });
  page.on("pageerror", (e) => log.errors.push(String(e)));
  page.on("request", (r) => { const u = new URL(r.url()); if (u.origin === origin) log.reqs.push(u.pathname); if (!["data:", "blob:"].includes(u.protocol) && u.origin !== origin && !/^fonts\.(googleapis|gstatic)\.com$/.test(u.hostname)) log.foreign.push(r.url()); });
  page.on("response", (r) => { if (r.status() >= 400) log.bad.push(`${r.status()} ${new URL(r.url()).pathname}`); });
  await page.route(/fonts\.(googleapis|gstatic)\.com/, (r) => r.abort());
  await page.goto(base + path, { waitUntil: "load" });
  return { ctx, page, log };
}
// Before the port the two fixed files of this kind are missing; nothing else may be.
const EXPECTED_MISSING = /\/(recorded\/lunar-station-coverage\.json|scenarios\/lunar-station-coverage\.toml)$/;
const clean = (log, what) => {
  ok(!log.console.length, `${what}: no console errors ${JSON.stringify(log.console)}`);
  ok(!log.errors.length, `${what}: no page errors ${JSON.stringify(log.errors)}`);
  ok(log.bad.every((b) => EXPECTED_MISSING.test(b)), `${what}: no 4xx/5xx beyond this kind's two files ${JSON.stringify(log.bad)}`);
  ok(!log.foreign.length, `${what}: no request to another host ${JSON.stringify(log.foreign)}`);
};
const HONEST = /MODELLED\.\s*Under the stated public assumptions, not a statement about the performance of any real system\.\s*Not affiliated with or endorsed by any agency\./;

// 1. Opening: the honesty block, and plain words when no recording ships yet.
{
  const { ctx, page, log } = await open("lunar/");
  await page.waitForFunction(() => !/Starting the engine/.test(document.getElementById("ls-engine").textContent), null, { timeout: 30000 });
  ok(HONEST.test(await page.textContent("#ls-honest")), "the honesty block, in full");
  const body = await page.textContent("body");
  ok(!/nova\s*moon|\bESA\b|European Space Agency|Moonlight/i.test(body), "generic names only: no programme or agency named in the page");
  const eng = await page.textContent("#ls-engine");
  if (!engineKind) ok(/No recording of this study ships yet/.test(eng), `no recording yet, said plainly (${eng})`);
  ok(log.reqs.every((p) => !/\.\.|%2e/i.test(p)), "fixed same-origin paths only");
  clean(log, "opening");
  await ctx.close();
}

// 2. A result file: the map, the contour, the station, the radius, the figures, the table.
{
  const { ctx, page, log } = await open("lunar/");
  await page.setInputFiles("#ls-file", fixture);
  await page.waitForFunction(() => /Showing the file/.test(document.getElementById("ls-engine").textContent), null, { timeout: 30000 });
  const cells = await page.locator("#ls-map path.ls-cell").count();
  ok(cells > 300, `map: one cell per grid point (${cells})`);
  ok(await page.locator("#ls-map path.ls-contour").count() === 1 && (await page.getAttribute("#ls-map path.ls-contour", "d")).length > 50, "map: the threshold contour is drawn");
  ok(await page.locator("#ls-map .ls-station").count() === 1 && await page.locator("#ls-map .ls-worst").count() === 1, "map: the station and the worst point are marked");
  ok(await page.locator("#ls-map .ls-radius-line").count() === 1, "map: the coverage radius per azimuth");
  ok(await page.locator("#ls-map path.ls-none").count() > 0, "map: unavailable points hatched, never coloured as good");
  const classes = await page.evaluate(() => [...new Set([...document.querySelectorAll("#ls-map path.ls-cell")].map((p) => p.getAttribute("class")))]);
  ok(classes.some((c) => /ls-b[012]/.test(c)) && classes.some((c) => /ls-b[345]/.test(c)), `map: cells on both sides of the threshold (${classes.join(", ")})`);
  const region = await page.textContent("#ls-region");
  ok(/Corrected error, 95 %, 3.D\s*1\.18 m/.test(region) && /27\.3 m/.test(region) && /23\.1 ×/.test(region) && /41\.0 %/.test(region), "region figures from the file");
  ok(/alert limit 10\.0 m \(illustrative\)/.test(region), "the alert limit is called illustrative");
  ok(await page.locator("#ls-radplot circle").count() === 36 && await page.locator("#ls-radtab tbody tr").count() === 36, "coverage radius: 36 azimuths, drawn and tabled");
  ok(/96\.2 %/.test(await page.textContent("#ls-availfigs")) && /fewer than four satellites/.test(await page.textContent("#ls-reasons")), "availability and the reasons in words");
  ok(await page.locator("#ls-points tbody tr").count() === cells, "the table has every point");
  ok(/Synthetic review fixture/.test(await page.textContent("#ls-note")), "the file's statement is shown when it names nothing");
  await page.selectOption("#ls-layer", "availability");
  ok(/availability/.test(await page.textContent("#ls-map-desc")), "the map recolours by availability");
  ok(!(await page.locator("#ls-save").isHidden()), "the result can be saved");
  if (shots) { await page.selectOption("#ls-layer", "error"); await page.screenshot({ path: shotDir + "lunar-light.png", fullPage: true }); }
  clean(log, "result file");
  await ctx.close();
}

// 3. A wrapped file whose statement names a programme: shown without it.
{
  const { ctx, page, log } = await open("lunar/");
  await page.setInputFiles("#ls-file", named);
  await page.waitForFunction(() => /Showing the file/.test(document.getElementById("ls-engine").textContent), null, { timeout: 30000 });
  const note = await page.textContent("#ls-note");
  ok(/names a programme or an agency/.test(note) && !/nova/i.test(await page.textContent("body")), "a named statement is withheld");
  clean(log, "named note");
  await ctx.close();
}

// 4. The address: inputs validated, bad ones explained.
{
  const { ctx, page, log } = await open("lunar/?src=live&thr=2.5&trials=5000&slat=-95");
  await page.waitForFunction(() => document.getElementById("ls-url-notes").children.length > 0, null, { timeout: 30000 });
  ok(await page.locator("#ls-url-notes li").count() === 2, "two out-of-range inputs are refused, in words");
  ok(await page.isChecked('input[name="ls-src"][value="live"]'), "the source comes from the address");
  if (!engineKind) ok(await page.isDisabled("#ls-run"), "no bundled scenario yet: a run here is not offered");
  clean(log, "address");
  await ctx.close();
}

// 5. A run here, with an engine that has the kind.
if (engineKind) {
  const { ctx, page, log } = await open("lunar/?src=live&trials=2&tstep=21600&gstep=2");
  await page.waitForFunction(() => !document.getElementById("ls-run").disabled, null, { timeout: 30000 });
  await page.click("#ls-run");
  await page.waitForFunction(() => /Showing a run in this browser/.test(document.getElementById("ls-engine").textContent), null, { timeout: 600000 });
  ok(await page.locator("#ls-map path.ls-cell").count() > 10, "a run here draws the map");
  clean(log, "run here");
  await ctx.close();
}

// 6. Phone width and dark.
{
  const { ctx, page, log } = await open("lunar/", { width: 375, height: 812 }, "dark");
  await page.setInputFiles("#ls-file", fixture);
  await page.waitForFunction(() => /Showing the file/.test(document.getElementById("ls-engine").textContent), null, { timeout: 30000 });
  const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  ok(over <= 0, `phone: no sideways scroll (${over} px)`);
  const small = await page.evaluate(() => [...document.querySelectorAll("button, .ls-radio, select, .ls-file, summary")].filter((b) => b.offsetParent && b.getBoundingClientRect().height < 24).length);
  ok(small === 0, `phone: controls are large enough to touch (${small} small)`);
  if (shots) await page.screenshot({ path: shotDir + "lunar-phone-dark.png", fullPage: true });
  clean(log, "phone dark");
  await ctx.close();
}

await browser.close();
console.log(fails ? `LUNAR CHECK: ${fails} failure(s)` : "LUNAR CHECK: all checks passed");
process.exit(fails ? 1 : 0);
