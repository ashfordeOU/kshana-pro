// R4 home waterfall: screenshots, liveness and section height. Usage: node r4_waterfall_shots.mjs <baseURL>
import { chromium } from "playwright";
const base = process.argv[2] || "http://127.0.0.1:18431/";
const out = "../shots/r4/home-waterfall";
const b = await chromium.launch();
const res = {};
for (const [w, h] of [[1440, 900], [390, 844]]) for (const scheme of ["dark", "light"]) {
  const ctx = await b.newContext({ viewport: { width: w, height: h }, colorScheme: scheme, deviceScaleFactor: 2 });
  const p = await ctx.newPage();
  const errs = []; p.on("console", (m) => { if (m.type() === "error" || m.type() === "warning") errs.push(m.text()); }); p.on("pageerror", (e) => errs.push(String(e)));
  await p.goto(base + "index.html", { waitUntil: "networkidle" });
  await p.evaluate(() => document.getElementById("lband").scrollIntoView({ block: "start" }));
  await p.waitForSelector("#lband.lb-ready", { timeout: 10000 });
  await p.waitForTimeout(6000);
  const sec = await p.$("#lband");
  const box = await sec.boundingBox();
  const hs = await p.evaluate(() => ({ doc: document.documentElement.scrollWidth, vw: innerWidth }));
  await sec.screenshot({ path: `${out}/home-waterfall-${w}x${h}-${scheme}.png` });
  await p.screenshot({ path: `${out}/home-waterfall-${w}x${h}-${scheme}-viewport.png` });
  const scope = await p.$("#lbScope");
  const a = await scope.screenshot(); await p.waitForTimeout(2000); const c = await scope.screenshot();
  res[`${w}-${scheme}`] = { height: Math.round(box.height), hscroll: hs.doc > hs.vw, live_differs: !a.equals(c), errs };
  await ctx.close();
}
// hover + reduced motion
{
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: "dark", deviceScaleFactor: 2 });
  const p = await ctx.newPage();
  await p.goto(base + "index.html", { waitUntil: "networkidle" });
  await p.evaluate(() => document.getElementById("lband").scrollIntoView({ block: "start" }));
  await p.waitForSelector("#lband.lb-ready"); await p.waitForTimeout(16000);
  const cv = await p.$("#lbScope canvas"); const bb = await cv.boundingBox();
  await p.mouse.move(bb.x + bb.width * 0.84, bb.y + bb.height * 0.45); await p.waitForTimeout(400);
  await (await p.$("#lband")).screenshot({ path: `${out}/home-waterfall-1440x900-dark-hover.png` });
  res.hoverTip = await p.$eval("#lbTip", (e) => e.textContent);
  await ctx.close();
}
{
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: "light", reducedMotion: "reduce", deviceScaleFactor: 2 });
  const p = await ctx.newPage();
  await p.goto(base + "index.html", { waitUntil: "networkidle" });
  await p.evaluate(() => document.getElementById("lband").scrollIntoView({ block: "start" }));
  await p.waitForSelector("#lband.lb-ready"); await p.waitForTimeout(800);
  const scope = await p.$("#lbScope");
  const a = await scope.screenshot(); await p.waitForTimeout(2000); const c = await scope.screenshot();
  await (await p.$("#lband")).screenshot({ path: `${out}/home-waterfall-1440x900-light-reduced-motion.png` });
  res.reduced_static = a.equals(c);
  await ctx.close();
}
console.log(JSON.stringify(res, null, 1));
await b.close();
