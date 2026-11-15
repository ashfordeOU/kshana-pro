// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Payments (roadmap F10g) end to end in test mode, against self-made webhook bodies
//! signed with each provider's published scheme (no provider account exists yet):
//! checkout, verified webhook, token issued, app unlocks, renewal, refund or
//! cancellation, token revoked; forged, replayed and out-of-order webhooks refused;
//! with the switch off, no checkout, no listener, no network call.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use kshana_pro::license::activation::{State, Store};
use kshana_pro::license::register::Register;
use kshana_pro::license::{public_key_for, Keyring, Product, Tier, VerifyOptions, KEY_LEN};
use kshana_pro::payments::paywall::{self, Paywall};
use kshana_pro::payments::provider::{sign_lemonsqueezy, sign_paddle};
use kshana_pro::payments::webhook::Issuer;
use kshana_pro::payments::{server, PaymentsConfig, Provider};

const SECRET: &str = "pdl_ntfset_test_secret_not_real";
const SEED: [u8; KEY_LEN] = [42; KEY_LEN];
const T0: i64 = 1_794_045_600; // 2026-11-07T10:00:00Z

fn tmp(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let d = std::env::temp_dir().join(format!(
        "kshana-pro-payments-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rfc3339(t: i64) -> String {
    let days = t.div_euclid(86_400);
    let tod = t.rem_euclid(86_400);
    // civil_from_days
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        tod % 3600 / 60,
        tod % 60
    )
}

fn fixture(name: &str, occurred: i64) -> Vec<u8> {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/payments")
            .join(name),
    )
    .unwrap()
    .trim()
    .replace("{{OCCURRED}}", &rfc3339(occurred))
    .into_bytes()
}

fn config(dir: &Path, provider: &str, enabled: bool) -> PaymentsConfig {
    std::fs::write(dir.join("webhook-secret"), SECRET).unwrap();
    std::fs::write(dir.join("signing-key"), hex::encode(SEED)).unwrap();
    let text = format!(
        r#"
enabled = {enabled}
provider = "{provider}"
mode = "test"
webhook_secret_file = "webhook-secret"
signing_key_file = "signing-key"
data_dir = "data"
[checkout]
site_checkout_url = "https://kshana.dev/checkout"
lemonsqueezy_store = "kshana-test"
[[editions]]
id = "pro-annual"
name = "Kshana Pro (annual)"
tier = "pro"
products = ["cli", "desktop"]
days = 365
price = "PLACEHOLDER"
provider_price_id = "{price}"
"#,
        price = if provider == "paddle" {
            "pri_test_pro_annual"
        } else {
            "4242"
        }
    );
    PaymentsConfig::parse(&text).unwrap()
}

fn desktop(now: i64) -> VerifyOptions<'static> {
    let mut o = VerifyOptions::at(now);
    o.product = Some(Product::Desktop);
    o
}

fn ring() -> Keyring {
    Keyring::single(public_key_for(&SEED))
}

fn token_from_outbox(issuer: &Issuer, tid: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(issuer.paths.outbox().join(format!("{tid}.json"))).unwrap(),
    )
    .unwrap();
    v["token"].as_str().unwrap().to_string()
}

