// SPDX-License-Identifier: AGPL-3.0-only
// Geometry view: the DOM layer. The state and its address are lib/geomstate.mjs, the skyline editing is
// lib/skyline.mjs, the drawing helpers are lib/skyplot.mjs, and every number shown comes from the engine
// through lib/geometry-adapter.mjs. Nothing here fetches anything or computes a navigation quantity.
import { parseState, serializeState, normaliseState, PLACES, SKYLINES } from "../lib/geomstate.mjs";
import { addPoint, movePoint, keyEdit, removePoint, describe, outline } from "../lib/skyline.mjs";
import { project, unproject, outlinePath, markerPath, niceExtent, ellipseAttrs, fmtDop, styleFor } from "../lib/skyplot.mjs";
import { createGeometryAdapter, SKYLINE_LABELS } from "../lib/geometry-adapter.mjs";
import { getEngine } from "../lib/engine-page.mjs";

const $ = (id) => document.getElementById(id);
const NS = "http://www.w3.org/2000/svg";
const CX = 200, CY = 200, R = 172;

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
const f1 = (x) => (Math.round(x * 10) / 10).toString();

// ---------------------------------------------------------------- state
const read = parseState(location.search);
let state = read.state;
let skyline = state.pts.slice();     // the outline drawn (a preset's points, or the person's own)
let selected = -1;                   // the focused skyline vertex
let adapter = null;
let result = null;
let ticket = 0;

function setNotes(list) { $("gm-url-notes").replaceChildren(...list.map((t) => el("li", {}, t))); }
setNotes(read.notes);

function writeAddress() {
  const q = serializeState(state);
  try { history.replaceState(null, "", location.pathname + q); } catch { /* a sandboxed frame */ }
}

// ---------------------------------------------------------------- controls
function fillControls() {
  const place = $("gm-place");
  if (!place.options.length) {
    for (const [k, p] of Object.entries(PLACES)) place.append(el("option", { value: k }, p.label));
    place.append(el("option", { value: "custom" }, "Custom place"));
  }
  place.value = state.place;
  $("gm-lat").value = state.lat; $("gm-lon").value = state.lon; $("gm-hgt").value = state.h;
  $("gm-time").value = state.t.slice(0, 19);
  $("gm-mask").value = state.mask; $("gm-mask-out").textContent = `${state.mask}°`;
  const sky = $("gm-sky");
  if (!sky.options.length) for (const k of SKYLINES) sky.append(el("option", { value: k }, SKYLINE_LABELS[k]));
  sky.value = state.sky;
  $("gm-sigma").value = state.sigma;
  $("gm-n").value = String(state.n);
  if ($("gm-n").value !== String(state.n)) { $("gm-n").append(el("option", {}, String(state.n))); $("gm-n").value = String(state.n); }
  $("gm-seed").value = state.seed;
  // Sync the boxes in place (rebuilding them would lose the focus of the one just toggled).
  for (const i of $("gm-sys").querySelectorAll("input")) i.checked = state.sys.includes(i.value);
}

function fillSystems() {
  const box = $("gm-sys");
  const list = adapter ? adapter.systems() : [];
  box.replaceChildren(...list.map((s) => {
    const st = styleFor(s.id);
    const input = el("input", { type: "checkbox", value: s.id, id: `gm-sys-${s.id}` });
    input.checked = state.sys.includes(s.id);
    input.disabled = !s.available;
    if (!s.available) input.setAttribute("aria-describedby", "gm-sys-note");
    const mark = sv("svg", { class: "gm-mk", viewBox: "0 0 16 16", "aria-hidden": "true" }, sv("path", { d: markerPath(st.shape, 8, 8, 5), style: `fill:${st.color}` }));
    return el("label", { class: "gm-sysbox" + (s.available ? "" : " off") }, input, mark, st.label);
  }));
  const off = list.filter((s) => !s.available);
  $("gm-sys-note").textContent = off.length ? off.map((s) => `${styleFor(s.id).label}: ${s.why}`).join(" ") : "";
}

// The controls' values, checked by the same rules as the address.
function readControls() {
  const num = (id) => { const v = $(id).valueAsNumber; return Number.isFinite(v) ? v : NaN; };
  const t = $("gm-time").value;
  const next = {
    ...state,
    place: $("gm-place").value,
    lat: num("gm-lat"), lon: num("gm-lon"), h: num("gm-hgt"),
    t: /^\d{4}-\d\d-\d\dT\d\d:\d\d(:\d\d)?$/.test(t) ? (t.length === 16 ? t + ":00" : t) + "Z" : state.t,
    sys: [...$("gm-sys").querySelectorAll("input:checked")].map((i) => i.value),
    mask: num("gm-mask"), sky: $("gm-sky").value, sigma: num("gm-sigma"), n: Number($("gm-n").value), seed: num("gm-seed"),
  };
  for (const k of ["lat", "lon", "h", "mask", "sigma", "seed"]) if (!Number.isFinite(next[k])) next[k] = state[k];
  return next;
}

