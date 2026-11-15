// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
// Roadmap 0.42 E1 delivered as a Pro pack (C10): `pro-incident-forensics` is listed at the
// `pro` edition in the signed tier manifest, refused until a verified forensics pack is
// loaded, then runs only under a licence tier that unlocks it. A pack for another method
// version, or one that does not provide the kind, is refused by the host's activator.
// Every key is generated here; no network. (A separate test binary: pack registration is
// global to the process.)

use std::path::PathBuf;

use kshana_pro::forensics::{self, ForensicsActivator, KIND, PACK_METHOD_ENTRY};
use kshana_pro::license::lifecycle::{Delivery, LicenceService};
use kshana_pro::license::machine::hash_machine_id;
use kshana_pro::license::tiers::{self, Edition};
use kshana_pro::license::{public_key_for, Entitlement, LicenseClaims, Tier};
use kshana_pro::payments::server::RawRequest;
use kshana_pro::propack::issuer::{Catalog, PackIssuer};
use kshana_pro::propack::loader::{LoadContext, PackStore};
use kshana_pro::propack::server::{self, Service, SECRET_HEADER};

const LIC_SEED: [u8; 32] = [0x71; 32];
const PACK_SEED: [u8; 32] = [0x72; 32];
const MASTER: [u8; 32] = [0x73; 32];
const T0: i64 = 1_800_000_000;
const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/forensics");

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kpro-forensics-pack-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Build a catalogue with one pack, issue it to a device and install it; returns the store.
fn installed(
    name: &str,
    pack_id: &str,
    provides: &[&str],
    method: Option<&[u8]>,
) -> (PathBuf, PackStore, String) {
    let dir = tmp(name);
    let payload = dir.join("payload");
    std::fs::create_dir_all(payload.join("lib")).unwrap();
    std::fs::write(
        payload.join("lib/libkshana_pro_forensics.so"),
        b"\x7fELF fake native payload",
    )
    .unwrap();
    for (n, b) in forensics::pack_payload_files() {
        let p = payload.join(&n);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, method.map(<[u8]>::to_vec).unwrap_or(b)).unwrap();
    }
    let catalog = Catalog::open(dir.join("catalog"));
    let provides: Vec<String> = provides.iter().map(|s| s.to_string()).collect();
    catalog
        .build_from_dir(&payload, pack_id, "1.0.0", "linux-x86_64", &provides, None)
        .unwrap();
    let licences = LicenceService::open(LIC_SEED, &[], &dir.join("vendor"));
    let device = hash_machine_id("adjuster-laptop");
    let first = licences
        .open_licence(
            &LicenseClaims {
                licensee: "Synthetic Adjusters Ltd".into(),
                tier: "pro".into(),
                products: vec!["desktop".into()],
                seats: 1,
                ..Default::default()
            },
            Delivery::Online,
            T0 + 365 * 86_400,
            "invoice:TEST-1",
            "",
            T0,
        )
        .unwrap();
    let seat = licences.activate(&first.token, &device, T0 + 10).unwrap();
    let svc = Service {
        licences,
        packs: PackIssuer::new(catalog, MASTER, PACK_SEED),
    };
    let req = RawRequest {
        method: "POST".into(),
        path: "/v1/packs/fetch".into(),
        headers: vec![],
        body: serde_json::json!({ "token": seat.token, "device": device, "pack_id": pack_id,
            "platform": "linux-x86_64" })
        .to_string()
        .into_bytes(),
    };
    let r = server::route(&svc, &req, T0 + 60);
    assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
    let secret = r
        .headers
        .iter()
        .find(|(k, _)| *k == SECRET_HEADER)
        .unwrap()
        .1
        .clone();
    let store = PackStore::in_app_data(&dir.join("app"));
    store
        .install(&r.body, &secret, &[public_key_for(&PACK_SEED)])
        .unwrap();
    (dir, store, device)
}

fn incident_src() -> String {
    format!(
        "kind = \"pro-incident-forensics\"\n[incident]\nid = \"PACK-1\"\ntitle = \"pack test\"\nsubject = \"area\"\n\
start_utc = \"2026-03-01T09:30:00Z\"\nend_utc = \"2026-03-01T14:30:00Z\"\n\
[[adsb]]\nlabel = \"ADS-B sample\"\npath = \"{FIX}/adsb.csv\"\n"
    )
}

