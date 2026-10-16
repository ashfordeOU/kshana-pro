// Element screenshots, before (8843: the pre-swap build) and after (8841), light and dark.
// Run: node shots.mjs <out dir> <widths e.g. 1440,390> <name|page|selector[|tabSelectorToClickFirst]> ...
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
const ROOT = new URL("../../", import.meta.url).pathname;
const require = createRequire(ROOT + "explorations/2026-09-27-audit/matrix/");
const { chromium } = require("playwright");
const [out, widths, ...specs] = process.argv.slice(2);
const bases = process.env.ONLY_AFTER ? { after: "http://127.0.0.1:8841/" } : { before: "http://127.0.0.1:8843/", after: "http://127.0.0.1:8841/" };
mkdirSync(out, { recursive: true });
const b = await chromium.launch(process.env.KSHANA_CHROME ? { executablePath: process.env.KSHANA_CHROME } : {});
for (const w of widths.split(",").map(Number)) for (const theme of ["light", "dark"]) {
  const ctx = await b.newContext({ viewport: { width: w, height: 900 }, colorScheme: theme, reducedMotion: "reduce", deviceScaleFactor: 1 });
  for (const [side, base] of Object.entries(bases)) {
    const byPage = {};
    for (const s of specs) { const [name, pg, sel, tab] = s.split("|"); (byPage[pg] ||= []).push({ name, sel, tab }); }
    for (const [pg, list] of Object.entries(byPage)) {
      const p = await ctx.newPage();
      try {
        await p.goto(base + pg, { waitUntil: "load", timeout: 180000 });
        await p.waitForTimeout(2500);
        const h = await p.evaluate(() => document.documentElement.scrollHeight);
        for (let y = 0; y < h; y += 700) { await p.evaluate((yy) => window.scrollTo(0, yy), y); await p.waitForTimeout(150); }
        for (const { name, sel, tab } of list) {
          try {
            if (tab) { await p.evaluate((t) => { const e = document.querySelector(t); if (e) e.click(); }, tab); await p.waitForTimeout(700); }
            const el = p.locator(sel).first();
            await el.scrollIntoViewIfNeeded({ timeout: 8000 });
            await p.waitForTimeout(900);
            await el.screenshot({ path: `${out}/${name}-${side}-${theme}-${w}.png`, timeout: 30000 });
          } catch (e) { console.log(`MISS ${name} ${side} ${theme} ${w}: ${String(e).split("\n")[0].slice(0, 120)}`); }
        }
      } catch (e) { console.log(`PAGE ${pg} ${side}: ${String(e).split("\n")[0]}`); }
      await p.close();
    }
  }
  await ctx.close();
}
await b.close();
console.log("done");
