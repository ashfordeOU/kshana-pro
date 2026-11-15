// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! From a signed webhook to an issued, renewed or revoked licence token.
//!
//! Order of checks for each delivery: the switch (off: refused, nothing read); the
//! provider; the signature (forged: 401); the decode; replay (an event already handled
//! is not acted on again); order (an event older than the last one applied to the same
//! order or subscription is refused with 409); then the action, through the issuance
//! register ([`crate::license::register`]). A refund, chargeback or ended subscription
//! revokes the token and rewrites the signed revocation list in the data directory,
//! which the next app update carries (F10c). A new or renewed token is written to the
//! outbox for delivery to the buyer.
//!
//! A refund or renewal for an order the service has not seen yet is refused with 409,
//! so the provider retries it after the purchase has arrived.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana::api::KshanaError;

use super::config::{PaymentsConfig, Provider};
use super::provider::{self, Action, Event};
use crate::license::lifecycle::{Delivery, LicenceService};
use crate::license::register::Register;
use crate::license::{LicenseClaims, KEY_LEN};

/// The service's memory between deliveries (`webhook-state.json`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct State {
    /// Event keys handled, with the time they were handled.
    #[serde(default)]
    pub seen: BTreeMap<String, i64>,
    /// Latest `occurred` applied per order or subscription.
    #[serde(default)]
    pub last: BTreeMap<String, i64>,
    /// Order or invoice ID to the token it issued.
    #[serde(default)]
    pub orders: BTreeMap<String, String>,
    /// Subscription ID to its current token.
    #[serde(default)]
    pub subscriptions: BTreeMap<String, String>,
    /// Subscription ID to its edition.
    #[serde(default)]
    pub sub_edition: BTreeMap<String, String>,
    /// Token ID to the buyer's e-mail address, for renewal deliveries.
    #[serde(default)]
    pub emails: BTreeMap<String, String>,
    /// Other names of an order (Stripe: payment intent, invoice, session) to the order.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

/// A delivery's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub status: u16,
    pub message: String,
    /// Token issued or renewed by this delivery.
    pub token_id: Option<String>,
    /// Token revoked by this delivery.
    pub revoked: Option<String>,
}

impl Outcome {
    fn new(status: u16, message: impl Into<String>) -> Outcome {
        Outcome {
            status,
            message: message.into(),
            token_id: None,
            revoked: None,
        }
    }
}

/// Files of the webhook service, all under the configured data directory.
pub struct Paths {
    pub dir: PathBuf,
}

impl Paths {
    pub fn register(&self) -> PathBuf {
        self.dir.join("register.jsonl")
    }
    pub fn state(&self) -> PathBuf {
        self.dir.join("webhook-state.json")
    }
    pub fn outbox(&self) -> PathBuf {
        self.dir.join("outbox")
    }
    pub fn revocations(&self) -> PathBuf {
        self.dir.join("revocations.txt")
    }
    pub fn events(&self) -> PathBuf {
        self.dir.join("events.jsonl")
    }
}

/// Everything a delivery needs, loaded once at start.
pub struct Issuer {
    pub config: PaymentsConfig,
    /// The merchant of record's webhook secret (empty while `enabled` is false).
    pub secret: String,
    pub seed: [u8; KEY_LEN],
    pub paths: Paths,
    /// Stripe's webhook endpoint secret, from `STRIPE_WEBHOOK_SECRET`; only read when
    /// `[stripe] enabled` is true.
    pub stripe_secret: Option<String>,
}

impl Issuer {
    /// Load secrets and paths. Refused when payments are off.
    pub fn open(config: PaymentsConfig, base: &Path) -> Result<Issuer, KshanaError> {
        Issuer::open_with_env(config, base, &|k| std::env::var(k).ok())
    }

