// Loads the page's catalogue and exposes it as window.kshanaL10n; included by every Studio page as a module script.
import { createL10n } from "./l10n.mjs";

const base = new URL("../../i18n/", import.meta.url).href; // <site>/i18n/<lang>/ (the port copies i18n/ next to studio/)
const l10n = createL10n({
  base,
  fetchText: async (u) => { const r = await fetch(u); if (!r.ok) throw new Error(String(r.status)); return r.text(); },
  storage: (() => { try { return window.localStorage; } catch { return null; } })(),
  languages: navigator.languages ? [...navigator.languages] : [navigator.language || "en"],
  doc: document,
});
window.kshanaL10n = l10n; // the engine page calls l10n.attachEngine((tag, text) => wasm.register_catalogue(tag, text)) when it is ready
const go = () => l10n.init(location.search);
if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", go, { once: true }); else go();
export default l10n;
