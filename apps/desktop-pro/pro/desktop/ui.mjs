// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: the Pro home and the licence screen (0.42 U3, concept D) as DOM. Used
// by the overlay on the open Desktop shell (overlay.mjs) and by the standalone page
// (home.html) when the staged open tag has no shell. Every value reaches the page through
// textContent; the licence comes only from the app's `licence_status` command.
import { licenceView, jobsModel, proKinds, PRODUCTS, tokenFromFile } from "./model.mjs";

export const h = (tag, props = {}, ...kids) => {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (v === undefined || v === null || v === false) continue;
    if (k === "class") n.className = v;
    else if (k === "text") n.textContent = v;
    else if (k === "style") n.style.cssText = v;
    else if (k.startsWith("on")) n.addEventListener(k.slice(2), v);
    else n.setAttribute(k, v === true ? "" : v);
  }
  for (const c of kids.flat()) if (c !== null && c !== undefined && c !== false) n.append(c.nodeType ? c : document.createTextNode(String(c)));
  return n;
};
const SVGNS = "http://www.w3.org/2000/svg";
const ICONS = {
  home: ["M3 12 12 4l9 8", "M5 10v10h14V10"], key: ["M8 15a4 4 0 1 0 0-8 4 4 0 1 0 0 8", "M11.5 11H21M18 11v3M15 11v2"],
  evidence: ["M6 3h9l3 3v15H6z", "M9 12l2 2 4-4"], fleet: ["M6 4a2 2 0 1 0 0 4 2 2 0 1 0 0-4", "M18 4a2 2 0 1 0 0 4 2 2 0 1 0 0-4", "M12 16a2 2 0 1 0 0 4 2 2 0 1 0 0-4", "M7.5 7.5 11 16.5M16.5 7.5 13 16.5"],
  forensics: ["M11 5a6 6 0 1 0 0 12 6 6 0 1 0 0-12", "m20 20-4-4"], scorecards: ["M4 20V10M10 20V4M16 20v-7M22 20H2"],
  dossiers: ["M5 4h14v16H5z", "M9 8h6M9 12h6M9 16h4"], timing: ["M12 3a9 9 0 1 0 0 18 9 9 0 1 0 0-18", "M12 7v5l3 2"],
  copy: ["M8 8h12v12H8z", "M4 16V4h12"], file: ["M6 3h8l4 4v14H6z", "M14 3v4h4"], arrow: ["M5 12h14", "m13 6 6 6-6 6"],
  mark: ["M12 3a9 9 0 1 0 0 18 9 9 0 1 0 0-18", "M12 3v18M3 12h18M6 6l12 12"], theme: ["M12 3a9 9 0 1 0 9 9 7 7 0 0 1-9-9z"],
};
export function ic(name, cls = "i") {
  const s = document.createElementNS(SVGNS, "svg");
  s.setAttribute("viewBox", "0 0 24 24");
  s.setAttribute("class", cls);
  s.setAttribute("aria-hidden", "true");
  for (const d of ICONS[name] || []) { const p = document.createElementNS(SVGNS, "path"); p.setAttribute("d", d); s.append(p); }
  return s;
}

// ------------------------------------------------------------------ state
/** The Pro state read from the app: the licence, the kinds it runs, the machine ID, the offer. */
export function createProState(api, now = () => Math.floor(Date.now() / 1000)) {
  const S = { status: null, error: "", view: licenceView(null, now()), kinds: null, machineId: null, offer: null, result: null, read: false };
  S.refresh = async () => {
    const st = await api.status();
    S.status = st.ok ? st.value : null;
    S.error = st.ok ? "" : st.error;
    S.view = licenceView(S.status, now(), S.error);
    const k = await api.listKinds();
    S.kinds = k.ok ? proKinds(k.value) : null;
    if (S.machineId === null) { const m = await api.machineId(); S.machineId = m.ok ? m.value || "" : ""; }
    const o = await api.upgradeOffer();
    S.offer = o.ok && o.value && o.value.offer ? o.value : null;
    S.read = true;
    return S;
  };
  return S;
}

// ------------------------------------------------------------------ parts
const badge = (view) => h("span", { class: `lic-badge ${view.tone}`, text: view.badge });
function licenceRows(view) {
  return h("dl", { class: "lic-rows" }, ...view.rows.flatMap(([k, v]) => [h("dt", { text: k }), h("dd", { text: v, class: k === "Token" ? "mono" : null })]));
}

