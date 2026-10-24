// The 0.38 + 0.39 release video: one page, every scene a pure function of the time t (window.__render(t)).
// Numbers come from data/numbers.json (build_numbers.py, from the engine runs collect.py records).
const NS = "http://www.w3.org/2000/svg";
const SVGT = new Set(["svg", "g", "rect", "line", "text", "circle", "path", "polyline", "defs", "pattern", "tspan"]);
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
top.append(h("img", { src: "kshana-mark.svg" }), h("b", {}, "kshana"), ...(branded ? [h("span", { class: "by" }, "by Ashforde® OÜ")] : []), h("span", { class: "ver" }, "0.38 • 0.39"),
  h("span", { class: "right" }, "scores: real engine output"));
const foot = document.getElementById("foot");
foot.append(h("span", {}, "Advisory software, not type-approved navigation equipment. Logs on screen are synthetic unless a scene says otherwise."),
  h("span", { class: "prov" }, `engine v${N.engine.version} · ${N.engine.commit.slice(0, 8)}`));

// ---- scene frame
const T = { title: [0, 4.5], jtA: [4.5, 15.5], jtB: [15.5, 26], jtC: [26, 36], flags: [36, 42.5], timing: [42.5, 49], std: [49, 55], bench: [55, 63], ssv: [63, 69.5], lunar: [69.5, 75.5], ledger: [75.5, 79], close: [79, 82] };
export const DURATION = 82;
const scenes = [];
const scene = (id, build, upd) => {
  const el = h("section", { class: "scene", id });
  build(el);
  document.getElementById("scenes").append(el);
  scenes.push({ id, el, t0: T[id][0], t1: T[id][1], upd });
};
const head = (kick, title, chips = []) => h("div", { class: "head" }, h("div", {}, h("div", { class: "kicker" }, kick), h("h2", {}, title)), h("div", { class: "chips" }, ...chips));
const chip = (txt, cls = "") => h("span", { class: "chip " + cls }, txt);
const reveal = (el, p) => { el.style.opacity = p; el.style.transform = `translateY(${(1 - p) * 18}px)`; };
const stag = (t, t0, i, step = 0.35, dur = 0.5) => ease(seg(t, t0 + i * step, t0 + i * step + dur));

// ---- 1 title
scene("title", (el) => {
  el.style.justifyContent = "center";
  el.append(h("div", { class: "kicker", id: "t-k" }, "Kshana 0.38 + 0.39"), h("h1", { id: "t-h" }, "A neutral referee", h("br"), "for GNSS receivers."),
    h("p", { class: "lede", id: "t-l" }, "Score a receiver's log against a published test plan. Every score here is a real engine run at version ", N.engine.version, "."),
    h("p", { class: "note", id: "t-n" }, "Synthetic logs. Open-source engine. Advisory software."));
}, (el, t) => { ["t-k", "t-h", "t-l", "t-n"].forEach((id, i) => reveal(el.querySelector("#" + id), stag(t, 0.2, i, 0.4, 0.7))); });

