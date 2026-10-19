// Screenshots of the ported site for the 0.38.0 integration review. Run: node shoot.mjs http://127.0.0.1:PORT OUT_DIR [OBS_FIXTURE_DIR]  (web/ served at that address; Playwright installed)
import { createRequire } from "node:module";
import { readdirSync } from "node:fs";
const require = createRequire("/opt/node22/lib/node_modules/");
const { chromium } = require("playwright");
const [base, out, obs] = process.argv.slice(2);
const pages = [["home", "/index.html"], ["capabilities", "/capabilities.html"], ["evidence", "/evidence.html"], ["developers", "/developers.html"], ["benchmark", "/benchmark.html"],
  ["replay", "/studio/replay/index.html"], ["optical", "/studio/optical/index.html"], ["holdover", "/studio/holdover/index.html"], ["studio-observatory", "/studio/observatory/index.html"], ["studio-observatory-synthetic-days", "/studio/observatory/index.html", "obs"]];
const b = await chromium.launch({ executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args: ["--no-sandbox"] });
const bad = [];
for (const [w, h, scheme] of [[1440, 1000, "light"], [1440, 1000, "dark"], [390, 900, "light"], [390, 900, "dark"]]) {
  const ctx = await b.newContext({ viewport: { width: w, height: h }, colorScheme: scheme });
  const p = await ctx.newPage(); const errs = [];
  p.on("pageerror", (e) => errs.push(e.message)); p.on("requestfailed", (r) => { if (!/fonts\.(googleapis|gstatic)/.test(r.url())) errs.push("failed " + r.url()); });
  p.on("request", (r) => { const u = new URL(r.url()); if (!["127.0.0.1", "localhost"].includes(u.hostname) && u.protocol.startsWith("http")) errs.push("third-party " + u.host); });
  for (const [name, path, mode] of pages) {
    if (mode === "obs" && !obs) continue;
    await p.goto(base + path, { waitUntil: "load" }); await p.waitForTimeout(name.startsWith("home") ? 1500 : 2500);
    if (mode === "obs") { await p.setInputFiles("#ko-files", readdirSync(obs).map((f) => `${obs}/${f}`)); await p.waitForSelector("#ko-body:not([hidden])"); await p.waitForTimeout(800); }
    await p.screenshot({ path: `${out}/${name}-${w}-${scheme}.png`, fullPage: false, clip: { x: 0, y: 0, width: w, height: Math.min(h * 2, await p.evaluate(() => document.documentElement.scrollHeight)) } });
    const over = await p.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 2);
    if (over) bad.push(`${name} ${w} ${scheme}: horizontal page scroll`);
  }
  if (errs.length) bad.push(`${w} ${scheme}: ${[...new Set(errs)].slice(0, 5).join(" | ")}`);
  await ctx.close();
}
console.log(bad.length ? "ISSUES:\n" + bad.join("\n") : "no page errors, no third-party request, no horizontal page scroll");
await b.close();
