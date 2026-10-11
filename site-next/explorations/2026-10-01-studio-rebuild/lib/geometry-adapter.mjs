// SPDX-License-Identifier: AGPL-3.0-only
// The Geometry view's one door to the engine (design note 2026-10-12, sections 3.2 and 3.5). Everything the
// view shows (where each satellite is, which ones the skyline hides, the DOPs, the open-sky fraction, the
// horizontal error ellipse and the Monte Carlo cloud) comes from the WebAssembly engine through this file; the
// view computes no navigation quantity. Switching engine functions is a change to this file only (plus adding
// the names to ENGINE_FNS in lib/engine.mjs once the package has them): the view reads the normalised result.
//
// Backends, best first:
//   "geometry" — the 0.36 functions (GEOMETRY_FNS): sky_geometry(request_json), sky_monte_carlo(request_json,
//                seed, n), skyline_presets(), skyline_profile(skyline_json), skyplot_svg(request_json). Chosen
//                once the engine has sky_geometry; the others are used when present.
//   "interim"  — the 0.35 package's nmea_training(toml, seed), the engine's training-sky model: the nominal
//                GPS, Galileo and BeiDou constellations at a site, read back from the GSV (azimuth, elevation)
//                and GSA (used satellites, PDOP, HDOP, VDOP) sentences of its first epoch. No GDOP, TDOP,
//                covariance, Monte Carlo, LEO or skyline: the view says "needs engine 0.36" for those.
//   "none"     — no engine in this browser.
//
// The normalised result:
//   { backend, source, sats: [{sys, id, az, el, used, masked}], nUsed, nVisible, nMasked,
//     dop: {gdop, pdop, hdop, vdop, tdop} (each a number or null), dopDigits, openFraction,
//     ellipse: {semiMajorM, semiMinorM, orientationDeg} (1-sigma) | null,
//     missing: [what this backend cannot give], notes: [plain sentences] }
//   and from monteCarlo(): { points: [[e, n, u]...], seed, samples, rmsHorizontalM, rmsVerticalM, inside95 } | null
// Tested in geometry-adapter.test.mjs.
import { SKYLINE_PRESETS } from "./geometry-url.mjs";
import { tableFromVertices, emptyTable, BINS, snapHalf } from "./skyline-edit.mjs";

export const GEOMETRY_FNS = Object.freeze(["sky_geometry", "sky_monte_carlo", "skyline_presets", "skyline_profile", "skyplot_svg"]);
export const INTERIM_FN = "nmea_training";
// The interim model needs a calendar instant: the time offset counts from this one.
export const INTERIM_EPOCH = "2026-10-12T00:00:00Z";

export const SKYLINE_LABELS = Object.freeze({
  "city-street": "City street",
  "harbour-approach": "Harbour approach",
  "fjord": "Fjord",
  "container-terminal": "Container terminal",
});

// Until the engine's skyline_presets() is available: rough outlines in the spirit of the design note's
// stated preset dimensions, drawn only, never applied. Not surveys of any real place.
const V = (...xs) => xs.map(([az, el]) => ({ az, el }));
export const INTERIM_SKYLINES = Object.freeze({
  // A street running north-south between 30 m facades: open along the street.
  "city-street": V([0, 0], [8, 45], [30, 65], [90, 70], [150, 65], [172, 45], [180, 0], [188, 45], [210, 65], [270, 70], [330, 65], [352, 45]),
  // Open to seaward (the south), quay buildings to the north, a breakwater light.
  "harbour-approach": V([0, 8], [44, 8], [46, 3], [89, 3], [91, 0], [269, 0], [271, 3], [314, 3], [316, 8]),
  // A north-south channel between steep walls: open along the axis.
  "fjord": V([0, 2], [20, 14], [60, 21], [90, 22], [120, 21], [160, 14], [180, 2], [200, 14], [240, 21], [270, 22], [300, 21], [340, 14]),
  // Crane booms over two narrow sectors, container stacks on the quay side, open to the water.
  "container-terminal": V([0, 12], [40, 12], [42, 32], [48, 32], [50, 12], [130, 12], [132, 30], [138, 30], [140, 12], [178, 12], [182, 0], [358, 0]),
});