/** The licence card of the Pro home (concept D, right column). */
function licenceCard(S, nav) {
  const v = S.view;
  return h("section", { class: "card lic-card", "aria-labelledby": "lic-card-h" },
    h("div", { class: "lic-head" }, h("h2", { id: "lic-card-h", text: "Licence" }), badge(v)),
    h("p", { class: "lic-headline", text: v.headline }),
    licenceRows(v),
    v.warning ? h("p", { class: `lic-warning ${v.tone}`, role: v.tone === "bad" ? "alert" : null, text: v.warning }) : null,
    h("div", { class: "row" },
      h("button", { class: v.unlocked ? "btn" : "btn primary", type: "button", id: "pro-manage", onclick: () => nav.licence() }, ic("key"), v.unlocked ? "Manage licence" : v.state === "none" ? "Activate a licence" : "Renew or replace")));
}

/** The Pro home (concept D). `adopt` holds nodes from the open shell's home: recent runs, questions. */
export function proHome(S, nav, adopt = {}) {
  const v = S.view;
  const jobs = jobsModel(S.kinds, v.unlocked);
  const chip = { ready: ["READY", "ok"], locked: ["LICENCE NEEDED", "warn"], coming: ["COMING", "none"] };
  const job = (j) => h("button", { class: `card job ${j.state}`, type: "button", "data-job": j.id,
    "aria-describedby": `job-${j.id}-state`,
    onclick: () => (j.state === "ready" ? nav.job(j) : j.state === "locked" ? nav.licence() : nav.toast(`${j.title}: coming in a later Desktop Pro release.`)) },
  h("span", { class: "job-top" }, ic(j.icon), h("span", { class: `state-chip ${chip[j.state][1]}`, id: `job-${j.id}-state`, text: chip[j.state][0] })),
  h("b", { text: j.title }), h("span", { class: "job-text", text: j.text }),
  j.state === "ready" && j.kinds.length ? h("span", { class: "job-kinds mono", text: j.kinds.slice(0, 3).join(" · ") + (j.kinds.length > 3 ? ` +${j.kinds.length - 3}` : "") }) : null);

  const product = (p) => h("div", { class: "card product" },
    h("h3", {}, ic(p.icon), p.title, h("span", { class: `state-chip ${p.state === "available" ? "ok" : "none"}`, text: p.state === "available" ? "IN THIS BUILD" : "COMING" })),
    h("p", { text: p.text }),
    p.view && nav.view ? h("button", { class: "linkish", type: "button", onclick: () => nav.view(p.view) }, p.id === "evidence" ? "Open a pack →" : `Open the ${p.title.toLowerCase()} →`)
      : p.id === "evidence" && nav.evidence ? h("button", { class: "linkish", type: "button", onclick: () => nav.evidence() }, "Open the evidence packs →") : null);

  const recent = adopt.recent && adopt.recent.length ? adopt.recent
    : [h("p", { class: "empty", text: adopt.shell ? "No runs yet this session. Open a scenario below, or press ⌘K / Ctrl+K." : "Runs appear here in the Desktop shell. This build's open Studio has no shell yet: open the Studio to run a scenario." })];

  return h("section", { class: "page pro-page", "aria-labelledby": "home-h", "data-pro": "home" },
    h("div", { class: "pro-home" },
      h("div", { class: "pro-main" },
        h("p", { class: "kicker", text: `Kshana Desktop Pro · ${v.unlocked ? `${v.edition} licence` : "no Pro licence in force"}` }),
        h("h1", { id: "home-h", text: "What do you need to prove today?" }),
        h("p", { class: "lede", text: "Every run uses the engine on this machine. Pro scenarios run under the licence on the right; the open scenarios all run without one." }),
        h("div", { class: "jobs" }, ...jobs.map(job)),
        h("h2", { text: "Recent runs" }),
        ...recent,
        h("h2", { text: "Pro products" }),
        h("div", { class: "grid" }, ...PRODUCTS.map(product)),
        adopt.questions && adopt.questions.length ? [h("h2", { text: "Open scenarios" }), ...adopt.questions] : null),
      h("aside", { class: "pro-side", "aria-label": "Licence" }, licenceCard(S, nav))));
}

