// SPDX-License-Identifier: AGPL-3.0-only
// Browser check of lib/svgsafe.mjs on a real parsed XML document, the way the Studio adopts a chart:
//   node check_svgsafe.mjs [folder of chart .svg files]
// Each vector is adopted twice: untouched (the control, which must run its handler, so the check can
// fail) and through sanitizeSvg (which must not). A folder of engine charts is checked the same way.
// Needs Playwright and a Chromium: PLAYWRIGHT_MODULE (default /opt/node-tools/node_modules) and,
// optionally, CHROMIUM (path to the browser).
import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const lib = path.resolve(here, "../../explorations/2026-10-01-studio-rebuild/lib/svgsafe.mjs");
const req = createRequire((process.env.PLAYWRIGHT_MODULE || "/opt/node-tools/node_modules") + "/");
const { chromium } = req("playwright");

const NS = 'xmlns="http://www.w3.org/2000/svg"';
const vectors = {
  "image handler": `<svg ${NS}><image href="x" onerror="window.__p=1"/></svg>`,
  "handler in a text run": `<svg ${NS}><text>a</text><image href="x" onerror="window.__p=1"/><text>b</text></svg>`,
  "embedded document": `<svg ${NS}><foreignObject><body xmlns="http://www.w3.org/1999/xhtml"><img src="x" onerror="window.__p=1"/></body></foreignObject></svg>`,
  "animated handler": `<svg ${NS}><image href="x"><set attributeName="onerror" to="window.__p=1"/></image></svg>`,
  "script element": `<svg ${NS}><script>window.__p=1</script></svg>`,
  "script address": `<svg ${NS}><a href="javascript:window.__p=1"><text>x</text></a></svg>`,
  "outside link": `<svg ${NS}><image href="https://other.example/x.png" onerror="window.__p=1"/></svg>`,
};
if (process.argv[2]) for (const f of fs.readdirSync(process.argv[2]).filter((n) => n.endsWith(".svg")).sort()) vectors[`chart ${f}`] = fs.readFileSync(path.join(process.argv[2], f), "utf8");

const source = fs.readFileSync(lib, "utf8").replace(/^export /gm, "") + "\nwindow.svgsafe = { hasDeclaration, sanitizeSvg };";
const browser = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
const page = await browser.newPage();
await page.setContent("<!doctype html><div id=box></div>");
await page.addScriptTag({ content: source });

// Adopt as the Studio does (app.js svgNode), with or without the sanitiser.
const adopt = (markup, clean) => page.evaluate(({ markup, clean }) => {
  window.__p = 0;
  if (window.svgsafe.hasDeclaration(markup)) return { drawn: false, why: "declaration" };
  const root = new DOMParser().parseFromString(markup, "image/svg+xml").documentElement;
  if (!root || root.nodeName !== "svg") return { drawn: false, why: "not svg" };
  if (clean) window.svgsafe.sanitizeSvg(root);
  const box = document.getElementById("box");
  box.replaceChildren(document.importNode(root, true));
  // What is left that could run or load something, counted on the live tree (text is not markup).
  let bad = 0;
  for (const e of box.querySelectorAll("*")) {
    if (/^(script|foreignobject|iframe|object|embed|audio|video|canvas|link|meta|base)$/i.test(e.localName)) bad++;
    for (const a of e.attributes) {
      if (/^on/i.test(a.name) || (a.localName.toLowerCase() === "href" && !/^\s*#/.test(a.value)) || /javascript\s*:/i.test(a.value.replace(/[\u0000-\u0020]+/g, ""))) bad++;
    }
  }
  return { drawn: true, bad };
}, { markup, clean });

const failures = [];
let controls = 0;
for (const [name, markup] of Object.entries(vectors)) {
  const raw = await adopt(markup, false);
  const injected = raw.drawn && raw.bad > 0;
  await page.waitForTimeout(150);
  const ran = (await page.evaluate(() => window.__p)) === 1;
  if (injected && ran) controls++;
  const clean = await adopt(markup, true);
  await page.waitForTimeout(150);
  const ranClean = (await page.evaluate(() => window.__p)) === 1;
  const ok = clean.drawn && !ranClean && clean.bad === 0;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}: control ${raw.drawn ? `${raw.bad} live item(s)${ran ? ", handler ran" : ""}` : raw.why}; sanitised ${clean.drawn ? `${clean.bad} left${ranClean ? ", HANDLER RAN" : ""}` : clean.why}`);
  if (!ok) failures.push(name);
}
await browser.close();
if (controls === 0) { console.error("FAIL: no control vector ran its handler, so this check could not have failed"); process.exit(1); }
if (failures.length) { console.error(`FAIL: ${failures.length} vector(s) not neutralised`); process.exit(1); }
console.log(`svgsafe browser check: ${Object.keys(vectors).length} vectors inert after sanitising; ${controls} controls ran their handler untouched`);
