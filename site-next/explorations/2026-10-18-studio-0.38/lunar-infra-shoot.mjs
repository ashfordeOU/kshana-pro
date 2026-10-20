// Screenshots and checks of the Lunar infrastructure page. Run: node ssv-shoot.mjs http://127.0.0.1:PORT OUT_DIR  (the Studio folder served at that address)
import { createRequire } from "node:module";
const require = createRequire("/opt/node22/lib/node_modules/");
const { chromium } = require("playwright");
const [base, out] = process.argv.slice(2);
const ids = ["od-consistency", "carrier-phase", "frame-coestimation", "space-tie", "geodesy-sensitivity"];
const b = await chromium.launch({ executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args: ["--no-sandbox"] });
let bad = 0;
for (const [w, h, scheme] of [[1440, 1000, "light"], [1440, 1000, "dark"], [390, 900, "light"], [390, 900, "dark"]]) {
  const p = await (await b.newContext({ viewport: { width: w, height: h }, colorScheme: scheme })).newPage(); const errs = [];
  p.on("pageerror", (e) => errs.push(e.message)); p.on("requestfailed", (r) => { if (!/fonts\./.test(r.url())) errs.push("failed " + r.url()); });
  for (const id of ids) {
    await p.goto(`${base}/lunar-infra/index.html?study=${id}`); await p.waitForSelector("#luk-kv div"); await p.waitForSelector("#luk-charts section"); await p.waitForTimeout(400);
    const n = await p.$$eval("#luk-charts section", (s) => s.length), over = await p.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1);
    const text = await p.$eval("body", (e) => e.innerText);
    if (!n || over || !/Advisory software/.test(text) || !/Against the pre-registered bars/.test(text) || /undefined|NaN/.test(text)) { bad++; console.log("ISSUE", id, w, scheme, { charts: n, over, nan: /undefined|NaN/.test(text) }); }
    if (id === "od-consistency" || w === 1440 && scheme === "light") await p.screenshot({ path: `${out}/lunar-infra-${id}-${w}-${scheme}.png`, fullPage: true, clip: { x: 0, y: 0, width: w, height: Math.min(await p.evaluate(() => document.documentElement.scrollHeight), w > 600 ? 3200 : 4800) } });
  }
  if (errs.length) { bad++; console.log("ERRORS", w, scheme, [...new Set(errs)].slice(0, 3)); }
  await p.context().close();
}
console.log(bad ? `${bad} issue(s)` : "Lunar infrastructure page: all five scenarios open at 1440 and 390 px, light and dark, no page error, no horizontal scroll");
await b.close(); process.exit(bad ? 1 : 0);
