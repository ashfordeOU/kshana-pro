// Pixel and colour maths for the display matrix: PNG decode, sampling, WCAG contrast,
// CIE Lab / CIEDE2000, and colour-vision-deficiency simulation matrices.
import { PNG } from 'pngjs';

export function decode(buf) {
  const p = PNG.sync.read(buf);
  return { w: p.width, h: p.height, d: p.data };
}
// Pixel at CSS coords (x, y) in an image captured at scale s (device pixels per CSS px).
export function px(img, x, y, s = 1) {
  const X = Math.min(img.w - 1, Math.max(0, Math.round(x * s))), Y = Math.min(img.h - 1, Math.max(0, Math.round(y * s)));
  const i = (Y * img.w + X) * 4;
  return [img.d[i], img.d[i + 1], img.d[i + 2]];
}
const lin = (v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
const unlin = (v) => { v = Math.min(1, Math.max(0, v)); return 255 * (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055); };
export const lum = ([r, g, b]) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
export const ratio = (a, b) => { const L1 = lum(a), L2 = lum(b); return (Math.max(L1, L2) + 0.05) / (Math.min(L1, L2) + 0.05); };
export const blend = (fg, bg) => [0, 1, 2].map((i) => fg[i] * (fg[3] ?? 1) + bg[i] * (1 - (fg[3] ?? 1)));
export const diff = (a, b) => Math.max(Math.abs(a[0] - b[0]), Math.abs(a[1] - b[1]), Math.abs(a[2] - b[2]));
export const hex = (c) => '#' + c.slice(0, 3).map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');

export function lab([r, g, b]) {
  const R = lin(r), G = lin(g), B = lin(b);
  let X = (R * 0.4124 + G * 0.3576 + B * 0.1805) / 0.95047;
  let Y = R * 0.2126 + G * 0.7152 + B * 0.0722;
  let Z = (R * 0.0193 + G * 0.1192 + B * 0.9505) / 1.08883;
  const f = (t) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  X = f(X); Y = f(Y); Z = f(Z);
  return [116 * Y - 16, 500 * (X - Y), 200 * (Y - Z)];
}
export function de2000(c1, c2) {
  const [L1, a1, b1] = lab(c1), [L2, a2, b2] = lab(c2);
  const rad = Math.PI / 180;
  const C1 = Math.hypot(a1, b1), C2 = Math.hypot(a2, b2), Cm = (C1 + C2) / 2;
  const G = 0.5 * (1 - Math.sqrt(Cm ** 7 / (Cm ** 7 + 25 ** 7)));
  const a1p = a1 * (1 + G), a2p = a2 * (1 + G);
  const C1p = Math.hypot(a1p, b1), C2p = Math.hypot(a2p, b2);
  const h = (a, b) => { if (a === 0 && b === 0) return 0; const x = Math.atan2(b, a) / rad; return x < 0 ? x + 360 : x; };
  const h1p = h(a1p, b1), h2p = h(a2p, b2);
  const dL = L2 - L1, dC = C2p - C1p;
  let dh = 0;
  if (C1p * C2p !== 0) { dh = h2p - h1p; if (dh > 180) dh -= 360; else if (dh < -180) dh += 360; }
  const dH = 2 * Math.sqrt(C1p * C2p) * Math.sin((dh / 2) * rad);
  const Lm = (L1 + L2) / 2, Cmp = (C1p + C2p) / 2;
  let hm = h1p + h2p;
  if (C1p * C2p !== 0) { if (Math.abs(h1p - h2p) > 180) hm += h1p + h2p < 360 ? 360 : -360; hm /= 2; }
  const T = 1 - 0.17 * Math.cos((hm - 30) * rad) + 0.24 * Math.cos(2 * hm * rad) + 0.32 * Math.cos((3 * hm + 6) * rad) - 0.2 * Math.cos((4 * hm - 63) * rad);
  const dTh = 30 * Math.exp(-(((hm - 275) / 25) ** 2));
  const RC = 2 * Math.sqrt(Cmp ** 7 / (Cmp ** 7 + 25 ** 7));
  const SL = 1 + (0.015 * (Lm - 50) ** 2) / Math.sqrt(20 + (Lm - 50) ** 2), SC = 1 + 0.045 * Cmp, SH = 1 + 0.015 * Cmp * T;
  const RT = -Math.sin(2 * dTh * rad) * RC;
  return Math.sqrt((dL / SL) ** 2 + (dC / SC) ** 2 + (dH / SH) ** 2 + RT * (dC / SC) * (dH / SH));
}
// Machado, Oliveira & Fernandes (2009), severity 1.0, applied in linear RGB. The same
// matrices go into the feColorMatrix filters the harness applies to the rendered page.
export const CVD = {
  protanopia: [0.152286, 1.052583, -0.204868, 0.114503, 0.786281, 0.099216, -0.003882, -0.048116, 1.051998],
  deuteranopia: [0.367322, 0.860646, -0.227968, 0.280085, 0.672501, 0.047413, -0.01182, 0.04294, 0.968881],
  tritanopia: [1.255528, -0.076749, -0.178779, -0.078411, 0.930809, 0.147602, 0.004733, 0.691367, 0.3039],
};
export function simulate(c, type) {
  const m = CVD[type];
  if (!m) return c.slice(0, 3);
  const [r, g, b] = [lin(c[0]), lin(c[1]), lin(c[2])];
  return [unlin(m[0] * r + m[1] * g + m[2] * b), unlin(m[3] * r + m[4] * g + m[5] * b), unlin(m[6] * r + m[7] * g + m[8] * b)];
}
export function filterSvg() {
  const f = Object.entries(CVD).map(([k, m]) => `<filter id="mx-${k}" color-interpolation-filters="linearRGB"><feColorMatrix type="matrix" values="${m.slice(0, 3).join(' ')} 0 0 ${m.slice(3, 6).join(' ')} 0 0 ${m.slice(6, 9).join(' ')} 0 0 0 0 0 1 0"/></filter>`).join('');
  return `<svg xmlns="http://www.w3.org/2000/svg" id="__mx_cvd" width="0" height="0" style="position:absolute" aria-hidden="true"><defs>${f}</defs></svg>`;
}
// Uniformity of a region: share of pixels within tol of the most common colour, and the
// highest contrast any sizeable colour cluster has against it.
export function regionStats(img, x, y, w, h, s = 1) {
  const X0 = Math.max(0, Math.floor(x * s)), Y0 = Math.max(0, Math.floor(y * s));
  const X1 = Math.min(img.w, Math.ceil((x + w) * s)), Y1 = Math.min(img.h, Math.ceil((y + h) * s));
  const buckets = new Map();
  let n = 0;
  const step = Math.max(1, Math.floor(Math.sqrt(((X1 - X0) * (Y1 - Y0)) / 40000)));
  for (let Y = Y0; Y < Y1; Y += step) for (let X = X0; X < X1; X += step) {
    const i = (Y * img.w + X) * 4;
    const k = ((img.d[i] >> 3) << 10) | ((img.d[i + 1] >> 3) << 5) | (img.d[i + 2] >> 3);
    buckets.set(k, (buckets.get(k) || 0) + 1);
    n++;
  }
  if (!n) return { n: 0, modalShare: 1, maxContrast: 1, distinct: 0 };
  const sorted = [...buckets.entries()].sort((a, b) => b[1] - a[1]);
  const col = (k) => [((k >> 10) & 31) * 8 + 4, ((k >> 5) & 31) * 8 + 4, (k & 31) * 8 + 4];
  const modal = col(sorted[0][0]);
  let near = 0;
  for (const [k, c] of sorted) if (diff(col(k), modal) <= 12) near += c;
  // Contrast reached by the top 0.5 % of pixels against the modal colour (a thin chart line
  // or a photo spread over many colours both count; single stray pixels do not).
  const rs = [];
  for (const [k, c] of sorted) { const r = ratio(col(k), modal); rs.push([r, c]); }
  rs.sort((a, b) => b[0] - a[0]);
  let acc = 0, maxContrast = 1;
  for (const [r, c] of rs) { acc += c; if (acc >= n * 0.005) { maxContrast = r; break; } }
  return { n, modalShare: near / n, maxContrast, distinct: sorted.filter(([, c]) => c / n >= 0.002).length, modal };
}
// Pixels of a CSS-px region as [r,g,b] triples (subsampled to at most ~4000).
export function regionPixels(img, x, y, w, h, s = 1) {
  const X0 = Math.max(0, Math.floor(x * s)), Y0 = Math.max(0, Math.floor(y * s));
  const X1 = Math.min(img.w, Math.ceil((x + w) * s)), Y1 = Math.min(img.h, Math.ceil((y + h) * s));
  const step = Math.max(1, Math.floor(Math.sqrt(((X1 - X0) * (Y1 - Y0)) / 4000)));
  const out = [];
  for (let Y = Y0; Y < Y1; Y += step) for (let X = X0; X < X1; X += step) { const i = (Y * img.w + X) * 4; out.push([img.d[i], img.d[i + 1], img.d[i + 2], X, Y]); }
  return out;
}
// Text contrast read purely from pixels (forced-colours mode, where CSS colours are not the
// colours used): the most common colour in the box is the ground, the glyph colour is the
// 97th-percentile contrast pixel against it.
export function glyphContrast(img, x, y, w, h) {
  const ps = regionPixels(img, x, y, w, h);
  if (ps.length < 6) return null;
  const buckets = new Map();
  for (const p of ps) { const k = ((p[0] >> 3) << 10) | ((p[1] >> 3) << 5) | (p[2] >> 3); const b = buckets.get(k) || { n: 0, s: [0, 0, 0] }; b.n++; b.s[0] += p[0]; b.s[1] += p[1]; b.s[2] += p[2]; buckets.set(k, b); }
  const top = [...buckets.values()].sort((a, b) => b.n - a.n)[0];
  const bg = top.s.map((v) => v / top.n);
  const rs = ps.map((p) => ({ r: ratio(p, bg), c: p })).sort((a, b) => a.r - b.r);
  // Third-strongest pixel: a small glyph (a middle dot, a comma) covers only a few pixels, so a
  // percentile would read the ground; one stray pixel is still not enough.
  const pick = rs[Math.max(0, rs.length - 3)];
  return { bg, fg: pick.c, r: pick.r };
}
// Non-text contrast of a small graphic from two captures (with and without it): changed
// pixels are the graphic; the 90th-percentile contrast between shown and hidden is its ratio.
export function regionMarkContrast(withImg, withoutImg, x, y, w, h) {
  const A = regionPixels(withImg, x, y, w, h), B = regionPixels(withoutImg, x, y, w, h);
  const rs = [];
  for (let i = 0; i < Math.min(A.length, B.length); i++) if (diff(A[i], B[i]) > 10) rs.push({ r: ratio(A[i], B[i]), c: A[i], bg: B[i] });
  if (rs.length < 3) return { changed: rs.length, r: 1, c: null, bg: B[0] || [0, 0, 0] };
  rs.sort((a, b) => a.r - b.r);
  const p = rs[Math.floor(rs.length * 0.9)];
  return { changed: rs.length, r: p.r, c: p.c, bg: p.bg };
}
// Mean absolute per-channel difference between two same-size images (subsampled).
export function meanDiff(a, b) {
  if (a.w !== b.w || a.h !== b.h) return 255;
  let s = 0, n = 0;
  for (let i = 0; i < a.d.length; i += 4 * 97) { s += Math.abs(a.d[i] - b.d[i]) + Math.abs(a.d[i + 1] - b.d[i + 1]) + Math.abs(a.d[i + 2] - b.d[i + 2]); n += 3; }
  return s / n;
}
