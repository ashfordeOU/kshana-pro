// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro on the open Desktop shell (0.42 U3). The open shell (the open
// repository's apps/desktop/shell, staged as studio/desktop/) is used unchanged; the Pro
// build adds this module and pro.css to the staged copy of its index.html
// (apps/desktop-pro/build-frontend.sh).
//
// A shell with the extension API (window.KshanaShell, announced by "kshana-shell-ready")
// gets the Pro parts through it (hook.mjs). For an older shell without it, this module keeps
// its first way, below: it reads the shell's markup and re-applies after each render. Both:
//   - turns the shell's Home into the Pro home (concept D), keeping the shell's own recent
//     runs table and scenario grid inside it, so the runs shown are the session's real runs;
//   - adds the licence screen as a Pro page in the centre view;
//   - replaces the shell's Pro section, edition box and edition chip in the sidebar and the
//     status bar with the licence actually read from the app;
//   - adds Pro rows to the ⌘K / Ctrl+K palette (mouse and arrow keys);
//   - shows the Pro home where the free shell shows its upgrade screen.
// On an older shell, which rebuilds its regions on every render, a MutationObserver re-applies
// the Pro parts each time. Nothing here changes how the shell runs scenarios.
import { appInvoke, licenceApi } from "./api.mjs";
import { proActions, actionMatches, palStep } from "./model.mjs";
import { h, ic, createProState, proHome, licenceScreen } from "./ui.mjs";
import { SIDE, toneColor, canAttach, attachToShell } from "./hook.mjs";

const $ = (id) => document.getElementById(id);
const api = licenceApi(appInvoke());
const S = createProState(api);
const R = { route: null, proSel: -1, shellIdx: -1 };
// The shell's regions are watched for its re-renders; the Pro module's own changes are taken
// off the queue right after they are made (mutate), so they never wake the observer.
const obs = new MutationObserver(() => applyAll(false));
function mutate(fn) {
  try { return fn(); } finally { obs.takeRecords(); }
}
const SHELL_NAV = ["home", "runs", "library", "devices", "evidence", "files"]; // the open shell's NAV order (U1)

// ------------------------------------------------------------------ shell hooks (by the DOM)
const shellNavItems = () => [...document.querySelectorAll("#sidebar > .nav-item:not([data-pro])")];
function shellGo(id) {
  const btn = shellNavItems()[SHELL_NAV.indexOf(id)];
  if (btn) btn.click();
}
let toastTimer = 0;
function toast(text) {
  const el = $("toast");
  if (!el) return;
  el.textContent = text; el.classList.add("on");
  clearTimeout(toastTimer); toastTimer = setTimeout(() => el.classList.remove("on"), 2600);
}
async function copy(text) {
  try { await navigator.clipboard.writeText(text); toast("Copied."); } catch { toast("Could not copy: select the text and copy it."); }
}
const nav = {
  api, toast, copy,
  licence: () => showPro("licence"),
  evidence: () => shellGo("evidence"),
  job: (j) => { shellGo("files"); toast(`${j.title}: open a scenario file of a Pro kind (${j.kinds[0]}); it runs under this licence.`); },
};

// ------------------------------------------------------------------ Pro routes
function showPro(route) {
  R.route = route;
  mutate(() => { paintView(true); decorateSidebar(true); });
  const v = $("view");
  if (v) { v.scrollTop = 0; v.focus({ preventScroll: true }); }
}
async function refresh() {
  await S.refresh();
  applyAll(true);
}

