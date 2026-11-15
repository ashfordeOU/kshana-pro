// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
// Roadmap 0.42 C10: Pro pack delivery. Build, issue through the endpoint, install,
// verify, decrypt, load through the extension hook; refusals (signature, device,
// revocation, ended licence, tampering); the expiry notices, the lock at expiry, renewal
// in the 30-day window without a download, and deletion of our files only at its end.
// Every key is generated here; no network.

use std::path::{Path, PathBuf};

use kshana::api::{KshanaError, ScenarioMeta};
use kshana::registry::PackRegistry;
use kshana_pro::extension::{ExtensionKind, PackExtension};
use kshana_pro::license::activation::{Status, Store};
use kshana_pro::license::lifecycle::{Delivery, LicenceService};
use kshana_pro::license::machine::hash_machine_id;
use kshana_pro::license::tiers::{self, Edition};
use kshana_pro::license::{
    public_key_for, Entitlement, Keyring, LicenseClaims, Product, Tier, VerifyOptions,
};
use kshana_pro::payments::server::RawRequest;
use kshana_pro::propack::format::{self, Container};
use kshana_pro::propack::issuer::{Catalog, PackIssuer};
use kshana_pro::propack::loader::{Activator, LoadContext, OpenedPack, PackStore, Phase};
use kshana_pro::propack::server::{self, Service, SECRET_HEADER};

const LIC_SEED: [u8; 32] = [0x61; 32];
const PACK_SEED: [u8; 32] = [0x62; 32];
const MASTER: [u8; 32] = [0x63; 32];
const T0: i64 = 1_800_000_000;
const DAY: i64 = 86_400;
const KIND: &str = "pro-test-pack-kind";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kpro-packs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pack_keys() -> Vec<[u8; 32]> {
    vec![public_key_for(&PACK_SEED)]
}

fn ring() -> Keyring {
    Keyring::single(public_key_for(&LIC_SEED))
}

fn opts_on(device: &str, now: i64) -> VerifyOptions<'static> {
    let d: &'static str = Box::leak(device.to_string().into_boxed_str());
    let f: &'static dyn Fn() -> Option<String> = Box::leak(Box::new(move || Some(d.to_string())));
    VerifyOptions {
        now_unix: now,
        product: Some(Product::Desktop),
        machine: Some(f),
        allow_expired: false,
    }
}

struct World {
    dir: PathBuf,
    svc: Service,
    device: String,
    root: String,
    seat_token: String,
}

fn world(name: &str, provides: &[&str]) -> World {
    let dir = tmp(name);
    let payload = dir.join("payload");
    std::fs::create_dir_all(payload.join("lib")).unwrap();
    std::fs::write(
        payload.join("lib/libkshana_pro_pack.so"),
        b"\x7fELF fake native payload",
    )
    .unwrap();
    std::fs::write(payload.join("kinds.json"), format!("[\"{KIND}\"]")).unwrap();
    let catalog = Catalog::open(dir.join("catalog"));
    let provides: Vec<String> = provides.iter().map(|s| s.to_string()).collect();
    catalog
        .build_from_dir(
            &payload,
            "pro-core",
            "1.0.0",
            "linux-x86_64",
            &provides,
            None,
        )
        .unwrap();
    let licences = LicenceService::open(LIC_SEED, &[], &dir.join("vendor"));
    let device = hash_machine_id("customer-laptop");
    let first = licences
        .open_licence(
            &LicenseClaims {
                licensee: "Harbour Pilots Ltd".into(),
                tier: "pro".into(),
                products: vec!["desktop".into()],
                seats: 1,
                ..Default::default()
            },
            Delivery::Online,
            T0 + 365 * DAY,
            "invoice:INV-7",
            "",
            T0,
        )
        .unwrap();
    let seat = licences.activate(&first.token, &device, T0 + 10).unwrap();
    World {
        svc: Service {
            licences,
            packs: PackIssuer::new(catalog, MASTER, PACK_SEED),
        },
        dir,
        device,
        root: first.root,
        seat_token: seat.token,
    }
}

fn fetch(w: &World, token: &str, device: &str, now: i64) -> (u16, Vec<u8>, Option<String>) {
    let req = RawRequest {
        method: "POST".into(),
        path: "/v1/packs/fetch".into(),
        headers: vec![],
        body: serde_json::json!({ "token": token, "device": device, "pack_id": "pro-core",
            "platform": "linux-x86_64" })
        .to_string()
        .into_bytes(),
    };
    let r = server::route(&w.svc, &req, now);
    let secret = r
        .headers
        .iter()
        .find(|(k, _)| *k == SECRET_HEADER)
        .map(|(_, v)| v.clone());
    (r.status, r.body, secret)
}

