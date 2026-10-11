// SPDX-License-Identifier: AGPL-3.0-only
// The Geometry view's one door to the engine. Everything the view shows (where each satellite is, which ones
// the fix uses, the DOPs, the horizontal error ellipse and the Monte Carlo cloud) comes from the WebAssembly
// engine through this file; the view itself computes no navigation quantity. Switching engine functions is a
// change to this file only: the view reads the normalised result below whatever the backend.
//
// Backends, best first:
//   "geometry" — the 0.36 engine functions `geometry(request_json)` and `skyline_preset(name)` (GEOMETRY_FNS).
//                Not in an engine package yet: chosen automatically once the package has them.
//   "interim"  — the 0.35 package's `nmea_training(toml, seed)`, the engine's training-sky model: the nominal
//                GPS, Galileo, BeiDou and GLONASS constellations at a place and a time, read back from the GSV
//                (azimuth, elevation) and GSA (used satellites, PDOP, HDOP, VDOP) sentences of its first epoch.
//                It has no GDOP, TDOP, covariance or Monte Carlo, no LEO and no skyline; the view says so.
//   "none"     — no engine in this browser.
//
// The normalised result:
//   { backend, source, sats: [{sys, id, az, el, used, blocked}], nUsed,
//     dop: {gdop, pdop, hdop, vdop, tdop} (each a number or null), dopDigits,
//     ellipse: {semiMajorM, semiMinorM, orientationDeg, sigmaM} | null,
//     cloud: {points: [[e, n, u], ...], seed, samples, rmsHorizontalM, rmsVerticalM} | null,
//     missing: [what this backend cannot give], notes: [plain sentences] }
// Tested in geometry-adapter.test.mjs.
import { SYSTEMS } from "./geomstate.mjs";
import { normalise } from "./skyline.mjs";

export const GEOMETRY_FNS = Object.freeze(["geometry", "skyline_preset"]);
export const INTERIM_FN = "nmea_training";

export const SKYLINE_LABELS = Object.freeze({
  "none": "Open horizon",
  "city-street": "City street",
  "harbour-approach": "Harbour approach",
  "fjord": "Fjord",
  "container-terminal": "Container terminal",
  "custom": "Drawn on the skyplot",
});

// Outlines for the four named skylines until the engine's skyline_preset() is available: illustrative shapes,
// not surveys of any real place. The engine's own presets replace them when it has them.
const P = (...xs) => xs.map(([az, el]) => ({ az, el }));
export const INTERIM_SKYLINES = Object.freeze({
  // A street running north-south between six-storey blocks: open along the street, walls east and west.
  "city-street": P([0, 8], [20, 35], [35, 55], [90, 62], [145, 55], [160, 35], [180, 8], [200, 35], [215, 55], [270, 60], [325, 55], [340, 35]),
  // A breakwater and a headland to the east, a low quay to the west, open sea to the south.
  "harbour-approach": P([0, 6], [40, 12], [70, 22], [95, 18], [120, 9], [180, 3], [240, 4], [280, 9], [320, 7]),
  // A fjord running east-west: steep walls north and south, a narrow open sky along its axis.
  "fjord": P([0, 42], [40, 38], [75, 14], [90, 9], [105, 14], [140, 40], [180, 46], [220, 40], [255, 15], [270, 10], [285, 15], [320, 38]),
  // Container stacks in most directions and a ship-to-shore crane to the south-west.
  "container-terminal": P([0, 18], [30, 22], [60, 14], [100, 24], [150, 20], [200, 26], [218, 46], [226, 46], [240, 22], [300, 16], [330, 24]),
});

export function chooseBackend(engine) {
  if (!engine || !engine.available) return "none";
  if (GEOMETRY_FNS.every((f) => engine.has(f))) return "geometry";
  if (engine.has(INTERIM_FN)) return "interim";
  return "none";
}

// What each backend can model, per constellation the view knows.
export function systemsFor(backend) {
  const interim = { gps: true, galileo: true, beidou: true, glonass: true, leo: false };
  return SYSTEMS.map((id) => {
    const available = backend === "geometry" ? true : backend === "interim" ? interim[id] : false;
    const why = available ? "" : backend === "none" ? "The engine is not running in this browser." : "This engine build has no LEO constellation for a ground user; the 0.36 geometry function adds it.";
    return { id, available, why };
  });
}

// ---------------------------------------------------------------- interim backend: the training sky
const TALKER = Object.freeze({ GP: "gps", GL: "glonass", GA: "galileo", GB: "beidou", BD: "beidou" });
const GSA_SYS = Object.freeze({ 1: "gps", 2: "glonass", 3: "galileo", 4: "beidou" });
const num = (x) => (typeof x === "number" && Number.isFinite(x) ? x : null);

