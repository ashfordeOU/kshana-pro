// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Stripe, built in and switched off (owner, 15 November 2026: invoice only for now;
//! Stripe fully built for activation at any time).
//!
//! The switch is `[stripe] enabled` in `payments.toml`, false by default. Off: no
//! checkout session is created (the transport is never called), the `/webhooks/stripe`
//! endpoint answers 404 and the issuer does not read any Stripe key. Keys come from the
//! environment only: `STRIPE_SECRET_KEY` (checkout; `sk_test_…` in test mode, `sk_live_…`
//! or a restricted `rk_…` key in live mode) and `STRIPE_WEBHOOK_SECRET` (`whsec_…`).
//!
//! * **Webhook signature**, Stripe's documented scheme: header
//!   `Stripe-Signature: t=<unix>,v1=<hex>[,v1=<hex>…][,v0=…]`; each `v1` is HMAC-SHA256,
//!   keyed with the endpoint secret, over `<t>.<raw body>`. A timestamp outside the
//!   tolerance (default 300 s) is refused; `v0` (test-mode legacy) is ignored; several
//!   `v1` values are accepted while a secret is rolled.
//! * **Events used**: `checkout.session.completed` and
//!   `checkout.session.async_payment_succeeded` (paid: issue), `invoice.paid` with
//!   `billing_reason = subscription_cycle` (renewal), `customer.subscription.deleted` and
//!   `customer.subscription.updated` to `canceled`, `unpaid` or `incomplete_expired`
//!   (ended: revoke), `charge.refunded` when fully refunded and `charge.dispute.created`
//!   (refund, chargeback: revoke). Everything else is acknowledged and ignored.
//! * Replay (event id), order (event `created`) and idempotent issue are handled by the
//!   common issuer ([`super::webhook`]).

use kshana::api::KshanaError;

use super::config::{Edition, Mode, PaymentsConfig, Provider};
use super::provider::{ct_eq, hmac_sha256, Action, Event, SigError};

/// The API host for checkout sessions.
pub const API_CHECKOUT_SESSIONS: &str = "https://api.stripe.com/v1/checkout/sessions";
/// Environment variable holding the API secret key.
pub const ENV_SECRET_KEY: &str = "STRIPE_SECRET_KEY";
/// Environment variable holding the webhook endpoint secret.
pub const ENV_WEBHOOK_SECRET: &str = "STRIPE_WEBHOOK_SECRET";

/// Check a `Stripe-Signature` header over `body` at `now`; returns the signed timestamp.
pub fn verify(
    header: Option<&str>,
    body: &[u8],
    secret: &str,
    now: i64,
    tolerance_secs: i64,
) -> Result<i64, SigError> {
    let header = header.ok_or(SigError::Missing)?;
    let mut ts: Option<i64> = None;
    let mut v1: Vec<Vec<u8>> = Vec::new();
    for part in header.split(',') {
        let (k, v) = part.trim().split_once('=').ok_or(SigError::Malformed)?;
        match k.trim() {
            "t" => {
                if ts.is_some() {
                    return Err(SigError::Malformed);
                }
                ts = Some(v.trim().parse().map_err(|_| SigError::Malformed)?)
            }
            "v1" => v1.push(hex::decode(v.trim()).map_err(|_| SigError::Malformed)?),
            _ => {}
        }
    }
    let ts = ts.ok_or(SigError::Malformed)?;
    if v1.is_empty() {
        return Err(SigError::Malformed);
    }
    let age = now - ts;
    if age.abs() > tolerance_secs {
        return Err(SigError::Stale { age_secs: age });
    }
    let mut msg = format!("{ts}.").into_bytes();
    msg.extend_from_slice(body);
    let want = hmac_sha256(secret.as_bytes(), &msg);
    if v1.iter().any(|h| ct_eq(h, &want)) {
        Ok(ts)
    } else {
        Err(SigError::Mismatch)
    }
}

