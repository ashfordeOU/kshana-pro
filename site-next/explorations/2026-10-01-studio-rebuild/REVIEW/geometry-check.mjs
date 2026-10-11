// Browser check of the Geometry view against the real WebAssembly engine.
//   python3 -m http.server 8871 --bind 127.0.0.1   (from the Studio folder)
//   node REVIEW/geometry-check.mjs [base-url] [--expect geometry|interim] [--shots]
// Playwright comes from $KSHANA_PLAYWRIGHT (a node_modules folder) or a global install; Chromium from
// $KSHANA_CHROME or Playwright's own. Exit 1 on any failure. Checks: the view loads with no console error,
// no page error, no 4xx/5xx and no request to another host; the skyplot draws satellites; the DOP readout
// shows engine numbers; the error panel shows the ellipse and the cloud (geometry backend) or says why not
// (interim); a hostile address is refused with notes and draws nothing it carried; the skyline is editable
// by keyboard and lands in the address; the phone width has no sideways scroll.
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
const require = createRequire(import.meta.url);
const pwPath = process.env.KSHANA_PLAYWRIGHT ? `${process.env.KSHANA_PLAYWRIGHT}/playwright` : "playwright";
let chromium;
try { ({ chromium } = require(pwPath)); } catch { ({ chromium } = require("/opt/node-tools/node_modules/playwright")); }

const args = process.argv.slice(2);
const base = (args.find((a) => /^https?:/.test(a)) || "http://127.0.0.1:8871/").replace(/\/?$/, "/");
const expect = args.includes("--expect") ? args[args.indexOf("--expect") + 1] : null;
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
  page.on("console", (m) => { if (m.type() === "error") log.console.push(m.text()); });
  page.on("pageerror", (e) => log.errors.push(String(e)));
  page.on("request", (r) => { const u = new URL(r.url()); if (!["data:", "blob:"].includes(u.protocol) && u.origin !== origin && !/^fonts\.(googleapis|gstatic)\.com$/.test(u.hostname)) log.foreign.push(r.url()); });
  page.on("response", (r) => { if (r.status() >= 400) log.bad.push(`${r.status()} ${r.url()}`); });
  await page.route(/fonts\.(googleapis|gstatic)\.com/, (r) => r.abort()); // the port self-hosts the fonts; do not reach out here
  await page.goto(base + path, { waitUntil: "load" });
  await page.waitForFunction(() => /^(Ready|The engine is not|The engine could not)/.test(document.getElementById("gm-engine").textContent), null, { timeout: 120000 });
  return { ctx, page, log };
}
const clean = (log, what) => {
  const fontAbort = (t) => /fonts\.(googleapis|gstatic)|ERR_FAILED/.test(t);
  ok(log.console.filter((t) => !fontAbort(t)).length === 0, `${what}: no console errors ${JSON.stringify(log.console.filter((t) => !fontAbort(t)))}`);
  ok(log.errors.length === 0, `${what}: no page errors ${JSON.stringify(log.errors)}`);
  ok(log.bad.length === 0, `${what}: no 4xx/5xx ${JSON.stringify(log.bad)}`);
  ok(log.foreign.length === 0, `${what}: no request to another host ${JSON.stringify(log.foreign)}`);
};

