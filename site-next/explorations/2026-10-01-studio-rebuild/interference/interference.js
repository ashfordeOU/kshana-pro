// SPDX-License-Identifier: AGPL-3.0-only
// Interference map page: the DOM layer. Everything it knows about the files is in lib/interference.mjs
// and lib/geojson-layer.mjs. Files are read with File.text() in this page; nothing is sent anywhere.
import { parseMapFile, groupBySource, statusCounts, statusInfo, STATUSES, KIND_LABEL, cellRows, cellBounds, cellAt, describeMap, cellsForTable, showValue, fmtLat, fmtLon } from "../lib/interference.mjs";
import { makeView, pathOf } from "../lib/geojson-layer.mjs";

const $ = (id) => document.getElementById(id);
const NS = "http://www.w3.org/2000/svg";
const MAX_MAPS = 12;

function el(tag, attrs, ...kids) {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) if (v != null) n.setAttribute(k, v);
  for (const k of kids.flat()) if (k != null) n.append(k.nodeType ? k : document.createTextNode(String(k)));
  return n;
}
function sv(tag, attrs, ...kids) {
  const n = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs || {})) if (v != null) n.setAttribute(k, v);
  for (const k of kids.flat()) if (k != null) n.append(k.nodeType ? k : document.createTextNode(String(k)));
  return n;
}

let maps = []; // parsed map files, each its own layer
let uid = 0;
const views = new Map(); // map.name+uid -> card state

function setStatus(msg, err) { const s = $("im-status"); s.textContent = msg; s.classList.toggle("err", !!err); }
function setNotes(list) { $("im-notes").replaceChildren(...list.map((t) => el("li", {}, t))); }

async function openTexts(items, label) {
  const notes = [], added = [];
  for (const it of items) {
    if (maps.length + added.length >= MAX_MAPS) { notes.push(`${it.name}: not opened, ${MAX_MAPS} maps are already open. Close some first.`); continue; }
    const r = parseMapFile(it.text, it.name);
    if (r.error) { notes.push(r.error); continue; }
    r.map.uid = ++uid;
    added.push(r.map);
  }
  maps.push(...added);
  render();
  $("im-clear").hidden = maps.length === 0;
  if (added.length) setStatus(label || `Opened ${added.length} file${added.length > 1 ? "s" : ""}; ${maps.length} map${maps.length > 1 ? "s" : ""} open.`);
  else setStatus(notes.length ? "No file could be opened." : "Nothing opened.", true);
  setNotes(notes);
}
async function openFiles(files) {
  if (!files || !files.length) return;
  setStatus("Reading…");
  const items = [];
  for (const f of files) items.push({ name: f.name, text: await f.text() });
  await openTexts(items);
}
async function openDemo() {
  setStatus("Opening the synthetic demo…");
  try {
    const names = ["adsb-2026-03-01.geojson", "adsb-2026-03-02.geojson", "ais-2026-03-01.geojson"];
    const items = [];
    for (const n of names) { const r = await fetch("interference/demo/" + n); if (!r.ok) throw new Error(`${n} did not load`); items.push({ name: n, text: await r.text() }); }
    await openTexts(items, "Synthetic demo: three made-up files on a made-up region (two ADS-B days, one of them confounded, and one AIS day). No real aircraft, vessel or day is behind them.");
  } catch (e) { setStatus(`The demo could not be opened: ${e.message}.`, true); }
}

// ---------- drawing ----------

function render() {
  const box = $("im-maps");
  box.replaceChildren();
  views.clear();
  for (const g of groupBySource(maps)) {
    const sec = el("section", { class: "im-group", "aria-labelledby": `im-g-${g.kind}` });
    sec.append(el("h2", { id: `im-g-${g.kind}`, class: "h3 im-gh" }, `${KIND_LABEL[g.kind]} maps`,
      el("small", {}, g.kind === "adsb" ? " cells where aircraft reported low navigation accuracy" : " cells where vessels reported implausible positions or motion")));
    for (const m of g.maps) sec.append(card(m));
    box.append(sec);
  }
  if (groupBySource(maps).length > 1) box.prepend(el("p", { class: "im-sep", role: "note" }, "ADS-B and AIS are shown separately and are never combined: each file keeps its own licence."));
}

