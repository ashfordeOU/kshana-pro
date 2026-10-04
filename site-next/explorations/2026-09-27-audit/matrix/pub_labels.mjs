import { chromium } from 'playwright';
const b = await chromium.launch();
for (const w of [390, 768, 1440, 1920]) {
  const p = await (await b.newContext({ viewport: { width: w, height: 900 }, reducedMotion: 'reduce' })).newPage();
  await p.goto('http://127.0.0.1:8841/evidence.html', { waitUntil: 'load' });
  await p.evaluate(() => document.querySelectorAll('.pub details').forEach((d) => d.setAttribute('open', '')));
  await p.waitForTimeout(700);
  const r = await p.evaluate(() => {
    const bad = [];
    for (const svg of document.querySelectorAll('svg.pub-svg')) {
      const sr = svg.getBoundingClientRect();
      const ts = [...svg.querySelectorAll('text')].filter((t) => t.textContent.trim()).map((t) => [t.textContent, t.getBoundingClientRect()]).filter(([, r]) => r.width > 0);
      for (let i = 0; i < ts.length; i++) { const a = ts[i][1];
        if (a.left < sr.left - 1 || a.right > sr.right + 1 || a.top < sr.top - 1 || a.bottom > sr.bottom + 1) bad.push('outside ' + ts[i][0]);
        for (let j = i + 1; j < ts.length; j++) { const c = ts[j][1]; if (Math.min(a.right, c.right) - Math.max(a.left, c.left) > 1 && Math.min(a.bottom, c.bottom) - Math.max(a.top, c.top) > 1) bad.push(`"${ts[i][0]}" x "${ts[j][0]}"`); } }
      const fs = Math.min(...ts.map(([, r]) => r.height)); if (fs < 8.5) bad.push(`smallest label ${fs.toFixed(1)} px in ${svg.getAttribute('aria-label').slice(0, 30)}`);
    }
    return { n: document.querySelectorAll('svg.pub-svg').length, hs: document.documentElement.scrollWidth > innerWidth, bad };
  });
  console.log(w, JSON.stringify(r));
  if (w === 1440 || w === 390) await p.locator('#pub-2607-05415').screenshot({ path: `../../../scratch/research/shots/card-open-${w}.png` });
  await p.close();
}
await b.close();
