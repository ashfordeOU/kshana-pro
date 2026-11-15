// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: the map maths of the Pro views (fleet, replay, area trust). Web Mercator,
// as the open shell's map (lib/map/proj.mjs), so a position drawn here sits where the shell's
// map would put it; the projection's name and a scale bar are always shown. Pure; tested.

export const PROJECTION = "Web Mercator";
const R = 6378137;
const MAX_LAT = 85.0511287798066;
const rad = Math.PI / 180;
const clamp = (x, a, b) => Math.min(b, Math.max(a, x));

/** Web Mercator metres for a longitude and latitude. */
export function merc(lon, lat) {
  const la = clamp(lat, -MAX_LAT, MAX_LAT) * rad;
  return [R * lon * rad, R * Math.log(Math.tan(Math.PI / 4 + la / 2))];
}

/** True for a finite latitude in [-90, 90] and longitude in [-180, 180]. */
export const validPos = (p) => !!p && Number.isFinite(p.lat) && Number.isFinite(p.lon) && Math.abs(p.lat) <= 90 && Math.abs(p.lon) <= 180;

/**
 * A view that fits `points` ({lat, lon}) into a w × h box with `pad` pixels on each side, at
 * least `minSpanM` metres across. Returns { project(lon, lat) -> [x, y], mpp (metres per pixel at
 * the centre latitude), scale: { px, label } } or null when no point is valid.
 */
export function fitView(points, w, h, { pad = 24, minSpanM = 2000 } = {}) {
  const ok = points.filter(validPos);
  if (!ok.length) return null;
  const xs = ok.map((p) => merc(p.lon, p.lat));
  let x0 = Math.min(...xs.map((p) => p[0])), x1 = Math.max(...xs.map((p) => p[0]));
  let y0 = Math.min(...xs.map((p) => p[1])), y1 = Math.max(...xs.map((p) => p[1]));
  const latC = ok.reduce((s, p) => s + p.lat, 0) / ok.length;
  const k = Math.cos(clamp(latC, -MAX_LAT, MAX_LAT) * rad); // Mercator metres per ground metre
  const minSpan = minSpanM / Math.max(k, 1e-6);
  if (x1 - x0 < minSpan) { const c = (x0 + x1) / 2; x0 = c - minSpan / 2; x1 = c + minSpan / 2; }
  if (y1 - y0 < minSpan) { const c = (y0 + y1) / 2; y0 = c - minSpan / 2; y1 = c + minSpan / 2; }
  const s = Math.min((w - 2 * pad) / (x1 - x0), (h - 2 * pad) / (y1 - y0));
  const cx = (x0 + x1) / 2, cy = (y0 + y1) / 2;
  const project = (lon, lat) => { const [mx, my] = merc(lon, lat); return [w / 2 + (mx - cx) * s, h / 2 - (my - cy) * s]; };
  const mpp = k / s; // ground metres per pixel at the centre latitude
  return { project, mpp, scale: scaleBar(mpp) };
}

/** A scale bar of a "nice" length (1, 2 or 5 × 10^n m) at most `maxPx` pixels long. */
export function scaleBar(mpp, maxPx = 120) {
  const max = mpp * maxPx;
  const p = 10 ** Math.floor(Math.log10(max));
  const m = [5, 2, 1].map((f) => f * p).find((v) => v <= max) || p;
  return { px: m / mpp, label: m >= 1000 ? `${m / 1000} km` : `${m} m` };
}

/** Great-circle distance in metres. */
export function haversine(lat1, lon1, lat2, lon2) {
  const a = Math.sin(((lat2 - lat1) * rad) / 2) ** 2 + Math.cos(lat1 * rad) * Math.cos(lat2 * rad) * Math.sin(((lon2 - lon1) * rad) / 2) ** 2;
  return 2 * R * Math.asin(Math.min(1, Math.sqrt(a)));
}

/** "59.4400° N, 24.7700° E". */
export function latLonText(lat, lon, d = 4) {
  return `${Math.abs(lat).toFixed(d)}° ${lat >= 0 ? "N" : "S"}, ${Math.abs(lon).toFixed(d)}° ${lon >= 0 ? "E" : "W"}`;
}

/** "2026-05-14 06:05:00 UTC" for Unix seconds. */
export function utcText(t) {
  if (!Number.isFinite(t)) return "";
  return new Date(Math.round(t) * 1000).toISOString().replace("T", " ").replace(/\.\d+Z$/, " UTC");
}
