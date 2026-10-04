import { chromium } from "playwright";
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:1440,height:900},colorScheme:"light"})).newPage();
await p.goto(process.argv[2]+"capabilities.html#journey",{waitUntil:"networkidle"}); await p.waitForTimeout(1500);
console.log(await p.evaluate(()=>{ const out=[]; for (const sel of ["#journey .fig-acts a, #journey .acts a, #journey a","#runs a"]) {} 
 const find=(root,txt)=>[...document.querySelectorAll(root+" a, "+root+" button")].find(a=>a.textContent.trim()===txt);
 for (const [r,t] of [["#journey","Report"],["#journey","Scenario file"],["#runs","Report"]]) { const a=find(r,t); if(!a){out.push(r+" none");continue;} const s=getComputedStyle(a); out.push(r+" "+t+" class="+a.className+" parent="+a.parentElement.className+" border="+s.border+" bg="+s.backgroundColor); }
 return out.join("\n");}));
await b.close();