/** The centre view. `force` repaints a Pro page already shown (after the licence changed). */
function paintView(force) {
  const v = $("view");
  if (!v) return;
  const first = v.firstElementChild;
  const ours = first && first.getAttribute("data-pro");
  if (R.route === "licence") {
    if (ours === "licence" && !force) return;
    v.removeAttribute("role"); v.removeAttribute("aria-labelledby");
    const keep = document.activeElement && document.activeElement.id;
    const typed = $("pro-token") ? $("pro-token").value : "";
    v.replaceChildren(licenceScreen(S, nav, refresh));
    if (typed && $("pro-token") && !(S.result && S.result.tone === "ok")) $("pro-token").value = typed;
    if (keep && $(keep) && v.contains($(keep))) $(keep).focus({ preventScroll: true });
    return;
  }
  if (!first) return;
  if (ours === "home") {
    if (!force) return;
    // Repaint with the new licence, moving the shell's nodes across again.
    const adopted = (sel) => [...first.querySelectorAll(`[data-adopted="${sel}"]`)];
    v.replaceChildren(proHome(S, nav, { shell: true, recent: adopted("recent"), questions: adopted("questions") }));
    return;
  }
  // The shell's home: the Pro home, with the shell's runs table and scenario grid inside.
  if (first.matches('section[aria-labelledby="home-h"]')) {
    const recent = first.querySelector("table.runs");
    const grid = first.querySelector(":scope > .grid");
    if (recent) recent.dataset.adopted = "recent";
    if (grid) grid.dataset.adopted = "questions";
    v.replaceChildren(proHome(S, nav, { shell: true, recent: recent ? [recent] : [], questions: grid ? [grid] : [] }));
    return;
  }
  // The free shell's upgrade screen has no place in Pro: the Pro home says what this licence runs.
  if (first.matches('section[aria-labelledby="up-h"]')) v.replaceChildren(proHome(S, nav, { shell: true }));
}

// ------------------------------------------------------------------ sidebar and status bar
function decorateSidebar(force) {
  const nav$ = $("sidebar");
  if (!nav$) return;
  const head = $("pro-head");
  const brand = nav$.querySelector(".brand");
  if (brand && !brand.querySelector("[data-pro]")) brand.querySelector("b")?.after(h("span", { class: "pro-tag", "data-pro": "1", text: "PRO" }));
  if (head) {
    // The shell's Pro items (upgrade links) and edition box give way to the Pro ones.
    let n = head.nextElementSibling;
    while (n) { const next = n.nextElementSibling; if (!n.hasAttribute("data-pro")) n.remove(); n = next; }
    if (force) for (const n of nav$.querySelectorAll("[data-pro-side], .pro-edition")) n.remove();
    if (!nav$.querySelector("[data-pro-side]")) {
      const item = (p) => h("button", { class: "nav-item", type: "button", "data-pro": "1", "data-pro-side": p.id,
        onclick: () => {
          if (p.coming) { shellGo("home"); toast(`${p.label}: coming in a later Desktop Pro release.`); return; }
          shellGo("home");
          requestAnimationFrame(() => { const card = document.querySelector(`[data-job="${p.id}"]`); if (card) card.focus(); });
        } }, ic(p.icon), h("span", { text: p.label }), p.coming ? h("span", { class: "soon", text: "SOON" }) : null);
      const lic = h("button", { class: "nav-item", type: "button", "data-pro": "1", "data-pro-side": "licence", onclick: () => showPro("licence") },
        ic("key"), h("span", { text: "Licence" }), h("span", { class: `lic-dot ${S.view.tone}`, title: S.view.badge }));
      const box = h("div", { class: "edition pro-edition", "data-pro": "1" },
        h("b", { text: "Kshana Desktop Pro" }),
        h("p", { text: S.read ? S.view.statusText : "Reading the licence…" }),
        h("button", { class: "linkish", type: "button", "data-pro": "1", onclick: () => showPro("licence") }, S.view.unlocked ? "Manage licence →" : "Activate a licence →"));
      head.after(...SIDE.map(item), lic, box);
    }
  }
  // Which item is current: a Pro page takes it from the shell's.
  for (const b of shellNavItems()) if (R.route) b.removeAttribute("aria-current");
  for (const b of nav$.querySelectorAll("[data-pro-side]")) {
    if (b.dataset.proSide === R.route) b.setAttribute("aria-current", "page"); else b.removeAttribute("aria-current");
  }
}
function decorateStatus(force) {
  const bar = $("statusbar");
  if (!bar) return;
  if (force) for (const n of bar.querySelectorAll("[data-pro]")) n.remove();
  if (bar.querySelector("[data-pro]")) return;
  const grow = bar.querySelector(".grow");
  if (!grow) return;
  const after = [];
  for (let n = grow.nextElementSibling; n; n = n.nextElementSibling) if (n.classList.contains("st")) after.push(n);
  const tone = toneColor(S.view.tone);
  const ed = h("span", { class: "st", "data-pro": "1" }, h("span", { class: "dot", style: `--c:${tone}` }), S.read ? S.view.statusText : "Reading the licence…");
  const chk = h("span", { class: "st", "data-pro": "1", text: S.view.state === "unavailable" ? "Licence not read" : "Licence checked on this machine" });
  if (after[0]) after[0].replaceWith(ed); else grow.after(ed);
  if (after[1]) after[1].replaceWith(chk); else ed.after(chk);
}