/// Sign a body as Stripe does (tests and fixtures).
pub fn sign(body: &[u8], secret: &str, ts: i64) -> String {
    let mut msg = format!("{ts}.").into_bytes();
    msg.extend_from_slice(body);
    format!(
        "t={ts},v1={}",
        hex::encode(hmac_sha256(secret.as_bytes(), &msg))
    )
}

fn s(v: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    match cur {
        serde_json::Value::String(x) if !x.is_empty() => Some(x.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The order key of a payment: its payment intent, else its invoice.
fn pay_key(pi: Option<String>, inv: Option<String>) -> Option<String> {
    pi.map(|p| format!("stripe-pi:{p}"))
        .or_else(|| inv.map(|i| format!("stripe-inv:{i}")))
}

/// Decode a verified Stripe event.
pub fn decode(body: &[u8]) -> Result<Event, String> {
    let v: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format!("event is not JSON: {e}"))?;
    let id = s(&v, &["id"]).ok_or("id missing")?;
    let kind = s(&v, &["type"]).ok_or("type missing")?;
    let created = v["created"].as_i64().ok_or("created missing")?;
    let o = &v["data"]["object"];
    let oid = s(o, &["id"]).unwrap_or_default();
    let sub_of_invoice = |o: &serde_json::Value| {
        s(o, &["subscription"])
            .or_else(|| s(o, &["parent", "subscription_details", "subscription"]))
    };
    let (object, action) = match kind.as_str() {
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
            let paid = matches!(
                s(o, &["payment_status"]).as_deref(),
                Some("paid") | Some("no_payment_required")
            );
            let sub = s(o, &["subscription"]);
            let order = pay_key(s(o, &["payment_intent"]), s(o, &["invoice"]))
                .unwrap_or_else(|| format!("stripe-cs:{oid}"));
            let object = sub.clone().unwrap_or_else(|| order.clone());
            if !paid {
                (object, Action::Ignore("checkout not paid yet".into()))
            } else {
                let edition = s(o, &["metadata", "edition"]).ok_or("metadata.edition missing")?;
                let quantity = s(o, &["metadata", "quantity"])
                    .and_then(|q| q.parse::<u32>().ok())
                    .unwrap_or(1)
                    .clamp(1, 10_000);
                (
                    object,
                    Action::Purchase {
                        order_id: order,
                        subscription_id: sub.map(|x| format!("stripe-sub:{x}")),
                        price_id: format!("edition:{edition}"),
                        quantity,
                        licensee: s(o, &["metadata", "licensee"]).unwrap_or_default(),
                        email: s(o, &["customer_details", "email"])
                            .or_else(|| s(o, &["customer_email"]))
                            .unwrap_or_default(),
                    },
                )
            }
        }
        "invoice.paid" => {
            let sub = sub_of_invoice(o);
            let reason = s(o, &["billing_reason"]).unwrap_or_default();
            match (sub, reason.as_str()) {
                (Some(sub), "subscription_cycle") => (
                    sub.clone(),
                    Action::Renewal {
                        subscription_id: format!("stripe-sub:{sub}"),
                        order_id: pay_key(s(o, &["payment_intent"]), Some(oid.clone()))
                            .unwrap_or_default(),
                    },
                ),
                (sub, r) => (
                    sub.unwrap_or(oid),
                    Action::Ignore(format!(
                        "invoice ({r}); the checkout issues the first token"
                    )),
                ),
            }
        }
        "customer.subscription.deleted" => (
            oid.clone(),
            Action::Cancel {
                subscription_id: format!("stripe-sub:{oid}"),
            },
        ),
        "customer.subscription.updated" => {
            let status = s(o, &["status"]).unwrap_or_default();
            if matches!(
                status.as_str(),
                "canceled" | "unpaid" | "incomplete_expired"
            ) {
                (
                    oid.clone(),
                    Action::Cancel {
                        subscription_id: format!("stripe-sub:{oid}"),
                    },
                )
            } else {
                (oid, Action::Ignore(format!("subscription {status}")))
            }
        }
        "charge.refunded" => {
            let order = pay_key(s(o, &["payment_intent"]), s(o, &["invoice"]))
                .ok_or("charge has no payment intent or invoice")?;
            if o["refunded"].as_bool() == Some(true) {
                (order.clone(), Action::Refund { order_id: order })
            } else {
                (order, Action::Ignore("partial refund".into()))
            }
        }
        "charge.dispute.created" => {
            let order =
                pay_key(s(o, &["payment_intent"]), None).ok_or("dispute has no payment intent")?;
            (order.clone(), Action::Refund { order_id: order })
        }
        other => (
            oid,
            Action::Ignore(format!("event type {other} is not used")),
        ),
    };
    Ok(Event {
        provider: Provider::Stripe,
        key: format!("stripe:{id}"),
        occurred_unix: created,
        object: format!("stripe:{object}"),
        action,
    })
}

/// Other names under which a checkout's payment can be refunded or disputed (its payment
/// intent, its invoice, the session). Used to find the order a refund names.
pub fn aliases(body: &[u8]) -> Vec<String> {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let o = &v["data"]["object"];
    let mut out = Vec::new();
    if let Some(p) = s(o, &["payment_intent"]) {
        out.push(format!("stripe-pi:{p}"));
    }
    if let Some(i) = s(o, &["invoice"]) {
        out.push(format!("stripe-inv:{i}"));
    }
    if let Some(c) = s(o, &["id"]) {
        out.push(format!("stripe-cs:{c}"));
    }
    out
}

// ─── Checkout ─────────────────────────────────────────────────────────────────────────

/// Sends one form-encoded POST to Stripe. The only outbound call in payments, and only
/// reachable when `[stripe] enabled` is true ([`create_checkout_session`]).
pub trait StripeTransport {
    /// POST `form` to `url` with the bearer key and idempotency key; the response body.
    fn post_form(
        &self,
        url: &str,
        secret_key: &str,
        idempotency_key: &str,
        form: &[(String, String)],
    ) -> Result<String, String>;
}

/// A created checkout session.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckoutSession {
    pub id: String,
    pub url: String,
}

