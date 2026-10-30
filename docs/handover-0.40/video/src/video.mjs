// The 0.40 release video: one page, every scene a pure function of the time t (window.__render(t)).
// Numbers come from data/numbers.json (build_numbers.py, from the engine output committed at the tag that collect.py reads).
const NS = "http://www.w3.org/2000/svg";
const SVGT = new Set(["svg", "g", "rect", "line", "text", "circle", "path", "polyline", "polygon", "ellipse", "defs", "pattern", "tspan", "clipPath"]);
const h = (tag, attrs = {}, ...kids) => {
  const e = SVGT.has(tag) ? document.createElementNS(NS, tag) : document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) { if (k === "html") e.innerHTML = v; else e.setAttribute(k, v); }
  for (const c of kids.flat()) if (c != null) e.append(c.nodeType ? c : document.createTextNode(String(c)));
  return e;
};
const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
const ease = (x) => (x < 0.5 ? 2 * x * x : 1 - Math.pow(-2 * x + 2, 2) / 2);
const seg = (t, a, b) => clamp((t - a) / (b - a));
const fmt = (v, d = 0) => Number(v).toLocaleString("en-US", { minimumFractionDigits: d, maximumFractionDigits: d });
const N = await (await fetch("data/numbers.json")).json();
const q = new URLSearchParams(location.search);
const branded = q.get("brand") === "1";
const SQ = q.get("sq") === "1";
document.body.classList.toggle("sq", SQ);

// ---- chrome
const top = document.getElementById("top");
top.append(h("img", { src: "kshana-mark.svg" }), h("b", {}, "kshana"), ...(branded ? [h("span", { class: "by" }, "by Ashforde® OÜ")] : []), h("span", { class: "ver" }, "0.40"),
  h("span", { class: "right" }, "numbers: engine output committed at the tag"));
const foot = document.getElementById("foot");
foot.append(h("span", {}, "Advisory software, not type-approved navigation equipment and not a certification; the operator remains responsible. Synthetic inputs say so on screen."),
  h("span", { class: "prov" }, `tag ${N.engine.tag} · ${N.engine.commit.slice(0, 8)}`));

// ---- scene frame
const T = { title: [0, 5], thread: [5, 17.5], oracle: [17.5, 29], evid: [29, 36.5], clock: [36.5, 45], emit: [45, 57], bench: [57, 65], lang: [65, 73.5], ledger: [73.5, 79.5], close: [79.5, 84] };
export const DURATION = 84;
const scenes = [];
const scene = (id, build, upd) => {
  const el = h("section", { class: "scene", id });
  build(el);
  const inner = h("div", { class: "inner" });
  for (const k of ["justifyContent", "alignItems", "textAlign"]) { inner.style[k] = el.style[k]; el.style[k] = ""; }
  while (el.firstChild) inner.append(el.firstChild);
  el.append(inner);
  document.getElementById("scenes").append(el);
  scenes.push({ id, el, t0: T[id][0], t1: T[id][1], upd });
};
const head = (kick, title, chips = []) => h("div", { class: "head" }, h("div", {}, h("div", { class: "kicker" }, kick), h("h2", {}, title)), h("div", { class: "chips" }, ...chips));
const chip = (txt, cls = "") => h("span", { class: "chip " + cls }, txt);
const reveal = (el, p) => { el.style.opacity = p; el.style.transform = `translateY(${(1 - p) * 18}px)`; };
const stag = (t, t0, i, step = 0.35, dur = 0.5) => ease(seg(t, t0 + i * step, t0 + i * step + dur));
const cmd = (...x) => h("div", { class: "cmd" }, h("i", {}, "$ "), ...x);

// ---- 1 title
const TH = N.thread;
scene("title", (el) => {
  el.style.justifyContent = "center";
  el.append(h("div", { class: "kicker", id: "t-k" }, `Kshana ${N.engine.version} · released 29 Oct 2026`), h("h1", { id: "t-h" }, "From launch to entry,", h("br"), "every number labelled."),
    h("p", { class: "lede", id: "t-l" }, `A mission-level error budget, an evidence label on every figure, measured clocks, an emitter region, and ${N.langs.codes.length - 1} languages beside English.`),
    h("p", { class: "note", id: "t-n" }, "Open-source engine. Advisory software. Misses are shown as misses."));
}, (el, t) => { ["t-k", "t-h", "t-l", "t-n"].forEach((id, i) => reveal(el.querySelector("#" + id), stag(t, 0.2, i, 0.4, 0.7))); });

