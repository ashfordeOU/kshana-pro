// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Licensing end to end (roadmap F10b, F10c, F10d): the vendor register issues a trial,
//! renews it and revokes it; a customer store activates offline, warns before expiry,
//! keeps working through the grace period, stops at the revocation list that comes
//! with the next update, and never touches the network unless online refresh is
//! switched on.

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use kshana_pro::license::activation::{self, Fetcher, State, Store};
use kshana_pro::license::register::{Action, Register};
use kshana_pro::license::{
    issue_v2, key_id, public_key_for, revocation, Keyring, LicenseClaims, Product, Tier,
    VerifyOptions, KEY_LEN,
};

const SEED: [u8; KEY_LEN] = [21; KEY_LEN];
const DAY: i64 = 86_400;
const T0: i64 = 1_800_000_000;

fn tmp(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let d = std::env::temp_dir().join(format!(
        "kshana-pro-licensing-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ring() -> Keyring {
    Keyring::single(public_key_for(&SEED))
}

fn desktop_at(now: i64) -> VerifyOptions<'static> {
    let mut o = VerifyOptions::at(now);
    o.product = Some(Product::Desktop);
    o
}

#[test]
fn trial_renewal_and_revocation_from_the_register() {
    let dir = tmp("register");
    let reg = Register::open(dir.join("register.jsonl"));
    let store = Store::open(dir.join("customer"));
    let ring = ring();

    // Trial: 30 days of Pro for Desktop, activated with no network.
    let (trial, entry) = reg
        .trial(&SEED, "Harbour Pilots Ltd", &["desktop".into()], T0)
        .unwrap();
    assert_eq!(entry.action, Some(Action::Trial));
    assert_eq!(entry.expires_unix, T0 + 30 * DAY);
    let st = store.activate(&ring, &trial, &desktop_at(T0)).unwrap();
    // A 30-day trial starts active: the first warning comes 14 days before expiry.
    assert_eq!(st.state, State::Active);
    assert_eq!(st.entitlement.tier, Tier::Pro);
    // One trial per licensee.
    assert!(reg
        .trial(&SEED, "harbour pilots ltd", &["desktop".into()], T0)
        .is_err());

    // Day 25: warned (7-day warning), still Pro.
    let st = store.status(&ring, &[], &desktop_at(T0 + 25 * DAY));
    assert_eq!(st.state, State::Expiring { days_left: 5 });
    assert!(st.warning.unwrap().contains("Renew now"));
    // The register lists it for a reminder.
    let due = reg.reminders(T0 + 25 * DAY, 30).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].token_id, entry.token_id);

    // Renewal: one year from the old expiry; the new token replaces the trial.
    let (renewed, r) = reg
        .renew(&SEED, &entry.token_id, 365, "manual", T0 + 25 * DAY)
        .unwrap();
    assert_eq!(r.renews.as_deref(), Some(entry.token_id.as_str()));
    assert_eq!(r.expires_unix, T0 + 30 * DAY + 365 * DAY);
    store
        .activate(&ring, &renewed, &desktop_at(T0 + 25 * DAY))
        .unwrap();
    assert_eq!(
        store.status(&ring, &[], &desktop_at(T0 + 40 * DAY)).state,
        State::Active
    );
    assert!(reg.reminders(T0 + 25 * DAY, 30).unwrap().is_empty());

    // Revocation: the vendor revokes, builds the list, the next update carries it.
    reg.revoke(&r.token_id, "refund", "manual", T0 + 50 * DAY)
        .unwrap()
        .unwrap();
    assert!(reg
        .revoke(&r.token_id, "again", "manual", T0 + 50 * DAY)
        .unwrap()
        .is_none());
    // Until the update arrives the token still works (no phoning home).
    assert_eq!(
        store.status(&ring, &[], &desktop_at(T0 + 51 * DAY)).state,
        State::Active
    );
    let list = reg.revocation_list(&SEED, &[], T0 + 50 * DAY).unwrap();
    store.install_revocations(&ring, &list).unwrap();
    let st = store.status(&ring, &[], &desktop_at(T0 + 51 * DAY));
    assert_eq!(st.state, State::Revoked);
    assert_eq!(st.entitlement.tier, Tier::Evaluation);
    // A revoked token cannot be renewed or activated again.
    assert!(reg
        .renew(&SEED, &r.token_id, 30, "manual", T0 + 52 * DAY)
        .is_err());
    assert!(store
        .activate(&ring, &renewed, &desktop_at(T0 + 52 * DAY))
        .is_err());

    // The register's hash chain holds, and an edit breaks it.
    assert_eq!(reg.verify().unwrap(), 3); // trial, renew, revoke
    let path = reg.path().to_path_buf();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("Harbour Pilots Ltd", "Harbour Pilots Plc"),
    )
    .unwrap();
    assert!(reg.verify().is_err());
    assert!(reg.revocation_list(&SEED, &[], T0).is_err());
}