#[test]
fn paddle_purchase_renewal_refund_and_refusals() {
    let dir = tmp("paddle");
    let cfg = config(&dir, "paddle", true);

    // Paywall: a checkout URL carrying the licensee and e-mail.
    match paywall::checkout(
        Some(&cfg),
        "pro-annual",
        3,
        "Harbour Pilots Ltd",
        "ops@harbour.example",
    )
    .unwrap()
    {
        Paywall::Checkout { url, provider, .. } => {
            assert_eq!(provider, Provider::Paddle);
            assert_eq!(
                url,
                "https://kshana.dev/checkout?price=pri_test_pro_annual&quantity=3&licensee=Harbour%20Pilots%20Ltd&email=ops%40harbour.example"
            );
        }
        Paywall::Off => panic!("payments are on"),
    }

    let issuer = Issuer::open(cfg, &dir).unwrap();
    let reg = Register::open(issuer.paths.register());
    let p = Provider::Paddle;

    // Purchase.
    let body = fixture("paddle-transaction-completed.json", T0);
    let sig = sign_paddle(&body, SECRET, T0);
    let o = issuer.handle(p, Some(&sig), &body, T0 + 2);
    assert_eq!(o.status, 200, "{}", o.message);
    let tid = o.token_id.unwrap();
    let entry = reg.find(&tid).unwrap().unwrap();
    assert_eq!(entry.licensee, "Harbour Pilots Ltd");
    assert_eq!(entry.seats, 3);
    assert!(entry.source.starts_with("paddle:paddle:evt_"));

    // The app unlocks with the delivered token.
    let token = token_from_outbox(&issuer, &tid);
    let store = Store::open(dir.join("customer"));
    let st = store.activate(&ring(), &token, &desktop(T0 + 60)).unwrap();
    assert_eq!(st.state, State::Active);
    assert_eq!(st.entitlement.tier, Tier::Pro);

    // Replay of the same delivery inside the window: not acted on.
    let o = issuer.handle(p, Some(&sig), &body, T0 + 30);
    assert_eq!(o.status, 200);
    assert!(o.message.starts_with("replay"), "{}", o.message);
    assert!(o.token_id.is_none());
    assert_eq!(reg.verify().unwrap(), 1);
    // Replay after the window: the signed timestamp is stale.
    assert_eq!(issuer.handle(p, Some(&sig), &body, T0 + 3_600).status, 401);

    // Forged: wrong secret, edited body, no header, a Lemon Squeezy-style header.
    let forged = sign_paddle(&body, "guessed", T0);
    assert_eq!(issuer.handle(p, Some(&forged), &body, T0 + 2).status, 401);
    let edited = String::from_utf8(body.clone())
        .unwrap()
        .replace("\"quantity\":3", "\"quantity\":300");
    assert_eq!(
        issuer
            .handle(p, Some(&sig), edited.as_bytes(), T0 + 2)
            .status,
        401
    );
    assert_eq!(issuer.handle(p, None, &body, T0 + 2).status, 401);
    assert_eq!(
        issuer
            .handle(p, Some(&sign_lemonsqueezy(&body, SECRET)), &body, T0 + 2)
            .status,
        401
    );
    // The other provider's endpoint is not open.
    assert_eq!(
        issuer
            .handle(Provider::LemonSqueezy, Some(&sig), &body, T0 + 2)
            .status,
        404
    );
    assert_eq!(
        reg.verify().unwrap(),
        1,
        "a refused webhook changed the register"
    );

    // Renewal a year later.
    let t1 = T0 + 365 * 86_400;
    let body = fixture("paddle-transaction-renewal.json", t1);
    let o = issuer.handle(p, Some(&sign_paddle(&body, SECRET, t1)), &body, t1 + 1);
    assert_eq!(o.status, 200, "{}", o.message);
    let renewed = o.token_id.unwrap();
    assert_eq!(
        reg.find(&renewed).unwrap().unwrap().renews.as_deref(),
        Some(tid.as_str())
    );

    // Out of order: a cancellation that happened before the renewal arrives late.
    let body = fixture("paddle-subscription-canceled.json", t1 - 86_400);
    let o = issuer.handle(p, Some(&sign_paddle(&body, SECRET, t1 + 5)), &body, t1 + 5);
    assert_eq!(o.status, 409, "{}", o.message);
    assert!(!reg.is_revoked(&renewed).unwrap());

    // A full refund of the first transaction revokes the token it issued; the signed
    // revocation list is rewritten and the app stops at the update that carries it.
    let t2 = t1 + 10;
    let body = fixture("paddle-adjustment-refund.json", t2);
    let o = issuer.handle(p, Some(&sign_paddle(&body, SECRET, t2)), &body, t2);
    assert_eq!(o.status, 200, "{}", o.message);
    assert_eq!(o.revoked.as_deref(), Some(tid.as_str()));
    let list = std::fs::read_to_string(issuer.paths.revocations()).unwrap();
    store.install_revocations(&ring(), &list).unwrap();
    assert_eq!(
        store.status(&ring(), &[], &desktop(t2)).state,
        State::Revoked
    );

    // Cancellation in order revokes the current (renewed) token.
    let t3 = t2 + 100;
    let body = fixture("paddle-subscription-canceled.json", t3);
    // A new event ID, since this is a separate delivery from the refused one.
    let body = String::from_utf8(body)
        .unwrap()
        .replace(
            "evt_01test0000000000000000003",
            "evt_01test0000000000000000005",
        )
        .into_bytes();
    let o = issuer.handle(p, Some(&sign_paddle(&body, SECRET, t3)), &body, t3);
    assert_eq!(o.status, 200, "{}", o.message);
    assert_eq!(o.revoked.as_deref(), Some(renewed.as_str()));
    assert_eq!(reg.verify().unwrap(), 4); // issue, renew, revoke, revoke
}

