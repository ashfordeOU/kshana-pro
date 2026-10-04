// Side-by-side review of rebuilt Home vs Observatory original.
import { chromium } from 'playwright';
import { PNG } from 'pngjs';
import fs from 'fs';
const OUT = process.argv[2] || '../shots/observatory-review';
const only = process.argv[3] || '';
const sites = { home: 'http://localhost:8731/index.html', original: 'http://localhost:8741/index.html' };
const sizes = [[1440,900],[1920,1080],[390,844],[768,1024]];
const themes = ['dark','light'];
function diff(a,b){ const A=PNG.sync.read(a),B=PNG.sync.read(b); let n=0; for(let i=0;i<A.data.length;i+=4){ if(Math.abs(A.data[i]-B.data[i])+Math.abs(A.data[i+1]-B.data[i+1])+Math.abs(A.data[i+2]-B.data[i+2])>30) n++; } return n/(A.width*A.height); }
const browser = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
const results = [];
for (const [name,url] of Object.entries(sites)) for (const [w,h] of sizes) for (const theme of themes) {
  const tag = `${name}-${w}x${h}-${theme}`; if (only && !tag.includes(only)) continue;
  const ctx = await browser.newContext({ viewport:{width:w,height:h}, colorScheme: theme, deviceScaleFactor:1 });
  const page = await ctx.newPage(); const errs=[];
  page.on('console', m=>{ if(m.type()==='error') errs.push(m.text()); }); page.on('pageerror', e=>errs.push(String(e)));
  page.on('requestfailed', r=>errs.push('FAIL '+r.url())); page.on('response', r=>{ if(r.status()>=400) errs.push(r.status()+' '+r.url()); });
  await page.goto(url, { waitUntil:'load' });
  await page.evaluate(t=>{ try{localStorage.setItem('theme',t);}catch(e){} document.documentElement.setAttribute('data-theme',t); }, theme);
  await page.waitForTimeout(16000);
  const a = await page.screenshot(); await page.waitForTimeout(2000); const b = await page.screenshot();
  fs.writeFileSync(`${OUT}/${tag}-hero.png`, b);
  const d = diff(a,b);
  const m = await page.evaluate(()=>{
    const q=s=>document.querySelector(s); const r=e=>e?e.getBoundingClientRect():null;
    const h1=q('h1'); const cs=h1?getComputedStyle(h1):null;
    return { sw: document.documentElement.scrollWidth, cw: document.documentElement.clientWidth, H: document.documentElement.scrollHeight,
      h1: cs? {fs:cs.fontSize, ff:cs.fontFamily.slice(0,40), lh:cs.lineHeight, ls:cs.letterSpacing, fw:cs.fontWeight, rect:r(h1)}:null,
      canvases:[...document.querySelectorAll('canvas')].map(c=>{const b=r(c);return [Math.round(b.x),Math.round(b.y),Math.round(b.width),Math.round(b.height)]}).slice(0,6) };
  });
  results.push({tag, heroDiff2s: +(d*100).toFixed(2), errs: errs.slice(0,5), ...m});
  console.log(JSON.stringify(results.at(-1)));
  await ctx.close();
}
await browser.close();
fs.writeFileSync(`${OUT}/review.json`, JSON.stringify(results,null,1));
