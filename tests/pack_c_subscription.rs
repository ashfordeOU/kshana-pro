// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Pack C first-subscriber flow end to end (roadmap F5), in payments test mode: a
//! signed purchase webhook issues the token; `kshana-pro ci` runs the subscriber's pack
//! with it (informational, then gated against the first run); a renewal webhook renews
//! it; a refund webhook revokes it and, with the revocation list installed, the same
//! command is refused.
//!
//! The payments key here is the debug builds' demo key, so the debug binary verifies the
//! tokens the issuer signs; a build made with a real licence key skips this test.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::payments::provider::sign_paddle;
use kshana_pro::payments::webhook::Issuer;
use kshana_pro::payments::{PaymentsConfig, Provider};

const SECRET: &str = "pdl_ntfset_pack_c_test";
const DEMO_SEED: &[u8; 32] = b"kshana-pro-demo-key-debug-only!!";

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn body(name: &str, occurred: &str, price: &str) -> Vec<u8> {
    std::fs::read_to_string(root().join("tests/fixtures/payments").join(name))
        .unwrap()
        .trim()
        .replace("{{OCCURRED}}", occurred)
        .replace("pri_test_pro_annual", price)
        .into_bytes()
}

#[test]
fn first_subscriber_flow() {
    if option_env!("KSHANA_PRO_LICENSE_PUBKEY").is_some() || !cfg!(debug_assertions) {
        return;
    }
    let dir: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("pack_c_subscription-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("secret"), SECRET).unwrap();
    std::fs::write(dir.join("key"), hex::encode(DEMO_SEED)).unwrap();
    // The committed edition, switched on in test mode with a test price ID.
    let committed = std::fs::read_to_string(root().join("commercial/payments.toml")).unwrap();
    assert!(committed.contains("id = \"pack-c-ci-monthly\""));
    let mut cfg = PaymentsConfig::parse(&committed).unwrap();
    cfg.enabled = true;
    cfg.webhook_secret_file = "secret".into();
    cfg.signing_key_file = "key".into();
    cfg.data_dir = "vendor".into();
    for e in cfg.editions.iter_mut() {
        if e.id == "pack-c-ci-monthly" {
            e.provider_price_id = "pri_test_pack_c".into();
        }
    }
    let issuer = Issuer::open(cfg, &dir).unwrap();

    // 1. Purchase.
    let t0 = 1_794_045_600;
    let b = body(
        "paddle-transaction-completed.json",
        "2026-11-07T10:00:00Z",
        "pri_test_pack_c",
    );
    let o = issuer.handle(Provider::Paddle, Some(&sign_paddle(&b, SECRET, t0)), &b, t0);
    assert_eq!(o.status, 200, "{}", o.message);
    let tid = o.token_id.unwrap();
    let mail: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(issuer.paths.outbox().join(format!("{tid}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(mail["to"], "ops@harbour.example");
    let token = mail["token"].as_str().unwrap().to_string();

    // 2. The subscriber's pipeline: an informational first run, then a gated run.
    let runner_home = dir.join("runner-home");
    let pack = root().join("scenarios/packs/resilience-baseline/pack.toml");
    let ci = |extra: &[&str], tok: &str| {
        Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .arg("ci")
            .arg("--pack")
            .arg(&pack)
            .args(["--utc", "2026-11-07T00:00:00Z", "--license", tok])
            .args(extra)
            .env("KSHANA_PRO_HOME", &runner_home)
            .output()
            .unwrap()
    };
    let first = dir.join("ci-1.json");
    let o1 = ci(&["--json", first.to_str().unwrap()], &token);
    assert!(
        o1.status.success(),
        "{}",
        String::from_utf8_lossy(&o1.stderr)
    );
    let second = dir.join("ci-2.json");
    let o2 = ci(
        &[
            "--baseline",
            first.to_str().unwrap(),
            "--json",
            second.to_str().unwrap(),
        ],
        &token,
    );
    assert!(
        o2.status.success(),
        "{}",
        String::from_utf8_lossy(&o2.stderr)
    );

    // 3. Renewal a month later: a new token that also runs.
    let t1 = t0 + 31 * 86_400;
    let b = body(
        "paddle-transaction-renewal.json",
        "2026-12-08T10:00:00Z",
        "pri_test_pack_c",
    );
    let o = issuer.handle(Provider::Paddle, Some(&sign_paddle(&b, SECRET, t1)), &b, t1);
    assert_eq!(o.status, 200, "{}", o.message);
    let renewed_id = o.token_id.unwrap();
    let renewed: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(issuer.paths.outbox().join(format!("{renewed_id}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        renewed["to"], "ops@harbour.example",
        "the renewal goes to the buyer"
    );
    let renewed = renewed["token"].as_str().unwrap().to_string();
    assert!(ci(&[], &renewed).status.success());

    // 4. Refund of the first payment: the licence is revoked; once the runner has the
    // revocation list, the pipeline refuses its tokens.
    let t2 = t1 + 60;
    let b = body(
        "paddle-adjustment-refund.json",
        "2026-12-08T10:01:00Z",
        "pri_test_pack_c",
    );
    let o = issuer.handle(Provider::Paddle, Some(&sign_paddle(&b, SECRET, t2)), &b, t2);
    assert_eq!(o.revoked.as_deref(), Some(tid.as_str()), "{}", o.message);
    let install = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(["license", "revocations", "install"])
        .arg(issuer.paths.revocations())
        .env("KSHANA_PRO_HOME", &runner_home)
        .output()
        .unwrap();
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    let o3 = ci(&[], &token);
    assert!(!o3.status.success());
    assert!(
        String::from_utf8_lossy(&o3.stderr).contains("revoked"),
        "{}",
        String::from_utf8_lossy(&o3.stderr)
    );
    // Owner, 15 November 2026 (roadmap 0.42 C1): a refund ends the licence, so every token
    // of it is revoked, the renewal included (no copy keeps working).
    assert!(
        !ci(&[], &renewed).status.success(),
        "the renewed token of a refunded licence is revoked too"
    );
}
