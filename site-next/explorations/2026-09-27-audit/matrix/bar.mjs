import { chromium } from 'playwright';
const b = await chromium.launch();
for (const w of [1280,1100,1000,940]) { const p = await (await b.newContext({viewport:{width:w,height:800}, reducedMotion:'reduce'})).newPage();
await p.goto('http://localhost:8731/index.html',{waitUntil:'domcontentloaded'}); await p.waitForTimeout(1500);
const r = await p.evaluate(()=>[...document.querySelector('.console-bar').children].map(e=>[e.className||e.tagName, Math.round(e.getBoundingClientRect().left), Math.round(e.getBoundingClientRect().width), Math.round(e.getBoundingClientRect().height), e.textContent.slice(0,30)]));
console.log(w, JSON.stringify(r)); await p.screenshot({path:`../shots/observatory-review/bar-${w}.png`, clip:{x:0,y:70,width:w,height:60}}); }
await b.close();