function card(m) {
  const st = { m, sel: -1, hidden: new Set(), svg: null, view: null, w: 0 };
  views.set(m.uid, st);
  const id = `im${m.uid}`;
  const art = el("article", { class: "card im-card", "data-kind": m.kind, "aria-labelledby": `${id}-h` });
  const close = el("button", { type: "button", class: "im-x", "aria-label": `Close ${m.name}` }, "Close");
  close.onclick = () => { maps = maps.filter((x) => x.uid !== m.uid); render(); $("im-clear").hidden = maps.length === 0; setStatus(maps.length ? `${maps.length} map${maps.length > 1 ? "s" : ""} open.` : "Nothing open yet."); };
  art.append(el("div", { class: "im-head" },
    el("span", { class: `im-chip k-${m.kind}` }, KIND_LABEL[m.kind]),
    el("h3", { id: `${id}-h` }, `${m.date || "unknown date"}`, el("small", {}, m.licence.name ? ` · ${m.licence.name}` : "")),
    el("span", { class: "im-file-name" }, m.name), close));
  if (m.warnings.length) art.append(el("ul", { class: "im-warn", role: "note" }, m.warnings.map((w) => el("li", {}, w))));
  art.append(licence(m));
  // map + detail
  const mapBox = el("div", { class: "im-mapbox" });
  const svg = sv("svg", { class: "im-svg", tabindex: "0", role: "application", "aria-label": `${describeMap(m)} Arrow keys move between drawn cells.`, "aria-describedby": `${id}-sel` });
  mapBox.append(svg);
  st.svg = svg;
  const side = el("div", { class: "im-side" });
  side.append(legend(m, st, id));
  const sel = el("div", { class: "im-sel", id: `${id}-sel`, "aria-live": "polite" }, el("p", { class: "im-none" }, "Click a cell, or focus the map and use the arrow keys, to see its figures."));
  st.selBox = sel;
  side.append(sel);
  art.append(el("div", { class: "im-body" }, mapBox, side));
  art.append(dayFigures(m));
  art.append(method(m));
  art.append(table(m, st));
  // draw once laid out
  requestAnimationFrame(() => draw(st));
  svg.addEventListener("pointerdown", (ev) => pick(st, ev));
  svg.addEventListener("pointermove", (ev) => { if (ev.pointerType === "mouse") hover(st, ev); });
  svg.addEventListener("keydown", (ev) => key(st, ev));
  return art;
}

function licence(m) {
  const L = m.licence;
  const box = el("section", { class: "im-licence", "aria-label": `Data licence of ${m.name}` });
  box.append(el("h4", {}, "Data licence and attribution"));
  const dl = el("dl", {});
  const row = (k, v) => { if (v) dl.append(el("dt", {}, k), el("dd", {}, v)); };
  row("Licence", L.licence || "(none stated in this file)");
  row("Licence text at", L.licenceUrl);
  row("Attribution", L.attribution || "(none stated in this file)");
  row("Dataset", [L.name, L.dataset && `(${L.dataset})`].filter(Boolean).join(" "));
  box.append(dl);
  if (L.coverage.length) box.append(el("p", { class: "im-sub" }, "Coverage limits of this source"), el("ul", {}, L.coverage.map((c) => el("li", {}, c))));
  if (L.notice) box.append(el("p", { class: "im-notice" }, L.notice));
  return box;
}

function legend(m, st, id) {
  const counts = statusCounts(m);
  const ul = el("ul", { class: "im-leg", "aria-label": "Cell statuses; untick one to fade those cells" });
  for (const s of STATUSES[m.kind]) {
    const cb = el("input", { type: "checkbox", checked: "", id: `${id}-s-${s.id}` });
    cb.onchange = () => { if (cb.checked) st.hidden.delete(s.id); else st.hidden.add(s.id); draw(st); };
    ul.append(el("li", {}, el("label", { for: `${id}-s-${s.id}`, title: s.text }, cb, swatch(s.tone), el("span", { class: "im-lt" }, el("b", {}, s.label), ` ${counts[s.id]}`), el("span", { class: "im-ld" }, s.text))));
  }
  ul.append(el("li", { class: "im-nobs" }, el("label", {}, swatch("blank"), el("span", { class: "im-lt" }, el("b", {}, "Not drawn"), " not observed"), el("span", { class: "im-ld" }, "No cell here: too few aircraft or vessels were observed. This is not evidence that the area was clear."))));
  return ul;
}
function swatch(tone) { return el("i", { class: `im-sw t-${tone}`, "aria-hidden": "true" }); }

