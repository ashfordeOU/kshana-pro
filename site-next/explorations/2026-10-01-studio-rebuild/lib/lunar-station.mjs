// SPDX-License-Identifier: AGPL-3.0-only
// The lunar reference-station view (0.36): reads the engine's lunar-station-coverage report, keeps the view's state
// in a validated address, writes the few inputs a reader may change into the bundled scenario file, and places the
// report's grid points on a south-polar stereographic map. The field names are the plan's proposals
// (docs/design/2026-10-12-lunar-reference-station-plan.md, A5); they live in FIELDS so a rename is one edit.
// Every figure shown comes from the report; the projection, cell outlines, threshold boundary and the azimuth
// lines here only place those figures on the drawing. Pure; tested in lunar-station.test.mjs.

export const KIND = "lunar-station-coverage";
export const SCENARIO_FILE = "scenarios/lunar-station-coverage.toml";
export const RECORDED_FILE = "recorded/lunar-station-coverage.json";
// Shown on every result, whatever the file says (the plan's rules 1, 3 and 4).
export const HONESTY = Object.freeze([
  "MODELLED.",
  "Under the stated public assumptions, not a statement about the performance of any real system.",
  "Not affiliated with or endorsed by any agency.",
]);
// Text from a file is shown only when it names no programme or agency: the view uses generic names only.
const NAMED = /nova\s*moon|moonlight|\besa\b|european space agency|\bnasa\b/i;
export const textIsGeneric = (s) => typeof s === "string" && !NAMED.test(s);

// The report's field names (plan A5), in one place.
export const FIELDS = Object.freeze({
  inputs: "inputs_echo", nPoints: "n_grid_points", nEpochs: "n_epochs", nTrials: "n_trials",
  region: "region", radius: "coverage_radius_km", points: "points", budget: "budget_at_station_and_at_edge",
  label: "label", tiers: "figure_tiers", note: "note", units: "units",
  r: { availability: "availability", coverage: "coverage_fraction", improvement: "improvement_factor", hplMax: "hpl_max_m", vplMax: "vpl_max_m", hplBelow: "hpl_below_alert_fraction", worst: "worst_point" },
  p: { lat: "lat", lon: "lon", dist: "distance_km", availability: "availability", coverage: "coverage_fraction", corrected: "corrected_p_3d_m", uncorrected: "uncorrected_p_3d_m", hplMedian: "hpl_median_m", reasons: "unavailable_reasons" },
  in: { stationLat: "station_lat_deg", stationLon: "station_lon_deg", threshold: "accuracy_threshold_m", percentile: "percentile", alert: "alert_limit_m", latMin: "grid_lat_min_deg", latMax: "grid_lat_max_deg", latStep: "grid_lat_step_deg", moonRadius: "moon_radius_m" },
});
// The reasons an epoch is unavailable at a point (plan A4).
export const REASONS = Object.freeze({ user_lt4: "fewer than four satellites in view of the user", station_missing_sat: "the station does not see every satellite the user uses" });
// Mean lunar radius, metres: used only to draw the coverage radius on the map when the report does not state one.
export const MOON_RADIUS_M = 1737400;

const isNum = (x) => typeof x === "number" && Number.isFinite(x);
const num = (x) => (isNum(x) ? x : null);
const frac = (x) => (isNum(x) && x >= 0 && x <= 1 ? x : null);
const obj = (x) => (x && typeof x === "object" && !Array.isArray(x) ? x : null);
const MAX_POINTS = 20000, MAX_RADIUS = 3600;

// The report inside what a reader may give: the bare report, a run_all reply or a recording ({json: "…"}), or
// {report: …}. Returns {report, svg, engineVersion} or {error}.
export function unwrap(x) {
  let v = x, svg = "", engineVersion = null;
  if (typeof v === "string") { try { v = JSON.parse(v); } catch { return { error: "This file is not JSON." }; } }
  if (!obj(v)) return { error: "This file is not a JSON object." };
  if (typeof v.engine_version === "string") engineVersion = v.engine_version.slice(0, 40);
  if (typeof v.svg === "string") svg = v.svg;
  if (typeof v.json === "string") { try { v = JSON.parse(v.json); } catch { return { error: "The file's json field is not JSON." }; } }
  else if (obj(v.json)) v = v.json;
  else if (obj(v.report)) v = v.report;
  if (!obj(v) || !obj(v[FIELDS.region]) || !Array.isArray(v[FIELDS.points])) return { error: `This is not a ${KIND} report (it has no region figures or grid points).` };
  return { report: v, svg, engineVersion };
}

