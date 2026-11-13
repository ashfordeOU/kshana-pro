// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// The standalone Pro page (../home.html): the Pro home and the licence screen without the
// open Desktop shell, for a build staged from an open tag that has no shell yet. Recent runs
// have no source here (the Studio pages keep their own), so the home says so instead.
import { appInvoke, licenceApi } from "./api.mjs";
import { ic, createProState, proHome, licenceScreen } from "./ui.mjs";

const $ = (id) => document.getElementById(id);
const api = licenceApi(appInvoke());
const S = createProState(api);
let toastTimer = 0;
function toast(text) {
  const el = $("toast");
  el.textContent = text; el.classList.add("on");
  clearTimeout(toastTimer); toastTimer = setTimeout(() => el.classList.remove("on"), 2600);
}
async function copy(text) {
  try { await navigator.clipboard.writeText(text); toast("Copied."); } catch { toast("Could not copy: select the text and copy it."); }
}
const nav = {
  api, toast, copy,
  licence: () => { location.hash = "licence"; },
  evidence: () => { location.href = "../studio/tools/index.html"; },
  job: (j) => toast(`${j.title}: open a scenario file of a Pro kind (${j.kinds[0]}) in the Studio; it runs under this licence.`),
};
const route = () => (location.hash === "#licence" ? "licence" : "home");

function paint() {
  const r = route();
  const typed = $("pro-token") ? $("pro-token").value : "";
  $("view").replaceChildren(r === "licence" ? licenceScreen(S, nav, refresh) : proHome(S, nav, { shell: false }));
  if (typed && $("pro-token") && !(S.result && S.result.tone === "ok")) $("pro-token").value = typed;
  for (const id of ["nav-home", "nav-licence"]) $(id).removeAttribute("aria-current");
  $(r === "home" ? "nav-home" : "nav-licence").setAttribute("aria-current", "page");
  document.title = r === "licence" ? "Licence · Kshana Desktop Pro" : "Kshana Desktop Pro";
}
async function refresh() { await S.refresh(); paint(); }

const theme = $("btn-theme");
theme.append(ic("theme"));
theme.addEventListener("click", () => {
  const root = document.documentElement;
  const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
  root.dataset.theme = dark ? "light" : "dark";
  try { localStorage.setItem("kshana-theme", root.dataset.theme); } catch { /* not remembered */ }
});
addEventListener("hashchange", () => { S.result = null; paint(); $("view").focus({ preventScroll: true }); });
paint();
refresh();