function dayFigures(m) {
  const d = m.day, rows = [];
  const lab = (k) => k.replace(/_/g, " ");
  for (const [k, v] of Object.entries(d)) rows.push([lab(k), typeof v === "number" && /share/.test(k) ? `${Math.round(v * 1000) / 10}%` : v === null ? "not evaluated" : showValue(v)]);
  const box = el("section", { class: "im-day", "aria-label": "Figures for the day" }, el("h4", {}, "The day"));
  if (!rows.length) { box.append(el("p", { class: "im-none" }, "This file states no day-level figures.")); return box; }
  box.append(el("dl", { class: "im-kv" }, rows.flatMap(([k, v]) => [el("dt", {}, k), el("dd", {}, v)])));
  return box;
}

function method(m) {
  const M = m.method, box = el("details", { class: "im-method", open: "" }, el("summary", {}, "Method and caveats"));
  if (M.id) box.append(el("p", { class: "im-mid" }, el("code", {}, M.id), m.version ? ` · written by Kshana ${m.version}` : ""));
  if (M.summary) box.append(el("p", {}, M.summary));
  if (Array.isArray(M.caveats) && M.caveats.length) box.append(el("h4", {}, "Caveats"), el("ul", { class: "im-cav" }, M.caveats.map((c) => el("li", {}, c))));
  if (Array.isArray(M.guards) && M.guards.length) box.append(el("h4", {}, "Guards against other causes"), el("ul", {}, M.guards.map((c) => el("li", {}, c))));
  if (M.parameters && typeof M.parameters === "object") box.append(el("h4", {}, "Parameters, fixed before any data was read"), el("dl", { class: "im-kv" }, Object.entries(M.parameters).flatMap(([k, v]) => [el("dt", {}, k.replace(/_/g, " ")), el("dd", {}, showValue(v))])));
  if (M.input_stats && typeof M.input_stats === "object") box.append(el("h4", {}, "Input"), el("dl", { class: "im-kv" }, Object.entries(M.input_stats).flatMap(([k, v]) => [el("dt", {}, k.replace(/_/g, " ")), el("dd", {}, showValue(v))])));
  const g = m.grid;
  if (g && g.cell_deg) box.append(el("p", { class: "im-sub" }, `Grid: square cells of ${g.cell_deg}° in latitude and longitude (${g.type || "fixed"}). East-west width shrinks with latitude.`));
  return box;
}

function table(m, st) {
  const box = el("details", { class: "im-tabled" }, el("summary", {}, `Every drawn cell as a table (${m.cells.length})`));
  const rows = cellsForTable(m);
  const head = m.kind === "adsb" ? ["Cell area", "Status", "Observed", "Sampled", "Affected", "Share"] : ["Cell area", "Status", "Observed", "Detectors"];
  const t = el("table", { class: "tbl" }, el("thead", {}, el("tr", {}, head.map((h) => el("th", { scope: "col" }, h)))));
  const tb = el("tbody", {});
  for (const { c, idx } of rows) {
    const b = cellBounds(c), p = c.props;
    const area = el("button", { type: "button", class: "im-rowbtn" }, `${fmtLat(b.south)}–${fmtLat(b.north)}, ${fmtLon(b.west)}–${fmtLon(b.east)}`);
    area.onclick = () => { select(st, idx); st.svg.scrollIntoView?.({ block: "nearest" }); };
    const cells = m.kind === "adsb"
      ? [p.aircraft_observed, p.aircraft_sampled, p.aircraft_affected, typeof p.affected_share === "number" ? `${Math.round(p.affected_share * 1000) / 10}%` : ""]
      : [p.vessels_observed, Array.isArray(p.detectors) && p.detectors.length ? p.detectors.join(", ").replace(/_/g, " ") : "none"];
    tb.append(el("tr", {}, el("td", {}, area), el("td", {}, statusInfo(m.kind, c.status).label), cells.map((v) => el("td", {}, v == null ? "" : String(v)))));
  }
  t.append(tb);
  box.append(el("div", { class: "table-scroll" }, t));
  return box;
}

