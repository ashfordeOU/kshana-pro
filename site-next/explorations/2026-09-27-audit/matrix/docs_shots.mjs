// Screenshots of docs pages at 1440 and 390 px in both themes (system light and dark), from a site
// snapshot. Each shot is the page from the top, at most 6000 px tall; a second shot per page frames
// a named element (a repaired table or the README figures) when one is given.
//   node docs_shots.mjs --root=SNAPSHOT_DIR --out=DIR page[:selector] ...
import { chromium } from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import net from 'node:net';
import { spawn } from 'node:child_process';
const argv = process.argv.slice(2);
const opt = (k) => (argv.find((a) => a.startsWith(`--${k}=`)) || '').slice(k.length + 3);
const root = path.resolve(opt('root')), out = path.resolve(opt('out'));
const pages = argv.filter((a) => !a.startsWith('--'));
fs.mkdirSync(out, { recursive: true });
const port = await new Promise((r) => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)); }); });
const srv = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1', '--directory', path.dirname(root)], { stdio: 'ignore' });
await new Promise((r) => setTimeout(r, 800));
const b = await chromium.launch();
const made = [];
for (const spec of pages) {
  const [pg, sel] = spec.split(':');
  for (const scheme of ['light', 'dark']) for (const [w, h] of [[1440, 900], [390, 844]]) {
    const ctx = await b.newContext({ viewport: { width: w, height: h }, colorScheme: scheme, deviceScaleFactor: 1 });
    const p = await ctx.newPage();
    await p.goto(`http://127.0.0.1:${port}/${path.basename(root)}/docs/${pg}.html`, { waitUntil: 'load', timeout: 120000 });
    await p.waitForTimeout(700);
    const H = await p.evaluate(() => document.documentElement.scrollHeight);
    const f = path.join(out, `${pg}-${w}-${scheme}.jpg`);
    await p.screenshot({ path: f, type: 'jpeg', quality: 72, fullPage: true, clip: { x: 0, y: 0, width: w, height: Math.min(H, 6000) } });
    made.push(f);
    if (sel) {
      const el = p.locator(sel).first();
      if (await el.count()) { await el.scrollIntoViewIfNeeded(); const f2 = path.join(out, `${pg}-${w}-${scheme}-detail.jpg`); await el.screenshot({ path: f2, type: 'jpeg', quality: 72 }); made.push(f2); }
    }
    // theme check: exactly one of each light/dark README figure pair is visible
    const vis = await p.evaluate(() => [...document.querySelectorAll('.prose img.theme-light, .prose img.theme-dark')].map((i) => i.className + ':' + (getComputedStyle(i).display !== 'none')));
    if (vis.length) console.log(pg, w, scheme, 'visible:', vis.filter((v) => v.endsWith('true')).map((v) => v.split(':')[0]).join(',').slice(0, 80), `(${vis.filter((v) => v.endsWith('true')).length}/${vis.length})`);
    await ctx.close();
  }
}
await b.close(); srv.kill();
console.log(made.length + ' screenshots'); made.forEach((f) => console.log(f));
