import { chromium } from "playwright";
const b = await chromium.launch(); const out = "/Users/chak/Code/kshana-site-next/scratch/assembly/shots/";
for (const [w, theme] of [[1200, "light"], [1600, "dark"], [1440, "light"], [390, "light"]]) {
  const ctx = await b.newContext({ viewport: { width: w, height: 800 }, colorScheme: theme, deviceScaleFactor: 2 });
  await ctx.addInitScript(() => { try { localStorage.setItem("kshana-studio-hint", "done"); } catch {} });
  const p = await ctx.newPage(); await p.goto("http://127.0.0.1:8853/?scenario=jamming-demo&tab=signal", { waitUntil: "networkidle" });
  await p.waitForSelector("#kpis-wrap:not([hidden])", { timeout: 60000 }); await p.waitForTimeout(1200);
  await p.screenshot({ path: `${out}studio-fix-${w}-${theme}.png`, clip: { x: 0, y: 0, width: w, height: w < 500 ? 800 : 380 } });
  const r = await p.evaluate(() => [...document.querySelectorAll("#crumbs .cr")].map((e) => [e.textContent.trim(), e.clientWidth, e.scrollWidth, getComputedStyle(e).textOverflow]));
  console.log(w, theme, JSON.stringify(r)); await ctx.close();
}
await b.close();