export function chooseBackend(engine) {
  if (!engine || !engine.available) return "none";
  if (engine.has("sky_geometry")) return "geometry";
  if (engine.has(INTERIM_FN)) return "interim";
  return "none";
}

// What each backend offers, so the page can enable a control or say why it cannot.
export function featuresFor(backend, engine) {
  const has = (f) => backend === "geometry" && !!engine && engine.has(f);
  return {
    systems: { gps: backend !== "none", galileo: backend !== "none", beidou: backend !== "none", leo: backend === "geometry" },
    skyline: backend === "geometry",          // the engine applies it
    canyon: has("skyline_profile"),           // the engine reduces the canyon form to a table
    presets: backend !== "none",              // engine presets, or the interim outlines
    monteCarlo: has("sky_monte_carlo"),
    svg: has("skyplot_svg"),
    tle: backend === "geometry",
    gdop: backend === "geometry",
  };
}

const num = (x) => (typeof x === "number" && Number.isFinite(x) ? x : null);
const NO_DOP = Object.freeze({ gdop: null, pdop: null, hdop: null, vdop: null, tdop: null });
const NEEDS_036 = "needs engine 0.36";

// ---------------------------------------------------------------- interim backend: the training sky
const TALKER = Object.freeze({ GP: "gps", GA: "galileo", GB: "beidou", BD: "beidou" });
const GSA_SYS = Object.freeze({ 1: "gps", 3: "galileo", 4: "beidou" });

export function interimInstant(tOffsetS) {
  return new Date(Date.parse(INTERIM_EPOCH) + Math.trunc(tOffsetS) * 1000).toISOString().replace(".000Z", "Z");
}

// The training scenario for one epoch at the site. Only numbers and names from fixed lists are written into
// it, so nothing typed in the page can reach the engine as TOML.
export function buildInterimToml(state) {
  const sys = state.sys.filter((s) => ["gps", "galileo", "beidou"].includes(s));
  const n = (x) => { if (!Number.isFinite(x)) throw new Error("a setting is not a number"); return String(x); };
  return [
    "[scenario]", 'name = "geometry-view"', `seed = ${n(Math.trunc(state.seed))}`, `start_utc = "${interimInstant(Number(n(state.t)))}"`, "duration_s = 1", "rate_hz = 1",
    "[vessel]", `lat_deg = ${n(Math.max(-89, Math.min(89, state.lat)))}`, `lon_deg = ${n(state.lon)}`, "speed_kn = 0", "heading_deg = 0",
    "[receiver]", `systems = [${sys.map((s) => `"${s}"`).join(", ")}]`, `elevation_mask_deg = ${n(Math.max(0, Math.min(45, state.mask)))}`, "max_used = 12",
    "[output]", 'sentences = ["GSA", "GSV"]', "",
  ].join("\n");
}

// Read the first epoch's GSV and GSA sentences. Strict: anything that is not a well-formed sentence is skipped.
export function parseInterimNmea(nmea) {
  const sats = new Map();
  const used = new Set();
  let dop = null, gsvTalkers = "";
  for (const line of String(nmea).split(/\r?\n/)) {
    const m = /^\$([A-Z]{2})(GSV|GSA),([0-9A-Za-z.,]*)\*[0-9A-F]{2}$/.exec(line.trim());
    if (!m) continue;
    const [, talker, type, body] = m;
    const f = body.split(",");
    if (type === "GSV") {
      const sys = TALKER[talker];
      if (!sys) continue;
      if (f[1] === "1" && gsvTalkers.includes(`|${talker}|`)) break; // the next epoch
      if (f[1] === "1") gsvTalkers += `|${talker}|`;
      for (let k = 3; k + 3 < f.length; k += 4) {
        const id = parseInt(f[k], 10), el = parseInt(f[k + 1], 10), az = parseInt(f[k + 2], 10);
        if (!(id > 0) || !(el >= 0 && el <= 90) || !(az >= 0 && az <= 360)) continue;
        sats.set(`${sys}:${id}`, { sys, id: String(id), az: az % 360, el, used: false, masked: false });
      }
    } else {
      if (gsvTalkers) break; // a GSA after the GSV block opens the next epoch
      const sys = GSA_SYS[f[f.length - 1]];
      const [p, h, v] = [f[14], f[15], f[16]].map((x) => (/^\d+(\.\d+)?$/.test(x || "") ? Number(x) : null));
      if (dop === null && p !== null) dop = { gdop: null, pdop: p, hdop: h, vdop: v, tdop: null };
      if (!sys) continue;
      for (const x of f.slice(2, 14)) if (/^\d+$/.test(x)) used.add(`${sys}:${parseInt(x, 10)}`);
    }
  }
  const order = ["gps", "galileo", "beidou"];
  const list = [...sats.values()];
  for (const s of list) s.used = used.has(`${s.sys}:${s.id}`);
  list.sort((a, b) => order.indexOf(a.sys) - order.indexOf(b.sys) || Number(a.id) - Number(b.id));
  return { sats: list, dop, nUsed: list.filter((s) => s.used).length };
}

