// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
// Stripe, built in and switched off (owner, 15 November 2026). Synthetic events signed
// here with a test endpoint secret; no network.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use kshana_pro::license::register::Register;
use kshana_pro::license::revocation;
use kshana_pro::license::{public_key_for, Keyring};
use kshana_pro::payments::config::Provider;
use kshana_pro::payments::stripe::{self, StripeTransport};
use kshana_pro::payments::webhook::Issuer;
use kshana_pro::payments::{server, PaymentsConfig};

const SEED: [u8; 32] = [0x33; 32];
const WHSEC: &str = "whsec_test_synthetic_secret";
const T0: i64 = 1_800_000_000;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kpro-stripe-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn config(dir: &Path, stripe_on: bool) -> PaymentsConfig {
    std::fs::write(dir.join("signing-key"), hex::encode(SEED)).unwrap();
    PaymentsConfig::parse(&format!(
        r#"
enabled = false
provider = "paddle"
mode = "test"
signing_key_file = "signing-key"
data_dir = "data"
[stripe]
enabled = {stripe_on}
success_url = "https://kshana.dev/thanks"
cancel_url = "https://kshana.dev/pricing"
[[editions]]
id = "pro-annual"
name = "Kshana Pro (annual)"
tier = "pro"
products = ["cli", "desktop"]
days = 365
price = "PLACEHOLDER"
provider_price_id = "PLACEHOLDER"
stripe_price_id = "price_test_pro_annual"
[[editions]]
id = "forensics-pack"
name = "Evidence pack"
tier = "pro"
products = ["desktop"]
days = 30
price = "PLACEHOLDER"
provider_price_id = "PLACEHOLDER"
stripe_price_id = "price_test_forensics"
stripe_mode = "payment"
"#
    ))
    .unwrap()
}

fn env(k: &str) -> Option<String> {
    match k {
        "STRIPE_WEBHOOK_SECRET" => Some(WHSEC.into()),
        "STRIPE_SECRET_KEY" => Some("sk_test_synthetic".into()),
        _ => None,
    }
}

fn event(id: &str, kind: &str, created: i64, object: serde_json::Value) -> Vec<u8> {
    serde_json::json!({
        "id": id, "object": "event", "type": kind, "created": created,
        "livemode": false, "data": { "object": object }
    })
    .to_string()
    .into_bytes()
}

fn checkout_sub(created: i64) -> Vec<u8> {
    event(
        "evt_cs_1",
        "checkout.session.completed",
        created,
        serde_json::json!({
            "id": "cs_test_1", "object": "checkout.session", "mode": "subscription",
            "payment_status": "paid", "subscription": "sub_1", "invoice": "in_1",
            "payment_intent": null,
            "customer_details": { "email": "ops@harbour.example" },
            "metadata": { "edition": "pro-annual", "licensee": "Harbour Pilots Ltd", "quantity": "3" }
        }),
    )
}

