import { chromium } from "playwright";
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
await p.goto(process.argv[2]+"capabilities.html",{waitUntil:"networkidle"});
let txt = await p.evaluate(()=>document.querySelector("main").innerText);
for (const t of await p.$$eval("[role=tab]",ts=>ts.map(t=>t.id))) { try{ await p.click("#"+t,{timeout:1500}); await p.waitForTimeout(300); txt += "\n"+await p.evaluate(()=>document.querySelector("main").innerText);}catch{} }
console.log(txt); await b.close();