// ---------- the map ----------

function draw(st) {
  const { m, svg } = st;
  const w = Math.max(280, Math.round(svg.parentElement.clientWidth) || 640), h = Math.round(w * (w < 520 ? 0.95 : 0.72));
  st.w = w;
  svg.setAttribute("viewBox", `0 0 ${w} ${h}`);
  svg.replaceChildren();
  if (!m.bbox) { svg.append(sv("text", { x: 20, y: 30 }, "This file has no cells to draw.")); return; }
  // pad the box a little so edge cells are not on the frame
  const cd = Number(m.grid.cell_deg) || 0.5;
  const bb = { west: m.bbox.west - cd * 0.25, east: m.bbox.east + cd * 0.25, south: m.bbox.south - cd * 0.25, north: m.bbox.north + cd * 0.25 };
  const view = makeView(bb, w, h, 28);
  st.view = view;
  const defs = sv("defs", {},
    sv("pattern", { id: `h-none-${m.uid}`, width: 6, height: 6, patternUnits: "userSpaceOnUse", patternTransform: "rotate(45)" }, sv("rect", { width: 6, height: 6, fill: "var(--im-none-bg)" }), sv("line", { x1: 0, y1: 0, x2: 0, y2: 6, stroke: "var(--im-none)", "stroke-width": 2 })),
    sv("pattern", { id: `h-hold-${m.uid}`, width: 6, height: 6, patternUnits: "userSpaceOnUse", patternTransform: "rotate(-45)" }, sv("rect", { width: 6, height: 6, fill: "var(--im-hold-bg)" }), sv("line", { x1: 0, y1: 0, x2: 0, y2: 6, stroke: "var(--im-hold)", "stroke-width": 2 })));
  svg.append(defs);
  // graticule
  const step = [0.5, 1, 2, 5, 10].find((s) => (bb.north - bb.south) / s <= 8) || 10;
  for (let lat = Math.ceil(bb.south / step) * step; lat <= bb.north; lat += step) { const y = view.project([bb.west, lat])[1]; svg.append(sv("line", { x1: 0, x2: w, y1: y, y2: y, class: "grat" }), sv("text", { x: 3, y: y - 3 }, fmtLat(lat))); }
  for (let lon = Math.ceil(bb.west / step) * step; lon <= bb.east; lon += step) { const x = view.project([lon, bb.north])[0]; svg.append(sv("line", { x1: x, x2: x, y1: 0, y2: h, class: "grat" }), sv("text", { x: x + 3, y: h - 4 }, fmtLon(lon))); }
  // cells, one path per status so drawing stays cheap
  const toneOf = (id) => statusInfo(m.kind, id).tone;
  for (const s of [...STATUSES[m.kind].map((x) => x.id), ...new Set(m.cells.map((c) => c.status).filter((x) => !STATUSES[m.kind].some((y) => y.id === x)))]) {
    const cells = m.cells.filter((c) => c.status === s);
    if (!cells.length) continue;
    const d = cells.map((c) => pathOf(c.geometry, view.project)).join("");
    svg.append(sv("path", { d, class: `cell t-${toneOf(s)}`, fill: toneOf(s) === "none" ? `url(#h-none-${m.uid})` : toneOf(s) === "hold" ? `url(#h-hold-${m.uid})` : null, opacity: st.hidden.has(s) ? 0.1 : 1 }));
  }
  // scale bar: 100 km at the map's mid latitude, or the nearest nice length
  const pxPerKm = view.k / 111.32; // equal scale on both axes: view.k is pixels per degree of latitude
  const km = [10, 20, 50, 100, 200, 500, 1000].filter((k) => k * pxPerKm < w * 0.35).pop() || 10;
  const px = km * pxPerKm;
  svg.append(sv("path", { d: `M16 ${h - 22} h${px.toFixed(1)} m0 -4 v8 m0 -4 M16 ${h - 26} v8`, class: "bar" }), sv("text", { x: 16, y: h - 30, class: "barlbl" }, `${km} km`),
    sv("path", { d: `M${w - 22} 56 v-30 m-5 9 l5 -9 l5 9`, class: "bar" }), sv("text", { x: w - 28, y: 70, class: "barlbl" }, "N"));
  st.selG = sv("g"); svg.append(st.selG);
  if (st.sel >= 0) markSel(st);
}

