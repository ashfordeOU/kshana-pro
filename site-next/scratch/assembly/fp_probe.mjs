import fs from 'fs';
const S='/Users/chak/Code/kshana-site-next/explorations/2026-09-30-studio-dash/';
const k=await import(S+'pkg/kshana.js'); k.initSync({module: fs.readFileSync(S+'pkg/kshana_bg.wasm')});
const toml=fs.readFileSync(S+'scenarios/maritime-strait-jamming.toml','utf8');
const out=k.run(toml);
const r=typeof out==='string'?JSON.parse(out):out;
const walk=(o,p='')=>{ if(o&&typeof o==='object'&&!Array.isArray(o)) for(const [a,b] of Object.entries(o)) walk(b,p+'.'+a); else if(/js|avail|j_s/i.test(p)&&typeof o==='number') console.log(p,o); };
walk(r);