// ---- 2 mission thread
scene("thread", (el) => {
  el.append(head("0.40 · mission thread", `${TH.title.replace(" (ILLUSTRATIVE)", "")}: ${TH.phases.length} phases, one budget`, [chip("Modelled", "m"), chip("illustrative inputs", "s")]));
  const P = TH.phases, W = 1600, Hh = 400, X0 = 92, X1 = 1590, bw = (X1 - X0) / P.length, Y = (v) => 330 - ((Math.log10(Math.max(v, 1e-3)) + 3) / 7) * 300;
  const svg = h("svg", { viewBox: `0 0 ${W} ${Hh}`, width: "100%" });
  for (const v of [1e-3, 1e-1, 10, 1e3]) svg.append(h("line", { x1: X0, x2: X1, y1: Y(v), y2: Y(v), stroke: "rgba(255,255,255,.08)" }), h("text", { x: X0 - 10, y: Y(v) + 5, "text-anchor": "end", fill: "#6f7d9d", "font-size": 15 }, v >= 1 ? fmt(v) + " m" : v + " m"));
  const wk = TH.weakest.phase_id, lines = [];
  P.forEach((p, i) => {
    const x = X0 + i * bw, weak = p.id === wk;
    svg.append(h("rect", { x: x + 2, y: 24, width: bw - 4, height: 306, rx: 6, fill: weak ? "rgba(255,178,36,.10)" : i < 5 || i === 10 ? "rgba(53,208,255,.045)" : "rgba(166,240,74,.04)", stroke: weak ? "rgba(255,178,36,.6)" : "none" }));
    svg.append(h("text", { x: x + bw / 2, y: 356, "text-anchor": "middle", fill: weak ? "#ffb224" : "#eaf0ff", "font-size": 17, "font-weight": 600 }, p.id), h("text", { x: x + bw / 2, y: 378, "text-anchor": "middle", fill: "#6f7d9d", "font-size": 13 }, ...(p.kind.length > 15 ? [h("tspan", { x: x + bw / 2 }, p.kind.slice(0, p.kind.lastIndexOf("-") + 1)), h("tspan", { x: x + bw / 2, dy: 16 }, p.kind.slice(p.kind.lastIndexOf("-") + 1))] : [p.kind])));
    const s = TH.series[i], dur = Math.max(1, p.end_s - p.start_s);
    const pts = s.map(([tt, v]) => `${(x + 8 + ((tt - p.start_s) / dur) * (bw - 16)).toFixed(1)},${Y(v).toFixed(1)}`);
    const pl = h("polyline", { points: "", fill: "none", stroke: weak ? "#ffb224" : "#35d0ff", "stroke-width": 3 });
    svg.append(pl); lines.push([pl, pts]);
    if (p.requirement.max_pos_m) svg.append(h("line", { x1: x + 6, x2: x + bw - 6, y1: Y(p.requirement.max_pos_m), y2: Y(p.requirement.max_pos_m), stroke: "#ff5d6c", "stroke-width": 2, "stroke-dasharray": "7 5" }), h("text", { x: x + bw / 2, y: Y(p.requirement.max_pos_m) - 7, "text-anchor": "middle", fill: "#ff5d6c", "font-size": 13 }, `limit ${fmt(p.requirement.max_pos_m)} m`));
  });
  svg.append(h("text", { x: X0, y: 16, fill: "#6f7d9d", "font-size": 15 }, "1-sigma position, log scale · equal width per phase · Earth segment in blue, lunar segment in green"));
  el._lines = lines;
  el.append(h("div", { class: "panel" }, svg),
    h("div", { class: "tiles", style: "grid-template-columns:repeat(3,1fr)", id: "th-tiles" }, ...[[TH.phases.length, "phases, launch to entry, Earth and lunar segments", "var(--ink)"], [wk, `weakest link: margin ${TH.weakest.margin.toFixed(2)}, its stated limit over its peak error; set by ${TH.weakest.limiting_source}`, "var(--amber)"], [TH.kinds, `scenario kinds (from ${TH.kinds_from}), 4 of them lunar phases`, "var(--cyan)"]].map(([v, l, c]) => h("div", { class: "tile" }, h("b", { style: `color:${c}` }, v), h("span", {}, l)))),
    cmd("kshana scenarios/mission-thread-artemis-class.toml"),
    h("p", { class: "note" }, "Every number is MODELLED and every input is stated; the example's numbers are illustrative, not any agency's. Not affiliated with or endorsed by any agency."));
}, (el, t) => {
  const p = ease(seg(t, T.thread[0] + 0.6, T.thread[0] + 6)), n = el._lines.length;
  el._lines.forEach(([pl, pts], i) => { const f = clamp(p * n - i); pl.setAttribute("points", pts.slice(0, Math.max(0, Math.ceil(f * pts.length))).join(" ")); });
  el.querySelectorAll("#th-tiles .tile").forEach((r, i) => reveal(r, stag(t, T.thread[0] + 6.3, i, 0.5, 0.6)));
});

