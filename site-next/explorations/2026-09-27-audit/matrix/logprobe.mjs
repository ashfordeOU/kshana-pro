import { chromium } from 'playwright';
const b = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
const p = await (await b.newContext({viewport:{width:1920,height:1080}, colorScheme:'dark'})).newPage();
await p.goto('http://localhost:8731/index.html');
for (let k=0;k<12;k++){ await p.waitForTimeout(2000);
 console.log(k, JSON.stringify(await p.evaluate(()=>{const L=document.querySelector('#hLog'); const r=L.getBoundingClientRect(); return {clk:document.querySelector('#hClock').textContent, h:Math.round(r.height), lines:[...L.children].map(c=>[c.textContent.slice(0,22), Math.round(c.getBoundingClientRect().top-r.top), getComputedStyle(c).opacity])}})));}
await b.close();