// The percentile the report was run at: inputs_echo.percentile, else the N in region.corrected_pN_3d_m.
export function percentileOf(rep) {
  const p = num(obj(rep[FIELDS.inputs])?.[FIELDS.in.percentile]);
  if (p != null && p > 0 && p < 100) return p;
  const k = Object.keys(obj(rep[FIELDS.region]) || {}).map((s) => /^corrected_p(\d{1,2})_3d_m$/.exec(s)).find(Boolean);
  return k ? Number(k[1]) : null;
}

// The report, normalised and checked. Nothing is filled in: a missing figure is null and the page says so.
export function readReport(input) {
  const u = unwrap(input);
  if (u.error) return u;
  const rep = u.report, inp = obj(rep[FIELDS.inputs]) || {}, reg = obj(rep[FIELDS.region]);
  const n = percentileOf(rep);
  const label = typeof rep[FIELDS.label] === "string" ? rep[FIELDS.label].slice(0, 40) : "";
  const note = typeof rep[FIELDS.note] === "string" ? rep[FIELDS.note].slice(0, 2000) : "";
  const worst = obj(reg[FIELDS.r.worst]);
  const points = rep[FIELDS.points].slice(0, MAX_POINTS).map((p) => {
    const q = obj(p) || {};
    const reasons = obj(q[FIELDS.p.reasons]) ? Object.entries(q[FIELDS.p.reasons]).filter(([, c]) => isNum(c) && c >= 0).map(([k, c]) => [k.slice(0, 40), Math.round(c)])
      : Array.isArray(q[FIELDS.p.reasons]) ? q[FIELDS.p.reasons].filter((s) => typeof s === "string").map((s) => [s.slice(0, 40), null]) : [];
    return { lat: num(q[FIELDS.p.lat]), lon: num(q[FIELDS.p.lon]), distKm: num(q[FIELDS.p.dist]), availability: frac(q[FIELDS.p.availability]), coverage: frac(q[FIELDS.p.coverage]), corrected: num(q[FIELDS.p.corrected]), uncorrected: num(q[FIELDS.p.uncorrected]), hplMedian: num(q[FIELDS.p.hplMedian]), reasons };
  }).filter((p) => p.lat != null && p.lon != null && p.lat >= -90 && p.lat <= 0);
  const radius = (Array.isArray(rep[FIELDS.radius]) ? rep[FIELDS.radius] : []).slice(0, MAX_RADIUS)
    .map((r) => ({ az: num(obj(r)?.azimuth_deg), km: num(obj(r)?.radius_km) })).filter((r) => r.az != null && r.az >= 0 && r.az <= 360 && r.km != null && r.km >= 0)
    .sort((a, b) => a.az - b.az);
  const units = obj(rep[FIELDS.units]) || {};
  return {
    label, modelled: /^MODELLED$/i.test(label),
    note: textIsGeneric(note) ? note : "", noteWithheld: !!note && !textIsGeneric(note),
    tiers: obj(rep[FIELDS.tiers]) || (Array.isArray(rep[FIELDS.tiers]) ? rep[FIELDS.tiers] : null),
    percentile: n,
    nPoints: num(rep[FIELDS.nPoints]), nEpochs: num(rep[FIELDS.nEpochs]), nTrials: num(rep[FIELDS.nTrials]),
    station: { lat: num(inp[FIELDS.in.stationLat]), lon: num(inp[FIELDS.in.stationLon]) },
    threshold: num(inp[FIELDS.in.threshold]), alert: num(inp[FIELDS.in.alert]),
    grid: { latMin: num(inp[FIELDS.in.latMin]), latMax: num(inp[FIELDS.in.latMax]), latStep: num(inp[FIELDS.in.latStep]) },
    moonRadiusM: num(inp[FIELDS.in.moonRadius]) ?? num(rep.moon_radius_m),
    region: {
      availability: frac(reg[FIELDS.r.availability]), coverage: frac(reg[FIELDS.r.coverage]),
      corrected3d: n == null ? null : num(reg[`corrected_p${n}_3d_m`]), correctedH: n == null ? null : num(reg[`corrected_p${n}_h_m`]), correctedV: n == null ? null : num(reg[`corrected_p${n}_v_m`]),
      uncorrected3d: n == null ? null : num(reg[`uncorrected_p${n}_3d_m`]),
      improvement: num(reg[FIELDS.r.improvement]), hplMax: num(reg[FIELDS.r.hplMax]), vplMax: num(reg[FIELDS.r.vplMax]), hplBelow: frac(reg[FIELDS.r.hplBelow]),
      worst: worst && isNum(worst.lat) && isNum(worst.lon) ? { lat: worst.lat, lon: worst.lon, value: num(worst.value) } : null,
    },
    radius, points,
    budget: obj(rep[FIELDS.budget]),
    inputs: Object.entries(inp).filter(([, v]) => isNum(v) || typeof v === "boolean" || (typeof v === "string" && v.length <= 200)).slice(0, 80),
    units: Object.fromEntries(Object.entries(units).filter(([, v]) => typeof v === "string").map(([k, v]) => [k.slice(0, 60), v.slice(0, 40)])),
    svg: u.svg, engineVersion: u.engineVersion,
  };
}

