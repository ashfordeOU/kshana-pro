// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Webhook signatures and event decoding for the two merchants of record, from their
//! published schemes:
//!
//! * **Paddle Billing**: header `Paddle-Signature: ts=<unix>;h1=<hex>` (more than one
//!   `h1` while a secret is being rotated). `h1` is HMAC-SHA256, keyed with the
//!   notification destination's secret, over `<ts>:<raw body>`. The timestamp bounds a
//!   replay: older than the tolerance (default 300 s) is refused. Events carry
//!   `event_id`, `event_type`, `occurred_at` and `data`.
//! * **Lemon Squeezy**: header `X-Signature: <hex>`, HMAC-SHA256 of the raw body keyed
//!   with the webhook's signing secret. The event name is `meta.event_name`; checkout
//!   custom data is `meta.custom_data`. There is no timestamp or event ID in the
//!   signature, so a replay is caught by the body's digest (an exact copy is refused)
//!   and by the object's `updated_at` (an older state is refused).
//!
//! Both are decoded into one [`Event`] the issuing logic acts on.

use sha2::{Digest, Sha256};

use super::config::Provider;

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const B: usize = 64;
    let mut k = [0u8; B];
    if key.len() > B {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(k.map(|b| b ^ 0x36));
    inner.update(msg);
    let ih = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(k.map(|b| b ^ 0x5c));
    outer.update(ih);
    outer.finalize().into()
}

/// Equality that takes the same time wherever the inputs differ.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Why a signature was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum SigError {
    Missing,
    Malformed,
    Stale { age_secs: i64 },
    Mismatch,
}

impl std::fmt::Display for SigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SigError::Missing => write!(f, "signature header missing"),
            SigError::Malformed => write!(f, "signature header malformed"),
            SigError::Stale { age_secs } => {
                write!(f, "signed timestamp outside the tolerance ({age_secs} s)")
            }
            SigError::Mismatch => write!(f, "signature does not match"),
        }
    }
}

/// Check a Paddle `Paddle-Signature` header over `body` at `now`.
pub fn verify_paddle(
    header: Option<&str>,
    body: &[u8],
    secret: &str,
    now: i64,
    tolerance_secs: i64,
) -> Result<i64, SigError> {
    let header = header.ok_or(SigError::Missing)?;
    let mut ts: Option<i64> = None;
    let mut h1: Vec<Vec<u8>> = Vec::new();
    for part in header.split(';') {
        let (k, v) = part.trim().split_once('=').ok_or(SigError::Malformed)?;
        match k.trim() {
            "ts" => ts = Some(v.trim().parse().map_err(|_| SigError::Malformed)?),
            "h1" => h1.push(hex::decode(v.trim()).map_err(|_| SigError::Malformed)?),
            _ => {}
        }
    }
    let ts = ts.ok_or(SigError::Malformed)?;
    if h1.is_empty() {
        return Err(SigError::Malformed);
    }
    let age = now - ts;
    if age.abs() > tolerance_secs {
        return Err(SigError::Stale { age_secs: age });
    }
    let mut msg = format!("{ts}:").into_bytes();
    msg.extend_from_slice(body);
    let want = hmac_sha256(secret.as_bytes(), &msg);
    if h1.iter().any(|h| ct_eq(h, &want)) {
        Ok(ts)
    } else {
        Err(SigError::Mismatch)
    }
}

/// Check a Lemon Squeezy `X-Signature` header over `body`.
pub fn verify_lemonsqueezy(
    header: Option<&str>,
    body: &[u8],
    secret: &str,
) -> Result<(), SigError> {
    let h = header.ok_or(SigError::Missing)?;
    let got = hex::decode(h.trim()).map_err(|_| SigError::Malformed)?;
    if ct_eq(&got, &hmac_sha256(secret.as_bytes(), body)) {
        Ok(())
    } else {
        Err(SigError::Mismatch)
    }
}

