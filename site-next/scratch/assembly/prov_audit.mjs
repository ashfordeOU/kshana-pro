// Renders each site page and lists every provenance line and every Studio link as the reader's
// browser has them after the page scripts ran (the static check cannot see script-written lines).
// Run: node prov_audit.mjs [--base http://127.0.0.1:8841/] [--out reports/prov-audit.json]
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
const ROOT = new URL("../../", import.meta.url).pathname;
const require = createRequire(ROOT + "explorations/2026-09-27-audit/matrix/");
const { chromium } = require("playwright");
const args = process.argv.slice(2);
const opt = (n, d) => { const i = args.indexOf(n); return i < 0 ? d : args[i + 1]; };
const BASE = opt("--base", "http://127.0.0.1:8841/");
const OUT = opt("--out", ROOT + "scratch/assembly/reports/prov-audit.json");
const PAGES = ["index.html", "missions.html", "capabilities.html", "evidence.html", "developers.html", "editions.html"];
const browser = await chromium.launch(process.env.KSHANA_CHROME ? { executablePath: process.env.KSHANA_CHROME } : {});
const report = {};
let missing = 0;
for (const pg of PAGES) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" });
  const page = await ctx.newPage();
  const errors = [];
  page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("requestfailed", (r) => errors.push(`request failed: ${r.url()}`));
  page.on("response", (r) => { if (r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
  await page.goto(BASE + pg, { waitUntil: "load", timeout: 120000 });
  await page.waitForTimeout(2500);
  // Walk the page so on-view modules draw, and open every tab panel once.
  const h = await page.evaluate(() => document.documentElement.scrollHeight);
  for (let y = 0; y < h; y += 700) { await page.evaluate((yy) => window.scrollTo(0, yy), y); await page.waitForTimeout(350); }
  await page.waitForTimeout(1500);
  // Open every tab once (a panel's script-written lines appear when it is first shown), then walk again.
  const nTabs = await page.evaluate(() => document.querySelectorAll('[role="tab"]').length);
  for (let i = 0; i < nTabs; i++) {
    await page.evaluate((k) => { const t = document.querySelectorAll('[role="tab"]')[k]; if (t && t.getClientRects().length) { t.scrollIntoView({ block: "center" }); t.click(); } }, i);
    await page.waitForTimeout(250);
  }
  for (let y = 0; y < h; y += 700) { await page.evaluate((yy) => window.scrollTo(0, yy), y); await page.waitForTimeout(200); }
  await page.waitForTimeout(1500);
  const data = await page.evaluate(() => {
    const secOf = (el) => { const s = el.closest("section[id], header[id], div[id], article[id]"); const top = el.closest("section[id], main > *[id]"); return (top && top.id ? top.id : "-") + (s && s.id && (!top || s.id !== top.id) ? " / " + s.id : ""); };
    const studio = (a) => /playground\/index\.html\?/.test(a.getAttribute("href") || "");
    const prov = [...document.querySelectorAll(".prov")].map((p) => {
      const files = [...p.querySelectorAll("code")].map((c) => c.textContent).filter((t) => /\.toml$/.test(t));
      const links = [...p.querySelectorAll("a")].filter(studio).map((a) => a.getAttribute("href"));
      return { where: secOf(p), text: p.textContent.replace(/\s+/g, " ").trim().slice(0, 160), files, links, hidden: !p.getClientRects().length };
    });
    const links = [...document.querySelectorAll("a")].filter(studio).map((a) => ({ where: secOf(a), text: a.textContent.replace(/\s+/g, " ").trim().slice(0, 60), href: a.getAttribute("href"), hidden: !a.getClientRects().length }));
    const cards = [...document.querySelectorAll(".sc-next[data-run]")].map((c) => ({ run: c.getAttribute("data-run"), links: [...c.querySelectorAll("a")].filter(studio).map((a) => a.getAttribute("href")) }));
    return { prov, links, cards };
  });
  // A second provenance line for a visual that already has its link (same section) is covered by it.
  const has = (p, links) => links.some((l) => l.includes(`scenario=${p.files[0].replace(/\.toml$/, "")}&`) || l.endsWith(`scenario=${p.files[0].replace(/\.toml$/, "")}`));
  const sectionLinks = (p) => data.prov.filter((o) => o.where.split(" / ")[0] === p.where.split(" / ")[0] && p.where !== "-").flatMap((o) => o.links);
  const noLink = data.prov.filter((p) => p.files.length && !has(p, p.links) && !has(p, sectionLinks(p)) && !p.links.some((l) => l.includes(`scenario=${p.files[0].replace(/\.toml$/, "")}&`) || l.endsWith(`scenario=${p.files[0].replace(/\.toml$/, "")}`)));
  const cardNoLink = data.cards.filter((c) => !c.links.some((l) => l.includes(`scenario=${c.run}&`)));
  missing += noLink.length + cardNoLink.length;
  report[pg] = { errors, prov: data.prov.length, studioLinks: data.links.length, provWithoutStudioLink: noLink, cardsWithoutStudioLink: cardNoLink, provLines: data.prov, links: data.links };
  console.log(`${pg}: ${data.prov.length} provenance lines, ${data.links.length} Studio links, ${data.cards.length} engine-run cards; without a Studio link: ${noLink.length} lines, ${cardNoLink.length} cards; console/network problems: ${errors.length}`);
  for (const p of noLink) console.log(`   NO LINK  [${p.where}] ${p.text}`);
  for (const c of cardNoLink) console.log(`   NO LINK  card ${c.run}`);
  for (const p of data.prov.filter((x) => !x.files.length)) console.log(`   no file named [${p.where}] ${p.text.slice(0, 110)}`);
  for (const e of errors.slice(0, 6)) console.log(`   ERR ${e}`);
  await ctx.close();
}
await browser.close();
writeFileSync(OUT, JSON.stringify(report, null, 1));
console.log(missing ? `PROVENANCE AUDIT: ${missing} run visual(s) without a Studio link` : "PROVENANCE AUDIT: every run visual links the Studio");
process.exit(missing ? 1 : 0);