// ------------------------------------------------------------------ palette
let palRows = [];
function palQuery() { const q = $("pal-q"); return q ? q.value : ""; }
const shellOpts = () => [...document.querySelectorAll("#pal-list > .pal-opt:not([data-pro])")];
function decoratePalette() {
  const list = $("pal-list");
  if (!list || list.querySelector("[data-pro]")) return;
  const q = palQuery();
  const newTabOnly = ($("pal-q")?.placeholder || "").toLowerCase().includes("new tab");
  palRows = newTabOnly ? [] : proActions(S.view).filter((a) => actionMatches(a, q));
  if (!palRows.length) { R.proSel = -1; return; }
  list.querySelector(".pal-empty")?.remove();
  list.append(h("li", { class: "pal-group", role: "presentation", "data-pro": "1", text: "Kshana Pro" }),
    ...palRows.map((a, i) => h("li", { class: "pal-opt", role: "option", id: `pal-pro-${i}`, "data-pro": "1", "aria-selected": "false",
      onmousemove: () => { if (R.proSel !== i) { R.proSel = i; paintPalSel(); } }, onclick: () => execPro(a) },
    ic(a.id === "pro-home" ? "home" : a.id === "pro-import" ? "file" : a.id === "pro-machine" ? "copy" : "key"),
    h("span", { class: "lbl", text: a.label }), h("span", { class: "pro-tag", text: "PRO" }))));
  if (R.proSel >= palRows.length) R.proSel = -1;
  paintPalSel();
}
function paintPalSel() {
  const input = $("pal-q");
  const pro = [...document.querySelectorAll("#pal-list > .pal-opt[data-pro]")];
  if (R.proSel < 0) {
    for (const o of pro) o.setAttribute("aria-selected", "false");
    if (R.shellIdx >= 0) { const o = shellOpts()[R.shellIdx]; if (o) { o.setAttribute("aria-selected", "true"); input?.setAttribute("aria-activedescendant", o.id); } R.shellIdx = -1; }
    return;
  }
  for (const o of shellOpts()) o.setAttribute("aria-selected", "false");
  pro.forEach((o, i) => o.setAttribute("aria-selected", i === R.proSel ? "true" : "false"));
  const sel = pro[R.proSel];
  if (sel) { input?.setAttribute("aria-activedescendant", sel.id); sel.scrollIntoView({ block: "nearest" }); }
}
function closePaletteThen(fn) {
  const q = $("pal-q");
  if (q) q.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
  R.proSel = -1;
  fn();
}
function execPro(a) {
  closePaletteThen(() => {
    if (a.id === "pro-home") { R.route = null; shellGo("home"); return; }
    showPro("licence");
    if (a.id === "pro-import") $("pro-lic-file")?.click();
    else if (a.id === "pro-machine") { if (S.machineId) copy(S.machineId); else toast("This machine's ID could not be read."); }
    else if (a.id === "pro-remove") $("pro-remove")?.focus();
    else $("pro-token")?.focus();
  });
}
function onPaletteKey(e) {
  if (!e.target || e.target.id !== "pal-q") return;
  if (!["ArrowDown", "ArrowUp", "Enter"].includes(e.key)) return;
  const opts = shellOpts();
  const shellIndex = R.proSel >= 0 ? R.shellIdx : opts.findIndex((o) => o.getAttribute("aria-selected") === "true");
  const step = palStep(e.key, { proSel: R.proSel, shellIndex, shellCount: opts.length, proCount: palRows.length });
  if (R.proSel < 0 && step.proSel >= 0) R.shellIdx = shellIndex;
  const leavingDown = R.proSel >= 0 && step.proSel < 0 && !step.consume;
  R.proSel = step.proSel;
  if (step.exec) { e.preventDefault(); e.stopPropagation(); execPro(palRows[R.proSel]); return; }
  if (leavingDown) { R.shellIdx = -1; return; } // the shell moves from its last row to its first and repaints
  if (step.consume) { e.preventDefault(); e.stopPropagation(); paintPalSel(); }
}