fn status(store: &Store, device: &str, now: i64) -> Status {
    store.status(&ring(), &[], &opts_on(device, now))
}

fn ctx<'a>(
    device: &'a str,
    keys: &'a [[u8; 32]],
    st: Option<&'a Status>,
    now: i64,
) -> LoadContext<'a> {
    LoadContext {
        keys,
        device,
        licence: st,
        revocations: &[],
        now,
    }
}

struct TestActivator;

fn no_build(_: &str) -> Result<Box<dyn kshana::api::ExternalPack>, KshanaError> {
    Err(KshanaError::Unsupported("test kind".into()))
}
fn no_register(_: &mut PackRegistry) {}

impl Activator for TestActivator {
    fn activate(&self, pack: &OpenedPack, unpack_dir: &Path) -> Result<PackExtension, KshanaError> {
        // The host would load the native library; here it writes it out and lists kinds.
        let lib = pack.payload.get("lib/libkshana_pro_pack.so").unwrap();
        std::fs::write(unpack_dir.join("libkshana_pro_pack.so"), lib).unwrap();
        let kinds: Vec<String> =
            serde_json::from_slice(pack.payload.get("kinds.json").unwrap()).unwrap();
        Ok(PackExtension {
            pack_id: pack.manifest.pack_id.clone(),
            kinds: kinds
                .iter()
                .map(|k| ExtensionKind {
                    meta: ScenarioMeta {
                        name: Box::leak(k.clone().into_boxed_str()),
                        description: "test kind delivered by a Pro pack",
                        required_fields: &[],
                        optional_fields: &[],
                    },
                    min_tier: Tier::Pro,
                    build: no_build,
                    register_into: no_register,
                })
                .collect(),
            features: vec![],
        })
    }
}

