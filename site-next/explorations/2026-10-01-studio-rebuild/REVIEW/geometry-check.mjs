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
  const backend = await page.evaluate(() => (/Needs engine 0\.36/.test(document.getElementById("gm-engine").textContent) ? "interim" : "geometry"));
  if (expect) ok(backend === expect, `backend is ${expect} (got ${backend})`);
  const nSat = await page.locator("#gm-plot .g-sat").count();
  ok(nSat >= 12, `default: the skyplot draws ${nSat} satellites`);
  ok(await page.locator("#gm-plot .g-sat.used").count() >= 4, "default: at least four used in the fix");
  const pdop = await page.locator("#gm-dops dd").nth(1).textContent();
  ok(/^\d+\.\d+$/.test(pdop), `default: PDOP is an engine number (${pdop})`);
  ok(/Advisory software, not type-approved navigation equipment \(IEC 61108, IEC 61162\): the operator remains responsible for the navigation of the vessel\./.test(await page.textContent(".gm-advice")), "default: the full advisory statement");
  ok(await page.locator("#gm-sats tbody tr").count() === nSat, "default: the table lists the same satellites");
  if (backend === "geometry") {
    ok(await page.locator("#gm-errplot ellipse.g-ell").count() === 1 && await page.locator("#gm-errplot ellipse.g-ell95").count() === 1, "geometry: the 1-sigma and 95 % ellipses are drawn");
    ok(await page.locator("#gm-errplot .g-cloud circle").count() >= 100, "geometry: the Monte Carlo cloud is drawn");
    ok(/^\d+\.\d+$/.test(await page.locator("#gm-dops dd").nth(0).textContent()), "geometry: GDOP is given");
  } else {
    ok(/Needs engine 0\.36/.test(await page.textContent("#gm-errplot")), "interim: the error panel says it needs engine 0.36");
    ok(/needs engine 0\.36/.test(await page.locator("#gm-dops dd").nth(0).textContent()), "interim: GDOP says it needs engine 0.36");
  }
  clean(log, "default");
  if (shots) await page.screenshot({ path: shotDir + "desktop-light.png", fullPage: true });
  await ctx.close();
}

