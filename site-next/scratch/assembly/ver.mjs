import fs from 'fs'; import init, * as k from './pkg-0.29.0/kshana.js';
await init({module_or_path: fs.readFileSync(new URL('./pkg-0.29.0/kshana_bg.wasm', import.meta.url))});
for (const n of Object.keys(k)) if (/version/i.test(n)) console.log(n, typeof k[n]==='function'? k[n]() : k[n]);