function commit(next, { recompute = true } = {}) {
  const custom = next.sky === "custom";
  const n = normaliseState({ ...next, pts: custom ? skyline : [] });
  const refused = ["lat", "lon", "h", "mask", "sigma", "seed", "n"].filter((k) => next[k] !== n[k] && Number.isFinite(next[k]));
  setNotes(refused.length ? [`Out of range, so kept at the last good value: ${refused.join(", ")}.`] : []);
  state = n;
  writeAddress();
  fillControls();
  if (recompute) schedule();
}

$("gm-form").addEventListener("change", async (ev) => {
  const t = ev.target;
  if (t.id === "gm-place") {
    const p = PLACES[t.value];
    if (p) { $("gm-lat").value = p.lat; $("gm-lon").value = p.lon; $("gm-hgt").value = p.h; }
  } else if (["gm-lat", "gm-lon"].includes(t.id)) {
    $("gm-place").value = "custom";
  }
  if (t.id === "gm-sky") { await loadSkyline(t.value); }
  commit(readControls());
});
$("gm-mask").addEventListener("input", () => { $("gm-mask-out").textContent = `${$("gm-mask").value}°`; drawPlot(); });
$("gm-form").addEventListener("submit", (ev) => ev.preventDefault());
for (const b of document.querySelectorAll("[data-step]")) b.addEventListener("click", () => {
  const ms = Date.parse(state.t) + Number(b.dataset.step) * 1000;
  const t = new Date(ms).toISOString().replace(".000Z", "Z");
  commit({ ...state, t });
});

async function loadSkyline(name) {
  if (name === "custom") return; // keep whatever is drawn
  try {
    const r = adapter ? await adapter.skyline(name) : { points: [], source: "" };
    skyline = r.points;
  } catch (e) { skyline = []; setNotes([`The skyline preset could not be loaded (${e && e.message ? e.message : e}).`]); }
  selected = -1;
}

// Any edit of the outline makes it the person's own ("custom").
function editSkyline(points, index) {
  skyline = points;
  selected = index;
  $("gm-sky").value = "custom";
  commit({ ...state, sky: "custom" });
  focusVertex();
}
$("gm-sky-add").addEventListener("click", () => {
  // The middle of the widest gap, at the height of its neighbours.
  let az = 0, el = 10;
  if (skyline.length) {
    let best = -1;
    skyline.forEach((p, i) => { const q = skyline[(i + 1) % skyline.length]; const gap = (((q.az - p.az) % 360) + 360) % 360 || 360; if (gap > best) { best = gap; az = p.az + gap / 2; el = (p.el + q.el) / 2; } });
  }
  const r = addPoint(skyline, az, el);
  if (r.index >= 0) editSkyline(r.points, r.index);
});
$("gm-sky-del").addEventListener("click", () => { if (selected >= 0) editSkyline(removePoint(skyline, selected), Math.min(selected, skyline.length - 2)); });
$("gm-sky-clear").addEventListener("click", () => { skyline = []; selected = -1; $("gm-sky").value = "none"; commit({ ...state, sky: "none" }); });

$("gm-copy").addEventListener("click", async () => {
  const url = location.origin + location.pathname + serializeState(state);
  try { await navigator.clipboard.writeText(url); $("gm-copy-out").textContent = "Link copied."; }
  catch { $("gm-copy-out").textContent = `Copy this link: ${url}`; }
});

$("btn-theme").addEventListener("click", () => {
  const root = document.documentElement;
  const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
  root.dataset.theme = dark ? "light" : "dark";
  try { localStorage.setItem("kshana-theme", root.dataset.theme); } catch { /* storage blocked */ }
});
// "What this shows" is on this page; with <base href="../"> a bare #about would leave it.
for (const a of document.querySelectorAll('a[href="geometry/#about"]')) a.addEventListener("click", (ev) => { ev.preventDefault(); $("about").scrollIntoView({ behavior: "smooth" }); $("gm-about-h").setAttribute("tabindex", "-1"); $("gm-about-h").focus({ preventScroll: true }); });

