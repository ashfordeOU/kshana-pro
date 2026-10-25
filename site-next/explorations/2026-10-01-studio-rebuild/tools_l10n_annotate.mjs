#!/usr/bin/env node
// Adds data-l10n / data-l10n-attr to the Studio's static HTML wherever a text or an attribute equals the English value of a
// studio-* message in i18n/en/ui.ftl (idempotent). Usage: node tools_l10n_annotate.mjs [--ui <ui.ftl>] [--check] <html file>...
//   --check   change nothing; exit 1 when the files would change (a gate), and print the coverage figure.
import { readFileSync, writeFileSync } from "node:fs";

const argv = process.argv.slice(2);
let ui = "/workspace/kshana/i18n/en/ui.ftl", check = false;
const files = [];
for (let i = 0; i < argv.length; i++) { if (argv[i] === "--ui") ui = argv[++i]; else if (argv[i] === "--check") check = true; else files.push(argv[i]); }

const byValue = new Map(); // English value -> id (a value shared by two ids is ambiguous and left alone)
for (const line of readFileSync(ui, "utf8").split("\n")) {
  const m = /^(studio-[\w-]+) = (.+)$/.exec(line);
  if (!m || /[{<]/.test(m[2])) continue;
  byValue.set(m[2], byValue.has(m[2]) ? null : m[1]);
}
const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#x27;");
const unesc = (s) => s.replace(/&#x27;|&#39;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");

let changed = 0, total = 0, hit = 0;
for (const f of files) {
  const html = readFileSync(f, "utf8");
  let out = html;
  // attributes
  out = out.replace(/<([a-zA-Z][\w-]*)\b((?:"[^"]*"|'[^']*'|[^'">])*)>/g, (tag, name, attrs) => {
    if (/\bdata-l10n-attr=/.test(attrs)) return tag;
    const pairs = [];
    for (const a of ["aria-label", "title", "placeholder"]) {
      const m = new RegExp(`\\s${a}="([^"]*)"`).exec(attrs);
      if (!m) continue;
      total++;
      const id = byValue.get(unesc(m[1]));
      if (id) { pairs.push(`${a}:${id}`); hit++; }
    }
    return pairs.length ? tag.replace(/>$/, ` data-l10n-attr="${pairs.join(";")}">`) : tag;
  });
  // leaf elements whose whole content is one message
  out = out.replace(/<(h[1-6]|p|span|summary|button|a|label|li|th|td|small|strong|div)\b((?:"[^"]*"|'[^']*'|[^'">])*)>([^<>]+)<\/\1>/g, (all, name, attrs, text) => {
    if (/\bdata-l10n=/.test(attrs)) return all;
    const t = unesc(text.trim());
    if (!t) return all;
    total++;
    const id = byValue.get(t);
    if (!id) return all;
    hit++;
    return `<${name}${attrs} data-l10n="${id}">${text}</${name}>`;
  });
  if (!out.includes("l10n-boot.mjs")) out = out.replace("</body>", `<script type="module" src="lib/l10n-boot.mjs"></script>\n</body>`);
  if (out !== html) { changed++; if (!check) writeFileSync(f, out); }
}
console.log(`l10n annotate: ${hit} of ${total} candidate strings in ${files.length} file(s) are catalogue messages${check ? "" : "; " + changed + " file(s) updated"}`);
if (check && changed) process.exit(1);