// ---- 2 jammertest: the timeline
const JT = N.jt;
const jtCmd = () => h("div", { class: "cmd" }, h("i", {}, "$ "), "kshana jammertest score session.toml --catalog testcatalog.json --plan plan.json --location 2 --local-offset-h 2");
scene("jtA", (el) => {
  el.append(head("0.38 · a neutral referee", "Score a receiver against the published test plan", [chip("Modelled", "m"), chip("synthetic log", "s")]));
  const W = 1500, Hh = 330, X0 = 60, X1 = 1450, S = JT.secs, X = (s) => X0 + (s / S) * (X1 - X0);
  const svg = h("svg", { viewBox: `0 0 ${W} ${Hh}`, width: "100%", id: "jt-svg" });
  svg.append(h("defs", {}, h("pattern", { id: "hatch", width: 10, height: 10, patternUnits: "userSpaceOnUse", patternTransform: "rotate(45)" }, h("rect", { width: 5, height: 10, fill: "rgba(255,79,182,.55)" }))));
  svg.append(h("text", { x: X0, y: 28, fill: "#6f7d9d", "font-size": 17 }, "scheduled test slots (published plan, Test Area 2, Monday 2025-09-15, local time)"));
  JT.tests.forEach((s) => {
    svg.append(h("rect", { x: X(s.onset), y: 44, width: X(s.end) - X(s.onset), height: 76, rx: 8, fill: "rgba(53,208,255,.10)", stroke: "rgba(53,208,255,.55)" }),
      h("text", { x: X(s.onset) + 12, y: 74, fill: "#eaf0ff", "font-size": 22, "font-weight": 600 }, s.id), h("text", { x: X(s.onset) + 12, y: 102, fill: "#a7b4d2", "font-size": 17 }, s.name));
  });
  svg.append(h("text", { x: X0, y: 156, fill: "#6f7d9d", "font-size": 17 }, "synthetic receiver log: jammer on (input, written for this video)"));
  JT.jam_s.forEach(([a, b]) => svg.append(h("rect", { x: X(a), y: 166, width: X(b) - X(a), height: 40, rx: 6, fill: "url(#hatch)", stroke: "rgba(255,79,182,.7)" })));
  svg.append(h("text", { x: X0, y: 240, fill: "#6f7d9d", "font-size": 17 }, "what Kshana's monitors found"));
  svg.append(h("line", { x1: X0, x2: X1, y1: 300, y2: 300, stroke: "rgba(255,255,255,.25)" }));
  for (let s = 0; s <= S; s += 600) { const m = 14 * 60 + s / 60; svg.append(h("line", { x1: X(s), x2: X(s), y1: 300, y2: 308, stroke: "rgba(255,255,255,.3)" }), h("text", { x: X(s), y: 326, "text-anchor": "middle", fill: "#6f7d9d", "font-size": 16 }, `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}`)); }
  const marks = [];
  JT.tests.forEach((s) => {
    const mid = (s.onset + s.end) / 2;
    if (s.outcome === "missed") marks.push({ at: s.end, g: h("g", {}, h("path", { d: `M${X(mid) - 10} 252 l20 20 M${X(mid) + 10} 252 l-20 20`, stroke: "#ff5d6c", "stroke-width": 5, "stroke-linecap": "round" }), h("text", { x: X(mid), y: 292, "text-anchor": "middle", fill: "#ff5d6c", "font-size": 20, "font-weight": 700 }, "MISSED")) });
    else { const at = s.onset + s.latency, col = s.outcome === "detected" ? "#a6f04a" : "#ffb224"; marks.push({ at, g: h("g", {}, h("circle", { cx: X(at), cy: 262, r: 11, fill: col }), h("text", { x: X(at) + 18, y: 270, fill: col, "font-size": 20, "font-weight": 700 }, `${s.outcome.toUpperCase()} +${fmt(s.latency)} s`)) }); }
  });
  JT.jam_s.filter(([a, b]) => !JT.tests.some((s) => a < s.end && b > s.onset)).forEach(([a, b]) => marks.push({ at: b, g: h("g", {}, h("circle", { cx: X((a + b) / 2), cy: 262, r: 11, fill: "none", stroke: "#ff5d6c", "stroke-width": 3, "stroke-dasharray": "5 4" }), h("text", { x: X((a + b) / 2) + 18, y: 270, fill: "#ff5d6c", "font-size": 20, "font-weight": 700 }, "FALSE ALARM")) }));
  marks.forEach((m) => { m.g.style.opacity = 0; svg.append(m.g); });
  const cur = h("line", { x1: X0, x2: X0, y1: 36, y2: 300, stroke: "#eaf0ff", "stroke-width": 2, id: "jt-cur" });
  svg.append(cur);
  el.append(h("div", { class: "panel" }, svg), jtCmd(), h("p", { class: "note" }, "Plan and catalogue: NPRA/jammertest-plan (MIT licence), fetched and hash-checked, not copied into the repository. The slots are the scheduled truth, not recorded on-air times. The log is synthetic."));
  el._marks = marks; el._X = X;
}, (el, t) => {
  const p = ease(seg(t, T.jtA[0] + 1, T.jtA[0] + 9.5)), s = p * JT.secs, X = el._X;
  const c = el.querySelector("#jt-cur"); c.setAttribute("x1", X(s)); c.setAttribute("x2", X(s));
  el._marks.forEach((m) => { m.g.style.opacity = s >= m.at ? 1 : 0; });
});

