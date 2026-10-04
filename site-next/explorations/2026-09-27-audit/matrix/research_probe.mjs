// Evidence #research / #cite and the Home proof strip: overflow, clipping, overlap, console errors,
// every link in the section, and element screenshots at 390/768/1440/1920 in light and dark.
// Usage: node research_probe.mjs <outdir>
import { chromium } from 'playwright';
import fs from 'node:fs';
const OUT = process.argv[2];
fs.mkdirSync(OUT, { recursive: true });
const BASE = 'http://127.0.0.1:8841/';
const b = await chromium.launch();
const problems = [];
const report = [];
for (const theme of ['light', 'dark']) {
  for (const [w, h] of [[390, 844], [768, 1024], [1440, 900], [1920, 1080]]) {
    const ctx = await b.newContext({ viewport: { width: w, height: h }, colorScheme: theme, reducedMotion: 'reduce' });
    await ctx.addInitScript((t) => { try { localStorage.setItem('kshana-theme', t); } catch (e) {} }, theme);
    for (const [pg, sel] of [['evidence.html', '#cite'], ['index.html', '#proof']]) {
      const p = await ctx.newPage();
      const errs = [];
      p.on('console', (m) => { if (m.type() === 'error') errs.push(m.text()); });
      p.on('pageerror', (e) => errs.push(String(e)));
      await p.goto(BASE + pg, { waitUntil: 'load' });
      await p.waitForTimeout(1200);
      const el = p.locator(sel).first();
      await el.scrollIntoViewIfNeeded();
      await p.waitForTimeout(900);
      // reveal-on-scroll elements: make sure they are shown before measuring
      await p.evaluate((s) => document.querySelectorAll(s + ' .rv').forEach((e) => e.classList.add('in')), sel);
      const r = await p.evaluate((s) => {
        const root = document.querySelector(s);
        const out = { hscroll: document.documentElement.scrollWidth > window.innerWidth + 1, clipped: [], overlap: [], height: Math.round(root.getBoundingClientRect().height) };
        for (const e of root.querySelectorAll('*')) {
          const cs = getComputedStyle(e);
          if (e.closest('pre') || e.tagName === 'PRE' || cs.display === 'none' || e.closest('details:not([open])') && !e.closest('summary')) continue;
          if (e.scrollWidth > e.clientWidth + 1 && e.clientWidth > 0 && ['hidden', 'clip'].includes(cs.overflowX)) out.clipped.push(e.className || e.tagName);
          const rb = e.getBoundingClientRect(), rr = root.getBoundingClientRect();
          if (rb.width && (rb.right > rr.right + 1 || rb.left < rr.left - 1) && !e.closest('svg')) out.clipped.push('outside:' + (e.className?.baseVal ?? e.className) + ':' + e.tagName);
        }
        // sibling overlap inside each card / stat
        for (const card of root.querySelectorAll('.pub, .pub-a, .pub-fig, .cite-eng, .stats-in')) {
          const kids = [...card.children].filter((k) => getComputedStyle(k).display !== 'none').map((k) => [k, k.getBoundingClientRect()]).filter(([, r]) => r.width && r.height);
          for (let i = 0; i < kids.length; i++) for (let j = i + 1; j < kids.length; j++) {
            const [a, ra] = kids[i], [c, rc] = kids[j];
            const ox = Math.min(ra.right, rc.right) - Math.max(ra.left, rc.left), oy = Math.min(ra.bottom, rc.bottom) - Math.max(ra.top, rc.top);
            if (ox > 1 && oy > 1) out.overlap.push(`${a.className || a.tagName} x ${c.className || c.tagName}`);
          }
        }
        // chart labels must not collide, and must stay inside their chart
        for (const svg of root.querySelectorAll('svg.pub-svg')) {
          const sr = svg.getBoundingClientRect();
          const ts = [...svg.querySelectorAll('text')].filter((t) => t.textContent.trim()).map((t) => [t.textContent, t.getBoundingClientRect()]).filter(([, r]) => r.width > 0);
          for (let i = 0; i < ts.length; i++) {
            const a = ts[i][1];
            if (a.left < sr.left - 1 || a.right > sr.right + 1 || a.top < sr.top - 1 || a.bottom > sr.bottom + 1) out.clipped.push('chart label outside: ' + ts[i][0]);
            for (let j = i + 1; j < ts.length; j++) { const b = ts[j][1]; const ox = Math.min(a.right, b.right) - Math.max(a.left, b.left), oy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top); if (ox > 1 && oy > 1) out.overlap.push(`label "${ts[i][0]}" x "${ts[j][0]}"`); }
          }
        }
        out.links = [...root.querySelectorAll('a[href]')].map((a) => a.href);
        return out;
      }, sel);
      // open every BibTeX block once for the shot at 1440 so its layout is seen too
      if (pg === 'evidence.html' && (w === 1440 || w === 390)) { await p.evaluate(() => document.querySelectorAll('#pub-2607-02566 details, .pub details.pub-figd').forEach((d) => d.setAttribute('open', ''))); await p.waitForTimeout(600); }
      const shot = `${OUT}/${pg.replace('.html', '')}-${sel.slice(1)}-${w}-${theme}.png`;
      await el.screenshot({ path: shot, timeout: 60000 }).catch((e) => problems.push(`${shot}: screenshot ${e.message.split('\n')[0]}`));
      const tag = `${pg}${sel} ${w} ${theme}`;
      if (r.hscroll) problems.push(`${tag}: horizontal scroll`);
      if (r.clipped.length) problems.push(`${tag}: clipped ${[...new Set(r.clipped)].slice(0, 6).join(', ')}`);
      if (r.overlap.length) problems.push(`${tag}: overlap ${[...new Set(r.overlap)].slice(0, 6).join(', ')}`);
      if (errs.length) problems.push(`${tag}: console ${errs.slice(0, 3).join(' | ')}`);
      report.push({ tag, height: r.height, links: r.links.length });
      if (w === 1440 && theme === 'light') fs.writeFileSync(`${OUT}/${pg.replace('.html', '')}-links.json`, JSON.stringify([...new Set(r.links)], null, 1));
      await p.close();
    }
    await ctx.close();
  }
}
// every internal link in the section opens (200), Studio links included
const links = JSON.parse(fs.readFileSync(`${OUT}/evidence-links.json`)).concat(JSON.parse(fs.readFileSync(`${OUT}/index-links.json`)));
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
for (const u of links.filter((u) => u.startsWith(BASE))) {
  const p = await ctx.newPage();
  const errs = [];
  p.on('pageerror', (e) => errs.push(String(e)));
  const res = await p.goto(u, { waitUntil: 'load' });
  await p.waitForTimeout(u.includes('playground') ? 4000 : 500);
  const frag = new URL(u).hash.slice(1);
  const ok = res && res.status() === 200 && (!frag || (await p.evaluate((f) => !!document.getElementById(f), frag)));
  const scen = new URL(u).searchParams.get('scenario');
  const title = scen ? await p.evaluate(() => document.body.innerText.slice(0, 4000)) : '';
  if (!ok) problems.push(`link ${u}: status ${res && res.status()} or missing #${frag}`);
  if (scen && !(await p.title()).includes('Kshana Studio')) problems.push(`link ${u}: the Studio does not show ${scen}`);
  if (errs.length) problems.push(`link ${u}: ${errs[0]}`);
  report.push({ link: u, ok, scen: scen || undefined });
  await p.close();
}
await b.close();
fs.writeFileSync(`${OUT}/report.json`, JSON.stringify({ problems, report }, null, 1));
console.log(report.filter((r) => r.tag).map((r) => `${r.tag}: ${r.height}px, ${r.links} links`).join('\n'));
console.log(`internal links checked: ${report.filter((r) => r.link).length}`);
console.log(problems.length ? 'PROBLEMS:\n' + problems.join('\n') : 'NO PROBLEMS');
