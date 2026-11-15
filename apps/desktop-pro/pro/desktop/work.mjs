// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: the Pro work views on the open shell (0.42 U6, U19, U27, U28), through
// the shell's extension API. Each view draws from W (the work state) and reads through the
// app (service.mjs); loading is explicit or on first open, never in a render loop. The models
// are pure and tested (fleet, replay, review, products, enterprise); registration touches no
// DOM (hook.test.mjs checks it with a fake shell).
import { fleetModel, fleetView, stationMap, toneVar } from "./fleet.mjs";
import { replayModel, replayMap, replayStrip, frameAt, stepCursor, gapText, levelOf } from "./replay.mjs";
import { newReview, replayItems, scorecardItems, decide, summary, reviewRecord, availableSections, buildReport } from "./review.mjs";
import { scorecardModel, areaModel, timingModel, trustRunsModel } from "./products.mjs";
import { accountModel, libraryModel, auditModel, auditCsv, wallModel } from "./enterprise.mjs";
import { auditPath } from "./service.mjs";
import { utcText } from "./geo.mjs";

/** The work views: id, sidebar label, icon, edition, and whether the service must be connected. */
export const WORK = [
  { id: "pro-fleet", label: "Station fleet", icon: "fleet", edition: "pro", service: true },
  { id: "pro-replay", label: "Incident replay", icon: "forensics", edition: "pro" },
  { id: "pro-scorecard", label: "Receiver scorecards", icon: "scorecards", edition: "pro" },
  { id: "pro-review", label: "Review", icon: "evidence", edition: "pro" },
  { id: "pro-report", label: "Report builder", icon: "dossiers", edition: "pro" },
  { id: "pro-area", label: "Area trust", icon: "mark", edition: "pro", service: true },
  { id: "pro-timing", label: "Timing fleet", icon: "timing", edition: "pro", service: true },
  { id: "pro-trust", label: "Multi-source trust", icon: "timing", edition: "pro" },
  { id: "pro-connect", label: "Sign in (SSO)", icon: "key", edition: "pro" },
  { id: "pro-library", label: "Team library", icon: "file", edition: "enterprise", service: true },
  { id: "pro-audit", label: "Audit log", icon: "evidence", edition: "enterprise", service: true },
  { id: "pro-wall", label: "Watch wall", icon: "home", edition: "enterprise", service: true },
];

/** A fresh work state. */
export function createWork() {
  return { conn: null, me: null, fleet: null, fleetSel: null, replay: null, packDir: "", cursor: 0, playing: false, review: null, reviewer: "", scorecard: null, area: null, timing: null, trust: null, library: null, audit: null, auditDays: 7, wall: null, wallFull: false, report: { sections: [], title: "", author: "", out: "" }, msg: {}, loaded: {} };
}

/** The nav item's badge: an edition the licence or the service does not have. */
export function workBadge(w, view, W) {
  if (w.edition === "pro" && !view.unlocked) return "PRO";
  if (w.edition === "enterprise" && !(view.unlocked && view.edition === "Enterprise")) return "ENTERPRISE";
  if (w.service && !(W.conn && W.conn.connected)) return "SIGN IN";
  return null;
}

const runsJson = (runs) => (runs || []).map((r) => { try { return { run: r, doc: typeof r.result === "string" ? JSON.parse(r.result) : r.result }; } catch { return { run: r, doc: null }; } });

/**
 * Register the work views and the WORK sidebar section. `h` builds DOM (ui.mjs); `papi` is
 * proApi(invoke); `S` the licence state; `now` Unix seconds. Returns the unregister functions.
 */
