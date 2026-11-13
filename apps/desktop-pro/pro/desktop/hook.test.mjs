// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Tests of the Pro overlay on the open shell's extension API (hook.mjs), with a fake shell
// that records what is registered (the shape of the open apps/desktop/shell/lib/ext.mjs).
//   node --test apps/desktop-pro/pro/desktop/*.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { attachToShell, canAttach, redrawLicence, SIDE, toneColor, VIEWS } from "./hook.mjs";
import { createProState } from "./ui.mjs";
import { proActions, licenceView } from "./model.mjs";

const NOW = Date.UTC(2026, 10, 13, 12) / 1000;
const ACTIVE = { state: "active", tier: "pro", licence_tier: "pro", licensee: "Example Lab", expires_unix: NOW + 365 * 86400, products: ["desktop"], seats: 5, stations: 10, machine_bound: false, token_id: "t", warning: null };

function fakeShell(over = {}) {
  const calls = { views: new Map(), replaced: new Map(), sections: [], groups: [], status: {}, edition: undefined, shown: [], refreshes: 0, toasts: [] };
  const shell = {
    version: 1,
    registerView: (id, spec) => { calls.views.set(id, spec); return () => calls.views.delete(id); },
    replaceView: (b, v) => { if (v === null) calls.replaced.delete(b); else calls.replaced.set(b, v); return () => calls.replaced.delete(b); },
    registerNavSection: (s) => { calls.sections.push(s); return () => calls.sections.splice(calls.sections.indexOf(s), 1); },
    registerPaletteGroup: (g) => { calls.groups.push(g); return () => calls.groups.splice(calls.groups.indexOf(g), 1); },
    setStatusItems: (side, items) => { calls.status[side] = items; },
    setEdition: (e) => { calls.edition = e; },
    show: (id) => calls.shown.push(id),
    refresh: () => { calls.refreshes++; },
    toast: (t) => calls.toasts.push(t),
    on: () => () => {},
    ...over,
  };
  return { shell, calls };
}
const fakeApi = (status) => {
  const ok = (value) => Promise.resolve({ ok: true, value, error: "" });
  const none = { ok: false, value: null, error: "not in the app" };
  return {
    inApp: !!status,
    status: () => (status ? ok(status) : Promise.resolve(none)),
    listKinds: () => ok(JSON.stringify([{ name: "pro-trade-study" }])),
    machineId: () => ok("abc"),
    upgradeOffer: () => ok({ offer: false }),
    activate: () => ok(true), deactivate: () => ok(true),
  };
};
function attach(status = ACTIVE, over) {
  const { shell, calls } = fakeShell(over);
  const api = fakeApi(status);
  const S = createProState(api, () => NOW);
  const pro = attachToShell(shell, { S, api, doc: null, later: (fn) => fn() });
  return { shell, calls, S, pro };
}
const resolve = (v) => (typeof v === "function" ? v() : v);

test("hook: only a shell with the API, version 1 or later", () => {
  assert.equal(canAttach(undefined), false);
  assert.equal(canAttach({}), false);
  assert.equal(canAttach(fakeShell({ version: 0 }).shell), false);
  assert.equal(canAttach(fakeShell({ setEdition: undefined }).shell), false);
  assert.equal(canAttach(fakeShell().shell), true);
  assert.equal(attachToShell(fakeShell({ version: undefined }).shell, { S: {}, api: {} }), null);
});

test("hook: the Pro home stands in for the shell's home and its other-edition screen; the licence is a view", () => {
  const { calls } = attach();
  assert.deepEqual([...calls.views.keys()].sort(), [VIEWS.home, VIEWS.licence].sort());
  assert.equal(calls.replaced.get("home"), VIEWS.home);
  assert.equal(calls.replaced.get("upgrade"), VIEWS.home);
  assert.equal(calls.views.get(VIEWS.licence).title, "Licence");
  assert.equal(typeof calls.views.get(VIEWS.home).render, "function");
});