// The training scenario for one epoch at the view's place and time. Only numbers and names from fixed
// lists are written into it, so nothing a person types can reach the engine as TOML.
export function buildInterimToml(state) {
  const sys = state.sys.filter((s) => ["gps", "galileo", "beidou", "glonass"].includes(s));
  const lat = Math.max(-89, Math.min(89, state.lat));
  const mask = Math.max(0, Math.min(45, state.mask));
  if (!/^20\d\d-\d\d-\d\dT\d\d:\d\d:\d\dZ$/.test(state.t)) throw new Error("the time is not a UTC instant");
  const n = (x) => { if (!Number.isFinite(x)) throw new Error("a setting is not a number"); return String(x); };
  return [
    "[scenario]", 'name = "geometry-view"', `seed = ${n(Math.trunc(state.seed))}`, `start_utc = "${state.t}"`, "duration_s = 1", "rate_hz = 1",
    "[vessel]", `lat_deg = ${n(lat)}`, `lon_deg = ${n(state.lon)}`, "speed_kn = 0", "heading_deg = 0",
    "[receiver]", `systems = [${sys.map((s) => `"${s}"`).join(", ")}]`, `elevation_mask_deg = ${n(mask)}`, "max_used = 12",
    "[output]", 'sentences = ["GSA", "GSV"]', "",
  ].join("\n");
}

// Read the first epoch's GSV and GSA sentences. Strict: anything that is not a well-formed sentence is skipped.
export function parseInterimNmea(nmea) {
  const sats = new Map(); // "sys:id" -> sat
  const used = new Set();
  let dop = null, epochs = 0, lastGsv = "";
  for (const line of String(nmea).split(/\r?\n/)) {
    const m = /^\$([A-Z]{2})(GSV|GSA),([0-9A-Za-z.,]*)\*[0-9A-F]{2}$/.exec(line.trim());
    if (!m) continue;
    const [, talker, type, body] = m;
    const f = body.split(",");
    if (type === "GSV") {
      const sys = TALKER[talker];
      if (!sys) continue;
      // A second round of GSV for the same talker starting at message 1 is the next epoch: stop there.
      if (f[1] === "1" && lastGsv.includes(`|${talker}|`)) { epochs = 2; break; }
      if (f[1] === "1") lastGsv += `|${talker}|`;
      for (let k = 3; k + 3 < f.length; k += 4) {
        const id = parseInt(f[k], 10), el = parseInt(f[k + 1], 10), az = parseInt(f[k + 2], 10);
        if (!(id > 0) || !(el >= 0 && el <= 90) || !(az >= 0 && az <= 360)) continue;
        sats.set(`${sys}:${id}`, { sys, id, az: az % 360, el, used: false, blocked: null });
      }
    } else {
      if (lastGsv) { epochs = 2; break; } // a GSA after the GSV block opens the next epoch
      const sys = GSA_SYS[f[f.length - 1]];
      const [p, h, v] = [f[14], f[15], f[16]].map((x) => (/^\d+(\.\d+)?$/.test(x || "") ? Number(x) : null));
      if (dop === null && p !== null) dop = { gdop: null, pdop: p, hdop: h, vdop: v, tdop: null };
      if (!sys) continue;
      for (const x of f.slice(2, 14)) if (/^\d+$/.test(x)) used.add(`${sys}:${parseInt(x, 10)}`);
    }
  }
  const list = [...sats.values()];
  for (const s of list) s.used = used.has(`${s.sys}:${s.id}`);
  list.sort((a, b) => SYSTEMS.indexOf(a.sys) - SYSTEMS.indexOf(b.sys) || a.id - b.id);
  return { sats: list, dop, nUsed: list.filter((s) => s.used).length, epochs };
}

const INTERIM_MISSING = Object.freeze(["GDOP", "TDOP", "the error ellipse", "the Monte Carlo cloud", "LEO", "the skyline"]);

async function computeInterim(engine, state) {
  const notes = [];
  if (state.sys.includes("leo")) notes.push("LEO is not modelled by this engine build and was left out.");
  if (state.sky !== "none") notes.push("This engine build does not apply a skyline: the outline is drawn, but satellites behind it still count. The 0.36 engine applies it.");
  if (Math.abs(state.lat) > 89) notes.push("This engine build models places within 89° of the equator; the latitude was limited to that.");
  notes.push("Engine training-sky model: nominal constellations, not the real sky on that date. Angles are whole degrees and DOPs one decimal, as the receiver sentences carry them. The fix uses at most 12 satellites, chosen by signal strength.");
  const sys = state.sys.filter((s) => s !== "leo");
  if (!sys.length) return { ...empty("interim"), notes: ["Pick at least one constellation the engine can model."] };
  const raw = await engine.call(INTERIM_FN, buildInterimToml(state), Math.trunc(state.seed));
  let doc;
  try { doc = JSON.parse(raw); } catch { throw new Error("the engine's reply was not JSON"); }
  if (!doc || typeof doc.nmea !== "string") throw new Error("the engine's reply has no sentences");
  const r = parseInterimNmea(doc.nmea);
  return {
    backend: "interim", source: `Kshana engine ${engine.version || ""} · training-sky model (nmea_training)`.replace("  ", " "),
    sats: r.sats, nUsed: r.nUsed, dop: r.dop || { gdop: null, pdop: null, hdop: null, vdop: null, tdop: null }, dopDigits: 1,
    ellipse: null, cloud: null, missing: [...INTERIM_MISSING], notes,
  };
}