// ---------------------------------------------------------------- the skyplot
const plot = $("gm-plot");
function svgPoint(ev) {
  const m = plot.getScreenCTM();
  if (!m) return null;
  const p = new DOMPoint(ev.clientX, ev.clientY).matrixTransform(m.inverse());
  return { x: p.x, y: p.y };
}

function drawPlot() {
  const kids = [];
  const mask = Number($("gm-mask").value);
  kids.push(sv("circle", { cx: CX, cy: CY, r: R, class: "g-sky" }));
  // Skyline: the sky between the outline and the horizon, hatched.
  if (skyline.length) kids.push(sv("path", { d: outlinePath(outline(skyline, 2), CX, CY, R), class: "g-skyline", "fill-rule": "evenodd" }));
  // Elevation mask ring.
  if (mask > 0) kids.push(sv("circle", { cx: CX, cy: CY, r: ((90 - mask) / 90) * R, class: "g-mask" }));
  for (const e of [30, 60]) kids.push(sv("circle", { cx: CX, cy: CY, r: ((90 - e) / 90) * R, class: "g-ring" }));
  for (let a = 0; a < 360; a += 30) { const p = project(a, 0, CX, CY, R); kids.push(sv("line", { x1: CX, y1: CY, x2: p.x, y2: p.y, class: "g-ring" })); }
  for (const [a, t] of [[0, "N"], [90, "E"], [180, "S"], [270, "W"]]) { const p = project(a, -7, CX, CY, R + 13); kids.push(sv("text", { x: p.x, y: p.y + 4, class: "g-card", "text-anchor": "middle", "aria-hidden": "true" }, t)); }
  for (const e of [30, 60]) { const p = project(0, e, CX, CY, R); kids.push(sv("text", { x: p.x + 3, y: p.y - 2, class: "g-tick", "aria-hidden": "true" }, `${e}°`)); }

  // Satellites.
  const sats = result ? result.sats : [];
  const g = sv("g", { class: "g-sats" });
  for (const s of sats) {
    const st = styleFor(s.sys);
    const p = project(s.az, s.el, CX, CY, R);
    const state_ = s.blocked ? `hidden by the ${s.blocked}` : s.used ? "used in the fix" : "in view, not used";
    const mk = sv("path", { d: markerPath(st.shape, p.x, p.y, 6.5), class: `g-sat ${s.blocked ? "blocked" : s.used ? "used" : "unused"}`, style: `--c:${st.color}` }, sv("title", {}, `${st.label} ${s.id}: azimuth ${s.az}°, elevation ${s.el}°, ${state_}`));
    g.append(mk, sv("text", { x: p.x + 8.5, y: p.y + 3.5, class: "g-id", "aria-hidden": "true" }, String(s.id)));
  }
  kids.push(g);

  // Skyline vertices: focusable and draggable.
  skyline.forEach((q, i) => {
    const p = project(q.az, q.el, CX, CY, R);
    const h = sv("circle", { cx: p.x, cy: p.y, r: 7, class: "g-vtx" + (i === selected ? " sel" : ""), tabindex: "0", role: "button", "data-i": String(i), "aria-label": `Skyline point ${i + 1} of ${skyline.length}: azimuth ${q.az}°, elevation ${q.el}°`, "aria-describedby": "gm-sky-help" });
    kids.push(h);
  });
  plot.replaceChildren(...kids);
  $("gm-plot-desc").textContent = `${describe(skyline)} ${sats.length} satellites in the plot, ${sats.filter((s) => s.used).length} used in the fix.`;
  $("gm-sky-del").disabled = selected < 0;
}
function focusVertex() {
  if (selected < 0) return;
  const v = plot.querySelector(`[data-i="${selected}"]`);
  if (v) v.focus({ preventScroll: true });
}

