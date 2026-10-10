// SPDX-License-Identifier: AGPL-3.0-only
// The tools hub: the DOM layer. Pure parts are in lib/tools35.mjs and lib/tools35-data.mjs; the engine runs through
// lib/engine-page.mjs (the Studio's WebAssembly engine in a worker). Files are read with File.arrayBuffer() in this
// page and nothing is sent anywhere.
import { getEngine } from "../lib/engine-page.mjs";
import { PAGES, FEATURES, CLI_CARDS, docsHref } from "../lib/tools35-data.mjs";
import { zip } from "../lib/zip.mjs";
import { packFilesJson, packProblems, parsePublicKey, verdictView, checkEpoch, benchEntries, scenarioKind, BENCH_KINDS, complianceRuns, MAX_COMPLIANCE_RUNS, toBase64 } from "../lib/tools35.mjs";

const $ = (id) => document.getElementById(id);
function el(tag, attrs, ...kids) {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) if (v != null) n.setAttribute(k, v);
  for (const k of kids.flat()) if (k != null) n.append(k.nodeType ? k : document.createTextNode(String(k)));
  return n;
}
const kb = (n) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} kB` : `${(n / 1048576).toFixed(1)} MB`);
const enc = new TextEncoder();
const MAX_FILE = 64 * 1024 * 1024; // the engine's own text limit

// ---------- download ----------
function download(name, data, mime) {
  const blob = new Blob([data], { type: mime || "application/octet-stream" });
  const a = el("a", { href: URL.createObjectURL(blob), download: name });
  document.body.append(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 4000);
}
const mimeOf = (n) => (/\.json$/.test(n) ? "application/json" : /\.csv$/.test(n) ? "text/csv" : /\.md$/.test(n) ? "text/markdown" : "text/plain");

// ---------- engine ----------
let engine = null;
async function useEngine(need) {
  const status = $("tl-engine");
  status.classList.remove("err");
  if (!engine) { status.textContent = "Starting the engine in this browser…"; engine = await getEngine(); }
  if (!engine.available) { status.textContent = `The engine could not start here (${engine.error}). Opening files works; running the tools needs WebAssembly.`; status.classList.add("err"); throw new Error("engine unavailable"); }
  status.textContent = `The engine (version ${engine.version}) is running in this browser. Nothing is uploaded.`;
  if (need && !engine.has(need)) {
    status.textContent = `This engine build (version ${engine.version}) has no ${need}: it predates the version this tool needs. The command-line equivalent is in the Command line tab.`;
    status.classList.add("err");
    throw new Error(`no ${need}`);
  }
  return engine;
}
const fail = (box, e) => box.replaceChildren(el("p", { class: "tl-verdict bad" }, el("b", {}, "That did not work"), el("p", {}, String(e && e.message ? e.message : e))));
async function readFiles(list) {
  const out = [];
  for (const f of list) {
    if (f.size > MAX_FILE) throw new Error(`${f.name} is larger than 64 MiB`);
    out.push({ name: f.name, path: f.webkitRelativePath || f.name, bytes: new Uint8Array(await f.arrayBuffer()) });
  }
  return out;
}
const asText = (f) => new TextDecoder().decode(f.bytes);

// ---------- tabs ----------
const TABS = [["evidence", "Evidence packs"], ["bench", "Test bench"], ["compliance", "Compliance"], ["osnma", "OSNMA"], ["cli", "Command line"], ["ai", "AI assistant"]];
function tabs() {
  const bar = $("tl-tabs");
  for (const [id, label] of TABS) {
    const b = el("button", { type: "button", role: "tab", class: "tl-tab", id: `t-${id}`, "aria-controls": `p-${id}`, "aria-selected": "false", tabindex: "-1" }, label);
    b.onclick = () => select(id, true);
    bar.append(b);
  }
  bar.addEventListener("keydown", (e) => {
    const ids = TABS.map((t) => t[0]), cur = ids.indexOf(location.hash.slice(1)) < 0 ? 0 : ids.indexOf(location.hash.slice(1));
    const next = e.key === "ArrowRight" ? (cur + 1) % ids.length : e.key === "ArrowLeft" ? (cur + ids.length - 1) % ids.length : e.key === "Home" ? 0 : e.key === "End" ? ids.length - 1 : -1;
    if (next >= 0) { e.preventDefault(); select(ids[next], true); $(`t-${ids[next]}`).focus(); }
  });
  addEventListener("hashchange", () => select(location.hash.slice(1) || "evidence", false));
  select(TABS.some((t) => t[0] === location.hash.slice(1)) ? location.hash.slice(1) : "evidence", false);
}
function select(id, push) {
  if (!TABS.some((t) => t[0] === id)) id = "evidence";
  for (const [t] of TABS) { const on = t === id; $(`t-${t}`).setAttribute("aria-selected", String(on)); $(`t-${t}`).tabIndex = on ? 0 : -1; $(`p-${t}`).hidden = !on; }
  if (push && location.hash.slice(1) !== id) history.replaceState(null, "", `#${id}`);
}