// 1. The default view.
{
  const { ctx, page, log } = await open("geometry/");
  const status = await page.textContent("#gm-engine");
  ok(/^Ready/.test(status), `default: engine ready (${status})`);
  const backend = await page.evaluate(() => (/does not give/.test(document.getElementById("gm-engine").textContent) ? "interim" : "geometry"));
  if (expect) ok(backend === expect, `backend is ${expect} (got ${backend})`);
  const nSat = await page.locator("#gm-plot .g-sat").count();
  ok(nSat >= 12, `default: the skyplot draws ${nSat} satellites`);
  ok(await page.locator("#gm-plot .g-sat.used").count() >= 4, "default: at least four used in the fix");
  const pdop = await page.locator("#gm-dops dd").nth(1).textContent();
  ok(/^\d+\.\d+$/.test(pdop), `default: PDOP is an engine number (${pdop})`);
  ok(await page.locator("#gm-sats tbody tr").count() === nSat, "default: the table lists the same satellites");
  if (backend === "geometry") {
    ok(await page.locator("#gm-errplot ellipse.g-ell").count() === 1, "geometry: the error ellipse is drawn");
    ok(await page.locator("#gm-errplot .g-cloud circle").count() >= 100, "geometry: the Monte Carlo cloud is drawn");
    ok(/^\d+\.\d+$/.test(await page.locator("#gm-dops dd").nth(0).textContent()), "geometry: GDOP is given");
  } else {
    ok(/Not available/.test(await page.textContent("#gm-errplot")), "interim: the error panel says it is not available");
  }
  clean(log, "default");
  if (shots) await page.screenshot({ path: shotDir + "desktop-light.png", fullPage: true });
  await ctx.close();
}

// 2. A shared address: every setting read back into the controls and the plot.
{
  const q = "?place=custom&lat=62.1&lon=7.2&h=5&t=2026-11-01T08:15:00Z&sys=gps,galileo&mask=12.5&sky=custom&pts=0:20,120:35,240:15&sigma=2.5&seed=42&n=500";
  const { ctx, page, log } = await open("geometry/" + q, { width: 1360, height: 1000 }, "dark");
  ok(await page.inputValue("#gm-lat") === "62.1" && await page.inputValue("#gm-mask") === "12.5" && await page.inputValue("#gm-seed") === "42", "shared: controls carry the address's values");
  ok(await page.locator("#gm-plot .g-vtx").count() === 3, "shared: three skyline points");
  ok(await page.locator("#gm-plot .g-skyline").count() === 1, "shared: the skyline is drawn");
  ok(new URL(page.url()).search === q, `shared: the address is unchanged (${new URL(page.url()).search})`);
  const sysShown = await page.evaluate(() => [...new Set([...document.querySelectorAll("#gm-sats tbody td:first-child")].map((t) => t.textContent))]);
  ok(sysShown.every((s) => ["GPS", "Galileo"].includes(s)), `shared: only GPS and Galileo (${sysShown})`);
  // Keyboard: focus the first skyline point, raise it by 5 degrees.
  await page.locator("#gm-plot .g-vtx").first().focus();
  await page.keyboard.press("Shift+ArrowUp");
  await page.waitForFunction(() => /pts=0:25,/.test(location.search));
  ok(/pts=0:25,/.test(new URL(page.url()).search), "keyboard: Shift+ArrowUp raises the point to 25° and the address follows");
  await page.keyboard.press("Delete");
  await page.waitForFunction(() => /pts=120:35,240:15(&|$)/.test(location.search));
  ok(await page.locator("#gm-plot .g-vtx").count() === 2, "keyboard: Delete removes the point");
  // Pointer: click the open sky to add a point.
  const box = await page.locator("#gm-plot").boundingBox();
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.25);
  await page.waitForFunction(() => document.querySelectorAll("#gm-plot .g-vtx").length === 3);
  ok(true, "pointer: a click on the sky adds a point");
  // Drag it.
  const v = page.locator("#gm-plot .g-vtx.sel");
  const vb = await v.boundingBox();
  await page.mouse.move(vb.x + vb.width / 2, vb.y + vb.height / 2);
  await page.mouse.down(); await page.mouse.move(vb.x + 30, vb.y + 40, { steps: 5 }); await page.mouse.up();
  await page.waitForTimeout(400);
  ok(await page.locator("#gm-plot .g-vtx").count() === 3, "pointer: dragging keeps the point count");
  // Time step and a constellation toggle recompute.
  const before = await page.textContent("#gm-sats tbody");
  await page.click('[data-step="3600"]');
  await page.waitForFunction(() => /t=2026-11-01T09:15:00Z/.test(location.search));
  await page.waitForFunction((b) => document.querySelector("#gm-sats tbody").textContent !== b, before, { timeout: 30000 });
  ok(true, "time: +1 h moves the sky and the address");
  await page.check("#gm-sys-beidou");
  await page.waitForFunction(() => !/sys=/.test(location.search) && /t=2026-11-01T09:15:00Z/.test(location.search));
  ok(true, "constellations: ticking BeiDou gives the default mix, which the address leaves out");
  await page.uncheck("#gm-sys-gps");
  await page.waitForFunction(() => /sys=galileo,beidou/.test(location.search));
  ok(true, "constellations: unticking GPS lands in the address");
  ok(await page.evaluate(() => document.activeElement && document.activeElement.id === "gm-sys-gps"), "constellations: the focus stays on the box");
  clean(log, "shared");
  if (shots) await page.screenshot({ path: shotDir + "desktop-dark.png", fullPage: true });
  await ctx.close();
}

