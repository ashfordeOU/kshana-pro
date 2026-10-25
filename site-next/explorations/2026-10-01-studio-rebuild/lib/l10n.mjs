// The Studio's message catalogue (design v2, sections 4 and 7): Project Fluent through the vendored @fluent/bundle,
// the language that follows the user, English fallback per id, Intl formatting, and the engine's own catalogue.
//
//   const l = createL10n({ base, fetchText, storage, languages, doc, registerEngine })
//   l.t(id, vars)            message text (English fallback per id, then the id itself)
//   await l.setLang(lang)    load ui, glossary and engine catalogues, update every [data-l10n], run the change hooks
//   l.apply(root)            re-apply to a subtree (after the page builds new elements)
//   l.onChange(fn)           fn(lang) after a language change; hooks re-render what the page builds in script
//   l.number(n, opts), l.date(d, opts)   Intl in the current language
//
// Markup: `data-l10n="id"` sets the text; `data-l10n-attr="aria-label:id1;title:id2"` sets attributes; `data-l10n-args='{"n":3}'`
// gives the variables. The English text in the HTML is the fallback until a catalogue is loaded.
import { FluentBundle, FluentResource } from "./vendor/fluent-bundle-0.19.1.mjs";

export const LANGS = ["nl", "pl", "pt-BR", "pt-PT", "fi", "sv", "da", "nb", "cs", "uk", "ru", "de", "fr", "es", "et", "ja", "zh-Hans"];
export const NAMES = { en: "English", de: "Deutsch", fr: "Français", es: "Español", "pt-BR": "Português (Brasil)", "pt-PT": "Português (Portugal)", nl: "Nederlands", da: "Dansk", sv: "Svenska", nb: "Norsk bokmål", pl: "Polski", cs: "Čeština", uk: "Українська", ru: "Русский", fi: "Suomi", et: "Eesti", ja: "日本語", "zh-Hans": "简体中文" };
export const STORAGE_KEY = "kshana-lang";
const RTL = new Set(["ar", "he", "fa", "ur"]);
export const dirOf = (lang) => (RTL.has(String(lang).split("-")[0]) ? "rtl" : "ltr");

/** First visit: the browser's languages in order, matched to the locales (the site's own rule, so both agree). */
export function negotiate(languages, available = LANGS) {
  const list = Array.isArray(languages) && languages.length ? languages : ["en"];
  for (const raw of list) {
    const t = String(raw).toLowerCase(), b = t.split("-")[0];
    if (b === "en") return "en";
    if (b === "pt") { const l = /^pt-(pt|ao|mz|cv|gw|st|tl|mo)$/.test(t) ? "pt-PT" : "pt-BR"; if (available.includes(l)) return l; continue; }
    if (b === "no" || b === "nn" || b === "nb") { if (available.includes("nb")) return "nb"; continue; }
    if (b === "zh") { if (/^zh-(tw|hk|mo|hant)/.test(t)) return "en"; if (available.includes("zh-Hans")) return "zh-Hans"; continue; }
    const hit = available.find((l) => l.toLowerCase() === b);
    if (hit) return hit;
  }
  return "en";
}

/** The language a page shows: ?lang= (also stored), the /<lang>/studio/ stub, the stored choice, then negotiation. */
export function chooseLang({ search = "", storage = null, languages = [], available = LANGS }) {
  const m = /[?&]lang=([\w-]+)/.exec(search);
  if (m && (m[1] === "en" || available.includes(m[1]))) { try { storage && storage.setItem(STORAGE_KEY, m[1]); } catch { /* per viewer, optional */ } return m[1]; }
  let s = null;
  try { s = storage && storage.getItem(STORAGE_KEY); } catch { s = null; }
  if (s && (s === "en" || available.includes(s))) return s;
  return negotiate(languages, available);
}

