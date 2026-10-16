// Follows every "Open in Kshana Studio" link of the built site in a headless browser.
//   1. renders every page that carries a Studio link (the six top pages and the docs pages),
//      opens every tab panel, and collects each link with the text of the visual it belongs to;
//   2. opens every distinct Studio address: the scenario and the view must be the ones the link
//      names, the run must finish, and the console and the network must stay clean;
//   3. compares the figures shown beside each link on the page with the result the Studio
//      computed for that run (its own JSON view), at the precision the page prints.
// Run: node studio_links_check.mjs [--base http://127.0.0.1:8841/] [--jobs 3] [--out reports/studio-links.json]
//      [--only <substring of a Studio address>] [--pages index.html,missions.html]
// (page.evaluate below is Playwright's in-page call.)
import { createRequire } from "node:module";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
const ROOT = new URL("../../", import.meta.url).pathname;
const SITE = ROOT + "explorations/2026-09-27-combo/";
const require = createRequire(ROOT + "explorations/2026-09-27-audit/matrix/");
const { chromium } = require("playwright");
const args = process.argv.slice(2);
const opt = (n, d) => { const i = args.indexOf(n); return i < 0 ? d : args[i + 1]; };
const BASE = opt("--base", "http://127.0.0.1:8841/");
const JOBS = parseInt(opt("--jobs", "3"), 10);
const OUT = opt("--out", ROOT + "scratch/assembly/reports/studio-links.json");
const ONLY = opt("--only", "");
const SHOTS = ROOT + "scratch/assembly/shots/studio-links/";
mkdirSync(SHOTS, { recursive: true });

