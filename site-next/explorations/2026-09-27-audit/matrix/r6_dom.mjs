import { chromium } from "playwright";
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
await p.goto(process.argv[2],{waitUntil:"networkidle"});
console.log(await p.evaluate(()=>[...document.querySelectorAll("section[id]")].map(s=>s.id+" "+(s.parentElement.closest("section")?.id||"-")+" "+Math.round(s.getBoundingClientRect().top+scrollY)+" "+Math.round(s.getBoundingClientRect().height)).join("\n")));
console.log(await p.evaluate(()=>document.documentElement.scrollHeight));
await b.close();