async function computeInterim(engine, state) {
  const notes = [];
  if (state.leo) notes.push(`LEO: ${NEEDS_036}; left out.`);
  if (state.sky.kind !== "none") notes.push(`The skyline is drawn but not applied: applying it ${NEEDS_036}. Satellites behind it still count here.`);
  if (Math.abs(state.lat) > 89) notes.push("This engine build models sites within 89° of the equator; the latitude was limited to that.");
  notes.push(`Engine ${engine.version || ""} training-sky model (nmea_training) at ${interimInstant(state.t)}: angles in whole degrees and DOPs to one decimal, as receiver sentences carry them; the fix uses at most 12 satellites, chosen by signal strength.`.replace("  ", " "));
  const sys = state.sys.filter((s) => ["gps", "galileo", "beidou"].includes(s));
  const base = { ...empty("interim"), missing: ["GDOP", "TDOP", "the open-sky fraction", "the error ellipse", "the Monte Carlo cloud", "LEO", "the skyline"] };
  if (!sys.length) return { ...base, notes: ["Pick at least one of GPS, Galileo and BeiDou."] };
  const raw = await engine.call(INTERIM_FN, buildInterimToml(state), Math.trunc(state.seed));
  let doc;
  try { doc = JSON.parse(raw); } catch { throw new Error("the engine's reply was not JSON"); }
  if (!doc || typeof doc.nmea !== "string") throw new Error("the engine's reply has no sentences");
  const r = parseInterimNmea(doc.nmea);
  return { ...base, source: `Kshana engine ${engine.version || ""}, training-sky model`.replace("  ", " "), sats: r.sats, nUsed: r.nUsed, nVisible: r.sats.length, nMasked: 0, dop: r.dop || { ...NO_DOP }, dopDigits: 1, notes };
}

// ---------------------------------------------------------------- 0.36 backend
// The skyline as the engine's [skyline] table takes it (design note section 2.3).
export function skylineRequest(sky, table) {
  if (!sky || sky.kind === "none") return null;
  if (sky.kind === "preset") return { preset: sky.name };
  if (sky.kind === "canyon") return { street_az_deg: sky.streetAz, width_m: sky.widthM, height_left_m: sky.leftM, height_right_m: sky.rightM, antenna_m: sky.antennaM };
  const t = sky.table || table;
  return { table: Array.from(t, (v, k) => [k, v / 2]) };
}

// The request sky_geometry, sky_monte_carlo and skyplot_svg take. The engine validates it again.
export function buildRequest(state, tleText = "") {
  const systems = [...state.sys];
  const req = {
    site: { lat_deg: state.lat, lon_deg: state.lon, height_m: state.h },
    t_offset_s: state.t,
    systems: tleText ? [] : systems,
    mask_deg: state.mask,
    sigma_uere_m: state.uere,
  };
  if (!tleText && state.leo) req.leo_preset = state.leo;
  if (tleText) req.tle = tleText;
  const sky = skylineRequest(state.sky);
  if (sky) req.skyline = sky;
  return req;
}