export function registerWork(shell, { h, ic, S, papi, W = createWork(), now = () => Math.floor(Date.now() / 1000), later = (fn) => setTimeout(fn, 0) }) {
  const off = [];
  const redraw = () => shell.refresh();
  const toast = (t) => (typeof shell.toast === "function" ? shell.toast(t) : undefined);
  const chip = (text, tone) => h("span", { class: `state-chip ${tone}`, text });
  const note = (k) => (W.msg[k] ? h("p", { class: `lic-result ${W.msg[k].tone}`, role: "status", text: W.msg[k].text }) : null);
  const say = (k, tone, text) => { W.msg[k] = { tone, text }; redraw(); };
  const page = (id, kicker, title, lede, ...body) => h("section", { class: "page pro-page", "data-pro": id, "aria-labelledby": `${id}-h` }, h("p", { class: "kicker", text: kicker }), h("h1", { id: `${id}-h`, text: title }), lede ? h("p", { class: "lede", text: lede }) : null, ...body);
  const btn = (label, onclick, primary) => h("button", { class: primary ? "btn primary" : "btn", type: "button", onclick }, label);
  const locked = (w) => page(w.id, "Kshana Desktop Pro", w.label, null, h("div", { class: "card" }, h("p", { text: w.edition === "enterprise" ? `${w.label} is part of Kshana Enterprise.` : `${w.label} needs a Kshana Pro licence.` }), btn("Licence →", () => shell.show("pro-licence"), true)));
  const needService = (w) => page(w.id, "Kshana Desktop Pro", w.label, null, h("div", { class: "card" }, h("p", { text: `${w.label} reads your organisation's Kshana Pro service. Sign in first.` }), btn("Sign in →", () => shell.show("pro-connect"), true)));
  const once = (key, fn) => { if (!W.loaded[key]) { W.loaded[key] = true; later(fn); } };
  const getJson = async (key, path, set) => { const r = await papi.get(path); if (r.ok) { try { set(r.value); W.msg[key] = null; } catch (e) { W.msg[key] = { tone: "bad", text: String(e.message || e) }; } } else W.msg[key] = { tone: "bad", text: r.error }; redraw(); };

  const loaders = {
    connect: async () => { const c = await papi.connection(); W.conn = c.ok ? c.value : null; if (W.conn && W.conn.connected) { const me = await papi.get("/whoami"); W.me = me.ok ? me.value : null; } redraw(); },
    fleet: () => getJson("fleet", "/fleet/stations", (v) => { W.fleet = v; }),
    area: async () => { await loaders.fleet(); await getJson("area", "/area/picture", (v) => { W.area = areaModel(v, W.fleet); }); },
    timing: () => getJson("timing", "/timing/assessment", (v) => { W.timing = timingModel(v); }),
    library: () => getJson("library", "/library", (v) => { W.library = libraryModel(v); }),
    audit: () => getJson("audit", auditPath(now(), W.auditDays), (v) => { W.audit = auditModel(v); }),
    wall: () => getJson("wall", "/wall/state", (v) => { W.wall = wallModel(v); }),
  };
  once("connect", loaders.connect);

  const draw = {
    "pro-fleet": () => {
      once("fleet", loaders.fleet);
      if (!W.fleet) return page("pro-fleet", "Kshana Desktop Pro", "Station fleet", "Reading the fleet from the service…", note("fleet"));
      const m = fleetModel(W.fleet, now());
      const view = fleetView(h, m, { get selected() { return W.fleetSel; }, set selected(v) { W.fleetSel = v; } }, { redraw });
      view.append(h("div", { class: "row" }, btn("Refresh", loaders.fleet)), note("fleet"));
      return view;
    },
    "pro-replay": () => {
      const open = async () => {
        const r = await papi.replay(W.packDir.trim());
        if (!r.ok) return say("replay", "bad", r.error);
        try { W.replay = replayModel(r.value); W.cursor = W.replay.t0; W.review = null; W.msg.replay = null; } catch (e) { W.msg.replay = { tone: "bad", text: String(e.message || e) }; }
        redraw();
      };
      const input = h("input", { class: "pro-input mono", id: "pro-pack-dir", value: W.packDir, placeholder: "Folder of the incident forensics pack", oninput: (e) => { W.packDir = e.target.value; } });
      const head = h("div", { class: "card" }, h("label", { for: "pro-pack-dir", text: "Pack folder" }), h("div", { class: "row" }, input, btn("Open and verify", open, true)), note("replay"));
      const m = W.replay;
      if (!m) return page("pro-replay", "Kshana Desktop Pro · incident forensics", "Incident replay", "Open a signed incident forensics pack. It is verified first; then the receiver's estimated position and the reported AIS or ADS-B position replay on one time cursor, shaded by the trust the pack's findings state.", head);
      const f = frameAt(m, W.cursor);
      const setT = (t) => { W.cursor = t; redraw(); };
      const play = () => {
        W.playing = !W.playing;
        const tick = () => { if (!W.playing) return; W.cursor = Math.min(m.t1, W.cursor + 5); if (W.cursor >= m.t1) W.playing = false; redraw(); if (W.playing) setTimeout(tick, 100); };
        tick(); redraw();
      };
      const keys = (e) => { if (e.key === "k" || e.key === " ") { e.preventDefault(); play(); } else { const t = stepCursor(m, W.cursor, e.key); if (t !== W.cursor) { e.preventDefault(); setT(t); } } };
      return page("pro-replay", `Kshana Desktop Pro · case ${m.caseId}`, m.title || "Incident replay", null, head,
        h("div", { class: "row" }, chip(m.verified ? (m.trusted ? "VERIFIED · TRUSTED KEY" : "VERIFIED") : "NOT VERIFIED", m.verified ? "ok" : "bad"), chip(String(m.status).toUpperCase(), "none"), h("span", { class: "mut", text: `Signer ${m.signer} · key ${m.keyId}` })),
        m.problems.length ? h("p", { class: "lic-warning bad", role: "alert", text: m.problems.join("; ") }) : null,
        h("div", { class: "pro-replay", tabindex: "0", onkeydown: keys, "aria-label": "Replay: J and L step, K plays or pauses" },
          replayMap(m, W.cursor), replayStrip(m, W.cursor, setT),
          h("div", { class: "row" }, btn(W.playing ? "Pause (K)" : "Play (K)", play), btn("−10 s (J)", () => setT(stepCursor(m, W.cursor, "j"))), btn("+10 s (L)", () => setT(stepCursor(m, W.cursor, "l"))),
            h("span", { class: "mono", text: utcText(W.cursor) }), chip(f.trust.level.toUpperCase(), { trusted: "ok", degraded: "warn", untrusted: "bad" }[f.trust.level]), h("span", { class: "mut", text: gapText(f.gap) }))),
        h("p", { class: "mut", text: `${m.verdict.sentence || ""} Trust shading is the pack's MODELLED inference. ${m.label}` }),
        h("div", { class: "row" }, btn("Review the findings →", () => { W.review = W.review || newReview({ kind: "incident-forensics", id: m.caseId, title: m.title }, replayItems(m)); shell.show("pro-review"); }), btn("Build a report →", () => shell.show("pro-report"))),
        h("ul", { class: "pro-limits" }, ...m.limits.map((l) => h("li", { text: l }))),
        h("table", { class: "pro-table" }, h("tbody", {}, ...f.trust.active.map((e) => h("tr", {}, h("td", {}, h("span", { class: "dot", style: `background:${toneVar({ degraded: "warn", untrusted: "bad" }[levelOf(e)])}` })), h("td", { text: `${e.category} (${e.confidence})` }), h("td", { text: e.source }))))));
    },
    "pro-scorecard": (ctx) => {
      const found = runsJson(ctx.runs).filter((x) => x.doc && x.doc.format === "kshana-pro-receiver-scorecard/1");
      if (!W.scorecard && found.length) { try { W.scorecard = scorecardModel(found[0].doc); } catch { /* shown below */ } }
      const c = W.scorecard;
      if (!c) return page("pro-scorecard", "Kshana Desktop Pro · E3", "Receiver scorecards", "Run a receiver scorecard campaign (a pro-receiver-scorecard scenario) in a tab; its scorecard appears here.", h("p", { class: "empty", text: "No scorecard run in this session yet." }));
      return page("pro-scorecard", `Kshana Desktop Pro · campaign ${c.campaign.id || ""}`, [c.receiver.maker, c.receiver.model].filter(Boolean).join(" ") || "Receiver scorecard", c.label,
        found.length > 1 ? h("div", { class: "row" }, ...found.map((x, i) => btn(x.run.title || `Run ${i + 1}`, () => { W.scorecard = scorecardModel(x.doc); redraw(); }))) : null,
        h("h2", { text: c.framework }),
        h("table", { class: "pro-table" }, h("thead", {}, h("tr", {}, ...["Level", "Summary", "Evidence", "Met", "Not met", "Not assessed"].map((t) => h("th", { text: t })))),
          h("tbody", {}, ...c.levels.map((l) => h("tr", {}, h("td", { text: String(l.level) }), h("td", { text: l.summary }), h("td", {}, chip(l.evidence.toUpperCase(), l.tone)), h("td", { text: String(l.met) }), h("td", { text: String(l.notMet) }), h("td", { text: String(l.notAssessed) }))))),
        h("p", { class: "mut", text: c.wording }),
        h("h2", { text: "Cases" }),
        h("table", { class: "pro-table" }, h("thead", {}, h("tr", {}, ...["Case", "Threat", "Monitor", "Fix availability", "Max error (m)", "Misleading epochs"].map((t) => h("th", { text: t })))),
          h("tbody", {}, ...c.cases.map((x) => h("tr", {}, h("td", { text: x.title }), h("td", { text: x.threat }), h("td", { text: x.outcomes.join(", ") }), h("td", { class: "mono", text: Number.isFinite(x.availability) ? `${Math.round(x.availability * 100)} %` : "—" }), h("td", { class: "mono", text: String(x.maxError ?? "—") }), h("td", { class: "mono", text: String(x.misleading) }))))),
        h("div", { class: "row" }, btn("Review the events →", () => { W.review = newReview({ kind: "receiver-scorecard", id: c.campaign.id, title: `${c.receiver.maker || ""} ${c.receiver.model || ""}`.trim() }, scorecardItems({ cases: c.cardCases })); shell.show("pro-review"); }), btn("Build a report →", () => shell.show("pro-report"))),
        h("ul", { class: "pro-limits" }, ...c.limits.map((l) => h("li", { text: l }))));
    },
    "pro-review": () => {
      const v = W.review;
      if (!v) return page("pro-review", "Kshana Desktop Pro", "Review", "Open an incident replay or a receiver scorecard and choose Review: every finding or event is listed here to accept or dispute.", h("p", { class: "empty", text: "Nothing under review." }));
      const s = summary(v);
      const act = (id, d) => { try { const n = d === "disputed" ? (document.getElementById(`note-${id}`) || {}).value || "" : ""; W.review = decide(W.review, id, d, n); W.msg.review = null; } catch (e) { W.msg.review = { tone: "bad", text: String(e.message || e) }; } redraw(); };
      const sign = async () => {
        let rec;
        try { rec = reviewRecord(W.review, { reviewer: W.reviewer, at: now() }); } catch (e) { return say("review", "bad", String(e.message || e)); }
        const dir = (W.packDir ? `${W.packDir.replace(/[\\/]+$/, "")}-review` : "").trim();
        if (!dir) return say("review", "bad", "Open the pack from a folder first: the review is written next to it.");
        const r = await papi.signFiles(dir, { "review.json": JSON.stringify(rec, null, 2) + "\n" }, W.reviewer);
        say("review", r.ok ? "ok" : "bad", r.ok ? `Signed review written to ${r.value.dir} (key ${r.value.key_id}).` : r.error);
      };
      return page("pro-review", `Kshana Desktop Pro · ${v.subject.kind}`, `Review: ${v.subject.title || v.subject.id}`, "Accept each item, or dispute it with a note. The signed review is your working record; it is not the claim-grade expert review.",
        h("div", { class: "row" }, chip(`${s.accepted} ACCEPTED`, "ok"), chip(`${s.disputed} DISPUTED`, s.disputed ? "warn" : "none"), chip(`${s.pending} PENDING`, s.pending ? "warn" : "ok")),
        ...v.items.map((i) => h("div", { class: `card pro-review-item ${i.decision}` }, h("b", { text: i.title }), h("p", { class: "mut", text: i.detail }),
          h("div", { class: "row" }, btn("Accept", () => act(i.id, "accepted")), h("input", { class: "pro-input", id: `note-${i.id}`, placeholder: "Why it is disputed", value: i.note }), btn("Dispute", () => act(i.id, "disputed")), chip(i.decision.toUpperCase(), { accepted: "ok", disputed: "warn", pending: "none" }[i.decision])))),
        h("div", { class: "row" }, h("input", { class: "pro-input", placeholder: "Reviewer's name", value: W.reviewer, oninput: (e) => { W.reviewer = e.target.value; } }), btn("Sign the review", sign, s.complete)),
        note("review"));
    },
    "pro-report": () => {
      const src = { replay: W.replay, review: W.review && summary(W.review).total ? W.review : null, scorecard: W.scorecard, area: W.area };
      const secs = availableSections(src);
      const R = W.report;
      if (!secs.length) return page("pro-report", "Kshana Desktop Pro", "Report builder", "Open a replay, a scorecard or the area trust picture; its parts become sections here.", h("p", { class: "empty", text: "Nothing open to report on." }));
      const toggle = (id) => { R.sections = R.sections.includes(id) ? R.sections.filter((x) => x !== id) : [...R.sections, id]; redraw(); };
      const make = async () => {
        let out;
        try { out = buildReport(src, R.sections, { title: R.title, author: R.author, at: now() }); } catch (e) { return say("report", "bad", String(e.message || e)); }
        if (!R.out.trim()) return say("report", "bad", "Name the folder the signed report is written to.");
        const r = await papi.signFiles(R.out.trim(), { "report.html": out.html, "report.json": out.json }, R.author || "Kshana Desktop Pro user");
        say("report", r.ok ? "ok" : "bad", r.ok ? `Signed report written to ${r.value.dir}. Recipients verify it with key ${r.value.public_key}.` : r.error);
      };
      return page("pro-report", "Kshana Desktop Pro", "Report builder", "Pick the sections, in order. The report carries every label and limit of its sources and is signed with this device's evidence key.",
        h("div", { class: "card" }, ...secs.map((s) => h("label", { class: "row" }, h("input", { type: "checkbox", checked: R.sections.includes(s.id) || null, onchange: () => toggle(s.id) }), s.title))),
        h("div", { class: "row" }, h("input", { class: "pro-input", placeholder: "Report title", value: R.title, oninput: (e) => { R.title = e.target.value; } }), h("input", { class: "pro-input", placeholder: "Prepared by", value: R.author, oninput: (e) => { R.author = e.target.value; } })),
        h("div", { class: "row" }, h("input", { class: "pro-input mono", placeholder: "Folder to write the signed report to", value: R.out, oninput: (e) => { R.out = e.target.value; } }), btn("Build and sign", make, true)),
        note("report"));
    },
    "pro-area": () => {
      once("area", loaders.area);
      const a = W.area;
      if (!a) return page("pro-area", "Kshana Desktop Pro · F1", "Area trust", "Reading the area console from the service…", note("area"));
      const placed = a.areas.filter((x) => x.pos).map((x) => ({ id: x.id, name: x.name, state: "live", tone: x.tone, trust: { tone: x.tone, text: x.current.toLowerCase(), value: null }, pos: x.pos, alarms: [], ageText: "" }));
      return page("pro-area", "Kshana Desktop Pro · F1", a.title, a.label,
        h("p", { class: "banner", text: a.positions }),
        stationMap({ stations: placed }, {}),
        h("p", { class: "mut", text: "Areas are placed at the centre of their Stations that reported a position; others are listed only." }),
        ...a.areas.map((x) => h("div", { class: "card" }, h("div", { class: "row" }, h("b", { text: x.name }), chip(x.current, x.tone), h("span", { class: "mut", text: `${x.events.length} event(s)` })),
          h("div", { class: "pro-bar" }, ...x.timeline.map((s) => h("span", { style: `left:${(s.from * 100).toFixed(3)}%;width:${Math.max(s.width * 100, 0.2).toFixed(3)}%;background:${toneVar(s.tone)}`, title: s.state }))))),
        h("div", { class: "row" }, btn("Refresh", loaders.area)), note("area"));
    },
    "pro-timing": () => {
      once("timing", loaders.timing);
      const t = W.timing;
      if (!t) return page("pro-timing", "Kshana Desktop Pro · T2", "Timing fleet", "Reading the timing fleet from the service…", note("timing"));
      return page("pro-timing", "Kshana Desktop Pro · T2", t.title, t.label,
        h("div", { class: "row" }, ...Object.entries(t.counts).map(([k, n]) => chip(`${n} ${k.toUpperCase()}`, { ok: "ok", watch: "warn", act: "bad", breached: "bad" }[k] || "none"))),
        h("table", { class: "pro-table" }, h("thead", {}, h("tr", {}, ...["Clock", "Site", "Make", "Status", "First warning in", "Limits"].map((x) => h("th", { text: x })))),
          h("tbody", {}, ...t.clocks.map((c) => h("tr", {}, h("td", { class: "mono", text: c.id }), h("td", { text: c.site }), h("td", { text: c.vendor }), h("td", {}, chip(String(c.status).toUpperCase(), c.tone)), h("td", { text: c.warnText }), h("td", { text: c.limits.join(", ") }))))),
        h("div", { class: "row" }, btn("Refresh", loaders.timing)), note("timing"));
    },
    "pro-trust": (ctx) => {
      const found = runsJson(ctx.runs).map((x) => { try { return { run: x.run, m: trustRunsModel(x.doc) }; } catch { return null; } }).filter(Boolean);
      if (!found.length) return page("pro-trust", "Kshana Desktop Pro · F3", "Multi-source trust", "Run a pro-multi-source-trust scenario in a tab: which time source was trusted, second by second, with the evidence of every switch.", h("p", { class: "empty", text: "No multi-source trust run in this session yet." }));
      const { m } = found[0];
      return page("pro-trust", "Kshana Desktop Pro · F3", "Multi-source trust", m.label,
        h("div", { class: "row" }, chip(`${m.counts.trusted} TRUSTED`, "ok"), chip(`${m.counts.holdover} HOLDOVER`, "warn"), chip(`${m.counts.none} NONE`, m.counts.none ? "bad" : "none"), h("span", { class: "mut", text: `${m.epochs} epochs · ${m.switches} switches` })),
        h("table", { class: "pro-table" }, h("thead", {}, h("tr", {}, ...["From (s)", "Source", "State", "Reason"].map((x) => h("th", { text: x })))),
          h("tbody", {}, ...m.runs.map((r) => h("tr", {}, h("td", { class: "mono", text: String(r.from) }), h("td", { text: r.source }), h("td", {}, chip(r.state.toUpperCase(), { trusted: "ok", holdover: "warn", none: "bad" }[r.state])), h("td", { text: r.reason }))))));
    },
    "pro-connect": () => {
      const acc = accountModel(W.me, W.conn);
      const base = h("input", { class: "pro-input mono", id: "pro-svc-base", placeholder: "The service address (an https address, or this machine)", value: (W.conn && W.conn.base) || "" });
      const cred = h("input", { class: "pro-input mono", id: "pro-svc-cred", type: "password", autocomplete: "off", placeholder: "Single sign-on token or API key" });
      const go = async () => {
        const r = await papi.connect(base.value, cred.value);
        if (!r.ok) return say("connect", "bad", r.error);
        await loaders.connect();
        say("connect", W.me ? "ok" : "bad", W.me ? `Signed in as ${W.me.id} (${W.me.role}, tenant ${W.me.tenant}).` : "The service did not accept the credential.");
      };
      const out = async () => { await papi.disconnect(); W.conn = null; W.me = null; W.fleet = W.area = W.timing = W.library = W.audit = W.wall = null; W.loaded = { connect: true }; say("connect", "ok", "Signed out; the credential was removed from this machine."); };
      return page("pro-connect", "Kshana Desktop Pro", "Sign in to your Kshana Pro service", "Use the token your organisation's single sign-on gives you, or an API key from your administrator. The app keeps it on this machine, readable by you only; this page never sees it again. The service decides your tenant and role, and refuses a person the identity provider has deactivated.",
        acc.signedIn ? h("div", { class: "card" }, h("h2", { text: acc.id }), h("dl", { class: "lic-rows" }, h("dt", { text: "Tenant" }), h("dd", { text: acc.tenant }), h("dt", { text: "Role" }), h("dd", { text: `${acc.role}: ${acc.roleText}` }), h("dt", { text: "Signed in with" }), h("dd", { text: acc.source }), h("dt", { text: "Service" }), h("dd", { class: "mono", text: acc.base }), h("dt", { text: "Service edition" }), h("dd", { text: acc.edition }), h("dt", { text: "Enterprise operations" }), h("dd", { text: acc.enterprise.join(", ") || "none" })), btn("Sign out", out)) : h("p", { class: "mut", text: acc.text }),
        h("div", { class: "card" }, h("label", { for: "pro-svc-base", text: "Service address" }), base, h("label", { for: "pro-svc-cred", text: "Token or key" }), cred, h("div", { class: "row" }, btn("Sign in", go, true))),
        note("connect"));
    },
    "pro-library": () => {
      once("library", loaders.library);
      const L = W.library;
      const run = async (name) => { const r = await papi.post(`/library/${encodeURIComponent(name)}/run`); say("library", r.ok ? "ok" : "bad", r.ok ? `${name}: job ${r.value.job_id || ""} ${r.status === 200 ? "already exists (same request)" : "queued"}.` : r.error); };
      return page("pro-library", "Kshana Enterprise", "Team library", "Your team's saved studies on the service. Running one again is an ordinary job; the same request is the same job.",
        L ? (L.length ? h("table", { class: "pro-table" }, h("thead", {}, h("tr", {}, ...["Study", "Kind", "Saved by", "Saved", ""].map((x) => h("th", { text: x })))),
          h("tbody", {}, ...L.map((s) => h("tr", {}, h("td", { text: s.name }), h("td", { class: "mono", text: s.kind || s.op }), h("td", { text: s.savedBy }), h("td", { class: "mono", text: s.saved }), h("td", {}, accountModel(W.me, W.conn).can && accountModel(W.me, W.conn).can.submit ? btn("Run", () => run(s.name)) : null))))) : h("p", { class: "empty", text: "The team library is empty." })) : h("p", { class: "mut", text: "Reading the library…" }),
        h("div", { class: "row" }, btn("Refresh", loaders.library)), note("library"));
    },
    "pro-audit": () => {
      once("audit", loaders.audit);
      const A = W.audit;
      const save = async () => {
        const dir = (W.msg.auditDir || "").trim();
        if (!dir) return say("audit", "bad", "Name a folder for the signed export.");
        const r = await papi.signFiles(dir, { "audit.csv": auditCsv(A) }, (W.me && W.me.id) || "Kshana Desktop Pro user");
        say("audit", r.ok ? "ok" : "bad", r.ok ? `Signed export written to ${r.value.dir}.` : r.error);
      };
      return page("pro-audit", "Kshana Enterprise", "Audit log", "Every request to the service, hash-chained. Admins of a tenant see their tenant; an operator sees every tenant and the server's own events.",
        A ? [h("div", { class: "row" }, h("span", { class: A.chainOk ? "chip ok" : "chip bad", text: A.chainText }), h("span", { class: "mut", text: `${A.count} lines · ${A.refused} refused` })),
          h("div", { class: "wrap" }, h("table", { class: "pro-table" }, h("thead", {}, h("tr", {}, ...["Time", "Who", "Role", "Via", "Request", "Status"].map((x) => h("th", { text: x })))),
            h("tbody", {}, ...A.rows.slice(0, 500).map((r) => h("tr", { class: r.refused ? "refused" : null }, h("td", { class: "mono", text: utcText(r.t) }), h("td", { text: r.who }), h("td", { text: r.role }), h("td", { text: r.source }), h("td", { class: "mono", text: `${r.method} ${r.route}` }), h("td", { class: "mono", text: String(r.status) })))))),
          h("div", { class: "row" }, h("input", { class: "pro-input mono", placeholder: "Folder for a signed CSV export", oninput: (e) => { W.msg.auditDir = e.target.value; } }), btn("Export and sign", save))] : h("p", { class: "mut", text: "Reading the audit log…" }),
        h("div", { class: "row" }, ...[1, 7, 30].map((d) => btn(`Last ${d} day${d > 1 ? "s" : ""}`, () => { W.auditDays = d; loaders.audit(); }))), note("audit"));
    },
    "pro-wall": () => {
      once("wall", loaders.wall);
      if (!W.wallTimer) { W.wallTimer = setInterval(() => { if (shell.view === "pro-wall") loaders.wall(); }, 30000); if (W.wallTimer && W.wallTimer.unref) W.wallTimer.unref(); }
      const m = W.wall;
      if (!m) return page("pro-wall", "Kshana Enterprise", "Watch wall", "Reading the wall…", note("wall"));
      return h("section", { class: `page pro-page pro-wall ${m.alarm ? "alarm" : ""} ${W.wallFull ? "full" : ""}`, "data-pro": "pro-wall", "aria-labelledby": "pro-wall-h" },
        h("div", { class: "row" }, h("h1", { id: "pro-wall-h", text: `Watch wall · ${m.tenant}` }), chip(m.alarm ? "ATTENTION" : "NO ALARM", m.alarm ? "bad" : "ok"), h("span", { class: "mut", text: `updated ${utcText(m.at)} · every 30 s` }), btn(W.wallFull ? "Leave full screen" : "Full screen", () => { W.wallFull = !W.wallFull; redraw(); })),
        h("div", { class: "pro-tiles" }, ...m.tiles.map((t) => h("div", { class: `pro-tile ${t.tone}` }, h("span", { class: "mut", text: t.group }), h("b", { text: t.name }), chip(String(t.state).toUpperCase(), t.tone)))),
        m.notConfigured.length ? h("p", { class: "mut", text: `Not configured on the service: ${m.notConfigured.join(", ")}.` }) : null,
        h("p", { class: "mut", text: `Jobs: ${Object.entries(m.jobs).map(([k, n]) => `${n} ${k}`).join(", ") || "none"}` }), note("wall"));
    },
  };

  for (const w of WORK) {
    off.push(shell.registerView(w.id, {
      title: w.label,
      render(el, ctx) {
        const badge = workBadge(w, S.view, W);
        const node = badge === "PRO" || badge === "ENTERPRISE" ? locked(w) : badge === "SIGN IN" && w.id !== "pro-connect" ? needService(w) : draw[w.id](ctx || {});
        el.replaceChildren(node);
      },
    }));
  }
  off.push(shell.registerNavSection({
    heading: "PRO WORK",
    items: () => WORK.map((w) => ({ id: w.id, label: w.label, icon: () => ic(w.icon), view: w.id, badge: workBadge(w, S.view, W) })),
  }));
  off.push(shell.registerPaletteGroup({
    heading: "Kshana Pro work",
    items: () => WORK.map((w) => ({ id: w.id, label: `Open ${w.label.toLowerCase()}`, keywords: `pro ${w.label} ${w.edition}`, tag: w.edition === "enterprise" ? "ENTERPRISE" : "PRO", icon: () => ic(w.icon), onSelect: () => shell.show(w.id) })),
  }));
  return { off, W, loaders };
}
