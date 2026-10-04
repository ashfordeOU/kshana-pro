import { chromium } from 'playwright';
const [w,h,theme,...sels] = process.argv.slice(2);
const b = await chromium.launch({ args:['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
const p = await (await b.newContext({viewport:{width:+w,height:+h}, colorScheme:theme, reducedMotion:'reduce'})).newPage();
await p.goto('http://localhost:8731/index.html',{waitUntil:'domcontentloaded'}); await p.waitForTimeout(2500);
let k=0; for (const s of sels) { const e = p.locator(s).first(); await e.scrollIntoViewIfNeeded(); await p.waitForTimeout(1500); await e.screenshot({path:`../shots/observatory-review/el-${k++}.png`}); }
await b.close();
