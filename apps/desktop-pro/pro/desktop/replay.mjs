// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: incident replay v1 on the map (0.42 U19). Opens an incident forensics
// pack (E1) through the app (`pro_incident_replay`: verified first, then its tracks, findings,
// area and verdict) and replays it on one time cursor: the position each receiver ESTIMATED
// (its own fix) against the position the vessel or aircraft REPORTED (AIS, ADS-B), with the
// trust at each instant shaded from the pack's findings. The shading is the report's MODELLED
// inference, drawn as the report states it; nothing is re-analysed here. Pure model, tested.
import { fitView, haversine, PROJECTION, utcText } from "./geo.mjs";

/** The trust levels of the shading, best first. */
export const LEVELS = ["trusted", "degraded", "untrusted"];
const INTERFERENCE = new Set(["jamming", "spoofing", "jamming-and-spoofing"]);
/** A fix older than this is not drawn as the current position (a gap in the track). */
export const MAX_GAP_S = 60;

/** The replay as the view draws it, from the `pro_incident_replay` document. */
export function replayModel(doc) {
  if (!doc || doc.format !== "kshana-pro-incident-replay/1") throw new Error("not an incident replay document");
  const tracks = (doc.tracks || []).map((t, i) => ({
    id: `t${i}`, source: String(t.source || `track ${i + 1}`), kind: t.kind === "estimated" ? "estimated" : "reported", cls: t.class,
    points: (t.points || []).filter((p) => Number.isFinite(p.t) && Number.isFinite(p.lat) && Number.isFinite(p.lon)).sort((a, b) => a.t - b.t),
  })).filter((t) => t.points.length);
  const episodes = (doc.episodes || []).filter((e) => Number.isFinite(e.start)).map((e) => ({
    id: String(e.id || ""), category: String(e.category || ""), confidence: String(e.confidence || "low"), source: String(e.source || ""),
    start: e.start, end: Number.isFinite(e.end) ? e.end : e.start, statement: String(e.statement || ""), about_subject: !!e.about_subject,
  }));
  const all = tracks.flatMap((t) => t.points.map((p) => p.t));
  const t0 = Number.isFinite(doc.window && doc.window.start) ? doc.window.start : Math.min(...all);
  const t1 = Number.isFinite(doc.window && doc.window.end) ? doc.window.end : Math.max(...all);
  const v = doc.verification || {};
  return {
    caseId: String(doc.case_id || ""), title: String(doc.title || ""), status: String(doc.status || ""),
    t0, t1, tracks, episodes,
    verified: v.ok === true, trusted: v.trusted === true, problems: Array.isArray(v.problems) ? v.problems.map(String) : [],
    signer: String(v.signer || ""), keyId: String(v.key_id || ""),
    verdict: doc.verdict || {}, area: doc.area || null, limits: Array.isArray(doc.limits) ? doc.limits.map(String) : [],
    label: String(doc.label || ""),
  };
}

/** The level of one finding while it is active. */
export function levelOf(e) {
  if (INTERFERENCE.has(e.category)) return e.confidence === "low" ? "degraded" : "untrusted";
  return "degraded";
}

/** The trust at `t`: the worst level of the findings active at `t` (subject's first). */
export function trustAt(m, t) {
  let worst = 0;
  const active = [];
  for (const e of m.episodes) {
    if (t < e.start || t > e.end) continue;
    active.push(e);
    worst = Math.max(worst, LEVELS.indexOf(levelOf(e)));
  }
  return { level: LEVELS[worst], active };
}

/** The shading of [t0, t1] in `n` equal steps, merged into runs: [{ from, to, level }]. */
export function trustBands(m, n = 400) {
  const out = [];
  const span = Math.max(m.t1 - m.t0, 1);
  for (let i = 0; i < n; i++) {
    const a = m.t0 + (span * i) / n, b = m.t0 + (span * (i + 1)) / n;
    const level = trustAt(m, (a + b) / 2).level;
    const last = out[out.length - 1];
    if (last && last.level === level) last.to = b;
    else out.push({ from: a, to: b, level });
  }
  return out;
}

/** The last point of `track` at or before `t`, or null (none yet, or older than MAX_GAP_S). */
export function positionAt(track, t) {
  const p = track.points;
  let lo = 0, hi = p.length - 1, best = -1;
  while (lo <= hi) { const mid = (lo + hi) >> 1; if (p[mid].t <= t) { best = mid; lo = mid + 1; } else hi = mid - 1; }
  if (best < 0 || t - p[best].t > MAX_GAP_S) return null;
  return p[best];
}

/** At `t`: each track's position, and the distance between the first estimated and the first reported positions. */
export function frameAt(m, t) {
  const at = m.tracks.map((tr) => ({ track: tr, p: positionAt(tr, t) }));
  const est = at.find((a) => a.track.kind === "estimated" && a.p);
  const rep = at.find((a) => a.track.kind === "reported" && a.p);
  const gap = est && rep ? haversine(est.p.lat, est.p.lon, rep.p.lat, rep.p.lon) : null;
  return { t, at, gap, trust: trustAt(m, t) };
}

/** The next cursor time: J (back), L (forward) by `step` seconds; Home and End to the ends. */
export function stepCursor(m, t, key, step = 10) {
  const c = (x) => Math.min(m.t1, Math.max(m.t0, x));
  if (key === "j" || key === "ArrowLeft") return c(t - step);
  if (key === "l" || key === "ArrowRight") return c(t + step);
  if (key === "Home") return m.t0;
  if (key === "End") return m.t1;
  return t;
}

