// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Tests of the Pro home's pure models (0.42 U3): every licence state drawn, no licensed
// state without a licence, the jobs, the palette rows and keys, the licence file, wording.
//   node --test apps/desktop-pro/pro/desktop/*.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { licenceView, STATES, dateText, daysUntil, jobsModel, proKinds, proActions, actionMatches, palStep, tokenFromFile, editionName } from "./model.mjs";

const NOW = Date.UTC(2026, 10, 13, 12) / 1000; // 13 Nov 2026 12:00 UTC
const DAY = 86400;
// As src-tauri/src/bridge.rs status_json writes it.
const status = (over = {}) => ({
  state: "active", tier: "pro", licensee: "Example Lab", expires_unix: NOW + 365 * DAY, products: ["desktop"], seats: 5,
  licence_tier: "pro", stations: 10, machine_bound: false, token_id: "tok-123", warning: null, ...over,
});
const row = (v, k) => (v.rows.find((r) => r[0] === k) || [])[1];

test("dates and days", () => {
  assert.equal(dateText(NOW + 365 * DAY), "13 Nov 2027");
  assert.equal(dateText(0), "");
  assert.equal(daysUntil(NOW + 12 * DAY, NOW), 12);
  assert.equal(daysUntil(NOW + 1, NOW), 1);
  assert.equal(daysUntil(NOW - 3 * DAY, NOW), -3);
  assert.equal(editionName("enterprise"), "Enterprise");
  assert.equal(editionName(""), "Evaluation");
});

test("an active licence: Pro, seats, Stations, renewal, checked offline", () => {
  const v = licenceView(status(), NOW);
  assert.equal(v.state, "active");
  assert.equal(v.badge, "ACTIVE");
  assert.equal(v.tone, "ok");
  assert.equal(v.unlocked, true);
  assert.equal(v.edition, "Pro");
  assert.equal(row(v, "Edition"), "Pro");
  assert.equal(row(v, "Licensee"), "Example Lab");
  assert.equal(row(v, "Seats"), "5 seats");
  assert.equal(row(v, "Stations"), "10 Stations");
  assert.equal(row(v, "Renews"), "13 Nov 2027");
  assert.equal(row(v, "Checked"), "on this machine, offline");
  assert.equal(v.statusText, "Pro · renews 13 Nov 2027");
  assert.equal(v.canRemove, true);
});

test("a perpetual, machine-bound enterprise licence", () => {
  const v = licenceView(status({ tier: "enterprise", licence_tier: "enterprise", expires_unix: 0, machine_bound: true, seats: 1, stations: 0 }), NOW);
  assert.equal(v.edition, "Enterprise");
  assert.equal(row(v, "Renews"), "never (perpetual)");
  assert.equal(row(v, "Bound to"), "this machine");
  assert.equal(row(v, "Seats"), "1 seat");
  assert.match(row(v, "Stations"), /not set/);
  assert.equal(v.statusText, "Enterprise · perpetual");
});

test("no licence: Evaluation, locked, nothing to remove", () => {
  const v = licenceView({ state: "none", tier: "evaluation", licensee: null, expires_unix: null, products: null, seats: null, token_id: null, warning: null }, NOW);
  assert.equal(v.state, "none");
  assert.equal(v.badge, "NO LICENCE");
  assert.equal(v.unlocked, false);
  assert.equal(v.edition, "Evaluation");
  assert.equal(row(v, "Licensee"), undefined);
  assert.equal(row(v, "Checked"), "no licence on this machine");
  assert.equal(v.canRemove, false);
  assert.match(v.statusText, /No licence/);
});

test("expiring: days left and the renewal warning", () => {
  const v = licenceView(status({ state: "expiring", expires_unix: NOW + 12 * DAY, warning: "Renew soon: the licence expires in 12 day(s)" }), NOW);
  assert.equal(v.badge, "EXPIRING");
  assert.equal(v.tone, "warn");
  assert.equal(v.unlocked, true);
  assert.equal(v.daysLeft, 12);
  assert.equal(row(v, "Expires"), dateText(NOW + 12 * DAY));
  assert.match(v.headline, /expires in 12 days/);
  assert.equal(v.statusText, "Pro · expires in 12 days");
  assert.match(v.warning, /Renew soon/);
});

test("grace: still unlocked, marked as expired", () => {
  const v = licenceView(status({ state: "grace", expires_unix: NOW - 3 * DAY, warning: "the licence has expired; grace" }), NOW);
  assert.equal(v.badge, "GRACE PERIOD");
  assert.equal(v.unlocked, true);
  assert.equal(row(v, "Expired"), dateText(NOW - 3 * DAY));
});