const pub = readFileSync(SITE + "PUBLISH.txt", "utf8").split("\n").filter(Boolean);
let pages = pub.filter((p) => p.endsWith(".html") && !p.startsWith("playground/") && !p.startsWith("assets/") && /playground\/index\.html\?[^"]*scenario=/.test(readFileSync(SITE + p, "utf8")));
for (const top of ["index.html", "missions.html", "capabilities.html", "evidence.html", "developers.html", "editions.html"]) if (!pages.includes(top)) pages.push(top);
if (opt("--pages", "")) pages = opt("--pages", "").split(",");
console.log(`${pages.length} pages carry Studio links: ${pages.join(", ")}`);

const browser = await chromium.launch(process.env.KSHANA_CHROME ? { executablePath: process.env.KSHANA_CHROME } : {});

// ---- 1. collect
const found = [];
const pageProblems = {};
for (const pg of pages) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" });
  const page = await ctx.newPage();
  const errs = [];
  page.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
  page.on("pageerror", (e) => errs.push(String(e)));
  page.on("requestfailed", (r) => errs.push(`request failed: ${r.url()}`));
  page.on("response", (r) => { if (r.status() >= 400) errs.push(`${r.status()} ${r.url()}`); });
  await page.goto(BASE + pg, { waitUntil: "load", timeout: 180000 });
  await page.waitForTimeout(2500);
  const walk = async () => { const h = await page.evaluate(() => document.documentElement.scrollHeight); for (let y = 0; y < h; y += 700) { await page.evaluate((yy) => window.scrollTo(0, yy), y); await page.waitForTimeout(pg.startsWith("docs/") ? 30 : 220); } };
  await walk();
  const grab = () => page.evaluate(() => {
    const secOf = (el) => { const top = el.closest("section[id], main > *[id]"); const near = el.closest("[id]"); return (top && top.id ? top.id : "-") + (near && near.id && (!top || near.id !== top.id) ? " / " + near.id : ""); };
    const txt = (el) => (el ? (el.innerText || el.textContent || "").replace(/\s+/g, " ").trim() : "");
    return [...document.querySelectorAll("a")].filter((a) => /playground\/index\.html\?[^#]*scenario=/.test(a.getAttribute("href") || "")).map((a) => {
      // The visual a link belongs to: its figure or card; a provenance line's visual is the block around it.
      const prov = a.closest(".prov");
      const box = a.closest("figure, article, .sc-chart, .ic-card, .fc, .cz-col, .ob-card, .mp, .ev-tile, .cap-card, .cg-one") || (prov && prov.parentElement) || a.parentElement;
      // A provenance line kept apart from its visual (a hero's, a console's): the visual is the section it
      // stands in, or for a line outside any section the section just before it.
      let wide = "";
      if (prov) { const sec = a.closest("section") || (a.closest("main > *") && a.closest("main > *").previousElementSibling); wide = txt(sec).slice(0, 5000); }
      return { href: a.href, raw: a.getAttribute("href"), text: txt(a).slice(0, 70), where: secOf(a), open: /Open in|Open this run/.test(txt(a)) || a.classList.contains("prov-open") || a.classList.contains("sa-open"), visual: txt(box).slice(0, 1400), wide, prov: prov ? txt(prov).slice(0, 200) : "" };
    });
  });
  const seenLinks = new Map();
  const keep = (list) => { for (const l of list) { const k = `${l.raw}|${l.where}|${l.text}`; if (!seenLinks.has(k)) seenLinks.set(k, l); } };
  keep(await grab());
  if (!pg.startsWith("docs/")) {
    // Every picker that swaps which run a console shows (tabs, run lists, options): click each, and keep
    // the links of every state, not only the last one.
    const SEL = '[role="tab"], [role="option"], [role="radio"], button[aria-selected], [data-ow-run]';
    const nPick = await page.evaluate((sel) => document.querySelectorAll(sel).length, SEL);
    for (let i = 0; i < nPick; i++) {
      const ok = await page.evaluate(([sel, k]) => { const t = document.querySelectorAll(sel)[k]; if (t && t.getClientRects().length) { t.scrollIntoView({ block: "center" }); t.click(); return true; } return false; }, [SEL, i]);
      if (!ok) continue;
      await page.waitForTimeout(260);
      keep(await grab());
    }
    // A second pass: a picker inside a panel that the first pass opened late.
    const nPick2 = await page.evaluate((sel) => document.querySelectorAll(sel).length, SEL);
    for (let i = 0; i < nPick2; i++) {
      const ok = await page.evaluate(([sel, k]) => { const t = document.querySelectorAll(sel)[k]; if (t && t.getClientRects().length && t.getAttribute("aria-selected") !== "true") { t.scrollIntoView({ block: "center" }); t.click(); return true; } return false; }, [SEL, i]);
      if (!ok) continue;
      await page.waitForTimeout(260);
      keep(await grab());
    }
    await walk();
    await page.waitForTimeout(1200);
    keep(await grab());
  }
  const links = [...seenLinks.values()];
  for (const l of links) found.push({ page: pg, ...l });
  pageProblems[pg] = errs;
  console.log(`${pg}: ${links.length} Studio links (${links.filter((l) => l.open).length} "Open in" links); page problems: ${errs.length}`);
  for (const e of errs.slice(0, 5)) console.log(`   ERR ${e}`);
  await ctx.close();
}

// ---- 2. open every distinct address
const urls = [...new Set(found.map((l) => l.href))].filter((u) => !ONLY || u.includes(ONLY)).sort();
console.log(`${found.length} links, ${urls.length} distinct Studio addresses`);
const results = {};
// --reuse <report.json>: an address that report opened without a problem is not opened again
// (use it only for a report of the SAME build).
const REUSE = opt("--reuse", "");
if (REUSE) { const prev = JSON.parse(readFileSync(REUSE, "utf8")).full || {}; for (const u of urls) if (prev[u] && !prev[u].problems.length) results[u] = prev[u]; }
const todo = urls.filter((u) => !results[u]);
console.log(`${todo.length} addresses to open (${urls.length - todo.length} already opened clean by ${REUSE || "nothing"})`);
let next = 0, done = 0;
async function worker(n) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: n % 2 ? "dark" : "light" });
  while (next < todo.length) {
    const url = todo[next++];
    const q = new URL(url).searchParams;
    const want = { scenario: q.get("scenario"), tab: q.get("tab"), embed: q.get("embed") === "1" };
    const page = await ctx.newPage();
    const errs = [];
    page.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); });
    page.on("pageerror", (e) => errs.push(String(e)));
    page.on("requestfailed", (r) => { if (!/fonts\.g(oogleapis|static)\.com/.test(r.url())) errs.push(`request failed: ${r.url()} ${r.failure() ? r.failure().errorText : ""}`); });
    page.on("response", (r) => { if (r.status() >= 400) errs.push(`${r.status()} ${r.url()}`); });
    const res = { want, problems: [] };
    try {
      await page.goto(url, { waitUntil: "load", timeout: 180000 });
      await page.waitForFunction(() => { const s = document.getElementById("status"); return s && (s.classList.contains("ran") || s.classList.contains("err") || /could not|failed|error/i.test(s.textContent)); }, null, { timeout: 240000 });
      await page.waitForTimeout(600);
      const st = await page.evaluate(() => {
        const sel = document.querySelector('#tabs [role="tab"][aria-selected="true"]');
        const view = [...document.querySelectorAll("section.view")].find((v) => !v.hidden);
        const notice = document.getElementById("notice");
        return {
          file: (document.getElementById("sc-file") || {}).textContent || "", tab: sel ? sel.dataset.tab : null,
          tabs: [...document.querySelectorAll('#tabs [role="tab"]')].map((t) => t.dataset.tab),
          status: document.getElementById("status").textContent, engine: (document.getElementById("engine-label") || {}).textContent || "",
          summary: (document.getElementById("summary") || {}).textContent || "", notice: notice && !notice.hidden ? notice.textContent : "",
          json: (document.getElementById("json") || {}).textContent || "", viewText: view ? (view.innerText || "").replace(/\s+/g, " ").slice(0, 6000) : "",
          viewBox: view ? view.getBoundingClientRect().height : 0, hscroll: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
        };
      });
      Object.assign(res, st);
      if (!st.file.endsWith(`${want.scenario}.toml`)) res.problems.push(`opened ${st.file}, the link names ${want.scenario}`);
      if (want.tab && st.tab !== want.tab) res.problems.push(`opened the ${st.tab} view, the link names ${want.tab} (the run has: ${st.tabs.join(", ")})`);
      if (!/^(Ran locally|Recorded run shown)/.test(st.status)) res.problems.push(`did not run: ${st.status}`);
      // A run the browser build cannot make is shown as recorded with the native engine, and says so in a notice: expected.
      if (st.notice && !/^Recorded run shown/.test(st.status)) res.problems.push(`notice: ${st.notice.slice(0, 160)}`);
      if (st.notice) res.note = st.notice.slice(0, 200);
      if (st.viewBox < 40) res.problems.push("the view is empty");
      if (st.hscroll) res.problems.push("horizontal scroll at 1440");
      res.live = /^Ran locally/.test(st.status);
    } catch (e) { res.problems.push(`did not finish: ${String(e).split("\n")[0].slice(0, 200)}`); }
    for (const e of [...new Set(errs)]) res.problems.push(`console/network: ${e.slice(0, 200)}`);
    if (res.problems.length) { try { await page.screenshot({ path: `${SHOTS}${want.scenario}.${want.tab || "default"}.png` }); } catch { /* under load */ } }
    results[url] = res;
    done++;
    if (done % 10 === 0 || done === todo.length) console.log(`  opened ${done}/${todo.length}`);
    await page.close();
  }
  await ctx.close();
}
await Promise.all(Array.from({ length: JOBS }, (_, i) => worker(i)));
await browser.close();

