import { chromium } from 'playwright';
import { PNG } from 'pngjs'; import fs from 'fs';
const OUT='../shots/observatory-review/final';
const b = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
for (const [w,h] of [[1440,900],[390,844]]) for (const theme of ['dark','light']) {
  const ctx = await b.newContext({ viewport:{width:w,height:h}, colorScheme:theme, reducedMotion:'reduce' });
  const p = await ctx.newPage(); const errs=[]; p.on('pageerror',e=>errs.push(String(e))); p.on('console',m=>{if(m.type()==='error')errs.push(m.text())});
  await p.goto('http://localhost:8731/index.html',{waitUntil:'domcontentloaded',timeout:90000}); await p.waitForTimeout(6000);
  const H = await p.evaluate(()=>document.documentElement.scrollHeight);
  const out = new PNG({width:w, height:H}); const slices=[];
  for (let y=0;y<H;y+=h){ const yy=Math.min(y,H-h); await p.evaluate(y=>scrollTo(0,y),yy); if(y>0) await p.evaluate(()=>{const n=document.querySelector('#nav'); if(n){n.style.transition='none';n.style.display='none';}});
    await p.waitForTimeout(y===0?12000:1800); const buf=await p.screenshot({timeout:120000}); const img=PNG.sync.read(buf);
    PNG.bitblt(img,out,0,0,w,h,0,yy); }
  fs.writeFileSync(`${OUT}/home-${w}x${h}-${theme}-full.png`, PNG.sync.write(out));
  console.log(w,theme,H,errs); await ctx.close();
}
await b.close();