#[test]
fn paddle_refund_before_purchase_is_deferred() {
    let dir = tmp("paddle-early");
    let issuer = Issuer::open(config(&dir, "paddle", true), &dir).unwrap();
    let body = fixture("paddle-adjustment-refund.json", T0);
    let o = issuer.handle(
        Provider::Paddle,
        Some(&sign_paddle(&body, SECRET, T0)),
        &body,
        T0,
    );
    assert_eq!(o.status, 409, "{}", o.message);
    // Not marked as handled: the provider's retry is processed once the purchase is in.
    let buy = fixture("paddle-transaction-completed.json", T0 - 10);
    assert_eq!(
        issuer
            .handle(
                Provider::Paddle,
                Some(&sign_paddle(&buy, SECRET, T0)),
                &buy,
                T0
            )
            .status,
        200
    );
    let o = issuer.handle(
        Provider::Paddle,
        Some(&sign_paddle(&body, SECRET, T0 + 60)),
        &body,
        T0 + 60,
    );
    assert_eq!(o.status, 200, "{}", o.message);
    assert!(o.revoked.is_some());
}

#[test]
fn lemonsqueezy_order_subscription_renewal_expiry_and_refusals() {
    let dir = tmp("ls");
    let cfg = config(&dir, "lemonsqueezy", true);
    match paywall::checkout(Some(&cfg), "pro-annual", 2, "Grid Timing Lab", "buyer@grid.example").unwrap() {
        Paywall::Checkout { url, .. } => assert_eq!(
            url,
            "https://kshana-test.lemonsqueezy.com/checkout/buy/4242?quantity=2&checkout%5Bemail%5D=buyer%40grid.example&checkout%5Bcustom%5D%5Blicensee%5D=Grid%20Timing%20Lab"
        ),
        Paywall::Off => panic!("payments are on"),
    }
    let issuer = Issuer::open(cfg, &dir).unwrap();
    let reg = Register::open(issuer.paths.register());
    let p = Provider::LemonSqueezy;
    let send = |name: &str, t: i64| {
        let b = fixture(name, t);
        issuer.handle(p, Some(&sign_lemonsqueezy(&b, SECRET)), &b, t + 1)
    };

    let o = send("lemonsqueezy-order-created.json", T0);
    assert_eq!(o.status, 200, "{}", o.message);
    let tid = o.token_id.unwrap();
    let e = reg.find(&tid).unwrap().unwrap();
    assert_eq!((e.licensee.as_str(), e.seats), ("Grid Timing Lab", 2));
    let token = token_from_outbox(&issuer, &tid);
    let store = Store::open(dir.join("customer"));
    assert_eq!(
        store
            .activate(&ring(), &token, &desktop(T0 + 5))
            .unwrap()
            .entitlement
            .tier,
        Tier::Pro
    );

    // Exact replay: not acted on (Lemon Squeezy signs no timestamp; the body digest
    // catches it).
    let o = send("lemonsqueezy-order-created.json", T0);
    assert!(o.message.starts_with("replay"), "{}", o.message);
    assert_eq!(reg.verify().unwrap(), 1);
    // Forged.
    let b = fixture("lemonsqueezy-order-created.json", T0 + 1);
    assert_eq!(
        issuer
            .handle(p, Some(&sign_lemonsqueezy(&b, "guessed")), &b, T0)
            .status,
        401
    );
    assert_eq!(issuer.handle(p, Some("not-hex"), &b, T0).status, 401);

    assert_eq!(
        send("lemonsqueezy-subscription-created.json", T0 + 10).status,
        200
    );
    let t1 = T0 + 365 * 86_400;
    let o = send("lemonsqueezy-payment-renewal.json", t1);
    assert_eq!(o.status, 200, "{}", o.message);
    let renewed = o.token_id.unwrap();
    // Out of order: an older state of the subscription after the renewal.
    let o = send("lemonsqueezy-subscription-expired.json", t1 - 1_000);
    assert_eq!(o.status, 409, "{}", o.message);
    // In order: the subscription expires; its current token is revoked.
    let o = send("lemonsqueezy-subscription-expired.json", t1 + 400 * 86_400);
    assert_eq!(o.status, 200, "{}", o.message);
    assert_eq!(o.revoked.as_deref(), Some(renewed.as_str()));
    // A refund of the first order revokes the first token.
    let o = send("lemonsqueezy-order-refunded.json", T0 + 20);
    assert_eq!(o.status, 200, "{}", o.message);
    assert_eq!(o.revoked.as_deref(), Some(tid.as_str()));
}

