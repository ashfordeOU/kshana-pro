// SPDX-License-Identifier: AGPL-3.0-only
// The geometry lane of the Maritime trust view (0.36; docs/MARITIME-TRUST.md "Studio view specification",
// item 6). Reads what the engine writes beside the score: per epoch `geometry` (status, reasons, n_sats, the
// DOPs, the largest azimuth gap, the age of the sky, the reported HDOP and whether it disagrees with the sky),
// `degradation` and `explained_by_geometry`; at the top of a batch result `geometry_model` (thresholds, hold,
// provenance, what the geometry can explain) and `geometry_summary`. Live JSON lines carry the per-epoch fields
// from schema 1.3. Every field is checked; a run from an engine older than 0.36 has none, and the view says so
// rather than drawing a good sky. The geometry never changes the score: this module only reads and labels.
// Pure; tested in trust-geometry.test.mjs.

export const GEO_STATUSES = Object.freeze(["good", "degraded", "unknown"]);
export const DEGRADATIONS = Object.freeze(["none", "geometry", "inconsistency", "geometry-and-inconsistency"]);
// Plain words for the reasons the engine gives (src/receiver_trust/geometry.rs).
export const GEO_REASONS = Object.freeze({
  "few-satellites": "fewer satellites than the minimum",
  "high-pdop": "PDOP above its threshold",
  "high-hdop": "HDOP above its threshold",
  "sky-blocked": "the tracked satellites leave a wide gap in azimuth (one side of the sky blocked)",
});
export const UNKNOWN_TEXT = "geometry unknown: no GSV or GSA in the log";
export const DEGRADATION_TEXT = Object.freeze({
  none: "no degradation",
  geometry: "geometry: the sky, not the signal",
  inconsistency: "inconsistency: attack-like, not explained by the geometry",
  "geometry-and-inconsistency": "geometry and inconsistency: a poor sky and an attack-like check together",
});
// The order the threshold fields are shown in, with their meaning.
export const THRESHOLDS = Object.freeze([
  ["pdop_max", "PDOP above which the geometry is degraded"],
  ["hdop_max", "HDOP above which the geometry is degraded"],
  ["min_sats", "fewest satellites for a geometry that is not degraded"],
  ["azimuth_gap_deg", "azimuth gap, degrees, at or above which the sky is partly blocked"],
  ["hdop_report_tol", "largest reported-versus-sky HDOP difference before the two disagree"],
  ["hold_s", "how long sky positions and GSA count after they arrive, s"],
]);

const isNum = (x) => typeof x === "number" && Number.isFinite(x);
const numOrNull = (x) => (isNum(x) ? x : null);
const str = (x, max = 2000) => (typeof x === "string" ? x.slice(0, max) : "");

// One epoch's `geometry` object, normalised; null when the epoch has none (an engine before 0.36).
// A status this page does not know is read as unknown, never as good, and flagged so the page can count it.
export function readEpochGeometry(g) {
  if (!g || typeof g !== "object" || Array.isArray(g)) return null;
  const known = GEO_STATUSES.includes(g.status);
  return {
    status: known ? g.status : "unknown",
    oddStatus: known ? null : str(String(g.status), 40),
    reasons: (Array.isArray(g.reasons) ? g.reasons : []).filter((r) => typeof r === "string").slice(0, 10).map((r) => r.slice(0, 40)),
    basis: ["sky", "reported", "none"].includes(g.basis) ? g.basis : null,
    nSats: isNum(g.n_sats) && g.n_sats >= 0 ? Math.round(g.n_sats) : null,
    pdop: numOrNull(g.pdop), hdop: numOrNull(g.hdop), vdop: numOrNull(g.vdop),
    gapDeg: numOrNull(g.max_azimuth_gap_deg),
    ageS: numOrNull(g.age_s),
    reportedHdop: numOrNull(g.reported_hdop),
    mismatch: g.hdop_report_mismatch === true,
  };
}

// `degradation`: one of DEGRADATIONS, else null (absent or unknown).
export const readDegradation = (d) => (DEGRADATIONS.includes(d) ? d : null);
// `explained_by_geometry`: monitor names.
export const readExplained = (a) => (Array.isArray(a) ? a.filter((m) => typeof m === "string").slice(0, 40).map((m) => m.slice(0, 40)) : []);