test("expired, revoked and invalid are never unlocked, whatever the tier says", () => {
  for (const state of ["expired", "revoked", "invalid"]) {
    const v = licenceView(status({ state, tier: "evaluation", expires_unix: NOW - 30 * DAY }), NOW);
    assert.equal(v.unlocked, false, state);
    assert.equal(v.tone, "bad", state);
    assert.equal(row(v, "Edition"), "Pro (not in force)", state);
    // Even a tier that still reads "pro" does not unlock a lapsed state.
    assert.equal(licenceView(status({ state, tier: "pro" }), NOW).unlocked, false, state);
  }
  assert.equal(licenceView(status({ state: "revoked" }), NOW).badge, "REVOKED");
  assert.equal(licenceView(status({ state: "expired" }), NOW).badge, "EXPIRED");
  assert.equal(licenceView({ state: "invalid", tier: "evaluation", warning: "wrong machine" }, NOW).warning, "wrong machine");
});

test("no status read (outside the app, or the command failed): not read, never Pro", () => {
  const v = licenceView(null, NOW, "no licence store directory");
  assert.equal(v.state, "unavailable");
  assert.equal(v.unlocked, false);
  assert.equal(v.badge, "NOT READ");
  assert.equal(v.warning, "no licence store directory");
  assert.equal(v.canRemove, false);
  assert.equal(licenceView(undefined, NOW).unlocked, false);
  // An unknown state is drawn as not valid.
  assert.equal(licenceView(status({ state: "golden" }), NOW).state, "invalid");
});

test("every state the plan names is drawn with a badge and a headline", () => {
  for (const state of STATES.filter((s) => s !== "unavailable")) {
    const v = licenceView(status({ state }), NOW);
    assert.ok(v.badge && v.headline && v.statusText, state);
  }
});

test("Pro kinds from list_kinds", () => {
  const listed = JSON.stringify([{ name: "gnss-coverage" }, { name: "pro-trade-study", description: "" }, { name: "pro-twin" }]);
  assert.deepEqual(proKinds(listed), ["pro-trade-study", "pro-twin"]);
  assert.deepEqual(proKinds("not json"), []);
  assert.deepEqual(proKinds(null), []);
});

test("jobs: ready only with a kind that serves them; coming otherwise; locked without a licence", () => {
  const ready = jobsModel(["pro-trade-study", "pro-lunar-time-budget-study", "pro-twin"], true);
  const by = Object.fromEntries(ready.map((j) => [j.id, j]));
  assert.equal(by.study.state, "ready");
  assert.deepEqual(by.study.kinds, ["pro-trade-study", "pro-lunar-time-budget-study"]);
  assert.equal(by.forensics.state, "coming");
  assert.equal(by.timing.state, "coming");
  assert.equal(by.scorecard.state, "coming");
  const locked = Object.fromEntries(jobsModel([], false).map((j) => [j.id, j.state]));
  assert.equal(locked.study, "locked");
  assert.equal(locked.forensics, "locked");
  assert.equal(locked.scorecard, "locked");
  const sc = Object.fromEntries(jobsModel(["pro-receiver-scorecard", "pro-timing-scorecard"], true).map((j) => [j.id, j]));
  assert.equal(sc.scorecard.state, "ready");
  assert.deepEqual(sc.scorecard.kinds, ["pro-receiver-scorecard", "pro-timing-scorecard"]);
  // Incident forensics is ready once its Pro pack is loaded and the licence runs it.
  const fx = Object.fromEntries(jobsModel(["pro-incident-forensics"], true).map((j) => [j.id, j]));
  assert.equal(fx.forensics.state, "ready");
  assert.deepEqual(fx.forensics.kinds, ["pro-incident-forensics"]);
  assert.equal(jobsModel(null, false).length, 6);
  // The timing fleet (T2) opens its view; ready with a licence, locked without.
  const tf = Object.fromEntries(jobsModel([], true).map((j) => [j.id, j]));
  assert.equal(tf["timing-fleet"].state, "ready");
  assert.equal(tf["timing-fleet"].view, "pro-timing");
  assert.equal(locked["timing-fleet"], "locked");
  // Multi-source trust (F3) is a kind of this build: ready when the licence runs it.
  assert.equal(locked.trust, "locked");
  assert.equal(Object.fromEntries(jobsModel(["pro-multi-source-trust"], true).map((j) => [j.id, j.state])).trust, "ready");
});