#[test]
fn switch_off_means_no_checkout_no_listener_no_delivery() {
    let dir = tmp("off");
    let cfg = config(&dir, "paddle", false);
    assert_eq!(
        paywall::checkout(Some(&cfg), "pro-annual", 1, "x", "y").unwrap(),
        Paywall::Off
    );
    assert_eq!(
        paywall::checkout(None, "pro-annual", 1, "x", "y").unwrap(),
        Paywall::Off
    );
    assert!(
        Issuer::open(cfg.clone(), &dir).is_err(),
        "the listener must refuse to start"
    );
    // Even an issuer built by hand answers 404 and writes nothing while off.
    let issuer = Issuer {
        config: cfg,
        secret: SECRET.into(),
        seed: SEED,
        paths: kshana_pro::payments::webhook::Paths {
            dir: dir.join("data"),
        },
        stripe_secret: None,
    };
    let body = fixture("paddle-transaction-completed.json", T0);
    let o = issuer.handle(
        Provider::Paddle,
        Some(&sign_paddle(&body, SECRET, T0)),
        &body,
        T0,
    );
    assert_eq!(o.status, 404);
    assert!(!dir.join("data").exists());

    // The committed configuration is off, with every price PLACEHOLDER.
    let committed = PaymentsConfig::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("commercial/payments.toml"),
    )
    .unwrap();
    assert!(!committed.enabled);
    assert!(committed.editions.iter().all(|e| e.price == "PLACEHOLDER"));
    assert_eq!(committed.placeholders().len(), 2 * committed.editions.len());
    // Live mode is refused while anything is PLACEHOLDER.
    let live = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("commercial/payments.toml"),
    )
    .unwrap()
    .replace("mode = \"test\"", "mode = \"live\"");
    assert!(PaymentsConfig::parse(&live)
        .unwrap_err()
        .to_string()
        .contains("live mode"));

    // The commands: checkout prints that payments are off; serve refuses to start.
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap()
    };
    let out = run(&["payments", "checkout", "--edition", "pro-annual"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("payments are off"));
    assert!(!run(&["payments", "serve", "--addr", "127.0.0.1:0"])
        .status
        .success());
}

#[test]
fn payments_code_makes_no_outbound_connection() {
    // The payments module only listens; it never opens a connection or runs a program
    // (the provider calls in). A source check, so a later change cannot add one quietly.
    // One exception, Stripe's checkout call (`stripe_transport.rs`), reachable only with
    // `[stripe] enabled` (tests/stripe_payments.rs checks the switch refuses first).
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/payments");
    for f in std::fs::read_dir(&dir).unwrap() {
        let p = f.unwrap().path();
        if p.file_name().is_some_and(|n| n == "stripe_transport.rs") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        for bad in [
            "TcpStream::connect",
            "UdpSocket",
            "Command::new",
            "curl",
            "reqwest",
            "ureq",
        ] {
            assert!(!src.contains(bad), "{} contains {bad}", p.display());
        }
    }
}

#[test]
fn webhook_over_a_socket() {
    let dir = tmp("socket");
    let issuer = Issuer::open(config(&dir, "paddle", true), &dir).unwrap();
    let l = server::start(issuer, "127.0.0.1:0").unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let body = fixture("paddle-transaction-completed.json", now);
    let post = |sig: &str| {
        let mut s = TcpStream::connect(l.addr).unwrap();
        write!(
            s,
            "POST /webhooks/paddle HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nPaddle-Signature: {sig}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .unwrap();
        s.write_all(&body).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    };
    assert!(post(&sign_paddle(&body, "guessed", now)).starts_with("HTTP/1.1 401"));
    let ok = post(&sign_paddle(&body, SECRET, now));
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("issued"), "{ok}");
    l.stop();
}