/** "the receiver's fix is 1.4 km from the reported position" text for a gap in metres. */
export const gapText = (g) => (g === null ? "no estimated and reported position at this instant" : g < 1000 ? `${Math.round(g)} m between estimated and reported` : `${(g / 1000).toFixed(2)} km between estimated and reported`);

// ------------------------------------------------------------------ the view (DOM)
const SVGNS = "http://www.w3.org/2000/svg";
function svg(tag, attrs = {}, ...kids) {
  const n = document.createElementNS(SVGNS, tag);
  for (const [k, v] of Object.entries(attrs)) if (v !== undefined && v !== null) n.setAttribute(k, String(v));
  for (const c of kids) if (c) n.append(c.nodeType ? c : document.createTextNode(String(c)));
  return n;
}
const LEVEL_VAR = { trusted: "var(--itg)", degraded: "var(--nav)", untrusted: "var(--int)" };

/** The map at cursor `t`. */
export function replayMap(m, t, { w = 720, h = 420 } = {}) {
  const pts = m.tracks.flatMap((tr) => tr.points);
  const v = fitView(pts, w, h, { pad: 32, minSpanM: 3000 });
  const root = svg("svg", { viewBox: `0 0 ${w} ${h}`, class: "pro-map", role: "img", "aria-label": "Incident replay map" });
  root.append(svg("rect", { x: 0, y: 0, width: w, height: h, class: "pro-map-ground" }));
  if (!v) return root;
  const a = m.area && Array.isArray(m.area.bbox) ? m.area.bbox : null; // [lat_min, lat_max, lon_min, lon_max], MODELLED
  if (a) {
    const [x0, y0] = v.project(a[2], a[1]), [x1, y1] = v.project(a[3], a[0]);
    root.append(svg("rect", { x: x0, y: y0, width: x1 - x0, height: y1 - y0, class: "pro-area-modelled" }), svg("text", { x: x0 + 4, y: y0 + 14, class: "pro-map-note" }, "Affected area (MODELLED)"));
  }
  const f = frameAt(m, t);
  for (const tr of m.tracks) {
    const d = (ps) => ps.map((p, i) => { const [x, y] = v.project(p.lon, p.lat); return `${i ? "L" : "M"}${x.toFixed(1)},${y.toFixed(1)}`; }).join("");
    root.append(svg("path", { d: d(tr.points), class: `pro-trail ${tr.kind} all` }));
    root.append(svg("path", { d: d(tr.points.filter((p) => p.t <= t)), class: `pro-trail ${tr.kind}` }));
    for (const p of tr.points.filter((p) => p.flagged && p.t <= t)) { const [x, y] = v.project(p.lon, p.lat); root.append(svg("circle", { cx: x, cy: y, r: 2.5, class: "pro-flag" })); }
  }
  for (const { track, p } of f.at) {
    if (!p) continue;
    const [x, y] = v.project(p.lon, p.lat);
    root.append(track.kind === "estimated"
      ? svg("circle", { cx: x, cy: y, r: 7, class: "pro-pos estimated", fill: LEVEL_VAR[f.trust.level] }, svg("title", {}, `${track.source}: estimated position`))
      : svg("rect", { x: x - 6, y: y - 6, width: 12, height: 12, class: "pro-pos reported" }, svg("title", {}, `${track.source}: reported position`)));
  }
  root.append(svg("line", { x1: 16, y1: h - 18, x2: 16 + v.scale.px, y2: h - 18, class: "pro-scale" }), svg("text", { x: 16, y: h - 24, class: "pro-map-note" }, v.scale.label), svg("text", { x: w - 12, y: h - 12, "text-anchor": "end", class: "pro-map-note" }, PROJECTION));
  return root;
}

/** The timeline strip: trust shading, findings and the cursor. Click or drag to move the cursor. */
export function replayStrip(m, t, onCursor, { w = 720, h = 54 } = {}) {
  const span = Math.max(m.t1 - m.t0, 1);
  const x = (tt) => ((tt - m.t0) / span) * w;
  const root = svg("svg", { viewBox: `0 0 ${w} ${h}`, class: "pro-strip", role: "slider", tabindex: "0", "aria-label": "Time cursor", "aria-valuemin": m.t0, "aria-valuemax": m.t1, "aria-valuenow": Math.round(t), "aria-valuetext": utcText(t) });
  for (const b of trustBands(m)) root.append(svg("rect", { x: x(b.from), y: 0, width: Math.max(x(b.to) - x(b.from), 0.5), height: 18, fill: LEVEL_VAR[b.level], opacity: 0.85 }));
  m.episodes.forEach((e, i) => root.append(svg("rect", { x: x(e.start), y: 24 + (i % 3) * 8, width: Math.max(x(e.end) - x(e.start), 2), height: 6, class: `pro-ep ${levelOf(e)}` }, svg("title", {}, `${e.category} (${e.confidence}) · ${e.source}`))));
  root.append(svg("line", { x1: x(t), y1: 0, x2: x(t), y2: h, class: "pro-cursor" }));
  const set = (ev) => { const r = root.getBoundingClientRect(); onCursor(m.t0 + Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width)) * span); };
  root.addEventListener("pointerdown", (ev) => { set(ev); root.setPointerCapture(ev.pointerId); root.onpointermove = set; });
  root.addEventListener("pointerup", () => { root.onpointermove = null; });
  return root;
}
