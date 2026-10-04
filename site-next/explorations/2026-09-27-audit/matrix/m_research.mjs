import { chromium } from 'playwright';
const b = await chromium.launch();
const p = await (await b.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
await p.goto('http://127.0.0.1:8841/evidence.html', { waitUntil: 'load' });
console.log(await p.evaluate(() => { const q = (s) => document.querySelector(s); const h = (s) => Math.round(q(s).getBoundingClientRect().height);
 return { section: h('#cite'), head: h('#research'), pubs: h('.pubs'), eng: h('#cite-engine'), cards: [...document.querySelectorAll('.pub')].map((e) => Math.round(e.getBoundingClientRect().height)) }; }));
await b.close();
