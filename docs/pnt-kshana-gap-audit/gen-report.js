#!/usr/bin/env node
/* Ashforde / Kshana — PNT paper capability-gap implementation tracker.
 * Reads ledger.json ({ perPaper, synthesis }) + optional status.json ({ [gapId|phaseN]: "done"|"wip" })
 * and emits a self-contained, on-brand HTML tracker to follow during implementation.
 * House palette locked to the Ashforde paper-banner series (deep navy + gold + cream).
 */
const fs = require('fs');
const path = require('path');

const IN = process.argv[2] || path.join(__dirname, 'ledger.json');
const STATUS_FILE = process.argv[3] || path.join(__dirname, 'status.json');
const OUT = process.argv[4] || path.join(__dirname, 'pnt-kshana-gap-report.html');
const STAMP = process.argv[5] || 'unstamped';

const data = JSON.parse(fs.readFileSync(IN, 'utf8'));
let status = {};
try {
  if (fs.existsSync(STATUS_FILE)) {
    const raw = fs.readFileSync(STATUS_FILE, 'utf8').trim();
    if (raw) status = JSON.parse(raw);
  }
} catch (e) { console.error('warn: status file unreadable, ignoring:', e.message); }
const perPaper = data.perPaper || [];
const S = data.synthesis || {};
const ledger = S.ledger || [];
const themes = S.themes || [];
const phases = (S.build_phases || []).slice().sort((a, b) => a.phase - b.phase);

const esc = (s) => String(s == null ? '' : s)
  .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;');

const PAPER_TITLES = {
  P1: 'Lunar signal security', P2: 'Surface-beacon DOP', P3: 'Lunar time is frame-limited',
  P4: 'Real-time frame = EOP prediction', P5: 'Optical/RF hybrid integrity',
  P6: 'Cislunar observability', P7: 'Conflict resilience',
};

const st = (id) => status[id] || 'todo';
const gapById = Object.fromEntries(ledger.map(g => [g.id, g]));

const byPrio = (p) => ledger.filter(g => g.priority === p).length;
const byRepo = (r) => ledger.filter(g => g.repo_target === r).length;
const upgrades = ledger.filter(g => g.is_upgrade_prescription).length;
const doneCount = ledger.filter(g => st(g.id) === 'done').length;
const wipCount = ledger.filter(g => st(g.id) === 'wip').length;
const pct = ledger.length ? Math.round((doneCount / ledger.length) * 100) : 0;

const prioClass = { P0: 'p0', P1: 'p1', P2: 'p2' };
const repoBadge = (r) => r === 'kshana'
  ? '<span class="badge repo-pub">kshana · public</span>'
  : '<span class="badge repo-pro">kshana-pro · private</span>';
const kindBadge = (k) => `<span class="badge kind">${esc(k)}</span>`;
const effChip = (e) => `<span class="eff eff-${esc(e)}">${esc(e)}</span>`;
const statusPill = (s) => {
  const m = { done: ['&#10003; done', 'sp-done'], wip: ['&#9689; in progress', 'sp-wip'], todo: ['&#9675; to build', 'sp-todo'] };
  const [t, c] = m[s] || m.todo;
  return `<span class="spill ${c}">${t}</span>`;
};
const paperChips = (arr) => (arr || []).map(p =>
  `<span class="pchip" title="${esc(PAPER_TITLES[p] || p)}">${esc(p)}</span>`).join('');