#[test]
fn grace_period_then_evaluation() {
    let dir = tmp("grace");
    let store = Store::open(&dir);
    let ring = ring();
    let tok = issue_v2(
        &SEED,
        &LicenseClaims {
            licensee: "Grid Operator".into(),
            tier: "enterprise".into(),
            expires_unix: T0,
            products: vec!["desktop".into()],
            ..Default::default()
        },
    )
    .unwrap();
    store.activate(&ring, &tok, &desktop_at(T0 - DAY)).unwrap();

    let at = |d: i64| store.status(&ring, &[], &desktop_at(T0 + d));
    assert_eq!(
        store.status(&ring, &[], &desktop_at(T0 - 15 * DAY)).state,
        State::Active
    );
    let w = store.status(&ring, &[], &desktop_at(T0 - 10 * DAY));
    assert_eq!(w.state, State::Expiring { days_left: 10 });
    assert!(w.warning.unwrap().contains("Renew soon"));
    let w = store.status(&ring, &[], &desktop_at(T0 - DAY + 1));
    assert_eq!(w.state, State::Expiring { days_left: 1 });
    assert!(w.warning.unwrap().contains("last day"));
    // Owner correction (15 November 2026): the 30-day window after expiry gives no Pro
    // use; it is a window to renew in.
    let g = at(DAY);
    assert_eq!(g.state, State::Grace { days_left: 29 });
    assert_eq!(g.entitlement.tier, Tier::Evaluation);
    assert!(!g.entitlement.allows("pro-batch", T0 + DAY));
    assert!(g.warning.unwrap().contains("renew within"));
    assert_eq!(at(30 * DAY - 1).state, State::Grace { days_left: 1 });
    let e = at(30 * DAY);
    assert_eq!(e.state, State::Expired);
    assert_eq!(e.entitlement.tier, Tier::Evaluation);
    // An expired token beyond the window is not activated.
    assert!(store
        .activate(&ring, &tok, &desktop_at(T0 + 31 * DAY))
        .is_err());
}

#[test]
fn revocation_list_cannot_be_rolled_back_and_retires_keys() {
    let dir = tmp("rollback");
    let store = Store::open(&dir);
    let ring = ring();
    let sign = |seq: u64, revoked: Vec<String>, retired: Vec<String>| {
        revocation::sign(
            &SEED,
            &revocation::RevocationList {
                sequence: seq,
                revoked,
                retired_kids: retired,
                ..Default::default()
            },
        )
        .unwrap()
    };
    store
        .install_revocations(&ring, &sign(5, vec!["kt_x".into()], vec![]))
        .unwrap();
    assert!(store
        .install_revocations(&ring, &sign(4, vec![], vec![]))
        .is_err());
    assert_eq!(store.revocations(&ring).unwrap().sequence, 5);

    // A token from a second key; then a list retires that key: it stops verifying.
    const OLD: [u8; KEY_LEN] = [22; KEY_LEN];
    let mut ring2 = ring.clone();
    ring2.add(public_key_for(&OLD));
    let tok = issue_v2(
        &OLD,
        &LicenseClaims {
            licensee: "Rail Lab".into(),
            tier: "pro".into(),
            products: vec!["cli".into()],
            ..Default::default()
        },
    )
    .unwrap();
    let st = activation::evaluate(&ring2, Some(&tok), &[], &VerifyOptions::at(T0));
    assert_eq!(st.state, State::Active);
    let list =
        revocation::verify(&ring, &sign(6, vec![], vec![key_id(&public_key_for(&OLD))])).unwrap();
    let st = activation::evaluate(&ring2, Some(&tok), &[list], &VerifyOptions::at(T0));
    assert!(matches!(st.state, State::Invalid(ref m) if m.contains("retired")));
}