export function createL10n({ base, fetchText, storage = null, languages = [], doc = null, registerEngine = null, available = LANGS } = {}) {
  let lang = "en";
  let bundles = []; // current language first, English last
  const en = { bundle: null };
  const hooks = [];
  const state = { engineFallback: false, loaded: { en: false } };

  const load = async (code, files) => {
    const b = new FluentBundle(code, { useIsolating: false });
    let any = false;
    for (const f of files) {
      let text = null;
      try { text = await fetchText(`${base}${code}/${f}`); } catch { text = null; }
      if (text) { any = true; b.addResource(new FluentResource(text), { allowOverrides: true }); }
    }
    return any ? b : null;
  };

  const format = (id, vars) => {
    for (const b of bundles) {
      const m = b.getMessage(id);
      if (m && m.value) { try { return b.formatPattern(m.value, vars || {}); } catch { /* fall through to the next bundle */ } }
    }
    return id;
  };
  const attr = (id, name, vars) => {
    for (const b of bundles) {
      const m = b.getMessage(id);
      const a = m && m.attributes && m.attributes[name];
      if (a) { try { return b.formatPattern(a, vars || {}); } catch { /* next */ } }
    }
    return null;
  };

  const argsOf = (el) => { try { return JSON.parse(el.getAttribute("data-l10n-args") || "{}"); } catch { return {}; } };
  const apply = (root = doc) => {
    if (!root || !root.querySelectorAll) return 0;
    let n = 0;
    for (const el of root.querySelectorAll("[data-l10n]")) {
      const id = el.getAttribute("data-l10n");
      const s = format(id, argsOf(el));
      if (s !== id) { el.textContent = s; n++; }
    }
    for (const el of root.querySelectorAll("[data-l10n-attr]")) {
      for (const pair of String(el.getAttribute("data-l10n-attr")).split(";")) {
        const [name, id] = pair.split(":").map((x) => x && x.trim());
        if (!name || !id) continue;
        const s = format(id, argsOf(el));
        if (s !== id) { el.setAttribute(name, s); n++; }
      }
    }
    return n;
  };

  // The engine's own text: W55's WASM is English-only and takes a catalogue per tag (register_catalogue(tag, ftl_text)).
  const registerCurrent = async () => {
    if (!registerEngine) return;
    let ok = true;
    if (lang !== "en") {
      let text = null;
      try { text = await fetchText(`${base}${lang}/engine.ftl`); } catch { text = null; }
      if (text) { try { await registerEngine(lang, text); } catch { ok = false; } } else ok = false;
    }
    state.engineFallback = !ok;
  };

  const api = {
    get lang() { return lang; },
    get engineFallback() { return state.engineFallback; },
    t: (id, vars) => format(id, vars),
    attr,
    apply,
    onChange: (fn) => { hooks.push(fn); return () => hooks.splice(hooks.indexOf(fn), 1); },
    number: (n, opts) => new Intl.NumberFormat(lang === "en" ? "en" : lang, opts).format(n),
    date: (d, opts) => new Intl.DateTimeFormat(lang === "en" ? "en" : lang, opts).format(d),
    /** Load a language; English is always loaded as the fallback. Never throws: a failed fetch leaves English. */
    async setLang(code, { remember = false } = {}) {
      if (!en.bundle) en.bundle = await load("en", ["ui.ftl", "glossary.ftl"]);
      let b = null;
      state.engineFallback = false;
      if (code !== "en") {
        b = await load(code, ["ui.ftl", "glossary.ftl"]);
        if (!b) code = "en";
      }
      lang = code;
      bundles = [b, en.bundle].filter(Boolean);
      if (remember) { try { storage && storage.setItem(STORAGE_KEY, code); } catch { /* optional */ } }
      if (doc && doc.documentElement) { doc.documentElement.setAttribute("lang", code); doc.documentElement.setAttribute("dir", dirOf(code)); }
      await registerCurrent();
      apply();
      for (const fn of hooks.slice()) { try { fn(code); } catch { /* a hook must not stop the others */ } }
      return code;
    },
    /** The page calls this once the engine is ready: its catalogue for the current language is registered then, and on every change. */
    async attachEngine(fn) { registerEngine = fn; await registerCurrent(); for (const h of hooks.slice()) { try { h(lang); } catch { /* ignore */ } } },
    /** Choose and load the page's language: ?lang=, the stored choice, then the browser's languages. */
    async init(search = "") {
      const code = chooseLang({ search, storage, languages, available });
      return api.setLang(code);
    },
  };
  return api;
}
