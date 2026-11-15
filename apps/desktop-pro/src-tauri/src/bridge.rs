// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! The Pro Desktop's engine bridge and licence (roadmap F10e).
//!
//! The command names are the free Desktop's (`engine_call`, `engine_native_fns`), so the
//! Studio's Desktop client is unchanged. A scenario of a Pro kind runs through the Pro
//! registry under the Desktop licence (`kshana_pro::registry::run_gated`); every other
//! scenario runs on the open engine exactly as in the free Desktop. The licence is the
//! token activated in the licence store for the `desktop` product, checked offline
//! ([`kshana_pro::license::activation`]).

use std::path::PathBuf;

use kshana_pro::license::activation::{State, Status, Store};
use kshana_pro::license::{Entitlement, Keyring, Product, VerifyOptions};
use kshana_pro::payments::paywall::{self, Paywall};
use kshana_pro::payments::PaymentsConfig;
use serde_json::{json, Value};

/// The functions this build runs natively (the free Desktop's set).
pub const NATIVE_FNS: &[&str] = &[
    "run",
    "run_all",
    "summary",
    "chart_svg",
    "table_csv",
    "run_human",
    "register_catalogue",
    "list_kinds",
    "version",
];

/// The version of the embedded open engine (the `kshana` pin); a test checks it against
/// the engine's own result documents.
pub const ENGINE_VERSION: &str = "0.41.0";

/// The keys this build verifies licences with: those compiled in, or in a debug build
/// without any, the demo key of the Pro debug builds.
pub fn keyring() -> Result<Keyring, String> {
    if let Some(k) = Keyring::embedded() {
        return k.map_err(|e| e.to_string());
    }
    if cfg!(debug_assertions) {
        Ok(Keyring::single(kshana_pro::license::public_key_for(
            b"kshana-pro-demo-key-debug-only!!",
        )))
    } else {
        Err("this build has no licence key; it runs the open features only".into())
    }
}

#[cfg(test)]
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn machine() -> Option<String> {
    kshana_pro::license::machine::this_machine_hash()
}

/// The licence store (`$KSHANA_PRO_HOME` or the per-user configuration directory).
pub fn store() -> Result<Store, String> {
    Store::default_dir()
        .map(Store::open)
        .ok_or_else(|| "no licence store directory (set KSHANA_PRO_HOME)".to_string())
}

fn opts(at: i64) -> VerifyOptions<'static> {
    VerifyOptions {
        now_unix: at,
        product: Some(Product::Desktop),
        machine: Some(&machine),
        allow_expired: false,
    }
}

/// The licence status in `store` at `at`.
pub fn status_in(store: &Store, at: i64) -> Status {
    match keyring() {
        Ok(ring) => store.status(&ring, &[], &opts(at)),
        Err(e) => Status {
            state: State::Invalid(e.clone()),
            licence: None,
            entitlement: Entitlement::evaluation(),
            warning: Some(e),
            pro_ends_unix: 0,
        },
    }
}

/// The licence as the licence page shows it.
pub fn status_json(st: &Status) -> Value {
    let l = st.licence.as_ref();
    json!({
        "state": st.state.as_str(),
        "tier": st.entitlement.tier.as_str(),
        "licensee": l.map(|l| l.claims.licensee.clone()),
        "expires_unix": l.map(|l| l.claims.expires_unix),
        "products": l.map(kshana_pro::license::activation::products_covered),
        "seats": l.map(|l| l.claims.seats),
        // The licence's own tier (the entitlement above falls to Evaluation once it lapses),
        // the Stations it allows and whether it is bound to one machine: for the Pro home.
        "licence_tier": l.map(|l| l.claims.tier.clone()),
        "stations": l.map(|l| l.claims.stations),
        "machine_bound": l.map(|l| l.claims.machine.is_some()),
        "token_id": l.map(|l| l.token_id.clone()),
        "warning": st.warning,
    })
}

/// Activate `token` in `store` for this machine's Desktop.
pub fn activate_in(store: &Store, token: &str, at: i64) -> Result<Value, String> {
    let ring = keyring()?;
    let st = store
        .activate(&ring, token, &opts(at))
        .map_err(|e| e.to_string())?;
    Ok(status_json(&st))
}

