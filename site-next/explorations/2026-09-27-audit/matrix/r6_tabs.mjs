// click every console tab, element-shot each panel, liveness diff 2 s apart, overlap/clip probe inside panel
import { chromium } from "playwright";
const [base,out,sz,schemes]=[process.argv[2],process.argv[3],process.argv[4]||"1440x900",process.argv[5]||"dark,light"];
const [w,h]=sz.split("x").map(Number);
const b = await chromium.launch(); const res={};
for (const scheme of schemes.split(",")) {
  const ctx = await b.newContext({viewport:{width:w,height:h},colorScheme:scheme}); const p=await ctx.newPage(); const errs=[];
  p.on("console",m=>{ if(["error","warning"].includes(m.type())) errs.push(m.text().slice(0,200)); }); p.on("pageerror",e=>errs.push(String(e)));
  await p.goto(base+"capabilities.html",{waitUntil:"networkidle"});
  const tabs = await p.$$eval("[data-cc] .cc-tabs [role=tab]",ts=>ts.map(t=>t.id));
  for (const t of tabs) {
    await p.click("#"+t); await p.waitForTimeout(300);
    const pid = await p.$eval("#"+t,e=>e.getAttribute("aria-controls"));
    const panel = await p.$("#"+pid);
    const hgt = (await panel.boundingBox()).height;
    // scroll through panel so lazy things init
    for (let y=0;y<hgt;y+=h/2){ await p.evaluate(([id,y])=>{const e=document.getElementById(id); scrollTo(0,e.getBoundingClientRect().top+scrollY+y-80);},[pid,y]); await p.waitForTimeout(250);}
    await p.evaluate(id=>{const e=document.getElementById(id); scrollTo(0,e.getBoundingClientRect().top+scrollY-80);},pid); await p.waitForTimeout(1200);
    const a=await p.screenshot(); await p.waitForTimeout(2000); const c=await p.screenshot();
    await panel.screenshot({path:`${out}/${w}-${scheme}-${pid}.png`});
    const probe = await p.evaluate(id=>{const P=document.getElementById(id); const out=[];
      for(const e of P.querySelectorAll("*")){ const s=getComputedStyle(e); if(s.visibility==="hidden"||s.display==="none") continue;
        if(e.children.length===0 && e.textContent.trim() && (e.scrollWidth>e.clientWidth+2) && /(hidden|clip)/.test(s.overflowX) && s.textOverflow!=="ellipsis") out.push("CLIP "+e.tagName+"."+e.className+" '"+e.textContent.trim().slice(0,40)+"'");
        const t=e.textContent.trim(); if(/NaN|undefined|null|Infinity|\{\{/.test(e.children.length===0?t:"")) out.push("BAD '"+t.slice(0,60)+"'"); }
      return out.slice(0,15);},pid);
    res[scheme+" "+pid]={h:Math.round(hgt), live:!a.equals(c), probe};
  }
  res[scheme+" errs"]=errs; await ctx.close();
}
console.log(JSON.stringify(res,null,1)); await b.close();