function lonLat(st, ev) {
  const r = st.svg.getBoundingClientRect();
  return st.view.invert([((ev.clientX - r.left) / r.width) * st.w, ((ev.clientY - r.top) / r.height) * (st.svg.viewBox.baseVal.height)]);
}
function pick(st, ev) {
  if (!st.view) return;
  const [lon, lat] = lonLat(st, ev);
  const i = cellAt(st.m, lon, lat);
  if (i >= 0) select(st, i);
  else { st.sel = -1; markSel(st); st.selBox.replaceChildren(el("p", { class: "im-none" }, `No cell at ${fmtLat(lat)}, ${fmtLon(lon)}: not observed in this file. That is not evidence that the area was clear.`)); }
}
function hover(st, ev) {
  if (!st.view) return;
  const [lon, lat] = lonLat(st, ev);
  const i = cellAt(st.m, lon, lat);
  st.svg.style.cursor = i >= 0 ? "pointer" : "crosshair";
}
function select(st, i) {
  st.sel = i;
  markSel(st);
  const c = st.m.cells[i], info = statusInfo(st.m.kind, c.status);
  st.selBox.replaceChildren(el("h4", {}, info.label), el("p", { class: "im-stext" }, info.text),
    el("dl", { class: "im-kv" }, cellRows(st.m, c).flatMap(([k, v]) => [el("dt", {}, k), el("dd", {}, v == null ? "" : String(v))])));
}
function markSel(st) {
  if (!st.selG) return;
  st.selG.replaceChildren();
  if (st.sel < 0) return;
  st.selG.append(sv("path", { d: pathOf(st.m.cells[st.sel].geometry, st.view.project), class: "selcell" }));
}
// Arrow keys: the nearest drawn cell in that direction.
function key(st, ev) {
  const dirs = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
  const d = dirs[ev.key];
  if (!d) return;
  ev.preventDefault();
  const cells = st.m.cells;
  if (st.sel < 0) { select(st, cellsForTable(st.m)[0].idx); return; }
  const ctr = (c) => { const b = cellBounds(c); return [(b.west + b.east) / 2, (b.south + b.north) / 2]; };
  const [x0, y0] = ctr(cells[st.sel]);
  let best = -1, bs = Infinity;
  cells.forEach((c, i) => {
    if (i === st.sel) return;
    const [x, y] = ctr(c), dx = x - x0, dy = y - y0, along = dx * d[0] + dy * d[1], across = Math.abs(dx * d[1] + dy * d[0]);
    if (along <= 1e-9 || across > along * 1.0 + 1e-9) return;
    const score = along + across * 2;
    if (score < bs) { bs = score; best = i; }
  });
  if (best >= 0) select(st, best);
}

function wire() {
  $("im-files").addEventListener("change", (e) => { openFiles([...e.target.files]); e.target.value = ""; });
  $("im-demo").addEventListener("click", openDemo);
  $("im-clear").addEventListener("click", () => { maps = []; render(); $("im-clear").hidden = true; setStatus("Nothing open yet."); setNotes([]); });
  const drop = $("im-drop");
  for (const ev of ["dragenter", "dragover"]) drop.addEventListener(ev, (e) => { e.preventDefault(); drop.classList.add("over"); });
  for (const ev of ["dragleave", "drop"]) drop.addEventListener(ev, (e) => { e.preventDefault(); drop.classList.remove("over"); });
  drop.addEventListener("drop", (e) => openFiles([...(e.dataTransfer?.files || [])]));
  $("btn-theme").addEventListener("click", () => {
    const root = document.documentElement;
    const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
    root.dataset.theme = dark ? "light" : "dark";
    try { localStorage.setItem("kshana-theme", root.dataset.theme); } catch (e) { /* storage blocked */ }
  });
  let rt = 0;
  addEventListener("resize", () => { clearTimeout(rt); rt = setTimeout(() => { for (const st of views.values()) { const w = Math.round(st.svg.parentElement.clientWidth); if (w !== st.w) draw(st); } }, 120); });
  if (new URLSearchParams(location.search).get("demo") === "1") openDemo();
}
wire();