// ---- 3 the pre-registered comparisons
scene("oracle", (el) => {
  el.append(head("0.40 · checked against public trajectories", "Two misses, one met: published as they came out", [chip("Modelled", "m"), chip("not met · stated", "n")]));
  const C = N.c1, W = 760, Hh = 380, X = (hr) => 70 + (hr / 72) * (W - 110), Y = (km) => 330 - (km / 50) * 290;
  const svg = h("svg", { viewBox: `0 0 ${W} ${Hh}`, width: "100%" });
  svg.append(h("line", { x1: 70, x2: W - 40, y1: 330, y2: 330, stroke: "rgba(255,255,255,.25)" }), h("rect", { x: 70, y: Y(C.bar_km), width: W - 110, height: 330 - Y(C.bar_km), fill: "rgba(166,240,74,.10)" }),
    h("line", { x1: 70, x2: W - 40, y1: Y(C.bar_km), y2: Y(C.bar_km), stroke: "#a6f04a", "stroke-dasharray": "7 5", "stroke-width": 2 }), h("text", { x: W - 44, y: Y(C.bar_km) + 20, "text-anchor": "end", fill: "#a6f04a", "font-size": 16 }, `bar ${C.bar_km} km, fixed before the run`));
  for (let hr = 0; hr <= 72; hr += 12) svg.append(h("text", { x: X(hr), y: 356, "text-anchor": "middle", fill: "#6f7d9d", "font-size": 15 }, hr + " h"));
  const pts = [[0, 0], ...C.per_epoch].map(([hr, km]) => `${X(hr)},${Y(km)}`);
  svg.append(h("polyline", { id: "c1-line", points: "", fill: "none", stroke: "#ff5d6c", "stroke-width": 4 }));
  C.per_epoch.forEach(([hr, km], i) => svg.append(h("circle", { class: "c1-pt", cx: X(hr), cy: Y(km), r: 6, fill: "#ff5d6c", "data-i": i })));
  svg.append(h("text", { id: "c1-lab", x: X(72) - 6, y: Y(C.per_epoch[5][1]) - 14, "text-anchor": "end", fill: "#ff5d6c", "font-size": 22, "font-weight": 700 }, `${C.max_pos_km} km at 72 h`));
  el._pts = pts;
  const card = (id, title, big, sub, verdict, ok) => h("div", { class: "panel ocard", id, style: `border-color:${ok ? "rgba(166,240,74,.45)" : "rgba(255,93,108,.5)"}` },
    h("div", { style: "font:600 22px var(--sans)" }, title), h("div", { style: `font:700 44px/1.15 var(--sans);color:${ok ? "var(--lime)" : "var(--coral)"};margin:6px 0` }, big), h("div", { class: "note" }, sub), h("div", { style: "margin-top:8px" }, chip(verdict, ok ? "v" : "n")));
  el.append(h("div", { class: "cols" },
    h("div", { class: "panel" }, h("div", { style: "font:600 22px var(--sans);margin-bottom:6px" }, "Cislunar propagation against the public Artemis I trajectory, 72 h"), svg,
      h("div", { style: "display:flex;gap:12px;align-items:center;flex-wrap:wrap" }, chip(`not met: ${C.max_pos_km} km against ${C.bar_km} km · ${C.vel_mps} m/s against ${C.bar_mps} m/s`, "n")),
      h("p", { class: "note", style: "margin-top:8px" }, `Cause: small impulsive events in the reference that the model does not include; between them it matches the hourly states to a median of ${C.diag_median_m} m (a diagnostic, not the verdict).`)),
    h("div", { style: "display:flex;flex-direction:column;gap:18px" },
      card("o-a2", "Lunar propagator against the public CAPSTONE orbit, one revolution", `${N.a2.end_km} km`, `end position, against a bar of ${N.a2.bar_km} km; periapsis radius ${N.a2.peri_km} km against ${N.a2.peri_bar_km} km`, "not met", false),
      card("o-a1", "The same propagator against the public LRO trajectory, 24 h", `${N.a1.max_m} m`, `largest 3-D difference, against a bar of ${N.a1.bar_m} m; RMS radial ${N.a1.rms_m} m against ${N.a1.rms_bar_m} m`, "met", true))),
    h("p", { class: "note" }, "Bars pre-registered before each run; no bar loosened, no window re-chosen. The rows stay MODELLED: a met oracle tests the propagator, not a navigation filter or a budget."));
}, (el, t) => {
  const p = ease(seg(t, T.oracle[0] + 0.6, T.oracle[0] + 4)), n = el._pts.length, k = Math.max(1, Math.ceil(p * n));
  el.querySelector("#c1-line").setAttribute("points", el._pts.slice(0, k).join(" "));
  el.querySelectorAll(".c1-pt").forEach((c) => { c.style.opacity = Number(c.dataset.i) + 2 <= k ? 1 : 0; });
  el.querySelector("#c1-lab").style.opacity = p >= 0.999 ? 1 : 0;
  reveal(el.querySelector("#o-a2"), ease(seg(t, T.oracle[0] + 4.5, T.oracle[0] + 5.2))); reveal(el.querySelector("#o-a1"), ease(seg(t, T.oracle[0] + 6, T.oracle[0] + 6.7)));
});

