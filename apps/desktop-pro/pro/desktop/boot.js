// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// The standalone Pro page, before first paint: the theme this window remembers (the Studio's
// key, so the Studio and Desktop Pro follow one choice). External: the CSP allows app scripts only.
try {
  var kt = localStorage.getItem("kshana-theme");
  if (kt === "dark" || kt === "light") document.documentElement.dataset.theme = kt;
} catch (e) { /* no storage: the system theme */ }
