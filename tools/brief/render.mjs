import fs from 'node:fs'; import pw from '/opt/node-tools/node_modules/playwright/index.js'; const { chromium } = pw
const d = JSON.parse(fs.readFileSync(new URL('./data.json', import.meta.url)))
const pct = Math.round(100*d.plan.done/d.plan.total)
const esc = s => String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;')
const li = (a, cls) => a.map(x=>`<li class="${cls}">${esc(x)}</li>`).join('')
const dot = s => ({active:'#2563eb',done:'#16a34a',waiting:'#d97706',idle:'#9ca3af',now:'#2563eb',ok:'#16a34a'}[s]||'#9ca3af')
const html = `<!doctype html><html><head><meta charset="utf-8"><style>
:root{--bg:#f7f7f5;--card:#fff;--ink:#1f2328;--muted:#6b7280;--line:#e5e7eb}
*{box-sizing:border-box}body{margin:0;background:var(--bg);font:15px/1.4 Inter,system-ui,-apple-system,Segoe UI,Roboto,sans-serif;color:var(--ink)}
.wrap{width:1600px;padding:36px 40px}
header{display:flex;justify-content:space-between;align-items:flex-end;margin-bottom:22px}
h1{margin:0;font-size:30px;letter-spacing:-.01em}.sub{color:var(--muted);margin-top:4px}
.eta{background:#eef4ff;color:#1e40af;border-radius:10px;padding:10px 14px;font-weight:600}
.grid{display:grid;grid-template-columns:1.05fr 1fr 1fr;gap:18px}
.card{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:18px 20px}
h2{font-size:13px;text-transform:uppercase;letter-spacing:.06em;color:var(--muted);margin:0 0 12px}
.bar{height:14px;background:#eceff3;border-radius:7px;overflow:hidden;margin:8px 0 6px}.bar>i{display:block;height:100%;width:${pct}%;background:linear-gradient(90deg,#16a34a,#22c55e)}
.big{font-size:44px;font-weight:700;line-height:1}.big small{font-size:16px;color:var(--muted);font-weight:500;margin-left:8px}
ul{margin:0;padding:0;list-style:none}li{padding:5px 0 5px 22px;position:relative;border-bottom:1px dashed #eef0f2}li:last-child{border:0}
li:before{position:absolute;left:0;top:5px;font-weight:700}
li.done:before{content:"✓";color:#16a34a}li.open:before{content:"●";color:#2563eb;font-size:12px;top:7px}li.park:before{content:"‖";color:#9ca3af}
.sec{margin-top:14px}.sec h3{font-size:13px;margin:0 0 4px;color:#374151}
table{width:100%;border-collapse:collapse}td{padding:6px 4px;border-bottom:1px dashed #eef0f2;vertical-align:top}tr:last-child td{border:0}
td.k{font-weight:600;white-space:nowrap;padding-right:12px}.d{display:inline-block;width:9px;height:9px;border-radius:50%;margin-right:8px;vertical-align:1px}
.wait td.k{color:#b45309}.wait tr:first-child td{background:#fff7ed}.wait tr:first-child td.k{color:#c2410c}
footer{margin-top:16px;color:var(--muted);font-size:12.5px}
</style></head><body><div class="wrap">
<header><div><h1>${esc(d.title)}</h1><div class="sub">As of ${esc(d.asof)} · read-only snapshot</div></div><div class="eta">${esc(d.eta)}</div></header>
<div class="grid">
<div class="card"><h2>Plan</h2><div class="big">${pct}%<small>${d.plan.done} of ${d.plan.total} ${esc(d.plan.label)} done</small></div><div class="bar"><i></i></div>
<div class="sec"><h3>Done</h3><ul>${li(d.milestones_done,'done')}</ul></div>
<div class="sec"><h3>Missing (in progress)</h3><ul>${li(d.milestones_open,'open')}</ul></div>
<div class="sec"><h3>Parked</h3><ul>${li(d.milestones_park||d.milestones_parked,'park')}</ul></div></div>
<div style="display:flex;flex-direction:column;gap:18px">
<div class="card"><h2>Who is working on what</h2><table>${d.people.map(([n,w,s])=>`<tr><td class="k"><span class="d" style="background:${dot(s)}"></span>${esc(n)}</td><td>${esc(w)}</td></tr>`).join('')}</table></div>
<div class="card"><h2>Recent releases</h2><table>${d.releases.map(([v,w,s])=>`<tr><td class="k"><span class="d" style="background:${dot(s)}"></span>${esc(v)}</td><td>${esc(w)}</td></tr>`).join('')}</table></div></div>
<div style="display:flex;flex-direction:column;gap:18px">
<div class="card"><h2>Tests</h2><table>${d.tests.map(([k,v])=>`<tr><td class="k">${esc(k)}</td><td>${esc(v)}</td></tr>`).join('')}</table></div>
<div class="card wait"><h2>Waiting on you</h2><table>${d.waiting.map(([k,v])=>`<tr><td class="k">${esc(k)}</td><td>${esc(v)}</td></tr>`).join('')}</table></div></div>
</div><footer>Sources: ${d.sources || "worker reports"}</footer></div></body></html>`
fs.writeFileSync(new URL('./brief.html', import.meta.url), html)
const b = await chromium.launch(); const p = await b.newPage({viewport:{width:1600,height:900},deviceScaleFactor:2})
await p.setContent(html); await p.screenshot({path:new URL('./kshana-brief.png', import.meta.url).pathname, fullPage:true}); await b.close()
console.log('ok')
