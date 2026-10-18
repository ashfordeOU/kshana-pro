import { createRequire } from "node:module";
const require = createRequire("/opt/node22/lib/node_modules/");
const { chromium } = require("playwright");
const [base, dir, out] = process.argv.slice(2);
const b = await chromium.launch({ executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args: ["--no-sandbox"] });
let fails = 0; const ok = (c, m) => { console.log(c ? "ok  " : "FAIL", m); if (!c) fails++; };
for (const [w, h, scheme] of [[1440, 1000, "light"], [1440, 1000, "dark"], [390, 900, "light"], [390, 900, "dark"]]) {
  const ctx = await b.newContext({ viewport: { width: w, height: h }, colorScheme: scheme, acceptDownloads: true });
  const p = await ctx.newPage(); const errs = []; p.on("pageerror", (e) => errs.push(e.message)); p.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
  await p.goto(`${base}/tools/index.html#jammertest`); await p.waitForSelector("#jt-cat", { state: "attached" });
  await p.waitForTimeout(1500);
  await p.setInputFiles("#jt-cat", `${dir}/catalogue.json`);
  await p.setInputFiles("#jt-plans", [`${dir}/plan.json`]);
  await p.setInputFiles("#jt-log", `${dir}/log.nmea`);
  await p.selectOption("#jt-format", "nmea");
  await p.fill("#jt-off", "0"); await p.fill("#jt-loc", "1"); await p.fill("#jt-start", "2025-09-15T12:00:00Z");
  await p.click("#jt-import"); await p.waitForTimeout(1500);
  const imp = await p.$eval("#jt-out", (e) => e.textContent);
  if (w === 1440 && scheme === "light") ok(/2/.test(imp) && !/error|fail/i.test(imp), "import: " + imp.slice(0, 160));
  await p.click("#jt-score"); await p.waitForTimeout(2500);
  const sc = await p.$eval("#jt-out", (e) => e.textContent);
  if (w === 1440 && scheme === "light") {
    ok(/detected/i.test(sc) && /missed/i.test(sc), "score shows detected and missed: " + sc.slice(0, 300));
    ok(/Advisory/i.test(await p.textContent("body")), "advisory present");
  }
  ok(errs.length === 0, `no page errors ${w} ${scheme} ${errs.join("|").slice(0, 200)}`);
  await p.screenshot({ path: `${out}/tools-jammertest-${w}-${scheme}.png`, fullPage: true });
  await ctx.close();
}
await b.close(); process.exit(fails ? 1 : 0);
