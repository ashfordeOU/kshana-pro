#!/usr/bin/env node
// Link crawler for the redesigned Kshana site. Loads EVERY published HTML page in Chromium
// (JavaScript run, every tab panel activated, every <details> opened), collects every <a href>
// with its visible label, then checks each link:
//   - it resolves: internal targets are in PUBLISH.txt and their #fragment is an id on the
//     rendered target page; external targets answer (HEAD, then GET, with a browser user agent;
//     crates.io, npm and PyPI through their APIs; DOIs through the doi.org handle API; GitHub
//     blob/tree links through `gh api` when the web answers 429);
//   - it is the right target for its label (rules in labelRules below; each flag is reviewed).
//
//   node linkcrawl.mjs [--site=2026-09-27-combo | --root=SNAPSHOT_DIR] [--out=DIR] [--no-external] [--self-test]
// Output: DIR/links.json (every link, where it appears, its check) and DIR/links.md (problems).
// Exit 1 when any link is broken or any label rule fails.
import { chromium } from 'playwright';
import fs from 'node:fs';
import path from 'node:path';
import net from 'node:net';
import { spawn, execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
// --root=DIR crawls a snapshot anywhere (a copy taken while no build runs); default explorations/<site>.
const ROOT_OPT = process.argv.slice(2).find((x) => x.startsWith('--root='));
const EXPL = ROOT_OPT ? path.dirname(path.resolve(ROOT_OPT.slice(7))) : path.resolve(HERE, '..', '..');
const argv = process.argv.slice(2);
const opt = (k, d) => { const a = argv.find((x) => x.startsWith(`--${k}=`)); return a ? a.slice(k.length + 3) : d; };
const flag = (k) => argv.includes(`--${k}`);
const SITE = ROOT_OPT ? path.basename(path.resolve(ROOT_OPT.slice(7))) : opt('site', '2026-09-27-combo');
const ROOT = path.join(EXPL, SITE);
const OUT = path.resolve(opt('out', path.join(HERE, 'out', 'links-' + new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19))));
fs.mkdirSync(OUT, { recursive: true });
const UA = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';
const PUB = fs.readFileSync(path.join(ROOT, 'PUBLISH.txt'), 'utf8').split('\n').map((s) => s.trim()).filter(Boolean);
const PUBSET = new Set(PUB);
const STUDIO_VIEWS = ['', '?scenario=jamming-demo&tab=signal', '?scenario=telecom-prtc-holdover-24h&tab=masks', '?scenario=ephemeris&tab=ground', '?scenario=orbit-sgp4-gps&tab=orbit'];

