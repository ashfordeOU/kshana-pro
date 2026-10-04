import { chromium } from "playwright";
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
for (const id of ["integrity","runs","clocks","timing","constellations","journey","campaigns","studio","explorer","standards","bands","atlas"]) {
  await p.goto(process.argv[2]+"capabilities.html#"+id,{waitUntil:"networkidle"}); await p.waitForTimeout(800);
  const r = await p.evaluate(id=>{const e=document.getElementById(id); const b=e.getBoundingClientRect(); return [Math.round(b.top),Math.round(b.height)];},id);
  console.log(id, r);
}
// in-page link click
await p.goto(process.argv[2]+"capabilities.html",{waitUntil:"networkidle"});
console.log(await p.$$eval('a[href^="#"]',as=>as.map(a=>a.getAttribute("href")).join(" ")));
await b.close();