const phaseCards = phases.map(ph => {
  const gids = ph.gap_ids || [];
  const gDone = gids.filter(id => st(id) === 'done').length;
  const gWip = gids.filter(id => st(id) === 'wip').length;
  const phState = st('phase' + ph.phase) !== 'todo' ? st('phase' + ph.phase)
    : (gids.length && gDone === gids.length ? 'done' : (gDone || gWip ? 'wip' : 'todo'));
  const ppct = gids.length ? Math.round((gDone / gids.length) * 100) : 0;
  const rows = gids.map(id => {
    const g = gapById[id];
    if (!g) return `<li class="miss">${esc(id)} <span class="dim">(not in ledger)</span></li>`;
    return `<li class="ph-gap ${st(id)}"><a href="#${esc(id)}">${esc(id)}</a> ${statusPill(st(id))} <span class="ph-gap-t">${esc(g.title)}</span> ${effChip(g.effort)}</li>`;
  }).join('');
  return `<div class="phase-card ${phState}" data-status="${phState}">
    <div class="phase-head">
      <div class="phase-no">Phase ${ph.phase}</div>
      <div class="phase-title">${esc(ph.title)}</div>
      <div class="phase-repo">${repoBadge(ph.repo === 'both' ? 'kshana' : ph.repo)}${ph.repo === 'both' ? repoBadge('kshana-pro') : ''}</div>
    </div>
    <div class="phase-goal">${esc(ph.goal)}</div>
    <div class="phase-bar"><div class="phase-bar-fill" style="width:${ppct}%"></div><span class="phase-bar-lab">${gDone}/${gids.length} · ${ppct}%</span></div>
    <div class="phase-rationale">${esc(ph.rationale)}</div>
    <ul class="phase-gaps">${rows}</ul>
  </div>`;
}).join('\n');

const ledgerRows = ledger.map(g => `
  <tr class="lrow" id="${esc(g.id)}" data-repo="${esc(g.repo_target)}" data-prio="${esc(g.priority)}" data-kind="${esc(g.kind)}" data-status="${st(g.id)}">
    <td class="c-id"><span class="gid">${esc(g.id)}</span>${g.is_upgrade_prescription ? '<span class="upg" title="Upgrade prescription from the hostile peer-review audit — highest value">&#9733; upgrade</span>' : ''}</td>
    <td class="c-title"><div class="lt">${esc(g.title)}</div><div class="ld">${esc(g.description)}</div>
      <div class="lmeta"><span class="lo">oracle:</span> ${esc(g.validation_oracle)}</div>
      ${(g.depends_on && g.depends_on.length) ? `<div class="ldep">depends on: ${g.depends_on.map(d => `<a href="#${esc(d)}">${esc(d)}</a>`).join(', ')}</div>` : ''}</td>
    <td class="c-papers">${paperChips(g.serves_papers)}</td>
    <td class="c-repo">${repoBadge(g.repo_target)}<br>${kindBadge(g.kind)}</td>
    <td class="c-prio"><span class="prio ${prioClass[g.priority] || ''}">${esc(g.priority)}</span> ${effChip(g.effort)}</td>
    <td class="c-loc"><code>${esc(g.suggested_location)}</code></td>
    <td class="c-status">${statusPill(st(g.id))}</td>
  </tr>`).join('\n');

