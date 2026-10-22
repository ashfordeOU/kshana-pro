// Build the docs section of the site from git-tracked Markdown in the Kshana repository.
// Pinned renderer (marked 12 + marked-gfm-heading-id 3, highlight.js 11); run by build.py.
// Only files that `git ls-files` lists are rendered, so an untracked working file never ships.
// Usage: node build_docs.mjs <config json written by build.py>
import fs from "node:fs";
import path from "node:path";
import { Marked } from "marked";
import { gfmHeadingId, getHeadingList } from "marked-gfm-heading-id";
import hljs from "highlight.js";
import { execFileSync } from "node:child_process";

const cfg = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const REPO = cfg.repo, OUT = cfg.out;
const TRACKED = new Set(cfg.tracked);
const REMOTE = new Set(cfg.remote || []);
// A GitHub link, or a plain note when the file is committed locally but not yet pushed.
const ghLink = (p, label) => (REMOTE.has(p) ? `<a href="${GH}${p}">${label}</a>` : `<span class="not-pushed">${label.replace(/^(View|Edit) on GitHub$/, "Not yet on GitHub")}</span>`);
const GH = "https://github.com/ashfordeOU/kshana/blob/main/";
// Read the committed version (HEAD), so work in progress in the checkout never reaches the site.
const gitShow = (p) => execFileSync("git", ["-C", REPO, "show", `HEAD:${p}`], { maxBuffer: 64 * 1024 * 1024 });
// The in-browser app's name comes from the site's one constant (src/data/brand.json, via build.py).
const STUDIO = cfg.studio || "Kshana Studio";
// The Studio's folder on the site (cfg.studioDir: "studio" since 0.29.2, served at /studio/).
const STUDIO_DIR = cfg.studioDir || "playground";
const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
// Text taken from source comments (kind and file descriptions, capability cards, ledger cells) may carry
// Markdown inline code: escape it, then turn each `x` into a code chip so readers never see raw backticks.
// Public wording, the same rules as build.py's PUBLIC_WORDING: no internal process words and no AI naming
// reach the public docs. "AI/ML" becomes "machine learning" (capitalised unless a lower-case word leads in).
const publicWording = (t) => t
  .replace(/\bfounder-gated\b/g, "pending review")
  .replace(/AI\/ML \(AI: artificial intelligence; ML: machine learning\)/g, "AI/ML")
  .replace(/AI\/ML, artificial intelligence \/ machine learning; /g, "")
  .replace(/(?<![\w/])AI\/ML(?![\w/])( (?=[A-Za-z]))?/g, (m, sp, off, str) => {
    const w = /^[a-z,&] $/.test(str.slice(Math.max(0, off - 2), off)) ? "machine" : "Machine";
    return sp ? `${w}-learning ` : `${w} learning`;
  });
