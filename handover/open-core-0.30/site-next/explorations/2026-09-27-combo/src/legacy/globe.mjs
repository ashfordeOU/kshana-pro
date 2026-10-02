import * as THREE from 'https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.module.js';

const KS = window.KS;
const reduced = KS.reduced;
const D2R = Math.PI / 180;
const ss = (a, b, x) => { x = Math.min(1, Math.max(0, (x - a) / (b - a))); return x * x * (3 - 2 * x); };
const lerp = (a, b, t) => a + (b - a) * t;
const ll = (lat, lon, r = 1) => new THREE.Vector3(r * Math.cos(lat * D2R) * Math.sin(lon * D2R), r * Math.sin(lat * D2R), r * Math.cos(lat * D2R) * Math.cos(lon * D2R));
const fresnelVS = `varying vec3 vN; varying vec3 vV; void main(){ vec4 mv = modelViewMatrix*vec4(position,1.0); vN = normalize(normalMatrix*normal); vV = normalize(-mv.xyz); gl_Position = projectionMatrix*mv; }`;
function makeRenderer(canvas){ try { const r = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: 'high-performance' }); r.setPixelRatio(Math.min(2, window.devicePixelRatio || 1)); r.setClearColor(0x000000, 0); return r; } catch (e) { return null; } }
function onVisible(el, cb){ new IntersectionObserver(es => cb(es[0].isIntersecting), { rootMargin: '80px' }).observe(el); }
function planet(radius, c1, c2, rim){ return new THREE.Mesh(new THREE.SphereGeometry(radius, 64, 64), new THREE.ShaderMaterial({ uniforms: { uL: { value: new THREE.Vector3(-0.6, 0.5, 0.65).normalize() }, uA: { value: new THREE.Color(c1) }, uB: { value: new THREE.Color(c2) }, uR: { value: new THREE.Color(rim) } }, vertexShader: fresnelVS,
  fragmentShader: `uniform vec3 uL; uniform vec3 uA; uniform vec3 uB; uniform vec3 uR; varying vec3 vN; varying vec3 vV; void main(){ float l = max(dot(vN, normalize(uL)), 0.0); float f = pow(1.0 - max(dot(vN, vV), 0.0), 2.6); vec3 c = mix(uA, uB, l); c += uR * f * 0.6; gl_FragColor = vec4(c, 1.0); }` })); }
const glowVS = `attribute float size; attribute vec3 color; uniform float uPx; varying vec3 vC; varying float vK; void main(){ vec4 mv = modelViewMatrix*vec4(position,1.0); vC = color; vK = size; gl_PointSize = uPx * size * (4.4 / -mv.z); gl_Position = projectionMatrix*mv; }`;
const glowFS = `varying vec3 vC; varying float vK; void main(){ vec2 c = gl_PointCoord-0.5; float d = length(c)*2.0; if (d>1.0) discard; float core = smoothstep(0.34, 0.0, d); float halo = exp(-d*d*5.0)*0.55; vec3 col = mix(vC, vec3(1.0), core*0.75); gl_FragColor = vec4(col, (core + halo) * min(vK,1.0)); }`;
const glowMat = (px, dpr) => new THREE.ShaderMaterial({ uniforms: { uPx: { value: px * dpr } }, vertexShader: glowVS, fragmentShader: glowFS, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending });
const orbitPos = (r, inc, raan, u, out) => { const ci = Math.cos(inc), si = Math.sin(inc), cO = Math.cos(raan), sO = Math.sin(raan), cu = Math.cos(u), su = Math.sin(u); return out.set(r * (cO * cu - sO * su * ci), r * (su * si), r * (sO * cu + cO * su * ci)); };