/// The form fields of a checkout session for `ed`: the edition, licensee and quantity go
/// in `metadata` (the webhook issues from them) and in the subscription's metadata.
pub fn checkout_form(
    cfg: &PaymentsConfig,
    ed: &Edition,
    quantity: u32,
    licensee: &str,
    email: &str,
) -> Vec<(String, String)> {
    let mode = ed
        .stripe_mode
        .clone()
        .unwrap_or_else(|| "subscription".into());
    let mut f: Vec<(String, String)> = vec![
        ("mode".into(), mode.clone()),
        ("line_items[0][price]".into(), ed.stripe_price_id.clone()),
        (
            "line_items[0][quantity]".into(),
            quantity.max(1).to_string(),
        ),
        ("success_url".into(), cfg.stripe.success_url.clone()),
        ("cancel_url".into(), cfg.stripe.cancel_url.clone()),
        ("metadata[edition]".into(), ed.id.clone()),
        ("metadata[licensee]".into(), licensee.to_string()),
        ("metadata[quantity]".into(), quantity.max(1).to_string()),
        ("client_reference_id".into(), ed.id.clone()),
    ];
    if !email.trim().is_empty() {
        f.push(("customer_email".into(), email.trim().to_string()));
    }
    if mode == "subscription" {
        f.push(("subscription_data[metadata][edition]".into(), ed.id.clone()));
        f.push((
            "subscription_data[metadata][licensee]".into(),
            licensee.to_string(),
        ));
    }
    f
}