// ---------------------------------------------------------------- label rules
// Each rule: (label, href, resolvedInternalPath|null, host|null) -> problem string or null.
const docsLike = (internal, host, href) => (internal && (internal.startsWith('docs/') || internal.startsWith('developers.html'))) || host === 'docs.rs' || /github\.com\/.*\.(md|rs)(#|$)/i.test(href);
const REGISTRY_HOSTS = new Set(['crates.io', 'pypi.org', 'www.npmjs.com', 'plugins.jetbrains.com', 'registry.modelcontextprotocol.io']);
export function labelRules(label, href, internal, host, rawHref = href) {
  const L = label.trim(), out = [];
  const inPage = /^#/.test(rawHref); // a link to a section of the same page
  if (!L) out.push('link has no visible label or accessible name');
  if (host && /(^|\.)kshana\.dev$/.test(host)) out.push('points at kshana.dev, the old live site (the Studio and pages live in this site)');
  if (/^(link|here|click here|more)$/i.test(L)) out.push(`generic label "${L}" does not say where it goes`);
  if (/^registry$/i.test(L) && !(host && (REGISTRY_HOSTS.has(host) || (host === 'github.com' && /pkgs\/container/.test(href))))) out.push('label "Registry" but target is not a package registry');
  if (/^docs?$/i.test(L) && !docsLike(internal, host, href)) out.push('label "Docs" but target is not documentation');
  if (L.length > 70 || inPage) return out; // long labels are sentences (a card's description); in-page links name a section
  if (/crates\.io/i.test(L) && host !== 'crates.io') out.push('label names crates.io but target is not crates.io');
  if (/^pypi\b/i.test(L) && host !== 'pypi.org') out.push('label names PyPI but target is not pypi.org');
  if (/^npm:/i.test(L) && host !== 'www.npmjs.com') out.push('label names npm but target is not npmjs.com');
  if (/marketplace/i.test(L) && host !== 'plugins.jetbrains.com') out.push('label names a marketplace but target is not the JetBrains Marketplace');
  if (/container registry|^ghcr\.io$/i.test(L) && !(host === 'github.com' && /pkgs\/container/.test(href))) out.push('label names the container registry but target is not a GitHub package');
  if (/docs\.rs/i.test(L) && host !== 'docs.rs') out.push('label names docs.rs but target is not docs.rs');
  const studioDoc = internal === 'docs/studio.html', home = internal === 'index.html';
  if (/\b(open|launch|run)\b.*\bstudio\b|^kshana studio$|^studio$/i.test(L) && !(internal && internal.startsWith('playground/')) && !(studioDoc && !/\b(open|launch|run)\b/i.test(L)) && !(home && /^kshana\s*studio$/i.test(L))) out.push('label opens the Studio but target is not the Studio');
  if (/\b(guide|docs|documentation|reference|usage)\b/i.test(L) && !docsLike(internal, host, href) && !(internal && internal.startsWith('playground/') && !/guide/i.test(L))) {
    if (!(host && /github\.com|zenodo|doi\.org|arxiv|ecss|iso\.org|itu\.int|rtca|navipedia|iers/.test(host))) out.push('label promises documentation but target is not a docs page');
  }
  if (/github/i.test(L) && !/not yet/i.test(L) && host && host !== 'github.com' && !host.endsWith('githubusercontent.com')) out.push('label names GitHub but target is not github.com');
  const m = L.match(/^(?:https?:\/\/)?((?:[a-z0-9-]+\.)+(?:com|org|dev|io|rs|net|eu|int|edu|gov|ai))(\/\S*)?$/i);
  // kshana.dev is this site's own domain: a "kshana.dev" label may point inside the site.
  if (m && !/^ghcr\.io$/i.test(m[1]) && !(internal && /(^|\.)kshana\.dev$/i.test(m[1]))) {
    if (!host && !internal) out.push(`label shows ${m[1]} but target is not a web page`);
    else if (host && m[1].toLowerCase().replace(/^www\./, '') !== host.replace(/^www\./, '')) out.push(`label shows ${m[1]} but target host is ${host}`);
  }
  return out;
}
if (flag('self-test')) {
  const cases = [
    ['Registry', 'https://kshana.dev', null, 'kshana.dev', 2], ['Docs', '../playground/index.html', 'playground/index.html', null, 1],
    ['crates.io: kshana', 'https://pypi.org/x', null, 'pypi.org', 1], ['Studio guide', 'playground/index.html', 'playground/index.html', null, 1],
    ['Open Kshana Studio', 'docs/studio.html', 'docs/studio.html', null, 1], ['kshana.dev', 'https://example.com', null, 'example.com', 1],
    ['', 'docs/x.html', 'docs/x.html', null, 1],
    ['crates.io: kshana', 'https://crates.io/crates/kshana', null, 'crates.io', 0], ['Studio guide', 'docs/studio.html', 'docs/studio.html', null, 0],
    ['Open Kshana Studio', 'playground/index.html', 'playground/index.html', null, 0], ['Python API guide', 'docs/python-api.html', 'docs/python-api.html', null, 0],
  ];
  let bad = 0;
  for (const [l, h, i, host, n] of cases) { const got = labelRules(l, h, i, host).length; if ((got > 0) !== (n > 0)) { bad++; console.log('SELF-TEST FAIL', l, h, got, labelRules(l, h, i, host)); } }
  console.log(bad ? `self-test: ${bad} failed` : `self-test: ${cases.length} cases OK`); process.exit(bad ? 1 : 0);
}

// ---------------------------------------------------------------- crawl
async function freePort() { return new Promise((res) => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); }); }
const PORT = await freePort();
const BASE = `http://127.0.0.1:${PORT}/${SITE}/`;
const server = spawn('python3', ['-m', 'http.server', String(PORT), '--bind', '127.0.0.1', '--directory', EXPL], { stdio: 'ignore' });
await new Promise((r) => setTimeout(r, 800));

