// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// Kshana Desktop Pro: the app's own commands (src-tauri/src/lib.rs) as the Pro home uses
// them. Outside the app there is no invoke function and every read says so; nothing here
// stands in a licensed state for a missing answer.

/** The app's invoke function, or null outside Kshana Desktop Pro. */
export function appInvoke(g = globalThis) {
  const i = g && g.__TAURI_INTERNALS__;
  return i && typeof i.invoke === "function" ? (cmd, args) => i.invoke(cmd, args) : null;
}

const msg = (e) => String(e && e.message ? e.message : e);

/** The licence API over `invoke` (null: outside the app). Each read returns { ok, value, error }. */
export function licenceApi(invoke) {
  const none = { ok: false, value: null, error: "This page is not running inside Kshana Desktop Pro, so there is no licence to read." };
  const call = async (cmd, args) => {
    if (!invoke) return none;
    try { return { ok: true, value: await invoke(cmd, args), error: "" }; } catch (e) { return { ok: false, value: null, error: msg(e) }; }
  };
  return {
    inApp: !!invoke,
    status: () => call("licence_status"),
    activate: (token) => call("licence_activate", { token }),
    deactivate: () => call("licence_deactivate"),
    machineId: () => call("licence_machine_id"),
    upgradeOffer: () => call("upgrade_offer", { edition: "pro-annual" }),
    /** The kinds this build and licence run (engine_call list_kinds; a JSON string). */
    listKinds: () => call("engine_call", { name: "list_kinds", args: [] }),
    version: () => call("engine_call", { name: "version", args: [] }),
  };
}