// ---------------------------------------------------------------- the address
// The inputs a reader may change, with their scenario key and bounds. Only keys given in the address are written
// into the scenario file; everything else keeps the file's own value.
export const INPUTS = Object.freeze([
  { q: "slat", key: "station_lat_deg", label: "Station latitude, degrees", min: -90, max: -80, step: 0.1 },
  { q: "slon", key: "station_lon_deg", label: "Station longitude, degrees", min: -180, max: 180, step: 0.1 },
  { q: "thr", key: "accuracy_threshold_m", label: "Accuracy threshold, metres", min: 0.05, max: 100, step: 0.05 },
  { q: "survey", key: "survey_sigma_m", label: "Station survey error (1-sigma), metres", min: 0, max: 10, step: 0.01 },
  { q: "lat", key: "latency_s", label: "Correction latency, seconds", min: 0, max: 60, step: 0.5 },
  { q: "trials", key: "n_trials", label: "Monte Carlo trials per epoch", min: 1, max: 1000, step: 1, int: true },
  { q: "tstep", key: "t_step_s", label: "Time step, seconds", min: 60, max: 86400, step: 60, int: true },
  { q: "gstep", key: "grid_lat_step_deg", label: "Grid ring spacing, degrees", min: 0.25, max: 5, step: 0.25 },
  { q: "seed", key: "seed", label: "Seed", min: 0, max: 2147483647, step: 1, int: true },
]);
export const MAX_QUERY = 512;

// {state: {src, set: {q: number}}, notes}. src: "recorded" (the recording that ships) or "live" (run here).
export function parseState(search) {
  const notes = [], set = {};
  let s = typeof search === "string" ? search : "";
  if (s.length > MAX_QUERY) { notes.push("The address was too long; the defaults are shown."); s = ""; }
  const q = new URLSearchParams(s.replace(/^\?/, ""));
  const src = q.get("src") === "live" ? "live" : "recorded";
  if (q.has("src") && !["live", "recorded"].includes(q.get("src"))) notes.push("Unknown source in the address; the recording is shown.");
  for (const d of INPUTS) {
    if (!q.has(d.q)) continue;
    const raw = q.get(d.q), v = Number(raw);
    if (raw === "" || !/^-?\d+(\.\d+)?$/.test(raw) || !Number.isFinite(v) || v < d.min || v > d.max || (d.int && !Number.isInteger(v))) { notes.push(`${d.label}: "${raw.slice(0, 20)}" is outside ${d.min} to ${d.max}; the file's value is used.`); continue; }
    set[d.q] = v;
  }
  return { state: { src, set }, notes };
}
export function buildSearch(state) {
  const q = new URLSearchParams();
  if (state.src === "live") q.set("src", "live");
  for (const d of INPUTS) if (isNum(state.set?.[d.q])) q.set(d.q, String(state.set[d.q]));
  const s = q.toString();
  return s ? `?${s}` : "";
}

