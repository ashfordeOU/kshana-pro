import { chromium } from 'playwright';
const b = await chromium.launch();
for (const w of [320,300,280]) { const p = await (await b.newContext({viewport:{width:w,height:700}, reducedMotion:'reduce'})).newPage();
await p.goto('http://localhost:8731/index.html',{waitUntil:'domcontentloaded'}); await p.waitForTimeout(1500);
const r = await p.evaluate((w)=>{ const out=[]; for (const e of document.querySelectorAll('#top *, #nav *, #timeline *')) { const r=e.getBoundingClientRect(); if (r.width && r.right>w+0.5 && getComputedStyle(e).position!=='fixed') out.push([e.tagName+'.'+(e.className?.baseVal??e.className), Math.round(r.left), Math.round(r.right)]); } return [document.documentElement.scrollWidth, out.slice(0,12)]; }, w);
console.log(w, JSON.stringify(r)); }
await b.close();