// ---- 4 evidence labels
scene("evid", (el) => {
  el.append(head("0.40 · evidence labels", "Every figure and table says what it rests on", [chip("read from the ledger, not typed", "s")]));
  const ids = TH.evidence_ids;
  const js = h("div", { class: "cmd", style: "font-size:19px;line-height:1.55" }, h("span", { style: "color:var(--ink3)" }, "mission-thread-artemis-class.result.json"), "\n", '"evidence": {\n  "tier": ', h("b", { style: "color:var(--amber)" }, `"${TH.evidence_tier}"`), ",\n  \"rows\": [", ...ids.map((x, i) => h("span", { class: "eid" }, `${i % 4 === 0 ? "\n    " : " "}"${x}"${i < ids.length - 1 ? "," : ""}`)), "\n  ]\n}");
  const forms = [["result JSON", "an evidence key: the tier and the row ids"], ["chart SVG", "a metadata element and a line under the plot"], ["CSV table", "# evidence: comment lines"], ["signed evidence pack", "the tier inside config.json; `kshana evidence verify` reports it"]];
  el.append(h("div", { class: "cols" }, h("div", { class: "panel" }, js),
    h("div", { class: "panel" }, h("div", { class: "kicker" }, "Where the label goes"), ...forms.map(([a, b]) => h("div", { class: "form", style: "margin:14px 0" }, h("div", { style: "font:600 26px var(--sans)" }, a), h("div", { class: "note" }, b))),
      h("p", { class: "note", style: "margin-top:12px" }, "The tier is the weakest of the rows the run rests on. A test fails if a figure has no label or a label disagrees with the ledger."))),
    h("p", { class: "note" }, `Here: ${ids.length} ledger rows, all MODELLED, so the run is MODELLED. Ids are stable: VM- and eight hex digits.`));
}, (el, t) => { el.querySelectorAll(".eid").forEach((r, i) => { r.style.opacity = stag(t, T.evid[0] + 0.6, i, 0.18, 0.3); }); el.querySelectorAll(".form").forEach((r, i) => reveal(r, stag(t, T.evid[0] + 1.5, i, 0.6, 0.6))); });