/// Call engine function `name` with the page's `args` at entitlement `ent`.
pub fn call(name: &str, args: &[Value], ent: &Entitlement, at: i64) -> Result<Value, String> {
    let text = |i: usize| -> Result<&str, String> {
        args.get(i)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("argument {} must be text", i + 1))
    };
    let lang = args
        .get(1)
        .and_then(Value::as_str)
        .unwrap_or("en")
        .to_string();
    let run = |src: &str| -> Result<kshana::api::RunOutput, String> {
        match kshana_pro::registry::peek_kind(src) {
            Some(k) if k.starts_with("pro-") => {
                kshana_pro::registry::run_gated(src, ent, at).map_err(|e| e.to_string())
            }
            _ => kshana::api::run_toml_lang(src, &lang),
        }
    };
    match name {
        "run" => run(text(0)?).map(|o| json!(o.json)),
        "chart_svg" => run(text(0)?).map(|o| json!(o.svg)),
        "summary" => run(text(0)?).map(|o| json!(o.summary)),
        "table_csv" => run(text(0)?).map(|o| json!(o.csv)),
        "run_all" => run(text(0)?).map(|o| {
            json!(
                json!({"json": o.json, "svg": o.svg, "summary": o.summary, "csv": o.csv})
                    .to_string()
            )
        }),
        "run_human" => {
            let src = text(0)?;
            if kshana_pro::registry::peek_kind(src).is_some_and(|k| k.starts_with("pro-")) {
                run(src).map(|o| json!({"summary": o.summary, "svg": o.svg}))
            } else {
                kshana::api::run_toml_human(src, &lang)
                    .map(|h| json!({"summary": h.summary, "svg": h.svg}))
            }
        }
        "register_catalogue" => {
            kshana::i18n::register_catalogue(text(0)?, text(1)?).map(|()| Value::Null)
        }
        // The open kinds and the Pro kinds this licence may run.
        "list_kinds" => {
            let mut kinds: Vec<Value> =
                serde_json::from_str(&kshana::api::list_scenario_kinds_json())
                    .map_err(|e| e.to_string())?;
            for m in kshana_pro::registry::pro_kinds() {
                if ent.allows(m.name, at) {
                    kinds.push(json!({"name": m.name, "description": m.description}));
                }
            }
            Ok(json!(Value::Array(kinds).to_string()))
        }
        "version" => Ok(json!(ENGINE_VERSION)),
        other => Err(format!(
            "{other} is not a native engine function of this build"
        )),
    }
}

/// What "Upgrade" offers: nothing while payments are off in this build (no checkout, no
/// network), else the checkout for `edition`.
pub fn upgrade_offer(edition: &str, licensee: &str, email: &str) -> Value {
    let cfg = PaymentsConfig::parse(include_str!("../../../../commercial/payments.toml")).ok();
    match paywall::checkout(cfg.as_ref(), edition, 1, licensee, email) {
        Ok(Paywall::Checkout {
            url, name, price, ..
        }) => {
            json!({"offer": true, "url": url, "name": name, "price": price})
        }
        _ => json!({"offer": false}),
    }
}

/// The store directory for tests.
#[doc(hidden)]
pub fn store_at(dir: PathBuf) -> Store {
    Store::open(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &[u8; 32] = b"kshana-pro-demo-key-debug-only!!";
    const PRO_SCENARIO: &str = include_str!("../../../../scenarios/quantum-systematics-sr.toml");

    fn token(products: &[&str], tier: &str) -> String {
        kshana_pro::license::issue_v2(
            DEMO,
            &kshana_pro::license::LicenseClaims {
                licensee: "Desk Lab".into(),
                tier: tier.into(),
                expires_unix: now() + 86_400 * 60,
                products: products.iter().map(|s| s.to_string()).collect(),
                seats: 2,
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn the_desktop_licence_unlocks_pro_kinds_and_nothing_else_does() {
        if Keyring::embedded().is_some() {
            return; // a build with a real key cannot verify demo tokens
        }
        let dir = std::env::temp_dir().join(format!("kshana-pro-desktop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let st = store_at(dir.clone());
        let t = now();
        // Without a licence: open kinds run, Pro kinds are refused and not listed.
        let ent = status_in(&st, t).entitlement;
        assert!(call("run", &[json!(PRO_SCENARIO)], &ent, t).is_err());
        let open = kshana_pro::registry::pro_kinds();
        let listed = call("list_kinds", &[], &ent, t).unwrap();
        assert!(!listed.as_str().unwrap().contains(open[1].name));
        // A token for another product is refused; a Desktop token activates.
        assert!(activate_in(&st, &token(&["cli"], "pro"), t).is_err());
        let s = activate_in(&st, &token(&["desktop"], "pro"), t).unwrap();
        assert_eq!(s["tier"], "pro");
        assert_eq!(s["licensee"], "Desk Lab");
        assert_eq!(s["licence_tier"], "pro");
        assert_eq!(s["seats"], 2);
        assert_eq!(s["stations"], 0);
        assert_eq!(s["machine_bound"], false);
        let ent = status_in(&st, t).entitlement;
        let out = call("run", &[json!(PRO_SCENARIO)], &ent, t).unwrap();
        assert!(out.as_str().unwrap().contains("MODELLED"));
        let all: Value = serde_json::from_str(
            call("run_all", &[json!(PRO_SCENARIO)], &ent, t)
                .unwrap()
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert!(all["json"]
            .as_str()
            .unwrap()
            .contains("pro-quantum-systematics"));
        // The version the page shows is the engine pin of the Pro build.
        let manifest = include_str!("../../../../Cargo.toml");
        assert!(
            manifest.contains(&format!("kshana = \"={ENGINE_VERSION}\"")),
            "ENGINE_VERSION is not the kshana pin"
        );
        // Payments are off in the committed configuration: no offer, no network.
        assert_eq!(
            upgrade_offer("pro-annual", "x", "y"),
            json!({"offer": false})
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