    /// [`Issuer::open`] reading Stripe's webhook secret through `env`.
    pub fn open_with_env(
        config: PaymentsConfig,
        base: &Path,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Issuer, KshanaError> {
        if !config.enabled && !config.stripe.enabled {
            return Err(KshanaError::Unsupported(
                "payments are off (payments.enabled = false, payments.stripe.enabled = false)"
                    .into(),
            ));
        }
        let secret = if config.enabled {
            config.webhook_secret(base)?
        } else {
            String::new()
        };
        let stripe_secret = if config.stripe.enabled {
            let s = env(super::stripe::ENV_WEBHOOK_SECRET)
                .map(|s| s.trim().to_string())
                .filter(|s| s.starts_with("whsec_"))
                .ok_or_else(|| {
                    KshanaError::InvalidInput(format!(
                        "Stripe is on but {} (whsec_…) is not set",
                        super::stripe::ENV_WEBHOOK_SECRET
                    ))
                })?;
            Some(s)
        } else {
            None
        };
        let seed = config.signing_seed(base)?;
        let dir = config.data_dir(base);
        std::fs::create_dir_all(&dir).map_err(|e| {
            KshanaError::InvalidInput(format!("could not create {}: {e}", dir.display()))
        })?;
        Ok(Issuer {
            config,
            secret,
            seed,
            paths: Paths { dir },
            stripe_secret,
        })
    }

    fn load_state(&self) -> State {
        std::fs::read_to_string(self.paths.state())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, st: &State) -> Result<(), KshanaError> {
        write_atomic(
            &self.paths.state(),
            &serde_json::to_string_pretty(st).unwrap_or_default(),
        )
    }

    /// Handle one delivery: `provider` from the URL path, `header` the signature header
    /// value, `body` the raw bytes.
    pub fn handle(
        &self,
        provider: Provider,
        header: Option<&str>,
        body: &[u8],
        now: i64,
    ) -> Outcome {
        if provider == Provider::Stripe {
            if !self.config.stripe.enabled || self.stripe_secret.is_none() {
                return Outcome::new(404, "Stripe is off");
            }
        } else {
            if !self.config.enabled {
                return Outcome::new(404, "payments are off");
            }
            if provider != self.config.provider {
                return Outcome::new(404, "no webhook for this provider");
            }
        }
        let verified = match provider {
            Provider::Stripe => super::stripe::verify(
                header,
                body,
                self.stripe_secret.as_deref().unwrap_or(""),
                now,
                self.config.stripe.signature_tolerance_secs,
            )
            .map(|_| ()),
            Provider::Paddle => provider::verify_paddle(
                header,
                body,
                &self.secret,
                now,
                self.config.signature_tolerance_secs,
            )
            .map(|_| ()),
            Provider::LemonSqueezy => provider::verify_lemonsqueezy(header, body, &self.secret),
        };
        if let Err(e) = verified {
            self.log(now, "refused", &format!("signature: {e}"), None);
            return Outcome::new(401, format!("refused: {e}"));
        }
        let ev = match provider {
            Provider::Paddle => provider::decode_paddle(body),
            Provider::LemonSqueezy => provider::decode_lemonsqueezy(body),
            Provider::Stripe => super::stripe::decode(body),
        };
        let ev = match ev {
            Ok(e) => e,
            Err(e) => return Outcome::new(400, format!("bad event: {e}")),
        };
        let mut st = self.load_state();
        if st.seen.contains_key(&ev.key) {
            self.log(now, "replay", "already handled; not acted on", Some(&ev));
            return Outcome::new(200, "replay: event already handled, not acted on again");
        }
        if let Some(&last) = st.last.get(&ev.object) {
            if ev.occurred_unix < last {
                self.log(now, "refused", "out of order", Some(&ev));
                return Outcome::new(
                    409,
                    format!(
                        "refused: out of order (event at {} is older than the state applied at {last})",
                        ev.occurred_unix
                    ),
                );
            }
        }
        if provider == Provider::Stripe {
            if let Action::Purchase { order_id, .. } = &ev.action {
                for a in super::stripe::aliases(body) {
                    st.aliases.entry(a).or_insert_with(|| order_id.clone());
                }
            }
        }
        let out = match self.act(&ev, &mut st, now) {
            Ok(o) => o,
            Err(o) => {
                self.log(now, "deferred", &o.message, Some(&ev));
                return o;
            }
        };
        st.seen.insert(ev.key.clone(), now);
        st.last.insert(ev.object.clone(), ev.occurred_unix);
        if let Err(e) = self.save_state(&st) {
            return Outcome::new(500, format!("state not saved: {e}"));
        }
        self.log(now, "applied", &out.message, Some(&ev));
        out
    }

    fn act(&self, ev: &Event, st: &mut State, now: i64) -> Result<Outcome, Outcome> {
        let reg = Register::open(self.paths.register());
        let source = format!("{}:{}", ev.provider.as_str(), ev.key);
        let fail = |e: KshanaError| Outcome::new(500, e.to_string());
        match &ev.action {
            Action::Ignore(why) => Ok(Outcome::new(200, format!("ignored: {why}"))),
            Action::Purchase {
                order_id,
                subscription_id,
                price_id,
                quantity,
                licensee,
                email,
            } => {
                if st.orders.contains_key(order_id) {
                    return Ok(Outcome::new(200, "order already issued"));
                }
                let ed = match price_id.strip_prefix("edition:") {
                    Some(id) => self.config.edition(id),
                    None => self.config.edition_by_price(price_id),
                };
                let Some(ed) = ed else {
                    return Ok(Outcome::new(
                        200,
                        format!("ignored: price {price_id} is not a Kshana edition"),
                    ));
                };
                let name = if !licensee.trim().is_empty() {
                    licensee.trim().to_string()
                } else if !email.trim().is_empty() {
                    email.trim().to_string()
                } else {
                    format!("customer {order_id}")
                };
                let claims = LicenseClaims {
                    licensee: name,
                    tier: ed.tier.clone(),
                    products: ed.products.clone(),
                    seats: *quantity,
                    stations: ed.stations,
                    ..Default::default()
                };
                // C1: the licence is paid for the edition's term; its tokens last 35 days
                // (or the term, when shorter) and the app renews them online.
                let issued = self
                    .licences()
                    .open_licence(
                        &claims,
                        Delivery::Online,
                        now + ed.days * 86_400,
                        &source,
                        &format!("order {order_id}; edition {}", ed.id),
                        now,
                    )
                    .map_err(fail)?;
                let token = issued.token;
                let entry = reg
                    .find(&issued.token_id)
                    .map_err(fail)?
                    .ok_or_else(|| Outcome::new(500, "issued token missing from the register"))?;
                st.orders.insert(order_id.clone(), entry.token_id.clone());
                st.emails.insert(entry.token_id.clone(), email.clone());
                if let Some(sub) = subscription_id {
                    st.subscriptions.insert(sub.clone(), entry.token_id.clone());
                    st.sub_edition.insert(sub.clone(), ed.id.clone());
                } else {
                    st.sub_edition.insert(order_id.clone(), ed.id.clone());
                }
                self.outbox(&entry.token_id, email, &entry.licensee, &token, "new")
                    .map_err(fail)?;
                let mut o = Outcome::new(200, format!("issued {} for {}", entry.token_id, ed.id));
                o.token_id = Some(entry.token_id);
                Ok(o)
            }
            Action::Link {
                subscription_id,
                order_id,
            } => {
                let Some(tid) = st.orders.get(order_id).cloned() else {
                    return Err(Outcome::new(
                        409,
                        format!("order {order_id} not seen yet; retry later"),
                    ));
                };
                if let Some(ed) = st.sub_edition.get(order_id).cloned() {
                    st.sub_edition.insert(subscription_id.clone(), ed);
                }
                st.subscriptions
                    .insert(subscription_id.clone(), tid.clone());
                Ok(Outcome::new(
                    200,
                    format!("linked {subscription_id} to {tid}"),
                ))
            }
            Action::Renewal {
                subscription_id,
                order_id,
            } => {
                let Some(tid) = st.subscriptions.get(subscription_id).cloned() else {
                    return Err(Outcome::new(
                        409,
                        format!("subscription {subscription_id} not seen yet; retry later"),
                    ));
                };
                if st.orders.contains_key(order_id) {
                    return Ok(Outcome::new(200, "renewal already issued"));
                }
                let days = st
                    .sub_edition
                    .get(subscription_id)
                    .and_then(|e| self.config.edition(e))
                    .map(|e| e.days)
                    .unwrap_or(365);
                let svc = self.licences();
                let (token, entry) = match svc.ledger.root_of(&tid).map_err(fail)? {
                    // C1: the payment moves the paid-through date; a fresh token follows.
                    Some(root) => {
                        let paid = svc
                            .ledger
                            .load()
                            .map_err(fail)?
                            .get(&root)
                            .map(|r| r.paid_through_unix)
                            .unwrap_or(now);
                        svc.extend(&root, paid.max(now) + days * 86_400)
                            .map_err(|e| Outcome::new(409, e.to_string()))?;
                        let i = svc
                            .reissue_unbound(&root, &source, now)
                            .map_err(|e| Outcome::new(409, e.to_string()))?;
                        let e = reg
                            .find(&i.token_id)
                            .map_err(fail)?
                            .ok_or_else(|| Outcome::new(500, "renewed token missing"))?;
                        (i.token, e)
                    }
                    // A token issued before the ledger existed.
                    None => reg
                        .renew(&self.seed, &tid, days, &source, now)
                        .map_err(|e| Outcome::new(409, e.to_string()))?,
                };
                st.subscriptions
                    .insert(subscription_id.clone(), entry.token_id.clone());
                st.orders.insert(order_id.clone(), entry.token_id.clone());
                let email = st.emails.get(&tid).cloned().unwrap_or_default();
                st.emails.insert(entry.token_id.clone(), email.clone());
                self.outbox(&entry.token_id, &email, &entry.licensee, &token, "renewal")
                    .map_err(fail)?;
                let mut o = Outcome::new(200, format!("renewed {tid} as {}", entry.token_id));
                o.token_id = Some(entry.token_id);
                Ok(o)
            }
            Action::Cancel { subscription_id } => {
                let Some(tid) = st.subscriptions.get(subscription_id).cloned() else {
                    return Err(Outcome::new(
                        409,
                        format!("subscription {subscription_id} not seen yet; retry later"),
                    ));
                };
                self.revoke(&reg, &tid, "subscription ended", &source, now)
            }
            Action::Refund { order_id } => {
                let order = st.aliases.get(order_id).unwrap_or(order_id);
                let Some(tid) = st.orders.get(order).cloned() else {
                    return Err(Outcome::new(
                        409,
                        format!("order {order_id} not seen yet; retry later"),
                    ));
                };
                self.revoke(&reg, &tid, "refund or chargeback", &source, now)
            }
        }
    }

    fn revoke(
        &self,
        reg: &Register,
        tid: &str,
        why: &str,
        source: &str,
        now: i64,
    ) -> Result<Outcome, Outcome> {
        let fail = |e: KshanaError| Outcome::new(500, e.to_string());
        let svc = self.licences();
        // The whole licence: the token this order issued, its renewals and every
        // device-bound seat token (C1), so no copy keeps working.
        let list = match svc.ledger.root_of(tid).map_err(fail)? {
            Some(root) => svc.revoke(&root, why, source, now).map_err(fail)?,
            None => {
                reg.revoke(tid, why, source, now).map_err(fail)?;
                reg.revocation_list(&self.seed, &[], now).map_err(fail)?
            }
        };
        write_atomic(&self.paths.revocations(), &list).map_err(fail)?;
        let mut o = Outcome::new(200, format!("revoked {tid} ({why})"));
        o.revoked = Some(tid.to_string());
        Ok(o)
    }

    /// The licence service over this issuer's register and ledger.
    pub fn licences(&self) -> LicenceService {
        LicenceService::open(self.seed, &[], &self.paths.dir)
    }

    /// Write a delivery for the buyer to the outbox. Sending it (e-mail) is the
    /// operator's mail step; the token is also kept in the register.
    fn outbox(
        &self,
        tid: &str,
        email: &str,
        licensee: &str,
        token: &str,
        kind: &str,
    ) -> Result<(), KshanaError> {
        let msg = serde_json::json!({
            "to": email,
            "licensee": licensee,
            "kind": kind,
            "token_id": tid,
            "token": token,
            "subject": "Your Kshana Pro licence",
            "body": format!(
                "Your Kshana Pro licence for {licensee} ({tid}).\n\nIn Kshana Desktop: Licence, Activate, then paste the token. On the command line: kshana-pro license activate <token>.\n"
            ),
        });
        write_atomic(
            &self.paths.outbox().join(format!("{tid}.json")),
            &serde_json::to_string_pretty(&msg).unwrap_or_default(),
        )
    }

    fn log(&self, now: i64, what: &str, msg: &str, ev: Option<&Event>) {
        use std::io::Write;
        let line = serde_json::json!({
            "utc_unix": now,
            "result": what,
            "message": msg,
            "event": ev.map(|e| &e.key),
            "object": ev.map(|e| &e.object),
        });
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.paths.events())
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn write_atomic(path: &Path, text: &str) -> Result<(), KshanaError> {
    let err = |e: std::io::Error| KshanaError::InvalidInput(format!("{}: {e}", path.display()));
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(err)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, text).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}