const stTag = (s) => {
  const c = s === 'Validated' ? 'v-val' : (s === 'Reproducible' ? 'v-repro' : (s === 'Modelled' ? 'v-mod' : 'v-imp'));
  return `<span class="vtag ${c}">${esc(s)}</span>`;
};
const paperSections = perPaper.map(p => {
  const analyses = (p.analyses || []).map(a => `
    <tr><td class="a-name">${esc(a.name)}</td><td class="a-comp">${esc(a.computes)}</td>
    <td class="a-sec">${esc(a.paper_section)}</td><td class="a-st">${stTag(a.status_in_paper)}</td></tr>`).join('');
  const caps = (p.capabilities_present || []).map(c => `
    <li><span class="cap-ok">&#10003;</span> <b>${esc(c.capability)}</b> <span class="cap-mod">${esc(c.module)}</span>
    <span class="badge ${c.repo === 'kshana' ? 'repo-pub' : 'repo-pro'}">${esc(c.repo)}</span>
    <span class="cap-ev">${esc(c.evidence)}</span></li>`).join('');
  const gaps = (p.verified_gaps || []).map(g => `
    <li class="pg"><a href="#${esc(g.id)}">${esc(g.id)}</a> ${statusPill(st(g.id))}
    <b>${esc(g.title)}</b> <span class="prio ${prioClass[g.priority] || ''}">${esc(g.priority)}</span>
    ${g.is_upgrade_prescription ? '<span class="upg">&#9733;</span>' : ''}
    ${g.verify_verdict ? `<span class="vv vv-${esc(g.verify_verdict)}">${esc(g.verify_verdict)}</span>` : ''}
    <div class="pg-d">${esc(g.description)}</div>
    ${g.verify_evidence ? `<div class="pg-ev"><i>verify:</i> ${esc(g.verify_evidence)}</div>` : ''}</li>`).join('');
  const removed = (p.false_gaps_removed || []).map(f => `
    <li class="fg"><s>${esc(f.gap_id)}</s> — already in <code>${esc(f.existing_module)}</code>: ${esc(f.reason)}</li>`).join('');
  return `<section class="paper" id="${esc(p.paper_id)}-sec">
    <h3><span class="pid">${esc(p.paper_id)}</span> ${esc(p.paper_title)}</h3>
    <p class="thesis">${esc(p.core_thesis)}</p>
    <details open><summary>Analyses in the paper (${(p.analyses || []).length})</summary>
      <table class="atab"><thead><tr><th>Analysis</th><th>Computes</th><th>&sect;</th><th>Paper status</th></tr></thead><tbody>${analyses}</tbody></table>
    </details>
    <details><summary>Capabilities already in Kshana (${(p.capabilities_present || []).length})</summary>
      <ul class="caps">${caps || '<li class="dim">none recorded</li>'}</ul></details>
    <details open><summary>Gaps to build (${(p.verified_gaps || []).length})</summary>
      <ul class="pgaps">${gaps || '<li class="dim">none</li>'}</ul></details>
    ${removed ? `<details><summary>False gaps removed by verification (${(p.false_gaps_removed || []).length})</summary><ul class="fgaps">${removed}</ul></details>` : ''}
  </section>`;
}).join('\n');

const themeCards = themes.map(t => `
  <div class="theme"><div class="theme-h">${esc(t.theme)} ${paperChips(t.papers)}</div>
  <div class="theme-d">${esc(t.description)}</div></div>`).join('');

