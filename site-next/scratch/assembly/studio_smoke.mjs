// Opens the Studio from the built site and checks that it boots, lists its library and runs.
// Run: node studio_smoke.mjs <expect-celeste: yes|no> [base]
import { createRequire } from "node:module";
const ROOT = new URL("../../", import.meta.url).pathname;
const require = createRequire(ROOT + "explorations/2026-09-27-audit/matrix/");
const { chromium } = require("playwright");
const want = process.argv[2] === "yes", base = process.argv[3] || "http://127.0.0.1:8841/";
const b = await chromium.launch(); const p = await (await b.newContext({ viewport: { width: 1440, height: 900 } })).newPage();
const errs = [], reqs = [];
p.on("console", (m) => { if (m.type() === "error") errs.push(m.text()); }); p.on("pageerror", (e) => errs.push(String(e)));
p.on("response", (r) => { reqs.push(`${r.status()} ${r.url().replace(base, "")}`); });
await p.goto(base + "playground/index.html?scenario=clock-holdover&tab=report", { waitUntil: "load", timeout: 180000 });
await p.waitForFunction(() => document.getElementById("status").classList.contains("ran"), null, { timeout: 240000 });
await p.waitForTimeout(1500);
const st = await p.evaluate(() => ({ status: document.getElementById("status").textContent, tab: document.querySelector('#tabs [aria-selected="true"]').dataset.tab, lib: document.getElementById("library") ? document.getElementById("library").innerText : document.body.innerText, matrix: document.getElementById("matrix-line").textContent, engine: document.getElementById("engine-label").textContent }));
const has = /celeste/i.test(st.lib);
const bad = reqs.filter((r) => !r.startsWith("200") && !r.startsWith("304") && !r.startsWith("206"));
console.log(`status: ${st.status}\nengine: ${st.engine}\ntab: ${st.tab}\nledger line: ${st.matrix}\noptional group in the library: ${has} (expected ${want})\nrequests: ${reqs.length}, not OK: ${bad.length ? bad.join("; ") : "none"}\npacks fetched: ${reqs.filter((r) => /pack-\d+/.test(r)).map((r) => r.split(" ")[1]).join(", ") || "none"}\nconsole errors: ${errs.length ? errs.join(" | ") : "none"}`);
await b.close();
process.exit(has === want && !errs.length && !bad.length && /^Ran locally/.test(st.status) && st.tab === "report" ? 0 : 1);
