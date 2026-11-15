// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro, home and licence (0.42 workstream U3, concept D): the pure models.
// No DOM, no Tauri; tested in model.test.mjs. The licence status is the JSON of the app's
// `licence_status` command (src-tauri/src/bridge.rs status_json); nothing here invents a
// licensed state: an absent or unreadable status is drawn as "not read", never as Pro.

const DAY = 86_400;
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** "13 Nov 2027" for Unix seconds (UTC), or "" for none. */
export function dateText(unix) {
  if (!unix) return "";
  const d = new Date(unix * 1000);
  return `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()}`;
}

/** Whole days from `now` to `then` (Unix seconds), rounded up; negative once past. */
export function daysUntil(then, now) {
  const s = then - now;
  return s >= 0 ? Math.ceil(s / DAY) : -Math.ceil(-s / DAY);
}

const plural = (n, one, other) => `${n} ${n === 1 ? one : other}`;
const EDITIONS = { evaluation: "Evaluation", pro: "Pro", enterprise: "Enterprise" };
/** The edition's display name for a tier string ("pro" -> "Pro"). */
export const editionName = (tier) => EDITIONS[String(tier || "").toLowerCase()] || (tier ? String(tier) : "Evaluation");

/** The licence states the app reports, plus "unavailable" (no status could be read). */
export const STATES = ["none", "active", "expiring", "grace", "expired", "revoked", "invalid", "unavailable"];

const BADGE = {
  none: ["NO LICENCE", "none"], active: ["ACTIVE", "ok"], expiring: ["EXPIRING", "warn"], grace: ["GRACE PERIOD", "warn"],
  expired: ["EXPIRED", "bad"], revoked: ["REVOKED", "bad"], invalid: ["NOT VALID HERE", "bad"], unavailable: ["NOT READ", "none"],
};

/**
 * The licence as the Pro home and the licence screen draw it.
 *   status: the `licence_status` JSON, or null when it could not be read (`error` says why)
 *   now:    Unix seconds
 * Returns { state, badge, tone, unlocked, edition, headline, rows: [[label, value]], warning,
 *           daysLeft, statusText, canRemove }.
 * `unlocked` is true only for an active, expiring or grace licence whose tier is not Evaluation.
 */
export function licenceView(status, now, error = "") {
  if (!status || typeof status !== "object") {
    return {
      state: "unavailable", badge: BADGE.unavailable[0], tone: "none", unlocked: false, edition: "Evaluation",
      headline: "The licence could not be read.", rows: [["Checked", "not checked"]],
      warning: error ? String(error) : "Open Kshana Desktop Pro to read the licence on this machine.",
      daysLeft: null, statusText: "Licence not read", canRemove: false,
    };
  }
  const state = STATES.includes(status.state) ? status.state : "invalid";
  const [badge, tone] = BADGE[state];
  const working = state === "active" || state === "expiring" || state === "grace";
  const tier = String(status.tier || "evaluation").toLowerCase();
  const unlocked = working && tier !== "evaluation";
  const held = status.licence_tier || (status.licensee ? status.tier : null);
  const edition = editionName(unlocked ? tier : held || "evaluation");
  const exp = Number(status.expires_unix) || 0;
  const hasLicence = !!status.licensee;
  const daysLeft = hasLicence && exp ? daysUntil(exp, now) : null;

  const rows = [["Edition", unlocked ? edition : held ? `${edition} (not in force)` : "Evaluation"]];
  if (hasLicence) {
    rows.push(["Licensee", String(status.licensee)]);
    if (status.seats) rows.push(["Seats", plural(Number(status.seats), "seat", "seats")]);
    rows.push(["Stations", status.stations ? plural(Number(status.stations), "Station", "Stations") : "1 Station (not set in the licence)"]);
    if (Array.isArray(status.products) && status.products.length) rows.push(["Products", status.products.join(", ")]);
    if (!exp) rows.push(["Renews", "never (perpetual)"]);
    else if (daysLeft >= 0) rows.push([state === "active" ? "Renews" : "Expires", dateText(exp)]);
    else rows.push(["Expired", dateText(exp)]);
    if (status.machine_bound === true) rows.push(["Bound to", "this machine"]);
  }
  rows.push(["Checked", state === "none" ? "no licence on this machine" : "on this machine, offline"]);
  if (status.token_id) rows.push(["Token", String(status.token_id)]);

  const headline = {
    none: "No licence on this machine. Pro scenarios are locked; the open features all run.",
    active: `${edition} is active on this machine.`,
    expiring: `${edition} expires ${daysLeft === 0 ? "today" : `in ${plural(daysLeft, "day", "days")}`}. Renew to keep the Pro features.`,
    grace: `${edition} has expired and keeps working through a grace period. Renew to keep the Pro features.`,
    expired: "The licence has expired. Pro scenarios are locked until it is renewed.",
    revoked: "This licence has been revoked. Pro scenarios are locked.",
    invalid: "The stored licence does not hold on this machine. Pro scenarios are locked.",
  }[state];

  const statusText = {
    none: "No licence · Evaluation",
    active: exp ? `${edition} · renews ${dateText(exp)}` : `${edition} · perpetual`,
    expiring: `${edition} · expires in ${plural(Math.max(daysLeft ?? 0, 0), "day", "days")}`,
    grace: `${edition} · grace period`,
    expired: "Licence expired · Evaluation",
    revoked: "Licence revoked · Evaluation",
    invalid: "Licence not valid here · Evaluation",
  }[state];

  return { state, badge, tone, unlocked, edition, headline, rows, warning: status.warning ? String(status.warning) : "", daysLeft, statusText, canRemove: state !== "none" };
}

