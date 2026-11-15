// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: review mode and the report builder (0.42 U27).
//
// Review mode walks a reviewer through every finding of an opened pack (incident forensics
// replay, or a receiver scorecard's events): accept, dispute with a note, or leave pending.
// A review is complete when nothing is pending and every dispute has a note. It is the
// reviewer's working record, signed as a pack with this device's evidence key; it is not
// the claim-grade expert review of `kshana-pro forensics review`, and says so.
//
// The report builder assembles the sections the user picks from what is open (the replay,
// the review, a scorecard, the area picture) into one HTML page and its JSON record, every
// text escaped, every label (MODELLED, MEASURED, SYNTHETIC) and limit carried as given, and
// hands them to the app to sign. Pure; tested in review.test.mjs.
import { utcText } from "./geo.mjs";

export const REVIEW_FORMAT = "kshana-pro-desktop-review/1";
export const REPORT_FORMAT = "kshana-pro-desktop-report/1";
export const DECISIONS = ["pending", "accepted", "disputed"];

/** A fresh review of `items` ([{ id, title, detail }]). */
export function newReview(subject, items) {
  return { subject: { kind: subject.kind, id: String(subject.id || ""), title: String(subject.title || "") }, items: items.map((i) => ({ id: String(i.id), title: String(i.title), detail: String(i.detail || ""), decision: "pending", note: "" })) };
}

/** The review items of a replay model (one per finding). */
export function replayItems(m) {
  return m.episodes.map((e, i) => ({ id: e.id || `finding-${i + 1}`, title: `${e.category} (${e.confidence}) · ${e.source}`, detail: `${utcText(e.start)} to ${utcText(e.end)}. ${e.statement}`.trim() }));
}

/** The review items of a receiver scorecard (one per case event). */
export function scorecardItems(card) {
  return (card.cases || []).flatMap((c) => (c.events || []).map((e, k) => ({ id: `${c.id}#${k + 1}`, title: `${c.title}: ${e.label || e.kind}`, detail: `monitor ${e.monitor_outcome}${Number.isFinite(e.monitor_latency_s) ? ` after ${e.monitor_latency_s} s` : ""}; max error ${e.max_error_m} m; recovered ${e.recovered ? "yes" : "no"}` })));
}

/** `review` with item `id` decided. A dispute needs a note; anything else is refused. */
export function decide(review, id, decision, note = "") {
  if (!DECISIONS.includes(decision)) throw new Error(`unknown decision ${decision}`);
  if (decision === "disputed" && !String(note).trim()) throw new Error("a dispute needs a note saying why");
  if (!review.items.some((i) => i.id === id)) throw new Error(`no item ${id}`);
  return { ...review, items: review.items.map((i) => (i.id === id ? { ...i, decision, note: String(note).trim() } : i)) };
}

/** Counts and whether the review is complete. */
export function summary(review) {
  const n = (d) => review.items.filter((i) => i.decision === d).length;
  return { total: review.items.length, pending: n("pending"), accepted: n("accepted"), disputed: n("disputed"), complete: review.items.length > 0 && n("pending") === 0 };
}

/** The review record to sign. Refused while incomplete or without a reviewer. */
export function reviewRecord(review, { reviewer, at }) {
  if (!String(reviewer || "").trim()) throw new Error("name the reviewer");
  const s = summary(review);
  if (!s.complete) throw new Error(`${s.pending} item(s) still pending`);
  return {
    format: REVIEW_FORMAT, reviewer: String(reviewer).trim(), reviewed_utc: new Date(at * 1000).toISOString().replace(/\.\d+Z$/, "Z"),
    subject: review.subject, summary: s, items: review.items,
    note: "A reviewer's working record made in Kshana Desktop Pro. It is not the claim-grade expert review of an incident forensics pack.",
  };
}