struct CountingFetcher {
    calls: Cell<u32>,
    revocations: Option<String>,
    token: Option<String>,
}

impl Fetcher for CountingFetcher {
    fn get(&self, url: &str) -> Result<Option<String>, String> {
        self.calls.set(self.calls.get() + 1);
        if url.ends_with("/revocations.txt") {
            Ok(self.revocations.clone())
        } else if url.contains("/token/") {
            Ok(self.token.clone())
        } else {
            Ok(None)
        }
    }
}

#[test]
fn online_refresh_is_off_by_default_and_opt_in() {
    let dir = tmp("refresh");
    let store = Store::open(&dir);
    let reg = Register::open(dir.join("vendor/register.jsonl"));
    let ring = ring();
    let claims = LicenseClaims {
        licensee: "Port Authority".into(),
        tier: "pro".into(),
        expires_unix: T0 + 10 * DAY,
        products: vec!["desktop".into()],
        ..Default::default()
    };
    let (tok, e) = reg.issue(&SEED, &claims, "manual", "", T0).unwrap();
    store.activate(&ring, &tok, &desktop_at(T0)).unwrap();
    let (renewed, _) = reg.renew(&SEED, &e.token_id, 365, "manual", T0).unwrap();

    let f = CountingFetcher {
        calls: Cell::new(0),
        revocations: Some(reg.revocation_list(&SEED, &[], T0).unwrap()),
        token: Some(renewed.clone()),
    };
    // Off (no refresh.toml): refused and no fetch at all.
    assert!(activation::refresh(&store, &ring, &f, &desktop_at(T0)).is_err());
    assert_eq!(f.calls.get(), 0, "refresh off must make no network call");
    // Explicitly disabled: still nothing.
    std::fs::write(
        store.refresh_path(),
        "enabled = false\nurl = \"https://licence.example\"\n",
    )
    .unwrap();
    assert!(activation::refresh(&store, &ring, &f, &desktop_at(T0)).is_err());
    assert_eq!(f.calls.get(), 0);

    // Opted in: the renewed token replaces the stored one.
    std::fs::write(
        store.refresh_path(),
        "enabled = true\nurl = \"https://licence.example\"\n",
    )
    .unwrap();
    let out = activation::refresh(&store, &ring, &f, &desktop_at(T0)).unwrap();
    assert!(out.renewed);
    assert_eq!(store.token().unwrap(), renewed);
    assert_eq!(f.calls.get(), 2);

    // A token for someone else from the refresh URL is not installed.
    let other = issue_v2(
        &SEED,
        &LicenseClaims {
            licensee: "Someone Else".into(),
            ..claims.clone()
        },
    )
    .unwrap();
    let f2 = CountingFetcher {
        calls: Cell::new(0),
        revocations: None,
        token: Some(other),
    };
    assert!(activation::refresh(&store, &ring, &f2, &desktop_at(T0)).is_err());
    assert_eq!(store.token().unwrap(), renewed);
}

