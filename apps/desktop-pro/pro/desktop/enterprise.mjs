// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro, Enterprise (0.42 U28): sign-in to the organisation's Kshana Pro service
// (single sign-on token or API key; the service maps the person to a tenant and a role, and
// SCIM deprovisioning takes effect at the next request), the team library (the tenant's
// saved studies), the audit log (GET /audit/export) and always-on watch walls
// (GET /wall/state). Pure models of the service's answers; tested in enterprise.test.mjs.

export const ROLES = ["reader", "submitter", "admin"];
const ROLE_TEXT = {
  reader: "can read jobs, results, the fleet and the wall",
  submitter: "can also run jobs and save studies to the team library",
  admin: "can also read the audit log and the delivery ledger and provision people",
};

/** The signed-in principal (GET /whoami) as the account card draws it. */
export function accountModel(me, connection) {
  if (!me) return { signedIn: false, text: connection && connection.connected ? "Connected; not signed in (the service refused the credential)." : "Not connected to a Kshana Pro service." };
  const ent = Array.isArray(me.enterprise) ? me.enterprise : [];
  return {
    signedIn: true, id: String(me.id), tenant: String(me.tenant), role: String(me.role), source: me.source === "oidc" ? "single sign-on" : me.source === "key" ? "API key" : String(me.source),
    roleText: ROLE_TEXT[me.role] || "", edition: String(me.edition || ""),
    can: { submit: ROLES.indexOf(me.role) >= 1, admin: me.role === "admin", wall: ent.includes("pro-watch-wall"), audit: me.role === "admin" && ent.includes("pro-audit-export"), library: true },
    enterprise: ent,
    base: connection && connection.base ? String(connection.base) : "",
  };
}

/** The team library (GET /library), newest first. */
export function libraryModel(list) {
  const arr = Array.isArray(list) ? list : (list && (list.studies || list.saved)) || [];
  return arr.map((s) => ({ name: String(s.name), op: String(s.op), savedBy: String(s.saved_by || ""), saved: String(s.saved_utc || ""), kind: s.request && typeof s.request.scenario === "string" ? (s.request.scenario.match(/kind\s*=\s*"([^"]+)"/) || [])[1] || "" : "" }))
    .sort((a, b) => b.saved.localeCompare(a.saved) || a.name.localeCompare(b.name));
}

/** The audit export (kshana-pro-audit-export/1) as a table, newest first, with the chain state. */
export function auditModel(ex, { route = "", who = "" } = {}) {
  if (!ex || ex.format !== "kshana-pro-audit-export/1") throw new Error("not an audit export");
  const rows = (ex.lines || [])
    .filter((l) => (!route || String(l.route).includes(route)) && (!who || String(l.who).includes(who)))
    .map((l) => ({ t: l.t, who: l.who, tenant: l.tenant, role: l.role, source: l.source, method: l.method, route: l.route, status: l.status, ms: l.ms, job: l.job || "", sha256: l.sha256, refused: l.status >= 400 }))
    .sort((a, b) => b.t - a.t);
  const ch = ex.chain || {};
  return { rows, count: ex.count ?? rows.length, refused: rows.filter((r) => r.refused).length, chainOk: ch.verified === true, chainText: ch.verified ? `Chain verified over ${ch.lines} lines in ${ch.files} day files.` : `Chain broken at ${ch.first_bad || "an unknown line"}.`, sha256: String(ex.lines_sha256 || "") };
}

/** CSV of the audit rows, for a SIEM import. */
export function auditCsv(m) {
  const q = (v) => { const s = String(v ?? ""); return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s; };
  return ["t,who,tenant,role,source,method,route,status,ms,job,sha256", ...m.rows.map((r) => [r.t, r.who, r.tenant, r.role, r.source, r.method, r.route, r.status, r.ms, r.job, r.sha256].map(q).join(","))].join("\n") + "\n";
}

/** The watch wall (kshana-pro-watch-wall/1): tiles worst first, and whether to alarm. */
export function wallModel(w) {
  if (!w || w.format !== "kshana-pro-watch-wall/1") throw new Error("not a watch wall");
  const tone = (s) => ({ live: "ok", TRUSTED: "ok", ok: "ok", stale: "bad", UNTRUSTED: "bad", act: "bad", breached: "bad", DEGRADED: "warn", watch: "warn", registered: "warn" })[s] || "none";
  const tiles = [
    ...((w.fleet && w.fleet.stations) || []).map((s) => ({ group: "Station", name: s.name, state: s.state, tone: tone(s.state) })),
    ...(w.area && w.area.configured && w.area.areas ? w.area.areas.map((a) => ({ group: "Area", name: a.name, state: a.current, tone: tone(a.current) })) : []),
    ...(w.timing && w.timing.configured && w.timing.clocks ? w.timing.clocks.map((c) => ({ group: "Clock", name: c.id, state: c.status, tone: tone(c.status) })) : []),
  ];
  const rank = { bad: 0, warn: 1, none: 2, ok: 3 };
  tiles.sort((a, b) => rank[a.tone] - rank[b.tone] || a.name.localeCompare(b.name));
  return { tenant: w.tenant, at: w.at_unix, alarm: !!w.alarm, tiles, jobs: w.jobs || {}, notConfigured: [w.area && !w.area.configured ? "area console" : null, w.timing && !w.timing.configured ? "timing fleet" : null].filter(Boolean) };
}
