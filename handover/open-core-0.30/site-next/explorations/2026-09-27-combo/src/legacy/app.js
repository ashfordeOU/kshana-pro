window.addEventListener('DOMContentLoaded', function(){
'use strict';
var D = window.KDATA, KS = window.KS, RM = KS.reduced, root = document.documentElement;
var $ = function(s, r){ return (r || document).querySelector(s); };
var $$ = function(s, r){ return Array.prototype.slice.call((r || document).querySelectorAll(s)); };
var clamp = function(v, a, b){ return v < a ? a : v > b ? b : v; };
var ss = function(a, b, x){ x = clamp((x - a) / (b - a), 0, 1); return x * x * (3 - 2 * x); };
var lerp = function(a, b, t){ return a + (b - a) * t; };
function el(tag, cls, text){ var e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; }
/* markup from escaped or static strings only */
function setMarkup(node, markup){ node.replaceChildren(document.createRange().createContextualFragment(markup)); }
function esc(s){ return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
function cssv(name, node){ return getComputedStyle(node || root).getPropertyValue(name).trim(); }
function fmt(v, d){ if (v === 0) return '0'; var a = Math.abs(v); if (a >= 1000) return Math.round(v).toLocaleString('en-GB'); if (a >= 100) return v.toFixed(0); if (a >= 1) return v.toFixed(d == null ? 2 : d); if (a >= 0.001) return v.toPrecision(3); return v.toExponential(2); }
function sci(v){ var e = Math.floor(Math.log10(v)), m = v / Math.pow(10, e); return (Math.abs(m - 1) < 1e-9 ? '' : (+m.toFixed(2)) + '×') + '10' + String(e).replace(/-/, '⁻').replace(/\d/g, function(c){ return '⁰¹²³⁴⁵⁶⁷⁸⁹'[+c]; }); }
function scroll(sel){ var t = typeof sel === 'string' ? $(sel) : sel; if (t) t.scrollIntoView({ behavior: RM ? 'auto' : 'smooth' }); }
var S = D.sample, FILES = Object.keys(D.fileKind).sort();

/* ---------- theme ---------- */
var themeHooks = [];
function effTheme(){ var a = root.getAttribute('data-theme'); return a || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'); }
function toggleTheme(){ var n = effTheme() === 'dark' ? 'light' : 'dark'; root.setAttribute('data-theme', n); try { localStorage.setItem('kshana-theme', n); } catch (e) {} themeHooks.forEach(function(f){ f(); }); }
$('#themeBtn').addEventListener('click', toggleTheme);
try { matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function(){ themeHooks.forEach(function(f){ f(); }); }); } catch (e) {}

/* ---------- toast, copy, download ---------- */
var toastEl = $('#toast'), toastT;
function toast(m){ toastEl.textContent = m; toastEl.classList.add('on'); clearTimeout(toastT); toastT = setTimeout(function(){ toastEl.classList.remove('on'); }, 1900); }
function copyText(t, btn, msg){
  var ok = function(){ if (btn){ btn.classList.add('copied'); setTimeout(function(){ btn.classList.remove('copied'); }, 1400); } toast(msg || 'Copied'); };
  var fb = function(){ var ta = document.createElement('textarea'); ta.value = t; ta.style.position = 'fixed'; ta.style.opacity = '0'; document.body.appendChild(ta); ta.select(); try { document.execCommand('copy'); ok(); } catch (e) { toast('Copy is blocked here: select the text instead'); } ta.remove(); };
  if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(t).then(ok, fb); else fb();
}
function bindCopy(scope){ $$('[data-copy]', scope).forEach(function(b){ if (b._c) return; b._c = 1; b.addEventListener('click', function(){ copyText(b.getAttribute('data-copy'), b, 'Copied: ' + b.getAttribute('data-copy')); }); }); }
bindCopy(document);
$$('[data-copy-from]').forEach(function(b){ b.addEventListener('click', function(){ copyText($('#' + b.getAttribute('data-copy-from')).textContent, b); }); });
$('#bibCopy').addEventListener('click', function(){ copyText($('#bibText').textContent, this, 'BibTeX copied'); });
function download(name, blob){ var a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = name; document.body.appendChild(a); a.click(); setTimeout(function(){ URL.revokeObjectURL(a.href); a.remove(); }, 500); toast('Downloading ' + name); }

/* ---------- canvas + loop helpers ---------- */
function fit(cv){ var r = cv.getBoundingClientRect(), d = Math.min(2, window.devicePixelRatio || 1); if (!r.width || !r.height) return null; var w = Math.round(r.width * d), h = Math.round(r.height * d); if (cv.width !== w || cv.height !== h){ cv.width = w; cv.height = h; } var c = cv.getContext('2d'); c.setTransform(d, 0, 0, d, 0, 0); c.clearRect(0, 0, r.width, r.height); return { c: c, w: r.width, h: r.height }; }
var loops = [];
function addLoop(target, fn){ var o = { fn: fn, vis: false }; loops.push(o); if ('IntersectionObserver' in window) new IntersectionObserver(function(es){ o.vis = es[0].isIntersecting; }, { rootMargin: '160px' }).observe(target); else o.vis = true; return o; }
/* lazily initialise a heavy module the first time it nears the viewport */
var lazyQ = [];
function lazy(target, init){ var job = { t: target, done: false, go: function(){ if (!job.done){ job.done = true; init(); } } }; lazyQ.push(job); if (!('IntersectionObserver' in window)) return job.go(); new IntersectionObserver(function(es, o){ if (es[0].isIntersecting){ o.disconnect(); job.go(); } }, { rootMargin: '600px' }).observe(target); }
/* backstop for jumps across pinned sections, where an observer can miss the crossing */
var lazyTick = false; addEventListener('scroll', function(){ if (lazyTick) return; lazyTick = true; requestAnimationFrame(function(){ lazyTick = false; lazyQ.forEach(function(j){ if (j.done) return; var r = j.t.getBoundingClientRect(); if (r.top < innerHeight + 600 && r.bottom > -600) j.go(); }); }); }, { passive: true });
var lastT = performance.now();
function frame(now){ var t = now / 1000, dt = Math.min(0.05, (now - lastT) / 1000); lastT = now; for (var i = 0; i < loops.length; i++){ if (loops[i].vis) loops[i].fn(t, dt); } requestAnimationFrame(frame); }
function lab(c, s, x, y, fg, al){ c.globalAlpha = .95; c.fillStyle = fg; c.font = '500 11px "Geist Mono", monospace'; c.textAlign = al || 'left'; c.textBaseline = 'middle'; c.fillText(s, x, y); c.globalAlpha = 1; }

/* ---------- nav tone ---------- */
var nav = $('#nav'), darkSecs = $$('#story,.stats,.tl,.journey,.eds-sec');
function navTone(){ var y = 28, dark = false; darkSecs.forEach(function(s){ var r = s.getBoundingClientRect(); if (r.top <= y && r.bottom > y) dark = true; }); nav.classList.toggle('on-page', !dark); }
addEventListener('scroll', navTone, { passive: true }); navTone();

var useST = !RM && window.gsap && window.ScrollTrigger;
if (useST) gsap.registerPlugin(ScrollTrigger);

/* =========================================================== HERO STORY */
(function(){
  var story = $('#story'), stage = $('#stage'), heroCopy = $('#heroCopy'), beats = $$('.beat'), rails = $$('#rail i'), tele = $('#telemetry');
  function readStory(){ var r = story.getBoundingClientRect(), h = r.height - innerHeight; KS.target = h > 0 ? clamp(-r.top / h, 0, 1) * 5 : 0; }
  if (useST) ScrollTrigger.create({ trigger: story, start: 'top top', end: 'bottom bottom', onUpdate: function(s){ KS.target = s.progress * 5; } });
  else addEventListener('scroll', readStory, { passive: true });
  readStory();
  function tCn(p){ if (p < 2) return 45; if (p < 3) return 45 - 23 * ss(2, 2.6, p); if (p < 4) return lerp(22, 47, ss(3, 3.3, p)); return NaN; }
  function tTe(p){ if (p < 4) return 1.6 + 1.2 * ss(2.3, 3, p) + 6 * ss(3.2, 4, p); var h = ss(4, 4.95, p); return 8.8 + h * h * 110; }
  function tPe(p){ if (p < 2) return 2; if (p < 3) return 2 + 30 * ss(2.2, 2.9, p); if (p < 4) return 32 + 560 * Math.pow(ss(3.1, 3.95, p), 1.6); return 592 + 400 * ss(4, 5, p); }
  var tmCn = $('#tmCn'), tmTe = $('#tmTe'), tmPe = $('#tmPe'), tmState = $('#tmState');
  function spark(cv, f, p, lo, hi, col, log){ var g = fit(cv); if (!g) return; var c = g.c, W = g.w, H = g.h, n = 60;
    var map = function(v){ var x = log ? (Math.log10(Math.max(v, 1e-3)) - Math.log10(lo)) / (Math.log10(hi) - Math.log10(lo)) : (v - lo) / (hi - lo); return H - 3 - clamp(x, 0, 1) * (H - 6); };
    c.beginPath(); var st = false; for (var i = 0; i <= n; i++){ var v = f(Math.max(p - 1.2 + 1.2 * i / n, 1)); if (isNaN(v)){ st = false; continue; } var X = W * i / n, Y = map(v); st ? c.lineTo(X, Y) : c.moveTo(X, Y); st = true; }
    c.strokeStyle = col; c.lineWidth = 1.8; c.lineJoin = 'round'; c.stroke(); }
  function setState(txt, s){ if (tmState.dataset.s !== s || tmState.lastChild.textContent !== txt){ tmState.dataset.s = s; tmState.lastChild.textContent = txt; } }
  var lastBeat = -2;
  addLoop(story, function(t, dt){
    if (RM) KS.p = 0; else { KS.p += (KS.target - KS.p) * (1 - Math.exp(-dt * 5.5)); if (Math.abs(KS.target - KS.p) < 1e-4) KS.p = KS.target; }
    var p = KS.p, out = RM ? 0 : ss(0.04, 0.75, p);
    heroCopy.style.opacity = (1 - out).toFixed(3); heroCopy.style.visibility = out > 0.995 ? 'hidden' : 'visible'; heroCopy.style.translate = '0 ' + (-out * 60).toFixed(1) + 'px';
    stage.dataset.phase = p > 0.6 ? 'story' : 'hero';
    var b = p < 0.95 ? -1 : Math.min(3, Math.floor(p - 1 + 0.05));
    if (b !== lastBeat){ beats.forEach(function(e, i){ e.classList.toggle('on', i === b); e.classList.toggle('past', i < b); }); lastBeat = b; }
    rails.forEach(function(e, i){ e.style.transform = 'scaleX(' + clamp(p - 1 - i, 0, 1).toFixed(3) + ')'; });
    tele.classList.toggle('on', p > 1.05);
    if (p > 1.05){ var cn = tCn(p), te = tTe(p), pe = tPe(p);
      tmCn.firstChild.textContent = isNaN(cn) ? '—' : cn.toFixed(1); tmTe.firstChild.textContent = te < 10 ? te.toFixed(1) : Math.round(te); tmPe.firstChild.textContent = Math.round(pe);
      if (p < 2) setState('Tracking', 'ok'); else if (p < 3) setState(cn < 28 ? 'Lost lock' : 'Degraded', cn < 28 ? 'bad' : 'warn'); else if (p < 4) setState(pe > 100 ? 'Spoof flagged' : 'Checking', 'spf'); else setState(te > 100 ? 'Outside guard' : 'Holdover', te > 100 ? 'bad' : 'warn');
      spark($('#tmCnC'), tCn, p, 15, 50, '#2CD0DE'); spark($('#tmTeC'), tTe, p, 1, 150, '#FFB224', true); spark($('#tmPeC'), tPe, p, 1, 1000, '#A58BFF', true); }
  });
})();

/* ---------- reveal ---------- */
(function(){ var rvs = $$('.rv'); function reveal(e){ e.classList.add('shown', 'in'); }
  if (RM || !('IntersectionObserver' in window)) return rvs.forEach(reveal);
  var io = new IntersectionObserver(function(es){ es.forEach(function(e){ if (e.isIntersecting){ var n = e.target, idx = n.parentElement ? Array.prototype.indexOf.call(n.parentElement.children, n) : 0; n.style.transitionDelay = Math.min(idx, 6) * 60 + 'ms'; reveal(n); setTimeout(function(){ n.style.transitionDelay = ''; }, 1500); io.unobserve(n); } }); }, { rootMargin: '0px 0px -8% 0px' });
  rvs.forEach(function(e){ io.observe(e); }); })();

/* =========================================================== WATERFALL (Spectrum) */
lazy($('#jam'), function(){
  var cv = $('#wf'), host = $('#scopeMain'), COLS = 420, ROWS = 240, RPS = 50;
  var buf = document.createElement('canvas'); buf.width = COLS; buf.height = ROWS;
  var bctx = buf.getContext('2d'), img = bctx.createImageData(COLS, ROWS), d = img.data;
  var stops = [[0,[5,6,15]],[.2,[13,23,69]],[.36,[26,63,184]],[.5,[0,166,194]],[.63,[57,211,138]],[.75,[255,194,51]],[.87,[255,85,59]],[1,[255,244,238]]], lut = new Uint8Array(768);
  for (var i = 0; i < 256; i++){ var v = i / 255, k = 0; while (k < stops.length - 2 && v > stops[k + 1][0]) k++; var a = stops[k], b = stops[k + 1], f = clamp((v - a[0]) / (b[0] - a[0]), 0, 1); for (var ch = 0; ch < 3; ch++) lut[i * 3 + ch] = Math.round(a[1][ch] + (b[1][ch] - a[1][ch]) * f); }
  /* the three GNSS carriers the engine models (axis compressed, not to scale) */
  var bands = [{ id: 'L5·E5a', x: .16, w: .05, f: '1176.45 MHz', nom: 47.5 }, { id: 'L2', x: .36, w: .042, f: '1227.60 MHz', nom: 44.0 }, { id: 'L1·E1', x: .8, w: .034, f: '1575.42 MHz', nom: 46.5 }];
  var base = new Float32Array(COLS);
  for (i = 0; i < COLS; i++){ var x = i / (COLS - 1), vv = .2; bands.forEach(function(bd){ var u = (x - bd.x) / bd.w; vv += .15 * Math.exp(-u * u) + .06 * Math.exp(-(u * u) * 28); var s = u * 3.2; vv += s !== 0 ? .025 * Math.pow(Math.sin(s) / s, 2) : .025; }); base[i] = vv; }
  bands.forEach(function(bd){ var l = el('div', 'bline'); l.style.left = 'calc(var(--ax,52px) + (100% - var(--ax,52px)) * ' + bd.x + ')'; l.appendChild(el('b', null, bd.id)); $('#blines').appendChild(l); var s = el('span', null, bd.f); s.style.left = (bd.x * 100) + '%'; $('#faxis').appendChild(s); });
  var J = { x: .8, tx: .8, p: 0, w: .02, hover: false, lastInput: -100, touchUntil: 0 }, ns = new Float32Array(COLS), dme = 0;
  function genRow(t){ var row = new Float32Array(COLS); if (Math.random() < .18) dme = .015 + Math.random() * .07; else if (Math.random() < .5) dme = 0; var tone = .6 + .012 * Math.sin(t * .35);
    for (var i = 0; i < COLS; i++){ var x = i / (COLS - 1), n = (Math.random() + Math.random() + Math.random()) / 3; ns[i] = ns[i] * .5 + n * .5; var v = base[i] + (ns[i] - .5) * .2; if (dme && Math.abs(x - dme) < .004) v += .32; var u = (x - tone) / .0022; v += .2 * Math.exp(-u * u); if (J.p > .002){ var dx = x - J.x, q = dx / J.w; v += J.p * (.46 * Math.exp(-q * q) + .14 * Math.exp(-Math.abs(dx) / (J.w * 3.5)) + .035) * (.9 + .2 * Math.random()); } row[i] = v; } return row; }
  function push(row){ d.copyWithin(COLS * 4, 0, (ROWS - 1) * COLS * 4); for (var i = 0; i < COLS; i++){ var k = clamp((row[i] * 255) | 0, 0, 255) * 3, o = i * 4; d[o] = lut[k]; d[o + 1] = lut[k + 1]; d[o + 2] = lut[k + 2]; d[o + 3] = 255; } }
  var bandsEl = $('#bands'); bands.slice().reverse().forEach(function(bd){ var r = el('div', 'brow'), bar = el('span', 'bar'), fill = el('i'); bar.appendChild(fill); var em = el('em', null, '--'); r.append(el('span', null, bd.id.split('·')[0]), bar, em); bandsEl.appendChild(r); bd.fill = fill; bd.em = em; });
  var cn0El = $('#cn0'), stEl = $('#status'), jsEl = $('#jsv'), jcur = $('#jcur'), hint = $('#hint'), sparkCv = $('#spark'), hist = new Float32Array(160).fill(46.5), state = 'ok', txtT = 0, jit = {};
  function gain(bd){ var dx = bd.x - J.x, q = dx / (J.w * 1.8); return J.p * (.9 * Math.exp(-q * q) + .1 * Math.exp(-Math.abs(dx) / .3)); }
  function cn0Of(bd){ jit[bd.id] = (jit[bd.id] || 0) * .9 + (Math.random() - .5) * .12; return bd.nom - 34 * gain(bd) + jit[bd.id] * 2; }
  function setStatus(s){ if (s === state) return; state = s; stEl.dataset.s = s === 'ok' ? 'ok' : s === 'deg' ? 'warn' : 'bad'; stEl.lastChild.textContent = s === 'ok' ? 'Tracking' : s === 'deg' ? 'Degraded' : 'Lost lock'; }
  function updJ(t, dt){ var drive = 0; if (J.hover || t < J.touchUntil) drive = 1; else if (t - J.lastInput > 3){ var ph = (t + 2) % 8.5; if (ph > .6 && ph < 4.8){ drive = 1; J.tx = .7 + .14 * (.5 - .5 * Math.cos(Math.PI * (ph - .6) / 4.2)); } }
    if (drive > J.p) J.p += (drive - J.p) * (1 - Math.exp(-dt * 3.2)); else J.p *= Math.exp(-dt / .9);
    if (drive > .5) J.w = Math.min(.075, J.w + dt * .02); else J.w += (.018 - J.w) * (1 - Math.exp(-dt * .7));
    J.x += (J.tx - J.x) * (1 - Math.exp(-dt * (J.hover ? 14 : 3))); }
  function readouts(t, force){ var L1 = bands[2], v = cn0Of(L1); hist.copyWithin(0, 1); hist[hist.length - 1] = v;
    if (v < 28.5) setStatus('lost'); else if (v < 36) setStatus(state === 'lost' && v < 30 ? 'lost' : 'deg'); else if (v > 37) setStatus('ok');
    if (force || t - txtT > .1){ txtT = t; cn0El.textContent = v.toFixed(1); bands.forEach(function(bd){ var c = bd === L1 ? v : cn0Of(bd), f = clamp((c - 15) / 35, .02, 1); bd.fill.style.transform = 'scaleX(' + f + ')'; bd.fill.style.background = c < 28.5 ? '#FF5B54' : c < 36 ? '#FFB224' : '#39D38A'; bd.em.textContent = c.toFixed(1); }); var g = gain(L1); jsEl.textContent = g > .03 ? '+' + (20 + 40 * g).toFixed(0) + ' dB' : 'none'; }
    var G = fit(sparkCv); if (!G) return; var c = G.c, w = G.w, h = G.h, y = function(q){ return h - 4 - clamp((q - 18) / 32, 0, 1) * (h - 8); };
    [[36, 'rgba(255,178,36,.5)'], [28, 'rgba(255,91,84,.55)']].forEach(function(z){ c.strokeStyle = z[1]; c.lineWidth = 1; c.setLineDash([3, 4]); c.beginPath(); c.moveTo(0, y(z[0])); c.lineTo(w, y(z[0])); c.stroke(); }); c.setLineDash([]);
    c.beginPath(); for (var i = 0; i < hist.length; i++){ var X = i / (hist.length - 1) * w, Y = y(hist[i]); i ? c.lineTo(X, Y) : c.moveTo(X, Y); } c.strokeStyle = '#fff'; c.lineWidth = 1.6; c.stroke(); c.lineTo(w, h); c.lineTo(0, h); c.closePath(); c.fillStyle = 'rgba(255,255,255,.06)'; c.fill(); }
  function paint(){ var G = fit(cv); if (!G) return; bctx.putImageData(img, 0, 0); G.c.drawImage(buf, 0, 0, G.w, G.h); jcur.style.opacity = clamp(J.p * 1.4, 0, 1); jcur.style.transform = 'translateX(' + (J.x * G.w) + 'px)'; }
  (function(){ var t = -ROWS / RPS, dt = 1 / RPS; for (var r = 0; r < ROWS; r++){ t += dt; updJ(t + 2.5, dt); push(genRow(t)); } })();
  if (RM){ J.x = .8; J.tx = .8; J.p = .9; J.w = .07; for (var r = 0; r < 90; r++) push(genRow(r / RPS)); }
  function setX(e){ var r = cv.getBoundingClientRect(); J.tx = clamp((e.clientX - r.left) / r.width, 0, 1); J.lastInput = performance.now() / 1000; hint.classList.add('off'); }
  cv.addEventListener('pointermove', function(e){ if (e.pointerType === 'touch') return; J.hover = true; setX(e); });
  cv.addEventListener('pointerleave', function(){ J.hover = false; });
  cv.addEventListener('pointerdown', function(e){ setX(e); if (e.pointerType === 'touch'){ J.touchUntil = performance.now() / 1000 + 1.6; J.x = J.tx; } });
  host.addEventListener('keydown', function(e){ if (e.key === 'ArrowLeft' || e.key === 'ArrowRight'){ e.preventDefault(); J.tx = clamp(J.tx + (e.key === 'ArrowLeft' ? -.03 : .03), 0, 1); var n = performance.now() / 1000; J.lastInput = n; J.touchUntil = n + 1.2; hint.classList.add('off'); } });
  var setAx = function(){ host.style.setProperty('--ax', innerWidth <= 560 ? '40px' : '52px'); }; setAx(); addEventListener('resize', setAx);
  var acc = 0, painted = false;
  addLoop(host, function(t, dt){ if (RM){ if (!painted){ paint(); readouts(t, true); painted = true; } return; } updJ(t, dt); acc += dt * RPS; var n = 0; while (acc >= 1 && n < 4){ push(genRow(t)); acc -= 1; n++; } paint(); readouts(t); });
});

/* =========================================================== BAND ATLAS (Spectrum colour + Observatory readout) */
lazy($('#bands-atlas'), function(){
  /* every band below was checked in the engine source; hatched ones are not modelled */
  var B = [
    { id: 'L5 · E5a', f: [1176.45], con: 'GPS · Galileo', c: '--s-int', m: true, src: 'sbas.rs (L1/L5 ionosphere-free), conflict_threat_params.rs', use: 'Dual-frequency L1/L5 ionosphere-free SBAS protection levels (DO-229E), and the wideband L5/E5a layer in layered-PNT conflict studies.', jam: 'Wideband, so more jam-resistant than L1 C/A, but still denied by an RF jammer on the same band.', files: ['conflict-resilience'] },
    { id: 'L2', f: [1227.6], con: 'GPS', c: '--s-orb', m: true, src: 'pvt.rs, ionex.rs', use: 'Dual-frequency L1/L2 ionosphere-free positioning on real IGS (International GNSS Service) station data, and IONEX delay scaling as 1/f².', jam: 'Not a jamming target in the bundled scenarios; used for ionosphere removal.', files: ['pvt-abmf'] },
    { id: 'L1 · E1', f: [1575.42], con: 'GPS · Galileo', c: '--s-tim', m: true, src: 'jamming.rs, tracking_loop.rs, gnss_sim.rs, ephemeris.rs, navsignal.rs', use: 'The C/A code carrier for jamming, tracking loops, measurement simulation, Doppler and positioning; Galileo E1 open service as a BOC(1,1) modulation in the anti-jam model.', jam: 'A jammer sets the jammer-to-signal ratio; processing gain converts it to an effective C/N₀ per satellite, and a satellite loses lock below ' + D.jamThr + ' dB-Hz.', files: ['jamming-demo', 'tracking-loop', 'small-uas-jammed-nav', 'gnss-sim-raim', 'spoof-detect', 'ephemeris', 'pvt-abmf', 'earth-gnss-lunar'] },
    { id: 'G1 · G2', f: [1602, 1246], con: 'GLONASS', c: '--s-nav', m: false, use: 'GLONASS broadcast ephemerides are parsed from RINEX, but its FDMA signals are not modelled.', jam: 'Not modelled.', files: [] },
    { id: 'B1 · B2', f: [1561.098, 1207.14], con: 'BeiDou', c: '--s-nav', m: false, use: 'BeiDou ephemerides are parsed from RINEX; its signals are not modelled.', jam: 'Not modelled.', files: [] },
    { id: 'E6', f: [1278.75], con: 'Galileo', c: '--s-spf', m: false, use: 'Galileo E6 is not modelled.', jam: 'Not modelled.', files: [] },
    { id: 'S-band 2.4 GHz', f: [2400], con: 'Lunar AFS', c: '--s-spf', m: true, src: 'lunar_service.rs, lunar_jamming', use: 'The lunar augmented forward signal carrier for lunar service volumes and lunar-native jamming.', jam: 'Per-satellite J/S for a lunar surface user, and the lunar signal-security attack surface.', files: ['lunar-jamming', 'lunar-attack-surface', 'moonlight-service-volume'] },
    { id: 'S · X · Ka', f: [2300, 8420, 32000], con: 'Deep Space Network', c: '--s-itg', m: true, src: 'linkbudget.rs, radiometric.rs', use: 'Deep-space link budgets in the DSN allocations: S near 2.3 GHz, X near 8.4 GHz and Ka near 32 GHz.', jam: 'Link margin, not jamming, is the question here.', files: ['link-budget'] },
    { id: 'Optical 1550 nm', f: null, con: 'Optical link', c: '--s-orb', m: true, src: 'optical_linkbudget.rs', use: 'The 1550 nm telecom C-band optical link for optical PNT and inter-satellite time transfer. This is optical, not RF C-band.', jam: 'Weather and pointing, not RF jamming, limit it.', files: ['hybrid-optical-rf', 'timetransfer'] }
  ];
  var zoom = 'l', sel = 2, cv = $('#baC'), chips = $('#baChips'), det = $('#baDetail');
  B.forEach(function(b, i){ var bt = el('button', b.m ? '' : 'nm'); bt.type = 'button'; bt.setAttribute('role', 'tab'); bt.style.setProperty('--c', 'var(' + b.c + ')'); bt.append(el('i'), document.createTextNode(b.id + (b.m ? '' : ' · not modelled'))); bt.addEventListener('click', function(){ pick(i); }); chips.appendChild(bt); });
  function xOf(f, W){ if (zoom === 'l'){ return 20 + (f - 1150) / (1640 - 1150) * (W - 40); } return 20 + (Math.log10(f) - Math.log10(1000)) / (Math.log10(40000) - Math.log10(1000)) * (W - 40); }
  var hits = [];
  function draw(){ var G = fit(cv); if (!G) return; var c = G.c, W = G.w, H = G.h, ink3 = cssv('--ink-3'), line2 = cssv('--line-2'); hits = [];
    c.strokeStyle = line2; c.beginPath(); c.moveTo(20, H - 34); c.lineTo(W - 20, H - 34); c.stroke(); c.font = '10px "Geist Mono", monospace'; c.fillStyle = ink3; c.textAlign = 'center';
    (zoom === 'l' ? [1150, 1250, 1350, 1450, 1550, 1640] : [1000, 2000, 5000, 10000, 20000, 40000]).forEach(function(f){ var x = xOf(f, W); c.beginPath(); c.moveTo(x, H - 38); c.lineTo(x, H - 30); c.stroke(); c.fillText(f >= 1000 && zoom !== 'l' ? (f / 1000) + ' GHz' : f + ' MHz', x, H - 16); });
    B.forEach(function(b, i){ if (!b.f) return; b.f.forEach(function(f){ if (zoom === 'l' && (f < 1150 || f > 1640)) return; if (zoom !== 'l' && f < 1000) return; var x = xOf(f, W), col = cssv(b.c), on = i === sel, hgt = b.m ? (on ? 86 : 64) : 44, top = H - 34 - hgt;
      if (b.m){ var gr = c.createLinearGradient(0, top, 0, H - 34); gr.addColorStop(0, col); gr.addColorStop(1, 'transparent'); c.fillStyle = gr; c.globalAlpha = on ? .9 : .55; c.beginPath(); c.moveTo(x - 16, H - 34); c.quadraticCurveTo(x, top - 30, x + 16, H - 34); c.fill(); c.globalAlpha = 1; c.strokeStyle = col; c.lineWidth = on ? 2.4 : 1.4; c.beginPath(); c.moveTo(x, H - 34); c.lineTo(x, top); c.stroke(); }
      else { c.strokeStyle = col; c.globalAlpha = .6; c.setLineDash([3, 3]); c.lineWidth = 1.2; c.strokeRect(x - 9, top, 18, hgt); c.setLineDash([]); c.globalAlpha = 1; }
      c.fillStyle = on ? cssv('--ink') : ink3; c.textAlign = 'center'; c.font = (on ? '600 ' : '') + '11px "Geist Mono", monospace'; c.fillText(b.id.split(' · ')[b.f.indexOf(f)] || b.id, x, top - 8); hits.push([x, i]); }); });
    if (zoom === 'l'){ c.fillStyle = ink3; c.textAlign = 'right'; c.font = '10px "Geist Mono", monospace'; c.fillText('S-band, X, Ka and optical links: switch to 1–40 GHz', W - 20, 14); } }
  cv.addEventListener('click', function(e){ var r = cv.getBoundingClientRect(), x = e.clientX - r.left, best = null, bd = 30; hits.forEach(function(h){ var dd = Math.abs(h[0] - x); if (dd < bd){ bd = dd; best = h[1]; } }); if (best != null) pick(best); });
  function pick(i){ sel = i; var b = B[i]; $$('button', chips).forEach(function(x, j){ x.setAttribute('aria-selected', j === i); });
    if (b.f && zoom === 'l' && b.f[0] > 1640){ zoom = 'all'; $$('#baZoom button').forEach(function(x){ x.setAttribute('aria-pressed', x.dataset.z === 'all'); }); }
    var fq = b.f ? b.f.map(function(f){ return f >= 2000 ? (f / 1000) + ' GHz' : f + ' MHz'; }).join(' · ') : '1550 nm (about 193 THz)';
    var left = el('div'); left.append(el('h4', null, b.id + ' · ' + b.con), el('div', 'fq', fq + (b.m ? ' · modelled' : ' · not modelled')), el('p', null, b.use)); if (b.src){ var sp = el('p', null, 'Engine source: ' + b.src); sp.style.cssText = 'font:500 11.5px/1.4 var(--f-mono);color:var(--ink-3)'; left.appendChild(sp); }
    var right = el('div'); right.append(el('span', 'lbl', 'What interference does'), el('p', null, b.jam)); var tg = el('div', 'tags'); tg.style.marginTop = '12px'; if (b.files.length){ b.files.forEach(function(f){ var cd = el('code', null, f); cd.style.cursor = 'pointer'; cd.title = 'Load in the playground'; cd.addEventListener('click', function(){ window.KSload(f); scroll('#playground'); }); tg.appendChild(cd); }); } else tg.appendChild(el('code', null, 'no bundled scenario')); right.appendChild(tg);
    det.replaceChildren(left, right); draw(); }
  $$('#baZoom button').forEach(function(b){ b.addEventListener('click', function(){ zoom = b.dataset.z; $$('#baZoom button').forEach(function(x){ x.setAttribute('aria-pressed', x === b); }); draw(); }); });
  themeHooks.push(draw); addEventListener('resize', draw); pick(2);
});

/* =========================================================== SIX CARDS (Spectrum) */
function bz(a, b, cc, dd, u){ var v = 1 - u; return v * v * v * a + 3 * v * v * u * b + 3 * v * u * u * cc + u * u * u * dd; }
function rr(c, x, y, w, h, r){ c.beginPath(); if (c.roundRect) c.roundRect(x, y, w, h, r); else c.rect(x, y, w, h); }
var DIAG = {
  jam: function(c, w, h, t, fg){ var cx = w * .52, cy = h * .56, R = Math.min(w, h) * (.3 + .035 * Math.sin(t * 1.3)), sp = Math.max(18, Math.min(w, h) / 11), x, y;
    for (y = sp * .6; y < h; y += sp) for (x = sp * .6; x < w; x += sp){ var dd = Math.hypot(x - cx, y - cy); c.beginPath(); if (dd < R){ c.globalAlpha = .85; c.arc(x, y, 2.8, 0, 6.283); c.lineWidth = 1.2; c.strokeStyle = fg; c.stroke(); } else { c.globalAlpha = .45; c.arc(x, y, 1.8, 0, 6.283); c.fillStyle = fg; c.fill(); } }
    for (var k = 0; k < 3; k++){ var ph = ((t * .42) + k / 3) % 1; c.globalAlpha = (1 - ph) * .6; c.beginPath(); c.arc(cx, cy, R * 1.55 * ph, 0, 6.283); c.lineWidth = 1.6; c.strokeStyle = fg; c.stroke(); }
    c.globalAlpha = 1; c.setLineDash([5, 5]); c.beginPath(); c.arc(cx, cy, R, 0, 6.283); c.lineWidth = 1.6; c.strokeStyle = fg; c.stroke(); c.setLineDash([]); c.fillStyle = fg; c.beginPath(); c.arc(cx, cy, 7, 0, 6.283); c.fill();
    var P = [[0, h * .18], [w * .38, h * .2], [w * .5, h * .9], [w, h * .86]]; c.globalAlpha = .3; c.lineWidth = 7; c.lineCap = 'round'; c.beginPath(); c.moveTo(P[0][0], P[0][1]); c.bezierCurveTo(P[1][0], P[1][1], P[2][0], P[2][1], P[3][0], P[3][1]); c.stroke(); c.lineWidth = 1;
    var u = (t * .075) % 1, vx = bz(P[0][0], P[1][0], P[2][0], P[3][0], u), vy = bz(P[0][1], P[1][1], P[2][1], P[3][1], u), inside = Math.hypot(vx - cx, vy - cy) < R;
    c.globalAlpha = 1; c.beginPath(); c.arc(vx, vy, 6, 0, 6.283); if (inside){ c.lineWidth = 2.4; c.stroke(); } else c.fill();
    lab(c, inside ? 'lost lock' : 'tracking', vx + 12, vy - 12, fg); lab(c, 'denial radius', cx + R * .72 + 6, cy - R * .72 - 6, fg); },
  spoof: function(c, w, h, t, fg){ var L = 18, Rr = w - 18, s0 = .36, Sd = Rr - L, yT = function(s){ return h * .66 + Math.sin(s * 5.4) * h * .05; }, yS = function(s){ return yT(s) - (s > s0 ? Math.pow((s - s0) / (1 - s0), 1.7) * h * .44 : 0); }, p = Math.min(1, (t * .13) % 1.3);
    var path = function(f, dash, a, lw){ c.globalAlpha = a; c.setLineDash(dash); c.lineWidth = lw; c.strokeStyle = fg; c.beginPath(); for (var i = 0; i <= 80; i++){ var s = i / 80 * p, X = L + s * Sd, Y = f(s); i ? c.lineTo(X, Y) : c.moveTo(X, Y); } c.stroke(); c.setLineDash([]); };
    for (var k = 0; k < 4; k++){ var X = L + 30 + k * (Sd / 4.4), Y = h * .26 + (k % 2) * 12; c.globalAlpha = .55; c.fillStyle = fg; c.fillRect(X - 5, Y - 2, 10, 4); c.fillRect(X - 1.5, Y - 5, 3, 10); }
    path(yT, [], .95, 2.4); path(yS, [6, 6], .9, 2.4); c.globalAlpha = 1; c.fillStyle = fg; c.beginPath(); c.arc(L + p * Sd, yT(p), 5, 0, 6.283); c.fill(); c.beginPath(); c.arc(L + p * Sd, yS(p), 6, 0, 6.283); c.lineWidth = 2; c.strokeStyle = fg; c.stroke();
    var sd = s0 + (1 - s0) * Math.pow(.1 / .44, 1 / 1.7); if (p > sd){ var Xs = L + sd * Sd; c.globalAlpha = .8; c.setLineDash([2, 4]); c.lineWidth = 1; c.beginPath(); c.moveTo(Xs, h * .14); c.lineTo(Xs, h * .92); c.stroke(); c.setLineDash([]); c.globalAlpha = .2; c.fillStyle = fg; rr(c, Xs - 6, h * .08, 114, 24, 6); c.fill(); lab(c, 'spoof flagged', Xs + 4, h * .08 + 12, fg); }
    if (p > .7){ lab(c, 'true track', L + p * Sd - 4, yT(p) + 20, fg, 'right'); lab(c, 'spoofed fix', L + p * Sd - 12, yS(p) - 2, fg, 'right'); } },
  clock: function(c, w, h, t, fg){ var L = 34, Rr = w - 64, T = 26, B = h - 30; c.globalAlpha = .35; c.strokeStyle = fg; c.lineWidth = 1; c.beginPath(); c.moveTo(L, T); c.lineTo(L, B); c.lineTo(Rr + 40, B); c.stroke();
    var G = B - (B - T) * .55; c.globalAlpha = .12; c.fillStyle = fg; c.fillRect(L, G, Rr + 40 - L, B - G); c.globalAlpha = .85; c.setLineDash([5, 5]); c.beginPath(); c.moveTo(L, G); c.lineTo(Rr + 40, G); c.stroke(); c.setLineDash([]); lab(c, 'guard interval', L + 8, G + 12, fg);
    var p = Math.min(1, (t * .16) % 1.35);
    [['TCXO', 8.8, .55], ['OCXO', 2.6, .7], ['Rb', .9, .85], ['Cs', .3, 1]].forEach(function(cl){ c.globalAlpha = cl[2]; c.lineWidth = 2.2; c.strokeStyle = fg; c.beginPath(); var lx = 0, ly = 0; for (var i = 0; i <= 80; i++){ var s = i / 80 * p, e = Math.min(1.05, cl[1] * s * s), X = L + s * (Rr - L), Y = B - e * (B - T); i ? c.lineTo(X, Y) : c.moveTo(X, Y); lx = X; ly = Y; if (e >= 1.05) break; } c.stroke(); var sc = Math.sqrt(.55 / cl[1]); if (sc < p){ c.globalAlpha = 1; c.fillStyle = fg; c.beginPath(); c.arc(L + sc * (Rr - L), G, 4.5, 0, 6.283); c.fill(); } lab(c, cl[0], lx + 6, ly, fg); });
    lab(c, 'holdover →', Rr + 36, B + 14, fg, 'right'); c.save(); c.translate(L - 14, T + 2); c.rotate(-Math.PI / 2); lab(c, 'time error', 0, 0, fg, 'right'); c.restore(); },
  orbit: function(c, w, h, t, fg){ var cx = w * .5, cy = h * .54, a = Math.min(w * .38, h * .6), e = .55, b = a * Math.sqrt(1 - e * e), f = a * e; c.save(); c.translate(cx - f * .5, cy); c.rotate(-.32); c.globalAlpha = .55; c.strokeStyle = fg; c.lineWidth = 1.4; c.beginPath(); c.ellipse(0, 0, a, b, 0, 0, 6.283); c.stroke();
    c.globalAlpha = .4; c.setLineDash([3, 5]); c.beginPath(); c.arc(f, 0, a * .42, 0, 6.283); c.stroke(); c.setLineDash([]); c.globalAlpha = .9; c.fillStyle = fg; c.beginPath(); c.arc(f, 0, a * .15, 0, 6.283); c.fill();
    var pos = function(M){ var E = M; for (var i = 0; i < 6; i++) E = E - (E - e * Math.sin(E) - M) / (1 - e * Math.cos(E)); return [a * Math.cos(E), b * Math.sin(E)]; }, M0 = t * .8;
    for (var k = 40; k > 0; k--){ var q = pos(M0 - k * .02); c.globalAlpha = (1 - k / 40) * .7; c.beginPath(); c.arc(q[0], q[1], 2.2, 0, 6.283); c.fill(); }
    var q0 = pos(M0); c.globalAlpha = 1; c.fillRect(q0[0] - 4, q0[1] - 4, 8, 8); c.fillRect(q0[0] - 12, q0[1] - 1.5, 24, 3); var m = t * 1.1; c.beginPath(); c.arc(f + Math.cos(m) * a * .42, Math.sin(m) * a * .42, 3.5, 0, 6.283); c.fill(); c.restore();
    lab(c, 'SGP4', cx - a * .95, cy - b * .95, fg); lab(c, 'numerical', cx + a * .15, cy + b * 1.1, fg); },
  integ: function(c, w, h, t, fg){ var L = 16, Rr = w - 16, T = 34, B = h - 28, n = 24, bw = (Rr - L) / n, off = t * 2.6, fi = Math.floor(off), fr = off - fi, AL = T + (B - T) * .26;
    var pl = function(i){ return .34 + .15 * Math.sin(i * .63) + .09 * Math.sin(i * 1.9 + 1) + ((i % 31) >= 15 && (i % 31) <= 19 ? .42 * Math.sin(((i % 31) - 14) / 6 * Math.PI) : 0); }, flagged = null;
    for (var k = -1; k <= n; k++){ var i = k + fi, v = clamp(pl(i), .05, .98), X = L + (k - fr) * bw + bw * .18, H = v * (B - T), Y = B - H; if (X < L - bw * .3 || X > Rr - bw * .6) continue; c.fillStyle = fg; c.strokeStyle = fg; if (Y < AL){ c.globalAlpha = 1; c.lineWidth = 1.6; c.strokeRect(X, Y, bw * .64, H); c.globalAlpha = .3; c.fillRect(X, Y, bw * .64, H); if (!flagged) flagged = [X, Y]; } else { c.globalAlpha = .5; c.fillRect(X, Y, bw * .64, H); } }
    c.globalAlpha = 1; c.lineWidth = 2; c.strokeStyle = fg; c.beginPath(); c.moveTo(L, AL); c.lineTo(Rr, AL); c.stroke(); lab(c, 'alert limit', L, AL - 12, fg); lab(c, 'protection level', L, B + 14, fg); if (flagged) lab(c, 'PL > AL', Math.min(flagged[0], Rr - 60), flagged[1] - 12, fg); },
  drift: function(c, w, h, t, fg){ var L = 18, Rr = w - 18, yc = h * .56, x0 = L + (Rr - L) * .28, x1 = L + (Rr - L) * .84, p = Math.min(1, (t * .1) % 1.25), xp = L + (Rr - L) * p, x;
    var tr = function(x){ return yc + Math.sin((x - L) / (Rr - L) * 4) * h * .06; }, hw = function(x){ return x < x0 || x > x1 ? 0 : Math.pow((x - x0) / (x1 - x0), 1.5) * h * .3; }, est = function(x){ return tr(x) - (x > x0 && x < x1 ? Math.pow((x - x0) / (x1 - x0), 1.6) * h * .2 : 0); };
    c.strokeStyle = fg; c.fillStyle = fg;
    if (xp > x0){ var xe = Math.min(xp, x1); c.beginPath(); for (x = x0; x <= xe; x += 3) c.lineTo(x, tr(x) - hw(x)); for (x = xe; x >= x0; x -= 3) c.lineTo(x, tr(x) + hw(x)); c.closePath(); c.globalAlpha = .16; c.fill(); c.globalAlpha = .7; c.setLineDash([4, 4]); c.lineWidth = 1.2; c.stroke(); c.setLineDash([]); }
    c.globalAlpha = .35; c.setLineDash([2, 4]); c.lineWidth = 1.4; c.beginPath(); for (x = L; x <= xp; x += 3) c.lineTo(x, tr(x)); c.stroke(); c.setLineDash([]); c.globalAlpha = 1; c.lineWidth = 2.4; c.beginPath(); for (x = L; x <= xp; x += 3) c.lineTo(x, est(x)); c.stroke();
    c.globalAlpha = .7; c.setLineDash([2, 4]); c.lineWidth = 1; c.beginPath(); c.moveTo(x0, h * .12); c.lineTo(x0, h * .92); c.stroke(); c.setLineDash([]); lab(c, 'GNSS lost', x0 + 6, h * .14, fg);
    var xm = (x0 + x1) / 2; if (xp > xm) lab(c, 'INS coasting', xm, tr(xm) + hw(xm) + 14, fg, 'center'); if (xp > x1){ c.globalAlpha = .7; c.setLineDash([2, 4]); c.beginPath(); c.moveTo(x1, h * .12); c.lineTo(x1, h * .92); c.stroke(); c.setLineDash([]); lab(c, 'reacquired', x1 - 6, h * .14, fg, 'right'); }
    c.globalAlpha = 1; c.beginPath(); c.arc(xp, est(xp), 5.5, 0, 6.283); c.fill(); }
};
$$('canvas[data-diag]').forEach(function(cv){ var fn = DIAG[cv.dataset.diag], card = cv.closest('.fcard'), drawn = false;
  addLoop(cv, function(t){ if (RM && drawn) return; var G = fit(cv); if (!G) return; G.c.save(); fn(G.c, G.w, G.h, RM ? 6.2 : t, getComputedStyle(card).color); G.c.restore(); drawn = true; }); });
var fsegs = $$('#fprog i'), failsST = null;
function setProg(p){ fsegs.forEach(function(s, i){ s.style.transform = 'scaleX(' + clamp(p * 6 - i, 0, 1) + ')'; }); }
$('#failsVp').addEventListener('scroll', function(e){ var v = e.target; setProg(v.scrollLeft / Math.max(1, v.scrollWidth - v.clientWidth)); }, { passive: true });
$$('.chip[data-card]').forEach(function(ch){ ch.addEventListener('click', function(e){ e.preventDefault(); var i = +ch.dataset.card, cards = $$('.fcard'), vp = $('#failsVp');
  if (failsST){ window.scrollTo({ top: failsST.start + (failsST.end - failsST.start) * (i / (cards.length - 1)), behavior: RM ? 'auto' : 'smooth' }); }
  else { scroll('#fails'); vp.scrollTo({ left: cards[i].offsetLeft - 24, behavior: RM ? 'auto' : 'smooth' }); } }); });

/* =========================================================== MISSIONS */
var PRO = { twin: 'Pro: clock digital twins calibrated to your device\'s published Allan budget.', trade: 'Pro: architecture trade studies ranked on a figure of merit.', pack: 'Pro: audit-grade, reproducible evidence packs for a review.', custom: 'Custom: sensor and resilience models calibrated to your hardware.', export: 'Custom: export-controlled resilience work under the appropriate clearance and NDA.', lic: 'Commercial licence: closed or proprietary integration without AGPL obligations.' };
var SECTORS = [
  { n: 'Defence and security', c: '--int', viz: 'jam', q: 'How long do our own navigation and timing stay trustworthy under jamming and spoofing, and which layers keep us in spec?', t: 'Jamming, spoofing, meaconing, GNSS denial', files: ['jamming-demo', 'spoof-detect', 'spoof-attack', 'spoof-meaconing', 'conflict-resilience', 'combined-altpnt', 'gps-denied-gravity-nav'], fom: ['Effective C/N₀ and lock state per satellite under a jammer', 'Spoof verdict and which detection layer fired', 'Total-loss probability against threat intensity for layered PNT', 'Bounded alternative-PNT position error through the outage'], std: ['RAIM / ARAIM', 'IS-GPS-200', 'IEEE 1139'], note: 'Defensive use only: protecting your own navigation and timing, resilience assessment and holdover planning. Kshana does not model targeting, interception or guidance, and it is not an RF signal simulator or a receiver.', pro: [PRO.export, PRO.pack] },
  { n: 'Space: LEO, GEO and lunar', c: '--orb', viz: 'orbit', q: 'Does my user in orbit see enough satellites, and how long does its clock hold a time slot between fixes?', t: 'GNSS outage above the constellation, weak signals at lunar distance, clock drift between fixes', files: ['orbit-sgp4-gps', 'orbit-molniya', 'slot-timing-ocxo-leo', 'ephemeris', 'passes', 'moonlight-service-volume', 'lunanet-araim', 'earth-gnss-lunar', 'mars-pnt-lmo'], build: 'A geostationary user: set the user orbit in an orbit-kind scenario, the way orbit-molniya sets a highly eccentric one.', fom: ['GNSS availability and dilution of precision from real geometry', 'Slot guard time and the fix interval a clock needs', 'Lunar service-volume coverage and protection levels', 'Earth GNSS reception margin at lunar distance'], std: ['TLE / SGP4', 'SP3-c / d', 'CCSDS OEM / OMM', 'CCSDS 133.0', 'CCSDS TDM', 'CCSDS 401 / DSN'], pro: [PRO.trade, PRO.twin] },
  { n: 'Aviation and drones', c: '--nav', viz: 'drift', q: 'After a jammer takes GNSS away, how many seconds does my UAS (uncrewed aircraft system) stay inside its position budget, and is RAIM available on my route?', t: 'GNSS jamming, loss of integrity', files: ['small-uas-jammed-nav', 'jamming-demo', 'gnss-ins', 'ins-trn-coast', 'integrity-raim', 'araim-gps-galileo', 'araim-reference-check'], fom: ['Seconds inside the position budget, flight-controller MEMS against tactical-grade IMU', 'Horizontal and vertical protection levels and their availability against APV-I alert limits (40 m horizontal, 50 m vertical)', 'Coast error against coast duration'], std: ['RAIM / ARAIM', 'SBAS / DO-229E', 'IS-GPS-200'], pro: [PRO.pack, PRO.custom] },
  { n: 'Maritime', c: '--tim', viz: 'jam', q: 'How do a vessel\'s position and ship time hold up through a jamming or spoofing event near port?', t: 'Jamming and spoofing near shore, GNSS loss at sea', files: [], build: 'No maritime scenario ships yet. Build it from the jamming, spoof-detect, gnss-ins, clock and integrity kinds.', fom: ['Lock state and effective C/N₀ under the jammer', 'Spoof verdict', 'Inertial coast error and clock holdover through the outage'], std: ['RAIM / ARAIM', 'IEEE 1139'], pro: [PRO.custom, PRO.pack] },
  { n: 'Telecom and 5G timing', c: '--tim', viz: 'mask', q: 'If the GNSS reference is lost, does my rubidium clock stay inside the ITU-T masks for 24 hours?', t: 'Loss of the GNSS time reference, time spoofing', files: ['telecom-prtc-holdover-24h', 'telecom-tie-ingest', 'clock-holdover', 'spoof-attack', 'timetransfer'], fom: ['MTIE and TDEV against the selected ITU-T masks, with a pass, a fail and a margin per mask', 'Time to leave each mask in holdover', 'Time-spoof detection by the clock\'s integrity monitor'], std: ['IEEE 1139'], note: 'The engine carries ITU-T G.8272, G.8272.1, G.8273.2 and G.8271.1 masks, including PRTC-A, PRTC-B and ePRTC.', pro: [PRO.twin, PRO.pack] },
  { n: 'Energy, finance and critical infrastructure', c: '--itg', viz: 'clock', q: 'How long can a substation or a trading venue keep traceable time after GNSS goes, and would a time spoof be caught?', t: 'Loss of GNSS time, time spoofing', files: ['clock-holdover', 'clock-ensemble', 'sweep-clock-stability', 'spoof-attack', 'spoof-meaconing', 'quantum-time-transfer'], build: 'Your sector\'s time-accuracy limit goes in as threshold_ns.', fom: ['Timing error 95th percentile, holdover time and availability', 'Monte Carlo ensemble spread', 'Holdover against clock stability in a sweep', 'Spoof detectability'], std: ['IEEE 1139'], pro: [PRO.twin, PRO.pack] },
  { n: 'Automotive and rail', c: '--spf', viz: 'drift', q: 'How far does a vehicle drift through a tunnel or a jammer, and when should integrity stop trusting the fix?', t: 'GNSS loss in tunnels and under jamming, integrity', files: [], build: 'No road or rail scenario ships yet. Build it from the gnss-ins, integrity, jamming and spoof-detect kinds.', fom: ['Inertial coast error through the outage', 'Protection level against an alert limit you set', 'Lock state under the jammer'], std: ['RAIM / ARAIM', 'SBAS / DO-229E'], pro: [PRO.custom, PRO.lic] },
  { n: 'Research and academia', c: '--spf', viz: 'orbit', q: 'Can I reproduce, extend and cite a PNT result, down to the seed?', t: 'Any of the above, as an experiment', files: ['quantum-trade', 'hybrid-pnt', 'pvt-abmf', 'cislunar-observability', 'lunar-vlbi', 'impairment-eval', 'sweep-nd-inertial', 'conflict-resilience'], fom: ['Every figure with its unit and provenance', 'Deterministic from scenario, seed and engine version', 'A citable DOI for every release'], std: ['RINEX 3', 'SP3-c / d', 'IGRF-14'], pro: [PRO.lic] }
];
lazy($('#missions'), function(){
  var tabs = $('#sectorTabs'), cur = 0, t0 = performance.now();
  SECTORS.forEach(function(s, i){ var b = el('button'); b.type = 'button'; b.setAttribute('role', 'tab'); b.style.setProperty('--c', 'var(' + s.c + ')'); b.append(el('i'), document.createTextNode(s.n)); b.addEventListener('click', function(){ show(i); }); tabs.appendChild(b); });
  function show(i){ cur = i; var s = SECTORS[i], p = $('#missionPanel'); p.style.setProperty('--mc', 'var(' + s.c + ')'); $$('button', tabs).forEach(function(b, j){ b.setAttribute('aria-selected', j === i); });
    $('#mK').textContent = s.n; $('#mQ').textContent = s.q; $('#mT').textContent = s.t;
    var mf = $('#mFiles'); mf.textContent = ''; s.files.forEach(function(f){ var b = el('button'); b.type = 'button'; b.append(document.createTextNode(f)); b.appendChild(el('small', null, D.fileKind[f])); b.title = D.fileDesc[f] || ''; b.addEventListener('click', function(){ window.KSload(f); scroll('#playground'); }); mf.appendChild(b); });
    if (!s.files.length){ var e = el('span', null, 'None yet.'); e.style.cssText = 'font-size:13.5px;color:var(--ink-3)'; mf.appendChild(e); }
    $('#mBuild').textContent = s.build ? (s.files.length ? '' : '') + s.build : '';
    var fl = $('#mFom'); fl.textContent = ''; s.fom.forEach(function(f){ fl.appendChild(el('li', null, f)); });
    var st = $('#mStd'); st.textContent = ''; s.std.forEach(function(x){ st.appendChild(el('span', null, x)); });
    $('#mNote').textContent = s.note || '';
    $('#mFree').textContent = s.files.length ? 'Run all ' + s.files.length + ' scenario files above, read the ledger and export reports, in the browser or from any install.' : 'Compose the scenario from the named kinds and run it in the browser or from any install.';
    $('#mPro').textContent = s.pro.join(' '); $('#mProL').textContent = s.pro[0].indexOf('Pro') === 0 ? 'Pro adds' : 'Pro and custom add'; }
  var mv = $('#mViz');
  addLoop(mv, function(t){ var G = fit(mv); if (!G) return; var c = G.c, W = G.w, H = G.h, s = SECTORS[cur], col = cssv('--s-' + s.c.slice(2)) || '#fff'; if (RM) t = 6;
    if (s.viz === 'mask'){ var L = 30, B = H - 26, x = function(k){ return L + k * (W - L - 16); }, y = function(k){ return B - k * (B - 20); }; c.strokeStyle = 'rgba(255,255,255,.12)'; c.beginPath(); c.moveTo(L, B); c.lineTo(W - 16, B); c.stroke(); c.strokeStyle = '#FF7A73'; c.setLineDash([5, 4]); c.beginPath(); c.moveTo(x(0), y(.35)); c.lineTo(x(.45), y(.8)); c.lineTo(x(1), y(.8)); c.stroke(); c.setLineDash([]); c.strokeStyle = col; c.lineWidth = 2.2; c.beginPath(); for (var i = 0; i <= 80; i++){ var k = i / 80; var v = .1 + .5 * k + .03 * Math.sin(i * .9 + t * 2); i ? c.lineTo(x(k), y(Math.min(v, .7))) : c.moveTo(x(k), y(v)); } c.stroke(); c.lineWidth = 1; lab(c, 'mask', x(.98), y(.8) - 10, '#FF7A73', 'right'); lab(c, 'MTIE', x(.98), y(.66) + 12, col, 'right'); }
    else if (s.viz === 'clock'){ var fns = [2.4, .9, .3]; c.strokeStyle = 'rgba(255,255,255,.3)'; c.setLineDash([5, 5]); var gy = H * .4; c.beginPath(); c.moveTo(20, gy); c.lineTo(W - 20, gy); c.stroke(); c.setLineDash([]); lab(c, 'your threshold_ns', 24, gy - 10, 'rgba(255,255,255,.6)'); var p = ((t * .12) % 1.2); fns.forEach(function(k, j){ c.strokeStyle = col; c.globalAlpha = .45 + j * .25; c.lineWidth = 2; c.beginPath(); for (var i = 0; i <= 60; i++){ var q = i / 60 * Math.min(p, 1), e = k * q * q; c.lineTo(20 + q * (W - 40), H - 20 - Math.min(e, 1.1) * (H - 60)); } c.stroke(); }); c.globalAlpha = 1; }
    else { var fn = DIAG[s.viz]; if (fn){ c.save(); fn(c, W, H, t, col); c.restore(); } } });
  show(0); window.KSsector = show;
});

/* =========================================================== ROLES + WAYS */
var ROLES = [
  { n: 'Mission and systems engineer', c: '--orb', p: 'Size a PNT architecture and see where it breaks.', e: ['Capability explorer', '#explorer'], cmd: 'kshana scenarios/hybrid-pnt.toml', cue: PRO.trade },
  { n: 'PNT or GNSS specialist', c: '--tim', p: 'Open the TOML, change the physics, check it against the ledger.', e: ['Validation ledger', '#ledger'], cmd: 'kshana --validate scenarios/integrity-raim.toml', cue: PRO.custom },
  { n: 'Test and evaluation lead', c: '--int', p: 'Pin scenarios as regression tests and run a whole suite into one stamped study.', e: ['Ways to use it', '#ways'], cmd: 'kshana --study scenarios/quantum-pnt-demonstrator.suite.toml', cue: PRO.pack },
  { n: 'Procurement or programme reviewer', c: '--itg', p: 'Use the validation ledger as due-diligence evidence: every capability names its oracle and its test.', e: ['Verification matrix', '#proof'], cmd: 'https://github.com/ashfordeOU/kshana', cue: PRO.pack },
  { n: 'Researcher or student', c: '--spf', p: 'Reproduce a result to the seed, extend it and cite the exact engine version.', e: ['Cite and publications', '#cite'], cmd: 'pip install kshana', cue: 'Free: the whole engine, citable by DOI.' },
  { n: 'Timing and network engineer', c: '--tim', p: 'Check holdover against ITU-T masks before the GNSS antenna fails.', e: ['Timing wall', '#timing'], cmd: 'kshana scenarios/telecom-prtc-holdover-24h.toml', cue: PRO.twin },
  { n: 'AI-assistant user', c: '--spf', p: 'Let your assistant run the validated engine instead of guessing the maths.', e: ['MCP server', '#mcp'], cmd: 'cargo install kshana-mcp', cue: 'Free: seven MCP tools, from run_scenario to export_oem.' },
  { n: 'Embedded or product developer', c: '--nav', p: 'Call the engine from Rust, Python or JavaScript inside your own product.', e: ['Developers', '#developers'], cmd: 'cargo add kshana', cue: PRO.lic }
];
lazy($('#who'), function(){ var g = $('#roles'); ROLES.forEach(function(r){ var a = el('article', 'role'); a.style.setProperty('--c', 'var(' + r.c + ')'); a.append(el('h4', null, r.n), el('p', null, r.p)); var en = el('div', 'entry', 'Start at '); var lk = el('a', null, r.e[0]); lk.href = r.e[1]; en.appendChild(lk); a.appendChild(en);
  var sn = el('div', 'snip'), cd = el('code', null, r.cmd), bt = el('button', 'tbtn', r.cmd.indexOf('http') === 0 ? 'Open' : 'Copy'); bt.type = 'button'; if (r.cmd.indexOf('http') === 0) bt.addEventListener('click', function(){ window.open(r.cmd, '_blank', 'noopener'); }); else bt.setAttribute('data-copy', r.cmd); sn.append(cd, bt); a.appendChild(sn);
  var cue = el('div', 'cue'), b = el('b', null, 'Free today: '); cue.append(b, document.createTextNode('everything in the open engine. ' + r.cue)); a.appendChild(cue); g.appendChild(a); }); bindCopy(g); });
var WAYS = [
  ['Browser playground', '--tim', 'https://kshana.dev', 'Any bundled scenario, run on your machine as WebAssembly; nothing installed, nothing uploaded'],
  ['Command line', '--int', 'cargo install kshana', 'kshana <scenario>.toml writes result.json, chart.svg and report.html'],
  ['Rust crate', '--nav', 'cargo add kshana', 'kshana::api::run_toml returns JSON, SVG, a summary and CSV'],
  ['Python', '--tim', 'pip install kshana', 'kshana.run and run_full, plus validate_toml and list_kinds'],
  ['npm · WebAssembly', '--itg', 'npm install kshana', 'run, summary and chart_svg in the browser or Node.js'],
  ['MCP server', '--spf', 'cargo install kshana-mcp', 'Seven tools an AI assistant can call, each a thin wrapper over the public API'],
  ['Docker image', '--orb', 'docker run --rm -i ghcr.io/ashfordeou/kshana-mcp', 'The MCP server without a Rust toolchain, multi-architecture'],
  ['JetBrains plugin', '--orb', 'https://plugins.jetbrains.com/plugin/32181-kshana--pnt-simulator', 'Right-click a scenario .toml, Run Kshana Scenario, read the figures in a tool window'],
  ['Embed mode', '--spf', 'https://kshana.dev/?embed=1&scenario=integrity-raim.toml&tab=fom', 'Only the playground, for a course page or a report, with the scenario and tab preselected'],
  ['CI regression', '--int', 'kshana --validate scenarios/integrity-raim.toml', 'Output is deterministic from scenario, seed and engine version, so a committed result.json is a regression test: rerun and diff it'],
  ['Reports for reviews', '--itg', 'kshana --study scenarios/quantum-pnt-demonstrator.suite.toml', 'A suite of scenarios into one stamped study.json and study.html']
];
lazy($('#ways'), function(){ var tb = $('#waysBody'); WAYS.forEach(function(w){ var tr = el('tr'), a = el('td'), i = el('i'); i.style.setProperty('--c', 'var(' + w[1] + ')'); a.append(i, document.createTextNode(w[0])); var b = el('td'), box = el('div', 'cmdc');
  if (w[2].indexOf('http') === 0){ var lk = el('a', null, w[2]); lk.href = w[2]; box.appendChild(lk); } else { box.appendChild(el('code', null, w[2])); var cb = el('button', null, 'Copy'); cb.type = 'button'; cb.setAttribute('data-copy', w[2]); box.appendChild(cb); }
  b.appendChild(box); tr.append(a, b, el('td', null, w[3])); tb.appendChild(tr); }); bindCopy(tb); });

/* =========================================================== MISSION TIMELINE (Observatory) */
var TL = { T: .66, manual: false, playing: false, t0: 0, from: 0 };
if (useST && matchMedia('(min-width: 900px)').matches){ TL.T = 0; ScrollTrigger.create({ trigger: '#tlPin', start: 'top top', end: '+=180%', pin: true, scrub: .6, onUpdate: function(s){ if (!TL.manual && !TL.playing) TL.T = s.progress; else if (Math.abs(s.getVelocity()) > 40){ TL.manual = false; TL.playing = false; } } }); }
lazy($('#timeline'), function(){
  var PH = [
    { n: 'Nominal', cap: 'Every satellite is tracked. The clock is disciplined to GNSS time and the protection level sits far below the alert limit.' },
    { n: 'Jamming', cap: 'A jammer raises the noise floor. C/N₀ falls through the tracking floor and satellites drop out; the clock starts to coast.' },
    { n: 'Spoofing', cap: 'A spoofer transmits counterfeit signals that look stronger than the real ones. Consistency checks flag the pull-off before the position is trusted.' },
    { n: 'Holdover', cap: 'GNSS is excluded. The receiver clock runs on its own oscillator and time error grows toward the 1 µs guard.' },
    { n: 'Integrity alarm', cap: 'The protection level crosses the alert limit. The system raises an integrity alarm instead of outputting a position it cannot bound.' }
  ];
  var LOG = [[0, 'ok', 'All four constellations tracked · 31 satellites'], [.08, '', 'Clock disciplined to GNSS time'], [.2, 'w', 'C/N₀ falling on L1 · jammer detected'], [.27, 'e', 'Loss of lock on 22 satellites'], [.33, 'w', 'Clock coasting on oscillator'], [.4, 's', 'Signal power anomaly · counterfeit suspected'], [.47, 's', 'Spoofed fix diverging from inertial track'], [.53, 'ok', 'Spoof flagged · GNSS excluded'], [.6, 'w', 'Holdover · oscillator free-running'], [.72, 'w', 'Time error 500 ns · half the guard'], [.8, 'e', 'Protection level 41 m > alert limit 40 m'], [.9, 'e', 'Integrity alarm raised · position withheld']];
  function clock(u){ var s = 5 * 3600 + 33 * 60 + Math.round(u * 45 * 60), h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, x = s % 60; return (h < 10 ? '0' : '') + h + ':' + (m < 10 ? '0' : '') + m + ':' + (x < 10 ? '0' : '') + x; }
  var logEl = $('#tlLog'), rows = LOG.map(function(L){ var d = el('div'); d.append(el('span', null, clock(L[0]).slice(0, 5)), el('span', L[1], L[2])); logEl.appendChild(d); return d; });
  function nz(u, k){ return Math.sin(u * 91 + k) * .6 + Math.sin(u * 237 + k * 2) * .4; }
  function cn(u){ if (u < .2) return 45 + nz(u, 1) * .8; if (u < .4) return 45 - 21 * ss(.2, .3, u) + nz(u, 2); if (u < .6) return 47.5 + nz(u, 3) * .6; if (u < .8) return NaN; return 34 + 6 * ss(.8, .95, u) + nz(u, 4); }
  function te(u){ if (u < .25) return 2 + nz(u, 5) * .3; if (u < .6) return 2 + 60 * Math.pow(ss(.25, .6, u), 2); return 62 + 1600 * Math.pow(ss(.6, 1, u), 1.6); }
  function pl(u){ if (u < .2) return 9 + nz(u, 6) * .6; if (u < .4) return 9 + 18 * ss(.2, .35, u); if (u < .6) return 27 - 12 * ss(.4, .55, u); return 15 + 43 * Math.pow(ss(.6, 1, u), 1.2); }
  var skyC = $('#skyC');
  var SATS = []; for (var i = 0; i < 14; i++) SATS.push({ az: (i * 137.5) % 360, el: 12 + ((i * 53) % 70), c: ['#58a4ff', '#ae90ff', '#ff7d66', '#ffc247'][i % 4], k: (i * 0.137) % 1 });
  function drawSky(u, t){ var G = fit(skyC); if (!G) return; var c = G.c, W = G.w, H = G.h, cx = W / 2, cy = H / 2 + 8, R = Math.min(W, H) / 2 - 22;
    c.strokeStyle = 'rgba(255,255,255,.1)'; c.lineWidth = 1; [1, 2 / 3, 1 / 3].forEach(function(k){ c.beginPath(); c.arc(cx, cy, R * k, 0, 6.283); c.stroke(); }); for (var a = 0; a < 360; a += 45){ var r = a * Math.PI / 180; c.beginPath(); c.moveTo(cx, cy); c.lineTo(cx + Math.sin(r) * R, cy - Math.cos(r) * R); c.stroke(); }
    lab(c, 'N', cx, cy - R - 12, 'rgba(255,255,255,.5)', 'center'); lab(c, 'E', cx + R + 10, cy, 'rgba(255,255,255,.5)', 'center');
    var jam = ss(.2, .3, u) * (1 - ss(.8, .9, u)), spf = ss(.4, .45, u) * (1 - ss(.58, .62, u)), hold = ss(.6, .62, u) * (1 - ss(.8, .86, u));
    if (jam > .01){ var ja = 110 * Math.PI / 180, jx = cx + Math.sin(ja) * R, jy = cy - Math.cos(ja) * R, gr = c.createRadialGradient(jx, jy, 4, jx, jy, R * 1.3); gr.addColorStop(0, 'rgba(255,91,84,' + (.45 * jam) + ')'); gr.addColorStop(1, 'rgba(255,91,84,0)'); c.fillStyle = gr; c.beginPath(); c.arc(cx, cy, R, 0, 6.283); c.fill(); lab(c, 'jammer', jx, jy + 14, '#FF7A73', 'center'); }
    SATS.forEach(function(s, i){ var r = (90 - s.el) / 90 * R, az = (s.az + t * 1.5) * Math.PI / 180, x = cx + Math.sin(az) * r, y = cy - Math.cos(az) * r, lost = jam > s.k * .9 + .08 || hold > .5;
      c.fillStyle = lost ? 'rgba(255,255,255,.25)' : s.c; c.beginPath(); c.arc(x, y, lost ? 3 : 4.5, 0, 6.283); c.fill(); if (!lost){ c.globalAlpha = .25; c.beginPath(); c.arc(x, y, 9, 0, 6.283); c.fill(); c.globalAlpha = 1; }
      if (spf > .05 && i % 3 === 0){ c.globalAlpha = spf; c.strokeStyle = '#B9A4FF'; c.setLineDash([3, 3]); c.beginPath(); c.arc(x + 16 * spf, y - 10 * spf, 6, 0, 6.283); c.stroke(); c.setLineDash([]); c.globalAlpha = 1; } });
    var alarm = ss(.79, .81, u); c.fillStyle = alarm > .5 ? '#FF7A73' : hold > .5 ? '#FFC24D' : '#4BE08F'; c.beginPath(); c.arc(cx, cy, 5, 0, 6.283); c.fill(); c.strokeStyle = c.fillStyle; c.globalAlpha = .6; c.beginPath(); c.arc(cx, cy, 10 + pl(u) * .35, 0, 6.283); c.stroke(); c.globalAlpha = 1; lab(c, 'receiver · PL ring', cx + 14, cy + 22, 'rgba(255,255,255,.6)'); }
  function chart(cv, f, u, lo, hi, col, log, line, lineLab){ var G = fit(cv); if (!G) return; var c = G.c, W = G.w, H = G.h;
    var map = function(v){ var x = log ? (Math.log10(Math.max(v, lo)) - Math.log10(lo)) / (Math.log10(hi) - Math.log10(lo)) : (v - lo) / (hi - lo); return H - 4 - clamp(x, 0, 1) * (H - 10); };
    [.2, .4, .6, .8].forEach(function(x){ c.strokeStyle = 'rgba(255,255,255,.06)'; c.beginPath(); c.moveTo(W * x, 0); c.lineTo(W * x, H); c.stroke(); });
    c.strokeStyle = 'rgba(255,255,255,.45)'; c.setLineDash([4, 4]); c.beginPath(); c.moveTo(0, map(line)); c.lineTo(W, map(line)); c.stroke(); c.setLineDash([]); lab(c, lineLab, W - 4, map(line) - 8, 'rgba(255,255,255,.55)', 'right');
    var n = 240, st = false, q, v, i; c.beginPath(); for (i = 0; i <= n; i++){ q = i / n; if (q > u) break; v = f(q); if (isNaN(v)){ st = false; continue; } st ? c.lineTo(W * q, map(v)) : c.moveTo(W * q, map(v)); st = true; } c.strokeStyle = col; c.lineWidth = 2; c.lineJoin = 'round'; c.stroke(); c.lineWidth = 1;
    c.strokeStyle = 'rgba(255,255,255,.5)'; c.beginPath(); c.moveTo(W * u, 0); c.lineTo(W * u, H); c.stroke(); }
  var lastPh = -1;
  function render(t){ var u = TL.T, ph = Math.min(4, Math.floor(u * 5));
    if (ph !== lastPh){ lastPh = ph; $('#tlTitle').textContent = PH[ph].n; $('#tlPn').textContent = (ph + 1) + '/5'; $('#tlCap').textContent = PH[ph].cap; $$('#scrubLabels span').forEach(function(s, i){ s.classList.toggle('on', i === ph); }); }
    $('#tlClock').textContent = clock(u); $('#scrubFill').style.width = (u * 100) + '%'; $('#scrubHandle').style.left = (u * 100) + '%'; $('#scrub').setAttribute('aria-valuenow', Math.round(u * 100));
    var v = cn(u); $('#tvCn').textContent = isNaN(v) ? 'no signal' : v.toFixed(0) + ' dB-Hz'; var e = te(u); $('#tvTe').textContent = e < 1000 ? Math.round(e) + ' ns' : (e / 1000).toFixed(2) + ' µs'; var p = pl(u); $('#tvPl').textContent = Math.round(p) + ' m'; $('#tvPl').style.color = p > 40 ? '#FF7A73' : '';
    rows.forEach(function(r, i){ r.classList.toggle('on', LOG[i][0] <= u + 1e-6); });
    drawSky(u, RM ? 0 : t); chart($('#tcCn'), cn, u, 15, 52, '#2CD0DE', false, 28, 'tracking floor 28'); chart($('#tcTe'), te, u, 1, 3000, '#FFB224', true, 1000, 'guard 1 µs'); chart($('#tcPl'), pl, u, 0, 60, '#3DDC84', false, 40, 'alert limit 40 m'); }
  addLoop($('#timeline'), function(t){ if (TL.playing){ TL.T = clamp(TL.from + (performance.now() - TL.t0) / 12000, 0, 1); if (TL.T >= 1) TL.playing = false; } render(t); });
  var scrub = $('#scrub');
  function setFromX(x){ var r = scrub.getBoundingClientRect(); TL.T = clamp((x - r.left) / r.width, 0, 1); TL.manual = true; TL.playing = false; }
  scrub.addEventListener('pointerdown', function(e){ scrub.setPointerCapture(e.pointerId); setFromX(e.clientX); });
  scrub.addEventListener('pointermove', function(e){ if (e.buttons) setFromX(e.clientX); });
  scrub.addEventListener('keydown', function(e){ if (e.key === 'ArrowRight' || e.key === 'ArrowLeft'){ e.preventDefault(); TL.T = clamp(TL.T + (e.key === 'ArrowRight' ? .02 : -.02), 0, 1); TL.manual = true; } if (e.key === 'Home'){ TL.T = 0; TL.manual = true; } if (e.key === 'End'){ TL.T = 1; TL.manual = true; } });
  $('#tlPlay').addEventListener('click', function(){ TL.playing = true; TL.manual = true; TL.from = TL.T >= .99 ? 0 : TL.T; TL.t0 = performance.now(); });
});

/* =========================================================== EARTH → MOON → MARS (copy side; 3D in the module) */
(function(){ var J = [['orbit-sgp4-gps', 'orbit-real-tle', 'orbit-molniya', 'slot-timing-ocxo-leo', 'passes'], ['earth-gnss-lunar', 'moonlight-service-volume', 'lunanet-araim', 'lunar-time-offset', 'lunar-llr-datum', 'lunar-vlbi', 'cislunar-observability'], ['mars-pnt-lmo', 'mars-pnt-surface', 'mars-pnt-transfer']];
  J.forEach(function(list, i){ var box = $('#jT' + i); list.forEach(function(f){ var cd = el('code', null, f); cd.title = D.fileDesc[f] || ''; cd.style.cursor = 'pointer'; cd.addEventListener('click', function(){ window.KSload(f); scroll('#playground'); }); box.appendChild(cd); }); });
  var sec = $('#journey'), stops = $$('.j-stop'), rail = $$('.j-rail span'), last = -1; KS.j = 0;
  function upd(p){ KS.j = p; var i = p < .36 ? 0 : p < .7 ? 1 : 2; if (i !== last){ last = i; stops.forEach(function(s, k){ s.classList.toggle('on', k === i); }); rail.forEach(function(s, k){ s.classList.toggle('on', k === i); }); } }
  if (useST) ScrollTrigger.create({ trigger: sec, start: 'top top', end: 'bottom bottom', onUpdate: function(s){ upd(s.progress); } });
  else if (!RM) addEventListener('scroll', function(){ var r = sec.getBoundingClientRect(), h = r.height - innerHeight; upd(h > 0 ? clamp(-r.top / h, 0, 1) : 0); }, { passive: true });
  upd(0); })();

/* =========================================================== CLOCK LADDER + TIMING WALL */
lazy($('#clocks'), function(){
  var LNAME = { OpticalMaster: 'Optical master', Phm: 'Passive hydrogen maser', Rafs: 'RAFS (lunar class)', MiniRafs: 'Mini-RAFS' };
  var PNAME = { ocxo: 'OCXO · OX-208', rubidium: 'Rubidium · 8040C', caesium: 'Caesium · 5071A', csac: 'CSAC · SA.45s' };
  var PCOL = { ocxo: '--nav', rubidium: '--tim', caesium: '--ink', csac: '--spf' };
  /* datasheet curves: log–log interpolation, held flat outside the datasheet range */
  function interp(pts, tau){ if (tau <= pts[0][0]) return pts[0][1]; for (var i = 1; i < pts.length; i++){ if (tau <= pts[i][0]){ var a = pts[i - 1], b = pts[i], k = (Math.log(tau) - Math.log(a[0])) / (Math.log(b[0]) - Math.log(a[0])); return Math.exp(Math.log(a[1]) + k * (Math.log(b[1]) - Math.log(a[1]))); } } return pts[pts.length - 1][1]; }
  var CLOCKS = [];
  D.presets.forEach(function(p){ CLOCKS.push({ n: PNAME[p.id] || p.id, c: PCOL[p.id] || '--orb', kind: 'datasheet', src: p.model, sy: function(t){ return interp(p.adev, t); }, pts: p.adev }); });
  CLOCKS.push({ n: 'TCXO class', c: '--int', kind: 'class', src: 'σy(1 s) ' + D.classes.tcxo.toExponential(0), sy: function(t){ return D.classes.tcxo / Math.sqrt(t); } });
  CLOCKS.push({ n: 'USO class', c: '--orb', kind: 'class', src: 'σy(1 s) ' + D.classes.uso.toExponential(0), sy: function(t){ return D.classes.uso / Math.sqrt(t); } });
  CLOCKS.push({ n: 'DSAC class', c: '--itg', kind: 'class', src: 'σy(1 s) ' + D.classes.dsac.toExponential(0), sy: function(t){ return D.classes.dsac / Math.sqrt(t); } });
  /* lunar clocks: flicker floors (optical, PHM) and white-FM lines (RAFS), calibrated to their cited one-day time error x = σy·τ */
  ['OpticalMaster', 'Phm'].forEach(function(k){ var fl = D.lunarDay[k] * 1e-9 / 86400; CLOCKS.push({ n: LNAME[k], c: k === 'Phm' ? '--orb' : '--spf', kind: 'floor', src: 'flicker floor ' + fl.toExponential(2), sy: function(){ return fl; } }); });
  var ldH = $('#ldH'), guard = 1000, cv = $('#ldC');
  function draw(){ var G = fit(cv); if (!G) return; var c = G.c, W = G.w, H = G.h, L = 58, R = 14, T = 12, B = 30, ink3 = cssv('--ink-3'), line = cssv('--line');
    var lx0 = 0, lx1 = 6.5, ly0 = -17, ly1 = -9, X = function(t){ return L + (Math.log10(t) - lx0) / (lx1 - lx0) * (W - L - R); }, Y = function(v){ return H - B - (Math.log10(v) - ly0) / (ly1 - ly0) * (H - T - B); };
    c.font = '10px "Geist Mono", monospace';
    for (var e = ly0; e <= ly1; e++){ c.strokeStyle = line; c.beginPath(); c.moveTo(L, Y(Math.pow(10, e))); c.lineTo(W - R, Y(Math.pow(10, e))); c.stroke(); c.fillStyle = ink3; c.textAlign = 'right'; c.fillText('1e' + e, L - 6, Y(Math.pow(10, e)) + 3); }
    [[1, '1 s'], [60, '1 min'], [3600, '1 h'], [86400, '1 day'], [2592000, '30 d']].forEach(function(p){ c.fillStyle = ink3; c.textAlign = 'center'; c.fillText(p[1], X(p[0]), H - 10); c.strokeStyle = line; c.beginPath(); c.moveTo(X(p[0]), T); c.lineTo(X(p[0]), H - B); c.stroke(); });
    var tau = Math.max(1, ldH.value * 3600); c.strokeStyle = cssv('--ink'); c.globalAlpha = .35; c.setLineDash([4, 4]); c.beginPath(); c.moveTo(X(tau), T); c.lineTo(X(tau), H - B); c.stroke(); c.setLineDash([]); c.globalAlpha = 1;
    CLOCKS.forEach(function(k){ var col = cssv(k.c); c.strokeStyle = col; c.lineWidth = k.kind === 'datasheet' ? 2.2 : 1.6; if (k.kind !== 'datasheet') c.setLineDash([6, 4]); c.beginPath(); var st = false; for (var i = 0; i <= 120; i++){ var t = Math.pow(10, lx0 + (lx1 - lx0) * i / 120), v = k.sy(t); if (v <= 0 || Math.log10(v) < ly0 || Math.log10(v) > ly1){ st = false; continue; } st ? c.lineTo(X(t), Y(v)) : c.moveTo(X(t), Y(v)); st = true; } c.stroke(); c.setLineDash([]); c.lineWidth = 1;
      if (k.pts) k.pts.forEach(function(p){ c.fillStyle = col; c.beginPath(); c.arc(X(p[0]), Y(p[1]), 3.2, 0, 6.283); c.fill(); }); }); }
  var key = $('#ldKey'); CLOCKS.forEach(function(k){ var s = el('span'), i = el('i', k.kind === 'datasheet' ? '' : 'dash'); i.style.setProperty('--c', 'var(' + k.c + ')'); s.title = k.src; s.append(i, document.createTextNode(k.n)); key.appendChild(s); });
  var ks = el('span'); ks.style.color = 'var(--ink-3)'; ks.textContent = 'solid + dots: datasheet points · dashed: class figure or power-law floor'; key.appendChild(ks);
  var bars = $('#ldBars');
  function bars_(){ var tau = ldH.value * 3600; $('#ldHo').textContent = ldH.value + ' h'; bars.textContent = ''; var items = CLOCKS.map(function(k){ return [k, tau > 0 ? k.sy(tau) * tau * 1e9 : 0]; }).sort(function(a, b){ return a[1] - b[1]; });
    items.forEach(function(it){ var r = el('div', 'ldb'), bar = el('span', 'bar'), fill = el('i'), mark = el('b'); var lo = -3, hi = 7, f = function(v){ return clamp((Math.log10(Math.max(v, 1e-3)) - lo) / (hi - lo), 0, 1) * 100; };
      fill.style.width = f(it[1]) + '%'; fill.style.setProperty('--c', 'var(' + it[0].c + ')'); fill.style.background = 'var(' + it[0].c + ')'; mark.style.left = f(guard) + '%'; bar.append(fill, mark); var v = it[1], txt = v < 1 ? (v * 1000).toFixed(v < .01 ? 2 : 0) + ' ps' : v < 1000 ? v.toFixed(v < 10 ? 1 : 0) + ' ns' : v < 1e6 ? (v / 1000).toFixed(1) + ' µs' : (v / 1e6).toFixed(1) + ' ms';
      var em = el('em', v > guard ? 'over' : '', txt); r.append(el('span', null, it[0].n), bar, em); bars.appendChild(r); }); draw(); }
  ldH.addEventListener('input', bars_); ldH.style.setProperty('--v', (ldH.value / 48 * 100) + '%'); ldH.addEventListener('input', function(){ ldH.style.setProperty('--v', (ldH.value / 48 * 100) + '%'); });
  $$('#ldG button').forEach(function(b){ b.addEventListener('click', function(){ guard = +b.dataset.g; $$('#ldG button').forEach(function(x){ x.setAttribute('aria-pressed', x === b); }); bars_(); }); });
  bars_();
  /* MTIE mask (real segments) with an illustrative curve */
  var mk = 'PRTC_A_MTIE', mkQ = $('#mkQ');
  function maskAt(tau){ var segs = D.masks[mk]; for (var i = 0; i < segs.length; i++){ var s = segs[i]; if (tau > s[0] && (s[1] == null || tau <= s[1])) return s[2] * tau + s[3]; } return null; }
  function mtie(tau){ var k = Math.pow(10, (mkQ.value - 40) / 40); return k * (6 + 40 * (1 - Math.exp(-Math.pow(tau / 60, .6)))); }
  function drawMask(){ var G = fit($('#mkC')); if (!G) return; var c = G.c, W = G.w, H = G.h, L = 56, R = 14, T = 12, B = 30, ink3 = cssv('--ink-3'), line = cssv('--line'), int = cssv('--int'), itg = cssv('--itg'), tim = cssv('--tim');
    var lx0 = -1, lx1 = 5, ly0 = 0, ly1 = 3.3, X = function(t){ return L + (Math.log10(t) - lx0) / (lx1 - lx0) * (W - L - R); }, Y = function(v){ return H - B - (Math.log10(Math.max(v, 1)) - ly0) / (ly1 - ly0) * (H - T - B); };
    c.font = '10px "Geist Mono", monospace'; [1, 10, 100, 1000].forEach(function(v){ c.strokeStyle = line; c.beginPath(); c.moveTo(L, Y(v)); c.lineTo(W - R, Y(v)); c.stroke(); c.fillStyle = ink3; c.textAlign = 'right'; c.fillText(v + ' ns', L - 6, Y(v) + 3); });
    [.1, 1, 10, 100, 1000, 10000, 100000].forEach(function(t){ c.textAlign = 'center'; c.fillStyle = ink3; c.fillText(t < 1 ? '0.1 s' : t >= 1000 ? (t / 1000) + 'k s' : t + ' s', X(t), H - 10); });
    c.beginPath(); c.moveTo(X(.1001), T); for (var i = 0; i <= 200; i++){ var t = Math.pow(10, lx0 + (lx1 - lx0) * i / 200), m = maskAt(t); if (m != null) c.lineTo(X(t), Y(m)); } c.lineTo(W - R, T); c.closePath(); c.fillStyle = int; c.globalAlpha = .08; c.fill(); c.globalAlpha = 1;
    c.beginPath(); var st = false; for (i = 0; i <= 200; i++){ t = Math.pow(10, lx0 + (lx1 - lx0) * i / 200); m = maskAt(t); if (m == null) continue; st ? c.lineTo(X(t), Y(m)) : c.moveTo(X(t), Y(m)); st = true; } c.strokeStyle = int; c.lineWidth = 2; c.setLineDash([6, 4]); c.stroke(); c.setLineDash([]); c.lineWidth = 1;
    var fail = false, worst = Infinity; c.beginPath(); for (i = 0; i <= 200; i++){ t = Math.pow(10, lx0 + (lx1 - lx0) * i / 200); var v = mtie(t); m = maskAt(t); if (m != null){ worst = Math.min(worst, m - v); if (v > m) fail = true; } i ? c.lineTo(X(t), Y(v)) : c.moveTo(X(t), Y(v)); } c.strokeStyle = fail ? int : tim; c.lineWidth = 2.4; c.stroke(); c.lineWidth = 1;
    c.fillStyle = int; c.textAlign = 'left'; c.fillText('fail region above the mask · ' + (mk === 'PRTC_A_MTIE' ? 'PRTC-A, G.8272 Table 1' : 'PRTC-B, G.8272 Table 2'), L + 8, T + 12); c.fillStyle = fail ? int : tim; c.fillText('MTIE (illustrative)', L + 8, T + 26);
    var vd = $('#mkV'); vd.textContent = fail ? 'FAIL' : 'PASS · margin ' + Math.round(worst) + ' ns'; vd.classList.toggle('fail', fail); $('#mkQo').textContent = '×' + Math.pow(10, (mkQ.value - 40) / 40).toFixed(2); }
  mkQ.addEventListener('input', function(){ mkQ.style.setProperty('--v', mkQ.value + '%'); drawMask(); }); mkQ.style.setProperty('--v', mkQ.value + '%');
  $$('#mkSel button').forEach(function(b){ b.addEventListener('click', function(){ mk = b.dataset.m; $$('#mkSel button').forEach(function(x){ x.setAttribute('aria-pressed', x === b); }); drawMask(); }); });
  function drawSlot(t){ var G = fit($('#slC')); if (!G) return; var c = G.c, W = G.w, H = G.h, ink3 = cssv('--ink-3'), nav = cssv('--nav'), tim = cssv('--tim'), gy1 = H * .28, gy2 = H * .72, cy = H / 2;
    c.fillStyle = cssv('--bg-2'); c.fillRect(0, gy1, W, gy2 - gy1); c.strokeStyle = ink3; c.setLineDash([5, 5]); [gy1, gy2].forEach(function(y){ c.beginPath(); c.moveTo(0, y); c.lineTo(W, y); c.stroke(); }); c.setLineDash([]); lab(c, 'slot guard', 6, gy1 - 10, ink3);
    var period = W * .42, ph = RM ? .3 : ((t * .08) % 1); c.strokeStyle = nav; c.lineWidth = 2.2; c.beginPath(); for (var x = 0; x <= W; x += 3){ var k = ((x / period) + ph) % 1, e = (k * k) * (H * .3) * (Math.floor(x / period + ph) % 2 ? 1 : -1); x ? c.lineTo(x, cy + e) : c.moveTo(x, cy + e); } c.stroke(); c.lineWidth = 1;
    for (var f = 0; f < 4; f++){ var fx = (f - ph) * period; if (fx < 0 || fx > W) continue; c.fillStyle = tim; c.beginPath(); c.arc(fx, cy, 4.5, 0, 6.283); c.fill(); lab(c, 'fix', fx + 6, cy + 14, tim); } }
  addLoop($('#slC'), drawSlot);
  themeHooks.push(function(){ draw(); drawMask(); bars_(); }); addEventListener('resize', function(){ draw(); drawMask(); }); drawMask();
});

/* =========================================================== PLAYGROUND */
var PG = { file: S.file.replace('.toml', ''), toml: S.toml, thr: S.threshold_ns, seed: S.seed, dur: 7200, stale: false, real: true, recomputed: false };
var POP = [['clock-holdover', 'var(--tim)'], ['jamming-demo', 'var(--int)'], ['spoof-detect', 'var(--spf)'], ['small-uas-jammed-nav', 'var(--nav)'], ['integrity-raim', 'var(--itg)'], ['orbit-sgp4-gps', 'var(--orb)'], ['telecom-prtc-holdover-24h', 'var(--tim)'], ['moonlight-service-volume', 'var(--orb)']];
var sel = $('#pgSelect');
FILES.forEach(function(f){ var o = el('option', null, f + '.toml · ' + D.fileKind[f]); o.value = f; sel.appendChild(o); });
POP.forEach(function(p){ var b = el('button', 'sb'); b.type = 'button'; b.style.setProperty('--c', p[1]); b.dataset.f = p[0]; b.append(el('i'), document.createTextNode(p[0])); b.addEventListener('click', function(){ loadScenario(p[0]); }); $('#pgPopular').appendChild(b); });
function markSide(){ $$('#pgPopular .sb').forEach(function(b){ b.classList.toggle('on', b.dataset.f === PG.file); }); sel.value = PG.file; }
sel.addEventListener('change', function(){ loadScenario(sel.value); });
function hlToml(src){ return src.split('\n').map(function(line){ var h = esc(line); if (/^\s*#/.test(line)) return '<span class="c">' + h + '</span>'; if (/^\s*\[/.test(line)) return '<span class="h">' + h + '</span>'; return h.replace(/^(\s*)([\w.]+)(\s*=)/, '$1<span class="k">$2</span>$3').replace(/(&quot;|")([^"]*)(")/g, '<span class="s">$1$2$3</span>').replace(/(=\s*)(-?[\d.]+(?:e-?\d+)?)/g, '$1<span class="n">$2</span>').replace(/(#.*)$/, '<span class="c">$1</span>'); }).join('\n'); }
var tomlView = $('#tomlView');
function renderToml(){ setMarkup(tomlView, hlToml(PG.toml)); }
tomlView.addEventListener('input', function(){ PG.toml = tomlView.textContent; PG.stale = true; status(); });
$('#pgToml').addEventListener('click', function(){ var on = !tomlView.classList.contains('on'); tomlView.classList.toggle('on', on); this.setAttribute('aria-pressed', on); this.classList.toggle('on', on); });
function patchScalar(src, key, val){ var re = new RegExp('^(\\s*' + key + '\\s*=\\s*)([^#\\n]*)', 'm'); return re.test(src) ? src.replace(re, function(_, a){ return a + val; }) : src; }
function status(){ var st = $('#pgStat'), prov = $('#pgProv');
  if (!PG.real){ st.textContent = PG.file + '.toml · runs in the live playground'; prov.className = 'illus'; prov.textContent = 'Illustrative in this concept'; }
  else if (PG.stale){ st.textContent = 'scenario changed · press Run'; prov.className = 'illus'; prov.textContent = 'Stale result'; }
  else { st.textContent = 'seed ' + PG.seed + ' · threshold ' + PG.thr + ' ns · ' + S.file; prov.className = PG.recomputed ? 'illus' : 'real'; prov.textContent = PG.recomputed ? 'Recomputed in the page from committed series' : 'Committed engine output'; } }
function knob(id, out, fmtf, key, tomlKey){ var i = $(id), o = $(out); var upd = function(){ i.style.setProperty('--v', ((i.value - i.min) / (i.max - i.min) * 100) + '%'); o.textContent = fmtf(i.value); }; upd(); i.addEventListener('input', function(){ upd(); PG[key] = +i.value; PG.toml = patchScalar(PG.toml, tomlKey, key === 'seed' ? i.value : (+i.value).toFixed(1)); renderToml(); PG.stale = true; status(); }); }
knob('#kSeed', '#oSeed', function(v){ return v; }, 'seed', 'seed'); knob('#kThr', '#oThr', function(v){ return v + ' ns'; }, 'thr', 'threshold_ns'); knob('#kDur', '#oDur', function(v){ return v + ' s'; }, 'dur', 'duration_s');
function derive(side, thr){ var ser = side.series, out = 600, inSpec = 0, hold = null; for (var i = 0; i < ser.length; i++){ var t = ser[i][0], e = Math.abs(ser[i][1]); if (e <= thr) inSpec++; if (t >= out && hold === null && e > thr) hold = t - out; } return { avail: inSpec / ser.length, hold: hold === null ? (ser[ser.length - 1][0] - out) : hold }; }
function loadScenario(f){ if (!D.fileKind[f]) return; PG.file = f; PG.real = f === S.file.replace('.toml', ''); PG.stale = false; PG.recomputed = false; $('#pgName').textContent = f + '.toml'; markSide();
  PG.toml = PG.real ? S.toml : '# ' + f + '.toml  (kind = "' + D.fileKind[f] + '")\n# ' + (D.fileDesc[f] || '') + '\n# This concept page carries one committed result: clock-holdover.\n# The live playground loads this file and runs it in your browser.\n'; renderToml(); status(); renderAll(); }
window.KSload = loadScenario;
$('#pgRun').addEventListener('click', function(){ var b = this; b.classList.add('on'); setTimeout(function(){ b.classList.remove('on'); }, 250);
  if (!PG.real){ toast('Runs in the live playground: this concept carries one committed result'); return; }
  PG.stale = false; PG.recomputed = PG.thr !== S.threshold_ns || PG.seed !== S.seed || PG.dur !== 7200; status(); renderAll();
  toast(PG.seed !== S.seed || PG.dur !== 7200 ? 'Seed and duration need the live engine; threshold recomputed here' : 'Run complete'); });
function tag(n, real){ n.className = real ? 'real' : 'illus'; n.textContent = real ? 'Real engine output' : 'Illustrative'; }
var curTab = 'sum';
$$('.rtabs button').forEach(function(b){ b.addEventListener('click', function(){ selectTab(b.dataset.p); }); b.addEventListener('keydown', function(e){ if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return; var bs = $$('.rtabs button'), i = (bs.indexOf(b) + (e.key === 'ArrowRight' ? 1 : -1) + bs.length) % bs.length; bs[i].focus(); selectTab(bs[i].dataset.p); }); });
function selectTab(p){ curTab = p; $$('.rtabs button').forEach(function(x){ x.setAttribute('aria-selected', x.dataset.p === p); x.tabIndex = x.dataset.p === p ? 0 : -1; }); $$('.rpane').forEach(function(x){ x.classList.toggle('on', x.dataset.p === p); }); renderTab(p); }
function sideFom(side){ var f = Object.assign({}, side.fom); if (PG.recomputed){ var d = derive(side, PG.thr); f.availability = d.avail; f.holdover_s = d.hold; } return f; }
function renderSummary(){ var q = sideFom(S.q), c = sideFom(S.c), thr = PG.recomputed ? PG.thr : S.threshold_ns;
  if (!PG.real){ $('#sumLine').textContent = PG.file + '.toml is a "' + D.fileKind[PG.file] + '" scenario. ' + (D.fileDesc[PG.file] || '') + ' In the live playground it runs here, in your browser, and this tab shows its one-line summary.'; tag($('#sumTag'), false); $('#sumKv').textContent = ''; $('#sumNote').textContent = 'This concept page ships one committed result so its numbers stay real: clock-holdover.'; return; }
  tag($('#sumTag'), !PG.recomputed);
  $('#sumLine').textContent = 'Through a ' + ((7200 - 600) / 60).toFixed(0) + '-minute GNSS outage, the ' + S.c.id + ' chip-scale atomic clock stays within ' + thr + ' ns for ' + Math.round(c.holdover_s) + ' s; the ' + S.q.id + ' optical clock for ' + Math.round(q.holdover_s) + ' s.';
  var kv = $('#sumKv'); kv.textContent = '';
  [['Holdover · CSAC', Math.round(c.holdover_s) + ' s'], ['Holdover · optical', Math.round(q.holdover_s) + ' s'], ['Timing p95 · CSAC', fmt(c.timing_p95_ns) + ' ns'], ['Availability · CSAC', (c.availability * 100).toFixed(1) + ' %']].forEach(function(r){ var d = el('div'); d.append(el('span', null, r[0]), el('b', null, r[1])); kv.appendChild(d); });
  $('#sumNote').textContent = 'Scenario ' + S.file + ' · seed ' + S.seed + ' · engine ' + S.engine + ' · scenario hash ' + S.hash + '…' + (PG.recomputed ? ' · holdover and availability recomputed in the page from the committed series at ' + PG.thr + ' ns (approximate, illustrative).' : ' · read from the committed result file.'); }
var FOM_ORDER = ['holdover_s', 'timing_rms_ns', 'timing_p95_ns', 'availability', 'integrity', 'security', 'resilience_slope_ns_per_s'];
function renderFom(){ var t = $('#fomT'), q = sideFom(S.q), c = sideFom(S.c); t.textContent = ''; var hd = el('tr'); ['Figure', 'CSAC (classical)', 'Optical (quantum)', 'Unit', 'Tier'].forEach(function(h){ hd.appendChild(el('th', null, h)); }); t.appendChild(hd);
  FOM_ORDER.forEach(function(k){ var u = S.units[k] || ['', '', '']; var tr = el('tr'), td = el('td'); td.append(document.createTextNode(k)); td.appendChild(el('small', null, u[1])); tr.appendChild(td); tr.appendChild(el('td', 'num', fmt(c[k], 3))); tr.appendChild(el('td', 'num', fmt(q[k], 3))); tr.appendChild(el('td', 'num', u[0])); var tc = el('td'); tc.appendChild(el('span', 'tier', 'Modelled')); tr.appendChild(tc); t.appendChild(tr); }); }
function axes(W, H, L, R, T, B){ return { x: function(f){ return L + f * (W - L - R); }, y: function(f){ return H - B - f * (H - T - B); } }; }
function drawTs(){ var G = fit($('#tsC')); if (!G) return; var c = G.c, W = G.w, H = G.h, L = 48, R = 14, T = 14, B = 30, A = axes(W, H, L, R, T, B), ink3 = cssv('--ink-3'), line = cssv('--line'), tim = cssv('--tim'), itg = cssv('--itg'), int = cssv('--int'), thr = PG.recomputed ? PG.thr : S.threshold_ns, ymax = 30;
  c.font = '10px "Geist Mono", monospace'; c.fillStyle = cssv('--bg-2'); c.fillRect(A.x(600 / 7200), T, A.x(1) - A.x(600 / 7200), H - T - B); c.fillStyle = ink3; c.textAlign = 'left'; c.fillText('GNSS denied', A.x(600 / 7200) + 6, T + 12); c.fillText('GNSS', A.x(0) + 4, T + 12);
  for (var v = -30; v <= 30; v += 10){ var y = A.y((v + ymax) / (2 * ymax)); c.strokeStyle = line; c.beginPath(); c.moveTo(L, y); c.lineTo(W - R, y); c.stroke(); c.fillStyle = ink3; c.textAlign = 'right'; c.fillText(v + ' ns', L - 6, y + 3); }
  [0, 1800, 3600, 5400, 7200].forEach(function(s, i){ c.textAlign = i === 0 ? 'left' : i === 4 ? 'right' : 'center'; c.fillText(s + ' s', A.x(s / 7200), H - 10); });
  [thr, -thr].forEach(function(z){ var yy = A.y((z + ymax) / (2 * ymax)); c.strokeStyle = int; c.setLineDash([5, 4]); c.beginPath(); c.moveTo(L, yy); c.lineTo(W - R, yy); c.stroke(); c.setLineDash([]); }); c.fillStyle = int; c.textAlign = 'right'; c.fillText('± threshold ' + thr + ' ns', W - R - 4, A.y((thr + ymax) / (2 * ymax)) - 6);
  [[S.c, tim, 2.2], [S.q, itg, 2]].forEach(function(s){ c.beginPath(); s[0].series.forEach(function(p, i){ var X = A.x(p[0] / 7200), Y = A.y((clamp(p[1], -ymax, ymax) + ymax) / (2 * ymax)); i ? c.lineTo(X, Y) : c.moveTo(X, Y); }); c.strokeStyle = s[1]; c.lineWidth = s[2]; c.stroke(); c.lineWidth = 1; });
  c.textAlign = 'left'; c.fillStyle = tim; c.fillText('■ ' + S.c.id + ' (CSAC)', L + 8, H - B - 26); c.fillStyle = itg; c.fillText('■ ' + S.q.id + ' · error below 0.0002 ns, flat at this scale', L + 8, H - B - 12); }
function drawAdev(){ var G = fit($('#adevC')); if (!G) return; var c = G.c, W = G.w, H = G.h, L = 58, R = 14, T = 14, B = 30, A = axes(W, H, L, R, T, B), ink3 = cssv('--ink-3'), line = cssv('--line'), tim = cssv('--tim'), itg = cssv('--itg');
  var lx0 = 1, lx1 = Math.log10(4000), ly0 = -17, ly1 = -9, X = function(t){ return A.x((Math.log10(t) - lx0) / (lx1 - lx0)); }, Y = function(v){ return A.y((Math.log10(v) - ly0) / (ly1 - ly0)); };
  c.font = '10px "Geist Mono", monospace'; for (var e = ly0; e <= ly1; e++){ c.strokeStyle = line; c.beginPath(); c.moveTo(L, Y(Math.pow(10, e))); c.lineTo(W - R, Y(Math.pow(10, e))); c.stroke(); c.fillStyle = ink3; c.textAlign = 'right'; c.fillText('1e' + e, L - 6, Y(Math.pow(10, e)) + 3); }
  [10, 100, 1000].forEach(function(t){ c.textAlign = 'center'; c.fillText('τ = ' + t + ' s', X(t), H - 10); });
  [[S.c, tim], [S.q, itg]].forEach(function(s){ c.beginPath(); s[0].adev.forEach(function(p, i){ i ? c.lineTo(X(p[0]), Y(p[1])) : c.moveTo(X(p[0]), Y(p[1])); }); c.strokeStyle = s[1]; c.lineWidth = 2; c.stroke(); c.lineWidth = 1;
    s[0].adev.forEach(function(p){ if (p[2] && p[3]){ c.strokeStyle = s[1]; c.globalAlpha = .5; c.beginPath(); c.moveTo(X(p[0]), Y(p[2])); c.lineTo(X(p[0]), Y(p[3])); c.stroke(); c.globalAlpha = 1; } c.fillStyle = s[1]; c.beginPath(); c.arc(X(p[0]), Y(p[1]), 3.5, 0, 6.283); c.fill(); }); });
  c.textAlign = 'left'; c.fillStyle = tim; c.fillText('■ ' + S.c.id, L + 8, T + 12); c.fillStyle = itg; c.fillText('■ ' + S.q.id, L + 8, T + 26); c.fillStyle = ink3; c.fillText('whiskers: 95% confidence interval', L + 8, T + 40); }
function drawHealth(){ var G = fit($('#healthC')); if (!G) return; var c = G.c, W = G.w, H = G.h, ink3 = cssv('--ink-3'), line2 = cssv('--line-2'), itg = cssv('--itg'), tim = cssv('--tim'), bg2 = cssv('--bg-2');
  var rows = [['NIS · ' + S.c.id, S.c.health.nis_mean, S.c.health.nis_chi2_lower_95, S.c.health.nis_chi2_upper_95, tim], ['NIS · ' + S.q.id, S.q.health.nis_mean, S.q.health.nis_chi2_lower_95, S.q.health.nis_chi2_upper_95, itg], ['NEES · ' + S.c.id, S.c.health.nees_mean, S.c.health.nees_chi2_lower_95, S.c.health.nees_chi2_upper_95, tim], ['NEES · ' + S.q.id, S.q.health.nees_mean, S.q.health.nees_chi2_lower_95, S.q.health.nees_chi2_upper_95, itg]];
  var L = Math.min(210, W * .42), R = 70, x = function(v){ return L + (v - .5) / 2.5 * (W - L - R); }, rh = (H - 30) / rows.length; c.font = '11px "Geist Mono", monospace';
  [1, 2].forEach(function(v){ c.strokeStyle = line2; c.setLineDash([3, 4]); c.beginPath(); c.moveTo(x(v), 4); c.lineTo(x(v), H - 22); c.stroke(); c.setLineDash([]); c.fillStyle = ink3; c.textAlign = 'center'; c.fillText(String(v), x(v), H - 8); });
  rows.forEach(function(r, i){ var y = 10 + i * rh + rh / 2; c.fillStyle = ink3; c.textAlign = 'left'; c.fillText(r[0], 0, y + 4); c.fillStyle = bg2; c.fillRect(x(r[2]), y - 7, x(r[3]) - x(r[2]), 14); c.strokeStyle = line2; c.strokeRect(x(r[2]), y - 7, x(r[3]) - x(r[2]), 14); var inb = r[1] >= r[2] && r[1] <= r[3]; c.fillStyle = r[4]; c.beginPath(); c.arc(x(r[1]), y, 5, 0, 6.283); c.fill(); c.fillStyle = inb ? itg : cssv('--int'); c.textAlign = 'left'; c.fillText(r[1].toFixed(3) + (inb ? ' ✓' : ' ✗'), W - R + 8, y + 4); }); }
var orbRot = { a: .6, b: .35, drag: false };
function drawOrbit(t){ var G = fit($('#orbC')); if (!G) return; var c = G.c, W = G.w, H = G.h, cx = W / 2, cy = H / 2, Rk = Math.min(W, H) * .16, ink3 = cssv('--ink-3'), tim = cssv('--tim'), orb = cssv('--orb'), line2 = cssv('--line-2');
  var a = orbRot.a + (RM || orbRot.drag ? 0 : t * .15), b = orbRot.b;
  function P(x, y, z){ var X = x * Math.cos(a) - z * Math.sin(a), Z = x * Math.sin(a) + z * Math.cos(a), Y = y * Math.cos(b) - Z * Math.sin(b), Z2 = y * Math.sin(b) + Z * Math.cos(b); return [cx + X * Rk, cy - Y * Rk, Z2]; }
  var gr = c.createRadialGradient(cx - Rk * .35, cy - Rk * .35, Rk * .1, cx, cy, Rk); gr.addColorStop(0, cssv('--surface-2')); gr.addColorStop(1, ink3); c.fillStyle = gr; c.beginPath(); c.arc(cx, cy, Rk, 0, 6.283); c.fill();
  c.strokeStyle = line2; for (var la = -60; la <= 60; la += 30){ c.beginPath(); for (var lo = 0; lo <= 360; lo += 6){ var r = la * Math.PI / 180, q = lo * Math.PI / 180, p = P(Math.cos(r) * Math.cos(q), Math.sin(r), Math.cos(r) * Math.sin(q)); if (p[2] < 0){ c.moveTo(p[0], p[1]); continue; } c.lineTo(p[0], p[1]); } c.stroke(); }
  var e = .72, sma = 3.4, inc = 63.4 * Math.PI / 180, pts = [], i; for (i = 0; i <= 240; i++){ var E = i / 240 * 6.283, xo = sma * (Math.cos(E) - e), yo = sma * Math.sqrt(1 - e * e) * Math.sin(E); pts.push(P(xo, yo * Math.sin(inc), yo * Math.cos(inc))); }
  c.lineWidth = 2; for (i = 1; i < pts.length; i++){ if (pts[i][2] < 0 && Math.hypot(pts[i][0] - cx, pts[i][1] - cy) < Rk) continue; c.strokeStyle = tim; c.globalAlpha = pts[i][2] < 0 ? .4 : 1; c.beginPath(); c.moveTo(pts[i - 1][0], pts[i - 1][1]); c.lineTo(pts[i][0], pts[i][1]); c.stroke(); } c.globalAlpha = 1; c.lineWidth = 1;
  var per = pts[0], apo = pts[120]; c.fillStyle = orb; [per, apo].forEach(function(p){ c.beginPath(); c.arc(p[0], p[1], 4, 0, 6.283); c.fill(); }); lab(c, 'perigee', per[0] + 8, per[1] - 8, cssv('--ink-2')); lab(c, 'apogee', apo[0] + 8, apo[1] - 8, cssv('--ink-2'));
  var M = (RM ? 1 : t * .4) % 6.283, E2 = M; for (var k = 0; k < 6; k++) E2 = E2 - (E2 - e * Math.sin(E2) - M) / (1 - e * Math.cos(E2)); var xs = sma * (Math.cos(E2) - e), ys = sma * Math.sqrt(1 - e * e) * Math.sin(E2), s = P(xs, ys * Math.sin(inc), ys * Math.cos(inc)); c.fillStyle = cssv('--ink'); c.beginPath(); c.arc(s[0], s[1], 5, 0, 6.283); c.fill();
  lab(c, 'Molniya-type orbit · 63.4° inclination', 10, H - 14, ink3); }
(function(){ var cv = $('#orbC'), sx, sy, a0, b0; cv.addEventListener('pointerdown', function(e){ orbRot.drag = true; sx = e.clientX; sy = e.clientY; a0 = orbRot.a; b0 = orbRot.b; cv.setPointerCapture(e.pointerId); }); cv.addEventListener('pointermove', function(e){ if (!orbRot.drag) return; orbRot.a = a0 + (e.clientX - sx) / 120; orbRot.b = clamp(b0 + (e.clientY - sy) / 160, -1.2, 1.2); }); cv.addEventListener('pointerup', function(){ orbRot.drag = false; }); })();
function renderCmp(){ var v = $('#cmpV'), q = sideFom(S.q), c = sideFom(S.c); v.textContent = ''; var better = { holdover_s: 1, availability: 1, integrity: 1, security: 1, timing_rms_ns: -1, timing_p95_ns: -1, resilience_slope_ns_per_s: -1 };
  [['A · ' + S.c.id, c, 'var(--tim)', q], ['B · ' + S.q.id, q, 'var(--itg)', c]].forEach(function(col){ var d = el('div', 'col'), h = el('h5'); h.style.setProperty('--c', col[2]); h.append(el('i'), document.createTextNode(col[0])); d.appendChild(h);
    FOM_ORDER.forEach(function(k){ var r = el('div', 'row'), b = el('b', null, fmt(col[1][k], 3) + ' ' + (S.units[k] ? S.units[k][0] : '')); if (better[k] > 0 ? col[1][k] > col[3][k] : col[1][k] < col[3][k]) b.classList.add('win'); r.append(el('span', null, k), b); d.appendChild(r); }); v.appendChild(d); }); }
function drawSweep(){ var G = fit($('#swC')); if (!G) return; var c = G.c, W = G.w, H = G.h, L = 56, R = 14, T = 14, B = 30, A = axes(W, H, L, R, T, B), ink3 = cssv('--ink-3'), line = cssv('--line'), tim = cssv('--tim'), key = $('#swKey').value, yk = $('#swY').value, a = +$('#swA').value, b = +$('#swB').value, n = clamp(+$('#swN').value | 0, 2, 40);
  var xs = [], ys = [], i; for (i = 0; i < n; i++){ var xv = a + (b - a) * i / (n - 1); xs.push(xv); var val; if (key === 'thr'){ var d = derive(S.c, Math.max(.1, xv)); val = yk === 'hold' ? d.hold : d.avail; } else { var d0 = derive(S.c, S.threshold_ns); val = (yk === 'hold' ? d0.hold : d0.avail) * (1 + .06 * Math.sin(xv * 2.3) + .03 * Math.sin(xv * 7.1)); } ys.push(val); }
  var ymin = Math.min.apply(null, ys), ymax = Math.max.apply(null, ys); if (ymax - ymin < 1e-9){ ymax += 1; ymin -= 1; } var pad = (ymax - ymin) * .12; ymin -= pad; ymax += pad;
  c.font = '10px "Geist Mono", monospace'; for (var g = 0; g <= 4; g++){ var yv = ymin + (ymax - ymin) * g / 4, y = A.y(g / 4); c.strokeStyle = line; c.beginPath(); c.moveTo(L, y); c.lineTo(W - R, y); c.stroke(); c.fillStyle = ink3; c.textAlign = 'right'; c.fillText(yk === 'hold' ? Math.round(yv) + ' s' : (yv * 100).toFixed(1) + '%', L - 6, y + 3); }
  c.textAlign = 'center'; [0, .5, 1].forEach(function(f){ c.fillText((a + (b - a) * f).toFixed(key === 'seed' ? 0 : 1), A.x(f), H - 10); });
  c.beginPath(); xs.forEach(function(xv, i){ var X = A.x(i / (n - 1)), Y = A.y((ys[i] - ymin) / (ymax - ymin)); i ? c.lineTo(X, Y) : c.moveTo(X, Y); }); c.strokeStyle = tim; c.lineWidth = 2.2; c.stroke(); c.lineWidth = 1;
  xs.forEach(function(xv, i){ c.fillStyle = tim; c.beginPath(); c.arc(A.x(i / (n - 1)), A.y((ys[i] - ymin) / (ymax - ymin)), 3.5, 0, 6.283); c.fill(); });
  c.fillStyle = ink3; c.textAlign = 'left'; c.fillText(S.c.id + ' · ' + (key === 'thr' ? 'computed from the committed series' : 'stand-in: each seed needs a live engine run'), L + 8, T + 12); }
$('#swGo').addEventListener('click', drawSweep); ['#swKey', '#swY'].forEach(function(s){ $(s).addEventListener('change', drawSweep); });
function jsonHl(o){ return esc(JSON.stringify(o, null, 2)).replace(/("(?:[^"\\]|\\.)*")(\s*:)?/g, function(m, s, colon){ return colon ? '<span class="k">' + s + '</span>' + colon : '<span class="s">' + s + '</span>'; }).replace(/(:\s|\[\s*|,\s*)(-?\d+\.?\d*(?:e[-+]?\d+)?)\b/g, '$1<span class="n">$2</span>'); }
function resultObj(){ var side = function(s){ return { spec: { id: s.id }, fom: sideFom(s), filter_health: s.health, adev_curve: s.adev.slice(0, 3).map(function(p){ return { tau_s: p[0], adev: p[1] }; }).concat(['… ' + (s.adev.length - 3) + ' more']), series: s.series.slice(0, 2).map(function(p){ return { t: p[0], error_ns: p[1] }; }).concat(['… ' + (s.series.length - 2) + ' more (every 8th sample kept on this page)']) }; };
  return { engine_version: S.engine, scenario: S.file, seed: S.seed, threshold_ns: PG.recomputed ? PG.thr : S.threshold_ns, classical: side(S.c), quantum: side(S.q), units: '… ' + Object.keys(S.units).length + ' figure-of-merit fields in the Figures of merit tab, each with unit, note and provenance' }; }
function renderJson(){ var j = $('#jsonV'); if (PG.real) setMarkup(j, jsonHl(resultObj())); else j.textContent = '{ "note": "' + PG.file + '.toml runs in the live playground; this concept carries one committed result." }'; }
$('#jsonCopy').addEventListener('click', function(){ copyText(JSON.stringify(resultObj(), null, 2), this, 'JSON copied'); });
function b64(s){ var bytes = new TextEncoder().encode(s), bin = ''; bytes.forEach(function(x){ bin += String.fromCharCode(x); }); return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, ''); }
function shareUrl(){ return 'https://kshana.dev/#s=' + b64(PG.toml); }
function renderEmbed(){ var u = shareUrl(); $('#shareSnip').textContent = u.length > 140 ? u.slice(0, 137) + '…' : u; $('#embedSnip').textContent = '<iframe src="https://kshana.dev/?embed=1&scenario=' + PG.file + '.toml&seed=' + PG.seed + '&tab=fom" width="100%" height="720" title="Kshana playground"></iframe>'; }
$('#pgShare').addEventListener('click', function(){ copyText(shareUrl(), this, 'Share link copied: it carries the whole scenario'); });
$$('[data-dl]').forEach(function(b){ b.addEventListener('click', function(){ var k = b.dataset.dl, base = 'kshana-' + PG.file;
  if (k === 'json') download(base + '.result.json', new Blob([JSON.stringify(resultObj(), null, 2)], { type: 'application/json' }));
  else if (k === 'report'){ var q = sideFom(S.q), c = sideFom(S.c), rows = FOM_ORDER.map(function(f){ return '<tr><td>' + f + '</td><td>' + fmt(c[f], 3) + '</td><td>' + fmt(q[f], 3) + '</td><td>' + esc(S.units[f] ? S.units[f][0] : '') + '</td><td>Modelled</td></tr>'; }).join('');
    download(base + '.report.html', new Blob(['<!doctype html><meta charset="utf-8"><title>Kshana report · ' + S.file + '</title><body style="font:15px system-ui;max-width:760px;margin:40px auto;padding:0 16px"><h1>Kshana report · ' + S.file + '</h1><p>Engine ' + S.engine + ' · seed ' + S.seed + ' · scenario hash ' + S.hash + '…</p><p>' + esc($('#sumLine').textContent) + '</p><table border="1" cellpadding="6" style="border-collapse:collapse"><tr><th>Figure</th><th>' + S.c.id + '</th><th>' + S.q.id + '</th><th>Unit</th><th>Tier</th></tr>' + rows + '</table><h2>Scenario</h2><pre>' + esc(S.toml) + '</pre><p>Generated from the committed result on the Kshana concept page.</p></body>'], { type: 'text/html' })); }
  else { var cv = k === 'adev' ? $('#adevC') : $('#tsC'); if (!cv.getBoundingClientRect().width) selectTab(k === 'adev' ? 'adev' : 'ts'); (k === 'adev' ? drawAdev : drawTs)(); cv.toBlob(function(bl){ if (bl) download(base + '-' + k + '.png', bl); }); } }); });
function renderTab(p){ if (p === 'sum') renderSummary(); else if (p === 'fom') renderFom(); else if (p === 'ts') drawTs(); else if (p === 'adev') drawAdev(); else if (p === 'health') drawHealth(); else if (p === 'cmp') renderCmp(); else if (p === 'sweep') drawSweep(); else if (p === 'json') renderJson(); else if (p === 'embed') renderEmbed(); }
function renderAll(){ renderTab(curTab); if (curTab !== 'sum') renderSummary(); }
addLoop($('#win'), function(t){ if (curTab === 'orbit') drawOrbit(t); });
themeHooks.push(function(){ renderTab(curTab); });
renderToml(); markSide(); status(); renderSummary(); selectTab('sum');
var TOUR = [['#pgSelect', 'Pick a scenario', 'Any of the ' + D.nFiles + ' bundled files, from jamming to lunar navigation. The popular ones sit underneath.'], ['.knobs', 'Tune the knobs', 'Seed, threshold and duration patch the scenario file. Everything stays reproducible.'], ['#pgToml', 'Or edit the TOML', 'Open the full scenario definition and change anything.'], ['#pgRun', 'Run it', 'The WebAssembly engine runs on your machine. Nothing is uploaded.'], ['.rtabs', 'Read the results', 'Summary, figures of merit with their tier, time series, stability, filter health, orbit, compare, sweep, JSON and exports.'], ['#pgShare', 'Share it', 'The link carries the whole scenario, so anyone can reproduce the run.']];
var tourI = -1, tip = $('#tourTip');
function tourStep(){ $$('.tour-ring').forEach(function(x){ x.classList.remove('tour-ring'); }); if (tourI < 0 || tourI >= TOUR.length){ tip.classList.remove('on'); tourI = -1; return; }
  var s = TOUR[tourI], target = $(s[0]); if (!target || !target.offsetParent) target = $('#pgName'); target.classList.add('tour-ring'); $('#ttT').textContent = s[1]; $('#ttB').textContent = s[2]; $('#ttN').textContent = (tourI + 1) + ' / ' + TOUR.length; $('#ttNext').textContent = tourI === TOUR.length - 1 ? 'Done' : 'Next';
  var hr = $('.pg-main').getBoundingClientRect(), tr = target.getBoundingClientRect(); tip.classList.add('on'); tip.style.left = clamp(tr.left - hr.left, 8, Math.max(8, hr.width - 290)) + 'px'; tip.style.top = (tr.bottom - hr.top + 10) + 'px'; }
$('#pgTour').addEventListener('click', function(){ tourI = 0; tourStep(); });
$('#ttNext').addEventListener('click', function(){ tourI++; tourStep(); });
$('#ttSkip').addEventListener('click', function(){ tourI = -1; tourStep(); });

/* =========================================================== PROOF */
(function(){ var sm = D.summary, C = 2 * Math.PI * 170, gap = 12, acc = 0, segs = [[sm.validated, '#arcV'], [sm.modelled, '#arcM'], [sm.partner_owned, '#arcP']];
  var arcs = segs.map(function(s){ var len = Math.max(1, s[0] / sm.total * C - gap), off = acc; acc += s[0] / sm.total * C; var e = $(s[1]); e.setAttribute('stroke-dasharray', len + ' ' + C); e.style.strokeDashoffset = -off; return { el: e, len: len }; });
  if (RM || !('IntersectionObserver' in window)) return; var n = $('#ringN'), done = false;
  arcs.forEach(function(a){ a.el.setAttribute('stroke-dasharray', '0 ' + C); }); n.textContent = '0';
  new IntersectionObserver(function(es, o){ es.forEach(function(e){ if (!e.isIntersecting || done) return; done = true; o.disconnect(); arcs.forEach(function(a, i){ var t0 = performance.now() + i * 260; (function tick(now){ var k = clamp((now - t0) / 1800, 0, 1); a.el.setAttribute('stroke-dasharray', (a.len * (1 - Math.pow(1 - k, 4))) + ' ' + C); if (k < 1) requestAnimationFrame(tick); })(performance.now()); }); var t1 = performance.now(); (function cnt(now){ var k = clamp((now - t1) / 1800, 0, 1); n.textContent = k < 1 ? Math.round(sm.validated * (1 - Math.pow(2, -10 * k))) : sm.validated; if (k < 1) requestAnimationFrame(cnt); })(t1); }); }, { threshold: .35 }).observe($('#ring')); })();
var STN = { v: 'Validated', m: 'Modelled', p: 'Partner-owned' };
lazy($('#proof'), function(){ var cells = $('#cells'), tipEl = $('#cellTip');
  D.ledger.forEach(function(r, i){ var b = el('button', r[2]); b.type = 'button'; b.dataset.i = i; b.setAttribute('aria-label', r[0] + ': ' + STN[r[2]]); cells.appendChild(b); });
  function show(i){ var r = D.ledger[i]; tipEl.replaceChildren(el('b', null, r[0]), document.createTextNode(' · ' + STN[r[2]] + '. ' + r[1] + (r[3] ? ' Oracle: ' + r[3] : ''))); }
  cells.addEventListener('mouseover', function(e){ var b = e.target.closest('button'); if (b) show(+b.dataset.i); });
  cells.addEventListener('focusin', function(e){ var b = e.target.closest('button'); if (b) show(+b.dataset.i); });
  cells.addEventListener('click', function(e){ var b = e.target.closest('button'); if (!b) return; $('#lgQ').value = D.ledger[+b.dataset.i][0]; setLg(''); scroll('#ledger'); });
  $$('#lgd button').forEach(function(b){ b.addEventListener('click', function(){ var on = b.getAttribute('aria-pressed') !== 'true'; $$('#lgd button').forEach(function(x){ x.setAttribute('aria-pressed', 'false'); }); b.setAttribute('aria-pressed', on); cells.classList.toggle('dim', on); $$('#cells button').forEach(function(c){ c.classList.toggle('hit', on && c.classList.contains(b.dataset.f)); }); }); });
  if (useST) gsap.from('#cells button', { scrollTrigger: { trigger: '#cells', start: 'top 85%' }, scale: 0, opacity: 0, duration: .5, ease: 'back.out(2)', stagger: { amount: 1.1, from: 'random' } });
  renderLedger(); });
var lgS = '', lgBody = $('#lgBody');
function setLg(s){ lgS = s; $$('#lgSeg button').forEach(function(b){ b.setAttribute('aria-pressed', b.dataset.s === s); }); renderLedger(); }
function renderLedger(){ var q = $('#lgQ').value.trim().toLowerCase(), n = 0, frag = document.createDocumentFragment();
  D.ledger.forEach(function(r){ if (lgS && r[2] !== lgS) return; if (q && (r[0] + ' ' + r[1] + ' ' + r[3] + ' ' + r[4] + ' ' + r[5]).toLowerCase().indexOf(q) < 0) return; n++; var tr = el('tr'); tr.appendChild(el('td', null, r[0])); tr.appendChild(el('td', null, r[1])); var st = el('td'); st.appendChild(el('span', 'st ' + r[2], STN[r[2]])); tr.appendChild(st); tr.appendChild(el('td', null, r[3] || '—')); var md = el('td'); md.appendChild(el('code', null, r[4] + (r[5] ? ' · ' + r[5] : ''))); tr.appendChild(md); frag.appendChild(tr); });
  lgBody.replaceChildren(frag); $('#lgCount').textContent = n + ' of ' + D.ledger.length + ' rows shown'; }
$$('#lgSeg button').forEach(function(b){ b.addEventListener('click', function(){ setLg(b.dataset.s); }); });
$('#lgQ').addEventListener('input', renderLedger);

/* =========================================================== BENTO */
var NS = 'http://www.w3.org/2000/svg';
function sv(tag, attrs, parent){ var e = document.createElementNS(NS, tag); for (var k in attrs) e.setAttribute(k, attrs[k]); if (parent) parent.appendChild(e); return e; }
function tx(parent, x, y, s, attrs){ var a = { x: x, y: y, 'font-size': 10, 'font-family': 'Geist Mono, monospace' }; for (var k in attrs) a[k] = attrs[k]; var t = sv('text', a, parent); t.textContent = s; return t; }
function lenOf(p, fb){ try { return Math.ceil(p.getTotalLength()); } catch (e) { return fb; } }
var lunarSats = [];
function drawBento(){
  (function(){ var s = $('#vHold'); s.replaceChildren(); var W = 700, H = 300, L = 40, R = 10, T = 10, B = 26, x = function(h){ return L + (h / 24) * (W - L - R); }, y = function(ns){ return H - B - (Math.min(ns, 200) / 200) * (H - T - B); };
    var g = sv('g', { class: 'gridl' }, s); [50, 100, 150, 200].forEach(function(v){ sv('line', { x1: L, x2: W - R, y1: y(v), y2: y(v) }, g); });
    var ax = sv('g', { class: 'ax' }, s); sv('line', { x1: L, x2: W - R, y1: H - B, y2: H - B }, ax); [0, 6, 12, 18, 24].forEach(function(h){ tx(ax, x(h), H - 8, h + ' h', { 'text-anchor': 'middle' }); }); [0, 100, 200].forEach(function(v){ tx(ax, L - 8, y(v) + 3, String(v), { 'text-anchor': 'end' }); }); tx(ax, L - 8, T, 'ns', { 'text-anchor': 'end' });
    sv('rect', { x: L, y: y(200), width: W - L - R, height: y(100) - y(200), fill: 'var(--int)', opacity: .05 }, s); sv('line', { x1: L, x2: W - R, y1: y(100), y2: y(100), stroke: 'var(--ink)', 'stroke-width': 1.2, 'stroke-dasharray': '5 5', opacity: .7 }, s); tx(s, W - R, y(100) - 8, 'guard 100 ns', { 'text-anchor': 'end', 'font-size': 11, fill: 'var(--ink-2)' });
    var f = function(k, h){ return k * h * h * .6 + k * h * 1.2; };
    [['var(--int)', 95], ['var(--nav)', 6.5], ['var(--spf)', 2.4], ['var(--tim)', .55], ['var(--ink)', .12]].forEach(function(cl, ci){ var d = ''; for (var i = 0; i <= 160; i++){ var h = i / 160 * 24, v = f(cl[1], h); d += (i ? 'L' : 'M') + x(h).toFixed(1) + ' ' + y(v).toFixed(1); if (v > 200) break; }
      var p = sv('path', { d: d, fill: 'none', stroke: cl[0], 'stroke-width': ci === 4 ? 2.4 : 2, 'stroke-linecap': 'round', class: 'draw' }, s); p.style.setProperty('--len', lenOf(p, 1000)); p.style.transitionDelay = (ci * 140) + 'ms';
      for (var j = 0; j <= 960; j++){ var hh = j / 960 * 24; if (f(cl[1], hh) >= 100){ sv('circle', { cx: x(hh), cy: y(100), r: 4.5, fill: 'var(--surface)', stroke: cl[0], 'stroke-width': 2 }, s); break; } } }); })();
  (function(){ var s = $('#vRaim'); s.replaceChildren(); var cx = 150, cy = 76; sv('line', { x1: cx - 90, x2: cx + 90, y1: cy, y2: cy, stroke: 'var(--line-2)' }, s); sv('line', { x1: cx, x2: cx, y1: cy - 72, y2: cy + 72, stroke: 'var(--line-2)' }, s);
    sv('circle', { cx: cx, cy: cy, r: 66, fill: 'none', stroke: 'var(--int)', 'stroke-width': 1.4, 'stroke-dasharray': '4 4' }, s); sv('circle', { cx: cx, cy: cy, r: 40, fill: 'var(--itg)', 'fill-opacity': .1, stroke: 'var(--itg)', 'stroke-width': 2, id: 'raimPL' }, s); sv('circle', { cx: cx + 9, cy: cy - 6, r: 4, fill: 'var(--ink)', id: 'raimE' }, s);
    tx(s, cx + 56, cy - 58, 'alert limit', { fill: 'var(--int)' }); tx(s, cx - 150, cy + 62, 'protection level', { fill: 'var(--itg)' }); sv('line', { x1: cx - 70, y1: cy + 54, x2: cx - 30, y2: cy + 27, stroke: 'var(--itg)', 'stroke-width': 1 }, s); tx(s, cx + 20, cy + 8, 'error', { fill: 'var(--ink-2)' }); })();
  (function(){ var s = $('#vMtie'); s.replaceChildren(); var W = 300, H = 150, L = 8, B = 16, x = function(lt){ return L + (lt + 1) / 5 * (W - L - 4); }, y = function(lv){ return H - B - (lv / 4) * (H - B - 6); };
    var ax = sv('g', { class: 'ax' }, s); sv('line', { x1: L, x2: W, y1: H - B, y2: H - B }, ax); ['0.1 s', '1', '10', '100', '1000', '10⁴ s'].forEach(function(t, i){ tx(ax, L + i * (W - L - 4) / 5, H - 3, t, { 'text-anchor': i === 0 ? 'start' : i === 5 ? 'end' : 'middle' }); });
    var d = ''; [[-1, 1.6], [0.2, 1.6], [1.2, 2.5], [2.2, 3.2], [4, 3.2]].forEach(function(p, i){ d += (i ? 'L' : 'M') + x(p[0]) + ' ' + y(p[1]); }); sv('path', { d: d, fill: 'none', stroke: 'var(--int)', 'stroke-width': 1.6, 'stroke-dasharray': '5 3' }, s);
    var dm = ''; for (var i = 0; i <= 60; i++){ var lt = -1 + i / 60 * 5, lv = .55 + .55 * (lt + 1) + .08 * Math.sin(i * 1.3) - .06 * Math.pow(Math.max(0, lt - 2), 2); dm += (i ? 'L' : 'M') + x(lt).toFixed(1) + ' ' + y(Math.min(lv, 2.9)).toFixed(1); }
    var p = sv('path', { d: dm, fill: 'none', stroke: 'var(--tim)', 'stroke-width': 2.2, 'stroke-linecap': 'round', class: 'draw' }, s); p.style.setProperty('--len', lenOf(p, 600)); tx(s, x(3.9), y(3.2) - 7, 'ITU-T mask', { 'text-anchor': 'end', fill: 'var(--int)' }); tx(s, x(3.9), y(2.1) + 16, 'measured MTIE', { 'text-anchor': 'end', fill: 'var(--tim)' }); })();
  (function(){ var s = $('#vIns'); s.replaceChildren(); var H = 150, x0 = 90, x1 = 330, yc = 70; sv('rect', { x: x0, y: 4, width: x1 - x0, height: H - 26, fill: 'var(--nav)', opacity: .08, rx: 8 }, s); tx(s, x0 + 6, 16, 'GNSS denied', { fill: 'var(--nav)' });
    var up = '', dn = '', up2 = '', dn2 = ''; for (var X = x0; X <= x1; X += 4){ var k = (X - x0) / (x1 - x0), w = Math.pow(k, 1.5) * 52, w2 = Math.min(w, 8 + 10 * k); up += (X === x0 ? 'M' : 'L') + X + ' ' + (yc - w); dn = 'L' + X + ' ' + (yc + w) + dn; up2 += (X === x0 ? 'M' : 'L') + X + ' ' + (yc - w2); dn2 = 'L' + X + ' ' + (yc + w2) + dn2; }
    sv('path', { d: up + dn + 'Z', fill: 'var(--nav)', opacity: .18 }, s); sv('path', { d: up2 + dn2 + 'Z', fill: 'var(--itg)', opacity: .35 }, s);
    var p = sv('path', { d: 'M10 ' + yc + ' L' + x0 + ' ' + yc + ' C 200 ' + (yc - 6) + ', 260 ' + (yc - 30) + ', ' + x1 + ' ' + (yc - 46) + ' L390 ' + yc, fill: 'none', stroke: 'var(--nav)', 'stroke-width': 2.2, class: 'draw' }, s); p.style.setProperty('--len', lenOf(p, 500));
    sv('path', { d: 'M10 ' + yc + ' L390 ' + yc, fill: 'none', stroke: 'var(--ink)', 'stroke-width': 1.2, 'stroke-dasharray': '3 4', opacity: .6 }, s); tx(s, x1 - 4, yc - 58, 'inertial only', { 'text-anchor': 'end', fill: 'var(--nav)' }); tx(s, x1 - 4, yc + 30, 'with map matching', { 'text-anchor': 'end', fill: 'var(--itg)' }); tx(s, 10, H - 6, 'true track (dashed)', { fill: 'var(--ink-3)' }); })();
  (function(){ var s = $('#vLunar'); s.replaceChildren(); var cx = 200, cy = 90, defs = sv('defs', {}, s), rg = sv('radialGradient', { id: 'moonG', cx: '38%', cy: '35%', r: '70%' }, defs); sv('stop', { offset: '0%', 'stop-color': 'var(--surface-2)' }, rg); sv('stop', { offset: '100%', 'stop-color': 'var(--ink-3)' }, rg);
    sv('ellipse', { cx: cx, cy: cy, rx: 170, ry: 52, fill: 'none', stroke: 'var(--line-2)', transform: 'rotate(-12 ' + cx + ' ' + cy + ')' }, s); sv('ellipse', { cx: cx, cy: cy, rx: 120, ry: 74, fill: 'none', stroke: 'var(--orb)', 'stroke-opacity': .55, 'stroke-width': 1.4, transform: 'rotate(18 ' + cx + ' ' + cy + ')' }, s); sv('circle', { cx: cx, cy: cy, r: 40, fill: 'url(#moonG)' }, s);
    lunarSats = []; [0, 2.1, 4.2].forEach(function(ph){ lunarSats.push({ ph: ph, halo: sv('circle', { r: 10, fill: 'var(--orb)', opacity: .15 }, s), el: sv('circle', { r: 4.5, fill: 'var(--orb)' }, s) }); }); lunarSats.push({ ph: 1, outer: true, halo: sv('circle', { r: 8, fill: 'var(--spf)', opacity: .15 }, s), el: sv('circle', { r: 3.5, fill: 'var(--spf)' }, s) });
    tx(s, 4, 166, 'relay orbit · Moon-centred frame', { fill: 'var(--ink-3)' }); placeLunar(0); })();
}
function placeLunar(t){ lunarSats.forEach(function(o){ var a = o.ph + t * (o.outer ? .18 : .42), rx = o.outer ? 170 : 120, ry = o.outer ? 52 : 74, rot = (o.outer ? -12 : 18) * Math.PI / 180, ex = rx * Math.cos(a), ey = ry * Math.sin(a), X = 200 + ex * Math.cos(rot) - ey * Math.sin(rot), Y = 90 + ex * Math.sin(rot) + ey * Math.cos(rot), behind = Math.sin(a) < 0 && Math.hypot(X - 200, Y - 90) < 42; [o.el, o.halo].forEach(function(e){ e.setAttribute('cx', X.toFixed(1)); e.setAttribute('cy', Y.toFixed(1)); e.style.visibility = behind ? 'hidden' : 'visible'; }); }); }
lazy($('#capabilities'), function(){ drawBento(); D.standards.forEach(function(s){ $('#fmtChips').appendChild(el('span', s[2] ? 'p' : '', s[0])); });
  addLoop($('#capabilities'), function(t){ if (RM) return; placeLunar(t); var pl = $('#raimPL'), e = $('#raimE'); if (pl){ pl.setAttribute('r', (40 + 4 * Math.sin(t * 1.3)).toFixed(2)); e.setAttribute('cx', (159 + 6 * Math.sin(t * .9)).toFixed(1)); e.setAttribute('cy', (70 + 5 * Math.cos(t * 1.1)).toFixed(1)); } }); renderEx(); });

/* explorer */
var FAM = { 'Orbits & space': ['Orbit & geometry', 'Numerical propagator', 'Maneuvers & trajectory design', 'Orbit determination', 'Observability & estimation theory', 'Mission analysis', 'Space environment'], 'Lunar & deep space': ['Lunar & cislunar', 'Deep-space & Mars PNT'], 'Navigation': ['Inertial', 'Alt-PNT (GPS-denied)', 'Fusion', 'GNSS positioning', 'GNSS measurement domain', 'Optical / RF PNT'], 'Timing': ['Time systems', 'Clock & timing', 'Timing integrity'], 'Integrity & threats': ['Integrity', 'Augmentation (SBAS)', 'Resilience', 'Nav-signal performance', 'Passive RF geolocation', 'Integrity impact', 'Resilience assessment'], 'Assurance & decisions': ['Verification & assurance', 'AI/ML evaluation & trade', 'Decision support', 'Interoperability', 'Artifact interchange'] };
var FCOL = { 'Orbits & space': 'var(--orb)', 'Lunar & deep space': 'var(--spf)', 'Navigation': 'var(--nav)', 'Timing': 'var(--tim)', 'Integrity & threats': 'var(--int)', 'Assurance & decisions': 'var(--itg)', 'More': 'var(--ink-3)' };
function famOf(d){ for (var k in FAM) if (FAM[k].indexOf(d) >= 0) return k; return 'More'; }
var exF = 'All', exAll = false;
function renderEx(){ var g = $('#exG'); g.textContent = ''; var list = D.cards.filter(function(c){ return exF === 'All' || famOf(c[0]) === exF; }), shown = exAll || exF !== 'All' ? list : list.slice(0, 9);
  shown.forEach(function(c){ var a = el('article', 'cap'), top = el('div', 'top'), fam = famOf(c[0]), dm = el('span', null, c[0]); dm.style.color = FCOL[fam]; top.append(dm, el('span', 'tier' + (c[1] === 'validated' ? ' v' : ''), c[1] === 'validated' ? 'Validated' : 'Modelled')); a.append(top, el('h4', null, c[2]), el('p', null, c[3])); if (c[4]) a.appendChild(el('div', 'pf', '✓ ' + c[4]));
    var runs = el('div', 'runs'); if (c[5]){ runs.appendChild(el('code', null, c[5])); var b = el('button', null, 'Run ▸'); b.type = 'button'; b.addEventListener('click', function(){ loadScenario(c[5].replace('.toml', '')); scroll('#playground'); }); runs.appendChild(b); } else runs.appendChild(el('code', null, 'engine API · no bundled scenario')); a.appendChild(runs); g.appendChild(a); });
  if (!exAll && exF === 'All' && list.length > 9){ var more = el('button', 'btn btn-ghost', 'Show all ' + list.length + ' capabilities'); more.type = 'button'; more.style.gridColumn = '1 / -1'; more.style.justifySelf = 'center'; more.addEventListener('click', function(){ exAll = true; renderEx(); }); g.appendChild(more); } }
['All'].concat(Object.keys(FAM)).forEach(function(k){ var n = k === 'All' ? D.cards.length : D.cards.filter(function(c){ return famOf(c[0]) === k; }).length; if (!n) return; var b = el('button', null, k + ' · ' + n); b.type = 'button'; b.setAttribute('aria-pressed', k === 'All'); b.addEventListener('click', function(){ exF = k; $$('#exF button').forEach(function(x){ x.setAttribute('aria-pressed', x === b); }); renderEx(); }); $('#exF').appendChild(b); });

/* standards */
D.standards.forEach(function(s){ var d = el('div', 'std'); d.append(el('b', null, s[0]), el('p', null, s[1]), el('span', s[2] ? 'real' : 'illus', s[2] ? 'Checked' : 'Implemented')); $('#stdG').appendChild(d); });
(function(){ var n = D.standards.length % 4; if (n){ var d = el('div', 'std note'); d.style.gridColumn = 'span ' + (4 - n); d.append(el('b', null, 'Plus'), el('p', null, 'CCSDS OEM, OMM and SP3 export from the engine and from three of the MCP server\'s seven tools, and the Kshana Interchange Format for cross-provider lunar budgets.')); $('#stdG').appendChild(d); } })();

/* =========================================================== CAPABILITY ATLAS */
var GROUPS = [['Earth GNSS and interference', '--int', ['jamming', 'tracking-loop', 'gnss-sim', 'pvt', 'spoof', 'spoof-detect', 'impairment-eval']], ['Integrity and ARAIM', '--itg', ['integrity', 'araim-reference-check']], ['Alternative PNT and fusion', '--nav', ['gravity-map', 'terrain-nav', 'terrain-slam', 'combined-altpnt', 'inertial', 'gnss-ins', 'ins-trn-coast', 'fusion', 'hybrid', 'hybrid-ukf', 'hybrid-optical-rf']], ['Clocks and timing', '--tim', ['clock', 'sweep', 'slot-timing', 'telecom-timing', 'timetransfer']], ['Orbits and mission analysis', '--orb', ['orbit', 'ephemeris', 'oem-interop', 'passes', 'launch-window', 'reentry', 'eo-coverage', 'attitude-budget', 'space-packet', 'space-weather', 'link-budget', 'aperture-duty-cycle']], ['Lunar and cislunar', '--spf', ['lunar-integrity', 'lunar-time-offset', 'lunar-vlbi', 'lunar-joint-od-clock', 'lunar-frame-realisation', 'moonlight-service-volume', 'lunar-differential-pnt', 'lunar-interop-export', 'lunar-time-budget', 'cislunar-observability', 'lunar-attack-surface', 'realtime-frame-eop', 'lunar-jamming', 'lunar-vlbi-fim', 'lunar-frame-campaign', 'lunar-llr-datum', 'lunar-beacon', 'earth-gnss-lunar', 'cislunar-arc-recovery']], ['Mars PNT', '--int', ['mars-pnt']], ['Quantum sensors and trade studies', '--spf', ['quantum-anomaly-detect', 'quantum-gnss-free-nav', 'quantum-time-transfer', 'quantum-trade', 'conflict-resilience', 'sweep-nd']]];
lazy($('#atlas'), function(){ var map = $('#atMap'), det = $('#atDetail'), placed = {};
  function statusOf(k){ var r = D.kindLedger[k]; if (!r) return 'u'; return r[0] > 0 ? 'v' : 'm'; }
  GROUPS.forEach(function(g){ var files = FILES.filter(function(f){ return g[2].indexOf(D.fileKind[f]) >= 0; }); if (!files.length) return; var box = el('div', 'at-grp'); box.style.setProperty('--c', 'var(' + g[1] + ')'); var h = el('h4'); h.append(document.createTextNode(g[0]), el('span', null, String(files.length))); box.appendChild(h); var nodes = el('div', 'at-nodes');
    files.forEach(function(f){ placed[f] = 1; var b = el('button', 'at-node'); b.type = 'button'; b.dataset.f = f; b.setAttribute('aria-pressed', 'false'); b.append(el('i', statusOf(D.fileKind[f])), document.createTextNode(f)); b.addEventListener('click', function(){ pick(f, b); }); nodes.appendChild(b); }); box.appendChild(nodes); map.appendChild(box); });
  var rest = FILES.filter(function(f){ return !placed[f]; }); if (rest.length){ var box = el('div', 'at-grp'); box.style.setProperty('--c', 'var(--ink-3)'); box.appendChild(el('h4', null, 'Other')); var nodes = el('div', 'at-nodes'); rest.forEach(function(f){ var b = el('button', 'at-node'); b.type = 'button'; b.dataset.f = f; b.append(el('i', statusOf(D.fileKind[f])), document.createTextNode(f)); b.addEventListener('click', function(){ pick(f, b); }); nodes.appendChild(b); }); box.appendChild(nodes); map.appendChild(box); }
  function pick(f, b){ $$('.at-node', map).forEach(function(x){ x.setAttribute('aria-pressed', x === b); }); var k = D.fileKind[f], r = D.kindLedger[k], kd = D.kinds.filter(function(x){ return x[0] === k; })[0];
    var p = el('div'); p.append(el('b', null, f + '.toml'), document.createTextNode(' · kind ' + k + '. ' + (D.fileDesc[f] || '') + ' ' + (kd ? kd[1] : '')));
    var st = el('div'); st.style.cssText = 'margin-top:6px;font-size:13px;color:var(--ink-3)'; st.textContent = r ? 'Ledger, module ' + k.replace(/-/g, '_') + ': ' + r[0] + ' of ' + r[1] + ' rows validated.' : 'No ledger module matches this kind by name; search the ledger for its capability.'; p.appendChild(st);
    var acts = el('div', 'acts'), a1 = el('button', 'tbtn run', 'Load in the playground'); a1.type = 'button'; a1.addEventListener('click', function(){ loadScenario(f); scroll('#playground'); });
    var a2 = el('a', 'tbtn', 'Run on kshana.dev (embed)'); a2.href = 'https://kshana.dev/?embed=1&scenario=' + f + '.toml'; var a3 = el('a', 'tbtn', 'Open in the new playground'); a3.href = '../2026-09-27-playground/index.html?scenario=' + f + '.toml'; acts.append(a1, a2, a3); p.appendChild(acts); det.replaceChildren(p); }
  $('#atQ').addEventListener('input', function(){ var q = this.value.trim().toLowerCase(); $$('.at-node', map).forEach(function(b){ var f = b.dataset.f; b.hidden = !!q && (f + ' ' + D.fileKind[f] + ' ' + (D.fileDesc[f] || '')).toLowerCase().indexOf(q) < 0; }); }); });

/* gallery */
$$('.gt[data-run]').forEach(function(b){ b.addEventListener('click', function(){ loadScenario(b.dataset.run); scroll('#playground'); }); if (b.hasAttribute('data-nav')) b.classList.add('nav-t'); });

/* =========================================================== DEVELOPERS */
var CODE = {
  cli: ['terminal · command line', '<span class="p">$</span> cargo install kshana\n<span class="p">$</span> kshana scenarios/clock-holdover.toml\n<span class="c"># writes clock-holdover.result.json, .chart.svg and .report.html</span>\n<span class="p">$</span> kshana --validate scenarios/integrity-raim.toml\n<span class="p">$</span> kshana scenarios/orbit-sgp4-gps.toml --export-sp3 gps.sp3\n<span class="p">$</span> kshana --study scenarios/quantum-pnt-demonstrator.suite.toml', 'cargo install kshana'],
  rust: ['main.rs · Rust crate', '<span class="p">$</span> cargo add kshana\n\n<span class="k">let</span> toml = std::fs::read_to_string(<span class="s">"scenarios/clock-holdover.toml"</span>)?;\n<span class="k">let</span> out = kshana::api::<span class="f">run_toml</span>(&amp;toml)?;\nprintln!(<span class="s">"{}"</span>, out.summary);   <span class="c">// also out.json, out.svg, out.csv</span>', 'cargo add kshana'],
  py: ['python · PyPI', '<span class="p">$</span> pip install kshana\n\n<span class="k">import</span> json, kshana\nresult = json.loads(kshana.<span class="f">run</span>(open(<span class="s">"scenarios/clock-holdover.toml"</span>).read()))\nprint(result[<span class="s">"quantum"</span>][<span class="s">"fom"</span>][<span class="s">"integrity"</span>])\n\n<span class="c"># json, svg and a one-line summary at once</span>\nresult_json, chart_svg, summary = kshana.<span class="f">run_full</span>(toml_text)', 'pip install kshana'],
  js: ['javascript · npm', '<span class="p">$</span> npm install kshana\n\n<span class="k">import</span> init, { run, summary, version } <span class="k">from</span> <span class="s">"kshana"</span>;\n<span class="k">await</span> <span class="f">init</span>();   <span class="c">// fetch and compile the WebAssembly binary</span>\n<span class="k">const</span> result = JSON.parse(<span class="f">run</span>(tomlText));\nconsole.log(<span class="f">version</span>(), result.classical.fom.timing_p95_ns);', 'npm install kshana'],
  mcp: ['mcp · AI assistants', '<span class="p">$</span> cargo install kshana-mcp\n\n<span class="c">// then register it in your client\'s mcpServers config</span>\n{\n  <span class="s">"mcpServers"</span>: {\n    <span class="s">"kshana"</span>: { <span class="s">"command"</span>: <span class="s">"kshana-mcp"</span> }\n  }\n}', 'cargo install kshana-mcp'],
  docker: ['container · ghcr.io', '<span class="p">$</span> docker run --rm -i ghcr.io/ashfordeou/kshana-mcp\n\n<span class="c"># the same MCP server, multi-architecture, no Rust toolchain needed</span>', 'docker run --rm -i ghcr.io/ashfordeou/kshana-mcp']
};
var curDoor = 'cli';
function setDoor(k){ curDoor = k; $$('.door').forEach(function(d){ d.setAttribute('aria-selected', d.dataset.k === k); }); $('#termTitle').textContent = CODE[k][0]; setMarkup($('#termPre'), CODE[k][1]); }
$$('.door').forEach(function(d){ d.addEventListener('click', function(){ setDoor(d.dataset.k); }); });
$('#termCopy').addEventListener('click', function(){ copyText(CODE[curDoor][2], this, 'Copied: ' + CODE[curDoor][2]); });
setDoor('cli');
(function(){ var ms = $$('#chat .msg'), ran = false; function run(){ if (ran) return; ran = true; ms.forEach(function(m, i){ setTimeout(function(){ m.classList.add('show'); }, 250 + i * 850); }); }
  if (RM) ms.forEach(function(m){ m.classList.add('show'); }); else if ('IntersectionObserver' in window) new IntersectionObserver(function(es, o){ if (es[0].isIntersecting){ run(); o.disconnect(); } }, { threshold: .3 }).observe($('#chat')); else run(); })();

/* =========================================================== GLOSSARY */
var GLOSS = [['PNT', 'positioning, navigation and timing'], ['GNSS', 'Global Navigation Satellite System (GPS, Galileo, GLONASS, BeiDou)'], ['GPS', 'Global Positioning System'], ['C/N₀', 'carrier-to-noise density, in dB-Hz'], ['J/S', 'jammer-to-signal ratio'], ['RF', 'radio frequency'], ['L-band', 'the 1 to 2 GHz radio band GNSS uses'], ['C/A', 'coarse/acquisition code of GPS L1'], ['BOC', 'binary offset carrier modulation'], ['FDMA', 'frequency-division multiple access'], ['AFS', 'augmented forward signal (lunar)'], ['INS', 'inertial navigation system'], ['IMU', 'inertial measurement unit'], ['MEMS', 'micro-electromechanical systems'], ['UAS', 'uncrewed aircraft system'], ['RAIM', 'receiver autonomous integrity monitoring'], ['ARAIM', 'advanced RAIM'], ['PL · AL', 'protection level · alert limit'], ['APV-I', 'approach with vertical guidance, level I'], ['SBAS', 'satellite-based augmentation system'], ['DOP', 'dilution of precision'], ['SGP4', 'Simplified General Perturbations 4, an orbit model'], ['TLE', 'two-line element set'], ['LEO · GEO', 'low Earth orbit · geostationary orbit'], ['CCSDS', 'Consultative Committee for Space Data Systems'], ['OEM · OMM · TDM', 'CCSDS orbit ephemeris, orbit mean-elements and tracking data messages'], ['SP3', 'Standard Product 3, a precise-orbit format'], ['RINEX', 'Receiver Independent Exchange Format'], ['IGS', 'International GNSS Service'], ['IONEX', 'Ionosphere Map Exchange format'], ['IGRF', 'International Geomagnetic Reference Field'], ['DSN', 'Deep Space Network'], ['VLBI', 'very long baseline interferometry'], ['LLR', 'lunar laser ranging'], ['CSAC', 'chip-scale atomic clock'], ['TCXO · OCXO', 'temperature-compensated · oven-controlled crystal oscillator'], ['RAFS', 'rubidium atomic frequency standard'], ['PHM', 'passive hydrogen maser'], ['USO · DSAC', 'ultra-stable oscillator · deep space atomic clock'], ['MTIE · TDEV', 'maximum time interval error · time deviation'], ['PRTC · ePRTC', 'primary reference time clock · enhanced PRTC'], ['ADEV · σy(τ)', 'Allan deviation at averaging time τ'], ['ITU-T', 'International Telecommunication Union, Telecommunication Standardization Sector'], ['NIS · NEES', 'normalised innovation squared · normalised estimation error squared'], ['FoM', 'figure of merit'], ['NIST', 'National Institute of Standards and Technology'], ['ESA', 'European Space Agency'], ['AIAA', 'American Institute of Aeronautics and Astronautics'], ['IEEE', 'Institute of Electrical and Electronics Engineers'], ['MCP', 'Model Context Protocol'], ['WASM', 'WebAssembly'], ['TOML', 'Tom\'s Obvious, Minimal Language'], ['JSON · CSV · SVG', 'JavaScript Object Notation · comma-separated values · Scalable Vector Graphics'], ['CLI · API · IDE', 'command-line interface · application programming interface · integrated development environment'], ['CI', 'continuous integration'], ['SBOM · SLSA', 'software bill of materials · Supply-chain Levels for Software Artifacts'], ['AGPL', 'GNU Affero General Public License'], ['NDA', 'non-disclosure agreement'], ['DOI', 'Digital Object Identifier'], ['UTC', 'Coordinated Universal Time']];
GLOSS.forEach(function(g){ var d = el('div'); d.append(el('b', null, g[0]), document.createTextNode(g[1])); $('#gloss').appendChild(d); });

/* =========================================================== PALETTE */
var SECT = [['Hero', '#top'], ['Jam it: spectrum waterfall', '#jam'], ['Band atlas', '#bands-atlas'], ['Six ways GNSS fails', '#fails'], ['Missions by sector', '#missions'], ['Who it is for', '#who'], ['Mission console', '#timeline'], ['Earth to Moon to Mars', '#journey'], ['Clock ladder', '#clocks'], ['Timing wall', '#timing'], ['Playground', '#playground'], ['Verification matrix', '#proof'], ['Validation ledger', '#ledger'], ['Capabilities', '#capabilities'], ['Capability explorer', '#explorer'], ['Capability atlas', '#atlas'], ['Standards grid', '#standards'], ['Scenario gallery', '#scenarios'], ['Developers and install', '#developers'], ['Ways to use it', '#ways'], ['MCP server', '#mcp'], ['Editions: free and Pro', '#editions'], ['Cite and publications', '#cite'], ['Glossary', '#glossary']];
var ITEMS = [];
SECT.forEach(function(s){ ITEMS.push({ g: 'Sections', id: s[0], d: 'Jump to ' + s[0].toLowerCase(), k: 'go', to: s[1], c: 'var(--ink-3)' }); });
SECTORS.forEach(function(s, i){ ITEMS.push({ g: 'Sectors', id: s.n, d: s.q, k: 'sector', v: i, c: 'var(' + s.c + ')' }); });
[['Take the 60-second playground tour', 'tour'], ['Toggle colour theme', 'theme'], ['Request a Kshana Pro evaluation', 'mail', 'Kshana Pro evaluation'], ['Talk to us about a commercial licence', 'mail', 'Kshana commercial licence'], ['Request a study', 'mail', 'Kshana study request'], ['Copy: cargo install kshana', 'copy', 'cargo install kshana'], ['Copy: pip install kshana', 'copy', 'pip install kshana'], ['Copy: npm install kshana', 'copy', 'npm install kshana'], ['Copy: cargo install kshana-mcp', 'copy', 'cargo install kshana-mcp'], ['Copy BibTeX citation', 'bib']].forEach(function(a){ ITEMS.push({ g: 'Actions', id: a[0], d: a[1] === 'copy' ? 'Copy to clipboard' : a[1] === 'tour' ? 'Walk through the playground' : a[1] === 'bib' ? 'Cite the engine' : a[1] === 'mail' ? 'Email contact@ashforde.org' : 'Light or dark', k: a[1], v: a[2], c: 'var(--tim)' }); });
D.kinds.forEach(function(k){ ITEMS.push({ g: 'Scenario kinds (' + D.nKinds + ')', id: k[0], d: k[1], k: 'kind', c: 'var(--spf)' }); });
FILES.forEach(function(f){ ITEMS.push({ g: 'Bundled scenario files (' + D.nFiles + ')', id: f + '.toml', d: 'kind ' + D.fileKind[f] + ' · ' + (D.fileDesc[f] || ''), k: 'file', v: f, c: 'var(--itg)' }); });
var pal = $('#palette'), inp = $('#palInput'), list = $('#palList'), act = 0, shown = [], lastFocus = null;
function fuzzy(q, s){ var t = s.toLowerCase(), qi = 0, score = 0, prev = -2, idx = []; for (var i = 0; i < t.length && qi < q.length; i++){ if (t[i] === q[qi]){ idx.push(i); score += (i === prev + 1 ? 3 : 1) + (i === 0 || /[-\s:.]/.test(t[i - 1]) ? 2 : 0); prev = i; qi++; } } return qi === q.length ? { score: score, idx: idx } : null; }
function renderPal(){ var raw = inp.value.trim().toLowerCase(), q = raw.replace(/\s+/g, '');
  shown = ITEMS.map(function(it){ if (!q) return /^(Bundled|Scenario kinds)/.test(it.g) ? null : Object.assign({}, it, { f: { score: 0, idx: [] } }); var f = fuzzy(q, it.id); if (f) return Object.assign({}, it, { f: f }); return it.d.toLowerCase().indexOf(raw) >= 0 ? Object.assign({}, it, { f: { score: -1, idx: [] } }) : null; }).filter(Boolean);
  if (q) shown.sort(function(a, b){ return b.f.score - a.f.score; }); shown = shown.slice(0, 60); act = clamp(act, 0, Math.max(0, shown.length - 1)); list.textContent = '';
  $('#palCount').textContent = q ? shown.length + (shown.length === 60 ? '+' : '') + ' matches' : 'type to search ' + D.nKinds + ' kinds and ' + D.nFiles + ' files';
  if (!shown.length){ list.appendChild(el('div', 'pal-empty', 'Nothing matches. Try a scenario kind such as "spoof" or "lunar".')); return; }
  var g = ''; shown.forEach(function(it, i){ if (!q && it.g !== g){ g = it.g; list.appendChild(el('div', 'pal-group', g)); } var row = el('div', 'pal-item' + (i === act ? ' act' : '')); row.setAttribute('role', 'option'); row.id = 'pi-' + i; row.setAttribute('aria-selected', i === act); row.dataset.i = i; var ic = el('span', 'ic'); ic.style.setProperty('--c', it.c); ic.appendChild(el('i')); var nm = el('div', 'nm'); it.id.split('').forEach(function(ch, j){ nm.appendChild(it.f.idx.indexOf(j) >= 0 ? el('mark', null, ch) : document.createTextNode(ch)); }); var txt = el('span'); txt.style.minWidth = '0'; txt.append(nm, el('div', 'ds', it.d.length > 110 ? it.d.slice(0, 108) + '…' : it.d)); row.append(ic, txt, el('span', 'go', '↵ ' + (it.k === 'go' || it.k === 'sector' ? 'jump' : it.k === 'file' ? 'load' : it.k === 'kind' ? 'show' : 'run'))); list.appendChild(row); });
  inp.setAttribute('aria-activedescendant', 'pi-' + act); var a = $('.pal-item.act', list); if (a) a.scrollIntoView({ block: 'nearest' }); }
function openPal(){ lastFocus = document.activeElement; pal.classList.add('open'); inp.value = ''; act = 0; renderPal(); setTimeout(function(){ inp.focus(); }, 10); }
function closePal(){ pal.classList.remove('open'); if (lastFocus && lastFocus.focus) lastFocus.focus(); }
function choose(it){ closePal(); if (!it) return;
  if (it.k === 'go') scroll(it.to); else if (it.k === 'copy') copyText(it.v, null, 'Copied: ' + it.v); else if (it.k === 'theme') toggleTheme(); else if (it.k === 'bib') copyText($('#bibText').textContent, null, 'BibTeX copied');
  else if (it.k === 'mail') location.href = 'mailto:contact@ashforde.org?subject=' + encodeURIComponent(it.v);
  else if (it.k === 'sector'){ scroll('#missions'); setTimeout(function(){ if (window.KSsector) window.KSsector(it.v); }, 50); }
  else if (it.k === 'tour'){ scroll('#playground'); setTimeout(function(){ tourI = 0; tourStep(); }, RM ? 0 : 700); }
  else if (it.k === 'kind'){ var f = FILES.filter(function(x){ return D.fileKind[x] === it.id; })[0]; if (f){ loadScenario(f); scroll('#playground'); } }
  else if (it.k === 'file'){ loadScenario(it.v); scroll('#playground'); } }
$('#kbtn').addEventListener('click', openPal); $$('[data-open-pal]').forEach(function(b){ b.addEventListener('click', openPal); });
inp.addEventListener('input', function(){ act = 0; renderPal(); });
list.addEventListener('click', function(e){ var r = e.target.closest('.pal-item'); if (r) choose(shown[+r.dataset.i]); });
pal.addEventListener('mousedown', function(e){ if (e.target === pal) closePal(); });
document.addEventListener('keydown', function(e){
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k'){ e.preventDefault(); pal.classList.contains('open') ? closePal() : openPal(); return; }
  if (e.key === 'Escape' && tourI >= 0){ tourI = -1; tourStep(); }
  if (!pal.classList.contains('open')) return;
  if (e.key === 'Escape'){ e.preventDefault(); closePal(); }
  else if (e.key === 'ArrowDown'){ e.preventDefault(); if (shown.length){ act = (act + 1) % shown.length; renderPal(); } }
  else if (e.key === 'ArrowUp'){ e.preventDefault(); if (shown.length){ act = (act - 1 + shown.length) % shown.length; renderPal(); } }
  else if (e.key === 'Enter'){ e.preventDefault(); choose(shown[act]); }
  else if (e.key === 'Tab'){ e.preventDefault(); inp.focus(); } });
if (!/Mac|iPhone|iPad/.test(navigator.platform || '')) $$('.kmod').forEach(function(k){ k.textContent = 'Ctrl K'; });

/* =========================================================== GSAP MOMENTS */
if (useST){
  gsap.fromTo('#win', { rotateX: 20, y: 50, scale: .93 }, { rotateX: 0, y: 0, scale: 1, ease: 'none', scrollTrigger: { trigger: '#playground .win-stage', start: 'top 95%', end: 'top 30%', scrub: .7 } });
  gsap.fromTo('.ring svg', { rotate: -40 }, { rotate: 0, ease: 'none', scrollTrigger: { trigger: '#ring', start: 'top 90%', end: 'bottom 40%', scrub: 1 } });
  gsap.matchMedia().add('(min-width: 900px)', function(){ var sec = $('#fails'), track = $('#failsTrack'); sec.classList.add('is-pinned'); var dist = function(){ return Math.max(0, track.scrollWidth - innerWidth); };
    var tw = gsap.to(track, { x: function(){ return -dist(); }, ease: 'none', scrollTrigger: { trigger: '.fails-pin', start: 'top top', end: function(){ return '+=' + dist(); }, pin: true, scrub: .7, invalidateOnRefresh: true, anticipatePin: 1, onUpdate: function(s){ setProg(s.progress); } } });
    failsST = tw.scrollTrigger; return function(){ sec.classList.remove('is-pinned'); failsST = null; }; });
  gsap.fromTo('.bigword', { xPercent: -4 }, { xPercent: 2, ease: 'none', scrollTrigger: { trigger: 'footer', start: 'top bottom', end: 'bottom bottom', scrub: true } });
  if (document.fonts) document.fonts.ready.then(function(){ ScrollTrigger.refresh(); });
}
addEventListener('resize', function(){ clearTimeout(window._rz); window._rz = setTimeout(function(){ renderTab(curTab); }, 150); });
requestAnimationFrame(frame);
});