// The numeric value of `key = <number>` in a TOML text, or null (the page shows the file's own value).
export function tomlNumber(toml, key) {
  const m = new RegExp(`^\\s*${key}\\s*=\\s*(-?\\d+(?:\\.\\d+)?(?:[eE][-+]?\\d+)?)\\s*(?:#.*)?$`, "m").exec(toml || "");
  return m ? Number(m[1]) : null;
}
// The scenario text with the reader's inputs written in. A key already in the file is replaced on its own line; a
// key that is not there is refused (the scenario says what it reads; the page does not invent fields).
export function withInputs(toml, set) {
  let out = String(toml || "");
  const missing = [];
  for (const d of INPUTS) {
    if (!isNum(set?.[d.q])) continue;
    const v = set[d.q];
    if (v < d.min || v > d.max || (d.int && !Number.isInteger(v))) return { error: `${d.label} is out of range.` };
    const re = new RegExp(`^(\\s*${d.key}\\s*=\\s*)(-?\\d+(?:\\.\\d+)?(?:[eE][-+]?\\d+)?)(\\s*(?:#.*)?)$`, "m");
    if (!re.test(out)) { missing.push(d.key); continue; }
    out = out.replace(re, (_, a, _b, c) => `${a}${v}${c}`);
  }
  if (missing.length) return { error: `The scenario file has no ${missing.join(", ")}; it cannot be changed here.` };
  return { toml: out };
}

// ---------------------------------------------------------------- the map
// South-polar stereographic: the pole in the centre, latitude `edge` on the rim of radius `r`, longitude 0 up,
// 90 degrees east to the right (as south-polar maps are drawn).
const rad = (d) => (d * Math.PI) / 180;
export function stereo(lat, lon, edge = -80, r = 180) {
  const rho = (r * Math.tan(rad((90 + lat) / 2))) / Math.tan(rad((90 + edge) / 2));
  return [rho * Math.sin(rad(lon)), -rho * Math.cos(rad(lon))];
}

// The cell each grid point stands for: its ring's latitude band and its share of the ring's longitudes. Points are
// grouped into rings by latitude; the band is half the ring spacing either side, clipped to the pole and the rim.
export function cellsOf(points, step) {
  const rings = new Map();
  for (const p of points) { const k = p.lat.toFixed(6); if (!rings.has(k)) rings.set(k, []); rings.get(k).push(p); }
  const lats = [...rings.keys()].map(Number).sort((a, b) => a - b);
  const h = (isNum(step) && step > 0 ? step : lats.length > 1 ? Math.min(...lats.slice(1).map((l, i) => l - lats[i])) : 1) / 2;
  const cells = [];
  lats.forEach((lat, ri) => {
    const ring = rings.get(lat.toFixed(6)).slice().sort((a, b) => a.lon - b.lon);
    const pole = lat <= -90 + 1e-9;
    const w = 360 / ring.length;
    ring.forEach((p, i) => cells.push({ p, ring: ri, i, n: ring.length, pole, lat0: Math.max(-90, lat - h), lat1: Math.min(lats.at(-1) + h, lat + h), lon0: pole ? -180 : p.lon - w / 2, lon1: pole ? 180 : p.lon + w / 2 }));
  });
  return { cells, lats, half: h };
}
const arc = (lat, a, b, edge, r, n = 8) => Array.from({ length: n + 1 }, (_, k) => stereo(lat, a + ((b - a) * k) / n, edge, r));
const pts = (list) => list.map(([x, y]) => `${x.toFixed(1)} ${y.toFixed(1)}`).join(" L");
export function cellPath(c, edge, r) {
  if (c.pole) { const [, y] = stereo(c.lat1, 0, edge, r); const rr = Math.abs(y); return `M${(-rr).toFixed(1)} 0 A${rr.toFixed(1)} ${rr.toFixed(1)} 0 1 0 ${rr.toFixed(1)} 0 A${rr.toFixed(1)} ${rr.toFixed(1)} 0 1 0 ${(-rr).toFixed(1)} 0Z`; }
  return `M${pts([...arc(c.lat1, c.lon0, c.lon1, edge, r), ...arc(c.lat0, c.lon1, c.lon0, edge, r)])}Z`;
}
// Whether a point meets the threshold: an available percentile at or below it.
export const meets = (p, thr) => p.corrected != null && isNum(thr) && p.corrected <= thr;