// ---- 5 measured clocks + data partners
scene("clock", (el) => {
  el.append(head("0.40 · measured clocks and data partners", "Read real clock logs; check a partner's data, score nothing", [chip("Modelled", "m"), chip("not met · stated", "n")]));
  const fm = ["RINEX clock", "TimeLab", "TICC", "ptp4l", "phc2sys", "ts2phc", "chrony tracking", "chrony statistics"];
  const left = h("div", { class: "panel" }, h("div", { class: "kicker" }, `${fm.length} clock-log formats`), h("div", { style: "display:grid;grid-template-columns:1fr 1fr;gap:8px 18px;margin:14px 0" }, ...fm.map((f) => h("div", { class: "fmt", style: "font:500 24px var(--mono);color:var(--ink)" }, "▸ " + f))),
    h("p", { class: "note" }, "Anything outside a documented subset is refused with an error that names what was found. A fitted series can drive the holdover."),
    h("div", { id: "ck-miss", style: "margin-top:16px;display:flex;flex-direction:column;gap:10px" }, h("div", { style: "font:700 24px var(--mono);color:var(--coral)" }, `F1(a) · NOT MET on ${N.clock.f1a} records`), h("div", { style: "font:700 24px var(--mono);color:var(--coral)" }, `F2 · NOT MET on ${N.clock.f2}`), h("div", { class: "note" }, "Both pinned by tests; the fit stays MODELLED.")));
  const right = h("div", { class: "panel" }, h("div", { class: "cmd", style: "margin-bottom:14px" }, h("i", {}, "$ "), "kshana partner check submission.toml"),
    h("div", { class: "note", style: "white-space:pre;font-size:17px;line-height:1.5" }, 'schema = "kshana.partner-submission/1"\n[device]   kind = "clock"   class = "rubidium"\n[data]     format = "timelab"   sha256 = "7fed33cd…"\n[rows]     targets = [ a MODELLED ledger row ]\n[licence]  spdx = "CC-BY-4.0"'),
    h("div", { id: "pc-say", style: "font:700 26px/1.4 var(--sans);margin-top:16px" }, "Checks the format, the metadata and the data.", h("br"), h("span", { style: "color:var(--amber)" }, "Scores nothing. Validates nothing.")),
    h("div", { style: "margin-top:10px" }, chip("synthetic example submission", "s")));
  el.append(h("div", { class: "cols", style: `grid-template-columns:${SQ ? "1fr" : "1fr 1fr"}` }, left, right));
}, (el, t) => { el.querySelectorAll(".fmt").forEach((r, i) => reveal(r, stag(t, T.clock[0] + 0.5, i, 0.22, 0.4))); reveal(el.querySelector("#ck-miss"), ease(seg(t, T.clock[0] + 3, T.clock[0] + 3.7))); reveal(el.querySelector("#pc-say"), ease(seg(t, T.clock[0] + 4.5, T.clock[0] + 5.2))); });