fn ent(tier: Tier) -> Entitlement {
    Entitlement {
        licensee: "x".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

#[test]
fn the_forensics_kind_runs_only_from_a_verified_pack_under_a_pro_licence() {
    // Listed in the signed tier manifest at `pro`, before any pack is loaded.
    tiers::startup_check().unwrap();
    assert_eq!(tiers::edition_of(KIND), Some(Edition::Pro));
    assert!(tiers::manifest()
        .unwrap()
        .named()
        .iter()
        .any(|(n, t, e)| *n == KIND && *t == "kind" && *e == Edition::Pro));
    assert_eq!(kshana_pro::license::feature_min_tier(KIND), Tier::Pro);
    assert!(!forensics::is_loaded());
    assert!(!kshana_pro::registry::pro_kinds()
        .iter()
        .any(|k| k.name == KIND));
    let src = incident_src();
    let e = kshana_pro::registry::run_gated(&src, &ent(Tier::Pro), T0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("comes in a Pro pack"), "{e}");
    // A build without a trusted pack key loads nothing (KSHANA_PRO_PACK_PUBKEY unset here).
    let (dir, store, device) = installed("good", "pro-forensics", &[KIND], None);
    let keys = vec![public_key_for(&PACK_SEED)];
    let st = kshana_pro::license::activation::Store::open(dir.join("app/licence"));
    let status = st.status(
        &kshana_pro::license::Keyring::single(public_key_for(&LIC_SEED)),
        &[],
        &kshana_pro::license::VerifyOptions {
            now_unix: T0 + 60,
            product: None,
            machine: None,
            allow_expired: false,
        },
    );
    assert!(forensics::load_installed(&dir.join("app"), &status, &device, T0 + 60).is_err());
    assert!(!forensics::is_loaded());

    // A pack for another method version, and one that does not provide the kind: refused.
    let (_, old, dev_old) = installed(
        "old",
        "pro-forensics-old",
        &[KIND],
        Some(br#"{"kind":"pro-incident-forensics","method_id":"kshana-pro/incident-forensics/v0","params_sha256":"00"}"#),
    );
    let ctx = |d: &'static str| LoadContext {
        keys: Box::leak(keys.clone().into_boxed_slice()),
        device: d,
        licence: None,
        revocations: &[],
        now: T0 + 60,
    };
    let d_old: &'static str = Box::leak(dev_old.into_boxed_str());
    let e = old
        .load("pro-forensics-old", &ctx(d_old), &ForensicsActivator)
        .unwrap_err()
        .to_string();
    assert!(e.contains("method"), "{e}");
    let (_, other, dev_other) = installed("other", "pro-other", &["pro-something-else"], None);
    let d_other: &'static str = Box::leak(dev_other.into_boxed_str());
    let e = other
        .load("pro-other", &ctx(d_other), &ForensicsActivator)
        .unwrap_err()
        .to_string();
    assert!(e.contains("does not provide"), "{e}");
    assert!(!forensics::is_loaded());

    // The verified pack loads; the payload carried the method record.
    let d_good: &'static str = Box::leak(device.into_boxed_str());
    let opened = store
        .load("pro-forensics", &ctx(d_good), &ForensicsActivator)
        .unwrap();
    assert!(opened.payload.get(PACK_METHOD_ENTRY).is_some());
    assert!(store
        .unpack_dir("pro-forensics")
        .unwrap()
        .join("method.json")
        .exists());
    assert!(forensics::is_loaded());
    assert!(kshana_pro::registry::pro_kinds()
        .iter()
        .any(|k| k.name == KIND));
    assert_eq!(tiers::edition_of(KIND), Some(Edition::Pro));
    tiers::startup_check().unwrap();

    // Gated by the licence tier: Evaluation is refused, Pro runs it.
    let e = kshana_pro::registry::run_gated(&src, &Entitlement::evaluation(), T0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("requires the 'pro' tier"), "{e}");
    let out = kshana_pro::registry::run_gated(&src, &ent(Tier::Pro), T0).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
    assert_eq!(v["format"], forensics::FORMAT);
    assert_eq!(v["verdict"]["jamming"]["status"], "indicated");
    assert!(out.svg.starts_with("<svg"));
    assert!(out.csv.unwrap().starts_with("utc,kind,class,finding,text"));
    // `kinds --json` lists it at the pro edition while the pack is loaded.
    let kinds: serde_json::Value =
        serde_json::from_str(&kshana_pro::registry::kinds_json()).unwrap();
    let k = kinds["pro"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["name"] == KIND)
        .unwrap();
    assert_eq!(k["edition"], "pro");
    assert_eq!(k["min_tier"], "pro");
    // A second load is refused.
    assert!(store
        .load("pro-forensics", &ctx(d_good), &ForensicsActivator)
        .is_err());
}