const html = `<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Kshana &times; PNT Papers — Capability-Gap Buildout Tracker</title>
<style>
:root{--navy:#0c1322;--navy2:#080c17;--panel:#111a2b;--panel2:#0e1626;--gold:#c9a25c;--gold2:#d4b06a;
--cream:#f1ebdd;--muted:#9aa2b2;--dim:#7c8496;--line:rgba(201,162,92,.22);--line2:rgba(201,162,92,.12);
--p0:#e0654f;--p1:#d4b06a;--p2:#6f8fb0;--ok:#5fbf8f;--wip:#d9a441;--pub:#6f8fb0;--pro:#c07ac0;}
*{box-sizing:border-box}
body{margin:0;background:linear-gradient(160deg,var(--navy),var(--navy2));color:var(--cream);
font-family:"Iowan Old Style","Palatino",Georgia,serif;line-height:1.5;font-size:15px}
a{color:var(--gold2);text-decoration:none}a:hover{text-decoration:underline}
code{font-family:Menlo,Consolas,monospace;font-size:.82em;color:var(--gold2);background:rgba(201,162,92,.08);padding:1px 5px;border-radius:4px}
.wrap{max-width:1240px;margin:0 auto;padding:34px 26px 90px}
header.hero{border:1px solid var(--line);border-radius:16px;background:linear-gradient(150deg,rgba(201,162,92,.05),rgba(0,0,0,0));
padding:30px 34px;position:relative;overflow:hidden}
.hero h1{font-family:"Hoefler Text",Georgia,serif;font-weight:600;font-size:30px;margin:0 0 6px;color:var(--cream);letter-spacing:.2px}
.hero .dev{font-family:"Kohinoor Devanagari",serif;color:var(--gold)}
.hero .sub{color:var(--muted);font-size:16px;max-width:860px}
.kicker{font-family:"Helvetica Neue",Arial,sans-serif;text-transform:uppercase;letter-spacing:.3em;font-size:11px;color:var(--gold);margin-bottom:12px}
.hero .stamp{position:absolute;top:20px;right:26px;font-family:Menlo,monospace;font-size:11px;color:var(--dim)}
.prog{margin:22px 0 6px}
.prog-bar{height:14px;border-radius:8px;background:rgba(255,255,255,.05);border:1px solid var(--line2);overflow:hidden;position:relative}
.prog-fill{height:100%;background:linear-gradient(90deg,var(--gold),var(--gold2));width:${pct}%;transition:width .4s}
.prog-lab{font-family:Menlo,monospace;font-size:12px;color:var(--muted);margin-top:6px}
.published{margin:20px 0 4px;border:1px solid var(--line);border-left:3px solid var(--ok);border-radius:12px;padding:14px 18px;background:linear-gradient(180deg,rgba(95,191,143,.06),var(--panel))}
.published .pub-h{font-family:"Hoefler Text",Georgia,serif;font-size:16px;color:var(--ok);margin-bottom:8px}
.published ul{margin:0;padding-left:18px}
.published li{color:var(--cream);font-size:13px;line-height:1.55;margin:4px 0}
.published li b{color:var(--gold2);font-weight:600}
.published a{color:var(--gold2);text-decoration:none;font-family:Menlo,monospace;font-size:12px}
.published a:hover{text-decoration:underline}
.published .pub-map{color:var(--dim);font-size:12px}
.published code{font-family:Menlo,monospace;font-size:11.5px;color:var(--muted)}
.published .pub-note{color:var(--muted);font-size:12px;margin-top:9px;border-top:1px solid var(--line2);padding-top:8px}
.tiles{display:grid;grid-template-columns:repeat(auto-fit,minmax(128px,1fr));gap:12px;margin:22px 0}
.tile{border:1px solid var(--line);border-radius:12px;padding:14px 16px;background:var(--panel)}
.tile .n{font-family:"Hoefler Text",Georgia,serif;font-size:28px;color:var(--gold2);line-height:1}
.tile .l{color:var(--muted);font-size:12px;margin-top:5px;text-transform:uppercase;letter-spacing:.08em}
h2{font-family:"Hoefler Text",Georgia,serif;font-weight:600;color:var(--cream);font-size:21px;
margin:40px 0 8px;padding-bottom:8px;border-bottom:1px solid var(--line2)}
.lead{color:var(--muted);max-width:900px;margin:0 0 14px}
.themes{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:12px}
.theme{border:1px solid var(--line2);border-radius:12px;padding:14px 16px;background:var(--panel2)}
.theme-h{font-weight:600;color:var(--gold2);margin-bottom:6px}
.theme-d{color:var(--muted);font-size:13.5px}
.phases{display:grid;grid-template-columns:repeat(auto-fit,minmax(340px,1fr));gap:14px}
.phase-card{border:1px solid var(--line);border-radius:14px;padding:16px 18px;background:var(--panel);position:relative}
.phase-card.done{border-color:rgba(95,191,143,.5);background:linear-gradient(160deg,rgba(95,191,143,.06),var(--panel))}
.phase-card.wip{border-color:rgba(217,164,65,.5)}
.phase-head{display:flex;align-items:baseline;gap:10px;flex-wrap:wrap}
.phase-no{font-family:Menlo,monospace;font-size:11px;color:var(--gold);text-transform:uppercase;letter-spacing:.15em}
.phase-title{font-family:"Hoefler Text",Georgia,serif;font-size:17px;color:var(--cream);flex:1}
.phase-goal{color:var(--muted);font-size:13.5px;margin:8px 0}
.phase-rationale{color:var(--dim);font-size:12.5px;font-style:italic;margin:6px 0 8px}
.phase-bar{height:9px;border-radius:6px;background:rgba(255,255,255,.05);position:relative;margin:6px 0}
.phase-bar-fill{height:100%;border-radius:6px;background:linear-gradient(90deg,var(--gold),var(--gold2))}
.phase-bar-lab{position:absolute;right:0;top:-17px;font-family:Menlo,monospace;font-size:10.5px;color:var(--dim)}
.phase-gaps{list-style:none;padding:0;margin:6px 0 0}
.phase-gaps li{font-size:12.5px;padding:3px 0;border-top:1px solid var(--line2);display:flex;align-items:center;gap:6px;flex-wrap:wrap}
.ph-gap-t{color:var(--muted);flex:1}
.controls{display:flex;gap:8px;flex-wrap:wrap;margin:14px 0}
.controls button{font-family:"Helvetica Neue",Arial,sans-serif;font-size:12px;color:var(--muted);background:var(--panel);
border:1px solid var(--line2);border-radius:20px;padding:5px 13px;cursor:pointer}
.controls button.on{color:var(--navy);background:var(--gold);border-color:var(--gold);font-weight:600}
table.ledger{width:100%;border-collapse:collapse;font-size:13px}
table.ledger th{text-align:left;color:var(--gold);font-family:"Helvetica Neue",Arial,sans-serif;text-transform:uppercase;
font-size:10.5px;letter-spacing:.1em;padding:8px 8px;border-bottom:1px solid var(--line)}
table.ledger td{padding:11px 8px;border-bottom:1px solid var(--line2);vertical-align:top}
.lrow[data-status=done]{opacity:.6}
.lrow[data-status=done] .lt{text-decoration:line-through}
.gid{font-family:Menlo,monospace;color:var(--gold2);font-weight:600}
.lt{font-weight:600;color:var(--cream)}
.ld{color:var(--muted);font-size:12px;margin-top:3px}
.lmeta{color:var(--dim);font-size:11px;margin-top:4px}.lo{color:var(--gold);opacity:.8}
.ldep{color:var(--dim);font-size:11px;margin-top:3px}
.badge{display:inline-block;font-family:"Helvetica Neue",Arial,sans-serif;font-size:10px;padding:2px 7px;border-radius:5px;
letter-spacing:.03em;white-space:nowrap}
.repo-pub{background:rgba(111,143,176,.16);color:#9db8d6;border:1px solid rgba(111,143,176,.4)}
.repo-pro{background:rgba(192,122,192,.14);color:#d6a6d6;border:1px solid rgba(192,122,192,.4)}
.kind{background:rgba(201,162,92,.1);color:var(--gold2);border:1px solid var(--line2);margin-top:4px}
.prio{font-family:Menlo,monospace;font-weight:700;font-size:11px;padding:2px 6px;border-radius:5px}
.prio.p0{background:rgba(224,101,79,.16);color:#f0917d;border:1px solid rgba(224,101,79,.5)}
.prio.p1{background:rgba(212,176,106,.14);color:var(--gold2);border:1px solid var(--line)}
.prio.p2{background:rgba(111,143,176,.12);color:#9db8d6;border:1px solid rgba(111,143,176,.35)}
.eff{font-family:Menlo,monospace;font-size:10px;padding:1px 5px;border-radius:4px;border:1px solid var(--line2);color:var(--dim)}
.pchip{font-family:Menlo,monospace;font-size:10px;background:rgba(201,162,92,.1);color:var(--gold2);
border:1px solid var(--line2);border-radius:4px;padding:1px 5px;margin:1px}
.upg{font-size:10px;color:#f0c97d;margin-left:6px;white-space:nowrap}
.spill{font-family:"Helvetica Neue",Arial,sans-serif;font-size:10.5px;padding:2px 8px;border-radius:20px;white-space:nowrap;cursor:pointer;user-select:none}
.sp-done{background:rgba(95,191,143,.16);color:var(--ok);border:1px solid rgba(95,191,143,.5)}
.sp-wip{background:rgba(217,164,65,.16);color:var(--wip);border:1px solid rgba(217,164,65,.5)}
.sp-todo{background:rgba(255,255,255,.04);color:var(--dim);border:1px solid var(--line2)}
.paper{border:1px solid var(--line2);border-radius:14px;padding:18px 20px;margin:14px 0;background:var(--panel2)}
.paper h3{font-family:"Hoefler Text",Georgia,serif;font-size:18px;margin:0 0 6px;color:var(--cream)}
.pid{font-family:Menlo,monospace;color:var(--navy);background:var(--gold);padding:1px 8px;border-radius:5px;font-size:14px;margin-right:8px}
.thesis{color:var(--muted);margin:0 0 12px;font-style:italic}
details{margin:8px 0;border-top:1px solid var(--line2);padding-top:6px}
summary{cursor:pointer;color:var(--gold2);font-family:"Helvetica Neue",Arial,sans-serif;font-size:13px;letter-spacing:.03em}
table.atab{width:100%;border-collapse:collapse;font-size:12.5px;margin-top:8px}
table.atab th{text-align:left;color:var(--dim);font-size:10.5px;text-transform:uppercase;letter-spacing:.08em;padding:5px 6px;border-bottom:1px solid var(--line2)}
table.atab td{padding:6px;border-bottom:1px solid var(--line2);color:var(--muted);vertical-align:top}
.a-name{color:var(--cream);font-weight:600;width:22%}
.vtag{font-family:Menlo,monospace;font-size:10px;padding:1px 6px;border-radius:4px}
.v-val{background:rgba(95,191,143,.14);color:var(--ok);border:1px solid rgba(95,191,143,.4)}
.v-mod{background:rgba(217,164,65,.12);color:var(--wip);border:1px solid rgba(217,164,65,.4)}
.v-imp{background:rgba(224,101,79,.12);color:#f0917d;border:1px solid rgba(224,101,79,.4)}
.v-repro{background:rgba(90,150,225,.14);color:#8fb8f0;border:1px solid rgba(90,150,225,.42)}
.caps{list-style:none;padding:0;margin:6px 0}
.caps li{font-size:12.5px;padding:4px 0;color:var(--muted);border-bottom:1px solid var(--line2)}
.cap-ok{color:var(--ok);font-weight:700}.cap-mod{font-family:Menlo,monospace;color:var(--gold2);font-size:11px}
.cap-ev{color:var(--dim);font-size:11px;font-family:Menlo,monospace}
.pgaps,.fgaps{list-style:none;padding:0;margin:6px 0}
.pg{padding:7px 0;border-bottom:1px solid var(--line2)}
.pg-d{color:var(--muted);font-size:12.5px;margin-top:3px}
.pg-ev{color:var(--dim);font-size:11.5px;margin-top:3px}.pg-ev i{color:var(--gold);opacity:.8}
.vv{font-family:Menlo,monospace;font-size:9.5px;padding:1px 5px;border-radius:4px;border:1px solid var(--line2);color:var(--dim)}
.vv-confirmed-missing{color:#f0917d;border-color:rgba(224,101,79,.4)}
.vv-partial-exists{color:var(--wip);border-color:rgba(217,164,65,.4)}
.vv-resolved{color:#7fc99a;border-color:rgba(45,212,191,.45)}
.fg{color:var(--dim);font-size:12px;padding:3px 0}
.dim{color:var(--dim)}
.toc{display:flex;gap:8px;flex-wrap:wrap;margin:8px 0 0}
.toc a{font-family:Menlo,monospace;font-size:12px;background:var(--panel);border:1px solid var(--line2);border-radius:6px;padding:4px 10px}
footer{margin-top:50px;padding-top:16px;border-top:1px solid var(--line2);color:var(--dim);font-size:12px;font-family:Menlo,monospace;display:flex;justify-content:space-between;flex-wrap:wrap;gap:8px}
</style></head>
<body><div class="wrap">
<header class="hero">
  <div class="stamp">generated ${esc(STAMP)}</div>
  <div class="kicker">Ashforde · Kshana <span class="dev">&#2325;&#2381;&#2359;&#2339;</span> · Build-in-public PNT program</div>
  <h1>Kshana &times; Lunar/Cislunar PNT Papers — Capability-Gap Buildout Tracker</h1>
  <div class="sub">Everything the seven preprints (P1&ndash;P7) compute or claim, mapped to the Kshana codebase, with the exact capabilities to build so each result is producible — and upgradable from <i>Modelled/asserted</i> to Kshana-computed <i>Validated</i>. Follow this during implementation; tick items as they land.</div>
  <div class="prog"><div class="prog-bar"><div class="prog-fill"></div></div>
    <div class="prog-lab" id="proglab">${doneCount}/${ledger.length} gaps complete · ${pct}% · ${wipCount} in progress</div></div>
</header>

<section class="published">
  <div class="pub-h">&#128220; Published on arXiv &mdash; program output now peer-visible</div>
  <ul>
    <li><a href="https://arxiv.org/abs/2607.06212">arXiv:2607.06212</a> &middot; <b>The Cost of Lunar South-Polar Geometry, and Surface Beacons as the Efficient Fix: A Dilution-of-Precision Analysis</b> <span class="pub-map">(P2 &middot; surface-beacon DOP &rarr; <code>src/lunar_beacon.rs</code>)</span> &middot; 7 Jul 2026</li>
    <li><a href="https://arxiv.org/abs/2607.02566">arXiv:2607.02566</a> &middot; <b>Earth-baseline VLBI restores the observability of a lunar surface station in joint orbit-and-clock determination</b> <span class="pub-map">(lunar joint OD+clock observability &rarr; <code>lunar_vlbi.rs</code> &middot; <code>lunar_combination.rs</code>; cites kshana v0.23.0)</span> &middot; 1 Jul 2026</li>
    <li><a href="https://arxiv.org/abs/2606.24210">arXiv:2606.24210</a> &middot; <b>A Conditional Timing Protection Level: Holdover-Limited Undetected Time Error Under GNSS Spoofing</b> <span class="pub-map">(TPL &rarr; <code>src/tpl.rs</code>; JammerTest 2024)</span> &middot; 23 Jun 2026</li>
    <li><a href="https://arxiv.org/abs/2606.22054">arXiv:2606.22054</a> &middot; <b>Anticipating the Optimism Gap: Predicting Distribution-Shift Degradation of RF-Impairment Detectors from In-Distribution Statistics</b> <span class="pub-map">(optimism-gap study &rarr; <code>impairment_study.rs</code>)</span> &middot; 20 Jun 2026</li>
  </ul>
  <div class="pub-note">All four cite the open <a href="https://github.com/ashfordeOU/kshana">kshana</a> engine; every headline figure regenerates from a committed scenario + seed. Papers stay <b>arXiv-first</b> &mdash; sources live in <code>papers/</code>, nothing else public until founder announce.</div>
</section>

<div class="tiles">
  <div class="tile"><div class="n">${ledger.length}</div><div class="l">Total gaps</div></div>
  <div class="tile"><div class="n" style="color:var(--p0)">${byPrio('P0')}</div><div class="l">P0 critical</div></div>
  <div class="tile"><div class="n">${byPrio('P1')}</div><div class="l">P1 strengthen</div></div>
  <div class="tile"><div class="n">${byPrio('P2')}</div><div class="l">P2 nice-to-have</div></div>
  <div class="tile"><div class="n" style="color:var(--pub)">${byRepo('kshana')}</div><div class="l">public kshana</div></div>
  <div class="tile"><div class="n" style="color:var(--pro)">${byRepo('kshana-pro')}</div><div class="l">kshana-pro</div></div>
  <div class="tile"><div class="n" style="color:#f0c97d">${upgrades}</div><div class="l">&#9733; upgrade fixes</div></div>
  <div class="tile"><div class="n">${phases.length}</div><div class="l">build phases</div></div>
</div>

<h2>Executive summary</h2>
<p class="lead">${esc(S.summary || 'Pending synthesis.')}</p>

<h2>Cross-cutting themes</h2>
<div class="themes">${themeCards || '<p class="dim">none</p>'}</div>

<h2>Build-phase roadmap</h2>
<p class="lead">Ordered, independently shippable phases. Each phase is a clean commit + push to one repo where possible; dependencies flow P0 core analyses &rarr; packs/studies &rarr; reports/fixtures.</p>
<div class="phases">${phaseCards || '<p class="dim">none</p>'}</div>

<h2>Master gap ledger</h2>
<p class="lead">Deduped across all seven papers. <b>Click a status pill</b> to cycle to-build &rarr; in-progress &rarr; done (saved in your browser). &#9733; = the hostile-peer-review upgrade prescription for a paper (highest value).</p>
<div class="controls" id="controls">
  <button data-f="all" class="on">All (${ledger.length})</button>
  <button data-f="repo:kshana">public kshana (${byRepo('kshana')})</button>
  <button data-f="repo:kshana-pro">kshana-pro (${byRepo('kshana-pro')})</button>
  <button data-f="prio:P0">P0 (${byPrio('P0')})</button>
  <button data-f="prio:P1">P1 (${byPrio('P1')})</button>
  <button data-f="upg">&#9733; upgrades (${upgrades})</button>
  <button data-f="todo">to build</button>
</div>
<table class="ledger"><thead><tr><th>ID</th><th>Capability</th><th>Papers</th><th>Repo · kind</th><th>Prio · effort</th><th>Location</th><th>Status</th></tr></thead>
<tbody>${ledgerRows}</tbody></table>

<h2>Per-paper detail</h2>
<div class="toc">${perPaper.map(p => `<a href="#${esc(p.paper_id)}-sec">${esc(p.paper_id)} · ${esc(PAPER_TITLES[p.paper_id] || '')}</a>`).join('')}</div>
${paperSections}

<footer>
  <span>Ashforde OÜ · Kshana &#2325;&#2381;&#2359;&#2339; open PNT program · for the missions that cannot fail</span>
  <span>kshana.dev · ashforde.org</span>
</footer>
</div>
<script>
(function(){
  var KEY='kshana-pnt-gap-status-v1';
  var saved={};try{saved=JSON.parse(localStorage.getItem(KEY)||'{}')}catch(e){}
  function setPill(pill,s){pill.className='spill '+(s==='done'?'sp-done':s==='wip'?'sp-wip':'sp-todo');
    pill.textContent=s==='done'?'✓ done':s==='wip'?'◙ in progress':'○ to build';}
  Object.keys(saved).forEach(function(id){
    var row=document.getElementById(id); if(row){row.setAttribute('data-status',saved[id]);
      var pill=row.querySelector('.c-status .spill'); if(pill) setPill(pill,saved[id]);}});
  var order={todo:'wip',wip:'done',done:'todo'};
  document.querySelectorAll('.c-status .spill').forEach(function(pill){
    pill.addEventListener('click',function(){
      var row=pill.closest('tr'); var id=row.id; var cur=row.getAttribute('data-status')||'todo';
      var nx=order[cur]; row.setAttribute('data-status',nx); setPill(pill,nx);
      saved[id]=nx; localStorage.setItem(KEY,JSON.stringify(saved)); recompute();
    });
  });
  function recompute(){
    var rows=document.querySelectorAll('table.ledger .lrow'); var d=0,w=0,t=rows.length;
    rows.forEach(function(r){var s=r.getAttribute('data-status'); if(s==='done')d++; else if(s==='wip')w++;});
    var pc=t?Math.round(d/t*100):0;
    document.querySelector('.prog-fill').style.width=pc+'%';
    document.getElementById('proglab').textContent=d+'/'+t+' gaps complete · '+pc+'% · '+w+' in progress';
  }
  var cur='all';
  document.querySelectorAll('#controls button').forEach(function(b){
    b.addEventListener('click',function(){
      document.querySelectorAll('#controls button').forEach(function(x){x.classList.remove('on')});
      b.classList.add('on'); cur=b.getAttribute('data-f');
      document.querySelectorAll('table.ledger .lrow').forEach(function(r){
        var show=true;
        if(cur==='all')show=true;
        else if(cur.indexOf('repo:')===0)show=r.getAttribute('data-repo')===cur.slice(5);
        else if(cur.indexOf('prio:')===0)show=r.getAttribute('data-prio')===cur.slice(5);
        else if(cur==='upg')show=!!r.querySelector('.upg');
        else if(cur==='todo')show=r.getAttribute('data-status')!=='done';
        r.style.display=show?'':'none';
      });
    });
  });
  recompute();
})();
</script>
</body></html>`;

fs.writeFileSync(OUT, html);
console.log('wrote', OUT, '(' + (html.length / 1024).toFixed(1) + ' KB)');
console.log('ledger items:', ledger.length, '| phases:', phases.length, '| papers:', perPaper.length);
