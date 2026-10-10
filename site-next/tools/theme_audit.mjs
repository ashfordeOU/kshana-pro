// Theme audit: classify every top-level section of the site against the Observatory theme.
// For each page in <urls file> (one URL per line, e.g. a locally served web/ via
// features/serve.py), it reads the computed colour, background, border, fill and stroke of up to
// 4,000 elements per section, classifies each colour by exact hex as combo (tokens.css alone),
// Observatory (theme.css), warm (the run-report and July chart palettes) or gold (Pro documents),
// records the heading font, and screenshots the section. Verdict used in
// handover/post-0.32/THEME-AUDIT-0.33.md: OLD when combo > Observatory and combo >= 25, MIXED when
// combo >= 25, otherwise CURRENT.
// Usage: node theme_audit.mjs <urls file> <out dir> [light|dark]
//   Needs playwright installed where it runs (`npm i playwright`, with
//   PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 if a Chromium exists); CHROMIUM_PATH picks the browser binary.
import { chromium } from "playwright";
import fs from "node:fs";
const [,, urlsFile, outDir, scheme="light"] = process.argv;
const urls = fs.readFileSync(urlsFile, "utf8").trim().split("\n");
const P = {
  combo: ["#B8262B","#6236E0","#006C7D","#0A6F38","#7C4800","#E5383B","#6B3FF0","#1F55F0","#008795","#0F8A47","#F5A300","#FF5B54","#A58BFF","#2CD0DE","#3DDC84","#FFB224","#9C7DFF","#5A8CFF","#24C8D6","#33CC78","#07080E","#0D0F18","#12141F","#181B28","#F2F4FB","#A6ADC4","#858CA6","#05060C","#0C0E18","#151826","#EEF0FA","#A3AAC2","#8088A6","#C9321F","#6B4FD8","#2F55C8","#3552A8","#1F7A3E","#7E4B00","#6A2CD6"],
  obs: ["#6A98FF","#066A86","#A8247F","#2F6E0A","#764600","#B42D1B","#7389BF","#3DDCF7","#F45CCB","#A8EE5E","#FFB547","#FF6A5C","#4F6FB0","#060A14","#0A1122","#0E1730","#0D1528","#111B33","#EAF0FF","#A7B4D2","#8190B0","#56637F","#D3DBE8","#66718A"],
  warm: ["#9A5B22","#E0A36A","#F0C060","#ECE6DA","#FBFAF7","#F3EFE7","#1D1A15","#62594B","#D9D2C5","#12100C","#1C1914","#3A3328","#C79E63","#241E18"],
  gold: ["#B6985A","#9A7E45","#7D6636","#8E7645","#11161E","#F6F4F0","#ECE6D8"],
};
const cls = {}; for (const [k, v] of Object.entries(P)) for (const h of v) cls[h] = k;
const b = await chromium.launch(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {});
const ctx = await b.newContext({ viewport: { width: 1280, height: 900 }, colorScheme: scheme });
const out = [];
for (const url of urls) {
  const p = await ctx.newPage();
  try { await p.goto(url, { waitUntil: "networkidle", timeout: 60000 }); } catch (e) { out.push({ url, error: String(e).slice(0, 200) }); await p.close(); continue; }
  await p.evaluate(async () => { for (let y = 0; y < document.body.scrollHeight; y += 700) { window.scrollTo(0, y); await new Promise(r => setTimeout(r, 120)); } window.scrollTo(0, 0); });
  await p.waitForTimeout(800);
  const secs = await p.evaluate((cls) => {
    const hex = (c) => { const m = c && c.match(/rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?/); if (!m) return null; if (m[4] !== undefined && +m[4] < 0.25) return null; return "#" + [m[1], m[2], m[3]].map(x => (+x).toString(16).padStart(2, "0")).join("").toUpperCase(); };
    const root = document.querySelector("main") || document.body;
    let list = [...(document.querySelector("body > header, header.nav, nav") ? [document.querySelector("body > header, header.nav, nav")] : []), ...root.children, ...(document.querySelector("footer") ? [document.querySelector("footer")] : [])];
    list = list.filter(e => e.getBoundingClientRect().height > 40 && getComputedStyle(e).display !== "none");
    return list.map((s, i) => {
      s.setAttribute("data-audit", String(i));
      const t = { combo: 0, obs: 0, warm: 0, gold: 0 }, hits = {}, radii = {}, fonts = {};
      const els = [s, ...s.querySelectorAll("*")].slice(0, 4000);
      for (const e of els) {
        const cs = getComputedStyle(e);
        const props = [cs.color, cs.backgroundColor, parseFloat(cs.borderTopWidth) > 0 ? cs.borderTopColor : null, cs.fill !== "none" ? cs.fill : null, cs.stroke !== "none" ? cs.stroke : null];
        for (const c of props) { const h = hex(c); if (h && cls[h]) { t[cls[h]]++; hits[h] = (hits[h] || 0) + 1; } }
        for (const a of ["fill", "stroke"]) { const v = e.getAttribute && e.getAttribute(a); const h = v && v.toUpperCase(); if (h && cls[h]) { t[cls[h]]++; hits[h] = (hits[h] || 0) + 1; } }
        const r = cs.borderTopLeftRadius; if (r && r !== "0px" && parseFloat(cs.borderTopWidth) + (cs.backgroundColor !== "rgba(0, 0, 0, 0)" ? 1 : 0) > 0) radii[r] = (radii[r] || 0) + 1;
        if (/^H[1-4]$/.test(e.tagName)) { const f = cs.fontFamily.split(",")[0].replace(/"/g, ""); fonts[f] = (fonts[f] || 0) + 1; }
      }
      const h = s.querySelector("h1,h2,h3,h4");
      return { i, tag: s.tagName.toLowerCase(), id: s.id || "", cls: (s.className && s.className.baseVal === undefined ? s.className : "").toString().slice(0, 60), heading: h ? h.textContent.trim().replace(/\s+/g, " ").slice(0, 70) : "", height: Math.round(s.getBoundingClientRect().height), t, hits, radii, fonts };
    });
  }, cls);
  const slug = (url.replace(/^https?:\/\/[^/]+\//, "") || "home").replace(/\/$/, "").replace(/\//g, "_") || "home";
  for (const s of secs) {
    try { const el = await p.$(`[data-audit="${s.i}"]`); const bb = await el.boundingBox(); if (bb) { s.shot = `${slug}--${s.i}-${scheme}.jpg`; await p.screenshot({ path: `${outDir}/${s.shot}`, type: "jpeg", quality: 40, fullPage: true, clip: { x: 0, y: bb.y + (await p.evaluate(() => scrollY)), width: 1280, height: Math.min(bb.height, 1400) } }); } } catch (e) { s.shotErr = String(e).slice(0, 100); }
  }
  out.push({ url, sections: secs });
  await p.close();
  process.stderr.write(`${url} ${secs.length}\n`);
}
fs.writeFileSync(`${outDir}/audit-${scheme}.json`, JSON.stringify(out, null, 1));
await b.close();