// ---- 3 jammertest: the table
scene("jtB", (el) => {
  el.append(head("0.38 · per test", "Detected, late or missed, with the latency", [chip("Modelled", "m"), chip("synthetic log", "s")]));
  const tb = h("table", {}, h("tr", {}, ...["test", "name", "onset", "outcome", "latency", "recovered after"].map((x) => h("th", {}, x))));
  JT.tests.forEach((s) => tb.append(h("tr", { class: "row" }, h("td", {}, s.id), h("td", {}, s.name), h("td", {}, fmt(s.onset) + " s"), h("td", {}, h("span", { class: "pill " + s.outcome }, s.outcome)), h("td", {}, s.latency == null ? "–" : fmt(s.latency, 0) + " s"), h("td", {}, s.recover == null ? "–" : fmt(s.recover, 0) + " s"))));
  const sm = JT.summary;
  el.append(h("div", { class: "panel" }, tb),
    h("div", { class: "tiles" }, ...[[sm.detected, "detected in tolerance", "var(--lime)"], [sm.late, "late", "var(--amber)"], [sm.missed, "missed", "var(--coral)"], [fmt(sm.median_latency_s) + " s", "median latency", "var(--ink)"], [sm.false_alarm_runs, `false-alarm runs (${fmt(sm.false_alarm_epochs)} of ${fmt(sm.clean_epochs)} clean epochs)`, "var(--coral)"]].map(([v, l, c]) => h("div", { class: "tile" }, h("b", { style: `color:${c}` }, v), h("span", {}, l)))),
    h("p", { class: "note" }, "MODELLED: scored against the scheduled slots of the published test plan, not against recorded on-air times; no external oracle states when a transmitter was on."));
}, (el, t) => { el.querySelectorAll(".row").forEach((r, i) => reveal(r, stag(t, T.jtB[0] + 0.8, i, 0.9, 0.6))); el.querySelectorAll(".tile").forEach((r, i) => reveal(r, stag(t, T.jtB[0] + 5, i, 0.3, 0.6))); });

// ---- 4 jammertest: signed evidence pack
scene("jtC", (el) => {
  el.append(head("0.38 · evidence", "The score ships as a signed evidence pack", [chip("signed", "s")]));
  const files = h("ul", { class: "files" }, ...JT.pack_files.map((f) => h("li", {}, "pack/" + f)));
  const ck = h("ul", { class: "checks" }, ...JT.verify.map((c) => h("li", {}, h("span", { class: "ok" }, "✓"), c.name, h("span", { style: "color:var(--ink3)" }, c.status))));
  el.append(h("div", { class: "cols" }, h("div", { class: "panel" }, h("div", { class: "note" }, "kshana jammertest score … --key signer.key --out pack"), files),
    h("div", { class: "panel" }, h("div", { class: "cmd", style: "margin-bottom:12px" }, h("i", {}, "$ "), "kshana evidence verify pack --pubkey signer.key.pub"), ck, h("p", { style: "font:700 26px var(--mono);color:var(--lime);margin-top:10px" }, `ok: ${JT.verify_ok}`))),
    h("div", { class: "panel", id: "tamper", style: "border-color:rgba(255,93,108,.5)" }, h("div", { style: "font:600 22px var(--mono);color:var(--coral)" }, `One byte appended to summary.html → kshana evidence verify exits ${JT.tamper_rc}`), h("div", { class: "note" }, JT.tamper_failures.map((f) => f.code).join(" · "))));
}, (el, t) => { el.querySelectorAll(".files li").forEach((r, i) => reveal(r, stag(t, T.jtC[0] + 0.6, i, 0.35, 0.5))); el.querySelectorAll(".checks li").forEach((r, i) => reveal(r, stag(t, T.jtC[0] + 3, i, 0.5, 0.5))); reveal(el.querySelector("#tamper"), ease(seg(t, T.jtC[0] + 7, T.jtC[0] + 8))); });

