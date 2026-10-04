import { createRequire } from "node:module";
const ROOT = new URL("../../", import.meta.url).pathname;
const require = createRequire(ROOT + "explorations/2026-09-27-audit/matrix/");
const { chromium } = require("playwright");
const [url, expr, wait = "4000"] = process.argv.slice(2);
const b = await chromium.launch(); const p = await (await b.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
const errs = []; p.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); }); p.on("pageerror", (e) => errs.push(String(e)));
p.on("response", (r) => { if (r.status() >= 400) errs.push(r.status() + " " + r.url()); });
await p.goto(url, { waitUntil: "load", timeout: 120000 }); await p.waitForTimeout(+wait);
const h = await p.evaluate(() => document.documentElement.scrollHeight);
for (let y = 0; y < h; y += 600) { await p.evaluate((yy) => window.scrollTo(0, yy), y); await p.waitForTimeout(300); }
console.log(JSON.stringify(await p.evaluate(new Function("return (" + expr + ")()")), null, 1)); console.log("errors:", errs); await b.close();