test("hook: the sidebar section, the licence item follows the licence", async () => {
  const { calls, pro } = attach();
  assert.equal(calls.sections.length, 1);
  const sec = calls.sections[0];
  assert.equal(sec.heading, "PRO");
  let items = resolve(sec.items);
  assert.deepEqual(items.map((i) => i.label), [...SIDE.map((s) => s.label), "Licence"]);
  assert.equal(items.find((i) => i.id === "fleet").badge, "SOON");
  const lic = items.at(-1);
  assert.equal(lic.view, VIEWS.licence);
  assert.equal(lic.badge.title, "NOT READ", "before the licence is read it is never shown as Pro");
  assert.equal(lic.badge.dot, toneColor("none"));
  await pro.refresh();
  assert.equal(calls.refreshes, 1, "a licence read redraws the shell");
  items = resolve(sec.items);
  assert.equal(items.at(-1).badge.title, "ACTIVE");
  assert.equal(items.at(-1).badge.dot, toneColor("ok"));
  // A job opens the Pro home; a product to come says so.
  items.find((i) => i.id === "forensics").onSelect();
  items.find((i) => i.id === "fleet").onSelect();
  assert.deepEqual(calls.shown, ["home", "home"]);
  assert.match(calls.toasts.at(-1), /Station fleet: coming/);
});

test("hook: the palette group holds the Pro actions and opens the licence screen", async () => {
  const { calls, pro } = attach();
  assert.equal(calls.groups.length, 1);
  const g = calls.groups[0];
  assert.equal(g.heading, "Kshana Pro");
  let items = resolve(g.items);
  assert.deepEqual(items.map((i) => i.label), proActions(licenceView(null, NOW)).map((a) => a.label));
  assert.ok(items.every((i) => i.tag === "PRO" && typeof i.onSelect === "function" && typeof i.icon === "function"));
  assert.ok(!items.some((i) => i.id === "pro-remove"), "nothing to remove before a licence is read");
  await pro.refresh();
  items = resolve(g.items);
  assert.ok(items.some((i) => i.id === "pro-remove"));
  items.find((i) => i.id === "pro-licence").onSelect();
  items.find((i) => i.id === "pro-home").onSelect();
  assert.deepEqual(calls.shown, [VIEWS.licence, "home"]);
});

test("hook: status bar and edition, read from the licence", async () => {
  const { calls, pro } = attach();
  let st = resolve(calls.status.right);
  assert.deepEqual(st.map((x) => x.text), ["Reading the licence…", "Licence not read"]);
  assert.equal(calls.status.left, undefined, "the shell's own items on the left stay");
  let ed = resolve(calls.edition);
  assert.equal(ed.appName, "Kshana Desktop Pro");
  assert.equal(ed.tag, "PRO");
  assert.equal(ed.text, "Reading the licence…");
  assert.equal(ed.action.label, "Activate a licence →");
  await pro.refresh();
  st = resolve(calls.status.right);
  assert.deepEqual(st.map((x) => x.text), ["Pro · renews 13 Nov 2027", "Licence checked on this machine"]);
  assert.equal(st[0].dot, toneColor("ok"));
  ed = resolve(calls.edition);
  assert.equal(ed.text, "Pro · renews 13 Nov 2027");
  assert.equal(ed.action.label, "Manage licence →");
  ed.action.onSelect();
  assert.deepEqual(calls.shown, [VIEWS.licence]);
});

test("hook: no licence read outside the app is never drawn as Pro", async () => {
  const { calls, pro } = attach(null);
  await pro.refresh();
  assert.equal(resolve(calls.status.right)[0].text, "Licence not read");
  assert.equal(resolve(calls.edition).action.label, "Activate a licence →");
  assert.equal(resolve(calls.sections[0].items).at(-1).badge.title, "NOT READ");
});

test("hook: detach gives every part back", () => {
  const { calls, pro } = attach();
  pro.detach();
  assert.equal(calls.views.size, 0);
  assert.equal(calls.replaced.size, 0);
  assert.equal(calls.sections.length, 0);
  assert.equal(calls.groups.length, 0);
  assert.equal(calls.status.right, null);
  assert.equal(calls.edition, null);
});

test("hook: the licence screen keeps what was typed unless it was opened or the licence changed", () => {
  assert.equal(redrawLicence(true, 3, 3), true);
  assert.equal(redrawLicence(false, 3, 3), false);
  assert.equal(redrawLicence(false, 4, 3), true);
});