// ---------- coverage table ----------
function coverage() {
  const body = $("tl-cov-body");
  for (const f of FEATURES) {
    const studio = f.studio.length
      ? el("ul", { class: "tl-list" }, f.studio.map(([pid, what]) => el("li", {}, el("a", { href: PAGES[pid].href }, PAGES[pid].label), `: ${what}.`)))
      : el("span", { class: "tl-no" }, "Not in a browser. ", el("a", { href: "tools/#" + (f.card === "marine" || f.card === "telemetry" || f.card === "iq" ? "cli" : "cli") }, "How to run it"), `. ${f.why}`);
    body.append(el("tr", {}, el("td", {}, f.name), el("td", {}, studio), el("td", {}, el("code", {}, f.cli))));
  }
}

// ---------- command-line cards ----------
function cliCards() {
  const box = $("cli-cards");
  for (const c of CLI_CARDS) {
    const card = el("article", { class: "card tl-cli", id: `cli-${c.id}` }, el("h3", {}, c.title), el("p", {}, c.what), el("p", { class: "tl-why" }, el("b", {}, "Why not in a browser: "), c.why));
    for (const cmd of c.commands) {
      const row = el("div", { class: "tl-cmd-row" }, el("pre", { class: "tl-cmd" }, cmd));
      const copy = el("button", { type: "button", class: "tl-copy", "aria-label": `Copy: ${cmd}` }, "Copy");
      copy.onclick = async () => { try { await navigator.clipboard.writeText(cmd); copy.textContent = "Copied"; setTimeout(() => (copy.textContent = "Copy"), 1500); } catch { copy.textContent = "Select and copy"; } };
      row.append(copy); card.append(row);
    }
    for (const [label, file] of c.samples) {
      const d = el("details", { class: "tl-sample" }, el("summary", {}, `Sample output: ${label}`));
      const pre = el("pre", { class: "tl-pre" }, "Loading…");
      d.append(el("p", { class: "tl-why" }, "A real output of the command, recorded from the engine on the synthetic demo run. Not a measurement of anything."), pre);
      d.addEventListener("toggle", async () => { if (d.open && pre.dataset.done !== "1") { try { const r = await fetch("tools/samples/" + file); if (!r.ok) throw new Error(`HTTP ${r.status}`); pre.textContent = await r.text(); pre.dataset.done = "1"; } catch (e) { pre.textContent = `The sample did not load (${e.message}).`; } } });
      card.append(d);
    }
    card.append(el("p", {}, el("a", { href: docsHref(c.docs) }, "Read the documentation")));
    box.append(card);
  }
}

