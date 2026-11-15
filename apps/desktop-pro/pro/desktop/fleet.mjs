// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: the Station fleet screen (0.42 U6, design concept B), built on the Pro
// service's Station fleet (GET /fleet/stations: every Station of the tenant with its last
// health report and state, and the licence's Station limit). The model is pure and tested;
// the view draws it with the map maths of geo.mjs. Nothing is invented: a Station without a
// reported position is listed, not placed; a trust the Station did not report is "not reported".
import { fitView, PROJECTION, latLonText, utcText } from "./geo.mjs";

const RANK = { stale: 0, untrusted: 1, degraded: 2, registered: 3, live: 4, retired: 5 };
export const TONE = { live: "ok", stale: "bad", registered: "warn", retired: "none" };
const TRUST_STATES = { TRUSTED: "ok", DEGRADED: "warn", UNTRUSTED: "bad", UNKNOWN: "none" };

/** "3 min ago" for an age in seconds. */
export function ageText(s) {
  if (!Number.isFinite(s)) return "never";
  if (s < 90) return `${Math.max(0, Math.round(s))} s ago`;
  if (s < 5400) return `${Math.round(s / 60)} min ago`;
  if (s < 172800) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}

/** A Station's trust from its health report: { value (0..1 or null), state, tone, text }. */
export function trustOf(health) {
  const hl = health && typeof health === "object" ? health : {};
  const n = Number(hl.trust ?? hl.trust_score ?? hl.score);
  const st = String(hl.trust_state || hl.state || "").toUpperCase();
  if (Number.isFinite(n) && n >= 0 && n <= 1) {
    const tone = n >= 0.8 ? "ok" : n >= 0.5 ? "warn" : "bad";
    return { value: n, state: tone === "ok" ? "TRUSTED" : tone === "warn" ? "DEGRADED" : "UNTRUSTED", tone, text: `${Math.round(n * 100)} %` };
  }
  if (TRUST_STATES[st]) return { value: null, state: st, tone: TRUST_STATES[st], text: st.toLowerCase() };
  return { value: null, state: "UNKNOWN", tone: "none", text: "not reported" };
}

/** The position a Station reported ({lat, lon} or null). */
export function posOf(health) {
  const hl = health && typeof health === "object" ? health : {};
  const lat = Number(hl.lat ?? hl.lat_deg), lon = Number(hl.lon ?? hl.lon_deg);
  return Number.isFinite(lat) && Number.isFinite(lon) && Math.abs(lat) <= 90 && Math.abs(lon) <= 180 ? { lat, lon } : null;
}

/**
 * The fleet as the screen draws it.
 *   fleet: the GET /fleet/stations answer ({ limit, active, stations: [...] })
 *   now:   Unix seconds
 * Returns { limit, active, capacity, counts, alarms, stations: [...] } with stations worst first.
 */
export function fleetModel(fleet, now) {
  const list = fleet && Array.isArray(fleet.stations) ? fleet.stations : [];
  const counts = { live: 0, stale: 0, registered: 0, retired: 0 };
  const stations = list.map((s) => {
    const state = String(s.state || (s.retired ? "retired" : "registered"));
    if (state in counts) counts[state]++;
    const trust = trustOf(s.health);
    const pos = posOf(s.health);
    const age = s.last_seen_unix ? now - s.last_seen_unix : NaN;
    const alarms = [];
    if (state === "stale") alarms.push("No health report for more than two minutes");
    if (state !== "retired" && trust.tone === "bad") alarms.push(`Trust ${trust.text}`);
    else if (state !== "retired" && trust.tone === "warn") alarms.push(`Trust degraded (${trust.text})`);
    const rank = state === "live" && trust.tone === "bad" ? RANK.untrusted : state === "live" && trust.tone === "warn" ? RANK.degraded : RANK[state] ?? 3;
    const health = s.health && typeof s.health === "object" && !Array.isArray(s.health) ? Object.entries(s.health).filter(([, v]) => v === null || typeof v !== "object").map(([k, v]) => [k, String(v)]) : [];
    return { id: String(s.id), name: String(s.name || s.id), tenant: s.tenant || "", url: s.url || "", state, tone: TONE[state] || "none", trust, pos, lastSeen: s.last_seen_unix || null, ageText: s.last_seen_unix ? ageText(age) : "never reported", alarms, rank, health };
  });
  stations.sort((a, b) => a.rank - b.rank || a.name.localeCompare(b.name));
  const limit = Number(fleet && fleet.limit) || 0;
  const active = Number(fleet && fleet.active) || stations.filter((s) => s.state !== "retired").length;
  return {
    limit, active,
    capacity: limit ? `${active} of ${limit} Station${limit === 1 ? "" : "s"} in use` : `${active} Stations`,
    counts,
    alarms: stations.reduce((n, s) => n + s.alarms.length, 0),
    placed: stations.filter((s) => s.pos).length,
    stations,
  };
}

// ------------------------------------------------------------------ the view (DOM)
const SVGNS = "http://www.w3.org/2000/svg";
function svg(tag, attrs = {}, ...kids) {
  const n = document.createElementNS(SVGNS, tag);
  for (const [k, v] of Object.entries(attrs)) if (v !== undefined && v !== null) n.setAttribute(k, String(v));
  for (const c of kids) if (c) n.append(c.nodeType ? c : document.createTextNode(String(c)));
  return n;
}
export const toneVar = (t) => ({ ok: "var(--itg)", warn: "var(--nav)", bad: "var(--int)" })[t] || "var(--ink-3)";

