import { chromium } from "playwright";
const [base,out]=process.argv.slice(2);
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
await p.goto(base+"capabilities.html",{waitUntil:"networkidle"}); await p.click("#cct-solar"); await p.evaluate(()=>document.getElementById("ssGlobe").scrollIntoView({block:"center"}));
for (const t of [500,1500,3000,6000]) { await p.waitForTimeout(t===500?500:t-500); await (await p.$("#ssGlobe")).screenshot({path:`${out}/g2-${t}.png`}); }
await b.close();
