import { createRequire } from "node:module";
const ROOT = new URL("../../", import.meta.url).pathname;
const require = createRequire(ROOT + "explorations/2026-09-27-audit/matrix/");
const { chromium } = require("playwright");
const b = await chromium.launch();
for (const base of ["http://127.0.0.1:8843/", "http://127.0.0.1:8841/"]) {
  const p = await (await b.newContext({ viewport: { width: 280, height: 900 } })).newPage();
  await p.goto(base + "docs/scenario-reference.html", { waitUntil: "load", timeout: 180000 }); await p.waitForTimeout(1500);
  console.log(base, JSON.stringify(await p.evaluate(() => { const e = document.getElementById("leo-navmsg-celeste-iod"); const W = innerWidth; const over = [...e.querySelectorAll("*")].filter((x) => x.getBoundingClientRect().right > W + 1).map((x) => `${x.tagName}.${x.className}:${Math.round(x.getBoundingClientRect().right - W)}:${(x.textContent || "").slice(0, 30)}`); const r = e.getBoundingClientRect(); return { right: Math.round(r.right), W, over: over.slice(0, 8), pTexts: [...e.querySelectorAll("p")].map((x) => x.textContent.slice(0, 60)) }; })));
  await p.close();
}
await b.close();