// ---- 5 receiver self-report
const FL = N.flags;
scene("flags", (el) => {
  el.append(head("Also new · 0.38", "The receiver's own spoofing flag, beside Kshana's alarm", [chip("Modelled", "m"), chip("synthetic UBX log", "s")]));
  const cell = (n, l, c) => h("div", { class: "tile cellq" }, h("b", { style: `color:${c}` }, n), h("span", {}, l));
  const W = 1450, X = (s) => 40 + (s / 200) * (W - 80);
  const tl = h("svg", { viewBox: `0 0 ${W} 120`, width: "100%" }, h("line", { x1: 40, x2: W - 40, y1: 70, y2: 70, stroke: "rgba(255,255,255,.25)" }),
    h("circle", { cx: X(FL.first_kshana_alarm_s), cy: 70, r: 12, fill: "#35d0ff" }), h("text", { x: X(FL.first_kshana_alarm_s), y: 40, "text-anchor": "middle", fill: "#35d0ff", "font-size": 20 }, `Kshana first alarm ${fmt(FL.first_kshana_alarm_s)} s`),
    h("circle", { cx: X(FL.first_receiver_flag_s), cy: 70, r: 12, fill: "#ffb224" }), h("text", { x: X(FL.first_receiver_flag_s), y: 108, "text-anchor": "middle", fill: "#ffb224", "font-size": 20 }, `receiver first flag ${fmt(FL.first_receiver_flag_s)} s`));
  el.append(h("div", { class: "tiles", style: "grid-template-columns:repeat(4,1fr)" }, cell(FL.both, "both flag the epoch", "var(--lime)"), cell(FL.receiver_only, "receiver only", "var(--amber)"), cell(FL.kshana_only, "Kshana only", "var(--cyan)"), cell(FL.neither, "neither", "var(--ink2)")),
    h("div", { class: "panel" }, tl), h("div", { class: "cmd" }, h("i", {}, "$ "), "kshana receiver-trust session.toml     # UBX log, ", fmt(FL.epochs_reported), " epochs after calibration"),
    h("p", { class: "note" }, "Neither is ground truth, and agreement is not validation. The receiver's flags are UBX-NAV-STATUS spoofDetState."));
}, (el, t) => { el.querySelectorAll(".cellq").forEach((r, i) => reveal(r, stag(t, T.flags[0] + 0.6, i, 0.4, 0.6))); });

// ---- 6 holdover test
const TM = N.timing;
scene("timing", (el) => {
  el.append(head("Also new · 0.38", "A holdover test, measured against the ITU-T masks", [chip("Modelled", "m"), chip("synthetic record", "s")]));
  const W = 1500, Hh = 320, X = (s) => 60 + (s / 3000) * (W - 100), Y = (v) => 270 - ((v + 40) / 360) * 240;
  const svg = h("svg", { viewBox: `0 0 ${W} ${Hh}`, width: "100%" });
  const st = TM.steps.find((s) => s.name === "holdover");
  svg.append(h("rect", { x: X(st.start), y: 20, width: X(st.end) - X(st.start), height: 250, fill: "rgba(255,178,36,.07)" }), h("text", { x: X(st.start) + 10, y: 40, fill: "#ffb224", "font-size": 18 }, "GNSS removed · holdover"));
  svg.append(h("line", { x1: 60, x2: W - 40, y1: Y(0), y2: Y(0), stroke: "rgba(255,255,255,.18)" }), h("line", { x1: 60, x2: W - 40, y1: Y(100), y2: Y(100), stroke: "#ff5d6c", "stroke-dasharray": "8 6" }), h("text", { x: W - 44, y: Y(100) - 8, "text-anchor": "end", fill: "#ff5d6c", "font-size": 17 }, "100 ns holdover envelope (G.8272.1)"));
  const pts = TM.series_ns.map((v, i) => `${X(i * TM.interval_s)},${Y(v)}`);
  svg.append(h("polyline", { points: pts.join(" "), fill: "none", stroke: "#35d0ff", "stroke-width": 3, id: "tm-line" }), h("text", { x: 70, y: 304, fill: "#6f7d9d", "font-size": 16 }, "time error (ns) against time (s), 3,000 s record"));
  el._pts = pts;
  const vd = TM.verdicts, col = (v) => (v === "PASS" || v === "RECOVERED" ? "v" : "n");
  el.append(h("div", { class: "panel" }, svg), h("div", { class: "chips", style: "display:flex;gap:12px;flex-wrap:wrap" }, chip("baseline masks " + vd.baseline_masks, col(vd.baseline_masks)), chip("holdover masks " + vd.holdover_masks, col(vd.holdover_masks)), chip("holdover envelope " + vd.holdover_envelope, col(vd.holdover_envelope)), chip(vd.recovery.toLowerCase(), col(vd.recovery)), chip(`max |time error| ${fmt(TM.steps.find((s) => s.name === "holdover").max_abs_te_ns)} ns`, "s")),
    h("div", { class: "cmd" }, h("i", {}, "$ "), "kshana scenarios/holdover-test-example.toml"), h("p", { class: "note" }, "Masks written from public ITU-T tables (G.8272, G.8272.1). A record too short for a mask is reported as incomplete."));
}, (el, t) => { const p = ease(seg(t, T.timing[0] + 0.6, T.timing[0] + 4)), n = Math.max(2, Math.floor(p * el._pts.length)); el.querySelector("#tm-line").setAttribute("points", el._pts.slice(0, n).join(" ")); });