// ---- 6 emitter region
const EM = N.emitter;
scene("emit", (el) => {
  el.append(head("0.40 · emitter region", "A region and an ellipse, never a point", [chip("Modelled", "m"), chip("synthetic reports", "s")]));
  const c = EM.centroid, kx = 111.32 * Math.cos((c.lat * Math.PI) / 180), ky = 110.57, S = 760, sc = S / 44;
  const P = ([lon, lat]) => [S / 2 + (lon - c.lon) * kx * sc, S / 2 - (lat - c.lat) * ky * sc];
  const ring = (r) => r.map((p) => P(p).map((v) => v.toFixed(1)).join(",")).join(" ");
  const svg = h("svg", { viewBox: `0 0 ${S} ${S}`, width: "100%", style: "max-height:100%" });
  for (let k = -20; k <= 20; k += 10) svg.append(h("line", { x1: S / 2 + k * sc, x2: S / 2 + k * sc, y1: 0, y2: S, stroke: "rgba(255,255,255,.06)" }), h("line", { y1: S / 2 + k * sc, y2: S / 2 + k * sc, x1: 0, x2: S, stroke: "rgba(255,255,255,.06)" }));
  const lay = [["95", "rgba(107,140,255,.22)", "#6b8cff"], ["90", "rgba(53,208,255,.25)", "#35d0ff"], ["50", "rgba(166,240,74,.35)", "#a6f04a"]];
  lay.forEach(([k, f, s]) => { const g = h("g", { class: "reg" }); for (const poly of EM.geom["estimated_emitter_region_" + k].coordinates) g.append(h("polygon", { points: ring(poly[0]), fill: f, stroke: s, "stroke-width": 2 })); svg.append(g); });
  const eg = h("g", { id: "ell" }, h("polygon", { points: ring(EM.geom.uncertainty_ellipse_95.coordinates[0]), fill: "none", stroke: "#ffb224", "stroke-width": 3, "stroke-dasharray": "9 6" }));
  const [cx, cy] = P(EM.geom.estimated_emitter_centroid.coordinates);
  eg.append(h("circle", { cx, cy, r: 5, fill: "#ffb224" }));
  svg.append(eg, h("line", { x1: 30, x2: 30 + 10 * sc, y1: S - 30, y2: S - 30, stroke: "#a7b4d2", "stroke-width": 3 }), h("text", { x: 30, y: S - 42, fill: "#a7b4d2", "font-size": 20 }, "10 km"));
  const leg = (col, txt) => h("div", { class: "eleg", style: "display:flex;gap:12px;align-items:center;font:500 22px var(--mono);margin:8px 0" }, h("i", { style: `width:18px;height:18px;border-radius:4px;background:${col};display:block` }), txt);
  const reg = Object.fromEntries(EM.regions.map((r) => [Math.round(r.confidence * 100), r.area_km2]));
  const right = h("div", { class: "panel" },
    h("div", { class: "kicker" }, "One session · synthetic ADS-B"), h("p", { class: "note", style: "margin:6px 0 10px" }, `${EM.affected_distinct} of ${EM.observed_distinct} aircraft affected over ${EM.duration_min} min, ${fmt(EM.reports)} reports`),
    leg("#a6f04a", `50 % region · ${fmt(reg[50])} km²`), leg("#35d0ff", `90 % region · ${fmt(reg[90])} km²`), leg("#6b8cff", `95 % region · ${fmt(reg[95])} km²`), leg("#ffb224", `95 % ellipse · ${EM.ellipse.semi_major_km} × ${EM.ellipse.semi_minor_km} km`),
    h("p", { class: "note", style: "margin-top:10px" }, "The centroid is always read with its ellipse; it is not a position fix. Aggregate only; sessions with too few aircraft are withheld."),
    h("div", { id: "jt25", class: "panel", style: "margin-top:14px;border-color:rgba(255,178,36,.5)" }, h("div", { style: "font:600 21px var(--sans)" }, "Compared once with the Jammertest 2025 transmitter sites"),
      ...EM.jt2025.map((j) => h("div", { style: "font:700 22px var(--mono);color:var(--amber);margin-top:6px" }, `${j.source} · ${j.outcome} (${j.scheduled} scheduled sessions)`)), h("div", { class: "note" }, "Too few scheduled sessions in the open data, so the row stays MODELLED.")));
  el.append(h("div", { class: "cols", style: "grid-template-columns:auto 1fr;align-items:stretch" }, h("div", { class: "panel", style: "aspect-ratio:1;height:620px;padding:10px" }, svg), right),
    cmd("kshana interference-map locate reports.csv --source adsb … --out out", SQ ? "" : "     # GeoJSON, summary, sessions CSV, SVG, each with its evidence label"));
}, (el, t) => {
  el.querySelectorAll(".reg").forEach((r, i) => { r.style.opacity = stag(t, T.emit[0] + 0.6, i, 0.7, 0.6); });
  el.querySelector("#ell").style.opacity = ease(seg(t, T.emit[0] + 3, T.emit[0] + 3.6));
  el.querySelectorAll(".eleg").forEach((r, i) => reveal(r, stag(t, T.emit[0] + 0.6, i, 0.7, 0.6)));
  reveal(el.querySelector("#jt25"), ease(seg(t, T.emit[0] + 5, T.emit[0] + 5.7)));
});