#[test]
fn issue_install_verify_and_load_through_the_extension_hook() {
    let w = world("load", &[KIND]);
    let keys = pack_keys();
    let (code, file, secret) = fetch(&w, &w.seat_token, &w.device, T0 + 60);
    assert_eq!(code, 200, "{}", String::from_utf8_lossy(&file));
    let secret = secret.unwrap();
    let app = w.dir.join("app-data");
    let store = PackStore::in_app_data(&app);

    // Bad signature: a pack checked against another key, or with a manifest byte changed.
    assert!(store
        .install(&file, &secret, &[public_key_for(&[9; 32])])
        .is_err());
    let mut bad = file.clone();
    bad[20] ^= 1;
    assert!(store.install(&bad, &secret, &keys).is_err());
    let m = store.install(&file, &secret, &keys).unwrap();
    assert_eq!(m.licensee, "Harbour Pilots Ltd");
    assert_eq!(m.device, w.device);
    assert_eq!(m.provides, vec![KIND.to_string()]);
    assert_eq!(m.expires_unix, T0 + 10 + 35 * DAY);

    let lic_store = Store::open(app.join("licence"));
    lic_store
        .activate(&ring(), &w.seat_token, &opts_on(&w.device, T0 + 60))
        .unwrap();
    let st = status(&lic_store, &w.device, T0 + 60);

    // Another device: refused (by the manifest, and the key would not derive anyway).
    let other = hash_machine_id("someone-else");
    let e = store
        .open("pro-core", &ctx(&other, &keys, None, T0 + 60))
        .unwrap_err()
        .to_string();
    assert!(e.contains("another device"), "{e}");
    let sp = format::verify_signed(&file, &keys).unwrap();
    let sec: [u8; 32] = hex::decode(&secret).unwrap().try_into().unwrap();
    assert!(format::open_sealed(&file, &sp, &sec, &other).is_err());
    // A wrong licence secret, or a changed ciphertext byte: refused.
    assert!(format::open_sealed(&file, &sp, &[0; 32], &w.device).is_err());
    let mut flip = file.clone();
    let last = flip.len() - 20;
    flip[last] ^= 1;
    assert!(format::open_sealed(&flip, &sp, &sec, &w.device).is_err());

    // The kind is unknown before the pack loads.
    assert_eq!(tiers::edition_of(KIND), None);
    let opened = store
        .load(
            "pro-core",
            &ctx(&w.device, &keys, Some(&st), T0 + 60),
            &TestActivator,
        )
        .unwrap();
    assert!(store
        .unpack_dir("pro-core")
        .unwrap()
        .join("libkshana_pro_pack.so")
        .exists());
    // Loaded through the hook: a Pro-edition kind, gated by the licence tier.
    assert_eq!(tiers::edition_of(KIND), Some(Edition::Pro));
    assert!(kshana_pro::registry::pro_kinds()
        .iter()
        .any(|k| k.name == KIND));
    let pro = Entitlement {
        licensee: "x".into(),
        tier: Tier::Pro,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(pro.allows(KIND, T0));
    assert!(!Entitlement::evaluation().allows(KIND, T0));
    // Loading twice is refused.
    assert!(store
        .load(
            "pro-core",
            &ctx(&w.device, &keys, Some(&st), T0 + 61),
            &TestActivator
        )
        .is_err());

    // The watermark names the licensee and verifies only under our key.
    let plain = opened.payload.to_bytes();
    let wm = format::trace(&plain, &w.svc.packs.watermark_key()).unwrap();
    assert_eq!(wm.licensee_id, format::licensee_id("Harbour Pilots Ltd"));
    assert_eq!(wm.token_id, m.token_id);
    assert!(format::trace(&plain, &[1; 32]).is_err());
    assert_eq!(&wm.mark[..32], m.watermark_id);
}

#[test]
fn the_hook_refuses_what_the_signed_pack_does_not_provide_or_collides() {
    // A pack whose manifest provides a built-in Pro kind: registration refused.
    let w = world("collide", &["pro-twin"]);
    let keys = pack_keys();
    let (_, file, secret) = fetch(&w, &w.seat_token, &w.device, T0 + 60);
    let store = PackStore::in_app_data(&w.dir.join("app"));
    store.install(&file, &secret.unwrap(), &keys).unwrap();
    // The activator offers KIND, which the manifest does not list.
    let e = store
        .load(
            "pro-core",
            &ctx(&w.device, &keys, None, T0 + 60),
            &TestActivator,
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("not in the signed pack manifest"), "{e}");
    struct Twin;
    impl Activator for Twin {
        fn activate(&self, p: &OpenedPack, d: &Path) -> Result<PackExtension, KshanaError> {
            let mut x = TestActivator.activate(p, d)?;
            x.kinds[0].meta.name = "pro-twin";
            Ok(x)
        }
    }
    let e = store
        .load("pro-core", &ctx(&w.device, &keys, None, T0 + 60), &Twin)
        .unwrap_err()
        .to_string();
    assert!(e.contains("collides"), "{e}");
}

#[test]
fn the_endpoint_refuses_unbound_expired_revoked_and_foreign_requests() {
    let w = world("refuse", &[KIND]);
    // An unbound token (no seat on the device).
    let unbound = w
        .svc
        .licences
        .reissue_unbound(&w.root, "manual", T0 + 20)
        .unwrap();
    assert_eq!(fetch(&w, &unbound.token, &w.device, T0 + 30).0, 403);
    // The seat token from another device.
    assert_eq!(
        fetch(&w, &w.seat_token, &hash_machine_id("other"), T0 + 30).0,
        403
    );
    // Expired: no new pack (renew first).
    assert_eq!(fetch(&w, &w.seat_token, &w.device, T0 + 40 * DAY).0, 403);
    // Garbage.
    assert_eq!(fetch(&w, "nonsense", &w.device, T0 + 30).0, 403);
    // Revoked.
    w.svc
        .licences
        .revoke(&w.root, "refund", "manual", T0 + 50)
        .unwrap();
    assert_eq!(fetch(&w, &w.seat_token, &w.device, T0 + 60).0, 403);
    // A macOS payload must name the Apple team ID it is signed with.
    let e = w
        .svc
        .packs
        .catalog
        .build_from_dir(
            &w.dir.join("payload"),
            "pro-core",
            "1.0.1",
            "macos-aarch64",
            &[KIND.into()],
            None,
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("team ID"), "{e}");
    assert!(w
        .svc
        .packs
        .catalog
        .build_from_dir(
            &w.dir.join("payload"),
            "pro-core",
            "1.0.1",
            "macos-aarch64",
            &[KIND.into()],
            Some("ABCDE12345")
        )
        .is_ok());
}

#[test]
fn expiry_notices_lock_at_expiry_renewal_without_download_and_deletion_after_the_window() {
    let w = world("expiry", &[KIND]);
    let keys = pack_keys();
    let (_, file, secret) = fetch(&w, &w.seat_token, &w.device, T0 + 60);
    let app = w.dir.join("app-data");
    // Things that are not ours, next to and inside the pack store: never touched.
    std::fs::create_dir_all(app.join("reports")).unwrap();
    std::fs::write(
        app.join("reports/signed-report.json"),
        b"{\"signature\":\"...\"}",
    )
    .unwrap();
    let store = PackStore::in_app_data(&app);
    store.install(&file, &secret.unwrap(), &keys).unwrap();
    std::fs::write(store.dir().join("notes.txt"), b"user file").unwrap();
    let lic = Store::open(app.join("licence"));
    lic.activate(&ring(), &w.seat_token, &opts_on(&w.device, T0 + 60))
        .unwrap();
    let exp = T0 + 10 + 35 * DAY;
    let d = w.device.clone();

    let notices = |now: i64| {
        let st = status(&lic, &d, now);
        store.enforce(&ctx(&d, &keys, Some(&st), now))
    };
    let opens = |now: i64| {
        let st = status(&lic, &d, now);
        store.open("pro-core", &ctx(&d, &keys, Some(&st), now))
    };
    // Active: no notice. 14, 7 and 1 days before: one notice each.
    assert!(notices(T0 + DAY).is_empty());
    assert_eq!(notices(exp - 14 * DAY).len(), 1);
    assert!(notices(exp - 13 * DAY).is_empty());
    assert_eq!(notices(exp - 7 * DAY).len(), 1);
    assert!(notices(exp - 6 * DAY).is_empty());
    let n = notices(exp - DAY + 60);
    assert_eq!(n.len(), 1);
    assert!(matches!(n[0].phase, Phase::Expiring { days_left: 1 }));
    assert!(opens(exp - 60).is_ok());

    // Expired, inside the renewal window: the pack refuses to load (no Pro use), stays on
    // disk, and a notice comes once a day.
    let e = opens(exp + 60).unwrap_err().to_string();
    assert!(e.contains("locked"), "{e}");
    let n = notices(exp + 60);
    assert_eq!(n.len(), 1);
    assert!(matches!(
        n[0].phase,
        Phase::Locked {
            window_days_left: 30
        }
    ));
    assert!(notices(exp + 120).is_empty());
    assert_eq!(notices(exp + DAY + 60).len(), 1);
    assert!(store.dir().join("pro-core.kpack").exists());

    // Renewal in the window: the licence server issues a new token for the seat; the
    // same pack file opens again, no download.
    let before = std::fs::read(store.dir().join("pro-core.kpack")).unwrap();
    let renewed = w
        .svc
        .licences
        .renew(&w.seat_token, &w.device, exp + 2 * DAY)
        .unwrap();
    lic.activate(&ring(), &renewed.token, &opts_on(&w.device, exp + 2 * DAY))
        .unwrap();
    assert!(opens(exp + 2 * DAY + 60).is_ok());
    assert_eq!(
        std::fs::read(store.dir().join("pro-core.kpack")).unwrap(),
        before
    );
    let exp2 = renewed.expires_unix;

    // A clock set back after the renewed licence ended does not reopen the pack.
    assert!(opens(exp2 + 60).is_err());
    assert!(
        opens(exp2 - 10 * DAY).is_err(),
        "the clock guard keeps the later time"
    );

    // The renewal window ends: the pack's own files are deleted, nothing else.
    let n = notices(exp2 + 30 * DAY + 60);
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].phase, Phase::Gone);
    assert!(n[0].message.contains("verifiable in the free edition"));
    for f in ["pro-core.kpack", "pro-core.key", "pro-core.state.json"] {
        assert!(!store.dir().join(f).exists(), "{f} still there");
    }
    assert!(!store.unpack_dir("pro-core").unwrap().exists());
    assert!(store.dir().join("notes.txt").exists());
    assert!(app.join("reports/signed-report.json").exists());
    assert!(app.join("licence").exists());
    assert!(store.installed().is_empty());
}

