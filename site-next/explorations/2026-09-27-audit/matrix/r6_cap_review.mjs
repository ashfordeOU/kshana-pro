// R6 capabilities review: per-section shots, 2 s liveness diff, hscroll, console, overflowing elements.
import { chromium } from "playwright";
const base = process.argv[2], out = process.argv[3];
const sizes = (process.argv[4]||"1440x900,1920x1080,768x1024,390x844").split(",").map(s=>s.split("x").map(Number));
const b = await chromium.launch(); const res = {};
for (const [w,h] of sizes) for (const scheme of ["dark","light"]) {
  const ctx = await b.newContext({ viewport:{width:w,height:h}, colorScheme:scheme });
  const p = await ctx.newPage(); const errs=[];
  p.on("console",m=>{ if(["error","warning"].includes(m.type())) errs.push(m.text().slice(0,200)); }); p.on("pageerror",e=>errs.push(String(e).slice(0,200)));
  p.on("requestfailed",r=>errs.push("REQFAIL "+r.url()));
  p.on("response",r=>{ if(r.status()>=400) errs.push("HTTP"+r.status()+" "+r.url()); });
  await p.goto(base+"capabilities.html",{waitUntil:"networkidle"});
  const ids = await p.evaluate(()=>[...document.querySelectorAll("section[id]")].map(s=>s.id));
  const r = { errs, live:{}, heights:{} };
  for (const id of ids) {
    await p.evaluate(id=>document.getElementById(id).scrollIntoView({block:"start"}), id);
    await p.waitForTimeout(1200);
    const bb = await (await p.$("#"+id)).boundingBox(); if(!bb) continue;
    r.heights[id]=Math.round(bb.height);
    const a = await p.screenshot(); await p.waitForTimeout(2000); const c = await p.screenshot();
    r.live[id] = !a.equals(c);
    await p.screenshot({path:`${out}/${w}x${h}-${scheme}-${id}.png`});
  }
  r.hs = await p.evaluate(()=>({doc:document.documentElement.scrollWidth, vw:innerWidth}));
  r.wide = await p.evaluate(()=>{const vw=innerWidth, o=[]; for(const e of document.querySelectorAll("main *")){const b=e.getBoundingClientRect(); if(b.width>0&&(b.right>vw+1)){ let p=e.parentElement, clipped=false; while(p&&p!==document.body){const s=getComputedStyle(p); if(/(hidden|auto|scroll|clip)/.test(s.overflowX)){clipped=true;break;} p=p.parentElement;} if(!clipped) o.push((e.id?'#'+e.id:e.tagName+'.'+[...e.classList].join('.'))+' r='+Math.round(b.right));}} return o.slice(0,10);});
  res[`${w}x${h}-${scheme}`]=r; await ctx.close();
}
console.log(JSON.stringify(res,null,1)); await b.close();