// 3. A hostile address.
{
  const q = "?lat=1e9&t=" + encodeURIComponent("<img src=x onerror=alert(1)>") + "&sys=gps,evil&sky=custom&pts=" + encodeURIComponent('"><script>alert(1)</script>') + "&place=__proto__&n=99999";
  const { ctx, page, log } = await open("geometry/" + q);
  let dialogs = 0; page.on("dialog", (d) => { dialogs++; d.dismiss(); });
  const notes = await page.locator("#gm-url-notes li").count();
  ok(notes >= 5, `hostile: ${notes} notes say what was refused`);
  ok(await page.locator("#gm-plot .g-vtx").count() === 0, "hostile: no skyline point from the address");
  ok(await page.evaluate(() => !document.querySelector("img[src='x'], script:not([src]):not([type])") || [...document.querySelectorAll("script:not([src])")].every((s) => /remembered theme/.test(s.textContent))), "hostile: no element from the address");
  ok(await page.inputValue("#gm-lat") === "59.444", "hostile: latitude kept at the default");
  ok(dialogs === 0, "hostile: no dialog");
  clean(log, "hostile");
  await ctx.close();
}

// 4. Phone width: no sideways scroll, targets at least 44 px.
{
  const { ctx, page, log } = await open("geometry/?sky=fjord", { width: 375, height: 812 });
  const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  ok(over <= 0, `phone: no sideways scroll (${over} px)`);
  const small = await page.evaluate(() => [...document.querySelectorAll("#gm-form button, #gm-form select, #gm-form input:not([type=checkbox]), .gm-sysbox")].filter((e) => e.getBoundingClientRect().height < 44 && e.offsetParent).map((e) => e.id || e.textContent.trim()).slice(0, 5));
  ok(small.length === 0, `phone: every control at least 44 px tall ${JSON.stringify(small)}`);
  ok(await page.locator("#gm-plot .g-vtx").count() > 3, "phone: the fjord preset is drawn");
  clean(log, "phone");
  if (shots) await page.screenshot({ path: shotDir + "phone.png", fullPage: true });
  await ctx.close();
}

// 5. Entry points: the Studio hub, its Views menu, the Advanced view and the tools page link here.
for (const [path, what] of [["", "Simple view"], ["advanced/", "Advanced view"], ["tools/", "tools page"]]) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();
  const errs = []; page.on("pageerror", (e) => errs.push(String(e)));
  await page.route(/fonts\.(googleapis|gstatic)\.com/, (r) => r.abort());
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  const n = await page.locator('a[href="geometry/"]').count();
  ok(n >= 1, `${what}: ${n} link(s) to the Geometry view`);
  ok(errs.length === 0, `${what}: no page errors ${JSON.stringify(errs)}`);
  await ctx.close();
}

await browser.close();
console.log(fails ? `GEOMETRY CHECK: ${fails} failure(s)` : "GEOMETRY CHECK: all checks passed");
process.exit(fails ? 1 : 0);
