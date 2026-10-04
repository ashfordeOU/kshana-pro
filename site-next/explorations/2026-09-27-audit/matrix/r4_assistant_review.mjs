// R4 review of Home #assistant. usage: node r4_assistant_review.mjs OUTDIR [tag]
import { chromium } from 'playwright';
import { PNG } from 'pngjs';
import fs from 'fs';
const OUT = process.argv[2]; const TAG = process.argv[3] || 'review';
const port = process.env.PORT || 8751;
fs.mkdirSync(OUT, { recursive: true });
const sizes = [[1440,900],[1920,1080],[768,1024],[390,844]];
function diff(a,b){ const A=PNG.sync.read(a),B=PNG.sync.read(b); if(A.width!==B.width||A.height!==B.height) return 1; let n=0; for(let i=0;i<A.data.length;i+=4){ if(Math.abs(A.data[i]-B.data[i])+Math.abs(A.data[i+1]-B.data[i+1])+Math.abs(A.data[i+2]-B.data[i+2])>30) n++; } return n/(A.width*A.height); }
const browser = await chromium.launch();
for (const [w,h] of sizes) for (const theme of ['dark','light']) {
  const ctx = await browser.newContext({ viewport:{width:w,height:h}, colorScheme: theme, deviceScaleFactor: 1 });
  await ctx.addInitScript(t=>{ try{localStorage.setItem('theme',t);}catch(e){} }, theme);
  const page = await ctx.newPage(); const errs=[];
  page.on('console', m=>{ if(m.type()==='error') errs.push(m.text()); }); page.on('pageerror', e=>errs.push(String(e)));
  page.on('response', r=>{ if(r.status()>=400) errs.push(r.status()+' '+r.url()); });
  await page.goto(`http://localhost:${port}/index.html`, { waitUntil:'load' });
  await page.evaluate(t=>document.documentElement.setAttribute('data-theme',t), theme);
  const H0 = await page.evaluate(()=>document.documentElement.scrollHeight);
  for (let y=0;y<H0;y+=700){ await page.evaluate(y=>scrollTo(0,y),y); await page.waitForTimeout(60); }
  await page.evaluate(()=>{ const s=document.getElementById('assistant'); scrollTo(0, s.getBoundingClientRect().top+scrollY-64); });
  await page.waitForTimeout(1200);
  const sec = page.locator('#assistant');
  const a = await sec.screenshot(); await page.waitForTimeout(2000); const b = await sec.screenshot();
  await page.waitForTimeout(4200); const c = await sec.screenshot();
  fs.writeFileSync(`${OUT}/assistant-${TAG}-${w}x${h}-${theme}.png`, c);
  const m = await page.evaluate(()=>{
    const q=s=>document.querySelector(s), R=e=>e.getBoundingClientRect();
    const s=q('#assistant'), sel=q('#mcpSteps [aria-selected="true"]');
    const clipped=[...s.querySelectorAll('*')].filter(e=>{ if(!e.offsetParent&&getComputedStyle(e).position!=='fixed') return false; const cs=getComputedStyle(e); return (cs.overflowX==='hidden'||cs.textOverflow==='ellipsis') && e.scrollWidth>e.clientWidth+1 && e.clientWidth>0 && !e.closest('.vh') && !(e.classList.contains('l')); }).map(e=>e.className+':'+e.textContent.slice(0,30));
    const lede=q('#assistant .sec-head>.lede'); const lh=parseFloat(getComputedStyle(lede).lineHeight);
    const h2=q('#assistant .h2');
    const out=[...s.querySelectorAll('*')].filter(e=>{const r=R(e);return r.width&&r.right>document.documentElement.clientWidth+1}).map(e=>e.className).slice(0,5);
    const panels=[...document.querySelectorAll('.mp-panel')].map(p=>({sh:p.scrollHeight,ch:p.clientHeight}));
    const small=[...s.querySelectorAll('button,a,summary,[role=tab]')].filter(e=>{const r=R(e);return r.width&&(r.width<24||r.height<24)}).map(e=>(e.className||e.tagName)+':'+e.textContent.trim().slice(0,20));
    return { secH:Math.round(R(s).height), pageH:document.documentElement.scrollHeight, hscroll:document.documentElement.scrollWidth>document.documentElement.clientWidth,
      step: sel&&sel.id, playing: q('#mcpChat').classList.contains('playing'), ledeLines: Math.round(R(lede).height/lh), h2op:getComputedStyle(h2).opacity,
      instH:Math.round(R(q('.asst-inst')).height), mpH:Math.round(R(q('#mcpChat')).height), panels, clipped:clipped.slice(0,6), out, small:small.slice(0,8),
      hero:Math.round(R(q('.hero')||q('section')).height), story: q('#story')?Math.round(R(q('#story')).height):null };
  });
  console.log(JSON.stringify({ tag:`${w}x${h}-${theme}`, diff2s:+(diff(a,b)*100).toFixed(2), diff6s:+(diff(a,c)*100).toFixed(2), errs:errs.slice(0,4), ...m }));
  await ctx.close();
}
await browser.close();