/* ======================= HERO GLOBE (Aurora) ======================= */
(function hero(){
  const canvas = document.getElementById('globe'), stageEl = document.getElementById('stage');
  const renderer = makeRenderer(canvas); if (!renderer) return;
  const DPR = Math.min(2, window.devicePixelRatio || 1);
  const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(34, 1, 0.1, 100); camera.position.set(0, 0, 4.4);
  const world = new THREE.Group(); scene.add(world); const tilt = new THREE.Group(); world.add(tilt); const spin = new THREE.Group(); tilt.add(spin);
  spin.add(planet(1, '#03040a', '#0d1320', '#3a64a8'));
  const LAND = [
    [[-168,66],[-162,70],[-140,70],[-125,70],[-110,73],[-95,72],[-85,70],[-80,64],[-94,59],[-90,57],[-82,53],[-78,58],[-72,61],[-64,59],[-60,54],[-56,52],[-66,45],[-70,43],[-76,38],[-76,35],[-81,31],[-80,26],[-82,26],[-84,30],[-90,29],[-97,27],[-97,22],[-92,18],[-87,21],[-88,15],[-83,10],[-79,9],[-78,7],[-80,7],[-85,11],[-92,14],[-105,20],[-110,24],[-112,29],[-115,30],[-117,33],[-121,35],[-124,40],[-124,46],[-123,49],[-130,54],[-135,58],[-145,60],[-152,58],[-158,56],[-165,55],[-160,59],[-166,62]],
    [[-120,69],[-80,70],[-70,78],[-88,82],[-120,77]],
    [[-55,60],[-45,60],[-40,65],[-22,70],[-20,76],[-18,81],[-35,83],[-60,82],[-70,78],[-58,75],[-55,70],[-52,65]],
    [[-80,8],[-72,12],[-62,10],[-52,5],[-50,0],[-44,-2],[-35,-5],[-35,-9],[-39,-15],[-40,-22],[-48,-26],[-53,-34],[-58,-38],[-62,-40],[-65,-45],[-68,-50],[-70,-55],[-74,-52],[-74,-45],[-73,-37],[-71,-30],[-70,-18],[-76,-14],[-81,-6],[-80,-1],[-78,2]],
    [[-9,37],[-9,43],[-1,46],[-4,48],[2,51],[8,54],[10,57],[5,58],[5,62],[14,67],[20,70],[28,71],[40,68],[55,69],[70,73],[80,73],[100,77],[112,74],[130,72],[140,72],[160,70],[180,68],[180,64],[170,60],[162,58],[156,51],[160,61],[150,59],[140,54],[137,47],[130,42],[127,39],[126,35],[121,31],[122,28],[117,23],[110,21],[108,17],[109,12],[105,9],[103,11],[100,13],[101,4],[104,1],[98,8],[98,16],[94,17],[92,22],[88,21],[80,15],[77,8],[73,17],[70,22],[67,25],[62,25],[57,26],[56,24],[59,22],[52,16],[45,13],[43,15],[35,28],[33,31],[35,36],[27,37],[26,40],[23,38],[20,40],[19,42],[14,45],[12,44],[18,40],[16,38],[13,41],[9,44],[3,43],[-1,37],[-5,36]],
    [[-17,21],[-17,15],[-12,8],[-8,4],[-2,5],[5,6],[9,4],[10,-2],[13,-6],[12,-13],[14,-22],[16,-29],[18,-34],[22,-34],[27,-33],[32,-28],[35,-24],[36,-18],[40,-15],[40,-10],[39,-5],[42,0],[48,5],[51,11],[44,11],[43,13],[39,18],[37,22],[35,28],[32,31],[25,32],[20,31],[20,33],[11,33],[10,37],[3,37],[-2,35],[-6,36],[-10,30],[-13,27]],
    [[44,-25],[47,-25],[50,-15],[49,-12],[44,-17]],
    [[114,-22],[114,-34],[118,-35],[124,-33],[131,-31],[138,-35],[141,-38],[147,-38],[150,-37],[153,-31],[153,-25],[146,-19],[145,-15],[142,-11],[141,-17],[136,-12],[132,-11],[127,-14],[122,-18]],
    [[109,-1],[110,2],[116,7],[119,5],[117,1],[116,-4],[111,-3]],[[95,5],[98,4],[104,-2],[106,-6],[101,-3]],[[105,-6],[114,-7],[114,-8.5],[106,-7.5]],[[131,-1],[138,-2],[146,-6],[150,-10],[142,-9],[138,-8],[132,-4]],
    [[130,31],[132,34],[135,35],[140,36],[141,40],[141,45],[145,44],[142,42],[141,38],[140,35],[136,33]],[[-5,50],[1,51],[2,53],[-2,56],[-3,58.5],[-6,58.5],[-5,55],[-3,54],[-5,52]],[[-10,51.5],[-6,52],[-6,55],[-8,55.3]],
    [[172,-34],[178,-38],[174,-41],[171,-44],[167,-46],[170,-41]],[[-24,63.5],[-14,64],[-14,66.3],[-22,66.5]],[[-85,22],[-74,20],[-78,22.5]],[[120,18],[122,18],[124,12],[126,7],[122,7],[120,14]],[[80,6],[82,7],[80,10]],[[12,77],[28,80],[20,80.5],[10,79.5]]
  ].map(p => { let a = 180, b = -180, c = 90, d = -90; p.forEach(([x, y]) => { a = Math.min(a, x); b = Math.max(b, x); c = Math.min(c, y); d = Math.max(d, y); }); return { p, box: [a, b, c, d] }; });
  const inPoly = (x, y, poly) => { let ins = false; for (let i = 0, j = poly.length - 1; i < poly.length; j = i++){ const [xi, yi] = poly[i], [xj, yj] = poly[j]; if (((yi > y) !== (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi)) ins = !ins; } return ins; };
  const isLand = (lon, lat) => { if (lat < -67) return true; for (const L of LAND){ const b = L.box; if (lon < b[0] || lon > b[1] || lat < b[2] || lat > b[3]) continue; if (inPoly(lon, lat, L.p)) return true; } return false; };
  const N = 24000, pts = [], shade = [], GA = Math.PI * (3 - Math.sqrt(5));
  for (let i = 0; i < N; i++){ const y = 1 - (i + 0.5) * 2 / N, r = Math.sqrt(1 - y * y), th = i * GA, x = Math.cos(th) * r, z = Math.sin(th) * r; if (isLand(Math.atan2(x, z) / D2R, Math.asin(y) / D2R)){ pts.push(x * 1.004, y * 1.004, z * 1.004); shade.push(0.6 + 0.4 * Math.random()); } }
  const dotGeo = new THREE.BufferGeometry(); dotGeo.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3)); dotGeo.setAttribute('shade', new THREE.Float32BufferAttribute(shade, 1));
  const dotMat = new THREE.ShaderMaterial({ transparent: true, depthWrite: false, uniforms: { uSize: { value: 2.3 * DPR }, uScale: { value: 1 } },
    vertexShader: `attribute float shade; uniform float uSize; uniform float uScale; varying float vA; varying float vS; void main(){ vec4 mv = modelViewMatrix*vec4(position,1.0); vec3 n = normalize(normalMatrix*position); float facing = dot(n, normalize(-mv.xyz)); vA = smoothstep(0.0, 0.55, facing); vS = shade; gl_PointSize = uSize * uScale * (4.2 / -mv.z); gl_Position = projectionMatrix*mv; }`,
    fragmentShader: `varying float vA; varying float vS; void main(){ vec2 c = gl_PointCoord-0.5; float d = length(c); if (d>0.5) discard; float a = smoothstep(0.5, 0.2, d); gl_FragColor = vec4(mix(vec3(0.55,0.64,0.78), vec3(0.86,0.91,1.0), vS), a * vA * 0.9); }` });
  spin.add(new THREE.Points(dotGeo, dotMat));
  const gratPts = []; for (let lat = -60; lat <= 60; lat += 30) for (let lon = -180; lon < 180; lon += 3) gratPts.push(ll(lat, lon, 1.002), ll(lat, lon + 3, 1.002)); for (let lon = -180; lon < 180; lon += 30) for (let lat = -84; lat < 84; lat += 3) gratPts.push(ll(lat, lon, 1.002), ll(lat + 3, lon, 1.002));
  spin.add(new THREE.LineSegments(new THREE.BufferGeometry().setFromPoints(gratPts), new THREE.LineBasicMaterial({ color: 0x5a7bb0, transparent: true, opacity: 0.09, depthWrite: false })));
  tilt.add(new THREE.Mesh(new THREE.SphereGeometry(1.2, 64, 64), new THREE.ShaderMaterial({ side: THREE.BackSide, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, vertexShader: fresnelVS, fragmentShader: `varying vec3 vN; varying vec3 vV; void main(){ float i = pow(max(0.0, 0.72 + dot(vN, vV)), 5.0); gl_FragColor = vec4(vec3(0.42,0.66,1.0), 1.0) * i * 1.25; }` })));
  /* constellations: stylised radii, real inclinations and plane counts */
  const CONS = [{ color: new THREE.Color('#58a4ff'), r: 1.62, inc: 55, planes: 6, per: 4, speed: 0.070, raan0: 0.2 }, { color: new THREE.Color('#ae90ff'), r: 1.86, inc: 56, planes: 3, per: 8, speed: 0.058, raan0: 0.9 }, { color: new THREE.Color('#ff7d66'), r: 1.50, inc: 64.8, planes: 3, per: 8, speed: 0.076, raan0: 0.5 }, { color: new THREE.Color('#ffc247'), r: 1.74, inc: 55, planes: 3, per: 8, speed: 0.064, raan0: 1.4 }];
  const orbitGroup = new THREE.Group(); tilt.add(orbitGroup); const sats = [], orbitLines = [];
  CONS.forEach((c, ci) => { for (let p = 0; p < c.planes; p++){ const raan = c.raan0 + p * (2 * Math.PI / c.planes), inc = c.inc * D2R, ring = [], v = new THREE.Vector3(); for (let k = 0; k <= 200; k++) ring.push(orbitPos(c.r, inc, raan, k / 200 * Math.PI * 2, v).clone());
    const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(ring), new THREE.LineBasicMaterial({ color: c.color, transparent: true, opacity: 0.2, depthWrite: false, blending: THREE.AdditiveBlending })); orbitGroup.add(line); orbitLines.push(line);
    for (let s = 0; s < c.per; s++) sats.push({ ci, raan, inc, r: c.r, u0: s * (2 * Math.PI / c.per) + p * 0.5, speed: c.speed, seed: Math.random(), pos: new THREE.Vector3() }); } });
  const satGeo = new THREE.BufferGeometry(), satPos = new Float32Array(sats.length * 3), satCol = new Float32Array(sats.length * 3), satSize = new Float32Array(sats.length);
  sats.forEach((s, i) => { const c = CONS[s.ci].color; satCol.set([c.r, c.g, c.b], i * 3); satSize[i] = 1; });
  satGeo.setAttribute('position', new THREE.BufferAttribute(satPos, 3)); satGeo.setAttribute('color', new THREE.BufferAttribute(satCol, 3)); satGeo.setAttribute('size', new THREE.BufferAttribute(satSize, 1)); orbitGroup.add(new THREE.Points(satGeo, glowMat(17, DPR)));
  const RX = { lat: 48.1, lon: 11.6 }, rxLocal = ll(RX.lat, RX.lon, 1.006), rxGeo = new THREE.BufferGeometry();
  rxGeo.setAttribute('position', new THREE.Float32BufferAttribute([rxLocal.x, rxLocal.y, rxLocal.z], 3)); rxGeo.setAttribute('color', new THREE.Float32BufferAttribute([0.8, 1, 0.9], 3)); rxGeo.setAttribute('size', new THREE.Float32BufferAttribute([1.4], 1)); spin.add(new THREE.Points(rxGeo, glowMat(22, DPR)));
  const pulseMat = new THREE.MeshBasicMaterial({ color: 0x3ddc84, transparent: true, opacity: 0.6, side: THREE.DoubleSide, depthWrite: false, blending: THREE.AdditiveBlending });
  const pulse = new THREE.Mesh(new THREE.RingGeometry(0.9, 1.0, 64), pulseMat); pulse.position.copy(ll(RX.lat, RX.lon, 1.007)); pulse.lookAt(ll(RX.lat, RX.lon, 3)); spin.add(pulse); const pulse2 = pulse.clone(); pulse2.material = pulseMat.clone(); spin.add(pulse2);
  const MAXL = 48, linkPos = new Float32Array(MAXL * 6), linkCol = new Float32Array(MAXL * 8), linkGeo = new THREE.BufferGeometry(); linkGeo.setAttribute('position', new THREE.BufferAttribute(linkPos, 3)); linkGeo.setAttribute('color', new THREE.BufferAttribute(linkCol, 4));
  tilt.add(new THREE.LineSegments(linkGeo, new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending })));
  const pktPos = new Float32Array(MAXL * 3), pktCol = new Float32Array(MAXL * 3), pktSize = new Float32Array(MAXL), pktGeo = new THREE.BufferGeometry(); pktGeo.setAttribute('position', new THREE.BufferAttribute(pktPos, 3)); pktGeo.setAttribute('color', new THREE.BufferAttribute(pktCol, 3)); pktGeo.setAttribute('size', new THREE.BufferAttribute(pktSize, 1)); tilt.add(new THREE.Points(pktGeo, glowMat(8, DPR)));
  const JAM = ll(47.4, 13.2, 1.0), bubbleMat = new THREE.ShaderMaterial({ transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, uniforms: { uA: { value: 0 } }, vertexShader: fresnelVS, fragmentShader: `uniform float uA; varying vec3 vN; varying vec3 vV; void main(){ float f = 1.0 - abs(dot(vN, vV)); float a = (0.07 + pow(f, 2.2)*0.85) * uA; gl_FragColor = vec4(vec3(1.0,0.26,0.2)*a, a); }` });
  const bubble = new THREE.Mesh(new THREE.SphereGeometry(1, 64, 48), bubbleMat); bubble.position.copy(JAM); bubble.scale.setScalar(0.0001); spin.add(bubble); const wave = new THREE.Mesh(new THREE.SphereGeometry(1, 48, 32), bubbleMat.clone()); wave.position.copy(JAM); spin.add(wave);
  const TR = 90, truthPts = [], falsePts = []; for (let i = 0; i <= TR; i++){ const k = i / TR, dk = Math.max(0, k - 0.18); truthPts.push(ll(RX.lat + 2.2 * Math.sin(k * 2.4), RX.lon + 16 * k, 1.006)); falsePts.push(ll(RX.lat + 2.2 * Math.sin(k * 2.4) + 9 * Math.pow(dk, 1.6), RX.lon + 16 * k - 5 * dk * dk, 1.006)); }
  const truthLine = new THREE.Line(new THREE.BufferGeometry().setFromPoints(truthPts), new THREE.LineBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.95, depthWrite: false }));
  const falseLine = new THREE.Line(new THREE.BufferGeometry().setFromPoints(falsePts), new THREE.LineDashedMaterial({ color: 0xa58bff, transparent: true, opacity: 1, dashSize: 0.018, gapSize: 0.012, depthWrite: false })); falseLine.computeLineDistances();
  truthLine.geometry.setDrawRange(0, 0); falseLine.geometry.setDrawRange(0, 0); spin.add(truthLine, falseLine);
  const heads = new THREE.BufferGeometry(); heads.setAttribute('position', new THREE.Float32BufferAttribute(new Float32Array(9), 3)); heads.setAttribute('color', new THREE.Float32BufferAttribute([1, 1, 1, 0.65, 0.55, 1, 0.65, 0.4, 1], 3)); heads.setAttribute('size', new THREE.Float32BufferAttribute([0, 0, 0], 1)); spin.add(new THREE.Points(heads, glowMat(16, DPR)));
  const SPOOFER = ll(50.6, 24.5, 1.006), beamGeo = new THREE.BufferGeometry().setFromPoints([SPOOFER.clone(), SPOOFER.clone()]), beam = new THREE.Line(beamGeo, new THREE.LineBasicMaterial({ color: 0xa58bff, transparent: true, opacity: 0, depthWrite: false, blending: THREE.AdditiveBlending })); spin.add(beam);
  let W = 1, H = 1, mobile = false;
  function resize(){ const r = canvas.getBoundingClientRect(); W = Math.max(1, r.width); H = Math.max(1, r.height); mobile = W < 900; renderer.setSize(W, H, false); camera.aspect = W / H; camera.updateProjectionMatrix(); dotMat.uniforms.uScale.value = Math.min(1.25, Math.max(0.7, Math.min(W, H * 1.4) / 1100)); }
  window.addEventListener('resize', resize); resize();
  const mouse = { x: 0, y: 0, sx: 0, sy: 0 }; window.addEventListener('pointermove', e => { mouse.x = e.clientX / window.innerWidth * 2 - 1; mouse.y = e.clientY / window.innerHeight * 2 - 1; }, { passive: true });
  let visible = true; onVisible(stageEl, v => { visible = v; });
  const tmpA = new THREE.Vector3(), rxT = new THREE.Vector3(), nrm = new THREE.Vector3(), rcol = new THREE.Color(), cRed = new THREE.Color('#ff3b30'), cMint = new THREE.Color('#3ddc84'), cWhite = new THREE.Color('#ffffff'), cAmber = new THREE.Color('#ffb224');
  const wrap = a => { while (a > Math.PI) a -= 2 * Math.PI; while (a < -Math.PI) a += 2 * Math.PI; return a; };
  let spinAuto = -RX.lon * D2R + 1.1, t = 0, last = performance.now(), first = true;
  function frame(now){ requestAnimationFrame(frame); const dt = Math.min(0.05, (now - last) / 1000); last = now; if (!visible && !first) return; if (!reduced) t += dt;
    const p = KS.p, focus = ss(0.25, 1.15, p), jam = ss(1.95, 2.7, p) * (1 - ss(3.05, 3.4, p)), jamFade = ss(1.95, 2.2, p) * (1 - ss(3.05, 3.4, p)), spoof = ss(3.0, 3.85, p), spoofVis = ss(2.95, 3.15, p) * (1 - ss(4.05, 4.3, p)), hold = ss(3.95, 4.6, p);
    if (!reduced) spinAuto += dt * 0.045 * (1 - focus);
    const target = -RX.lon * D2R + 0.18 * (1 - ss(1.2, 2.2, p)) - 0.12 * spoofVis; spin.rotation.y = spinAuto + wrap(target - spinAuto) * focus; tilt.rotation.x = lerp(0.32, RX.lat * D2R * 0.82, focus); tilt.rotation.z = lerp(-0.12, 0, focus); orbitGroup.rotation.y = t * 0.01;
    const heroX = mobile ? 0 : 0.95, heroY = mobile ? -1.1 : -0.02, stX = mobile ? 0 : 0.35, stY = mobile ? 0.45 : 0.0; world.position.set(lerp(heroX, stX, focus), lerp(heroY, stY, focus), 0);
    mouse.sx += (mouse.x - mouse.sx) * 0.04; mouse.sy += (mouse.y - mouse.sy) * 0.04; camera.position.set(mouse.sx * 0.14, -mouse.sy * 0.09, lerp(mobile ? 6.4 : 5.0, mobile ? 4.9 : 3.7, focus)); camera.lookAt(world.position.x * 0.15, world.position.y * 0.15, 0);
    sats.forEach((s, i) => { orbitPos(s.r, s.inc, s.raan, s.u0 + t * s.speed, s.pos); satPos[i * 3] = s.pos.x; satPos[i * 3 + 1] = s.pos.y; satPos[i * 3 + 2] = s.pos.z; satSize[i] = 1 - 0.45 * hold; }); satGeo.attributes.position.needsUpdate = true; satGeo.attributes.size.needsUpdate = true;
    spin.updateMatrix(); rxT.copy(rxLocal).applyMatrix4(spin.matrix); nrm.copy(rxT).normalize();
    let n = 0; const linkAlpha = 0.6 * (1 - hold), rr = lerp(cMint.r, cRed.r, jamFade), rg = lerp(cMint.g, cRed.g, jamFade), rb = lerp(cMint.b, cRed.b, jamFade);
    for (let i = 0; i < sats.length && n < MAXL; i++){ const s = sats[i]; tmpA.copy(s.pos).sub(rxT); const dist = tmpA.length(), el = tmpA.dot(nrm) / dist; if (el < 0.12) continue; const fadeEl = ss(0.12, 0.3, el), cut = jam * 0.92 - s.seed, a = linkAlpha * fadeEl * (jam > 0.02 && cut > 0 ? Math.max(0, 1 - cut * 9) : 1), c1 = CONS[s.ci].color;
      linkPos.set([s.pos.x, s.pos.y, s.pos.z, rxT.x, rxT.y, rxT.z], n * 6); linkCol.set([lerp(c1.r, cRed.r, jamFade), lerp(c1.g, cRed.g, jamFade), lerp(c1.b, cRed.b, jamFade), a * 0.55, rr, rg, rb, a], n * 8);
      const f = reduced ? 0.5 : ((t * 0.55 + s.seed * 3.7) % 1); pktPos.set([lerp(s.pos.x, rxT.x, f), lerp(s.pos.y, rxT.y, f), lerp(s.pos.z, rxT.z, f)], n * 3); pktCol.set([lerp(1, cRed.r, jamFade), lerp(1, cRed.g, jamFade), lerp(1, cRed.b, jamFade)], n * 3); pktSize[n] = a * 1.6; n++; }
    for (let k = n; k < MAXL; k++) pktSize[k] = 0; linkGeo.setDrawRange(0, n * 2); linkGeo.attributes.position.needsUpdate = true; linkGeo.attributes.color.needsUpdate = true; pktGeo.attributes.position.needsUpdate = true; pktGeo.attributes.color.needsUpdate = true; pktGeo.attributes.size.needsUpdate = true;
    rcol.copy(cMint).lerp(cRed, jamFade).lerp(cWhite, spoofVis * 0.6).lerp(cAmber, hold); rxGeo.attributes.color.array.set([rcol.r, rcol.g, rcol.b]); rxGeo.attributes.color.needsUpdate = true;
    [pulse, pulse2].forEach((m, i) => { const k = reduced ? 0.5 : ((t * 0.6 + i * 0.5) % 1); m.scale.setScalar(0.01 + k * 0.07 * (1 + hold)); m.material.opacity = (1 - k) * 0.7; m.material.color.copy(rcol); });
    const jr2 = 0.02 + jam * 0.3; bubble.scale.setScalar(Math.max(0.0001, jr2 * (1 + 0.015 * Math.sin(t * 3)))); bubbleMat.uniforms.uA.value = jamFade * 0.9; const wk = reduced ? 0.6 : ((t * 0.45) % 1); wave.scale.setScalar(Math.max(0.0001, jr2 * (0.3 + wk * 0.9))); wave.material.uniforms.uA.value = jamFade * (1 - wk) * 0.5;
    const cnt = Math.floor(spoof * TR); truthLine.geometry.setDrawRange(0, spoofVis > 0 ? cnt + 1 : 0); falseLine.geometry.setDrawRange(0, spoofVis > 0 ? cnt + 1 : 0); truthLine.material.opacity = 0.95 * spoofVis; falseLine.material.opacity = spoofVis;
    const hp = heads.attributes.position.array, hs = heads.attributes.size.array, tp = truthPts[Math.min(cnt, TR)], fp = falsePts[Math.min(cnt, TR)]; hp.set([tp.x, tp.y, tp.z, fp.x, fp.y, fp.z, SPOOFER.x, SPOOFER.y, SPOOFER.z]); hs[0] = spoofVis * 1.1; hs[1] = spoofVis * 1.1; hs[2] = spoofVis * (1.2 + 0.3 * Math.sin(t * 6)); heads.attributes.position.needsUpdate = true; heads.attributes.size.needsUpdate = true;
    beamGeo.attributes.position.array.set([SPOOFER.x, SPOOFER.y, SPOOFER.z, fp.x, fp.y, fp.z]); beamGeo.attributes.position.needsUpdate = true; beam.material.opacity = spoofVis * (0.45 + 0.25 * Math.sin(t * 8));
    orbitLines.forEach(o => { o.material.opacity = 0.2 * (1 - 0.6 * hold); });
    renderer.render(scene, camera); if (first){ first = false; requestAnimationFrame(() => canvas.classList.add('ready')); } }
  requestAnimationFrame(frame);
})();