const pages = PUB.filter((p) => p.endsWith('.html') && !p.startsWith('playground/recorded'));
const views = [];
for (const p of pages) {
  if (p === 'playground/index.html') for (const q of STUDIO_VIEWS) views.push({ page: p, url: BASE + p + q });
  else views.push({ page: p, url: BASE + p });
}
const browser = await chromium.launch();
const links = new Map(); // key href-resolved + label -> record
const ids = new Map(); // page -> Set(ids)
const errors = [];
async function crawl(v, scheme) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: scheme });
  const page = await ctx.newPage();
  try {
    await page.goto(v.url, { waitUntil: 'load', timeout: 90000 });
    await page.waitForTimeout(v.page.startsWith('playground/') ? 2500 : 600);
    // open every <details>, activate every tab (JS-generated panels render on activation)
    await page.evaluate(() => document.querySelectorAll('details').forEach((d) => d.setAttribute('open', '')));
    const ntabs = await page.locator('[role="tab"]').count();
    for (let i = 0; i < ntabs; i++) {
      const t = page.locator('[role="tab"]').nth(i);
      try { if (await t.isVisible()) { await t.click({ timeout: 2000 }); await page.waitForTimeout(v.page.startsWith('playground/') ? 500 : 80); } } catch { /* hidden or covered: its panel is still in the DOM */ }
      await collect(page, v);
    }
    await collect(page, v);
  } catch (e) { errors.push({ view: v.url, error: String(e).slice(0, 300) }); }
  await ctx.close();
}
async function collect(page, v) {
  const got = await page.evaluate(() => {
    const lab = (a) => (a.innerText || '').replace(/\s+/g, ' ').trim() || a.getAttribute('aria-label') || a.getAttribute('title') || [...a.querySelectorAll('img[alt]')].map((i) => i.alt).join(' ') || '';
    return { links: [...document.querySelectorAll('a[href]')].map((a) => ({ href: a.getAttribute('href'), abs: a.href, label: lab(a).slice(0, 160) })), ids: [...document.querySelectorAll('[id]')].map((e) => e.id).concat([...document.querySelectorAll('a[name]')].map((e) => e.getAttribute('name'))) };
  });
  const set = ids.get(v.page) || new Set(); got.ids.forEach((i) => set.add(i)); ids.set(v.page, set);
  for (const l of got.links) {
    if (!l.href || l.href.startsWith('javascript:')) continue;
    const key = l.abs + '\u0000' + l.label;
    const rec = links.get(key) || { href: l.href, abs: l.abs, label: l.label, pages: new Set() };
    rec.pages.add(v.page); links.set(key, rec);
  }
}
const Q = [...views];
async function worker() { while (Q.length) { const v = Q.shift(); await crawl(v, 'light'); } }
await Promise.all([worker(), worker(), worker()]);
// dark theme: a second pass over the pages whose markup could differ by theme (docs pictures, home)
for (const v of views.filter((v) => v.page === 'index.html' || v.page === 'docs/overview.html')) await crawl(v, 'dark');

