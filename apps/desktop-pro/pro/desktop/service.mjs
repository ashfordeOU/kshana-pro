// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: the app's Pro functions (`engine_call` names starting `pro_`, served by
// the Pro crate's desktop module through the bridge) as the Pro views use them. Outside the
// app there is no invoke function and every call says so. Each call returns { ok, value, error }.
// The service credential is held by the app (owner-only file); the page never sees it.

const msg = (e) => String(e && e.message ? e.message : e);
const NOT_IN_APP = "This page is not running inside Kshana Desktop Pro.";

/** The Pro functions over `invoke` (null: outside the app). */
export function proApi(invoke) {
  const call = async (name, args = []) => {
    if (!invoke) return { ok: false, value: null, error: NOT_IN_APP };
    try {
      return { ok: true, value: await invoke("engine_call", { name, args }), error: "" };
    } catch (e) {
      return { ok: false, value: null, error: msg(e) };
    }
  };
  /** One request to the connected service: { ok, value: { status, json | text }, error }; a non-2xx status is an error with the server's reason. */
  const req = async (method, path, body) => {
    const r = await call("pro_service", body === undefined ? [method, path] : [method, path, typeof body === "string" ? body : JSON.stringify(body)]);
    if (!r.ok) return r;
    const st = Number(r.value && r.value.status);
    if (st >= 200 && st < 300) return { ok: true, value: r.value.json !== undefined ? r.value.json : r.value.text, status: st, error: "" };
    const why = r.value && r.value.json && (r.value.json.error || r.value.json.detail);
    return { ok: false, value: r.value.json ?? null, status: st, error: `${st}: ${why || "the service refused the request"}` };
  };
  return {
    inApp: !!invoke,
    verifyPack: (dir, trust = "") => call("pro_pack_verify", [dir, trust]),
    replay: (dir) => call("pro_incident_replay", [dir]),
    signFiles: (dir, files, signer) => call("pro_sign_files", [dir, files, signer]),
    signPack: (dir, signer) => call("pro_sign_pack", [dir, signer]),
    evidenceKey: () => call("pro_evidence_key"),
    connect: (base, credential) => call("pro_service_connect", [base, credential]),
    connection: () => call("pro_service_status"),
    disconnect: () => call("pro_service_disconnect"),
    get: (path) => req("GET", path),
    post: (path, body = "") => req("POST", path, body),
    patch: (path, body) => req("PATCH", path, body),
    del: (path) => req("DELETE", path),
  };
}

/** Days as YYYY-MM-DD (UTC) for a Unix time. */
export function dayOf(unix) {
  return new Date(unix * 1000).toISOString().slice(0, 10);
}

/** The audit export path for the last `days` days ending at `now` (Unix seconds). */
export function auditPath(now, days = 7) {
  return `/audit/export?since=${dayOf(now - (days - 1) * 86400)}&until=${dayOf(now)}`;
}