let drag = null;
plot.addEventListener("pointerdown", (ev) => {
  const p = svgPoint(ev);
  if (!p) return;
  const t = ev.target;
  if (t.classList && t.classList.contains("g-vtx")) {
    selected = Number(t.dataset.i);
    drag = { id: ev.pointerId, moved: false };
    plot.setPointerCapture(ev.pointerId);
    ev.preventDefault();
    drawPlot(); focusVertex();
    return;
  }
  if (Math.hypot(p.x - CX, p.y - CY) <= R) {
    const q = unproject(p.x, p.y, CX, CY, R);
    const r = addPoint(skyline, q.az, q.el);
    if (r.index >= 0) editSkyline(r.points, r.index);
  }
});
plot.addEventListener("pointermove", (ev) => {
  if (!drag || ev.pointerId !== drag.id) return;
  const p = svgPoint(ev);
  if (!p) return;
  const q = unproject(p.x, p.y, CX, CY, R);
  skyline = movePoint(skyline, selected, q.az, q.el);
  drag.moved = true;
  drawPlot();
});
const endDrag = (ev) => {
  if (!drag || ev.pointerId !== drag.id) return;
  const moved = drag.moved;
  drag = null;
  if (moved) editSkyline(skyline, selected);
};
plot.addEventListener("pointerup", endDrag);
plot.addEventListener("pointercancel", endDrag);
plot.addEventListener("keydown", (ev) => {
  const t = ev.target;
  if (!t.classList || !t.classList.contains("g-vtx")) return;
  const i = Number(t.dataset.i);
  const r = keyEdit(skyline, i, ev.key, ev.shiftKey);
  if (!r) return;
  ev.preventDefault();
  if (ev.key === "Home" || ev.key === "End") { selected = r.index; drawPlot(); focusVertex(); return; }
  editSkyline(r.points, r.index);
  if (r.index < 0) $("gm-sky-add").focus();
});
plot.addEventListener("focusin", (ev) => { const t = ev.target; if (t.classList && t.classList.contains("g-vtx")) { selected = Number(t.dataset.i); $("gm-sky-del").disabled = false; for (const v of plot.querySelectorAll(".g-vtx")) v.classList.toggle("sel", v === t); } });

// ---------------------------------------------------------------- readouts
function drawLegend() {
  const shown = new Set((result ? result.sats : []).map((s) => s.sys));
  $("gm-legend").replaceChildren(...state.sys.filter((s) => shown.has(s)).map((s) => {
    const st = styleFor(s);
    const n = result.sats.filter((x) => x.sys === s);
    return el("li", {}, sv("svg", { class: "gm-mk", viewBox: "0 0 16 16", "aria-hidden": "true" }, sv("path", { d: markerPath(st.shape, 8, 8, 5), style: `fill:${st.color}` })), `${st.label}: ${n.filter((x) => x.used).length} used of ${n.length}`);
  }));
}

function drawDops() {
  const d = result ? result.dop : {};
  const digits = result ? result.dopDigits : 2;
  const rows = [["GDOP", "gdop", "geometric"], ["PDOP", "pdop", "position, 3-D"], ["HDOP", "hdop", "horizontal"], ["VDOP", "vdop", "vertical"], ["TDOP", "tdop", "time"]];
  $("gm-dops").replaceChildren(...rows.flatMap(([abbr, k, what]) => {
    const v = d && Number.isFinite(d[k]) ? d[k] : null;
    return [el("dt", {}, el("abbr", { title: `${what} dilution of precision` }, abbr), el("small", {}, what)), el("dd", { class: v === null ? "na" : "" }, v === null ? "not given" : fmtDop(v, digits))];
  }));
  const usedLine = result ? `${result.nUsed} satellites in the fix, ${result.sats.length} in the plot.` : "";
  const sig = state.sigma;
  const h = d && Number.isFinite(d.hdop) ? d.hdop : null, v = d && Number.isFinite(d.vdop) ? d.vdop : null;
  const prod = h !== null ? ` With σ_UERE = ${sig} m: one-sigma horizontal error ≈ ${f1(sig * h)} m (σ_UERE × HDOP)${v !== null ? `, vertical ≈ ${f1(sig * v)} m (σ_UERE × VDOP)` : ""}.` : "";
  $("gm-used").textContent = usedLine + prod;
}

