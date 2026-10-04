// R4 full-matrix owner: every page at 1440x900 and 390x844, dark and light, first screen + full page (capped).
// usage: node r4_full_shots.mjs <baseURL of 2026-09-27-combo/> <outdir>
import { chromium } from 'playwright';
import fs from 'node:fs';
const [base, OUT] = process.argv.slice(2);
const pages = { home: 'index.html', missions: 'missions.html', capabilities: 'capabilities.html', evidence: 'evidence.html', developers: 'developers.html', editions: 'editions.html', 'docs-index': 'docs/index.html', 'docs-scenario-reference': 'docs/scenario-reference.html', studio: 'playground/index.html' };
const b = await chromium.launch();
for (const [name, rel] of Object.entries(pages)) {
  fs.mkdirSync(`${OUT}/${name}`, { recursive: true });
  for (const [w, h] of [[1440, 900], [390, 844]]) for (const t of ['dark', 'light']) {
    const c = await b.newContext({ viewport: { width: w, height: h }, colorScheme: t });
    const pg = await c.newPage();
    await pg.goto(base + rel, { waitUntil: 'load' });
    await pg.evaluate((t) => document.documentElement.setAttribute('data-theme', t), t);
    await pg.evaluate(async () => { for (let y = 0; y < document.documentElement.scrollHeight; y += 700) { scrollTo(0, y); await new Promise((r) => setTimeout(r, 60)); } scrollTo(0, 0); });
    await pg.waitForTimeout(1500);
    await pg.screenshot({ path: `${OUT}/${name}/${w}x${h}-${t}.jpg`, quality: 80 });
    const H = Math.min(16000, await pg.evaluate(() => document.documentElement.scrollHeight));
    await pg.screenshot({ path: `${OUT}/${name}/${w}x${h}-${t}-full.jpg`, quality: 70, clip: { x: 0, y: 0, width: w, height: H }, fullPage: true });
    await c.close();
  }
  console.log('shot', name);
}
await b.close();