// ------------------------------------------------------------------ the four jobs (concept D)
/**
 * The jobs the Pro home offers. `match` picks the Pro scenario kinds (from `list_kinds`) that
 * serve a job in this build; a job with none is drawn as coming, never as available.
 */
export const JOBS = [
  { id: "forensics", icon: "forensics", title: "Investigate an incident", text: "A vessel or flight and a time window, to a signed forensics pack.", match: /^pro-incident-forensics$/ },
  { id: "timing", icon: "timing", title: "Check timing assurance", text: "Holdover and time to breach for every clock in the fleet.", match: /timing-assurance|holdover-fleet/ },
  { id: "scorecard", icon: "scorecards", title: "Score a receiver", text: "A test campaign and a referee scorecard, pre-registered.", match: /^pro-(receiver|timing)-scorecard$/ },
  { id: "study", icon: "dossiers", title: "Write a study", text: "A trade study or mission study from a scenario file, run under this licence.", match: /study|trade/ },
];
/** Further Pro products: drawn as entries with their own state. */
export const PRODUCTS = [
  { id: "evidence", icon: "evidence", title: "Evidence packs", text: "Every run exports a pack with the SHA-256 of each file. Signed packs come later in Desktop Pro.", state: "available" },
  { id: "fleet", icon: "fleet", title: "Station fleet", text: "Many receivers and timing stations side by side, with trust scores and alarms.", state: "coming" },
];

/** The Pro kinds in a `list_kinds` answer (a JSON string or an array of {name}). */
export function proKinds(listKinds) {
  let arr = listKinds;
  if (typeof arr === "string") { try { arr = JSON.parse(arr); } catch { arr = []; } }
  if (!Array.isArray(arr)) return [];
  return arr.map((k) => (k && typeof k === "object" ? k.name : k)).filter((n) => typeof n === "string" && n.startsWith("pro-"));
}

/**
 * The jobs with their state: "ready" (a Pro kind this licence runs serves it), "locked" (the
 * build has such kinds but the licence does not run them) or "coming" (not in this build).
 *   kinds:     the Pro kinds the licence runs (proKinds of list_kinds), or null when unknown
 *   unlocked:  licenceView().unlocked
 */