// ---------------------------------------------------------------- 0.36 backend: geometry()
// The request the view sends. The engine validates it again; nothing here is free text.
export function buildGeometryRequest(state, skyline) {
  return {
    lat_deg: state.lat, lon_deg: state.lon, height_m: state.h, epoch_utc: state.t,
    systems: state.sys.slice(), elevation_mask_deg: state.mask,
    skyline: skyline.map((p) => ({ az_deg: p.az, el_deg: p.el })),
    sigma_uere_m: state.sigma, monte_carlo: { seed: Math.trunc(state.seed), samples: Math.trunc(state.n) },
  };
}

// Read the engine's geometry reply into the normalised result. Every field is checked; a malformed reply
// throws rather than drawing something the engine did not say.
export function parseGeometryResult(doc, version = "") {
  if (!doc || typeof doc !== "object") throw new Error("the engine's geometry reply is not an object");
  const sats = (Array.isArray(doc.satellites) ? doc.satellites : []).flatMap((s) => {
    if (!s || !SYSTEMS.includes(s.system)) return [];
    const az = num(s.az_deg), el = num(s.el_deg);
    if (az === null || el === null) return [];
    return [{ sys: s.system, id: typeof s.id === "string" || typeof s.id === "number" ? String(s.id).slice(0, 16) : "?", az: ((az % 360) + 360) % 360, el, used: s.used === true, blocked: s.blocked_by === "skyline" || s.blocked_by === "mask" ? s.blocked_by : null }];
  });
  const d = doc.dop && typeof doc.dop === "object" ? doc.dop : {};
  const dop = { gdop: num(d.gdop), pdop: num(d.pdop), hdop: num(d.hdop), vdop: num(d.vdop), tdop: num(d.tdop) };
  const e = doc.horizontal_ellipse;
  const ellipse = e && num(e.semi_major_m) !== null && num(e.semi_minor_m) !== null && num(e.orientation_deg) !== null
    ? { semiMajorM: e.semi_major_m, semiMinorM: e.semi_minor_m, orientationDeg: e.orientation_deg, sigmaM: num(doc.sigma_uere_m) } : null;
  const mc = doc.monte_carlo;
  const pts = mc && Array.isArray(mc.points_enu_m) ? mc.points_enu_m.filter((p) => Array.isArray(p) && p.length >= 2 && p.slice(0, 3).every((x) => num(x) !== null)).slice(0, 5000) : null;
  const cloud = pts ? { points: pts, seed: num(mc.seed), samples: pts.length, rmsHorizontalM: num(mc.rms_horizontal_m), rmsVerticalM: num(mc.rms_vertical_m) } : null;
  const notes = (Array.isArray(doc.notes) ? doc.notes : []).filter((x) => typeof x === "string").map((x) => x.slice(0, 400)).slice(0, 10);
  return { backend: "geometry", source: `Kshana engine ${version} · geometry`.trim(), sats, nUsed: sats.filter((s) => s.used).length, dop, dopDigits: 2, ellipse, cloud, missing: [], notes };
}

async function computeGeometry(engine, state, skyline) {
  const raw = await engine.call("geometry", JSON.stringify(buildGeometryRequest(state, skyline)));
  let doc;
  try { doc = JSON.parse(raw); } catch { throw new Error("the engine's reply was not JSON"); }
  return parseGeometryResult(doc, engine.version || "");
}

function empty(backend) {
  return { backend, source: "", sats: [], nUsed: 0, dop: { gdop: null, pdop: null, hdop: null, vdop: null, tdop: null }, dopDigits: 2, ellipse: null, cloud: null, missing: [], notes: [] };
}

// ---------------------------------------------------------------- the adapter the view uses
// engine: the object lib/engine-page.mjs getEngine() resolves to ({available, version, has(fn), call(fn, ...args)}).
export function createGeometryAdapter(engine) {
  const backend = chooseBackend(engine);
  return {
    backend,
    systems: () => systemsFor(backend),
    // The skyline outline for a named preset: the engine's when it has presets, the illustrative one otherwise.
    async skyline(name) {
      if (name === "none" || name === "custom" || !Object.hasOwn(INTERIM_SKYLINES, name)) return { points: [], source: "" };
      if (backend === "geometry") {
        const doc = JSON.parse(await engine.call("skyline_preset", name));
        const pts = Array.isArray(doc && doc.points) ? doc.points.map((p) => ({ az: num(p && p.az_deg), el: num(p && p.el_deg) })).filter((p) => p.az !== null && p.el !== null) : [];
        return { points: normalise(pts), source: "engine preset" };
      }
      return { points: INTERIM_SKYLINES[name].map((p) => ({ ...p })), source: "illustrative outline" };
    },
    async compute(state, skyline = []) {
      if (backend === "geometry") return computeGeometry(engine, state, skyline);
      if (backend === "interim") return computeInterim(engine, state);
      return { ...empty("none"), notes: ["The engine is not running in this browser, so there is nothing to show. The engine is WebAssembly; a browser or host that blocks it cannot run this view."] };
    },
  };
}