// ------------------------------------------------------------------ wiring
function applyAll(force) {
  mutate(() => {
    paintView(force);
    decorateSidebar(force);
    decorateStatus(force);
    decoratePalette();
    if (/Kshana Desktop$/.test(document.title)) document.title = document.title.replace(/Kshana Desktop$/, "Kshana Desktop Pro");
  });
}
// The user leaving a Pro page through the shell's own controls (sidebar, tabs, palette rows).
function leaveOnShellNav(e) {
  if (!R.route) return;
  if (e.type === "keydown" && !["Enter", " ", "ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) return;
  const t = e.target && e.target.closest && e.target.closest("#sidebar button, #doc-tabs button, #tab-new, #pal-list .pal-opt, #pal-q");
  if (!t || t.closest("[data-pro]") || t.hasAttribute("data-pro")) return;
  if (t.id === "pal-q" && (e.key !== "Enter" || R.proSel >= 0)) return;
  R.route = null;
}

// The markup path, for a shell without the extension API.
function start() {
  if (!$("app") || !$("view") || !$("sidebar")) return false;
  document.documentElement.dataset.edition = "pro";
  for (const id of ["view", "sidebar", "statusbar"]) obs.observe($(id), { childList: true });
  const layer = $("palette-layer");
  if (layer) {
    obs.observe(layer, { childList: true, subtree: true });
    layer.addEventListener("keydown", onPaletteKey, true);
    layer.addEventListener("input", () => { R.proSel = -1; R.shellIdx = -1; }, true);
    layer.addEventListener("mousemove", (e) => { const o = e.target.closest && e.target.closest(".pal-opt"); if (o && !o.hasAttribute("data-pro") && R.proSel >= 0) { R.proSel = -1; R.shellIdx = -1; paintPalSel(); } }, true);
  }
  document.addEventListener("click", leaveOnShellNav, true);
  document.addEventListener("keydown", leaveOnShellNav, true);
  applyAll(false);
  refresh();
  setInterval(refresh, 10 * 60 * 1000); // a licence can lapse while the window is open
  return true;
}

// ------------------------------------------------------------------ which way
let mode = null;
function startHook(shell) {
  if (mode || !canAttach(shell)) return false;
  const pro = attachToShell(shell, { S, api });
  if (!pro) return false;
  mode = "hook";
  document.documentElement.dataset.edition = "pro";
  document.documentElement.dataset.proOverlay = "hook";
  pro.refresh();
  setInterval(pro.refresh, 10 * 60 * 1000); // a licence can lapse while the window is open
  return true;
}
function startMarkup() {
  if (mode) return;
  if (start()) { mode = "markup"; document.documentElement.dataset.proOverlay = "markup"; }
}
if (!startHook(globalThis.KshanaShell)) {
  // A shell that loads after this module announces itself; one that ran before it and did not
  // (an older shell) has no API, so the markup path starts once the page's scripts have run.
  addEventListener("kshana-shell-ready", (e) => startHook(e.detail), { once: true });
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", startMarkup, { once: true });
  else startMarkup();
}