/** The licence screen: status, activate (paste or file), this machine's ID, remove, the offer. */
export function licenceScreen(S, nav, refresh) {
  const v = S.view;
  const result = h("p", { class: `lic-result ${S.result ? S.result.tone : ""}`, role: "status", "aria-live": "polite", text: S.result ? S.result.text : "" });
  const token = h("textarea", { id: "pro-token", spellcheck: "false", autocomplete: "off", rows: "4", "aria-describedby": "pro-token-help", placeholder: "Paste the licence token" });
  const file = h("input", { type: "file", id: "pro-lic-file", class: "vh", accept: ".txt,.lic,.licence,.license,.key,.token,text/plain" });
  const busy = (on) => { for (const id of ["pro-activate", "pro-import"]) { const b = document.getElementById(id); if (b) b.disabled = on || null; } };
  const activate = async (text, from) => {
    const t = String(text || "").trim();
    if (!t) { S.result = { tone: "bad", text: from === "file" ? "That file holds no licence token." : "Paste a licence token first." }; await refresh(); return; }
    busy(true);
    const r = await nav.api.activate(t);
    S.result = r.ok ? { tone: "ok", text: from === "file" ? "Licence imported from the file and activated on this machine." : "Licence activated on this machine." } : { tone: "bad", text: r.error };
    await refresh();
  };
  file.addEventListener("change", async () => {
    const f = file.files && file.files[0];
    if (!f) return;
    if (f.size > 64 * 1024) { S.result = { tone: "bad", text: "A licence file is a few kilobytes; this one is too large." }; await refresh(); return; }
    await activate(tokenFromFile(await f.text()), "file");
  });
  const remove = h("button", { class: "btn danger", type: "button", id: "pro-remove", onclick: async (e) => {
    const b = e.currentTarget;
    if (b.dataset.confirm !== "1") { b.dataset.confirm = "1"; b.textContent = "Remove it: Pro scenarios lock on this machine"; return; }
    const r = await nav.api.deactivate();
    S.result = r.ok ? { tone: "ok", text: "The licence was removed from this machine." } : { tone: "bad", text: r.error };
    await refresh();
  } }, "Remove this licence");

  return h("section", { class: "page pro-page", "aria-labelledby": "lic-h", "data-pro": "licence" },
    h("p", { class: "kicker", text: "Kshana Desktop Pro" }),
    h("h1", { id: "lic-h", text: "Licence" }),
    h("p", { class: "lede", text: "Kshana Desktop Pro checks its licence on this machine, offline. Activating, importing or removing a licence sends nothing anywhere." }),
    h("div", { class: "lic-grid" },
      h("section", { class: "card lic-card", "aria-labelledby": "lic-status-h" },
        h("div", { class: "lic-head" }, h("h2", { id: "lic-status-h", text: "Status" }), badge(v)),
        h("p", { class: "lic-headline", text: v.headline }),
        licenceRows(v),
        v.warning ? h("p", { class: `lic-warning ${v.tone}`, text: v.warning }) : null),
      h("div", { class: "lic-actions" },
        h("section", { class: "card", "aria-labelledby": "lic-act-h" },
          h("h2", { id: "lic-act-h", text: v.state === "none" || v.state === "unavailable" ? "Activate a licence" : "Activate another licence" }),
          h("p", { id: "pro-token-help", text: "Paste the token from your licence e-mail or from your administrator, or import the licence file. The new licence replaces the one on this machine." }),
          token,
          h("div", { class: "row" },
            h("button", { class: "btn primary", type: "button", id: "pro-activate", disabled: !nav.api.inApp || null, onclick: () => activate(token.value, "paste") }, "Activate"),
            file,
            h("label", { for: "pro-lic-file", class: "btn", id: "pro-import", role: "button", tabindex: nav.api.inApp ? "0" : "-1", "aria-disabled": nav.api.inApp ? null : "true",
              onkeydown: (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); file.click(); } } }, ic("file"), "Import a licence file…")),
          result),
        h("section", { class: "card", "aria-labelledby": "lic-mid-h" },
          h("h2", { id: "lic-mid-h", text: "This machine" }),
          h("p", { text: "For a licence bound to one machine, send this ID with your order. It is a hash; it names no person." }),
          h("div", { class: "copyline" }, h("code", { id: "pro-mid", text: S.machineId || (nav.api.inApp ? "not readable on this machine" : "read inside the app") }),
            S.machineId ? h("button", { class: "btn", type: "button", onclick: () => nav.copy(S.machineId) }, ic("copy"), "Copy") : null)),
        v.canRemove ? h("section", { class: "card", "aria-labelledby": "lic-rm-h" },
          h("h2", { id: "lic-rm-h", text: "Remove" }),
          h("p", { text: "Removing the licence moves it off this machine, for example before you activate it on another one. The open features keep working." }),
          h("div", { class: "row" }, remove)) : null,
        S.offer ? h("section", { class: "card", "aria-labelledby": "lic-up-h" },
          h("h2", { id: "lic-up-h", text: "Upgrade" }),
          h("p", { text: `${S.offer.name}: ${S.offer.price}` }),
          h("a", { class: "btn", href: S.offer.url, target: "_blank", rel: "noopener noreferrer" }, "Open the checkout")) : null)));
}