#[test]
fn activation_refuses_a_token_for_another_product_or_signer() {
    let dir = tmp("refuse");
    let store = Store::open(&dir);
    let ring = ring();
    let station_only = issue_v2(
        &SEED,
        &LicenseClaims {
            licensee: "Site".into(),
            tier: "pro".into(),
            products: vec!["station".into()],
            stations: 4,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(store
        .activate(&ring, &station_only, &desktop_at(T0))
        .is_err());
    assert!(store.token().is_none());
    let foreign = issue_v2(
        &[99; KEY_LEN],
        &LicenseClaims {
            licensee: "Site".into(),
            tier: "pro".into(),
            products: vec!["desktop".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(store.activate(&ring, &foreign, &desktop_at(T0)).is_err());
    assert!(store.deactivate().is_ok());
}

/// The `kshana-pro license` commands, driven as a customer and a vendor would. Debug
/// builds verify against the demo key when no licence key is compiled in; a build made
/// with `KSHANA_PRO_LICENSE_PUBKEY` set (the release smoke test) skips this.
#[test]
fn license_commands_end_to_end() {
    if option_env!("KSHANA_PRO_LICENSE_PUBKEY").is_some() || !cfg!(debug_assertions) {
        return;
    }
    let dir = tmp("cli");
    let home = dir.join("home");
    let reg = dir.join("vendor/register.jsonl");
    let run = |args: &[&str]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .env("KSHANA_MACHINE_ID", "test-machine-1")
            .env_remove("KSHANA_PRO_SIGNING_KEY")
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let scenario = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scenarios/quantum-systematics-sr.toml"
    );

    // No licence: the Pro kind is refused.
    let (ok, _, err) = run(&["run", scenario]);
    assert!(!ok, "Pro kind ran with no licence: {err}");

    // Vendor: a machine-bound trial for this machine, from the register.
    let (ok, mid, _) = run(&["license", "machine-id"]);
    assert!(ok);
    let mid = mid.trim().to_string();
    assert_eq!(mid.len(), 64);
    let (ok, tok, err) = run(&[
        "license",
        "issue",
        "--tier",
        "pro",
        "--licensee",
        "Test Lab",
        "--days",
        "60",
        "--product",
        "cli,desktop",
        "--seats",
        "2",
        "--machine",
        &mid,
        "--register",
        reg.to_str().unwrap(),
    ]);
    assert!(ok, "{err}");
    let tok = tok.trim().to_string();

    // Customer: activate offline, then run without --license.
    let (ok, out, err) = run(&["license", "activate", &tok]);
    assert!(ok, "{err}");
    assert!(out.contains("activated: pro tier for Test Lab"), "{out}");
    let (ok, out, _) = run(&["license", "status", "--json"]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["state"], "active");
    assert_eq!(v["seats"], 2);
    let (ok, _, err) = run(&["run", scenario]);
    assert!(ok, "activated licence did not unlock the Pro kind: {err}");
    assert!(err.contains("(activated)"), "{err}");

    // Another machine cannot use it.
    let other = std::process::Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(["license", "verify", &tok])
        .env("KSHANA_MACHINE_ID", "test-machine-2")
        .output()
        .unwrap();
    assert!(!other.status.success());

    // Vendor revokes and ships a list; the customer installs it with the update.
    let token_id = v["token_id"].as_str().unwrap().to_string();
    let (ok, _, err) = run(&[
        "license",
        "revoke",
        &token_id,
        "--reason",
        "test",
        "--register",
        reg.to_str().unwrap(),
    ]);
    assert!(ok, "{err}");
    let list = dir.join("revocations.txt");
    let (ok, _, err) = run(&[
        "license",
        "revocations",
        "build",
        "--register",
        reg.to_str().unwrap(),
        "--out",
        list.to_str().unwrap(),
    ]);
    assert!(ok, "{err}");
    let (ok, _, err) = run(&["license", "revocations", "install", list.to_str().unwrap()]);
    assert!(ok, "{err}");
    let (_, out, _) = run(&["license", "status", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["state"], "revoked");
    let (ok, _, _) = run(&["run", scenario]);
    assert!(!ok, "a revoked licence still unlocked the Pro kind");
    let (ok, _, _) = run(&["license", "register", "verify", reg.to_str().unwrap()]);
    assert!(ok);
    // Online refresh is off: refused without any network call.
    let (ok, _, err) = run(&["license", "refresh"]);
    assert!(!ok && err.contains("online refresh is off"), "{err}");
}
