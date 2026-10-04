import { chromium } from "playwright";
const [url,sel,out,w,h,scheme,wait]=process.argv.slice(2);
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:+w,height:+h},colorScheme:scheme||"dark"})).newPage();
await p.goto(url,{waitUntil:"networkidle"}); const e=await p.$(sel); await e.scrollIntoViewIfNeeded(); await p.waitForTimeout(+(wait||2500));
await e.screenshot({path:out}); await b.close();