// ---- 7 benchmark re-campaign
scene("bench", (el) => {
  el.append(head("0.40 · kshana-bench re-campaign", "Four new bars met; three old misses stay missed", [chip("Modelled", "m"), chip("not met · stated", "n")]));
  const short = { A3: "2.5 % bootstrap AUC 0.8502 against 0.80", D3: "three of five detectors' optimism gap above 0.05", R3: "AUC 1.0 against 0.95", R4: "P_fa 0.0, P_d 1.0" };
  const met = N.bench40.map((b) => h("div", { class: "tile bm" }, h("b", { style: "color:var(--lime)" }, b.id), h("span", {}, `${short[b.id] || b.measured}${b.exploratory ? " · exploratory" : ""}`), h("div", { style: "margin-top:8px" }, chip(b.verdict.toLowerCase(), "v"))));
  const BM = N.bench_missed;
  const miss = [["A2", `agc_k_sigma AUC ${BM.A2} against 0.80`], ["D1", `${BM.D1} against 0.05`], ["R1", "task (b), carried"]].map(([a, b]) => h("div", { class: "tile bm", style: "border-color:rgba(255,93,108,.45)" }, h("b", { style: "color:var(--coral)" }, a), h("span", {}, b), h("div", { style: "margin-top:8px" }, chip("not met", "n"))));
  el.append(h("div", { class: "kicker", style: "color:var(--lime)" }, "Pre-registered with the owner's sign-off before any run"), h("div", { class: "tiles", style: "grid-template-columns:repeat(4,1fr)" }, ...met),
    h("div", { class: "kicker", style: "color:var(--coral)" }, "Untouched: nothing in 0.40 closes them"), h("div", { class: "tiles", style: "grid-template-columns:repeat(3,1fr)" }, ...miss),
    h("p", { class: "note" }, "No bar is loosened. Source: xval/bench/RESULTS-0.40.md, RESULTS-v2.md and docs/BENCH.md, committed with the engine."));
}, (el, t) => { el.querySelectorAll(".bm").forEach((r, i) => reveal(r, stag(t, T.bench[0] + 0.5, i, 0.45, 0.5))); });

// ---- 8 languages
const LGS = N.langs, ORDER = ["en", "de", "fr", "es", "ja", "uk", "pt-BR", "zh-Hans", "nl", "pl", "fi", "sv", "da", "nb", "cs", "ru", "et", "pt-PT"].filter((c) => LGS.codes.includes(c));
scene("lang", (el) => {
  el.append(head("0.40 · languages", `Site, Studio and engine in ${LGS.codes.length - 1} more languages`, [chip("machine translation", "m")]));
  const grid = h("div", { style: `display:grid;grid-template-columns:repeat(${SQ ? 6 : 9},1fr);gap:10px` }, ...ORDER.map((c) => h("div", { class: "lc", "data-c": c, style: "font:600 24px var(--mono);text-align:center;padding:10px 0;border-radius:10px;border:1px solid var(--line)" }, c)));
  el.append(grid, h("div", { class: "panel", style: "min-height:190px;display:flex;flex-direction:column;justify-content:center" }, h("div", { class: "note", id: "lg-code" }, ""), h("div", { id: "lg-line", lang: "en", style: "font:500 36px/1.35 var(--sans);margin-top:6px" }, Object.values(LGS.advisory).sort((a, b) => b.length - a.length)[0])),
    h("p", { class: "note" }, "The advisory line, as the engine prints it in each language. Machine translations that no native speaker has reviewed; English is the reference. Verdict words, evidence tiers and the advisory scope are kept in every language; machine files (JSON, CSV, the ledger) stay English."));
}, (el, t) => {
  const t0 = T.lang[0] + 0.4, per = (T.lang[1] - 0.8 - t0) / ORDER.length, i = clamp(Math.floor((t - t0) / per), 0, ORDER.length - 1), c = ORDER[i];
  el.querySelectorAll(".lc").forEach((d) => { const on = d.dataset.c === c; d.style.background = on ? "rgba(53,208,255,.18)" : "transparent"; d.style.color = on ? "var(--cyan)" : "var(--ink2)"; d.style.borderColor = on ? "rgba(53,208,255,.6)" : "var(--line)"; });
  const ln = el.querySelector("#lg-line"); ln.textContent = LGS.advisory[c]; ln.setAttribute("lang", c);
  ln.style.fontFamily = c === "ja" ? "'Noto Sans JP',var(--sans)" : c === "zh-Hans" ? "'Noto Sans SC',var(--sans)" : "var(--sans)";
  el.querySelector("#lg-code").textContent = c === "en" ? "en · the reference" : `${c} · machine translation`;
  ln.style.opacity = ease(seg(t - t0 - i * per, 0, 0.25));
});