test("palette: Pro actions, matching, remove only with a licence", () => {
  const none = proActions(licenceView(null, NOW));
  assert.ok(none.some((a) => a.id === "pro-licence" && /activate/.test(a.label)));
  assert.ok(!none.some((a) => a.id === "pro-remove"));
  assert.ok(proActions(licenceView(status(), NOW)).some((a) => a.id === "pro-remove"));
  const lic = none.find((a) => a.id === "pro-licence");
  assert.ok(actionMatches(lic, "licence"));
  assert.ok(actionMatches(lic, "license key"));
  assert.ok(!actionMatches(lic, "waterfall"));
  assert.ok(actionMatches(lic, ""));
});

test("palette keys: down from the shell's last row enters the Pro rows and wraps back", () => {
  const base = { shellCount: 3, proCount: 2 };
  assert.deepEqual(palStep("ArrowDown", { ...base, proSel: -1, shellIndex: 1 }), { proSel: -1, consume: false, exec: false });
  assert.deepEqual(palStep("ArrowDown", { ...base, proSel: -1, shellIndex: 2 }), { proSel: 0, consume: true, exec: false });
  assert.deepEqual(palStep("ArrowDown", { ...base, proSel: 0, shellIndex: 2 }), { proSel: 1, consume: true, exec: false });
  // Past the last Pro row the shell takes the key and wraps to its first row.
  assert.deepEqual(palStep("ArrowDown", { ...base, proSel: 1, shellIndex: 2 }), { proSel: -1, consume: false, exec: false });
  // Up from the shell's first row goes to the last Pro row; up from the first Pro row back to the shell.
  assert.deepEqual(palStep("ArrowUp", { ...base, proSel: -1, shellIndex: 0 }), { proSel: 1, consume: true, exec: false });
  assert.deepEqual(palStep("ArrowUp", { ...base, proSel: 0, shellIndex: 2 }), { proSel: -1, consume: true, exec: false });
  assert.deepEqual(palStep("Enter", { ...base, proSel: 1, shellIndex: 2 }), { proSel: 1, consume: true, exec: true });
  assert.deepEqual(palStep("Enter", { ...base, proSel: -1, shellIndex: 2 }), { proSel: -1, consume: false, exec: false });
  // No shell rows: the Pro rows cycle on their own.
  assert.deepEqual(palStep("ArrowDown", { shellCount: 0, proCount: 2, proSel: -1, shellIndex: -1 }), { proSel: 0, consume: true, exec: false });
  assert.deepEqual(palStep("ArrowDown", { shellCount: 0, proCount: 2, proSel: 1, shellIndex: -1 }), { proSel: 0, consume: true, exec: false });
  // No Pro rows: the shell keeps every key.
  assert.deepEqual(palStep("ArrowDown", { shellCount: 3, proCount: 0, proSel: -1, shellIndex: 2 }), { proSel: -1, consume: false, exec: false });
});

test("licence file: the token line, comments skipped", () => {
  const tok = "7b226c6963656e736565223a22.a1b2c3d4e5f6a7b8c9d0";
  assert.equal(tokenFromFile(tok), tok);
  assert.equal(tokenFromFile(`${tok}\n`), tok);
  assert.equal(tokenFromFile(`# Kshana Pro licence for Example Lab\n# issued 2026-11-13\n${tok}\n`), tok);
  assert.equal(tokenFromFile("Licensee: Example Lab\nToken:\n" + tok), tok);
  assert.equal(tokenFromFile(""), "");
  assert.equal(tokenFromFile("# only a comment"), "");
});

test("wording: the Pro desktop pages avoid the words the product never uses", () => {
  const dir = dirname(fileURLToPath(import.meta.url));
  const files = [...readdirSync(dir).filter((f) => !f.endsWith(".test.mjs")).map((f) => join(dir, f)), join(dir, "..", "home.html"), join(dir, "..", "licence.html")];
  const banned = [/\bcompliant\b/i, /\bconform/i, /\bcertified\b/i, /runs in your browser/i, /\bno server\b/i, /works offline/i];
  for (const f of files) {
    // Comments may name the shell's own strings; the text the pages show is in the code.
    const text = readFileSync(f, "utf8").split("\n").filter((l) => !/^\s*(\/\/|\/\*|\*|<!--)/.test(l)).join("\n");
    for (const re of banned) assert.ok(!re.test(text), `${f}: ${re}`);
  }
});

test("no outside request: the Pro desktop modules fetch nothing and name no host", () => {
  const dir = dirname(fileURLToPath(import.meta.url));
  for (const f of readdirSync(dir).filter((x) => /\.m?js$/.test(x) && !x.endsWith(".test.mjs"))) {
    const text = readFileSync(join(dir, f), "utf8").replaceAll("http://www.w3.org/2000/svg", "");
    assert.ok(!/\bfetch\(|XMLHttpRequest|WebSocket|https?:\/\//.test(text), f);
  }
});