// ---- 7 standards packs
scene("std", (el) => {
  el.append(head("Also new · 0.38", "Standards packs: every test, met or not", [chip("pack results", "s")]));
  const rows = N.standards.map((p) => { const tot = p.met + p.not_met + p.needs_input; const bar = h("div", { style: "display:flex;height:34px;border-radius:8px;overflow:hidden;margin:8px 0 4px" }, ...[[p.met, "var(--lime)"], [p.not_met, "var(--coral)"], [p.needs_input, "var(--amber)"]].map(([n, c]) => h("div", { style: `flex:${n};background:${c};opacity:.85` })));
    return h("div", { class: "panel srow" }, h("div", { style: "font:600 24px var(--sans)" }, p.title), bar, h("div", { class: "note" }, `${p.met} met · ${p.not_met} not met · ${p.needs_input} needs input   (${tot} tests)`)); });
  el.append(...rows, h("div", { class: "cmd" }, h("i", {}, "$ "), "kshana standards-pack run ", h("i", {}, "<pack>"), " --json"), h("p", { class: "note" }, "A pack reports what Kshana measured against each public test; it is not a certification or a statement of conformance."));
}, (el, t) => { el.querySelectorAll(".srow").forEach((r, i) => reveal(r, stag(t, T.std[0] + 0.5, i, 0.6, 0.6))); });

// ---- 8 kshana-bench
const BN = N.bench;
scene("bench", (el) => {
  el.append(head("Also new · 0.38, 0.39", "kshana-bench, with the bars that were not met", [chip("Modelled", "m"), chip("not met · stated", "n")]));
  const W = 880, rows = BN.baselines, X = (v) => 250 + ((v - 0.5) / 0.5) * (W - 300);
  const svg = h("svg", { viewBox: `0 0 ${W} ${rows.length * 70 + 60}`, width: "100%" });
  svg.append(h("text", { x: 250, y: 22, fill: "#6f7d9d", "font-size": 16 }, "AUC, test split (0.5 = chance)"));
  rows.forEach((r, i) => { const y = 44 + i * 70; svg.append(h("text", { x: 238, y: y + 24, "text-anchor": "end", fill: "#eaf0ff", "font-size": 19 }, r.name),
    h("rect", { x: 250, y, width: X(r.auc_in) - 250, height: 20, rx: 4, fill: "#6b8cff" }), h("text", { x: X(r.auc_in) + 8, y: y + 16, fill: "#a7b4d2", "font-size": 15 }, r.auc_in.toFixed(3)),
    h("rect", { x: 250, y: y + 24, width: X(r.auc_out) - 250, height: 20, rx: 4, fill: "#ffb224" }), h("text", { x: X(r.auc_out) + 8, y: y + 40, fill: "#a7b4d2", "font-size": 15 }, r.auc_out.toFixed(3))); });
  svg.append(h("rect", { x: 560, y: 4, width: 14, height: 14, fill: "#6b8cff" }), h("text", { x: 580, y: 16, fill: "#a7b4d2", "font-size": 15 }, "in-distribution"), h("rect", { x: 730, y: 4, width: 14, height: 14, fill: "#ffb224" }), h("text", { x: 750, y: 16, fill: "#a7b4d2", "font-size": 15 }, "shifted"));
  const bars = h("div", { class: "panel" }, h("div", { class: "kicker", style: "color:var(--coral)" }, "Bars written before the run · recorded verdicts"),
    ...[["A2", "agc_k_sigma AUC ≥ 0.80 on test", `${BN.A2[0]} · NOT MET`], ["D1", "some detector's gap > 0.05 at scale 0.5", `${BN.D1[0].replace("largest gap ", "largest gap ")} · NOT MET`], ["task (b) R1", "carried, not re-run", "NOT MET"]].map(([a, b, c]) => h("div", { class: "brow", style: "margin:14px 0" }, h("div", { style: "font:700 26px var(--mono);color:var(--coral)" }, a, " · ", c), h("div", { class: "note" }, b))),
    h("p", { class: "note" }, "No bar is loosened. Source: xval/bench/RESULTS-v2.md and docs/BENCH.md, committed with the engine."));
  el.append(h("div", { class: "cols" }, h("div", { class: "panel" }, svg), bars), h("div", { class: "cmd" }, h("i", {}, "$ "), "kshana bench baseline --task d --split test --out bench     # " + BN.spec));
}, (el, t) => { el.querySelectorAll(".brow").forEach((r, i) => reveal(r, stag(t, T.bench[0] + 2, i, 0.9, 0.6))); });