#[test]
fn stripe_purchase_renewal_refund_and_refusals() {
    let dir = tmp("flow");
    let issuer = Issuer::open_with_env(config(&dir, true), &dir, &env).unwrap();
    let reg = Register::open(issuer.paths.register());
    let p = Provider::Stripe;
    let send = |body: &[u8], ts: i64, now: i64| {
        issuer.handle(p, Some(&stripe::sign(body, WHSEC, ts)), body, now)
    };

    // Paid checkout: one licence, 3 seats, a 35-day token.
    let body = checkout_sub(T0);
    let o = send(&body, T0, T0 + 1);
    assert_eq!(o.status, 200, "{}", o.message);
    let tid = o.token_id.clone().unwrap();
    let e = reg.find(&tid).unwrap().unwrap();
    assert_eq!((e.licensee.as_str(), e.seats), ("Harbour Pilots Ltd", 3));
    assert_eq!(e.expires_unix, T0 + 1 + 35 * 86_400);
    assert!(e.source.starts_with("stripe:stripe:evt_cs_1"));

    // Replayed (same event id): not acted on. Stale signature: refused. Forged: refused.
    let o = send(&body, T0 + 5, T0 + 6);
    assert!(o.message.starts_with("replay"), "{}", o.message);
    assert_eq!(send(&body, T0, T0 + 3_600).status, 401);
    assert_eq!(
        issuer
            .handle(
                p,
                Some(&stripe::sign(&body, "whsec_guess", T0)),
                &body,
                T0 + 1
            )
            .status,
        401
    );
    let edited = String::from_utf8(body.clone())
        .unwrap()
        .replace("\"3\"", "\"300\"");
    assert_eq!(
        issuer
            .handle(
                p,
                Some(&stripe::sign(&body, WHSEC, T0)),
                edited.as_bytes(),
                T0 + 1
            )
            .status,
        401
    );
    assert_eq!(issuer.handle(p, None, &body, T0 + 1).status, 401);
    // Idempotent: the same session delivered again under another event id issues nothing.
    let again = String::from_utf8(body.clone())
        .unwrap()
        .replace("evt_cs_1", "evt_cs_1b")
        .replace(
            "checkout.session.completed",
            "checkout.session.async_payment_succeeded",
        );
    let o = send(again.as_bytes(), T0 + 10, T0 + 11);
    assert_eq!(o.status, 200);
    assert!(o.token_id.is_none(), "{}", o.message);
    assert_eq!(reg.verify().unwrap(), 1);

    // Renewal invoice a year later: the licence is paid on and a fresh token follows.
    let t1 = T0 + 365 * 86_400;
    let inv = event(
        "evt_inv_2",
        "invoice.paid",
        t1,
        serde_json::json!({ "id": "in_2", "object": "invoice", "billing_reason": "subscription_cycle",
            "subscription": "sub_1", "payment_intent": "pi_2" }),
    );
    let o = send(&inv, t1, t1 + 1);
    assert_eq!(o.status, 200, "{}", o.message);
    let renewed = o.token_id.unwrap();
    assert_eq!(
        reg.find(&renewed).unwrap().unwrap().renews.as_deref(),
        Some(tid.as_str())
    );
    // The first invoice of the subscription issues nothing (the checkout did).
    let first_inv = event(
        "evt_inv_1",
        "invoice.paid",
        t1 + 2,
        serde_json::json!({ "id": "in_1", "object": "invoice", "billing_reason": "subscription_create",
            "subscription": "sub_1" }),
    );
    assert!(send(&first_inv, t1 + 2, t1 + 3).token_id.is_none());

    // Out of order: a cancellation created before the renewal, arriving late.
    let old_cancel = event(
        "evt_sub_del_old",
        "customer.subscription.deleted",
        t1 - 86_400,
        serde_json::json!({ "id": "sub_1", "object": "subscription", "status": "canceled" }),
    );
    assert_eq!(send(&old_cancel, t1 + 4, t1 + 4).status, 409);
    assert!(!reg.is_revoked(&renewed).unwrap());

    // Full refund of the first invoice's charge: the whole licence is revoked, and the
    // signed revocation list is rewritten.
    let refund = event(
        "evt_ch_ref",
        "charge.refunded",
        t1 + 10,
        serde_json::json!({ "id": "ch_1", "object": "charge", "refunded": true,
            "payment_intent": null, "invoice": "in_1" }),
    );
    let o = send(&refund, t1 + 10, t1 + 10);
    assert_eq!(o.status, 200, "{}", o.message);
    assert_eq!(o.revoked.as_deref(), Some(tid.as_str()));
    let list = revocation::verify(
        &Keyring::single(public_key_for(&SEED)),
        &std::fs::read_to_string(issuer.paths.revocations()).unwrap(),
    )
    .unwrap();
    assert!(list.is_revoked(&tid) && list.is_revoked(&renewed));
    // A partial refund is not a revocation.
    let partial = event(
        "evt_ch_part",
        "charge.refunded",
        t1 + 20,
        serde_json::json!({ "id": "ch_2", "object": "charge", "refunded": false, "payment_intent": "pi_2" }),
    );
    assert!(send(&partial, t1 + 20, t1 + 20)
        .message
        .starts_with("ignored"));
}