// ------------------------------------------------------------------ report builder
export const esc = (s) => String(s ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The sections the builder offers for what is open (`src`: { replay, review, scorecard, area }). */
export function availableSections(src) {
  const S = [];
  if (src.replay) S.push(["verdict", "Verdict and findings"], ["verification", "Pack verification"]);
  if (src.scorecard) S.push(["scorecard", "Receiver scorecard"]);
  if (src.area) S.push(["area", "Area trust"]);
  if (src.review) S.push(["review", "Review decisions"]);
  if (src.replay || src.scorecard || src.area) S.push(["limits", "Limits and labels"]);
  return S.map(([id, title]) => ({ id, title }));
}

const table = (head, rows) => `<table><thead><tr>${head.map((x) => `<th>${esc(x)}</th>`).join("")}</tr></thead><tbody>${rows.map((r) => `<tr>${r.map((x) => `<td>${esc(x)}</td>`).join("")}</tr>`).join("")}</tbody></table>`;

/**
 * Build the report: { html, json }. `sections`: ids from availableSections, in the order given.
 * Every value is escaped; nothing is added that the sources do not hold.
 */
export function buildReport(src, sections, { title, author, at }) {
  if (!String(title || "").trim()) throw new Error("give the report a title");
  if (!sections.length) throw new Error("pick at least one section");
  const known = new Set(availableSections(src).map((s) => s.id));
  for (const s of sections) if (!known.has(s)) throw new Error(`section ${s} has nothing open to draw from`);
  const parts = [], rec = { format: REPORT_FORMAT, title: String(title).trim(), author: String(author || "").trim(), created_utc: new Date(at * 1000).toISOString().replace(/\.\d+Z$/, "Z"), sections: {} };
  const r = src.replay, c = src.scorecard, a = src.area, v = src.review;
  for (const s of sections) {
    if (s === "verdict") {
      const jam = r.verdict.jamming || {}, spf = r.verdict.spoofing || {};
      parts.push(`<h2>Verdict and findings</h2><p>${esc(r.verdict.sentence)}</p><p>Jamming: ${esc(jam.status)}${jam.confidence ? ` (${esc(jam.confidence)})` : ""}. Spoofing: ${esc(spf.status)}${spf.confidence ? ` (${esc(spf.confidence)})` : ""}.</p>`
        + table(["Finding", "Confidence", "Source", "From", "To"], r.episodes.map((e) => [e.category, e.confidence, e.source, utcText(e.start), utcText(e.end)])));
      rec.sections.verdict = { case_id: r.caseId, verdict: r.verdict, findings: r.episodes };
    } else if (s === "verification") {
      parts.push(`<h2>Pack verification</h2><p>${r.verified ? "The pack's signature and every file verified" : "The pack did NOT verify"}${r.trusted ? " under the pinned issuer key" : ""}. Signer: ${esc(r.signer)} (key ${esc(r.keyId)}).</p>${r.problems.length ? `<ul>${r.problems.map((p) => `<li>${esc(p)}</li>`).join("")}</ul>` : ""}`);
      rec.sections.verification = { verified: r.verified, trusted: r.trusted, signer: r.signer, key_id: r.keyId, problems: r.problems };
    } else if (s === "scorecard") {
      parts.push(`<h2>Receiver scorecard</h2><p>${esc([c.receiver.maker, c.receiver.model, c.receiver.firmware].filter(Boolean).join(" "))} · campaign ${esc(c.campaign.id)}</p>`
        + table(["Level", "Summary", "Evidence", "Cases met", "Not met"], c.levels.map((l) => [l.level, l.summary, l.evidence, l.met, l.notMet]))
        + `<p>${esc(c.wording)}</p>`);
      rec.sections.scorecard = { receiver: c.receiver, campaign: c.campaign, levels: c.levels, totals: c.totals };
    } else if (s === "area") {
      parts.push(`<h2>Area trust</h2><p>${esc(a.positions)}</p>` + table(["Area", "Now", "Events"], a.areas.map((x) => [x.name, x.current, x.events.length])));
      rec.sections.area = { title: a.title, areas: a.areas.map((x) => ({ id: x.id, current: x.current, events: x.events.length })) };
    } else if (s === "review") {
      const sm = summary(v);
      parts.push(`<h2>Review decisions</h2><p>${sm.accepted} accepted, ${sm.disputed} disputed, ${sm.pending} pending.</p>` + table(["Item", "Decision", "Note"], v.items.map((i) => [i.title, i.decision, i.note])));
      rec.sections.review = { summary: sm, items: v.items };
    } else if (s === "limits") {
      const lim = [...(r ? r.limits : []), ...(c ? c.limits : [])];
      const labels = [r && r.label, c && c.label].filter(Boolean);
      parts.push(`<h2>Limits and labels</h2>${labels.map((l) => `<p>${esc(l)}</p>`).join("")}<ul>${lim.map((l) => `<li>${esc(l)}</li>`).join("")}</ul>`);
      rec.sections.limits = { labels, limits: lim };
    }
  }
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${esc(rec.title)}</title><style>body{font:14px/1.5 system-ui,sans-serif;max-width:900px;margin:24px auto;padding:0 16px;color:#111}table{border-collapse:collapse;width:100%;font-size:13px}th,td{border-bottom:1px solid #ddd;padding:4px 6px;text-align:left;vertical-align:top}h1{font-size:22px}h2{font-size:16px;margin-top:24px}.meta{color:#555}</style></head><body><h1>${esc(rec.title)}</h1><p class="meta">${esc(rec.author ? `Prepared by ${rec.author}. ` : "")}Made with Kshana Desktop Pro on ${esc(utcText(at))}. Signed as a pack: verify it with kshana-pro dossier verify or scripts/verify-dossier-signature.py.</p>${parts.join("\n")}</body></html>\n`;
  return { html, json: JSON.stringify(rec, null, 2) + "\n" };
}
