// Render the video frame by frame: node render.mjs --variant promo|branded|showcase|feed --out file.mp4
//   or a single frame:             node render.mjs --variant promo --still 20 --out frame.png
// Needs Playwright with Chromium (PLAYWRIGHT_BROWSERS_PATH / executablePath below) and ffmpeg.
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
const require = createRequire(process.env.NODE_PATH_PW || "/opt/node22/lib/node_modules/");
const { chromium } = require("playwright");
const here = path.dirname(new URL(import.meta.url).pathname);
const arg = (n, d) => { const i = process.argv.indexOf(n); return i < 0 ? d : process.argv[i + 1]; };
const variant = arg("--variant", "promo"), out = arg("--out", `${variant}.mp4`), still = arg("--still", null);
const FPS = 30, DURATION = 84;
const V = { promo: [1920, 1080, 0, 0], branded: [1920, 1080, 1, 0], showcase: [1080, 1080, 0, 1], "showcase-branded": [1080, 1080, 1, 1], feed: [1080, 1350, 0, 1] }[variant];
if (!V) throw new Error("unknown variant " + variant);
const [W, H, brand, sq] = V;
const MIME = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json", ".svg": "image/svg+xml", ".woff2": "font/woff2" };
const srv = http.createServer((req, res) => { const f = path.join(here, decodeURIComponent(req.url.split("?")[0]).replace(/^\/$/, "/index.html")); fs.readFile(f, (e, b) => { if (e) { res.writeHead(404); res.end(); } else { res.writeHead(200, { "content-type": MIME[path.extname(f)] || "application/octet-stream" }); res.end(b); } }); }).listen(0);
const port = srv.address().port;
const b = await chromium.launch({ ...(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {}), args: ["--no-sandbox"] });
const p = await (await b.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 1 })).newPage();
p.on("pageerror", (e) => console.error("page error:", e.message));
await p.goto(`http://127.0.0.1:${port}/index.html?brand=${brand}&sq=${sq}`);
await p.waitForFunction("window.__ready === true");
await p.evaluate(() => document.fonts.ready);
if (arg("--check", null) != null) {
  // overflow check: at the end of every scene's build-up, does its content fit the scene box?
  const res = await p.evaluate(() => { const o = []; for (const s of document.querySelectorAll(".scene")) { const id = s.id; const t0 = window.__starts[id]; window.__render(t0 + (window.__ends[id] - t0) - 0.7); const r = s.getBoundingClientRect(); let mn = 1e9, mx = -1e9; s.querySelectorAll("*").forEach((c) => { const b = c.getBoundingClientRect(); if (b.height > 0) { mn = Math.min(mn, b.top); mx = Math.max(mx, b.bottom); } }); const kids = [...s.children].map((c) => c.getBoundingClientRect()); let ovl = 0; for (let i = 0; i < kids.length; i++) for (let j = i + 1; j < kids.length; j++) if (kids[i].height > 0 && kids[j].height > 0 && kids[i].bottom > kids[j].top + 2 && kids[i].top < kids[j].bottom) ovl += Math.round(kids[i].bottom - kids[j].top); o.push([id, Math.round(mx - mn), Math.round(r.height), Math.round(Math.max(0, r.top - mn) + Math.max(0, mx - r.bottom)) + ovl]); } return o; });
  for (const [id, sh, ch, ov] of res) console.log(`${ov > 2 ? "OVERFLOW" : "ok      "} ${id.padEnd(8)} content ${sh} / box ${ch} (outside by ${ov})`);
} else if (still != null) { await p.evaluate((t) => window.__render(t), Number(still)); await p.waitForTimeout(100); await p.screenshot({ path: out }); }
else {
  const ff = spawn("ffmpeg", ["-v", "error", "-y", "-f", "image2pipe", "-framerate", String(FPS), "-i", "-", "-c:v", "libx264", "-preset", "slow", "-crf", "27", "-pix_fmt", "yuv420p", "-r", String(FPS), "-movflags", "+faststart", out], { stdio: ["pipe", "inherit", "inherit"] });
  const n = FPS * DURATION;
  for (let i = 0; i < n; i++) {
    await p.evaluate((t) => window.__render(t), i / FPS);
    const png = await p.screenshot({ type: "png" });
    if (!ff.stdin.write(png)) await new Promise((r) => ff.stdin.once("drain", r));
    if (i % 300 === 0) console.log(`frame ${i}/${n}`);
  }
  ff.stdin.end(); await new Promise((r) => ff.on("close", r));
}
await b.close(); srv.close();
