// R4 Developers/Editions screenshot helper. usage: node r4_devshots.mjs OUTDIR page1,page2 [sizes]
import { chromium } from 'playwright';
import fs from 'fs';
const OUT = process.argv[2]; const pages = (process.argv[3]||'developers,editions').split(',');
const sizes = (process.argv[4]||'1440x900,390x844').split(',').map(s=>s.split('x').map(Number));
const port = process.env.PORT||8761;
fs.mkdirSync(OUT,{recursive:true});
const browser = await chromium.launch();
for (const p of pages) for (const [w,h] of sizes) for (const theme of ['dark','light']) {
  const ctx = await browser.newContext({ viewport:{width:w,height:h}, colorScheme: theme });
  await ctx.addInitScript(t=>{ try{localStorage.setItem('theme',t);}catch(e){} }, theme);
  const page = await ctx.newPage(); const errs=[];
  page.on('console', m=>{ if(m.type()==='error') errs.push(m.text()); }); page.on('pageerror', e=>errs.push(String(e)));
  page.on('response', r=>{ if(r.status()>=400) errs.push(r.status()+' '+r.url()); });
  await page.goto(`http://localhost:${port}/${p}.html`, { waitUntil:'load' });
  await page.evaluate(t=>document.documentElement.setAttribute('data-theme',t), theme);
  // scroll through to trigger reveals
  const H = await page.evaluate(()=>document.documentElement.scrollHeight);
  for (let y=0;y<H;y+=600){ await page.evaluate(y=>scrollTo(0,y),y); await page.waitForTimeout(120); }
  await page.evaluate(()=>scrollTo(0,0)); await page.waitForTimeout(1500);
  const f = `${OUT}/${p}-${w}x${h}-${theme}.png`;
  await page.screenshot({ path:f, fullPage:true });
  const m = await page.evaluate(()=>{ const r={}; for (const s of document.querySelectorAll('main section[id], main section')) { const b=s.getBoundingClientRect(); r[s.id||s.className]=Math.round(b.height); } return {sw:document.documentElement.scrollWidth,cw:document.documentElement.clientWidth,H:document.documentElement.scrollHeight,sections:r}; });
  console.log(JSON.stringify({f, errs: errs.slice(0,5), ...m}));
  await ctx.close();
}
await browser.close();
