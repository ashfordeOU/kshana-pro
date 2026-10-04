import { chromium } from 'playwright';
const b = await chromium.launch();
for (const theme of ['light','dark']) {
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce', colorScheme: theme });
const p = await ctx.newPage();
await p.goto('http://127.0.0.1:8841/evidence.html', { waitUntil: 'load' });
await p.evaluate(() => { document.querySelectorAll('.rv').forEach((e) => e.classList.add('in')); document.querySelectorAll('#pub-2607-02566 details.pub-figd, #pub-2606-24210 details.pub-figd, #pub-2606-22054 details.pub-figd').forEach((d) => d.setAttribute('open','')); });
await p.waitForTimeout(600);
await p.locator('.pubs').screenshot({ path: `../../../scratch/research/shots/pubs-figures-open-1440-${theme}.png` });
await ctx.close(); }
await b.close();
