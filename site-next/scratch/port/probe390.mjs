import { createRequire } from "node:module";
import { join } from "node:path";
const require = createRequire(join(process.env.HOME, "Code/kshana-site-next/explorations/2026-09-27-audit/matrix/package.json"));
const { chromium } = require("playwright");
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 390, height: 844 } });
const p = await ctx.newPage();
await p.goto("http://127.0.0.1:8861/playground/index.html?scenario=integrity-raim&tab=overview");
await p.waitForFunction(() => /^Ran locally/.test(document.getElementById("status").textContent), null, { timeout: 120000 });
const info = await p.evaluate(() => {
  const r = document.getElementById("run"); const bb = r.getBoundingClientRect();
  const cs = getComputedStyle(r);
  const chain = []; for (let e = r; e; e = e.parentElement) { const c = getComputedStyle(e); chain.push(`${e.tagName}#${e.id}.${e.className} d=${c.display} v=${c.visibility} ${e.hidden ? "HIDDEN" : ""} ${e.tagName === "DETAILS" ? "open=" + e.open : ""}`); }
  const top = document.elementFromPoint(Math.max(1, bb.x + bb.width / 2), Math.max(1, bb.y + bb.height / 2));
  return { bb: [bb.x, bb.y, bb.width, bb.height], disp: cs.display, vis: cs.visibility, disabled: r.disabled, chain: chain.slice(0, 8), top: top ? `${top.tagName}#${top.id}.${top.className}` : null, scrollY, docW: document.documentElement.scrollWidth };
});
console.log(JSON.stringify(info, null, 1));
await p.screenshot({ path: "shots/studio-probe-390.png" });
await b.close();
