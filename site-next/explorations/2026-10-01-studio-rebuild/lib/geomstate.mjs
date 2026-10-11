// SPDX-License-Identifier: AGPL-3.0-only
// The Geometry view's state, and its shareable address. Every setting of the view lives in the URL query,
// so a link opens the same sky. Reading is an allow-list: only the keys below are read, each value must
// match its exact form and range, and anything else falls back to the default with a note that says so.
// Nothing from the address is ever put into markup or into a request; numbers go to the engine as numbers.
// Tested in geomstate.test.mjs.

// The constellations the view can name. Which ones the running engine can model is the adapter's business
// (lib/geometry-adapter.mjs); the address may only name these.
export const SYSTEMS = Object.freeze(["gps", "galileo", "beidou", "glonass", "leo"]);
// Skyline presets the address may name. "none" is an open horizon, "custom" means the points in `sky`.
export const SKYLINES = Object.freeze(["none", "city-street", "harbour-approach", "fjord", "container-terminal", "custom"]);
// Named places (a convenience; the coordinates are what the engine uses).
export const PLACES = Object.freeze({
  tallinn: { label: "Tallinn harbour, Estonia", lat: 59.444, lon: 24.765, h: 10 },
  rotterdam: { label: "Port of Rotterdam, Netherlands", lat: 51.955, lon: 4.05, h: 5 },
  geiranger: { label: "Geirangerfjord, Norway", lat: 62.101, lon: 7.206, h: 5 },
  manhattan: { label: "Midtown Manhattan, United States", lat: 40.754, lon: -73.984, h: 15 },
  singapore: { label: "Singapore Strait", lat: 1.264, lon: 103.84, h: 5 },
  tromso: { label: "Tromsø, Norway", lat: 69.649, lon: 18.956, h: 10 },
});

export const MAX_SKY_POINTS = 72;
export const MAX_QUERY = 2048;

export const DEFAULTS = Object.freeze({
  place: "tallinn",
  lat: 59.444,
  lon: 24.765,
  h: 10,
  t: "2026-10-12T12:00:00Z",
  sys: Object.freeze(["gps", "galileo", "beidou"]),
  mask: 5,
  sky: "none",
  pts: Object.freeze([]),
  sigma: 3,
  seed: 1,
  n: 1000,
});

// Each numeric key: its range and the most decimals it may carry.
const NUM = {
  lat: { min: -89.9, max: 89.9, dp: 4 },
  lon: { min: -180, max: 180, dp: 4 },
  h: { min: -500, max: 9000, dp: 1 },
  mask: { min: 0, max: 45, dp: 1 },
  sigma: { min: 0.1, max: 50, dp: 2 },
  seed: { min: 0, max: 2147483647, dp: 0 },
  n: { min: 100, max: 5000, dp: 0 },
};
const KEYS = ["place", "lat", "lon", "h", "t", "sys", "mask", "sky", "pts", "sigma", "seed", "n"];

// A plain decimal with at most `dp` decimals: no exponent, no hex, no spaces, no "Infinity".
function readNum(raw, { min, max, dp }) {
  if (typeof raw !== "string" || raw.length > 16) return null;
  const re = dp > 0 ? new RegExp(`^-?\\d{1,10}(\\.\\d{1,${dp}})?$`) : /^\d{1,10}$/;
  if (!re.test(raw)) return null;
  const v = Number(raw);
  return Number.isFinite(v) && v >= min && v <= max ? v : null;
}

// "2026-10-12T12:00:00Z": UTC only, whole seconds, years 2000 to 2099, and a real calendar date.
export function readTime(raw) {
  if (typeof raw !== "string" || !/^20\d\d-\d\d-\d\dT\d\d:\d\d:\d\dZ$/.test(raw)) return null;
  const ms = Date.parse(raw);
  if (!Number.isFinite(ms)) return null;
  return new Date(ms).toISOString().replace(".000Z", "Z") === raw ? raw : null;
}

// "120:25,180:30.5": azimuth:elevation pairs, degrees, at most one decimal.
export function readPoints(raw) {
  if (typeof raw !== "string" || raw.length > MAX_SKY_POINTS * 12) return null;
  if (raw === "") return [];
  const parts = raw.split(",");
  if (parts.length > MAX_SKY_POINTS) return null;
  const out = [];
  for (const p of parts) {
    const m = /^(\d{1,3}(?:\.\d)?):(\d{1,2}(?:\.\d)?)$/.exec(p);
    if (!m) return null;
    const az = Number(m[1]), el = Number(m[2]);
    if (!(az >= 0 && az < 360 && el >= 0 && el <= 90)) return null;
    out.push({ az, el });
  }
  return out;
}

