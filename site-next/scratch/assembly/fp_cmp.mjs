import fs from 'fs';
const load=async(S)=>{const k=await import(S+'pkg/kshana.js'); k.initSync({module: fs.readFileSync(S+'pkg/kshana_bg.wasm')}); return k;};
const NEW='/Users/chak/Code/kshana-site-next/explorations/2026-09-30-studio-dash/', OLD='/Users/chak/Code/kshana-site-next/scratch/assembly/old-dash/2026-09-30-studio-dash/';
const kn=await load(NEW), ko=await load(OLD); 
const g=await import(NEW+'lib/guided.mjs');
const M='/Users/chak/Code/kshana-site-next/explorations/2026-09-27-combo/src/js/missions.mjs';
const src=fs.readFileSync(M,'utf8'); const i0=src.indexOf('const FOOTPRINT = {'); const blk=src.slice(i0, src.indexOf('};',i0));
const num=(key)=>Number(blk.match(new RegExp('\\b'+key+':\\s*([-0-9.]+)'))[1]); const arr=(key)=>blk.match(new RegExp('\\b'+key+':\\s*\\[([^\\]]+)\\]'))[1].split(',').map(Number);
const F={jlat:num('jlat'),jlon:num('jlon'),step:num('step'),nx:num('nx'),ny:num('ny'),js10:arr('js10')};
const toml=fs.readFileSync(NEW+'scenarios/maritime-strait-jamming.toml','utf8');
const kmLat=111.2, kmLon=111.32*Math.cos(F.jlat*Math.PI/180);
let same=0, diffPkg=0, exactFull=0, q=0;
for (let j=-F.ny;j<=F.ny;j++) for (let i=-F.nx;i<=F.nx;i++,q++){
  const la=(F.jlat+j*F.step/kmLat).toFixed(3), lo=(F.jlon+i*F.step/kmLon).toFixed(3);
  let t=g.patchSectionScalar(toml,'receiver','lat_deg',la); t=g.patchSectionScalar(t,'receiver','lon_deg',lo);
  const a=JSON.parse(kn.run(t)).fom, b=JSON.parse(ko.run(t)).fom;
  if (a.mean_js_db===b.mean_js_db && a.availability_under_jamming===b.availability_under_jamming) same++; else diffPkg++;
  let t2=g.patchSectionScalar(toml,'receiver','lat_deg',String(F.jlat+j*F.step/kmLat)); t2=g.patchSectionScalar(t2,'receiver','lon_deg',String(F.jlon+i*F.step/kmLon));
  if (Math.round(JSON.parse(kn.run(t2)).fom.mean_js_db*10)===F.js10[q]) exactFull++;
}
console.log('cells identical between 0.28.0 and 0.29.0 packages:', same, 'differ:', diffPkg, '| full-precision coordinates reproduce js10 in', exactFull, 'of', q);