// ---- 9 ledger
const LG = N.ledger;
scene("ledger", (el) => {
  el.append(head("Every claim wears its label", `${LG.rows} ledger rows, none promoted`, [chip("validated only with an external oracle", "s")]));
  const grid = h("div", { id: "grid", style: "display:grid;grid-template-columns:repeat(" + (SQ ? 28 : 40) + ",1fr);gap:5px" });
  const order = [...Array(LG.VALIDATED).fill("var(--lime)"), ...Array(LG.MODELLED).fill("rgba(167,180,210,.35)"), ...Array(LG.PARTNER).fill("var(--magenta)")];
  order.forEach((c) => grid.append(h("i", { style: `display:block;aspect-ratio:1;border-radius:3px;background:${c}` })));
  el.append(h("div", { class: "panel" }, grid), h("div", { class: "tiles", style: "grid-template-columns:repeat(4,1fr)" }, ...[[LG.rows, `ledger rows (${N.growth.rows_from} in 0.39)`, "var(--ink)"], [LG.VALIDATED, "VALIDATED against an independent oracle, unchanged", "var(--lime)"], [LG.MODELLED, "MODELLED, honestly labelled", "var(--ink2)"], [LG.PARTNER, "PARTNER-owned", "var(--magenta)"]].map(([v, l, c]) => h("div", { class: "tile" }, h("b", { style: `color:${c}` }, v), h("span", {}, l)))));
}, (el, t) => { const cells = el.querySelectorAll("#grid i"), p = ease(seg(t, T.ledger[0] + 0.3, T.ledger[0] + 2.2)); cells.forEach((c, i) => { c.style.opacity = i / cells.length <= p ? 1 : 0.08; }); });

// ---- 10 close
scene("close", (el) => {
  el.style.justifyContent = "center"; el.style.alignItems = "center"; el.style.textAlign = "center";
  el.append(h("img", { src: "kshana-mark.svg", style: "width:96px;height:96px" }), h("h1", { style: "font-size:84px" }, "kshana 0.40"), ...(branded ? [h("p", { class: "lede" }, "by Ashforde® OÜ")] : []),
    h("p", { class: "lede", style: "color:var(--cyan)" }, "kshana.dev — runs in your browser, no install"), h("p", { class: "note", style: "font-size:20px" }, `Open source · AGPL-3.0 · github.com/ashfordeOU/kshana · Rust ${N.security.msrv}+`),
    h("p", { class: "note" }, "Numbers from engine output committed at the release tag. Advisory software, not type-approved navigation equipment."));
}, () => { });

// ---- the renderer
window.__duration = DURATION;
window.__starts = Object.fromEntries(Object.entries(T).map(([k, v]) => [k, v[0]]));
window.__ends = Object.fromEntries(Object.entries(T).map(([k, v]) => [k, v[1]]));
window.__render = (t) => {
  for (const s of scenes) {
    const a = ease(seg(t, s.t0, s.t0 + 0.5)), b = 1 - ease(seg(t, s.t1 - 0.5, s.t1));
    const vis = t >= s.t0 && t < s.t1 + 0.001;
    s.el.style.display = vis ? "flex" : "none";
    if (!vis) continue;
    const last = s.id === "close";
    s.el.style.opacity = String(last ? a : Math.min(a, b));
    s.upd(s.el, t);
  }
  return true;
};
// Square and portrait frames: scale each scene's content block up to fill the frame, centred (the layout is measured, not guessed).
if (SQ) for (const s of scenes) {
  const el = s.el, inner = el.firstElementChild, bw = el.clientWidth, bh = el.clientHeight;
  el.style.display = "flex";
  inner.style.cssText += ";position:absolute;left:0;top:0;transform-origin:0 0;justify-content:flex-start";
  const measure = (z) => {
    inner.style.width = `${bw / z}px`; inner.style.transform = `scale(${z})`;
    const r0 = inner.getBoundingClientRect(); let mx = r0.bottom;
    inner.querySelectorAll("*").forEach((c) => { const b = c.getBoundingClientRect(); if (b.height > 0) mx = Math.max(mx, b.bottom); });
    return mx - r0.top;
  };
  let z = 1;
  for (let i = 0; i < 8; i++) { const ch = measure(z) / z; const nz = Math.min(2.2, Math.max(1, (0.92 * bh) / ch)); if (Math.abs(nz - z) < 0.01) break; z = nz; }
  while (measure(z) > 0.92 * bh && z > 0.5) z *= 0.97;
  inner.style.top = `${Math.max(0, (bh - measure(z)) / 2)}px`;
}
window.__ready = true;
window.__render(0);