const pick = (o, ...keys) => { for (const k of keys) if (o && Object.hasOwn(o, k)) return o[k]; return undefined; };
const SYS_NAMES = Object.freeze({ gps: "gps", galileo: "galileo", beidou: "beidou", leo: "leo", G: "gps", E: "galileo", C: "beidou", L: "leo" });

// Read the engine's sky_geometry reply into the normalised result. Every field is checked; a malformed reply
// throws rather than drawing something the engine did not say.
export function parseGeometry(doc, version = "") {
  if (!doc || typeof doc !== "object" || Array.isArray(doc)) throw new Error("the engine's geometry reply is not an object");
  const sats = (Array.isArray(doc.satellites) ? doc.satellites : []).slice(0, 2000).flatMap((s) => {
    if (!s || typeof s !== "object") return [];
    const sysRaw = pick(s, "system", "sys");
    const sys = typeof sysRaw === "string" && Object.hasOwn(SYS_NAMES, sysRaw) ? SYS_NAMES[sysRaw] : null;
    const az = num(pick(s, "az_deg", "azimuth_deg")), el = num(pick(s, "el_deg", "elevation_deg"));
    if (!sys || az === null || el === null) return [];
    const id = pick(s, "id", "prn", "name");
    const masked = pick(s, "masked", "masked_by_skyline") === true;
    const visible = pick(s, "visible", "used") === true;
    return [{ sys, id: typeof id === "string" || typeof id === "number" ? String(id).slice(0, 24) : "?", az: ((az % 360) + 360) % 360, el, used: visible && !masked, masked }];
  });
  const d = doc.dop && typeof doc.dop === "object" ? doc.dop : {};
  const dop = { gdop: num(d.gdop), pdop: num(d.pdop), hdop: num(d.hdop), vdop: num(d.vdop), tdop: num(d.tdop) };
  const e = pick(doc, "ellipse", "horizontal_ellipse");
  const a = e && num(pick(e, "semi_major_m", "major_m")), b = e && num(pick(e, "semi_minor_m", "minor_m")), o = e && num(pick(e, "orientation_deg", "angle_deg"));
  const ellipse = a !== null && b !== null && o !== null && a >= 0 && b >= 0 ? { semiMajorM: a, semiMinorM: b, orientationDeg: o } : null;
  const notes = (Array.isArray(doc.notes) ? doc.notes : []).filter((x) => typeof x === "string").map((x) => x.slice(0, 400)).slice(0, 10);
  const prov = doc.skyline && typeof doc.skyline.provenance === "string" ? doc.skyline.provenance.slice(0, 400) : "";
  if (prov) notes.unshift(`Skyline: ${prov}`);
  const of = num(pick(doc, "open_fraction")) ?? num(doc.skyline && doc.skyline.open_fraction);
  return {
    backend: "geometry", source: `Kshana engine ${version}, sky_geometry`.replace("  ", " "),
    sats, nUsed: sats.filter((s) => s.used).length, nVisible: sats.filter((s) => !s.masked).length, nMasked: sats.filter((s) => s.masked).length,
    dop, dopDigits: 2, openFraction: of !== null && of >= 0 && of <= 1 ? of : null, ellipse, missing: [], notes,
  };
}

export function parseMonteCarlo(doc) {
  if (!doc || typeof doc !== "object" || Array.isArray(doc)) throw new Error("the engine's Monte Carlo reply is not an object");
  const raw = pick(doc, "samples_enu_m", "points_enu_m");
  const points = (Array.isArray(raw) ? raw : []).slice(0, 5000).filter((p) => Array.isArray(p) && p.length >= 2 && p.slice(0, 3).every((x) => num(x) !== null));
  const inside = num(pick(doc, "inside_95_fraction", "containment_95"));
  return {
    points, seed: num(doc.seed), samples: num(pick(doc, "n", "samples")) ?? points.length,
    rmsHorizontalM: num(pick(doc, "rms_horizontal_m", "rms_h_m")), rmsVerticalM: num(pick(doc, "rms_vertical_m", "rms_v_m")),
    inside95: inside !== null && inside >= 0 && inside <= 1 ? inside : null,
  };
}