#[test]
fn a_revoked_or_ended_licence_locks_the_pack_at_once() {
    let w = world("revoked", &[KIND]);
    let keys = pack_keys();
    let (_, file, secret) = fetch(&w, &w.seat_token, &w.device, T0 + 60);
    let store = PackStore::in_app_data(&w.dir.join("app"));
    store.install(&file, &secret.unwrap(), &keys).unwrap();
    let list_text = w
        .svc
        .licences
        .revoke(&w.root, "chargeback", "manual", T0 + 100)
        .unwrap();
    let list = kshana_pro::license::revocation::verify(&ring(), &list_text).unwrap();
    let c = LoadContext {
        keys: &keys,
        device: &w.device,
        licence: None,
        revocations: std::slice::from_ref(&list),
        now: T0 + 200,
    };
    let e = store.open("pro-core", &c).unwrap_err().to_string();
    assert!(e.contains("revoked"), "{e}");
    assert!(matches!(
        store.phase("pro-core", &c).unwrap().1,
        Phase::Revoked { .. }
    ));
}

#[test]
fn container_names_are_checked() {
    let mut c = Container::default();
    assert!(c.put("../escape", vec![]).is_err());
    assert!(c.put("/abs", vec![]).is_err());
    assert!(c.put("a\\b", vec![]).is_err());
    assert!(c.put("ok/file.bin", vec![1, 2]).is_ok());
    let mut b = c.to_bytes();
    assert_eq!(Container::from_bytes(&b).unwrap(), c);
    b.push(0);
    assert!(Container::from_bytes(&b).is_err());
    assert!(Container::from_bytes(&b[..5]).is_err());
}
