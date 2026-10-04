import { chromium } from "playwright";
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
await p.goto(process.argv[2]+"capabilities.html",{waitUntil:"networkidle"});
for (const t of await p.$$eval("[role=tab]",ts=>ts.map(t=>t.id))) { try{ await p.click("#"+t,{timeout:1500}); await p.waitForTimeout(150);}catch{} }
const hrefs = await p.$$eval("a[href]",as=>[...new Set(as.map(a=>a.href))]);
const bad=[];
for (const h of hrefs) { if(!h.startsWith(process.argv[2])) continue; const u=h.split("#")[0].split("?")[0]; const r=await p.request.get(u); if(r.status()>=400) bad.push(r.status()+" "+h); 
  if (h.includes("#") && u.endsWith(".html")) { const frag=h.split("#")[1]; if(frag && u.endsWith("capabilities.html")) { const ok=await p.evaluate(f=>!!document.getElementById(f),frag); if(!ok) bad.push("NOANCHOR "+h);} } }
console.log(hrefs.length, "links"); console.log(bad.join("\n")); await b.close();
