import { chromium } from "playwright";
const [base,out,page,w,h,scheme]=process.argv.slice(2);
const b = await chromium.launch(); const p = await (await b.newContext({viewport:{width:+w,height:+h},colorScheme:scheme})).newPage();
await p.goto(base+page,{waitUntil:"networkidle"}); await p.waitForTimeout(1500);
await p.screenshot({path:out}); await b.close();