// Heading text from the renderer carries HTML entities; decode before escaping once for the TOC.
const unent = (t) => String(t).replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, "&");
const escIC = (s) => esc(s).replace(/`([^`\n]+)`/g, "<code>$1</code>");

// ---------------------------------------------------------------- the docs, grouped
// [repo path, slug, sidebar title]; the page title comes from the file's own first heading.
const GROUPS = [
  ["Get started", [["README.md", "overview", "Overview"], ["docs/CONCEPTS.md", "concepts", "Concepts primer"], ["docs/GLOSSARY.md", "glossary", "Glossary"], ["web/README.md", "studio", "Studio guide"], ["docs/tutorials/README.md", "tutorials", "Tutorials"],
    ["docs/tutorials/01-first-orbit.md", "tutorial-first-orbit", "1 · Your first orbit"], ["docs/tutorials/02-clock-holdover.md", "tutorial-clock-holdover", "2 · Clock holdover"],
    ["docs/tutorials/03-quantum-vs-classical.md", "tutorial-quantum-vs-classical", "3 · Quantum against classical"], ["docs/examples/multi-tool-pipeline.md", "example-multi-tool", "Worked example: multi-tool pipeline"]]],
  ["Scenarios", [["docs/SCENARIOS.md", "scenarios", "Scenario kinds"], ["docs/KINDS.md", "kinds", "Every scenario kind, by family"], ["docs/COMMANDS.md", "commands", "Every command"], ["docs/ALL-SURFACES.md", "all-surfaces", "Every surface"], ["docs/CAPABILITY-DOMAINS.md", "capability-domains", "Capability domains"], ["@scenario-reference", "scenario-reference", "Scenario reference"], ["docs/SCHEMA.md", "schema", "Result schema"], ["docs/REAL_TLE_GUIDE.md", "real-tle-guide", "Using real TLEs"], ["docs/CAMPAIGNS.md", "campaigns", "Campaigns"]]],
  ["Outputs and exports", [["docs/REPORTS.md", "reports", "Run reports"], ["docs/ANIMATION.md", "animation", "Animation export"], ["docs/INTEROP.md", "interop", "Interoperability exports and imports"]]],
  ["Validation and evidence", [["docs/VALIDATION.md", "validation", "Validation status"], ["docs/VERIFICATION-MATRIX.md", "verification-matrix", "Verification matrix"], ["@capability-reference", "capability-reference", "Capability reference"],
    ["docs/MODELLED-RATIONALE.md", "modelled-rationale", "Why rows stay Modelled"], ["docs/CLAIMS-VS-REALITY.md", "claims-vs-reality", "Claims against reality"], ["docs/PROVENANCE.md", "provenance", "Provenance"],
    ["docs/REPRODUCIBILITY.md", "reproducibility", "Reproducibility"], ["docs/CAPABILITY.md", "capability-map", "Capability map"], ["docs/STANDARDS.md", "standards", "Standards and interoperability"], ["docs/COVERAGE.md", "line-coverage", "Line coverage"], ["docs/EVIDENCE-PACKS.md", "evidence-packs", "Evidence packs"]]],
  ["Timing", [["docs/OPTICAL-ATMOSPHERE.md", "optical-atmosphere", "Optical links through the atmosphere"], ["docs/SLOT-TIMING.md", "slot-timing", "Slot timing"], ["docs/TELECOM-TIMING.md", "telecom-timing", "Telecom timing"], ["docs/DECEIVED-TIME.md", "deceived-time", "Deceived time"],
    ["docs/TIMING-HIL.md", "timing-hil", "Holdover test and timing hardware in the loop"], ["docs/CLOCK-LOG-FORMATS.md", "clock-log-formats", "Clock log formats"], ["docs/HARDWARE-SAFETY.md", "hardware-safety", "Hardware safety"]]],
  ["Integrity and interference", [["docs/INTEGRITY.md", "integrity", "Integrity and security"], ["docs/SPECTRUM.md", "spectrum", "Spectrum and waterfall"], ["docs/ARAIM_REFERENCE.md", "araim-reference", "ARAIM reference"], ["docs/COMPLIANCE.md", "compliance", "DO-316 / DO-229E map"], ["docs/RESILIENCE-CROSSWALK.md", "resilience-crosswalk", "Resilience cross-walk"]]],
  ["Receiver trust and maritime", [["docs/RECEIVER-TRUST.md", "receiver-trust", "Receiver trust"], ["docs/MARITIME-TRUST.md", "maritime-trust", "Maritime trust"], ["docs/NMEA-TRAINING.md", "nmea-training", "NMEA training streams"],
    ["docs/INTERFERENCE-MAP.md", "interference-map", "Interference map and route exposure"], ["docs/data/INTERFERENCE-DATA-SOURCES.md", "interference-data-sources", "Interference map: data sources and licences"], ["docs/MARINE-INTEGRATIONS.md", "marine-integrations", "Marine integrations"], ["docs/SKYLINE.md", "skyline", "Sky geometry and skylines"], ["docs/TRUST-TELEMETRY.md", "trust-telemetry", "Trust as security telemetry"], ["docs/OSNMA.md", "osnma", "Galileo OSNMA verification"], ["docs/LUNANET-AFS.md", "lunanet-afs", "LunaNet augmented forward signal"],
    ["docs/JAMMERTEST.md", "jammertest", "Jammertest as a referee"], ["docs/MACHINE-INPUTS.md", "machine-inputs", "Machine and sensor inputs"], ["docs/RECEIVER-PROTOCOLS.md", "receiver-protocols", "Receiver protocols read"],
    ["docs/observatory/METHOD.md", "observatory-method", "Interference observatory: method"], ["docs/observatory/RESULTS.md", "observatory-results", "Interference observatory: results"], ["docs/BENCH.md", "bench", "kshana-bench"]]],
  ["Compliance and test bench", [["docs/TEST-BENCH.md", "test-bench", "Running a receiver against a scenario"], ["docs/compliance/README.md", "compliance-mapping", "Compliance mapping"], ["docs/compliance/dhs-rpcf.md", "compliance-dhs-rpcf", "US DHS resilient PNT framework"],
    ["docs/compliance/imo-ships.md", "compliance-imo-ships", "IMO guidance for ships"], ["docs/compliance/easa-aviation.md", "compliance-easa-aviation", "EASA guidance for aviation"], ["docs/compliance/nis2-art21.md", "compliance-nis2-art21", "EU NIS2, Article 21"], ["docs/compliance/en-16803.md", "compliance-en-16803", "EN 16803"]]],
  ["Orbits", [["docs/SGP4-VALIDATION.md", "sgp4-validation", "SGP4 validation"], ["docs/AGENCY-ORBIT-VALIDATION.md", "agency-orbit-validation", "Force-model validation"], ["docs/SOLAR-SYSTEM.md", "solar-system", "Solar system and other bodies"], ["docs/LUNAR-REFERENCE-STATION.md", "lunar-reference-station", "Lunar reference station"], ["docs/SSV.md", "ssv", "Space-user navigation (gnss-ssv)"], ["docs/LUNAR-INFRASTRUCTURE.md", "lunar-infrastructure", "Lunar orbit and surface as infrastructure"]]],
  ["Low Earth orbit (LEO) navigation", [["docs/LEO-PNT.md", "leo-pnt", "LEO positioning, navigation and timing"], ["docs/LEO-SIGNAL.md", "leo-signal", "LEO signal designs"],
    ["docs/LEO-PASS.md", "leo-pass", "LEO pass and link budget"], ["docs/LEO-NAVMSG.md", "leo-navmsg", "LEO navigation message"], ["docs/LEO-PNT-FUSION.md", "leo-pnt-fusion", "Fused MEO and LEO positioning"],
    ["docs/CONSTELLATION-DESIGN.md", "constellation-design", "Constellation design"]]],
  ["Quantum sensors", [["docs/QUANTUM-MODELS.md", "quantum-models", "Quantum-sensor models"], ["docs/QUANTUM.md", "quantum-physics", "Cold-atom interferometer"]]],
  ["APIs and integrations", [["docs/PYTHON_API.md", "python-api", "Python API"], ["README.npm.md", "npm", "npm and WebAssembly"], ["README.crates.md", "rust", "Rust crate"],
    ["mcp/kshana-mcp/README.md", "mcp", "MCP server"], ["docs/MCP-INSTALL.md", "mcp-install", "MCP server: every install channel"], ["docs/deploy/mcp-http.md", "mcp-http", "MCP server: self-hosted HTTP mode"], ["docs/AGENTS.md", "agents", "Using Kshana from an agent"], ["docs/SURFACES.md", "surfaces", "Where each capability is reachable"], ["ide/jetbrains/README.md", "jetbrains", "JetBrains plugin"],
    ["docs/DEPLOYMENT-TARGETS.md", "deployment-targets", "Where Kshana runs"], ["docs/ARCHITECTURE.md", "architecture", "Architecture"], ["docs/POSITIONING.md", "positioning", "Where Kshana sits"]]],
  ["Project", [["CHANGELOG.md", "changelog", "Changelog"], ["LICENSING.md", "licensing", "Licensing"],
    ["SECURITY.md", "security", "Security"], ["CONTRIBUTING.md", "contributing", "Contributing"], ["GOVERNANCE.md", "governance", "Governance"], ["CODE_OF_CONDUCT.md", "code-of-conduct", "Code of conduct"]]],
];
// The kind families and the one-page-per-kind reference (docs/KINDS.md links both): written from the tracked
// files, so a kind added to the engine appears without an edit here. The 87 kind pages stay out of the side
// navigation and the index list (they would bury everything else); each is reached from its family page.
const kindFam = cfg.tracked.filter((p) => /^docs\/kinds\/[^/]+\.md$/.test(p)).sort();
GROUPS.push(["Scenario kind families", kindFam.map((p) => [p, "kinds-" + path.posix.basename(p, ".md"), path.posix.basename(p, ".md").replace(/-/g, " ").replace(/^./, (c) => c.toUpperCase())])]);
const kindPages = cfg.tracked.filter((p) => /^docs\/kinds\/by-kind\/[^/]+\.md$/.test(p)).sort();
GROUPS.push(["Scenario kind pages", kindPages.map((p) => [p, "kind-" + path.posix.basename(p, ".md"), path.posix.basename(p, ".md")])]);
const HIDDEN_GROUPS = new Set(["Scenario kind pages"]);
// Left out on purpose: maintainer-process documents, not user-facing.
const EXCLUDED = ["docs/RELEASING.md (release procedure)", "docs/WHEEL_TAGS.md (packaging detail)", "docs/revisions/ (internal change notes)", "docs/design/ (internal design notes)",
  "README.pypi.md (the PyPI page; the Python API doc covers it)", ".github/ templates",
  "docs/integrations.md (organised around named third-party AI products, which this site does not name)"];
// Repository files the site neither renders nor links: a link to one in a rendered page is shown as
// plain text. ROADMAP.md (plans, not a record of what ships) stays in the repository; the site's
// record of what has shipped is the changelog. Kept out of the "Not included" list on purpose, so the
// docs index does not point at it either.
const UNLINKED = new Set(["ROADMAP.md"]);

// The site names no AI vendor or product. A rendered passage that does (a README paragraph, a
// changelog entry) is left out and replaced by a pointer to the repository file.
const VENDOR = /\b(claude|anthropic)\b/i;
const KNOWN_TAGS = new Set("a abbr b blockquote br caption code col colgroup dd del details div dl dt em figcaption figure h1 h2 h3 h4 h5 h6 hr i img ins kbd li mark ol p picture pre q s samp section small source span strong sub summary sup table tbody td tfoot th thead tr u ul var wbr".split(" "));

const pages = [];
for (const [g, items] of GROUPS) for (const [p, slug, title] of items) {
  if (!p.startsWith("@")) {
    if (!TRACKED.has(p)) { console.warn(`docs: skipping ${p}, not tracked by git`); continue; }
    if (!fs.existsSync(path.join(REPO, p))) { console.warn(`docs: skipping ${p}, missing`); continue; }
  }
  pages.push({ group: g, path: p, slug, nav: title, hidden: HIDDEN_GROUPS.has(g) });
}
const bySrc = new Map(pages.filter((p) => !p.path.startsWith("@")).map((p) => [p.path, p]));
fs.mkdirSync(OUT, { recursive: true });

// ---------------------------------------------------------------- link and image rewriting
// Images are embedded as data: URIs, never copied as separate files: the preview host takes at
// most 255 files per publish, and one file per figure would push the docs past it. The set still
// records every repository path embedded, for the provenance manifest check.py reads.
const copied = new Set();
const MIME = { ".svg": "image/svg+xml", ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp", ".gif": "image/gif" };
const dataUri = new Map();
function copyAsset(repoPath) {
  if (!TRACKED.has(repoPath)) return null;
  const mime = MIME[path.posix.extname(repoPath).toLowerCase()];
  if (!mime) return null;
  if (!dataUri.has(repoPath)) {
    const buf = gitShow(repoPath);
    // SVG as percent-encoded UTF-8 (smaller than base64); raster images as base64, wrapped at
    // 76 characters as MIME does. URL parsing drops the newlines, so the image is the same; a
    // one-line run of base64 can match a credential pattern by chance (an AIza... run in a
    // README screenshot did), which a secret scan rightly refuses.
    const uri = mime === "image/svg+xml"
      ? "data:image/svg+xml;charset=utf-8," + buf.toString("utf8").replace(/[\r\n]+/g, " ").replace(/[%#<>"&{}|\\^`]/g, (c) => "%" + c.charCodeAt(0).toString(16).toUpperCase().padStart(2, "0"))
      : `data:${mime};base64,${buf.toString("base64").replace(/.{76}/g, "$&\n")}`;
    dataUri.set(repoPath, uri);
    copied.add(repoPath);
  }
  return dataUri.get(repoPath);
}
function resolveRel(fromPath, href) {
  const base = path.posix.dirname(fromPath);
  return path.posix.normalize(path.posix.join(base === "." ? "" : base, href));
}
const RAW = /^https:\/\/raw\.githubusercontent\.com\/ashfordeou\/kshana\/(?:main|master)\//i;
const BLOB = /^https:\/\/github\.com\/ashfordeou\/kshana\/(?:blob|tree)\/(?:main|master)\//i;
function rewriteHref(fromPath, href) {
  if (!href || href.startsWith("#") || href.startsWith("mailto:")) return href;
  let target = null, frag = "";
  const hashAt = href.indexOf("#");
  const bare = hashAt >= 0 ? href.slice(0, hashAt) : href;
  if (hashAt >= 0) frag = href.slice(hashAt);
  if (BLOB.test(bare)) target = bare.replace(BLOB, "");
  else if (/^[a-z]+:/i.test(bare) || bare.startsWith("//")) return href;
  else target = resolveRel(fromPath, bare);
  target = target.replace(/\/$/, "");
  if (UNLINKED.has(target)) return "";
  const page = bySrc.get(target);
  if (page) return `${page.slug}.html${frag}`;
  if (target === "docs/tutorials" || target === "docs/tutorials/README.md") return `tutorials.html${frag}`;
  // Only paths GitHub already serves (origin/main as last fetched) get a GitHub link; a file that
  // exists only in the docs checkout's branch would be a dead link until it is merged and pushed.
  if (onRemote(target)) return GH + target + frag;
  return null;
}
const REMOTE_DIRS = new Set([...REMOTE].flatMap((p) => p.split("/").slice(0, -1).map((_, i, a) => a.slice(0, i + 1).join("/"))));
const onRemote = (t) => REMOTE.has(t) || REMOTE_DIRS.has(t);
// kshana.dev in the repository means the in-browser app (the old single-page site). On this site the
// app is the Studio and the validation ledger is on the evidence page.
const KSHANA_DEV = /^https?:\/\/(?:www\.)?kshana\.dev\/?(#[\w-]*)?$/i;
function rewriteKshanaDev(href, before) {
  const m = href.match(KSHANA_DEV);
  if (!m) return href;
  if (m[1] === "#ledger" || (!m[1] && /ledger\W*(<[^>]+>\W*)*\s*on\s*$/i.test(before))) return "../evidence.html#ledger";
  return `../${STUDIO_DIR}/index.html`;
}
// External pages their publisher has withdrawn (no archived copy): shown as text, not a dead link.
// Each document that cites one already says it is withdrawn.
const WITHDRAWN = new Set(["https://open-receiver.com/blogs/leo-pnt-signals-celeste/"]);
function rewriteImg(fromPath, src) {
  if (!src) return null;
  let target = null;
  if (RAW.test(src)) target = src.replace(RAW, "");
  else if (BLOB.test(src)) target = src.replace(BLOB, "").replace(/\?raw=true$/, "");
  else if (/^[a-z]+:/i.test(src) || src.startsWith("//")) return null; // other hosts: dropped (no third-party images)
  else target = resolveRel(fromPath, src);
  return copyAsset(target);
}

// ---------------------------------------------------------------- table pipes
// GitHub-flavoured Markdown splits a table row on every unescaped "|", even inside `code` or an
// absolute value like |B| or |Δv|², so such a row gets extra cells and the renderer (like GitHub)
// drops everything past the last column. Only for a row whose cell count is wrong: a "|" with a
// non-space character on either side is content and is escaped; the fix is kept only when the row
// then has exactly the table's column count. Each repair is reported, since the repository file
// renders broken on GitHub too.
const DELIM = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)+\|?\s*$/;
const cellCount = (line) => line.trim().replace(/^\|/, "").replace(/(?<!\\)\|$/, "").split(/(?<!\\)\|/).length;
const tablePipeRepairs = [];
function fixTablePipes(src, from) {
  const lines = src.split("\n");
  let fence = false, fixed = 0, unresolved = 0; const left = [];
  for (let i = 0; i < lines.length; i++) {
    if (/^\s{0,3}(```|~~~)/.test(lines[i])) fence = !fence;
    if (fence || i + 1 >= lines.length || !lines[i].includes("|") || !DELIM.test(lines[i + 1])) continue;
    const ncols = cellCount(lines[i + 1]);
    const repair = (k) => {
      if (cellCount(lines[k]) === ncols) return;
      const raw = lines[k];
      const lead = raw.match(/^\s*\|?/)[0], trail = raw.match(/\|?\s*$/)[0];
      const mid = raw.slice(lead.length, raw.length - trail.length);
      const esc2 = mid.replace(/(?<!\\)\|/g, (m, at) => (/\s/.test(mid[at - 1] || " ") && /\s/.test(mid[at + 1] || " ") ? "|" : "\\|"));
      let cand = lead + esc2 + trail;
      if (cellCount(cand) !== ncols) {
        // second try: a "|" inside [square brackets] is matrix notation ([I₃ | [p]ₓ | p]), also content
        let depth = 0;
        const esc3 = esc2.replace(/\\\||\[|\]|\|/g, (m) => { if (m === "[") depth++; else if (m === "]") depth = Math.max(0, depth - 1); else if (m === "|" && depth > 0) return "\\|"; return m; });
        cand = lead + esc3 + trail;
      }
      if (cellCount(cand) === ncols) { lines[k] = cand; fixed++; return; }
      const n = cellCount(raw);
      if (n > ncols) {
        // A row with more cells than columns (GitHub drops the surplus): the surplus cells are kept,
        // joined into the last cell with a literal " | ", so no words are lost.
        let seen = 0;
        lines[k] = lead + mid.replace(/(?<!\\)\|/g, (m) => (++seen >= ncols ? "\\|" : m)) + trail;
        unresolved++; left.push(`line ${k + 1}: ${n} cells for ${ncols} columns, surplus joined into the last cell`);
      } else { unresolved++; left.push(`line ${k + 1}: ${n} cells for ${ncols} columns`); }
    };
    repair(i);
    let k = i + 2;
    for (; k < lines.length && lines[k].includes("|") && lines[k].trim(); k++) repair(k);
    i = k - 1;
  }
  if (fixed || unresolved) {
    tablePipeRepairs.push({ file: from, fixed, unresolved, left });
    console.warn(`docs: ${from}: ${fixed} table row(s) with a "|" inside a cell repaired${unresolved ? `, ${unresolved} left as written (${left.join("; ")})` : ""} (unescaped in the repository file)`);
  }
  return lines.join("\n");
}

// A bare "<" in prose ("PL < error", "to <5 %"): marked 12 skips anything between a "<" and a later
// ">" when it pairs emphasis, so "*a* (x < y) or *c* (e > f" leaves "*c*" unrendered (GitHub renders
// it). Outside code, a "<" followed by a space, digit or "=" is written as &lt; (it reads the same).
// So is an upper-case placeholder in angle brackets ("IAU_<BODY>", "<NAME>"): HTML tag names are
// case-insensitive, so marked passed "<BODY>" through as a <body> tag and broke the page's markup.
function fixLessThan(src) {
  let fence = false;
  return src.split("\n").map((line) => {
    if (/^\s{0,3}(```|~~~)/.test(line)) { fence = !fence; return line; }
    if (fence || /^ {4,}|^\t/.test(line)) return line;
    return line.split(/(`+[^`]*`+)/).map((seg, i) => (i % 2 ? seg : seg.replace(/<(?=[\s\d=])/g, "&lt;").replace(/<(\/?[A-Z][A-Z0-9_]*)>/g, "&lt;$1&gt;"))).join("");
  }).join("\n");
}

// ---------------------------------------------------------------- renderer
// The label drawn in each code block's head bar (docs.css reads it with attr(data-lang)).
const LANG_LABEL = { bash: "shell", ini: "toml", js: "javascript", ts: "typescript", plaintext: "text", "": "text" };
function makeMarked() {
  const m = new Marked();
  m.use(gfmHeadingId());
  // Strikethrough only for ~~double tildes~~. The docs use a single ~ for "approximately"
  // ("~59% of the time ... **~0.83**"), which marked would otherwise pair up into <del>.
  m.use({
    tokenizer: {
      del(src) {
        const mm = /^~~(?=[^\s~])([\s\S]*?[^\s~])~~(?!~)/.exec(src);
        if (mm) return { type: "del", raw: mm[0], text: mm[1], tokens: this.lexer.inlineTokens(mm[1]) };
        return undefined; // no token and no fallback: a single ~ stays a literal character
      },
    },
  });
  m.use({
    renderer: {
      code(code, lang) {
        const l = (lang || "").split(/\s/)[0].toLowerCase().replace(/^toml$/, "ini").replace(/^sh$|^shell$|^console$/, "bash");
        let out;
        try { out = l && hljs.getLanguage(l) ? hljs.highlight(code, { language: l }).value : esc(code); } catch { out = esc(code); }
        return `<pre data-lang="${esc(LANG_LABEL[l] || l || "text")}"><code class="hljs${l ? " language-" + esc(l) : ""}">${out}</code></pre>\n`;
      },
      table(header, body) { return `<div class="tbl"><table><thead>${header}</thead><tbody>${body}</tbody></table></div>\n`; },
    },
  });
  return m;
}
function postProcess(html, fromPath) {
  // Links in rendered Markdown and raw HTML.
  html = html.replace(/<a\s+([^>]*?)href="([^"]*)"([^>]*)>([\s\S]*?)<\/a>/g, (m, pre, href, post, inner, at, all) => {
    const raw = href.replace(/&amp;/g, "&");
    if (WITHDRAWN.has(raw)) return `<span class="withdrawn" title="Withdrawn by its publisher; no archived copy">${inner}</span>`;
    if (KSHANA_DEV.test(raw)) return `<a ${pre}href="${esc(rewriteKshanaDev(raw, all.slice(Math.max(0, at - 160), at)))}"${post}>${inner}</a>`;
    const to = rewriteHref(fromPath, raw);
    if (to === "") return inner;
    if (to === null) return `<span class="not-pushed" title="In the repository branch these docs come from; not yet on GitHub">${inner}</span>`;
    return `<a ${pre}href="${esc(to)}"${post}>${inner}</a>`;
  });
  // <picture> with a dark and a light source: both images, each shown in its own site theme
  // (the site's theme is a data-theme toggle as well as the system setting, which <source media>
  // cannot follow). docs.css hides the one that does not match.
  html = html.replace(/<picture>([\s\S]*?)<\/picture>/g, (m, inner) => {
    const img = inner.match(/<img\b[^>]*>/);
    if (!img) return m;
    const dark = (inner.match(/<source[^>]*media="\(prefers-color-scheme:\s*dark\)"[^>]*srcset="([^"]+)"/) || [])[1];
    const light = (inner.match(/<source[^>]*media="\(prefers-color-scheme:\s*light\)"[^>]*srcset="([^"]+)"/) || [])[1];
    const withSrc = (tag, src, cls) => tag.replace(/src="[^"]*"/, `src="${src}"`).replace(/^<img\b/, `<img class="${cls}"`);
    if (!dark) return img[0];
    return withSrc(img[0], light || (img[0].match(/src="([^"]*)"/) || [])[1], "theme-light") + withSrc(img[0], dark, "theme-dark");
  });
  // Images: tracked ones embedded (see copyAsset); images from any other host dropped.
  html = html.replace(/<img\b([^>]*?)src="([^"]*)"([^>]*)>/g, (m, a, src, b) => {
    const local = rewriteImg(fromPath, src.replace(/&amp;/g, "&"));
    if (!local) { const alt = (m.match(/alt="([^"]*)"/) || [])[1]; return alt ? `<span class="img-alt">${alt}</span>` : ""; }
    return `<img${a}src="${local}"${b} loading="lazy">`;
  });
  // Passages naming an AI vendor: dropped, with a pointer to the source.
  const note = `<p class="omitted">A passage naming third-party AI clients is left out here; it is in <a href="${GH}${fromPath}">${esc(fromPath)}</a> on GitHub.</p>`;
  html = html.replace(/<(p|li|pre|tr|blockquote)\b[^>]*>[\s\S]*?<\/\1>/g, (m) => (VENDOR.test(m.replace(/<[^>]+>/g, " ")) || VENDOR.test(m) ? note : m));
  // A placeholder written as <target> outside a code span is raw HTML to the renderer and opens an element that
  // never closes: show it as the text it is. (Code spans are already escaped; only known HTML elements stay; a Rust generic such as From<TwoPartJd> is text too, whatever its case.)
  html = html.replace(/<(\/?)([A-Za-z][A-Za-z0-9-]*)>/g, (m, sl, n) => (KNOWN_TAGS.has(n.toLowerCase()) ? m : `&lt;${sl}${n}&gt;`));
  // A heading that names one (MCP-INSTALL.md has one per client) keeps its level and loses the name.
  html = html.replace(/<h([2-6])\b[^>]*>[\s\S]*?<\/h\1>/g, (m, lv) => (VENDOR.test(m.replace(/<[^>]+>/g, " ")) || VENDOR.test(m) ? `<h${lv}>Another client</h${lv}>` : m));
  html = html.replace(/(<p class="omitted">[\s\S]*?<\/p>)(\s*\1)+/g, "$1");
  // Scripts, styles and event handlers never come through from repository HTML.
  html = html.replace(/<script[\s\S]*?<\/script>/gi, "").replace(/<style[\s\S]*?<\/style>/gi, "").replace(/\son[a-z]+="[^"]*"/gi, "");
  // Heading anchors.
  html = html.replace(/<h([23]) id="([^"]+)">([\s\S]*?)<\/h\1>/g, (m, l, id, inner) => `<h${l} id="${id}">${inner}<a class="anchor" href="#${id}" aria-label="Link to this section">#</a></h${l}>`);
  return html;
}

// ---------------------------------------------------------------- Rust API reference (generated)
// Every top-level public function of src/api.rs with its signature and the first line of its
// doc comment, plus the interchange formats, read from the committed source.
function rustApiReference() {
  const src = gitShow("src/api.rs").toString("utf8").split("\n");
  const rows = [];
  for (let i = 0; i < src.length; i++) {
    const m = src[i].match(/^pub fn (\w+)\(([^)]*)\)\s*(->\s*[^{]+)?/);
    if (!m) continue;
    const doc = [];
    for (let j = i - 1; j >= 0 && /^\s*(\/\/\/|#\[)/.test(src[j]); j--) if (src[j].trim().startsWith("///")) doc.unshift(src[j].trim().replace(/^\/\/\/\s?/, ""));
    const first = doc.join(" ").split(/(?<=\.)\s/)[0] || "";
    rows.push(`<tr><td><code>kshana::api::${esc(m[1])}</code></td><td><code>(${esc(m[2].trim())})${esc((m[3] || "").trim().replace(/^->\s*/, " -> "))}</code></td><td>${esc(first.replace(/`/g, ""))}</td></tr>`);
  }
  const fmt = (p) => { const t = gitShow(p).toString("utf8").split("\n").filter((l) => l.startsWith("//!")).map((l) => l.replace(/^\/\/!\s?/, "")).join(" "); return t.split(/(?<=\.)\s|—/)[0].trim(); };
  return `<h3 id="api-reference">API reference, generated from src/api.rs<a class="anchor" href="#api-reference" aria-label="Link to this section">#</a></h3>
<p>Every public function of <code>kshana::api</code> at engine v${esc(cfg.version)}, read from the source at build time.</p>
<div class="tbl"><table><thead><tr><th>Function</th><th>Signature</th><th>What it does</th></tr></thead><tbody>${rows.join("")}</tbody></table></div>
<p>Interchange formats in the crate: ${esc(fmt("src/ccsds_tdm.rs"))} (<code>kshana::ccsds_tdm</code>); ${esc(fmt("src/interchange.rs"))} (<code>kshana::interchange</code>).</p>`;
}

// ---------------------------------------------------------------- generated reference pages
const STATUS = { v: "kind has validated ledger rows", m: "kind's ledger rows are Modelled", u: "not matched to a ledger module by name" };
function scenarioReference() {
  const byKind = new Map();
  for (const s of cfg.scenarios) { if (!byKind.has(s.kind)) byKind.set(s.kind, []); byKind.get(s.kind).push(s); }
  let md = `<h1 id="scenario-reference">Scenario reference</h1>\n<p>Every bundled scenario file (${cfg.scenarios.length}), grouped by kind. The description is the file's own header comment; the key parameters are its top-level settings. Status reads the verification matrix for the kind's module. <b>Open in ${STUDIO}</b> opens the recorded run.</p>\n`;
  for (const [kind, list] of [...byKind.entries()].sort()) {
    md += `<h2 id="kind-${esc(kind)}">${esc(kind)}</h2>\n<p>${escIC(list[0].kindDesc)}</p>\n`;
    for (const s of list) {
      const params = s.toml.split("\n").filter((l) => /^[a-z_]+\s*=\s*[^[{]/.test(l)).slice(0, 8).map((l) => l.replace(/\s*#.*$/, "").trim());
      md += `<div class="ref-card" id="${esc(s.file)}"><h3>${esc(s.file)}.toml</h3><div class="ref-meta"><span>kind ${esc(s.kind)}</span><span>${esc(STATUS[s.status])}${s.ledger ? ` · ${s.ledger[0]} of ${s.ledger[1]} rows validated` : ""}</span></div><p>${escIC(s.desc)}</p>`;
      if (params.length) md += `<pre data-lang="scenario · toml"><code class="hljs language-ini">${hljs.highlight(params.join("\n"), { language: "ini" }).value}</code></pre>`;
      md += `<p>${s.recorded ? `<a href="../${STUDIO_DIR}/index.html?scenario=${encodeURIComponent(s.file)}&amp;tab=overview">Open in ${STUDIO}</a> · ` : ""}${ghLink(`scenarios/${s.file}.toml`, "View on GitHub")}</p></div>\n`;
    }
  }
  if ((cfg.suites || []).length) {
    md += `<h2 id="study-suites">Study suites</h2>\n<p>A suite is a scenario file that lists other scenarios; <code>kshana --study</code> runs it into one stamped <code>study.json</code> and <code>study.html</code>. This is the free suite runner, not the paid custom study.</p>\n`;
    for (const s of cfg.suites) md += `<div class="ref-card" id="${esc(s.file)}"><h3>${esc(s.file)}.toml</h3><p>${escIC(s.desc)}</p><pre data-lang="shell"><code class="hljs language-bash">kshana --study scenarios/${esc(s.file)}.toml</code></pre><p>${ghLink(`scenarios/${s.file}.toml`, "View on GitHub")}</p></div>\n`;
  }
  return md;
}
function capabilityReference() {
  let md = `<h1 id="capability-reference">Capability reference</h1>\n<p>The ${cfg.cards.length} capability cards from <code>web/capabilities.json</code>, then all ${cfg.ledger.length} rows of the verification matrix.</p>\n<h2 id="cards">Capability cards</h2>\n`;
  for (const c of cfg.cards) md += `<div class="ref-card"><h3>${esc(c.name)}</h3><div class="ref-meta"><span>${esc(c.domain)}</span><span>${c.status === "validated" ? "Validated" : "Modelled"}</span>${c.proof ? `<span>${escIC(c.proof)}</span>` : ""}</div><p>${escIC(c.summary)}</p>${c.run ? `<p><a href="../${STUDIO_DIR}/index.html?scenario=${encodeURIComponent(c.run.replace(".toml", ""))}&amp;tab=overview">Run ${esc(c.run)} in ${STUDIO}</a></p>` : ""}</div>\n`;
  md += `<h2 id="matrix">Verification matrix</h2>\n<div class="tbl"><table><thead><tr><th>Requirement</th><th>Capability</th><th>Status</th><th>Oracle</th><th>Module</th></tr></thead><tbody>`;
  for (const r of cfg.ledger) md += `<tr><td>${escIC(r.req)}</td><td>${escIC(r.cap)}</td><td>${esc(r.status)}</td><td>${escIC(r.oracle)}</td><td><code>${esc(String(r.module).replace(/`/g, ""))}</code></td></tr>`;
  md += `</tbody></table></div>\n`;
  return md;
}

// ---------------------------------------------------------------- page assembly
const search = [];
const sideNav = (cur) => GROUPS.map(([g]) => {
  const items = pages.filter((p) => p.group === g);
  if (!items.length) return "";
  if (HIDDEN_GROUPS.has(g) && !items.some((p) => p.slug === cur)) return "";
  return `<div><h2>${esc(g)}</h2>${items.map((p) => `<a href="${p.slug}.html"${p.slug === cur ? ' aria-current="page"' : ""}>${esc(p.nav)}</a>`).join("")}</div>`;
}).join("");

function shellize(title, main, desc) {
  let out = cfg.shell;
  const navLinks = cfg.nav.map(([k, l, h]) => `      <a href="../${h}"${k === "docs" ? ' aria-current="page"' : ""}>${l}</a>`).join("\n");
  const scripts = `<script src="docs.js" defer></script>`;
  const rep = { "{{TITLE}}": esc(title), "{{DESC}}": esc(desc), "{{PAGE}}": "docs", "{{ROOT}}": "../", "{{NAV_LINKS}}": navLinks, "{{MAIN}}": main, "{{SCRIPTS}}": scripts, "{{HEAD_EXTRA}}": '<link rel="stylesheet" href="../css/docs.css">', "{{IMAGERY_CREDIT}}": cfg.imagery };
  for (const [k, v] of Object.entries(rep)) out = out.split(k).join(v);
  for (const [k, v] of Object.entries(cfg.scalars)) out = out.split(k).join(v);
  return out;
}

const marked = makeMarked();
// A one-line description for the landing cards: the first sentence of the page's first paragraph,
// cut at a word boundary. Generated pages carry their own.
const plain = (h) => h.replace(/<br\s*\/?>/gi, " ").replace(/<[^>]+>/g, "").replace(/&amp;/g, "&").replace(/&#39;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/\s+/g, " ").trim();
function firstSentence(html) {
  for (const m of html.matchAll(/<p>([\s\S]*?)<\/p>/g)) {
    const t = plain(m[1]);
    // skip badges, metadata lines ("Kind: orbit · Scenario: ...") and the vendor-omission note
    if (t.length < 24 || /^(\[|!|<)/.test(t) || /^(Version|License|Status|Kinds?|Scenarios?|Difficulty|Install)\b/i.test(t) || /omitted|left out here/.test(t)) continue;
    // a sentence ends at . ! ? followed by a space and a capital (so "tests.rs" or "v2.0" never ends one)
    const sent = t.split(/(?<=[.!?])\s+(?=[A-Z"“‘(])/);
    let s1 = sent[0].trim();
    for (let k = 1; s1.length < 48 && k < sent.length; k++) s1 += " " + sent[k].trim(); // a very short opener takes the next sentence too
    if (/:$/.test(s1)) continue; // a lead-in to a list says nothing on its own
    if (s1.length > 110) { s1 = s1.slice(0, 109); s1 = s1.slice(0, s1.lastIndexOf(" ")).replace(/[\s,;:(—-]+$/, "") + "…"; }
    return s1;
  }
  return "";
}
pages.forEach((p, i) => {
  let body, title, toc;
  if (p.path === "@scenario-reference" || p.path === "@capability-reference") {
    body = p.path === "@scenario-reference" ? scenarioReference() : capabilityReference();
    title = p.nav;
    toc = [...body.matchAll(/<h2 id="([^"]+)">([^<]+)<\/h2>/g)].map((m) => ({ level: 2, id: m[1], text: m[2] }));
    body = body.replace(/<h2 id="([^"]+)">([^<]+)<\/h2>/g, (m, id, t) => `<h2 id="${id}">${t}<a class="anchor" href="#${id}" aria-label="Link to this section">#</a></h2>`);
    p.desc = p.path === "@scenario-reference" ? `Every bundled scenario file (${cfg.scenarios.length}), grouped by kind, with its key settings.` : `The capability cards and all ${cfg.ledger.length} rows of the verification matrix.`;
  } else {
    // The verification matrix stays verbatim (its requirement names are the feature register's exact probes).
    const raw = gitShow(p.path).toString("utf8");
    const src = p.path === "docs/VERIFICATION-MATRIX.md" ? raw.replace(/\bfounder-gated\b/g, "pending review") : publicWording(raw);
    body = postProcess(marked.parse(fixLessThan(fixTablePipes(src, p.path))), p.path);
    if (p.path === "README.crates.md") {
      // Place the generated API reference inside the "Usage — library" section.
      const m = body.match(/<h2 id="usage--library">/);
      if (m) {
        const after = body.indexOf("<h2 ", m.index + 5);
        const at = after < 0 ? body.length : after;
        body = body.slice(0, at) + rustApiReference() + body.slice(at);
      }
    }
    if (p.slug === "glossary" && Array.isArray(cfg.siteGlossary)) {
      // The glossary lives in Docs only: abbreviations the site spells out that the repository's
      // glossary does not define are listed here, so every expansion on the site has one home.
      const have = (t) => new RegExp(`\\*\\*${t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`).test(src);
      const extra = cfg.siteGlossary.filter(([a]) => !have(a));
      if (extra.length) body += `<h2 id="on-this-site">Also used on kshana.dev</h2>\n<p>Abbreviations the website uses that are not in the repository's glossary above.</p>\n<dl class="site-gloss">${extra.map(([a, b]) => `<dt>${esc(a)}</dt><dd>${esc(b)}</dd>`).join("")}</dl>\n`;
    }
    p.desc = firstSentence(body);
    // web/README.md still calls the app by its old name in its opening line; the landing card uses
    // the README's own one-line description of the app instead (README.md, the Studio badge).
    if (p.slug === "studio") p.desc = `${STUDIO}: run the engine in your browser, no install.`;
    const heads = getHeadingList();
    const h1 = heads.find((h) => h.level === 1);
    title = h1 ? unent(h1.text.replace(/<[^>]+>/g, "")) : p.nav;
    toc = heads.filter((h) => (h.level === 2 || h.level === 3) && !VENDOR.test(h.text)).map((h) => ({ level: h.level, id: h.id, text: h.text.replace(/<[^>]+>/g, "") }));
  }
  for (const h of [{ id: "", text: title }].concat(toc)) search.push({ t: unent(h.text), h: `/docs/${p.slug}${h.id ? "#" + h.id : ""}`, k: "Docs", d: h.id ? title : p.group, g: p.group, x: h.id ? "" : (p.desc || "") });
  const sib = pages.filter((q) => q.hidden === p.hidden), si = sib.indexOf(p), prev = sib[si - 1], next = sib[si + 1];
  const edit = p.path.startsWith("@") ? `<span>Generated from the repository's scenario files and verification matrix at build time.</span>` : `${ghLink(p.path, "Edit on GitHub")}<span>Source: <code>${esc(p.path)}</code></span>`;
  const tocHtml = toc.length ? `<nav class="docs-toc" aria-label="On this page"><h2>On this page</h2>${toc.slice(0, 60).map((h) => `<a class="l${h.level}" href="#${h.id}">${esc(unent(h.text))}</a>`).join("")}</nav>` : `<div></div>`;
  const main = `<div class="docs docs-page">
  <aside class="docs-side" aria-label="Documentation">
    <div class="docs-search"><label class="vh" for="docsQ">Search the docs</label><input id="docsQ" type="search" placeholder="Search the docs" autocomplete="off" aria-keyshortcuts="/" role="combobox" aria-autocomplete="list" aria-expanded="false" aria-controls="docsR"><div class="docs-results" id="docsR" role="listbox" aria-label="Search results"></div><p class="vh" id="docsCount" aria-live="polite"></p></div>
    <details class="docs-nav-d" open><summary>All docs pages</summary><nav class="docs-nav" aria-label="Docs pages">${sideNav(p.slug)}</nav></details>
  </aside>
  <article class="docs-main">
    <div class="docs-meta"><span>${esc(p.group)}</span>${edit}</div>
    <div class="prose">${body}</div>
    <nav class="docs-pn" aria-label="Previous and next">${prev ? `<a class="prev" href="${prev.slug}.html"><small>Previous</small>${esc(prev.nav)}</a>` : ""}${next ? `<a class="next" href="${next.slug}.html"><small>Next</small>${esc(next.nav)}</a>` : ""}</nav>
  </article>
  ${tocHtml}
</div>`;
  fs.writeFileSync(path.join(OUT, `${p.slug}.html`), shellize(`${title} · Kshana docs`, main, `${title}. Kshana documentation, generated from the repository.`));
});

// Docs home.
const homeMain = `<div class="docs docs-home">
  <aside class="docs-side" aria-label="Documentation">
    <div class="docs-search"><label class="vh" for="docsQ">Search the docs</label><input id="docsQ" type="search" placeholder="Search the docs" autocomplete="off" aria-keyshortcuts="/" role="combobox" aria-autocomplete="list" aria-expanded="false" aria-controls="docsR"><div class="docs-results" id="docsR" role="listbox" aria-label="Search results"></div><p class="vh" id="docsCount" aria-live="polite"></p></div>
    <details class="docs-nav-d" open><summary>All docs pages</summary><nav class="docs-nav" aria-label="Docs pages">${sideNav("index")}</nav></details>
  </aside>
  <article class="docs-main"><div class="prose">
    <h1 id="docs">Kshana documentation</h1>
    <p>Everything here is rendered at build time from Markdown files tracked in the <a href="https://github.com/ashfordeOU/kshana">Kshana repository</a> (engine v${esc(cfg.version)}), plus two reference pages generated from the scenario files and the verification matrix. Nothing is written by hand for the site.</p>
    ${GROUPS.map(([g]) => { const items = pages.filter((p) => p.group === g); if (HIDDEN_GROUPS.has(g)) return items.length ? `<h2 id="${esc(g.toLowerCase().replace(/[^a-z]+/g, "-"))}">${esc(g)}</h2><p>${items.length} pages, one for each scenario kind, linked from <a href="kinds.html">Every scenario kind, by family</a>.</p>` : ""; return items.length ? `<h2 id="${esc(g.toLowerCase().replace(/[^a-z]+/g, "-"))}">${esc(g)}</h2><ul>${items.map((p) => `<li><a href="${p.slug}.html"><span>${esc(p.nav)}</span>${p.desc ? `<small>${esc(p.desc)}</small>` : ""}</a></li>`).join("")}</ul>` : ""; }).join("")}
    <h2 id="not-included">Not included</h2><p>Maintainer-process documents stay in the repository: ${EXCLUDED.map(esc).join("; ")}.</p>
  </div></article>
  <div></div>
</div>`;
fs.writeFileSync(path.join(OUT, "index.html"), shellize("Kshana docs", homeMain, "Kshana documentation, generated from the repository at build time."));
search.push({ t: "Kshana documentation", h: "/docs/", k: "Docs", d: "Docs home" });

// In-docs script (client side): search over the build-time index with keyboard use, a copy button
// on every code block, the section in view marked in "On this page", and the page list folded on
// phones. Written as a function here and serialised into docs.js, so it is linted with this file.
function docsClient() {
  var q = document.getElementById("docsQ"), r = document.getElementById("docsR"), cnt = document.getElementById("docsCount"), idx = null, act = -1;
  // ---- search
  function load() { if (idx) return Promise.resolve(idx); return fetch("search-docs.json").then(function (x) { return x.json(); }).then(function (d) { idx = d; return d; }).catch(function () { idx = []; return idx; }); }
  function items() { return r ? Array.prototype.slice.call(r.querySelectorAll("a")) : []; }
  function mark(i) {
    var a = items(); act = a.length ? (i + a.length) % a.length : -1;
    a.forEach(function (el, j) { el.setAttribute("aria-selected", j === act ? "true" : "false"); if (j === act) el.scrollIntoView({ block: "nearest" }); });
    if (act >= 0) q.setAttribute("aria-activedescendant", a[act].id); else q.removeAttribute("aria-activedescendant");
  }
  function closeR() { if (!r) return; r.replaceChildren(); act = -1; q.setAttribute("aria-expanded", "false"); q.removeAttribute("aria-activedescendant"); }
  function run() {
    var s = q.value.trim().toLowerCase();
    if (!s) { closeR(); if (cnt) cnt.textContent = ""; return; }
    load().then(function (d) {
      if (q.value.trim().toLowerCase() !== s) return;
      var words = s.split(/\s+/), scored = [];
      d.forEach(function (e) {
        var t = e.t.toLowerCase(), rest = ((e.d || "") + " " + (e.g || "") + " " + (e.x || "")).toLowerCase(), sc = 0;
        for (var i = 0; i < words.length; i++) {
          var w = words[i];
          if (t.indexOf(w) === 0) sc += 6; else if (t.indexOf(w) >= 0) sc += 4; else if (rest.indexOf(w) >= 0) sc += 1; else return;
        }
        if (t === s) sc += 10;
        if (e.h.indexOf("#") < 0) sc += 1; // a whole page above one of its sections
        scored.push([sc, e]);
      });
      scored.sort(function (a, b) { return b[0] - a[0]; });
      var out = scored.slice(0, 12).map(function (x) { return x[1]; });
      r.replaceChildren();
      out.forEach(function (e, i) {
        var a = document.createElement("a"); a.href = e.h; // a clean address from the site root (/docs/changelog#x) a.id = "docsR-" + i; a.setAttribute("role", "option"); a.setAttribute("aria-selected", "false"); a.textContent = e.t;
        var sm = document.createElement("small"); sm.textContent = e.d || ""; a.appendChild(sm); r.appendChild(a);
      });
      if (!out.length) { var p = document.createElement("small"); p.textContent = "No match in the docs."; r.appendChild(p); }
      q.setAttribute("aria-expanded", out.length ? "true" : "false"); act = -1;
      if (cnt) cnt.textContent = scored.length ? (scored.length > 12 ? "12 of " + scored.length + " results shown" : scored.length + (scored.length === 1 ? " result" : " results")) : "No results";
    });
  }
  if (q && r) {
    q.addEventListener("input", run);
    q.addEventListener("keydown", function (e) {
      if (e.key === "ArrowDown") { e.preventDefault(); if (!items().length) run(); else mark(act + 1); }
      else if (e.key === "ArrowUp") { e.preventDefault(); mark(act - 1); }
      else if (e.key === "Enter") { var a = items(); var t = a[act >= 0 ? act : 0]; if (t) { e.preventDefault(); location.href = t.href; } }
      else if (e.key === "Escape") { if (q.value || items().length) { e.stopPropagation(); q.value = ""; closeR(); if (cnt) cnt.textContent = ""; } }
    });
    document.addEventListener("keydown", function (e) {
      if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
      var el = document.activeElement, tag = el && el.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || (el && el.isContentEditable)) return;
      var pal = document.getElementById("palette"); if (pal && !pal.hidden) return;
      e.preventDefault(); q.focus(); q.select();
    });
    document.addEventListener("click", function (e) { if (!e.target.closest(".docs-search")) closeR(); });
  }
  // ---- copy button on each code block (the site's own copy helper: data-copy-from + toast)
  Array.prototype.forEach.call(document.querySelectorAll(".prose pre"), function (pre, i) {
    var code = pre.querySelector("code"); if (!code) return;
    if (!code.id) code.id = "docs-code-" + i;
    var b = document.createElement("button"); b.type = "button"; b.className = "docs-copy"; b.setAttribute("data-copy-from", code.id);
    b.setAttribute("aria-label", "Copy this code"); b.textContent = "Copy"; pre.appendChild(b);
  });
  // ---- "On this page": mark the section in view
  var toc = document.querySelector(".docs-toc");
  if (toc && "IntersectionObserver" in window) {
    var links = {}, heads = [];
    Array.prototype.forEach.call(toc.querySelectorAll("a[href^='#']"), function (a) {
      var id = decodeURIComponent(a.getAttribute("href").slice(1)), h = document.getElementById(id);
      if (h) { links[id] = a; heads.push(h); }
    });
    var vis = new Set(), cur = null;
    function set(id) {
      if (id === cur) return; cur = id;
      Object.keys(links).forEach(function (k) { if (k === id) links[k].setAttribute("aria-current", "location"); else links[k].removeAttribute("aria-current"); });
      var a = links[id]; if (a && toc.scrollHeight > toc.clientHeight) { var top = a.offsetTop - toc.clientHeight / 2; toc.scrollTop = Math.max(0, top); }
    }
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (en) { if (en.isIntersecting) vis.add(en.target); else vis.delete(en.target); });
      var first = null;
      if (vis.size) { heads.forEach(function (h) { if (!first && vis.has(h)) first = h; }); }
      else { heads.forEach(function (h) { if (h.getBoundingClientRect().top < 120) first = h; }); }
      if (first) set(first.id);
    }, { rootMargin: "-88px 0px -55% 0px" });
    heads.forEach(function (h) { io.observe(h); });
  }
  // ---- page list: open on wide screens, folded on phones (the reader opens it from the top)
  var d = document.querySelector(".docs-nav-d");
  if (d && window.matchMedia) {
    var mq = window.matchMedia("(max-width: 820px)");
    var sync = function () { if (mq.matches) d.removeAttribute("open"); else d.setAttribute("open", ""); };
    sync(); if (mq.addEventListener) mq.addEventListener("change", sync);
  }
}
fs.writeFileSync(path.join(OUT, "docs.js"), `(${docsClient.toString()})();\n`);
fs.writeFileSync(path.join(OUT, "search-docs.json"), JSON.stringify(search));
// The provenance manifest is a build record, not a page asset: written beside the config, not published.
fs.writeFileSync(cfg.manifest || path.join(OUT, "manifest.json"), JSON.stringify({ pages: pages.map((p) => ({ slug: p.slug, source: p.path, group: p.group })), excluded: EXCLUDED, assets: [...copied], tablePipeRepairs }, null, 1));
console.log(`docs: ${pages.length} pages + index, ${copied.size} assets, ${search.length} search entries`);