const fmt = (v, dp) => String(Number(v.toFixed(dp)));

// Read the view's state from a query string (or URLSearchParams). Returns { state, notes }: notes say which
// values were refused. Unknown keys are ignored silently (a link may carry the Studio's theme, for example).
export function parseState(query) {
  const notes = [];
  const state = { ...DEFAULTS, sys: [...DEFAULTS.sys], pts: [] };
  let text = typeof query === "string" ? query : query && typeof query.toString === "function" ? query.toString() : "";
  if (text.startsWith("?")) text = text.slice(1);
  if (text.length > MAX_QUERY) return { state, notes: [`The address is longer than ${MAX_QUERY} characters, so its settings were not read.`] };
  let params;
  try { params = new URLSearchParams(text); } catch { return { state, notes: ["The address could not be read."] }; }
  const seen = new Set();
  for (const [k, v] of params) {
    if (!KEYS.includes(k)) continue;
    if (seen.has(k)) { notes.push(`"${k}" appears more than once; the first value is used.`); continue; }
    seen.add(k);
    if (k in NUM) {
      const x = readNum(v, NUM[k]);
      if (x === null) notes.push(`"${k}" is not a number from ${NUM[k].min} to ${NUM[k].max}; the default is used.`);
      else state[k] = x;
    } else if (k === "place") {
      if (Object.hasOwn(PLACES, v) || v === "custom") state.place = v;
      else notes.push("The place is not one this view knows; the default is used.");
    } else if (k === "t") {
      const t = readTime(v);
      if (t === null) notes.push("The time is not a UTC time from 2000 to 2099 (YYYY-MM-DDThh:mm:ssZ); the default is used.");
      else state.t = t;
    } else if (k === "sys") {
      const list = v === "" ? [] : v.split(",");
      if (list.length > SYSTEMS.length || !list.every((s) => SYSTEMS.includes(s))) notes.push("The constellation list names one this view does not know; the default is used.");
      else state.sys = SYSTEMS.filter((s) => list.includes(s));
    } else if (k === "sky") {
      if (SKYLINES.includes(v)) state.sky = v;
      else notes.push("The skyline is not one this view knows; an open horizon is used.");
    } else if (k === "pts") {
      const p = readPoints(v);
      if (p === null) notes.push(`The skyline points are not azimuth:elevation pairs in range (at most ${MAX_SKY_POINTS}); they were dropped.`);
      else state.pts = p;
    }
  }
  // A named place carries its own coordinates unless the address gives them.
  if (state.place !== "custom" && Object.hasOwn(PLACES, state.place)) {
    const p = PLACES[state.place];
    if (!seen.has("lat")) state.lat = p.lat;
    if (!seen.has("lon")) state.lon = p.lon;
    if (!seen.has("h")) state.h = p.h;
    if ((seen.has("lat") && state.lat !== p.lat) || (seen.has("lon") && state.lon !== p.lon)) state.place = "custom";
  }
  if (state.sky !== "custom") state.pts = [];
  return { state, notes };
}

// The query string for a state: only what differs from the defaults, in a fixed key order.
export function serializeState(state) {
  const s = { ...DEFAULTS, ...state };
  const q = [];
  const put = (k, v) => q.push(`${k}=${encodeURIComponent(v)}`);
  if (s.place !== DEFAULTS.place) put("place", s.place);
  const named = s.place !== "custom" && Object.hasOwn(PLACES, s.place) ? PLACES[s.place] : null;
  // A named place's own coordinates are implied by its name; a custom place always states them.
  for (const k of ["lat", "lon", "h"]) if (!named || s[k] !== named[k]) put(k, fmt(s[k], NUM[k].dp));
  if (s.t !== DEFAULTS.t) q.push(`t=${s.t}`); // already checked to digits, "-", ":", "T" and "Z": no escaping needed
  const sys = SYSTEMS.filter((x) => s.sys.includes(x));
  if (sys.join(",") !== DEFAULTS.sys.join(",")) q.push(`sys=${sys.join(",")}`);
  for (const k of ["mask", "sigma", "seed", "n"]) if (s[k] !== DEFAULTS[k]) put(k, fmt(s[k], NUM[k].dp));
  if (s.sky !== DEFAULTS.sky) put("sky", s.sky);
  if (s.sky === "custom" && s.pts.length) q.push(`pts=${s.pts.map((p) => `${fmt(p.az, 1)}:${fmt(p.el, 1)}`).join(",")}`);
  return q.length ? "?" + q.join("&") : "";
}

// Check a state built in the page (from form controls) against the same rules the address is read with:
// serialise it and read it back. Returns the state as read, so a control can never hold a value a link could not.
export function normaliseState(state) {
  return parseState(serializeState(state)).state;
}