// 2. A shared address: every setting read back into the controls and the plot; drawing by keyboard and pointer.
{
  const q = "?site=62.1,7.2,5&t=3600&sys=G,E&mask=12.5&uere=2.5&sky=p:fjord&seed=42";
  const { ctx, page, log } = await open("geometry/" + q, { width: 1360, height: 1000 }, "dark");
  ok(await page.inputValue("#gm-lat") === "62.1" && await page.inputValue("#gm-mask") === "12.5" && await page.inputValue("#gm-uere") === "2.5", "shared: controls carry the address's values");
  ok(await page.inputValue("#gm-sky") === "p:fjord", "shared: the fjord preset is selected");
  ok(await page.locator("#gm-plot .g-skyline").count() === 1, "shared: the skyline is drawn");
  ok(new URL(page.url()).search === q, `shared: the address is unchanged (${new URL(page.url()).search})`);
  ok(/representative sky, not the sky of that date/i.test(await page.textContent(".gm-rep")), "shared: the representative-sky notice is above the plot");
  const sysShown = await page.evaluate(() => [...new Set([...document.querySelectorAll("#gm-sats tbody td:first-child")].map((t) => t.textContent))]);
  ok(sysShown.every((s) => ["GPS", "Galileo"].includes(s)), `shared: only GPS and Galileo (${sysShown})`);
  // Keyboard: focus the skyplot, move to azimuth 90, raise the skyline there by 5 degrees.
  await page.focus("#gm-plot");
  for (let i = 0; i < 9; i++) await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Shift+ArrowUp");
  await page.waitForFunction(() => /sky=t:[A-Za-z0-9_-]{480}(&|$)/.test(location.search));
  ok(/Azimuth 90°/.test(await page.textContent("#gm-cursor-live")), "keyboard: the cursor is announced");
  ok(await page.inputValue("#gm-sky") === "table", "keyboard: editing makes the skyline the person's own table");
  const drawn = new URL(page.url()).search;
  // Pointer: a stroke across the northern sky.
  const box = await page.locator("#gm-plot").boundingBox();
  await page.mouse.move(box.x + box.width * 0.35, box.y + box.height * 0.2);
  await page.mouse.down(); await page.mouse.move(box.x + box.width * 0.65, box.y + box.height * 0.2, { steps: 8 }); await page.mouse.up();
  await page.waitForFunction((d) => location.search !== d, drawn);
  ok(true, "pointer: a stroke changes the drawn table in the address");
  // Reload the drawn address: the same table comes back.
  const drawnUrl = page.url();
  await page.goto(drawnUrl); await page.waitForFunction(() => /^Ready/.test(document.getElementById("gm-engine").textContent));
  ok(page.url() === drawnUrl && await page.locator("#gm-plot .g-skyline").count() === 1, "reload: the drawn skyline comes back from the address");
  // Time step and a constellation toggle recompute.
  const before = await page.textContent("#gm-sats tbody");
  await page.click('[data-step="3600"]');
  await page.waitForFunction(() => /t=7200/.test(location.search));
  await page.waitForFunction((b) => document.querySelector("#gm-sats tbody").textContent !== b, before, { timeout: 30000 });
  ok(true, "time: +1 h moves the sky and the address");
  await page.check("#gm-sys-beidou");
  await page.waitForFunction(() => !/sys=/.test(location.search));
  ok(true, "constellations: ticking BeiDou gives the default mix, which the address leaves out");
  await page.uncheck("#gm-sys-gps");
  await page.waitForFunction(() => /sys=E,C/.test(location.search));
  ok(await page.evaluate(() => document.activeElement && document.activeElement.id === "gm-sys-gps"), "constellations: the focus stays on the box");
  clean(log, "shared");
  if (shots) await page.screenshot({ path: shotDir + "desktop-dark.png", fullPage: true });
  await ctx.close();
}

// 3. A hostile address.
{
  const q = "?site=1e9,5&t=" + encodeURIComponent("<img src=x onerror=alert(1)>") + "&sys=G,evil&sky=" + encodeURIComponent('t:"><script>alert(1)</script>') + "&mc=99999&uere=0";
  const { ctx, page, log } = await open("geometry/" + q);
  let dialogs = 0; page.on("dialog", (d) => { dialogs++; d.dismiss(); });
  const notes = await page.locator("#gm-url-notes li").count();
  ok(notes >= 6, `hostile: ${notes} notes say what was refused`);
  ok(await page.locator("#gm-plot .g-skyline").count() === 0, "hostile: no skyline from the address");
  ok(await page.evaluate(() => !document.querySelector("img[src='x']") && [...document.querySelectorAll("script:not([src])")].every((s) => /remembered theme/.test(s.textContent))), "hostile: no element from the address");
  ok(await page.inputValue("#gm-lat") === "59.91", "hostile: latitude kept at the default");
  ok(dialogs === 0, "hostile: no dialog");
  clean(log, "hostile");
  await ctx.close();
}

// 4. Phone width: no sideways scroll, targets at least 44 px.
{
  const { ctx, page, log } = await open("geometry/?sky=p:fjord", { width: 375, height: 812 });
  const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  ok(over <= 0, `phone: no sideways scroll (${over} px)`);
  const small = await page.evaluate(() => [...document.querySelectorAll("#gm-form button, #gm-form select, #gm-form input:not([type=checkbox]), .gm-sysbox")].filter((e) => e.getBoundingClientRect().height < 44 && e.offsetParent).map((e) => e.id || e.textContent.trim()).slice(0, 5));
  ok(small.length === 0, `phone: every control at least 44 px tall ${JSON.stringify(small)}`);
  ok(await page.locator("#gm-plot .g-skyline").count() === 1, "phone: the fjord preset is drawn");
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