// ---- 9 space-user GNSS
const SS = N.ssv;
scene("ssv", (el) => {
  el.append(head("Also new · 0.38", "GNSS above, around and past the constellations", [chip("Modelled", "m")]));
  const comb = SS.combined, W = 920, rowsD = [["L1 · at least 1 signal", comb.l1.at_least_1], ["L1 · at least 4 signals", comb.l1.at_least_4], ["L5 · at least 1 signal", comb.l5.at_least_1], ["L5 · at least 4 signals", comb.l5.at_least_4]];
  const svg = h("svg", { viewBox: `0 0 ${W} 300`, width: "100%" });
  rowsD.forEach(([l, v], i) => { const y = 20 + i * 66; svg.append(h("text", { x: 0, y: y + 22, fill: "#eaf0ff", "font-size": 21 }, l), h("rect", { x: 300, y, width: 500, height: 30, rx: 6, fill: "rgba(255,255,255,.08)" }), h("rect", { x: 300, y, width: 5 * v, height: 30, rx: 6, fill: "#a6f04a", opacity: 0.85 }), h("text", { x: 300 + 5 * v + 10, y: y + 22, fill: "#a7b4d2", "font-size": 19 }, v.toFixed(1) + " %")); });
  el.append(h("div", { class: "cols" }, h("div", { class: "panel" }, h("div", { class: "kicker" }, `One high orbit · up to ${fmt(SS.apogee_km)} km altitude · all four systems`), svg, h("p", { class: "note" }, `Signals counted above ${SS.cn0_min_dbhz} dB-Hz, main lobes and sidelobes; GPS, GLONASS, Galileo and BeiDou combined.`)),
    h("div", { class: "panel" }, h("div", { class: "kicker" }, "Onboard filter and formation"), h("div", { class: "tile", style: "margin:12px 0" }, h("b", {}, fmt(SS.nav_final_err_m) + " m"), h("span", {}, "final position error, transfer toward the Moon")), h("div", { class: "tile" }, h("b", {}, fmt(SS.formation_final_err_m) + " m"), h("span", {}, "final relative error, two spacecraft on a high orbit")))),
    h("div", { class: "cmd" }, h("i", {}, "$ "), "kshana scenarios/gnss-ssv-heo.toml   scenarios/gnss-ssv-nav-lunar.toml   scenarios/gnss-ssv-formation-heo.toml"),
    h("p", { class: "note" }, "Every figure is MODELLED. Compared with the public Space Service Volume booklet under pre-registered bars, most bars were not met: docs/SSV.md lists each."));
}, (el, t) => { el.querySelectorAll(".panel,.tile").forEach((r, i) => reveal(r, stag(t, T.ssv[0] + 0.4, i, 0.3, 0.6))); });

