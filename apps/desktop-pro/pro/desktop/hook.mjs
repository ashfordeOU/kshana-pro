// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro through the open shell's extension API (window.KshanaShell, the open
// repository's apps/desktop/shell/lib/ext.mjs, documented in apps/desktop/README.md). The
// overlay (overlay.mjs) uses this when the staged shell has the API and keeps its markup
// path for older shells. Registration touches no DOM: every node is made in a render or a
// function the shell calls, so attachToShell is tested in node (hook.test.mjs) with a fake shell.
import { proActions } from "./model.mjs";
import { ic, proHome, licenceScreen } from "./ui.mjs";

/** The Pro items of the sidebar: four jobs on the Pro home, one product still to come. */
export const SIDE = [
  { id: "forensics", icon: "forensics", label: "Incident forensics" },
  { id: "timing", icon: "timing", label: "Timing assurance" },
  { id: "scorecard", icon: "scorecards", label: "Scorecards" },
  { id: "study", icon: "dossiers", label: "Studies" },
  { id: "fleet", icon: "fleet", label: "Station fleet", coming: true },
];
/** The colour of a licence tone, as the shell's tokens name it. */
export const toneColor = (tone) => ({ ok: "var(--itg)", warn: "var(--nav)", bad: "var(--int)" })[tone] || "var(--ink-3)";
const PAL_ICON = { "pro-home": "home", "pro-import": "file", "pro-machine": "copy" };
export const VIEWS = { home: "pro-home", licence: "pro-licence" };
const REQUIRED = ["registerView", "replaceView", "registerNavSection", "registerPaletteGroup", "setStatusItems", "setEdition", "show", "refresh"];

/** True when `shell` is a KshanaShell this module can attach to (version 1 or later, every call it uses). */
export function canAttach(shell) {
  return !!shell && Number(shell.version) >= 1 && REQUIRED.every((m) => typeof shell[m] === "function");
}

/**
 * Whether the licence screen draws itself again on a render of the shell: when it is opened,
 * and when the licence (or the result of an action on it) changed since it last drew.
 * Otherwise its fields keep what the user typed.
 */
export const redrawLicence = (fresh, stamp, drawnStamp) => !!fresh || stamp !== drawnStamp;

/**
 * Attach the Pro home, the licence screen, the sidebar section, the palette group, the status
 * items and the edition to `shell`. `S` is createProState(api) (ui.mjs). Returns
 * { refresh, nav, detach }: refresh reads the licence again and redraws.
 */
export function attachToShell(shell, { S, api, doc = globalThis.document, later = (fn) => (globalThis.requestAnimationFrame || setTimeout)(fn) }) {
  if (!canAttach(shell)) return null;
  const byId = (id) => (doc ? doc.getElementById(id) : null);
  let stamp = 0, drawnStamp = -1;
  const toast = (text) => (typeof shell.toast === "function" ? shell.toast(text) : undefined);
  async function copy(text) {
    try { await navigator.clipboard.writeText(text); toast("Copied."); } catch { toast("Could not copy: select the text and copy it."); }
  }
  async function refresh() {
    await S.refresh();
    stamp++;
    shell.refresh();
  }
  const nav = {
    api, toast, copy,
    licence: () => shell.show(VIEWS.licence),
    evidence: () => shell.show("evidence"),
    job: (j) => { shell.show("files"); toast(`${j.title}: open a scenario file of a Pro kind (${j.kinds[0]}); it runs under this licence.`); },
  };
  const readText = () => (S.read ? S.view.statusText : "Reading the licence…");
  const off = [];

  off.push(shell.registerView(VIEWS.home, {
    title: "",
    render(el, ctx) {
      const recent = ctx.parts && ctx.parts.recentRuns ? ctx.parts.recentRuns() : null;
      const grid = ctx.parts && ctx.parts.scenarioGrid ? ctx.parts.scenarioGrid() : null;
      return proHome(S, nav, { shell: true, recent: recent ? [recent] : [], questions: grid ? [grid] : [] });
    },
  }));
  off.push(shell.registerView(VIEWS.licence, {
    title: "Licence",
    render(el, ctx) {
      if (!redrawLicence(ctx.fresh, stamp, drawnStamp)) return;
      const keep = doc.activeElement && doc.activeElement.id;
      const typed = byId("pro-token") ? byId("pro-token").value : "";
      el.replaceChildren(licenceScreen(S, nav, refresh));
      drawnStamp = stamp;
      if (typed && byId("pro-token") && !(S.result && S.result.tone === "ok")) byId("pro-token").value = typed;
      if (keep && byId(keep) && el.contains(byId(keep))) byId(keep).focus({ preventScroll: true });
    },
  }));
  // The Pro home stands where the shell shows its home and its screen on other editions.
  off.push(shell.replaceView("home", VIEWS.home));
  off.push(shell.replaceView("upgrade", VIEWS.home));

  off.push(shell.registerNavSection({
    heading: "PRO",
    items: () => [
      ...SIDE.map((p) => ({
        id: p.id, label: p.label, icon: p.icon, badge: p.coming ? "SOON" : null,
        onSelect: () => {
          shell.show("home");
          if (p.coming) { toast(`${p.label}: coming in a later Desktop Pro release.`); return; }
          later(() => { const card = doc && doc.querySelector(`[data-job="${p.id}"]`); if (card) card.focus(); });
        },
      })),
      { id: "licence", label: "Licence", icon: () => ic("key"), badge: { dot: toneColor(S.view.tone), title: S.view.badge }, view: VIEWS.licence },
    ],
  }));

  function execPro(a) {
    if (a.id === "pro-home") { shell.show("home"); return; }
    shell.show(VIEWS.licence);
    if (a.id === "pro-import") byId("pro-lic-file")?.click();
    else if (a.id === "pro-machine") { if (S.machineId) copy(S.machineId); else toast("This machine's ID could not be read."); }
    else if (a.id === "pro-remove") byId("pro-remove")?.focus();
    else byId("pro-token")?.focus();
  }
  off.push(shell.registerPaletteGroup({
    heading: "Kshana Pro",
    items: () => proActions(S.view).map((a) => ({ id: a.id, label: a.label, keywords: a.keywords, tag: "PRO", icon: () => ic(PAL_ICON[a.id] || "key"), onSelect: () => execPro(a) })),
  }));

  shell.setStatusItems("right", () => [
    { text: readText(), dot: toneColor(S.view.tone) },
    { text: S.view.state === "unavailable" ? "Licence not read" : "Licence checked on this machine" },
  ]);
  shell.setEdition(() => ({
    appName: "Kshana Desktop Pro", tag: "PRO", title: "Kshana Desktop Pro", text: readText(),
    action: { label: S.view.unlocked ? "Manage licence →" : "Activate a licence →", onSelect: () => shell.show(VIEWS.licence) },
  }));

  const detach = () => {
    for (const f of off) if (typeof f === "function") f();
    shell.setStatusItems("right", null);
    shell.setEdition(null);
  };
  return { refresh, nav, detach, execPro };
}