// A batch result's `geometry_model`: thresholds with their provenance, and what the geometry can explain.
export function readGeometryModel(m) {
  if (!m || typeof m !== "object" || Array.isArray(m)) return null;
  const thresholds = THRESHOLDS.filter(([k]) => isNum(m[k])).map(([k, what]) => ({ key: k, value: m[k], what }));
  const prov = m.provenance && typeof m.provenance === "object" && !Array.isArray(m.provenance) ? m.provenance : {};
  const provenance = Object.keys(prov).filter((k) => typeof prov[k] === "string").sort().slice(0, 20).map((k) => ({ key: k.slice(0, 40), text: str(prov[k]) }));
  return { thresholds, provenance, explains: str(m.explains) };
}

// A batch result's `geometry_summary` (or a stream reply's summary.geometry): epoch counts.
export function readGeometrySummary(s) {
  if (!s || typeof s !== "object" || Array.isArray(s)) return null;
  const n = (k) => (isNum(s[k]) && s[k] >= 0 ? Math.round(s[k]) : null);
  return { good: n("good"), degraded: n("degraded"), unknown: n("unknown"), geometry: n("attributed_geometry"), inconsistency: n("attributed_inconsistency"), both: n("attributed_both"), firstDegradedS: numOrNull(s.first_degraded_t_s) };
}

// Counts over the epochs the view holds (a stream has no summary of its own).
export function countGeometry(epochs) {
  const c = { good: 0, degraded: 0, unknown: 0, missing: 0, none: 0, geometry: 0, inconsistency: 0, "geometry-and-inconsistency": 0, odd: 0 };
  for (const e of epochs) {
    if (!e.geometry) c.missing++; else c[e.geometry.status]++;
    if (e.geometry && e.geometry.oddStatus) c.odd++;
    if (e.degradation) c[e.degradation]++;
  }
  return c;
}

// The strip keys for an epoch: the status (or "missing" for an older engine) and the cause.
export const statusKey = (e) => (e.geometry ? e.geometry.status : "missing");
export const causeKey = (e) => e.degradation || "missing";

const fmt = (x, d) => (x == null ? "not stated" : x.toFixed(d));

// What the panel shows for the epoch under the cursor: a heading and rows of label and value.
export function geometryAt(e) {
  const g = e.geometry;
  if (!g) return { heading: "No geometry in this run (an engine before 0.36 writes none).", tone: "missing", rows: [] };
  if (g.status === "unknown") return { heading: g.oddStatus ? `Geometry unknown (the file says "${g.oddStatus}", which this page does not know).` : `${UNKNOWN_TEXT[0].toUpperCase()}${UNKNOWN_TEXT.slice(1)}.`, tone: "unknown", rows: rowsOf(g, e) };
  const why = g.reasons.map((r) => (Object.hasOwn(GEO_REASONS, r) ? GEO_REASONS[r] : r));
  return { heading: g.status === "good" ? "Geometry good." : `Geometry degraded: ${why.join("; ") || "no reason stated"}.`, tone: g.status, rows: rowsOf(g, e) };
}
function rowsOf(g, e) {
  const rows = [
    ["Satellites", g.nSats == null ? "not stated" : String(g.nSats)],
    ["PDOP", fmt(g.pdop, 2)], ["HDOP", fmt(g.hdop, 2)], ["VDOP", fmt(g.vdop, 2)],
    ["Largest azimuth gap", g.gapDeg == null ? "not stated" : `${Math.round(g.gapDeg)}°`],
    ["Age of the sky", g.ageS == null ? "not stated" : `${g.ageS.toFixed(1)} s`],
  ];
  if (g.reportedHdop != null) rows.push(["HDOP the receiver reports", g.reportedHdop.toFixed(1) + (g.mismatch ? " (disagrees with the sky it reports: nothing is put down to geometry)" : "")]);
  if (g.basis) rows.push(["From", { sky: "the satellites the log reports (GSV)", reported: "the receiver's stated DOP (GSA)", none: "nothing" }[g.basis]]);
  if (e.degradation) rows.push(["Degradation put down to", DEGRADATION_TEXT[e.degradation]]);
  return rows;
}

// A short phrase for the epoch's spoken description.
export function describeGeometry(e) {
  const g = e.geometry;
  if (!g) return "";
  if (g.status === "unknown") return UNKNOWN_TEXT;
  const d = e.degradation && e.degradation !== "none" ? `, ${DEGRADATION_TEXT[e.degradation].split(":")[0]}` : "";
  return `geometry ${g.status}${g.hdop != null ? ` (HDOP ${g.hdop.toFixed(1)})` : ""}${d}`;
}