function drawError() {
  const svg = $("gm-errplot");
  const facts = [];
  const e = result && result.ellipse;
  const c = result && result.cloud;
  if (!e && !c) {
    const why = !result ? "Waiting for the engine." : result.backend === "interim" ? "The engine build in this browser does not give the covariance or the Monte Carlo cloud. The 0.36 engine's geometry function does; this panel fills in when the Studio carries it." : "No result.";
    svg.replaceChildren(sv("rect", { x: 0, y: 0, width: 400, height: 400, class: "g-errbg" }), sv("text", { x: 200, y: 200, class: "g-empty", "text-anchor": "middle" }, "Not available from this engine build"));
    $("gm-err-desc").textContent = why;
    $("gm-errfacts").replaceChildren(el("dt", {}, "Why"), el("dd", { class: "na wide" }, why));
    return;
  }
  const vals = [];
  if (e) vals.push(e.semiMajorM * 1.2);
  if (c) for (const p of c.points) vals.push(p[0], p[1]);
  const ext = niceExtent(vals, 1);
  const PX = 180, scale = PX / ext, cx = 200, cy = 200;
  const kids = [sv("rect", { x: 0, y: 0, width: 400, height: 400, class: "g-errbg" })];
  for (const k of [-1, -0.5, 0.5, 1]) {
    kids.push(sv("line", { x1: cx + k * PX, y1: cy - PX, x2: cx + k * PX, y2: cy + PX, class: "g-ring" }), sv("line", { x1: cx - PX, y1: cy + k * PX, x2: cx + PX, y2: cy + k * PX, class: "g-ring" }));
  }
  kids.push(sv("line", { x1: cx - PX, y1: cy, x2: cx + PX, y2: cy, class: "g-axis" }), sv("line", { x1: cx, y1: cy - PX, x2: cx, y2: cy + PX, class: "g-axis" }));
  kids.push(sv("text", { x: cx + PX, y: cy - 6, class: "g-tick", "text-anchor": "end" }, `E +${ext} m`), sv("text", { x: cx + 6, y: cy - PX + 10, class: "g-tick" }, `N +${ext} m`));
  if (c) {
    const g = sv("g", { class: "g-cloud", "aria-hidden": "true" });
    for (const p of c.points) g.append(sv("circle", { cx: (cx + p[0] * scale).toFixed(1), cy: (cy - p[1] * scale).toFixed(1), r: 1.6 }));
    kids.push(g);
  }
  if (e) kids.push(sv("ellipse", { ...ellipseAttrs(e, cx, cy, scale), class: "g-ell" }));
  svg.replaceChildren(...kids);
  if (e) facts.push(["Ellipse, one sigma", `${f1(e.semiMajorM)} m × ${f1(e.semiMinorM)} m, long axis ${Math.round(((e.orientationDeg % 180) + 180) % 180)}° from north`]);
  if (c) {
    facts.push(["Monte Carlo", `${c.samples} samples, seed ${c.seed ?? state.seed}`]);
    if (Number.isFinite(c.rmsHorizontalM)) facts.push(["Cloud RMS, horizontal", `${f1(c.rmsHorizontalM)} m`]);
    if (Number.isFinite(c.rmsVerticalM)) facts.push(["Cloud RMS, vertical", `${f1(c.rmsVerticalM)} m`]);
  }
  $("gm-errfacts").replaceChildren(...facts.flatMap(([k, v]) => [el("dt", {}, k), el("dd", {}, v)]));
  $("gm-err-desc").textContent = facts.map(([k, v]) => `${k}: ${v}.`).join(" ");
}

function drawTable() {
  const rows = (result ? result.sats : []).map((s) => el("tr", {}, el("td", {}, styleFor(s.sys).label), el("td", {}, String(s.id)), el("td", { class: "num" }, String(s.az)), el("td", { class: "num" }, String(s.el)), el("td", {}, s.blocked ? `No: hidden by the ${s.blocked}` : s.used ? "Yes" : "No: in view, not used")));
  $("gm-tab-n").textContent = `${rows.length} satellites, the same as the skyplot.`;
  $("gm-sats").tBodies[0].replaceChildren(...(rows.length ? rows : [el("tr", {}, el("td", { colspan: "5" }, "No satellites to show."))]));
}

function drawAll() { drawPlot(); drawLegend(); drawDops(); drawError(); drawTable(); }

// ---------------------------------------------------------------- compute
let timer = 0;
function schedule() { clearTimeout(timer); drawPlot(); timer = setTimeout(compute, 180); }
async function compute() {
  if (!adapter) return;
  const mine = ++ticket;
  $("gm-engine").textContent = "Computing…";
  try {
    const r = await adapter.compute(state, skyline);
    if (mine !== ticket) return;
    result = r;
    $("gm-source").textContent = [r.source, ...r.notes].filter(Boolean).join(" ");
    $("gm-engine").textContent = r.backend === "none" ? r.notes[0] : r.missing.length ? `Ready. This engine build does not give ${r.missing.join(", ")}.` : "Ready.";
  } catch (e) {
    if (mine !== ticket) return;
    result = null;
    $("gm-engine").textContent = `The engine could not compute this sky: ${e && e.message ? e.message : e}`;
  }
  drawAll();
}

async function start() {
  fillControls();
  drawAll();
  const engine = await getEngine();
  adapter = createGeometryAdapter(engine);
  if (!engine.available) $("gm-engine").textContent = `The engine is not running in this browser (${engine.error || "WebAssembly is blocked"}).`;
  fillSystems();
  if (state.sky !== "custom" && state.sky !== "none") await loadSkyline(state.sky);
  await compute();
}
start();
