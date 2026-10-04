// R4 capabilities critical review: full-page shots, liveness diffs, hscroll, console. Usage: node r4_cap_review.mjs <base> <outdir>
import { chromium } from "playwright";
const base = process.argv[2] || "http://127.0.0.1:18777/";
const out = process.argv[3] || "/tmp/capshots";
const sizes = [[1440,900],[1920,1080],[768,1024],[390,844]];
const b = await chromium.launch();
const res = {};
for (const [w,h] of sizes) for (const scheme of ["dark","light"]) {
  const ctx = await b.newContext({ viewport:{width:w,height:h}, colorScheme:scheme, deviceScaleFactor:1 });
  const p = await ctx.newPage();
  const errs=[]; p.on("console",m=>{ if(m.type()==="error"||m.type()==="warning") errs.push(m.text().slice(0,200)); }); p.on("pageerror",e=>errs.push(String(e).slice(0,200)));
  await p.goto(base+"capabilities.html",{waitUntil:"networkidle"});
  const ids = await p.evaluate(()=>[...document.querySelectorAll("main section[id], section[id]")].map(s=>s.id));
  const r = { errs, live:{}, heights:{} };
  for (const id of ids) {
    await p.evaluate(id=>document.getElementById(id).scrollIntoView({block:"start"}), id);
    await p.waitForTimeout(1500);
    const el = await p.$("#"+id); const bb = await el.boundingBox(); if(!bb) continue;
    r.heights[id]=Math.round(bb.height);
    const clipH = Math.min(h, bb.height);
    const a = await p.screenshot({clip:{x:0,y:0,width:w,height:h}});
    await p.waitForTimeout(2000);
    const c = await p.screenshot({clip:{x:0,y:0,width:w,height:h}});
    r.live[id] = !a.equals(c);
    await p.screenshot({path:`${out}/${w}x${h}-${scheme}-${id}.png`});
  }
  r.hs = await p.evaluate(()=>({doc:document.documentElement.scrollWidth, vw:innerWidth}));
  res[`${w}x${h}-${scheme}`]=r;
  await ctx.close();
}
console.log(JSON.stringify(res,null,1));
await b.close();