#[test]
fn stripe_one_time_purchase_and_chargeback() {
    let dir = tmp("chargeback");
    let issuer = Issuer::open_with_env(config(&dir, true), &dir, &env).unwrap();
    let p = Provider::Stripe;
    let send = |body: &[u8], t: i64| issuer.handle(p, Some(&stripe::sign(body, WHSEC, t)), body, t);
    let buy = event(
        "evt_pay_1",
        "checkout.session.completed",
        T0,
        serde_json::json!({ "id": "cs_test_2", "mode": "payment", "payment_status": "paid",
            "payment_intent": "pi_9", "customer_email": "lab@example.org",
            "metadata": { "edition": "forensics-pack", "licensee": "Lab" } }),
    );
    let o = send(&buy, T0);
    assert_eq!(o.status, 200, "{}", o.message);
    let tid = o.token_id.unwrap();
    // Unpaid checkouts (async methods still pending) issue nothing.
    let pending = event(
        "evt_pay_2",
        "checkout.session.completed",
        T0 + 1,
        serde_json::json!({ "id": "cs_test_3", "mode": "payment", "payment_status": "unpaid",
            "payment_intent": "pi_10", "metadata": { "edition": "forensics-pack" } }),
    );
    assert!(send(&pending, T0 + 1).token_id.is_none());
    // A dispute (chargeback) revokes.
    let dispute = event(
        "evt_dp_1",
        "charge.dispute.created",
        T0 + 100,
        serde_json::json!({ "id": "dp_1", "payment_intent": "pi_9" }),
    );
    let o = send(&dispute, T0 + 100);
    assert_eq!(o.revoked.as_deref(), Some(tid.as_str()), "{}", o.message);
    // A dispute for an order not seen yet: deferred for the retry.
    let unknown = event(
        "evt_dp_2",
        "charge.dispute.created",
        T0 + 101,
        serde_json::json!({ "id": "dp_2", "payment_intent": "pi_unknown" }),
    );
    assert_eq!(send(&unknown, T0 + 101).status, 409);
    // Subscription ended through status (payment failed and not recovered): revoke path.
    let unpaid = event(
        "evt_sub_upd",
        "customer.subscription.updated",
        T0 + 200,
        serde_json::json!({ "id": "sub_none", "status": "unpaid" }),
    );
    assert_eq!(send(&unpaid, T0 + 200).status, 409);
}

/// One recorded call: URL, key, idempotency key, form.
type Call = (String, String, String, Vec<(String, String)>);

/// A transport that records what it would send; panics if called when it must not be.
struct Recorder {
    calls: RefCell<Vec<Call>>,
    forbid: bool,
}