/** The map of placed Stations, with the projection's name and a scale bar. */
export function stationMap(model, { w = 640, h = 380, selected, onSelect } = {}) {
  const placed = model.stations.filter((s) => s.pos);
  const v = fitView(placed.map((s) => s.pos), w, h, { pad: 36, minSpanM: 5000 });
  const root = svg("svg", { viewBox: `0 0 ${w} ${h}`, class: "pro-map", role: "img", "aria-label": `Map of ${placed.length} Stations with a reported position` });
  root.append(svg("rect", { x: 0, y: 0, width: w, height: h, class: "pro-map-ground" }));
  if (!v) {
    root.append(svg("text", { x: w / 2, y: h / 2, "text-anchor": "middle", class: "pro-map-note" }, "No Station has reported a position."));
    return root;
  }
  for (const s of placed) {
    const [x, y] = v.project(s.pos.lon, s.pos.lat);
    const g = svg("g", { class: `pro-pin ${s.id === selected ? "sel" : ""}`, tabindex: "0", role: "button", "aria-label": `${s.name}: ${s.state}, trust ${s.trust.text}` },
      svg("circle", { cx: x, cy: y, r: s.id === selected ? 9 : 7, fill: toneVar(s.trust.tone === "none" ? s.tone : s.trust.tone), stroke: toneVar(s.tone), "stroke-width": 3 }),
      svg("text", { x: x + 12, y: y + 4, class: "pro-pin-label" }, s.name));
    if (onSelect) { g.addEventListener("click", () => onSelect(s.id)); g.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onSelect(s.id); } }); }
    root.append(g);
  }
  root.append(svg("line", { x1: 16, y1: h - 18, x2: 16 + v.scale.px, y2: h - 18, class: "pro-scale" }), svg("text", { x: 16, y: h - 24, class: "pro-map-note" }, v.scale.label));
  root.append(svg("text", { x: w - 12, y: h - 12, "text-anchor": "end", class: "pro-map-note" }, PROJECTION));
  return root;
}

/** The fleet screen. `h` is ui.mjs's element helper; `st` holds the selection and the last read. */
export function fleetView(h, model, st, nav) {
  const sel = model.stations.find((s) => s.id === st.selected) || model.stations[0] || null;
  const chip = (text, tone) => h("span", { class: `state-chip ${tone}`, text });
  const select = (id) => { st.selected = id; nav.redraw(); };
  const counts = h("div", { class: "pro-counts" },
    ...[["live", "ok"], ["stale", "bad"], ["registered", "warn"], ["retired", "none"]].map(([k, t]) => h("div", { class: "pro-count" }, h("b", { text: String(model.counts[k]) }), chip(k.toUpperCase(), t))),
    h("div", { class: "pro-count" }, h("b", { text: String(model.alarms) }), chip("ALARMS", model.alarms ? "bad" : "ok")));
  const rows = model.stations.map((s) => h("tr", { class: s.id === (sel && sel.id) ? "sel" : null, tabindex: "0", onclick: () => select(s.id), onkeydown: (e) => { if (e.key === "Enter") select(s.id); } },
    h("td", {}, h("b", { text: s.name }), h("div", { class: "mono mut", text: s.id })),
    h("td", {}, chip(s.state.toUpperCase(), s.tone)),
    h("td", {}, h("span", { class: "trust-bar", style: `--v:${s.trust.value ?? 0};--c:${toneVar(s.trust.tone)}` }), h("span", { class: "mono", text: s.trust.text })),
    h("td", { class: "mono", text: s.ageText }),
    h("td", { text: s.alarms.length ? s.alarms.join("; ") : "—" })));
  const inspector = sel ? h("aside", { class: "card pro-inspector", "aria-label": "Station" },
    h("h2", { text: sel.name }),
    h("dl", { class: "lic-rows" },
      h("dt", { text: "Id" }), h("dd", { class: "mono", text: sel.id }),
      h("dt", { text: "State" }), h("dd", { text: sel.state }),
      h("dt", { text: "Trust" }), h("dd", { text: sel.trust.text }),
      h("dt", { text: "Position" }), h("dd", { text: sel.pos ? latLonText(sel.pos.lat, sel.pos.lon) : "not reported" }),
      h("dt", { text: "Last report" }), h("dd", { text: sel.lastSeen ? `${utcText(sel.lastSeen)} (${sel.ageText})` : "never" }),
      sel.url ? [h("dt", { text: "Address" }), h("dd", { class: "mono", text: sel.url })] : null,
      ...sel.health.slice(0, 12).flatMap(([k, v]) => [h("dt", { text: k }), h("dd", { class: "mono", text: v })])),
    sel.alarms.length ? h("p", { class: "lic-warning bad", role: "alert", text: sel.alarms.join(". ") }) : null) : null;
  return h("section", { class: "page pro-page", "data-pro": "fleet", "aria-labelledby": "fleet-h" },
    h("p", { class: "kicker", text: `Kshana Desktop Pro · ${model.capacity}` }),
    h("h1", { id: "fleet-h", text: "Station fleet" }),
    h("p", { class: "lede", text: "Every Station of your organisation's Kshana Pro service, with its last health report. Stations report to the service; this screen reads it." }),
    counts,
    h("div", { class: "pro-split" },
      h("div", { class: "pro-col" }, stationMap(model, { selected: sel && sel.id, onSelect: select }),
        model.placed < model.stations.length ? h("p", { class: "mut", text: `${model.stations.length - model.placed} Station(s) have not reported a position and are listed only.` }) : null,
        h("div", { class: "wrap" }, h("table", { class: "pro-table" }, h("thead", {}, h("tr", {}, ...["Station", "State", "Trust", "Last report", "Alarms"].map((t) => h("th", { text: t })))), h("tbody", {}, ...rows)))),
      inspector));
}
