import { chromium } from "playwright";
const b = await chromium.launch();
for (const w of [320,390,768,1024,1280]) { const p = await (await b.newContext({viewport:{width:w,height:900}})).newPage();
await p.goto(process.argv[2]+"capabilities.html",{waitUntil:"networkidle"}); await p.waitForTimeout(800);
console.log(w, await p.$eval(".cap-bar",e=>{const r=e.getBoundingClientRect(); const t=e.querySelector(".right")?.getBoundingClientRect(); return {h:Math.round(r.height),sw:e.scrollWidth,cw:e.clientWidth,right:t&&[Math.round(t.left),Math.round(t.right),Math.round(t.top-r.top)], ov:getComputedStyle(e).overflow};})); }
await b.close();