export function jobsModel(kinds, unlocked) {
  const known = Array.isArray(kinds);
  return JOBS.map((j) => {
    const serving = known ? kinds.filter((k) => j.match.test(k)) : [];
    // The study kinds ship in this build (registry::pro_kinds); incident forensics
    // (pro-incident-forensics) is in the build's tier manifest and arrives as a Pro pack.
    // Receiver and timing scorecards (pro-receiver-scorecard, pro-timing-scorecard) ship in
    // this build too (roadmap 0.42 E3, T4).
    const buildHas = j.id === "study" || j.id === "forensics" || j.id === "scorecard";
    const state = serving.length ? "ready" : !unlocked && buildHas ? "locked" : "coming";
    return { id: j.id, icon: j.icon, title: j.title, text: j.text, state, kinds: serving };
  });
}

// ------------------------------------------------------------------ palette entries
/** The Pro actions added to the command palette. */
export function proActions(view) {
  const A = [
    { id: "pro-home", label: "Pro home", keywords: "pro home start licence edition" },
    { id: "pro-licence", label: view && view.state !== "none" && view.state !== "unavailable" ? "Licence: status, renew or remove" : "Licence: activate a licence", keywords: "pro licence license activate token key seats" },
    { id: "pro-import", label: "Import a licence file…", keywords: "pro licence license file import token" },
    { id: "pro-machine", label: "Copy this machine's ID (for a bound licence)", keywords: "pro licence machine id bound" },
  ];
  if (view && view.canRemove) A.push({ id: "pro-remove", label: "Remove the licence from this machine", keywords: "pro licence license remove deactivate" });
  return A;
}
const words = (q) => String(q || "").toLowerCase().split(/\s+/).filter(Boolean);
/** An action matches when every word of the query is in its label or keywords. */
export function actionMatches(a, q) {
  const w = words(q);
  if (!w.length) return true;
  const hay = `${a.label} ${a.keywords || ""}`.toLowerCase();
  return w.every((x) => hay.includes(x));
}

/**
 * Arrow and Enter keys when the Pro rows follow the shell's rows in the palette.
 *   proSel:     the selected Pro row, or -1 when the shell's selection holds
 *   shellIndex: the shell's selected row (-1 when it has none), shellCount its rows
 *   proCount:   the Pro rows shown
 * Returns { proSel, consume, exec }: `consume` stops the shell from seeing the key; `exec`
 * runs the Pro row `proSel`. Moving down past the shell's last row enters the Pro rows;
 * past the last Pro row the key goes to the shell, which wraps to its first row.
 */
export function palStep(key, { proSel, shellIndex, shellCount, proCount }) {
  const keep = { proSel, consume: false, exec: false };
  if (!proCount) return { ...keep, proSel: -1 };
  const atShellEnd = shellCount === 0 || shellIndex === shellCount - 1;
  const atShellStart = shellCount === 0 || shellIndex <= 0;
  if (key === "ArrowDown") {
    if (proSel < 0) return atShellEnd ? { proSel: 0, consume: true, exec: false } : keep;
    if (proSel < proCount - 1) return { proSel: proSel + 1, consume: true, exec: false };
    return shellCount ? { proSel: -1, consume: false, exec: false } : { proSel: 0, consume: true, exec: false };
  }
  if (key === "ArrowUp") {
    if (proSel < 0) return atShellStart ? { proSel: proCount - 1, consume: true, exec: false } : keep;
    if (proSel > 0) return { proSel: proSel - 1, consume: true, exec: false };
    return shellCount ? { proSel: -1, consume: true, exec: false } : { proSel: proCount - 1, consume: true, exec: false };
  }
  if (key === "Enter" && proSel >= 0) return { proSel, consume: true, exec: true };
  return keep;
}

// ------------------------------------------------------------------ licence file import
/**
 * The token in a licence file's text: the file is the token itself, or a text with one
 * line that looks like a token (comment lines with # are skipped). Returns "" for none.
 */
export function tokenFromFile(text) {
  const lines = String(text || "").split(/\r?\n/).map((l) => l.trim()).filter((l) => l && !l.startsWith("#"));
  if (!lines.length) return "";
  const tokenish = lines.filter((l) => /^[A-Za-z0-9_\-.=+/:]{16,}$/.test(l));
  if (tokenish.length === 1) return tokenish[0];
  return lines.length === 1 ? lines[0] : "";
}