// The boundary between cells that meet the threshold and cells that do not, as [lat, lon, lat, lon] segments
// along cell edges (constant latitude or constant longitude). The rim of the grid is not a boundary.
export function thresholdEdges(cellInfo, thr) {
  const { cells, lats } = cellInfo, segs = [];
  const byRing = lats.map((_, ri) => cells.filter((c) => c.ring === ri));
  const norm = (l) => ((l + 540) % 360) - 180;
  const at = (ri, lon) => { const ring = byRing[ri]; if (!ring) return null; if (ring[0].pole) return ring[0]; return ring.find((c) => { const d = norm(lon - c.p.lon); return d >= -180 / c.n && d < 180 / c.n; }) || null; };
  for (const c of cells) {
    const ok = meets(c.p, thr);
    // the band toward the rim: test the neighbour ring along the shared edge in short pieces
    const out = byRing[c.ring + 1];
    if (out) {
      const a0 = c.pole ? -180 : c.lon0, a1 = c.pole ? 180 : c.lon1, k = c.pole ? 72 : 6;
      for (let j = 0; j < k; j++) {
        const x0 = a0 + ((a1 - a0) * j) / k, x1 = a0 + ((a1 - a0) * (j + 1)) / k, nb = at(c.ring + 1, (x0 + x1) / 2);
        if (nb && meets(nb.p, thr) !== ok) segs.push([c.lat1, x0, c.lat1, x1]);
      }
    }
    // the side shared with the next cell in the ring (each side once)
    if (!c.pole && c.n > 1) { const nb = byRing[c.ring][(c.i + 1) % c.n]; if (meets(nb.p, thr) !== ok) segs.push([c.lat0, c.lon1, c.lat1, c.lon1]); }
  }
  return segs;
}
export function segmentPath(segs, edge, r) {
  return segs.map(([la0, lo0, la1, lo1]) => {
    const line = la0 === la1 ? arc(la0, lo0, lo1, edge, r, 4) : [stereo(la0, lo0, edge, r), stereo(la1, lo1, edge, r)];
    return `M${pts(line)}`;
  }).join("");
}

// The point `km` from (lat, lon) along azimuth `az` (degrees from north) on a sphere of radius `radiusM`: places
// the report's coverage radius on the drawing.
export function destination(lat, lon, az, km, radiusM = MOON_RADIUS_M) {
  const d = (km * 1000) / radiusM, p1 = rad(lat), l1 = rad(lon), t = rad(az);
  const p2 = Math.asin(Math.sin(p1) * Math.cos(d) + Math.cos(p1) * Math.sin(d) * Math.cos(t));
  const l2 = l1 + Math.atan2(Math.sin(t) * Math.sin(d) * Math.cos(p1), Math.cos(d) - Math.sin(p1) * Math.sin(p2));
  return [(p2 * 180) / Math.PI, ((((l2 * 180) / Math.PI) + 540) % 360) - 180];
}

// Colour bins, as multiples of the threshold: three that meet it, three that do not.
export const BINS = Object.freeze([0.25, 0.5, 1, 2, 4]);
export function binOf(value, thr) {
  if (value == null || !isNum(thr) || thr <= 0) return null;
  const k = BINS.findIndex((b) => value <= b * thr);
  return k < 0 ? BINS.length : k;
}
export const pct = (x) => (x == null ? "not stated" : `${(x * 100).toFixed(x > 0.999 && x < 1 ? 2 : 1)} %`);
export const metres = (x) => (x == null ? "not stated" : x < 10 ? `${x.toFixed(2)} m` : `${x.toFixed(1)} m`);
