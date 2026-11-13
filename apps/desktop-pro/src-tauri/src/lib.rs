// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Kshana Desktop Pro: the Studio in a native window with the Pro overlay, unlocked by
//! the Desktop licence; the Pro home and licence screen in `pro/desktop/` (0.42 U3).

pub mod bridge;

use serde_json::Value;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
async fn engine_call(name: String, args: Vec<Value>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let t = now();
        let ent = bridge::store()
            .map(|s| bridge::status_in(&s, t).entitlement)
            .unwrap_or_else(|_| kshana_pro::license::Entitlement::evaluation());
        bridge::call(&name, &args, &ent, t)
    })
    .await
    .map_err(|e| format!("the run stopped: {e}"))?
}

#[tauri::command]
fn engine_native_fns() -> Vec<&'static str> {
    bridge::NATIVE_FNS.to_vec()
}

#[tauri::command]
fn licence_status() -> Result<Value, String> {
    let s = bridge::store()?;
    Ok(bridge::status_json(&bridge::status_in(&s, now())))
}

#[tauri::command]
fn licence_activate(token: String) -> Result<Value, String> {
    bridge::activate_in(&bridge::store()?, &token, now())
}

#[tauri::command]
fn licence_deactivate() -> Result<bool, String> {
    bridge::store()?.deactivate().map_err(|e| e.to_string())
}

#[tauri::command]
fn licence_machine_id() -> Option<String> {
    kshana_pro::license::machine::this_machine_hash()
}

#[tauri::command]
fn upgrade_offer(edition: String) -> Value {
    bridge::upgrade_offer(&edition, "", "")
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            engine_call,
            engine_native_fns,
            licence_status,
            licence_activate,
            licence_deactivate,
            licence_machine_id,
            upgrade_offer
        ])
        .run(tauri::generate_context!())
        .expect("Kshana Desktop Pro could not start");
}
