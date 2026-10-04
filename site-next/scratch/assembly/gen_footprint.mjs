// Recompute the Missions jamming footprint (src/js/missions.mjs FOOTPRINT) with the Studio's package:
// one run of maritime-strait-jamming.toml per cell, only [receiver] lat_deg / lon_deg changed.
import fs from 'fs';
const S='/Users/chak/Code/kshana-site-next/explorations/2026-09-30-studio-dash/';
const M='/Users/chak/Code/kshana-site-next/explorations/2026-09-27-combo/src/js/missions.mjs';
const k=await import(S+'pkg/kshana.js'); k.initSync({module: fs.readFileSync(S+'pkg/kshana_bg.wasm')});
const g=await import(S+'lib/guided.mjs');
const src=fs.readFileSync(M,'utf8'); const i0=src.indexOf('const FOOTPRINT = {'); const blk=src.slice(i0, src.indexOf('};',i0));
const num=(key)=>Number(blk.match(new RegExp('\\b'+key+':\\s*([-0-9.]+)'))[1]);
const arr=(key)=>blk.match(new RegExp('\\b'+key+':\\s*\\[([^\\]]+)\\]'))[1].split(',').map(Number);
const F={jlat:num('jlat'),jlon:num('jlon'),step:num('step'),nx:num('nx'),ny:num('ny'),js10:arr('js10'),av:arr('av')};
const toml=fs.readFileSync(S+'scenarios/maritime-strait-jamming.toml','utf8');
const kmLat=111.2, kmLon=111.32*Math.cos(F.jlat*Math.PI/180);
const js10=[], av=[]; let diff=0;
for (let j=-F.ny;j<=F.ny;j++) for (let i=-F.nx;i<=F.nx;i++){
  const la=(F.jlat+j*F.step/kmLat).toFixed(3), lo=(F.jlon+i*F.step/kmLon).toFixed(3);
  let t=g.patchSectionScalar(toml,'receiver','lat_deg',la); t=g.patchSectionScalar(t,'receiver','lon_deg',lo);
  const r=JSON.parse(k.run(t)); js10.push(Math.round(r.fom.mean_js_db*10)); av.push(Math.round(r.fom.availability_under_jamming*1000));
}
for (let q=0;q<js10.length;q++) if (js10[q]!==F.js10[q]||av[q]!==F.av[q]) diff++;
console.log('engine', k.version(), 'cells', js10.length, 'cells differing from the published grid', diff);
if (process.argv[2]==='--write'){
  let s=src.replace(/(const FOOTPRINT = \{\n  engine: ")[^"]+"/, `$1${k.version()}"`);
  s=s.replace(/js10: \[[^\]]+\]/, `js10: [${js10}]`).replace(/av: \[[^\]]+\]/, `av: [${av}]`);
  fs.writeFileSync(M,s); console.log('wrote', M);
}
if (process.argv[2]==='--show') for (let q=0;q<js10.length;q++) if (js10[q]!==F.js10[q]||av[q]!==F.av[q]) console.log(q, 'js10', F.js10[q], '->', js10[q], 'av', F.av[q], '->', av[q]);