// ---- 10 lunar infrastructure
const LU = N.lunar;
scene("lunar", (el) => {
  el.append(head("Also new · 0.38", "The Moon's orbit and surface as navigation infrastructure", [chip("Modelled", "m"), chip("not met · stated", "n")]));
  const t1 = h("div", { class: "tile" }, h("b", { style: "color:var(--lime)" }, LU.od_mean_q.toFixed(2)), h("span", {}, `mean q against ${LU.od_expected.toFixed(1)} expected: onboard orbit determination consistent with the ground (${fmt(LU.od_trials)} trials)`));
  const t2 = h("div", { class: "tile" }, h("b", { style: "color:var(--coral)" }, LU.od_frac_exceed_err.toFixed(3)), h("span", {}, `share above the 95 % point with a ${LU.od_err_accel} m/s² injected error: the check cannot see it (bar: at least 0.5, not met)`));
  const arcs = LU.carrier_arcs, t3 = h("div", { class: "tile" }, h("b", { style: "color:var(--coral)" }, (100 * arcs[arcs.length - 1].success).toFixed(0) + " %"), h("span", {}, `carrier-phase integer fixing success on the longest arc (committed scored run: ${arcs.map((a) => a.days + " d " + (100 * a.success).toFixed(0) + " %").join(", ")}): not reached`));
  el.append(h("div", { class: "tiles", style: "grid-template-columns:repeat(3,1fr)" }, t1, t2, t3),
    h("div", { class: "panel" }, h("div", { class: "note", style: "font-size:19px" }, "Five studies: orbit determination from Earth GNSS in lunar orbit, carrier-phase fixing between a surface station and an orbiter, co-estimating the lunar frame, tying GNSS, VLBI and laser ranging on one spacecraft, and sensitivity to the Moon's tides and libration.")),
    h("div", { class: "cmd" }, h("i", {}, "$ "), "kshana scenarios/lunar-infrastructure-od-consistency.toml   (and four more studies)"),
    h("p", { class: "note" }, "MODELLED. Bars were fixed before the runs; those not met are stated in docs/LUNAR-INFRASTRUCTURE.md."));
}, (el, t) => { el.querySelectorAll(".tile").forEach((r, i) => reveal(r, stag(t, T.lunar[0] + 0.5, i, 0.5, 0.6))); });

// ---- 11 ledger
const LG = N.ledger;
scene("ledger", (el) => {
  el.append(head("Every claim wears its label", `${LG.rows} ledger rows`, [chip("validated only with an external oracle", "s")]));
  const grid = h("div", { id: "grid", style: "display:grid;grid-template-columns:repeat(" + (SQ ? 28 : 40) + ",1fr);gap:5px" });
  const order = [...Array(LG.VALIDATED).fill("var(--lime)"), ...Array(LG.MODELLED).fill("rgba(167,180,210,.35)"), ...Array(LG.PARTNER).fill("var(--magenta)")];
  order.forEach((c) => grid.append(h("i", { style: `display:block;aspect-ratio:1;border-radius:3px;background:${c}` })));
  el.append(h("div", { class: "panel" }, grid), h("div", { class: "tiles", style: "grid-template-columns:repeat(4,1fr)" }, ...[[LG.rows, "ledger rows", "var(--ink)"], [LG.VALIDATED, "VALIDATED against an independent oracle", "var(--lime)"], [LG.MODELLED, "MODELLED, honestly labelled", "var(--ink2)"], [LG.PARTNER, "PARTNER-owned", "var(--magenta)"]].map(([v, l, c]) => h("div", { class: "tile" }, h("b", { style: `color:${c}` }, v), h("span", {}, l)))));
}, (el, t) => { const cells = el.querySelectorAll("#grid i"), p = ease(seg(t, T.ledger[0] + 0.3, T.ledger[0] + 2.2)); cells.forEach((c, i) => { c.style.opacity = i / cells.length <= p ? 1 : 0.08; }); });

// ---- 12 close
scene("close", (el) => {
  el.style.justifyContent = "center"; el.style.alignItems = "center"; el.style.textAlign = "center";
  el.append(h("img", { src: "kshana-mark.svg", style: "width:96px;height:96px" }), h("h1", { style: "font-size:84px" }, "kshana"), ...(branded ? [h("p", { class: "lede" }, "by Ashforde® OÜ")] : []),
    h("p", { class: "lede", style: "color:var(--cyan)" }, "kshana.dev — runs in your browser, no install"), h("p", { class: "note", style: "font-size:20px" }, "Open source · AGPL-3.0 · github.com/ashfordeOU/kshana"),
    h("p", { class: "note" }, "Scores from real Kshana engine runs; logs are synthetic. Advisory software, not type-approved navigation equipment."));
}, (el, t) => { reveal(el.firstChild, 1); });

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
window.__ready = true;
window.__render(0);