// ---------- evidence packs ----------
let pack = null; // { files: [{name, path, bytes}], label }
function setPack(files, label) {
  pack = { files, label };
  $("ev-sel").textContent = `${label}: ${files.map((f) => f.path.split("/").slice(-1)[0]).join(", ")} (${kb(files.reduce((n, f) => n + f.bytes.length, 0))})`;
  $("ev-out").replaceChildren();
}
async function fetchBytes(url) { const r = await fetch(url); if (!r.ok) throw new Error(`${url}: HTTP ${r.status}`); return new Uint8Array(await r.arrayBuffer()); }
async function openExamplePack(bad) {
  try {
    const idx = await (await fetch("tools/examples/evidence/index.json")).json();
    const files = [];
    for (const n of idx.files) files.push({ name: n, path: n, bytes: await fetchBytes("tools/examples/evidence/" + n) });
    if (bad) { const f = files.find((x) => x.name === idx.tamper); const b = f.bytes.slice(); b[Math.floor(b.length / 2)] ^= 1; f.bytes = b; }
    setPack(files, bad ? "Example pack with one byte of epochs.json changed" : "Synthetic example pack");
    $("ev-key").value = idx.public_key;
    $("ev-out").replaceChildren(el("p", { class: "tl-notice" }, bad ? "One byte of a file in the pack has been changed after signing. Verify it: the check names the file." : "A synthetic pack made by the engine's command line from the demo ferry log with a demonstration key. The key is filled in below; a real signer's key comes from the signer by another route."));
  } catch (e) { fail($("ev-out"), e); }
}
async function verifyPack() {
  const out = $("ev-out");
  try {
    if (!pack) throw new Error("Choose the pack's files first.");
    const k = parsePublicKey($("ev-key").value);
    if (k.error) throw new Error(k.error);
    const json = packFilesJson(pack.files);
    const probs = packProblems(json);
    if (probs.length) throw new Error(probs.join("; ") + ".");
    const logFile = $("ev-log").files[0];
    const log = logFile ? new TextDecoder().decode(new Uint8Array(await logFile.arrayBuffer())) : "";
    const e = await useEngine("evidence_verify");
    out.replaceChildren(el("p", { class: "tl-notice" }, "Verifying…"));
    const report = JSON.parse(await e.call("evidence_verify", JSON.stringify(json), k.key, log, $("ev-ts").checked));
    out.replaceChildren(...renderVerify(report, !!k.key));
  } catch (e) { fail(out, e); }
}
function renderVerify(r, pinned) {
  const v = verdictView(r);
  const nodes = [el("div", { class: `tl-verdict ${v.tone}`, role: "status" }, el("b", {}, v.title), el("p", {}, v.text))];
  if (r.signer_fingerprint) nodes.push(el("p", { class: "tl-sel" }, `Signer fingerprint: ${r.signer_fingerprint}${r.signer_pinned ? " (pinned)" : " (not pinned: the key the pack names itself)"}`));
  if (Array.isArray(r.failures) && r.failures.length) nodes.push(el("h4", {}, "What failed"), el("ul", { class: "tl-list" }, r.failures.map((f) => el("li", {}, typeof f === "string" ? f : JSON.stringify(f)))));
  if (Array.isArray(r.checks) && r.checks.length) {
    const t = el("table", { class: "tl-checks" }, el("thead", {}, el("tr", {}, ["Check", "Result", "Detail"].map((h) => el("th", { scope: "col" }, h)))));
    const tb = el("tbody", {});
    for (const c of r.checks) {
      const name = c.name || c.check || c.id || "", st = String(c.status || c.result || (c.ok === true ? "pass" : c.ok === false ? "fail" : "")), det = c.detail || c.message || c.note || "";
      tb.append(el("tr", {}, el("td", {}, name), el("td", {}, el("span", { class: `tl-st ${/pass/i.test(st) ? "pass" : /fail/i.test(st) ? "fail" : "skipped"}` }, st)), el("td", {}, det)));
    }
    t.append(tb); nodes.push(el("h4", {}, "Every check"), t);
  }
  if (Array.isArray(r.notes) && r.notes.length) nodes.push(el("h4", {}, "Notes"), el("ul", { class: "tl-list" }, r.notes.map((n) => el("li", {}, String(n)))));
  nodes.push(el("p", { class: "tl-notice" }, "An evidence pack is a technical record of what the engine computed from a stated log under stated settings. It is not a legal opinion, a finding of fact about any event or a certification. A pass says the record is unchanged."));
  return nodes;
}
async function attachTimestamp() {
  const out = $("ts-out");
  try {
    if (!pack) throw new Error("Choose the pack's files first (above).");
    const f = $("ts-file").files[0];
    if (!f) throw new Error("Choose the timestamp response (.tsr) first.");
    const e = await useEngine("evidence_attach_timestamp");
    const token = toBase64(new Uint8Array(await f.arrayBuffer()));
    const res = JSON.parse(await e.call("evidence_attach_timestamp", JSON.stringify(packFilesJson(pack.files)), token, $("ts-replace").checked));
    const entries = Object.entries(res.files).map(([name, v]) => ({ name, data: typeof v === "string" ? v : atob(v.base64) }));
    out.replaceChildren(el("div", { class: "tl-verdict ok" }, el("b", {}, "Token attached"), el("p", {}, "The updated pack is below. The timestamp authority's signature was NOT checked here: run openssl ts -verify.")), el("ul", { class: "tl-list" }, (res.notes || []).map((n) => el("li", {}, String(n)))));
    const z = el("button", { type: "button", class: "tl-btn" }, "Download the updated pack (zip)");
    z.onclick = () => download("evidence-pack-with-timestamp.zip", zip(Object.entries(res.files).map(([name, v]) => ({ name, data: typeof v === "string" ? v : Uint8Array.from(atob(v.base64), (c) => c.charCodeAt(0)) }))), "application/zip");
    out.append(z);
  } catch (e) { fail(out, e); }
}

function wireEvidence() {
  $("ev-files").addEventListener("change", async (e) => { try { setPack(await readFiles([...e.target.files]), "Chosen"); } catch (x) { fail($("ev-out"), x); } });
  $("ev-demo").onclick = () => openExamplePack(false);
  $("ev-demo-bad").onclick = () => openExamplePack(true);
  $("ev-run").onclick = verifyPack;
  $("ts-run").onclick = attachTimestamp;
}

// (bench, compliance, osnma, ai: added below as each tab is built)
const later = [];
export function register(fn) { later.push(fn); }

function init() {
  tabs(); coverage(); cliCards(); wireEvidence();
  for (const fn of later) fn({ $, el, kb, enc, download, mimeOf, useEngine, fail, readFiles, asText, fetchBytes });
}
init();
