// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Self-contained HTML dashboard for a [`super::MonitorResult`]: no external assets, light
//! and dark themes, a timeline player over the snapshots, and an optional live mode that
//! follows a growing JSON-lines stream (`monitor.html?live=monitor.jsonl` when both files
//! are served over HTTP, e.g. by `python3 -m http.server`).

use super::MonitorResult;

const PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>eLoran Monitor</title>
<style>
:root{--bg:#f6f7f9;--panel:#fff;--ink:#111827;--muted:#6b7280;--line:#e5e7eb;--accent:#0b6e99;--ok:#15803d;--warn:#b45309;--bad:#b91c1c;--grid:#eef0f3}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#0f1115;--panel:#171a21;--ink:#e5e7eb;--muted:#9ca3af;--line:#2a2f3a;--accent:#38bdf8;--ok:#4ade80;--warn:#fbbf24;--bad:#f87171;--grid:#1f2430}}
:root[data-theme="dark"]{--bg:#0f1115;--panel:#171a21;--ink:#e5e7eb;--muted:#9ca3af;--line:#2a2f3a;--accent:#38bdf8;--ok:#4ade80;--warn:#fbbf24;--bad:#f87171;--grid:#1f2430}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:14px/1.4 system-ui,-apple-system,Segoe UI,Roboto,sans-serif}
header{padding:12px 16px;border-bottom:1px solid var(--line);display:flex;flex-wrap:wrap;gap:8px 20px;align-items:baseline}
header h1{font-size:16px;margin:0}header .meta{color:var(--muted);font-size:12px}
main{display:grid;gap:12px;padding:12px 16px;grid-template-columns:repeat(auto-fit,minmax(320px,1fr))}
.panel{background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:10px 12px;min-width:0}
.panel h2{font-size:13px;margin:0 0 8px;color:var(--muted);font-weight:600}
.wide{grid-column:1/-1}.tiles{display:flex;flex-wrap:wrap;gap:8px}.tile{flex:1 1 120px;border:1px solid var(--line);border-radius:6px;padding:6px 8px}
.tile b{display:block;font-size:18px}.tile span{color:var(--muted);font-size:12px}
table{width:100%;border-collapse:collapse;font-size:12px}th,td{padding:4px 6px;border-bottom:1px solid var(--line);text-align:right;white-space:nowrap}th:first-child,td:first-child{text-align:left}
.tablewrap{overflow-x:auto}svg{width:100%;height:auto;display:block}
.controls{display:flex;gap:8px;align-items:center;flex-wrap:wrap}input[type=range]{flex:1;min-width:160px}
button,select{background:var(--panel);color:var(--ink);border:1px solid var(--line);border-radius:6px;padding:4px 10px;font:inherit}
.ok{color:var(--ok)}.warn{color:var(--warn)}.bad{color:var(--bad)}.muted{color:var(--muted)}
</style>
</head>
<body>
<header><h1>eLoran Monitor</h1><span class="meta" id="meta"></span><span style="flex:1"></span>
<button id="theme" title="Toggle theme">◐</button></header>
<main>
<section class="panel wide"><div class="controls"><button id="play">▶</button><input type="range" id="slider" min="0" max="0" value="0"><span id="time" class="muted"></span><span id="live" class="muted"></span></div></section>
<section class="panel"><h2>Health</h2><div class="tiles" id="health"></div></section>
<section class="panel"><h2>Map</h2><svg id="map" viewBox="0 0 400 300" role="img" aria-label="Stations and receiver"></svg></section>
<section class="panel wide"><h2>Stations</h2><div class="tablewrap"><table id="stations"></table></div></section>
<section class="panel"><h2>Spectrum (dBFS)</h2><svg id="spec" viewBox="0 0 400 200" role="img" aria-label="Spectrum"></svg></section>
<section class="panel"><h2>Pulse view (µV/m)</h2><div class="controls"><select id="psel"></select></div><svg id="pulse" viewBox="0 0 400 200" role="img" aria-label="Pulse"></svg></section>
<section class="panel"><h2>Fix</h2><div id="fix" class="tablewrap"></div></section>
<section class="panel"><h2>Truth comparison</h2><div id="truth" class="tablewrap"></div></section>
</main>
<script type="application/json" id="data">__DATA__</script>
<script>
const D=JSON.parse(document.getElementById('data').textContent);
let H=D.header,S=D.snapshots,SUM=D.summary,idx=Math.max(0,S.length-1),timer=null;
const $=id=>document.getElementById(id);
const fmt=(v,d=1)=>v==null||!isFinite(v)?'–':Number(v).toFixed(d);
const esc=s=>String(s).replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
function meta(){$('meta').textContent=`${H.source} · start ${H.start_utc} · ${(H.fs_hz/1e6).toFixed(2)} MS/s · ${H.channels} ch · ${fmt(H.duration_s,1)} s`+(H.scenario_sha256?` · scenario ${H.scenario_sha256.slice(0,12)}`:'');}
function line(svg,xs,ys,{w=400,h=200,pad=28,color='var(--accent)',ymin,ymax,xlab='',ylab=''}={}){
 if(!xs.length){svg.innerHTML='';return}
 const x0=Math.min(...xs),x1=Math.max(...xs);let y0=ymin??Math.min(...ys),y1=ymax??Math.max(...ys);if(y1===y0){y1+=1;y0-=1}
 const X=x=>pad+(x-x0)/(x1-x0||1)*(w-pad-6),Y=y=>h-pad+( -(y-y0)/(y1-y0))*(h-pad-8);
 let g='';for(let i=0;i<=4;i++){const yy=y0+(y1-y0)*i/4;g+=`<line x1="${pad}" x2="${w-6}" y1="${Y(yy)}" y2="${Y(yy)}" stroke="var(--grid)"/><text x="2" y="${Y(yy)+4}" font-size="9" fill="var(--muted)">${fmt(yy,yy>100||yy<-100?0:1)}</text>`}
 g+=`<text x="${w-6}" y="${h-4}" font-size="9" text-anchor="end" fill="var(--muted)">${esc(xlab)}</text>`;
 const pts=xs.map((x,i)=>`${X(x).toFixed(1)},${Y(ys[i]).toFixed(1)}`).join(' ');
 svg.innerHTML=g+`<polyline fill="none" stroke="${color}" stroke-width="1.4" points="${pts}"/>`;
}
function map(s){
 const svg=$('map');const pts=H.stations.map(x=>[x.lon_deg,x.lat_deg]).concat(H.receiver_track.map(r=>[r[2],r[1]]));
 if(s&&s.fix&&s.fix.lat_deg!=null)pts.push([s.fix.lon_deg,s.fix.lat_deg]);
 const lo=pts.map(p=>p[0]),la=pts.map(p=>p[1]);let x0=Math.min(...lo),x1=Math.max(...lo),y0=Math.min(...la),y1=Math.max(...la);
 const mx=(x1-x0)*.12+.2,my=(y1-y0)*.12+.2;x0-=mx;x1+=mx;y0-=my;y1+=my;const k=Math.cos((y0+y1)/2*Math.PI/180);
 const sx=380/((x1-x0)*k),sy=280/(y1-y0),sc=Math.min(sx,sy);const X=x=>10+(x-x0)*k*sc,Y=y=>290-(y-y0)*sc;
 let g='';const tr=H.receiver_track.map(r=>`${X(r[2]).toFixed(1)},${Y(r[1]).toFixed(1)}`).join(' ');
 g+=`<polyline fill="none" stroke="var(--muted)" stroke-dasharray="3 3" points="${tr}"/>`;
 const tracked=new Set((s?s.stations:[]).map(v=>v.chain+'/'+v.station));
 for(const st of H.stations){const on=tracked.has(st.chain+'/'+st.id);g+=`<g><circle cx="${X(st.lon_deg)}" cy="${Y(st.lat_deg)}" r="${st.role==='master'?6:4.5}" fill="${on?'var(--ok)':'var(--panel)'}" stroke="${on?'var(--ok)':'var(--muted)'}"/><text x="${X(st.lon_deg)+7}" y="${Y(st.lat_deg)+4}" font-size="10" fill="var(--ink)">${esc(st.chain)}/${esc(st.id)}</text></g>`}
 if(H.receiver_track.length){const t=s?s.t_s:0;let r=H.receiver_track[0];for(const q of H.receiver_track)if(q[0]<=t)r=q;g+=`<rect x="${X(r[2])-4}" y="${Y(r[1])-4}" width="8" height="8" fill="var(--accent)"/>`}
 if(s&&s.fix&&s.fix.lat_deg!=null){g+=`<path d="M${X(s.fix.lon_deg)-6} ${Y(s.fix.lat_deg)}h12M${X(s.fix.lon_deg)} ${Y(s.fix.lat_deg)-6}v12" stroke="var(--bad)" stroke-width="2"/>`}
 svg.innerHTML=g;
}
function stations(s){
 const rows=(s?s.stations:[]).map(v=>`<tr><td>${esc(v.chain)}/${esc(v.station)}${v.channel!=null?' ch'+v.channel:''}</td><td>${fmt(v.snr_db)}</td><td>${fmt(v.ecd_us,2)}</td><td>${fmt(v.field_uv_m,0)}</td><td>${v.bearing_deg==null?'–':fmt(v.bearing_deg,1)}</td><td>${v.measurements}</td><td class="${v.cycle_slips?'warn':''}">${v.cycle_slips}</td><td>${fmt(v.toa_s,6)}</td></tr>`).join('');
 $('stations').innerHTML='<tr><th>Station</th><th>SNR dB</th><th>ECD µs</th><th>Field µV/m</th><th>Bearing °</th><th>Meas.</th><th>Slips</th><th>Last ToA s</th></tr>'+(rows||'<tr><td colspan="8" class="muted">acquiring…</td></tr>');
 const sel=$('psel'),cur=sel.value;sel.innerHTML=(s?s.pulses:[]).map(p=>`<option>${esc(p.chain)}/${esc(p.station)}</option>`).join('');if([...sel.options].some(o=>o.value===cur))sel.value=cur;
}
function health(s){if(!s){$('health').innerHTML='';return}const h=s.health;
 const rt=h.realtime_factor;$('health').innerHTML=[[fmt(rt,1)+'×','real time',rt>=1?'ok':'bad'],[fmt(h.latency_s,2)+' s','latency',''],[h.locks,'locks',h.locks?'ok':'warn'],[fmt(h.peak_fs,3),'peak FS',h.peak_fs>1?'bad':''],[h.clipped,'clipped',h.clipped?'bad':'ok']].map(([b,l,c])=>`<div class="tile"><b class="${c}">${b}</b><span>${l}</span></div>`).join('')}
function fix(s){const f=s&&s.fix;if(!f){$('fix').innerHTML='<span class="muted">no solution yet</span>';return}
 let t=`<table><tr><th>Type</th><td>${esc(f.kind)}</td></tr><tr><th>At</th><td>${fmt(f.at_s,2)} s</td></tr>`;if(f.lat_deg!=null)t+=`<tr><th>Lat / Lon</th><td>${fmt(f.lat_deg,6)} / ${fmt(f.lon_deg,6)}</td></tr><tr><th>HDOP</th><td>${fmt(f.hdop,2)}</td></tr>`;
 for(const [k,v] of f.clock_ns)t+=`<tr><th>${esc(k)}</th><td>${fmt(v,1)} ns</td></tr>`;$('fix').innerHTML=t+'</table>'}
function truth(){if(!SUM){$('truth').innerHTML='<span class="muted">run in progress</span>';return}
 const r=SUM.truth.map(t=>`<tr><td>${esc(t.chain)}/${esc(t.station)}</td><td>${t.n}</td><td>${fmt(t.toa_mean_ns,1)}</td><td>${fmt(t.toa_rms_ns,1)}</td><td class="${t.cycle_errors?'bad':'ok'}">${t.cycle_errors}</td></tr>`).join('');
 $('truth').innerHTML=(SUM.truth.length?'<table><tr><th>Station</th><th>n</th><th>ToA mean ns</th><th>ToA RMS ns</th><th>Cycle errors</th></tr>'+r+'</table>':'<span class="muted">no truth (recording)</span>')+(SUM.final_fix_error_m!=null?`<p>Final fix error: <b>${fmt(SUM.final_fix_error_m,1)} m</b></p>`:'')+`<p class="muted">${SUM.measurements} measurements · ${SUM.stations_tracked} stations · evidence: ${esc(SUM.evidence_label)}</p>`}
function pulse(s){const sel=$('psel').value;const p=s&&s.pulses.find(q=>q.chain+'/'+q.station===sel);if(!p){$('pulse').innerHTML='';return}line($('pulse'),H.pulse_us,p.values,{xlab:'µs from ToA'})}
function render(){const s=S[idx];$('slider').max=Math.max(0,S.length-1);$('slider').value=idx;$('time').textContent=s?`t = ${fmt(s.t_s,2)} s  (${idx+1}/${S.length})`:'waiting for data';
 health(s);map(s);stations(s);if(s)line($('spec'),H.spectrum_hz.map(f=>f/1e3),s.spectrum_db,{xlab:'kHz'});pulse(s);fix(s);truth()}
$('slider').oninput=e=>{idx=+e.target.value;render()};$('psel').onchange=()=>pulse(S[idx]);
$('play').onclick=()=>{if(timer){clearInterval(timer);timer=null;$('play').textContent='▶';return}$('play').textContent='❚❚';if(idx>=S.length-1)idx=0;timer=setInterval(()=>{if(idx<S.length-1){idx++;render()}else{clearInterval(timer);timer=null;$('play').textContent='▶'}},300)};
$('theme').onclick=()=>{const r=document.documentElement;const dark=r.dataset.theme?r.dataset.theme==='dark':matchMedia('(prefers-color-scheme: dark)').matches;r.dataset.theme=dark?'light':'dark'};
const live=new URLSearchParams(location.search).get('live');
if(live){$('live').textContent='live';const poll=async()=>{try{const t=await (await fetch(live,{cache:'no-store'})).text();const L=t.trim().split('\n').filter(Boolean).map(l=>JSON.parse(l));
 const h=L.find(x=>x.type==='header');if(h)H=h.header;const follow=idx>=S.length-1;S=L.filter(x=>x.type==='snapshot').map(x=>x.snapshot);const sm=L.find(x=>x.type==='summary');SUM=sm?sm.summary:null;if(follow)idx=Math.max(0,S.length-1);meta();render();if(!sm)setTimeout(poll,1000);else $('live').textContent='complete'}catch(e){$('live').textContent='live: '+e;setTimeout(poll,2000)}};poll()}
meta();render();
</script>
</body>
</html>
"##;

/// The dashboard page for a monitor result (data embedded).
pub fn html(r: &MonitorResult) -> String {
    let data = serde_json::to_string(r)
        .unwrap_or_else(|_| "{}".into())
        .replace("</", "<\\/");
    PAGE.replace("__DATA__", &data)
}
