import { chromium } from "playwright";
const [base,sel,tab]=process.argv.slice(2);
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
await p.goto(base+"capabilities.html",{waitUntil:"networkidle"}); if(tab) await p.click(tab);
await p.evaluate(s=>document.querySelector(s).scrollIntoView({block:"center"}),sel); await p.waitForTimeout(1200);
const e=await p.$(sel); const a=await e.screenshot(); const t1=await p.$eval("#hoT",x=>x.textContent).catch(()=>"");
await p.waitForTimeout(2000); const c=await e.screenshot(); const t2=await p.$eval("#hoT",x=>x.textContent).catch(()=>"");
console.log(sel,"live",!a.equals(c),t1,"->",t2); await b.close();