/* ======================= EARTH → MOON → MARS (lazy) ======================= */
(function journey(){
  const canvas = document.getElementById('jGl'), sec = document.getElementById('journey'); if (!canvas || !sec) return;
  let started = false;
  new IntersectionObserver((es, o) => { if (es[0].isIntersecting && !started){ started = true; o.disconnect(); init(); } }, { rootMargin: '700px' }).observe(sec);
  function init(){
    const renderer = makeRenderer(canvas); if (!renderer) return;
    const DPR = Math.min(2, window.devicePixelRatio || 1), scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(38, 1, 0.1, 400);
    /* positions are illustrative, not to scale */
    const EARTH = new THREE.Vector3(0, 0, 0), MOON = new THREE.Vector3(16, 1.2, -6), MARS = new THREE.Vector3(40, -1.5, -18);
    const earth = planet(1.6, '#04060d', '#12345a', '#4f86d8'); earth.position.copy(EARTH); scene.add(earth);
    const moon = planet(0.6, '#15161b', '#8a8d97', '#b9bfd0'); moon.position.copy(MOON); scene.add(moon);
    const mars = planet(0.95, '#1c0904', '#b4502c', '#ff9a6a'); mars.position.copy(MARS); scene.add(mars);
    const stars = new THREE.BufferGeometry(), sp = []; for (let i = 0; i < 1600; i++){ const v = new THREE.Vector3().randomDirection().multiplyScalar(160 + Math.random() * 60); sp.push(v.x + 20, v.y, v.z - 10); } stars.setAttribute('position', new THREE.Float32BufferAttribute(sp, 3));
    scene.add(new THREE.Points(stars, new THREE.PointsMaterial({ color: 0xffffff, size: 0.45, sizeAttenuation: true, transparent: true, opacity: 0.55, depthWrite: false })));
    const moving = [];
    function shell(center, r, inc, planes, per, color, speed){ const g = new THREE.Group(); g.position.copy(center); scene.add(g); const col = new THREE.Color(color), v = new THREE.Vector3();
      for (let p = 0; p < planes; p++){ const raan = p * 2 * Math.PI / planes, ring = []; for (let k = 0; k <= 160; k++) ring.push(orbitPos(r, inc * D2R, raan, k / 160 * Math.PI * 2, v).clone()); g.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(ring), new THREE.LineBasicMaterial({ color: col, transparent: true, opacity: 0.22, depthWrite: false, blending: THREE.AdditiveBlending })));
        for (let s = 0; s < per; s++) moving.push({ g, r, inc: inc * D2R, raan, u0: s * 2 * Math.PI / per + p * .4, speed, col }); } return g; }
    shell(EARTH, 3.4, 55, 6, 4, '#58a4ff', .22); shell(EARTH, 3.8, 56, 3, 6, '#ae90ff', .18); shell(EARTH, 3.1, 64.8, 3, 5, '#ff7d66', .24);
    shell(MOON, 1.6, 60, 3, 2, '#2cd0de', .5); shell(MARS, 2.0, 30, 2, 2, '#ffb224', .4);
    const tr = []; for (let i = 0; i <= 120; i++){ const k = i / 120; tr.push(new THREE.Vector3().lerpVectors(EARTH, MOON, k).add(new THREE.Vector3(0, Math.sin(k * Math.PI) * 2.2, 0))); } scene.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(tr), new THREE.LineDashedMaterial({ color: 0xa58bff, dashSize: .3, gapSize: .25, transparent: true, opacity: .5 })));
    const tr2 = []; for (let i = 0; i <= 160; i++){ const k = i / 160; tr2.push(new THREE.Vector3().lerpVectors(EARTH, MARS, k).add(new THREE.Vector3(0, Math.sin(k * Math.PI) * 6, 0))); } const tl2 = new THREE.Line(new THREE.BufferGeometry().setFromPoints(tr2), new THREE.LineDashedMaterial({ color: 0xff7d66, dashSize: .4, gapSize: .35, transparent: true, opacity: .35 })); tl2.computeLineDistances(); scene.add(tl2);
    scene.children.forEach(o => { if (o.isLine && o.material.isLineDashedMaterial) o.computeLineDistances(); });
    const satGeo = new THREE.BufferGeometry(), sPos = new Float32Array(moving.length * 3), sCol = new Float32Array(moving.length * 3), sSize = new Float32Array(moving.length).fill(1);
    moving.forEach((m, i) => sCol.set([m.col.r, m.col.g, m.col.b], i * 3)); satGeo.setAttribute('position', new THREE.BufferAttribute(sPos, 3)); satGeo.setAttribute('color', new THREE.BufferAttribute(sCol, 3)); satGeo.setAttribute('size', new THREE.BufferAttribute(sSize, 1)); scene.add(new THREE.Points(satGeo, glowMat(14, DPR)));
    const STOPS = [{ at: 0, pos: new THREE.Vector3(3.5, 2.2, 11), look: EARTH }, { at: .5, pos: new THREE.Vector3(18.5, 2.8, -0.5), look: MOON }, { at: 1, pos: new THREE.Vector3(43, 0.8, -11), look: MARS }];
    function resize(){ const r = canvas.getBoundingClientRect(); renderer.setSize(Math.max(1, r.width), Math.max(1, r.height), false); camera.aspect = Math.max(1, r.width) / Math.max(1, r.height); camera.updateProjectionMatrix(); }
    window.addEventListener('resize', resize); resize();
    let visible = true; onVisible(sec, v => { visible = v; });
    const cp = new THREE.Vector3(), cl = new THREE.Vector3(), tmp = new THREE.Vector3(); let js = 0, t = 0, last = performance.now(), first = true;
    function frame(now){ requestAnimationFrame(frame); const dt = Math.min(.05, (now - last) / 1000); last = now; if (!visible && !first) return; if (!reduced) t += dt;
      const target = reduced ? .5 : (KS.j || 0); js += (target - js) * (1 - Math.exp(-dt * 4));
      const k = js < .5 ? ss(.12, .45, js) : ss(.58, .92, js), a = js < .5 ? STOPS[0] : STOPS[1], b = js < .5 ? STOPS[1] : STOPS[2];
      cp.lerpVectors(a.pos, b.pos, k); cl.lerpVectors(a.look, b.look, k); camera.position.copy(cp); camera.lookAt(cl);
      moving.forEach((m, i) => { orbitPos(m.r, m.inc, m.raan, m.u0 + t * m.speed, tmp).add(m.g.position); sPos.set([tmp.x, tmp.y, tmp.z], i * 3); }); satGeo.attributes.position.needsUpdate = true;
      earth.rotation.y += dt * .05; moon.rotation.y += dt * .02; mars.rotation.y += dt * .04;
      renderer.render(scene, camera); if (first){ first = false; requestAnimationFrame(() => canvas.classList.add('ready')); } }
    requestAnimationFrame(frame);
  }
})();