// ---- 3. page figures against the Studio's result
const leavesOf = (text) => {
  const out = [];
  let j = null;
  try { j = JSON.parse(text); } catch { /* elided or not JSON */ }
  const walk = (v) => { if (typeof v === "number" && Number.isFinite(v)) out.push(v); else if (Array.isArray(v)) v.forEach(walk); else if (v && typeof v === "object") Object.values(v).forEach(walk); };
  if (j) walk(j); else for (const m of text.matchAll(/-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?/g)) out.push(Number(m[0]));
  return out;
};
const SCALES = [1, 100, 1e-3, 1e3, 1 / 60, 1 / 3600, 1 / 86400, 1e9, 1e6, 1e-6, 1e-9];
function figuresOf(text) {
  // Numbers a reader would call a figure: three or more significant digits, not a version, date, hash or seed.
  const t = text.replace(/v\d+\.\d+\.\d+/g, " ").replace(/\b\d{4}-\d{2}-\d{2}\b/g, " ").replace(/seed \d+/g, " ").replace(/build [0-9a-f]{7}/g, " ").replace(/T\+\d\d:\d\d:\d\d/g, " ").replace(/SHA-256 [0-9a-f…]+/g, " ");
  const out = [];
  for (const m of t.matchAll(/(?<![\w.#-])-?\d[\d,]*(?:\.\d+)?(?![\w])/g)) {
    const s = m[0].replace(/,/g, "");
    if (s.replace(/[-.]/g, "").replace(/^0+/, "").length < 3) continue;
    out.push({ shown: m[0], value: Number(s), decimals: (s.split(".")[1] || "").length });
  }
  return out;
}
const matches = (fig, leaves) => leaves.some((v) => SCALES.some((s) => Number((v * s).toFixed(fig.decimals)) === fig.value));
const rows = [];
for (const l of found) {
  const r = results[l.href];
  if (!r) continue;
  const row = { page: l.page, where: l.where, link: l.raw, text: l.text, open: l.open, result: r.problems.length ? "FAIL" : "pass", problems: r.problems, live: r.live, status: r.status, studioSummary: (r.summary || "").slice(0, 200) };
  if (l.open && r.json) {
    const leaves = leavesOf(r.json);
    const pool = leaves.concat(leavesOf(r.summary || ""), leavesOf(r.viewText || ""));
    let figs = figuresOf(l.visual.replace(l.prov, " "));
    if (!figs.length && l.wide) { figs = figuresOf(l.wide.replace(l.prov, " ")); row.scope = "section"; }
    row.figures = figs.length;
    row.figuresFound = figs.filter((f) => matches(f, pool)).map((f) => f.shown);
    row.figuresNotFound = figs.filter((f) => !matches(f, pool)).map((f) => f.shown);
  }
  rows.push(row);
}
const fails = rows.filter((r) => r.result === "FAIL");
const failedUrls = Object.entries(results).filter(([, r]) => r.problems.length);
writeFileSync(OUT, JSON.stringify({ base: BASE, pages, pageProblems, links: rows.length, addresses: urls.length, failedAddresses: failedUrls.length, rows, results: Object.fromEntries(Object.entries(results).map(([u, r]) => [u, { ...r, json: undefined, viewText: undefined }])) }, null, 1));
writeFileSync(OUT.replace(/\.json$/, ".full.json"), JSON.stringify({ full: results }));
// A table for the report.
let md = `# Studio links followed from the site\n\n${rows.length} links on ${pages.length} pages, ${urls.length} distinct Studio addresses, ${failedUrls.length} failed.\n\n`;
md += `| Page | Where | Link text | Studio address | Result | Page figures found in the Studio's result |\n|---|---|---|---|---|---|\n`;
const seen = new Set();
for (const r of rows.filter((x) => x.open && !x.page.startsWith("docs/"))) {
  const key = `${r.page}|${r.where}|${r.link}`;
  if (seen.has(key)) continue; seen.add(key);
  md += `| ${r.page} | ${r.where} | ${r.text} | \`${r.link.replace(/^.*playground\//, "")}\` | ${r.result}${r.live === false ? " (recorded run)" : ""}${r.problems.length ? ": " + r.problems.join("; ") : ""} | ${r.figures ? `${r.scope === "section" ? "section text: " : ""}${r.figuresFound.length} of ${r.figures}${r.figuresNotFound.length ? ` (not found: ${r.figuresNotFound.slice(0, 6).join(", ")})` : ""}` : "no figure beside the link"} |\n`;
}
writeFileSync(OUT.replace(/\.json$/, ".md"), md);
for (const [u, r] of failedUrls.slice(0, 40)) console.log(`FAIL ${u.replace(BASE, "")}\n     ${r.problems.join("\n     ")}`);
console.log(`${rows.length} links, ${urls.length} distinct addresses: ${urls.length - failedUrls.length} pass, ${failedUrls.length} fail; page problems: ${Object.values(pageProblems).flat().length}`);
console.log(failedUrls.length || Object.values(pageProblems).flat().length ? "STUDIO LINKS: problems found" : "STUDIO LINKS: every link opens its scenario and view, runs, and is clean");
process.exit(failedUrls.length ? 1 : 0);