/// Create a checkout session. Refused, with no call to `transport`, while Stripe is off;
/// the secret key comes from `env` (the process environment in production) and must
/// match the mode.
pub fn create_checkout_session(
    cfg: &PaymentsConfig,
    transport: &dyn StripeTransport,
    env: &dyn Fn(&str) -> Option<String>,
    edition: &str,
    quantity: u32,
    licensee: &str,
    email: &str,
) -> Result<CheckoutSession, KshanaError> {
    if !cfg.stripe.enabled {
        return Err(KshanaError::Unsupported(
            "Stripe is off (payments.stripe.enabled = false); licences are issued by invoice"
                .into(),
        ));
    }
    let ed = cfg
        .edition(edition)
        .ok_or_else(|| KshanaError::InvalidInput(format!("no edition '{edition}'")))?;
    if ed.stripe_price_id.trim().is_empty() || ed.stripe_price_id == super::PLACEHOLDER {
        return Err(KshanaError::InvalidInput(format!(
            "edition '{edition}' has no Stripe price ID yet"
        )));
    }
    let key = env(ENV_SECRET_KEY)
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .ok_or_else(|| KshanaError::InvalidInput(format!("{ENV_SECRET_KEY} is not set")))?;
    let ok = match cfg.mode {
        Mode::Test => key.starts_with("sk_test_") || key.starts_with("rk_test_"),
        Mode::Live => key.starts_with("sk_live_") || key.starts_with("rk_live_"),
    };
    if !ok {
        return Err(KshanaError::InvalidInput(format!(
            "{ENV_SECRET_KEY} does not match the {} mode",
            if cfg.mode == Mode::Live {
                "live"
            } else {
                "test"
            }
        )));
    }
    let form = checkout_form(cfg, ed, quantity, licensee, email);
    let idem = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        for (k, v) in &form {
            h.update(k.as_bytes());
            h.update([0]);
            h.update(v.as_bytes());
            h.update([0]);
        }
        // One session per buyer and edition per hour: a retried request returns the same
        // session instead of a second one.
        let hour = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 3600)
            .unwrap_or(0);
        h.update(hour.to_be_bytes());
        format!("kshana-{}", hex::encode(&h.finalize()[..16]))
    };
    let body = transport
        .post_form(API_CHECKOUT_SESSIONS, &key, &idem, &form)
        .map_err(|e| KshanaError::InvalidInput(format!("Stripe: {e}")))?;
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| KshanaError::InvalidInput(format!("Stripe answer: {e}")))?;
    if let Some(m) = v["error"]["message"].as_str() {
        return Err(KshanaError::InvalidInput(format!("Stripe: {m}")));
    }
    match (v["id"].as_str(), v["url"].as_str()) {
        (Some(id), Some(url)) if id.starts_with("cs_") && url.starts_with("https://") => {
            Ok(CheckoutSession {
                id: id.into(),
                url: url.into(),
            })
        }
        _ => Err(KshanaError::InvalidInput(
            "Stripe answer has no session id and URL".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_scheme() {
        let body = br#"{"id":"evt_1"}"#;
        let h = sign(body, "whsec_test", 1_000);
        assert_eq!(verify(Some(&h), body, "whsec_test", 1_100, 300), Ok(1_000));
        assert_eq!(
            verify(Some(&h), body, "whsec_other", 1_100, 300),
            Err(SigError::Mismatch)
        );
        assert_eq!(
            verify(Some(&h), b"{}", "whsec_test", 1_100, 300),
            Err(SigError::Mismatch)
        );
        assert!(matches!(
            verify(Some(&h), body, "whsec_test", 1_301, 300),
            Err(SigError::Stale { .. })
        ));
        assert!(matches!(
            verify(Some(&h), body, "whsec_test", 699, 300),
            Err(SigError::Stale { .. })
        ));
        assert_eq!(
            verify(None, body, "whsec_test", 1_000, 300),
            Err(SigError::Missing)
        );
        assert_eq!(
            verify(Some("t=1000"), body, "whsec_test", 1_000, 300),
            Err(SigError::Malformed)
        );
        assert_eq!(
            verify(Some("t=1,t=2,v1=00"), body, "whsec_test", 1, 300),
            Err(SigError::Malformed)
        );
        // Secret roll: two v1 values, one valid; a v0 value is ignored.
        let two = format!("{h},v1={},v0=abcd", "00".repeat(32));
        assert!(verify(Some(&two), body, "whsec_test", 1_000, 300).is_ok());
        // Only a v0 signature: refused.
        let v0 = h.replace("v1=", "v0=");
        assert_eq!(
            verify(Some(&v0), body, "whsec_test", 1_000, 300),
            Err(SigError::Malformed)
        );
    }
}