/// Sign a body as Paddle does (for tests and the recorded fixtures).
pub fn sign_paddle(body: &[u8], secret: &str, ts: i64) -> String {
    let mut msg = format!("{ts}:").into_bytes();
    msg.extend_from_slice(body);
    format!(
        "ts={ts};h1={}",
        hex::encode(hmac_sha256(secret.as_bytes(), &msg))
    )
}

/// Sign a body as Lemon Squeezy does (for tests and the recorded fixtures).
pub fn sign_lemonsqueezy(body: &[u8], secret: &str) -> String {
    hex::encode(hmac_sha256(secret.as_bytes(), body))
}

/// What an event asks the issuer to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// A first purchase: issue a token.
    Purchase {
        order_id: String,
        subscription_id: Option<String>,
        price_id: String,
        quantity: u32,
        licensee: String,
        email: String,
    },
    /// A subscription renewal payment: renew the subscription's token.
    Renewal {
        subscription_id: String,
        order_id: String,
    },
    /// Link a subscription to the order that started it (Lemon Squeezy).
    Link {
        subscription_id: String,
        order_id: String,
    },
    /// The subscription has ended: revoke its token.
    Cancel { subscription_id: String },
    /// A full refund or chargeback of an order: revoke the token it issued.
    Refund { order_id: String },
    /// Nothing to do (the reason is logged).
    Ignore(String),
}

/// A verified, decoded webhook event.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub provider: Provider,
    /// Unique key for replay detection: Paddle's `event_id`; for Lemon Squeezy the
    /// SHA-256 of the body.
    pub key: String,
    /// When the provider says the change happened, Unix seconds.
    pub occurred_unix: i64,
    /// The object whose state the event carries (order or subscription), for ordering.
    pub object: String,
    pub action: Action,
}

