// SPDX-License-Identifier: AGPL-3.0-only
// Skyline editing for the Geometry view: the horizon a person draws on the skyplot, as a list of
// {az, el} vertices (degrees), kept sorted by azimuth with no two vertices closer than MIN_GAP_DEG.
// Between vertices the outline is a straight line in (azimuth, elevation), wrapping through north.
// This module only edits and draws the outline; which satellites it hides, and what that does to
// the geometry, is the engine's answer (lib/geometry-adapter.mjs). Pure; tested in skyline.test.mjs.
import { MAX_SKY_POINTS } from "./geomstate.mjs";

export const MIN_GAP_DEG = 1;
export { MAX_SKY_POINTS };

const r1 = (x) => Math.round(x * 10) / 10;
export const wrapAz = (az) => { const a = r1(((az % 360) + 360) % 360); return a >= 360 ? 0 : a; };
export const clampEl = (el) => r1(Math.min(90, Math.max(0, el)));

// Sorted, rounded to 0.1 degree, in range, with vertices closer than MIN_GAP_DEG in azimuth merged
// (the later one wins), and at most MAX_SKY_POINTS of them.
export function normalise(points) {
  const list = (Array.isArray(points) ? points : [])
    .filter((p) => p && Number.isFinite(p.az) && Number.isFinite(p.el))
    .map((p) => ({ az: wrapAz(p.az), el: clampEl(p.el) }))
    .sort((a, b) => a.az - b.az);
  const out = [];
  for (const p of list) {
    const last = out[out.length - 1];
    if (last && p.az - last.az < MIN_GAP_DEG) out[out.length - 1] = p;
    else out.push(p);
  }
  if (out.length > 1 && out[0].az + 360 - out[out.length - 1].az < MIN_GAP_DEG) out.shift();
  return out.slice(0, MAX_SKY_POINTS);
}

// The outline's elevation at an azimuth (for drawing): linear between neighbours, wrapping through north.
export function outlineAt(points, az) {
  const p = points;
  if (!p.length) return 0;
  if (p.length === 1) return p[0].el;
  const a = wrapAz(az);
  let i = p.findIndex((q) => q.az > a);
  const hi = i === -1 ? p[0] : p[i];
  const lo = i === -1 ? p[p.length - 1] : i === 0 ? p[p.length - 1] : p[i - 1];
  const span = (((hi.az - lo.az) % 360) + 360) % 360 || 360;
  const off = (((a - lo.az) % 360) + 360) % 360;
  return lo.el + (hi.el - lo.el) * (off / span);
}

// Densely sampled outline, for drawing a closed polygon.
export function outline(points, stepDeg = 2) {
  const out = [];
  for (let az = 0; az < 360; az += stepDeg) out.push({ az, el: outlineAt(points, az) });
  return out;
}

// Add a vertex; returns { points, index } (index of the new vertex, or -1 when it was refused).
export function addPoint(points, az, el) {
  if (points.length >= MAX_SKY_POINTS) return { points, index: -1 };
  const p = { az: wrapAz(az), el: clampEl(el) };
  if (points.some((q) => Math.min(Math.abs(q.az - p.az), 360 - Math.abs(q.az - p.az)) < MIN_GAP_DEG)) return { points, index: -1 };
  const next = normalise([...points, p]);
  return { points: next, index: next.findIndex((q) => q.az === p.az && q.el === p.el) };
}

// Move vertex i. Its azimuth stays strictly between its neighbours' (so dragging never reorders the
// outline); the elevation is clamped to 0..90.
export function movePoint(points, i, az, el) {
  if (!(i >= 0 && i < points.length)) return points;
  const n = points.length;
  let a = wrapAz(az);
  if (n > 1) {
    const prev = i > 0 ? points[i - 1].az : points[n - 1].az - 360;
    const next = i < n - 1 ? points[i + 1].az : points[0].az + 360;
    // Choose the representation of `a` nearest the vertex's current azimuth, then keep it between the neighbours.
    const cur = points[i].az;
    let rep = a;
    for (const c of [a - 360, a + 360]) if (Math.abs(c - cur) < Math.abs(rep - cur)) rep = c;
    rep = Math.min(next - MIN_GAP_DEG, Math.max(prev + MIN_GAP_DEG, rep));
    if (next - prev < 2 * MIN_GAP_DEG) rep = cur;
    a = wrapAz(rep);
  }
  const out = points.slice();
  out[i] = { az: a, el: clampEl(el) };
  return normalise(out);
}

export function removePoint(points, i) {
  if (!(i >= 0 && i < points.length)) return points;
  return points.filter((_, k) => k !== i);
}

// Keyboard editing of the selected vertex. Returns { points, index } or null for a key it does not handle.
// Up/Down: elevation by 1 degree (5 with Shift); Left/Right: azimuth by 1 degree (5 with Shift);
// Delete/Backspace removes it; Home/End select the first/last vertex.
export function keyEdit(points, i, key, shift = false) {
  const step = shift ? 5 : 1;
  const p = points[i];
  if (!p) return null;
  switch (key) {
    case "ArrowUp": return { points: movePoint(points, i, p.az, p.el + step), index: i };
    case "ArrowDown": return { points: movePoint(points, i, p.az, p.el - step), index: i };
    case "ArrowRight": { const next = movePoint(points, i, p.az + step, p.el); return { points: next, index: indexOfNear(next, p, step) }; }
    case "ArrowLeft": { const next = movePoint(points, i, p.az - step, p.el); return { points: next, index: indexOfNear(next, p, -step) }; }
    case "Delete": case "Backspace": { const next = removePoint(points, i); return { points: next, index: next.length ? Math.min(i, next.length - 1) : -1 }; }
    case "Home": return { points, index: 0 };
    case "End": return { points, index: points.length - 1 };
    default: return null;
  }
}
// After a move that may cross north (the list re-sorts), find the vertex that moved.
function indexOfNear(points, was, dAz) {
  const target = wrapAz(was.az + dAz);
  let best = 0, bd = Infinity;
  points.forEach((q, k) => { const d = Math.min(Math.abs(q.az - target), 360 - Math.abs(q.az - target)); if (d < bd) { bd = d; best = k; } });
  return best;
}

// A short spoken description of the outline, for the skyplot's text alternative.
export function describe(points) {
  if (!points.length) return "Open horizon: no skyline drawn.";
  const top = points.reduce((a, b) => (b.el > a.el ? b : a));
  return `Skyline of ${points.length} point${points.length === 1 ? "" : "s"}; highest ${top.el}° at azimuth ${top.az}°.`;
}