// ---------------------------------------------------------------- check
const internalOf = (abs) => { if (!abs.startsWith(BASE)) return null; const u = new URL(abs); return { path: decodeURIComponent(u.pathname.slice(new URL(BASE).pathname.length)) || 'index.html', frag: decodeURIComponent(u.hash.slice(1)) }; };
const extCache = new Map();
async function fetchOk(url, method) {
  const ctl = new AbortController(); const t = setTimeout(() => ctl.abort(), 25000);
  try { const r = await fetch(url, { method, redirect: 'follow', headers: { 'user-agent': UA, accept: 'text/html,application/json;q=0.9,*/*;q=0.8' }, signal: ctl.signal }); return { status: r.status, final: r.url }; }
  catch (e) { return { status: 0, error: String(e.cause?.code || e.name || e).slice(0, 80) }; } finally { clearTimeout(t); }
}
async function fetchJson(url) { try { const r = await fetch(url, { headers: { 'user-agent': UA } }); return r.ok ? await r.json() : null; } catch { return null; } }
async function checkExternal(abs) {
  const u = new URL(abs.split('#')[0]);
  const host = u.hostname;
  let m;
  if (host === 'crates.io' && (m = u.pathname.match(/^\/crates\/([\w-]+)/))) {
    const r = await fetchOk(`https://crates.io/api/v1/crates/${m[1]}`, 'GET'); return { ok: r.status === 200, how: 'crates.io API', status: r.status };
  }
  if (host === 'www.npmjs.com' && (m = u.pathname.match(/^\/package\/((?:@[\w-]+\/)?[\w.-]+)/))) {
    const r = await fetchOk(`https://registry.npmjs.org/${m[1]}`, 'GET'); return { ok: r.status === 200, how: 'npm registry API', status: r.status };
  }
  if (host === 'pypi.org' && (m = u.pathname.match(/^\/project\/([\w.-]+)/))) {
    const r = await fetchOk(`https://pypi.org/pypi/${m[1]}/json`, 'GET'); return { ok: r.status === 200, how: 'PyPI JSON API', status: r.status };
  }
  if ((host === 'doi.org' || host === 'dx.doi.org') && u.pathname.length > 1) {
    const r = await fetchOk(`https://doi.org/api/handles/${u.pathname.slice(1)}`, 'GET'); return { ok: r.status === 200, how: 'doi.org handle API', status: r.status };
  }
  let r = await fetchOk(abs, 'HEAD');
  if (r.status === 0 || r.status >= 400) r = await fetchOk(abs, 'GET');
  if ((r.status === 429 || r.status === 403) && host === 'github.com' && (m = u.pathname.match(/^\/([\w.-]+)\/([\w.-]+)\/(?:blob|tree)\/([\w.-]+)\/(.+)$/))) {
    try { execFileSync('gh', ['api', `repos/${m[1]}/${m[2]}/contents/${m[4]}?ref=${m[3]}`, '--silent'], { stdio: 'ignore', timeout: 20000 }); return { ok: true, how: 'gh api contents', status: 200 }; } catch { return { ok: false, how: 'gh api contents', status: 404 }; }
  }
  if ((r.status === 429 || r.status === 403) && host === 'github.com' && (m = u.pathname.match(/^\/([\w.-]+)\/([\w.-]+)\/?$/))) {
    try { execFileSync('gh', ['api', `repos/${m[1]}/${m[2]}`, '--silent'], { stdio: 'ignore', timeout: 20000 }); return { ok: true, how: 'gh api repo', status: 200 }; } catch { return { ok: false, how: 'gh api repo', status: 404 }; }
  }
  if (r.status === 403 || r.status === 0 || r.status === 429) {
    // Sites that refuse scripted requests: load the page in the real browser and read the main response.
    const ctx = await browser.newContext({ userAgent: UA }); const pg = await ctx.newPage();
    let res;
    try { const resp = await pg.goto(abs, { waitUntil: 'domcontentloaded', timeout: 30000 }); const st = resp ? resp.status() : 0; const title = (await pg.title()).slice(0, 80); res = { ok: st >= 200 && st < 400 && !/just a moment|access denied|attention required/i.test(title), how: 'browser load', status: st, title }; }
    catch (e) { res = { ok: false, how: 'browser load', status: 0, error: String(e).slice(0, 80) }; }
    await ctx.close();
    if (res.ok || !/just a moment|attention required/i.test(res.title || '')) return res;
    // A bot challenge (the page is served to people, not to scripts): accept it only when the web
    // archive holds a 200 capture of that exact URL; the report lists these separately.
    const w = await fetchJson(`https://archive.org/wayback/available?url=${encodeURIComponent(abs.replace(/^https?:\/\//, ''))}`);
    const snap = w && w.archived_snapshots && w.archived_snapshots.closest;
    if (snap && snap.status === '200') return { ok: true, how: 'bot challenge; web archive 200 capture ' + snap.timestamp.slice(0, 8), status: res.status, challenged: true };
    return res;
  }
  return { ok: r.status >= 200 && r.status < 400, how: 'HTTP', status: r.status, error: r.error, final: r.final };
}
const recs = [...links.values()].map((r) => ({ ...r, pages: [...r.pages].sort() }));
const extUrls = [...new Set(recs.filter((r) => /^https?:/.test(r.abs) && !r.abs.startsWith(BASE)).map((r) => r.abs.split('#')[0]))];
if (!flag('no-external')) {
  const EQ = [...extUrls];
  await Promise.all([0, 1, 2].map(async () => { while (EQ.length) { const x = EQ.shift(); extCache.set(x, await checkExternal(x)); } }));
}
for (const r of recs) {
  r.problems = [];
  let host = null, internal = null;
  if (r.abs.startsWith('mailto:')) { if (!/^mailto:[^@\s]+@[^@\s]+\.[a-z]+/i.test(r.abs)) r.problems.push('malformed mailto'); r.check = 'mailto'; }
  else if (r.abs.startsWith(BASE)) {
    const t = internalOf(r.abs); internal = t.path;
    const file = t.path.endsWith('/') ? t.path + 'index.html' : t.path;
    if (!PUBSET.has(file)) r.problems.push(`internal target ${file} is not published`);
    else if (t.frag && file.endsWith('.html') && ids.has(file) && !ids.get(file).has(t.frag)) r.problems.push(`anchor #${t.frag} not found on ${file}`);
    else if (t.frag && file.endsWith('.html') && !ids.has(file)) r.problems.push(`anchor #${t.frag} on ${file}: page not crawled`);
    r.check = 'internal';
  } else if (/^https?:/.test(r.abs)) {
    host = new URL(r.abs).hostname;
    const c = extCache.get(r.abs.split('#')[0]);
    r.check = c || 'skipped';
    if (c && !c.ok) r.problems.push(`external ${c.how} answered ${c.status}${c.error ? ' ' + c.error : ''}`);
  } else { r.check = 'other scheme'; }
  r.labelProblems = labelRules(r.label, r.abs, internal, host, r.href);
}
server.kill(); await browser.close();
const broken = recs.filter((r) => r.problems.length), wrong = recs.filter((r) => r.labelProblems.length);
fs.writeFileSync(path.join(OUT, 'links.json'), JSON.stringify({ site: SITE, idCounts: Object.fromEntries([...ids].map(([k, v]) => [k, v.size])), views: views.length, links: recs.length, external: extUrls.length, errors, broken: broken.length, wrong: wrong.length, recs }, null, 1));
const md = [`# Link crawl: ${SITE}`, '', `${views.length} views loaded (${pages.length} pages, Studio in ${STUDIO_VIEWS.length} views), ${recs.length} distinct links (label + target), ${extUrls.length} distinct external URLs.`, `Broken: ${broken.length}. Label/target mismatch: ${wrong.length}. Crawl errors: ${errors.length}.`, ''];
md.push('## Broken', ''); for (const r of broken) md.push(`- \`${r.href}\` "${r.label}" on ${r.pages.slice(0, 4).join(', ')}${r.pages.length > 4 ? ` +${r.pages.length - 4}` : ''}: ${r.problems.join('; ')}`);
md.push('', '## Label and target', ''); for (const r of wrong) md.push(`- \`${r.href}\` "${r.label}" on ${r.pages.slice(0, 4).join(', ')}${r.pages.length > 4 ? ` +${r.pages.length - 4}` : ''}: ${r.labelProblems.join('; ')}`);
md.push('', '## Verified another way (bot challenge, web-archive capture)', ''); for (const r of recs.filter((r) => r.check && r.check.challenged)) md.push(`- \`${r.href}\` "${r.label}": ${r.check.how}`);
md.push('', '## Crawl errors', ''); for (const e of errors) md.push(`- ${e.view}: ${e.error}`);
fs.writeFileSync(path.join(OUT, 'links.md'), md.join('\n') + '\n');
console.log(md.slice(0, 5).join('\n')); console.log('report:', path.join(OUT, 'links.md'));
process.exit(broken.length || wrong.length || errors.length ? 1 : 0);