/// Parse an RFC 3339 time (`2026-11-07T10:00:00Z`, `…​.123456Z`, `…+02:00`) to Unix
/// seconds.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 20 {
        return None;
    }
    let num = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if s.as_bytes()[4] != b'-' || s.as_bytes()[10] != b'T' && s.as_bytes()[10] != b't' {
        return None;
    }
    let mut rest = &s[19..];
    if let Some(r) = rest.strip_prefix('.') {
        let n = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        rest = &r[n..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        o if o.len() == 6 && (o.starts_with('+') || o.starts_with('-')) => {
            let sign = if o.starts_with('-') { -1 } else { 1 };
            let oh: i64 = o.get(1..3)?.parse().ok()?;
            let om: i64 = o.get(4..6)?.parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
        _ => return None,
    };
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    // Days from civil (Howard Hinnant).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se - offset)
}

fn s(v: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    match cur {
        serde_json::Value::String(x) => Some(x.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Decode a verified Paddle Billing event.
pub fn decode_paddle(body: &[u8]) -> Result<Event, String> {
    let v: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format!("event is not JSON: {e}"))?;
    let key = s(&v, &["event_id"]).ok_or("event_id missing")?;
    let kind = s(&v, &["event_type"]).ok_or("event_type missing")?;
    let occurred = s(&v, &["occurred_at"])
        .and_then(|t| parse_rfc3339(&t))
        .ok_or("occurred_at missing or not RFC 3339")?;
    let d = &v["data"];
    let id = s(d, &["id"]).unwrap_or_default();
    let (object, action) = match kind.as_str() {
        "transaction.completed" => {
            let sub = s(d, &["subscription_id"]);
            let origin = s(d, &["origin"]).unwrap_or_default();
            let item = &d["items"][0];
            let price = s(item, &["price", "id"]).or_else(|| s(item, &["price_id"]));
            let quantity = item["quantity"].as_u64().unwrap_or(1).clamp(1, 10_000) as u32;
            let object = sub.clone().unwrap_or_else(|| id.clone());
            if origin == "subscription_recurring" {
                match sub {
                    Some(sub) => (
                        object,
                        Action::Renewal {
                            subscription_id: sub,
                            order_id: id,
                        },
                    ),
                    None => (
                        object,
                        Action::Ignore("recurring transaction without a subscription".into()),
                    ),
                }
            } else {
                let price = price.ok_or("transaction has no price")?;
                (
                    object,
                    Action::Purchase {
                        order_id: id,
                        subscription_id: sub,
                        price_id: price,
                        quantity,
                        licensee: s(d, &["custom_data", "licensee"]).unwrap_or_default(),
                        email: s(d, &["custom_data", "email"]).unwrap_or_default(),
                    },
                )
            }
        }
        "subscription.canceled" => (
            id.clone(),
            Action::Cancel {
                subscription_id: id,
            },
        ),
        "adjustment.created" | "adjustment.updated" => {
            let txn = s(d, &["transaction_id"]).unwrap_or_default();
            let action = s(d, &["action"]).unwrap_or_default();
            let status = s(d, &["status"]).unwrap_or_default();
            let full = s(d, &["type"]).map(|t| t == "full").unwrap_or(true);
            let object = s(d, &["subscription_id"]).unwrap_or_else(|| txn.clone());
            if (action == "refund" && full || action == "chargeback") && status == "approved" {
                (object, Action::Refund { order_id: txn })
            } else {
                (
                    object,
                    Action::Ignore(format!(
                        "adjustment {action} {status} (not a full approved refund)"
                    )),
                )
            }
        }
        other => (
            id,
            Action::Ignore(format!("event type {other} is not used")),
        ),
    };
    Ok(Event {
        provider: Provider::Paddle,
        key: format!("paddle:{key}"),
        occurred_unix: occurred,
        object: format!("paddle:{object}"),
        action,
    })
}

/// Decode a verified Lemon Squeezy event.
pub fn decode_lemonsqueezy(body: &[u8]) -> Result<Event, String> {
    let v: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format!("event is not JSON: {e}"))?;
    let name = s(&v, &["meta", "event_name"]).ok_or("meta.event_name missing")?;
    let d = &v["data"];
    let a = &d["attributes"];
    let id = s(d, &["id"]).ok_or("data.id missing")?;
    let occurred = s(a, &["updated_at"])
        .or_else(|| s(a, &["created_at"]))
        .and_then(|t| parse_rfc3339(&t))
        .ok_or("attributes.updated_at missing or not RFC 3339")?;
    let key = format!("lemonsqueezy:{}", hex::encode(Sha256::digest(body)));
    let order = |oid: &str| format!("ls-order:{oid}");
    let (object, action) = match name.as_str() {
        "order_created" => {
            if s(a, &["status"]).as_deref() != Some("paid") {
                (order(&id), Action::Ignore("order not paid".into()))
            } else {
                let item = &a["first_order_item"];
                (
                    order(&id),
                    Action::Purchase {
                        order_id: order(&id),
                        subscription_id: None,
                        price_id: s(item, &["variant_id"]).ok_or("order has no variant")?,
                        quantity: item["quantity"].as_u64().unwrap_or(1).clamp(1, 10_000) as u32,
                        licensee: s(&v, &["meta", "custom_data", "licensee"])
                            .or_else(|| s(a, &["user_name"]))
                            .unwrap_or_default(),
                        email: s(a, &["user_email"]).unwrap_or_default(),
                    },
                )
            }
        }
        "order_refunded" => {
            if s(a, &["status"]).as_deref() == Some("refunded") {
                (
                    order(&id),
                    Action::Refund {
                        order_id: order(&id),
                    },
                )
            } else {
                (order(&id), Action::Ignore("partial refund".into()))
            }
        }
        "subscription_created" => {
            let oid = s(a, &["order_id"]).ok_or("subscription has no order_id")?;
            (
                format!("ls-sub:{id}"),
                Action::Link {
                    subscription_id: format!("ls-sub:{id}"),
                    order_id: order(&oid),
                },
            )
        }
        "subscription_payment_success" => {
            let sub = s(a, &["subscription_id"]).ok_or("invoice has no subscription_id")?;
            if s(a, &["billing_reason"]).as_deref() == Some("renewal") {
                (
                    format!("ls-sub:{sub}"),
                    Action::Renewal {
                        subscription_id: format!("ls-sub:{sub}"),
                        order_id: format!("ls-invoice:{id}"),
                    },
                )
            } else {
                (
                    format!("ls-sub:{sub}"),
                    Action::Ignore("first payment (the order issues the token)".into()),
                )
            }
        }
        "subscription_payment_refunded" => {
            let sub = s(a, &["subscription_id"]).unwrap_or_default();
            (
                format!("ls-sub:{sub}"),
                Action::Refund {
                    order_id: format!("ls-invoice:{id}"),
                },
            )
        }
        "subscription_expired" => (
            format!("ls-sub:{id}"),
            Action::Cancel {
                subscription_id: format!("ls-sub:{id}"),
            },
        ),
        "subscription_cancelled" => (
            format!("ls-sub:{id}"),
            Action::Ignore("cancelled; the licence ends when the subscription expires".into()),
        ),
        other => (id, Action::Ignore(format!("event {other} is not used"))),
    };
    Ok(Event {
        provider: Provider::LemonSqueezy,
        key,
        occurred_unix: occurred,
        object: format!("lemonsqueezy:{object}"),
        action,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc4231() {
        // RFC 4231 test case 2.
        assert_eq!(
            hex::encode(hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Test case 6: a key longer than the block.
        assert_eq!(
            hex::encode(hmac_sha256(
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn rfc3339_parses() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339("2026-11-07T10:00:00.123456Z"),
            Some(1_794_045_600)
        );
        assert_eq!(
            parse_rfc3339("2026-11-07T12:00:00+02:00"),
            Some(1_794_045_600)
        );
        assert_eq!(parse_rfc3339("2026-11-07 10:00"), None);
        assert_eq!(parse_rfc3339("2026-13-07T10:00:00Z"), None);
    }

    #[test]
    fn paddle_signature_scheme() {
        let body = br#"{"event_id":"evt_1"}"#;
        let h = sign_paddle(body, "pdl_secret", 1_000);
        assert_eq!(
            verify_paddle(Some(&h), body, "pdl_secret", 1_010, 300),
            Ok(1_000)
        );
        assert_eq!(
            verify_paddle(Some(&h), body, "other", 1_010, 300),
            Err(SigError::Mismatch)
        );
        assert_eq!(
            verify_paddle(Some(&h), b"{}", "pdl_secret", 1_010, 300),
            Err(SigError::Mismatch)
        );
        assert!(matches!(
            verify_paddle(Some(&h), body, "pdl_secret", 1_400, 300),
            Err(SigError::Stale { .. })
        ));
        assert_eq!(
            verify_paddle(None, body, "pdl_secret", 1_000, 300),
            Err(SigError::Missing)
        );
        assert_eq!(
            verify_paddle(Some("ts=abc;h1=00"), body, "pdl_secret", 1_000, 300),
            Err(SigError::Malformed)
        );
        // Rotation: two h1 values, one valid.
        let two = format!("{h};h1={}", "00".repeat(32));
        assert!(verify_paddle(Some(&two), body, "pdl_secret", 1_000, 300).is_ok());
    }

    #[test]
    fn lemonsqueezy_signature_scheme() {
        let body = br#"{"meta":{}}"#;
        let h = sign_lemonsqueezy(body, "ls_secret");
        assert!(verify_lemonsqueezy(Some(&h), body, "ls_secret").is_ok());
        assert_eq!(
            verify_lemonsqueezy(Some(&h), body, "nope"),
            Err(SigError::Mismatch)
        );
        assert_eq!(
            verify_lemonsqueezy(Some("zz"), body, "ls_secret"),
            Err(SigError::Malformed)
        );
    }
}
