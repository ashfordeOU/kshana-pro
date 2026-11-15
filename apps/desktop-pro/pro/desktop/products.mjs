// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: the Pro products in the Desktop (0.42 U27): receiver scorecards (E3),
// area trust from the area console (F1, GET /area/picture), the timing fleet (T2,
// GET /timing/assessment) and multi-source trust (F3, the pro-multi-source-trust kind run on
// this machine). Pure models of the documents the Pro crate writes; every label and limit is
// carried as given. Tested in products.test.mjs.
import { posOf } from "./fleet.mjs";

const TONE = { TRUSTED: "ok", DEGRADED: "warn", UNTRUSTED: "bad", UNKNOWN: "none" };

// ------------------------------------------------------------------ E3 receiver scorecard
/** A receiver scorecard (kshana-pro-receiver-scorecard/1) as the view draws it. */
export function scorecardModel(card) {
  if (!card || card.format !== "kshana-pro-receiver-scorecard/1") throw new Error("not a receiver scorecard");
  const lv = card.levels || {};
  return {
    receiver: card.receiver || {}, campaign: card.campaign || {}, label: String(card.label || ""),
    framework: String(lv.framework || ""), wording: String(lv.wording || ""),
    highest: lv.highest_supported ?? null,
    levels: (lv.levels || []).map((l) => ({ level: l.level, summary: String(l.summary || ""), evidence: String(l.evidence || ""), tone: l.evidence === "supported" ? "ok" : l.evidence === "not supported" ? "bad" : "none", met: l.cases_met ?? 0, notMet: l.cases_not_met ?? 0, notAssessed: l.cases_not_assessed ?? 0 })),
    totals: card.totals || {},
    cases: (card.cases || []).map((c) => ({ id: c.id, title: c.title, threat: (c.threat || []).join(", "), availability: c.fix_availability, maxError: c.max_error_m,
      outcomes: (c.events || []).map((e) => e.monitor_outcome), misleading: (c.events || []).reduce((n, e) => n + (e.misleading_unwarned || 0), 0) })),
    cardCases: card.cases || [],
    limits: (card.limits || []).map(String),
  };
}

// ------------------------------------------------------------------ F1 area trust
/**
 * The area trust picture (kshana-pro-area-picture/1) for the map: each area with its state now,
 * its timeline as fractions of the picture's span, its events, and a position only where the
 * area's sources include Stations of the fleet that reported one (their centre). Emitter
 * positions are never shown (F2): the picture withholds them and so does this model.
 */
export function areaModel(picture, fleet) {
  if (!picture || !Array.isArray(picture.areas)) throw new Error("not an area picture");
  const stations = new Map(((fleet && fleet.stations) || []).map((s) => [String(s.id), posOf(s.health)]));
  const g0 = Math.min(...picture.areas.map((a) => a.first_g ?? Infinity)), g1 = Math.max(...picture.areas.map((a) => a.last_g ?? -Infinity));
  const span = Number.isFinite(g0) && Number.isFinite(g1) ? Math.max(g1 - g0 + 1, 1) : 1;
  const areas = picture.areas.map((a) => {
    const ps = Object.keys(a.sources || {}).map((id) => stations.get(id)).filter(Boolean);
    const pos = ps.length ? { lat: ps.reduce((s, p) => s + p.lat, 0) / ps.length, lon: ps.reduce((s, p) => s + p.lon, 0) / ps.length } : null;
    const current = String(a.current || "UNKNOWN");
    return {
      id: String(a.id), name: String(a.name || a.id), tenant: a.tenant, current, tone: TONE[current] || "none", pos, placedFrom: ps.length,
      timeline: (a.timeline || []).map(([g, f, len]) => ({ from: (g - g0) / span, width: len / span, state: f, tone: TONE[f] || "none" })),
      events: (a.events || []).map((e) => ({ id: e.id, worst: e.worst, tone: TONE[e.worst] || "none", duration: e.duration_s, basis: e.basis, corroborated: !!e.corroborated, sources: e.sources || [] })),
    };
  });
  const rank = { UNTRUSTED: 0, DEGRADED: 1, UNKNOWN: 2, TRUSTED: 3 };
  areas.sort((x, y) => (rank[x.current] ?? 2) - (rank[y.current] ?? 2) || x.name.localeCompare(y.name));
  const pos = picture.positions || {};
  return {
    title: String(picture.title || "Area trust"), label: String(picture.label || ""), areas, wideArea: (picture.wide_area || []).length,
    positions: `Emitter positions: ${pos.emitter_positions || "not shown"}. ${pos.reason || ""}`.trim(),
    worst: areas.length ? areas[0].current : "UNKNOWN",
  };
}

// ------------------------------------------------------------------ T2 timing fleet
const CLOCK_TONE = { ok: "ok", watch: "warn", act: "bad", breached: "bad" };
/** "2 h 05 min" for seconds. */
export function durText(s) {
  if (!Number.isFinite(s)) return "beyond the horizon";
  if (s <= 0) return "now";
  const h = Math.floor(s / 3600), m = Math.round((s % 3600) / 60);
  return h ? `${h} h ${String(m).padStart(2, "0")} min` : `${m} min`;
}
/** The timing fleet assessment (GET /timing/assessment), clocks in the assessment's own order (earliest warning first). */
export function timingModel(a) {
  if (!a || !Array.isArray(a.clocks)) throw new Error("not a timing fleet assessment");
  const by = new Map(a.clocks.map((c) => [c.id, c]));
  const order = Array.isArray(a.order) && a.order.length ? a.order : a.clocks.map((c) => c.id);
  const clocks = order.map((id) => by.get(id)).filter(Boolean).map((c) => {
    const warns = (c.breaches || []).map((b) => b.t_warn_s).filter(Number.isFinite);
    const warn = warns.length ? Math.min(...warns) : null;
    return { id: c.id, site: c.site || "", vendor: [c.vendor, c.model].filter(Boolean).join(" "), status: c.status, tone: CLOCK_TONE[c.status] || "none", warn, warnText: warn === null ? "no limit reached within the horizon" : durText(warn), limits: (c.breaches || []).map((b) => b.name) };
  });
  return { title: String(a.title || a.fleet || "Timing fleet"), label: String(a.label || ""), horizon: a.horizon_s, counts: a.counts || {}, clocks };
}

// ------------------------------------------------------------------ F3 multi-source trust
/**
 * The trust evidence of a pro-multi-source-trust run (kshana-pro-trust-evidence/1): the
 * hash-chained switch records as runs of one chosen source (state trusted, holdover or none),
 * with the epoch counts by state and by source as the evidence states them.
 */
export function trustRunsModel(result) {
  const r = typeof result === "string" ? JSON.parse(result) : result;
  const ev = r && r.format === "kshana-pro-trust-evidence/1" ? r : r && r.evidence;
  if (!ev || ev.format !== "kshana-pro-trust-evidence/1") throw new Error("not multi-source trust evidence");
  const sw = (ev.switches || []).slice().sort((a, b) => a.t_s - b.t_s);
  const state = (src) => (src === "holdover" ? "holdover" : src === "none" ? "none" : "trusted");
  const runs = sw.map((s, i) => ({ source: String(s.to), from: s.t_s, to: i + 1 < sw.length ? sw[i + 1].t_s : null, reason: String(s.reason || ""), state: state(String(s.to)) }));
  const c = ev.counts || {};
  return { epochs: ev.epochs || 0, counts: { trusted: c.trusted || 0, holdover: c.holdover || 0, none: c.none || 0 }, bySource: c.by_source || [], runs, switches: Math.max(runs.length - 1, 0), label: String(ev.label || "") };
}