// skyline_presets() reply: [{name, table: [[az, el]...] | 360 numbers, provenance}] (or {presets: [...]}).
export function parsePresets(doc) {
  const list = Array.isArray(doc) ? doc : doc && Array.isArray(doc.presets) ? doc.presets : [];
  const out = {};
  for (const p of list) {
    if (!p || !SKYLINE_PRESETS.includes(p.name)) continue;
    const t = parseProfile(p);
    if (t) out[p.name] = { table: t, provenance: typeof p.provenance === "string" ? p.provenance.slice(0, 400) : "" };
  }
  return out;
}

// skyline_profile() reply, or a preset's table: 360 elevations, or [az, el] pairs on whole degrees.
export function parseProfile(doc) {
  const raw = Array.isArray(doc) ? doc : doc && (doc.table || doc.profile);
  if (!Array.isArray(raw)) return null;
  const t = emptyTable();
  if (raw.length === BINS && raw.every((x) => num(x) !== null)) { raw.forEach((x, k) => { t[k] = snapHalf(x); }); return t; }
  const pairs = raw.filter((p) => Array.isArray(p) && num(p[0]) !== null && num(p[1]) !== null);
  if (!pairs.length) return null;
  return tableFromVertices(pairs.map(([az, el]) => ({ az, el })));
}

function empty(backend) {
  return { backend, source: "", sats: [], nUsed: 0, nVisible: 0, nMasked: 0, dop: { ...NO_DOP }, dopDigits: 2, openFraction: null, ellipse: null, missing: [], notes: [] };
}
const parseJson = (raw, what) => { try { return JSON.parse(raw); } catch { throw new Error(`the engine's ${what} reply was not JSON`); } };

// ---------------------------------------------------------------- the adapter the view uses
// engine: what lib/engine-page.mjs getEngine() resolves to ({available, version, has(fn), call(fn, ...args)}).
export function createGeometryAdapter(engine) {
  const backend = chooseBackend(engine);
  const features = featuresFor(backend, engine);
  let presetCache = null;
  return {
    backend,
    features,
    // A named preset's table and provenance: the engine's when it has presets, the interim outline otherwise.
    async preset(name) {
      if (!SKYLINE_PRESETS.includes(name)) return { table: emptyTable(), provenance: "" };
      if (backend === "geometry" && engine.has("skyline_presets")) {
        presetCache = presetCache || parsePresets(parseJson(await engine.call("skyline_presets"), "skyline presets"));
        if (presetCache[name]) return { table: new Uint8Array(presetCache[name].table), provenance: presetCache[name].provenance };
      }
      return { table: tableFromVertices(INTERIM_SKYLINES[name]), provenance: `Illustrative outline drawn by this page; applying a skyline ${NEEDS_036}.` };
    },
    // The table the engine will use for a canyon (or any non-table skyline), so the page draws exactly that.
    async profile(sky) {
      if (!features.canyon) return null;
      return parseProfile(parseJson(await engine.call("skyline_profile", JSON.stringify(skylineRequest(sky))), "skyline profile"));
    },
    async compute(state, tleText = "") {
      if (backend === "geometry") return parseGeometry(parseJson(await engine.call("sky_geometry", JSON.stringify(buildRequest(state, tleText))), "geometry"), engine.version || "");
      if (backend === "interim") return computeInterim(engine, state);
      return { ...empty("none"), notes: ["The engine is not running in this browser, so there is nothing to show. It runs as WebAssembly; a browser or host that blocks WebAssembly cannot run this view."] };
    },
    async monteCarlo(state, tleText = "") {
      if (!features.monteCarlo || !state.mc) return null;
      return parseMonteCarlo(parseJson(await engine.call("sky_monte_carlo", JSON.stringify(buildRequest(state, tleText)), Math.trunc(state.seed), Math.trunc(state.mc)), "Monte Carlo"));
    },
    // The engine's own SVG skyplot, as markup still to be parsed and sanitised by the caller (lib/svgsafe.mjs).
    async svg(state, tleText = "") {
      if (!features.svg) return null;
      const out = await engine.call("skyplot_svg", JSON.stringify(buildRequest(state, tleText)));
      return typeof out === "string" ? out : null;
    },
  };
}