impl StripeTransport for Recorder {
    fn post_form(
        &self,
        url: &str,
        key: &str,
        idem: &str,
        form: &[(String, String)],
    ) -> Result<String, String> {
        assert!(!self.forbid, "the transport was called while Stripe is off");
        self.calls
            .borrow_mut()
            .push((url.into(), key.into(), idem.into(), form.to_vec()));
        Ok(r#"{"id":"cs_test_new","url":"https://checkout.stripe.com/c/pay/cs_test_new"}"#.into())
    }
}

#[test]
fn switch_off_means_no_checkout_call_no_endpoint_no_key_read() {
    let dir = tmp("off");
    let cfg = config(&dir, false);
    let t = Recorder {
        calls: RefCell::new(Vec::new()),
        forbid: true,
    };
    let e = stripe::create_checkout_session(&cfg, &t, &env, "pro-annual", 1, "x", "y")
        .unwrap_err()
        .to_string();
    assert!(e.contains("Stripe is off"), "{e}");
    // Both switches off: the listener does not start, and no key is read.
    let read = RefCell::new(Vec::<String>::new());
    let spy = |k: &str| {
        read.borrow_mut().push(k.to_string());
        env(k)
    };
    assert!(Issuer::open_with_env(cfg.clone(), &dir, &spy).is_err());
    assert!(read.borrow().is_empty());
    // A hand-built issuer still answers 404 on /webhooks/stripe.
    let issuer = Issuer {
        config: cfg,
        secret: String::new(),
        seed: SEED,
        paths: kshana_pro::payments::webhook::Paths {
            dir: dir.join("data"),
        },
        stripe_secret: Some(WHSEC.into()),
    };
    let body = checkout_sub(T0);
    let o = issuer.handle(
        Provider::Stripe,
        Some(&stripe::sign(&body, WHSEC, T0)),
        &body,
        T0,
    );
    assert_eq!(o.status, 404);
    let req = server::RawRequest {
        method: "POST".into(),
        path: "/webhooks/stripe".into(),
        headers: vec![("Stripe-Signature".into(), stripe::sign(&body, WHSEC, T0))],
        body,
    };
    assert_eq!(server::route(&issuer, &req, T0).status, 404);
    assert!(!dir.join("data").exists());

    // The committed configuration has Stripe off.
    let committed = PaymentsConfig::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("commercial/payments.toml"),
    )
    .unwrap();
    assert!(!committed.stripe.enabled);
    // Stripe on but no webhook secret in the environment: refused.
    assert!(Issuer::open_with_env(config(&dir, true), &dir, &|_| None).is_err());
}

#[test]
fn checkout_session_when_on() {
    let dir = tmp("checkout");
    let cfg = config(&dir, true);
    let t = Recorder {
        calls: RefCell::new(Vec::new()),
        forbid: false,
    };
    let s = stripe::create_checkout_session(
        &cfg,
        &t,
        &env,
        "pro-annual",
        3,
        "Harbour Pilots Ltd",
        "ops@harbour.example",
    )
    .unwrap();
    assert_eq!(s.id, "cs_test_new");
    let calls = t.calls.borrow();
    let (url, key, idem, form) = &calls[0];
    assert_eq!(url, stripe::API_CHECKOUT_SESSIONS);
    assert_eq!(key, "sk_test_synthetic");
    assert!(idem.starts_with("kshana-"));
    let get = |k: &str| form.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    assert_eq!(get("mode"), Some("subscription"));
    assert_eq!(get("line_items[0][price]"), Some("price_test_pro_annual"));
    assert_eq!(get("line_items[0][quantity]"), Some("3"));
    assert_eq!(get("metadata[edition]"), Some("pro-annual"));
    assert_eq!(get("metadata[licensee]"), Some("Harbour Pilots Ltd"));
    assert_eq!(get("customer_email"), Some("ops@harbour.example"));
    drop(calls);
    // A live key in test mode, or no key: refused before any call.
    let live = |k: &str| (k == "STRIPE_SECRET_KEY").then(|| "sk_live_x".to_string());
    assert!(stripe::create_checkout_session(&cfg, &t, &live, "pro-annual", 1, "a", "").is_err());
    assert!(
        stripe::create_checkout_session(&cfg, &t, &|_| None, "pro-annual", 1, "a", "").is_err()
    );
    assert_eq!(t.calls.borrow().len(), 1);
    // Stripe as the merchant-of-record provider is refused; live mode needs every Stripe
    // price ID.
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("commercial/payments.toml"),
    )
    .unwrap();
    assert!(
        PaymentsConfig::parse(&text.replace("provider = \"paddle\"", "provider = \"stripe\""))
            .is_err()
    );
}
