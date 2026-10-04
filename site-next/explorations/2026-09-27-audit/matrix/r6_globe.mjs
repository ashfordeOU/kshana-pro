import { chromium } from "playwright";
const [base,out]=process.argv.slice(2);
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
await p.goto(base+"capabilities.html#journey",{waitUntil:"networkidle"}); await p.waitForTimeout(2500);
for (const nm of ["Earth","Mars","Jupiter","Moon"]) { await p.click(`button[data-body="${nm}"]`); await p.waitForTimeout(1500); await (await p.$("#ssGlobe")).screenshot({path:`${out}/globe-${nm}.png`}); }
await b.close();
