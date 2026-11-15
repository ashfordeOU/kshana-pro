// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Licence lifecycle on the vendor side (roadmap 0.42 C1 and C2, owner 15 November 2026).
//!
//! * **C1 subscription tokens** are valid [`TOKEN_DAYS`] (35) days and are renewed online
//!   while the subscription is paid ([`LicenceService::renew`]). Each renewal is bound to
//!   the device asking for it, and a licence holds at most its number of seats
//!   ([`LicenceService::activate`]). The app keeps working offline through the 30-day grace
//!   period after a token's expiry ([`super::activation::GRACE_DAYS`]).
//! * **C2 offline tokens** (air-gapped Pro and Sovereign, invoiced) cover the whole prepaid
//!   term in one token; the same 30-day grace applies, and renewal is a new signed file.
//!   They are never renewed online.
//! * **Revocation**: revoking a licence revokes every token issued for it (the first, each
//!   renewal and each device token) and returns the new signed revocation list
//!   ([`super::register::Register::revocation_list`]).
//!
//! The ledger (`licences.json`, next to the issuance register) records each licence: its
//! claims, how far it is paid, its seats and the devices holding them, and every token ID
//! issued for it. Like the register it lives with the vendor and is never shipped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana::api::KshanaError;

use super::activation::GRACE_DAYS;
use super::register::Register;
use super::{public_key_for, verify_with, Keyring, LicenseClaims, VerifyOptions, KEY_LEN};

/// Days a C1 subscription token is valid.
pub const TOKEN_DAYS: i64 = 35;
const DAY: i64 = 86_400;

/// How a licence is delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Delivery {
    /// C1: 35-day tokens renewed online while paid.
    #[default]
    Online,
    /// C2: one token for the prepaid term; renewal by signed file.
    Offline,
}

/// A device holding one seat.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Seat {
    /// The device's machine hash ([`super::machine::hash_machine_id`]).
    pub device: String,
    /// The latest token issued to it.
    pub token_id: String,
    pub since_unix: i64,
}

/// One licence in the ledger.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LicenceRecord {
    /// The first token ID issued for the licence: its stable name.
    pub root: String,
    /// The claims every token of this licence carries (without ID, expiry or machine).
    pub claims: LicenseClaims,
    pub delivery: Delivery,
    /// Unix seconds up to which the licence is paid (the end of the prepaid term).
    pub paid_through_unix: i64,
    #[serde(default)]
    pub seats: Vec<Seat>,
    /// Every token ID issued for the licence, oldest first.
    #[serde(default)]
    pub tokens: Vec<String>,
    #[serde(default)]
    pub revoked: bool,
    /// Where the licence came from: `invoice:<ref>`, `stripe:<event>`, `paddle:<event>`.
    #[serde(default)]
    pub source: String,
}

impl LicenceRecord {
    /// Seats the licence may hold (at least one).
    pub fn seat_limit(&self) -> usize {
        self.claims.seats.max(1) as usize
    }
}

/// The ledger file.
#[derive(Debug, Clone)]
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    pub fn open(path: impl Into<PathBuf>) -> Ledger {
        Ledger { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every licence, by root token ID.
    pub fn load(&self) -> Result<BTreeMap<String, LicenceRecord>, KshanaError> {
        match std::fs::read_to_string(&self.path) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| {
                KshanaError::InvalidInput(format!("ledger {}: {e}", self.path.display()))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(KshanaError::InvalidInput(format!(
                "ledger {}: {e}",
                self.path.display()
            ))),
        }
    }

    pub fn save(&self, all: &BTreeMap<String, LicenceRecord>) -> Result<(), KshanaError> {
        let err = |e: std::io::Error| {
            KshanaError::InvalidInput(format!("ledger {}: {e}", self.path.display()))
        };
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d).map_err(err)?;
        }
        let tmp = self
            .path
            .with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(&tmp, serde_json::to_string_pretty(all).unwrap_or_default()).map_err(err)?;
        std::fs::rename(&tmp, &self.path).map_err(err)
    }

    /// The root of the licence that issued `token_id`.
    pub fn root_of(&self, token_id: &str) -> Result<Option<String>, KshanaError> {
        Ok(self
            .load()?
            .into_iter()
            .find(|(_, r)| r.tokens.iter().any(|t| t == token_id))
            .map(|(k, _)| k))
    }
}

/// The vendor's licence service: issue, activate a seat, renew, extend, revoke.
pub struct LicenceService {
    seed: [u8; KEY_LEN],
    ring: Keyring,
    pub register: Register,
    pub ledger: Ledger,
}

/// A token issued by the service, with the licence it belongs to.
#[derive(Debug, Clone)]
pub struct Issued {
    pub token: String,
    pub token_id: String,
    pub root: String,
    pub expires_unix: i64,
}

fn bad(m: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(m.into())
}

impl LicenceService {
    /// A service signing with `seed`, writing `dir/register.jsonl` and `dir/licences.json`.
    /// Tokens are verified against `seed`'s own public key plus `extra_keys` (earlier
    /// signing keys still in use).
    pub fn open(seed: [u8; KEY_LEN], extra_keys: &[[u8; KEY_LEN]], dir: &Path) -> LicenceService {
        let mut ring = Keyring::single(public_key_for(&seed));
        for k in extra_keys {
            ring.add(*k);
        }
        LicenceService {
            seed,
            ring,
            register: Register::open(dir.join("register.jsonl")),
            ledger: Ledger::open(dir.join("licences.json")),
        }
    }

    /// The keyring tokens are checked against.
    pub fn keyring(&self) -> &Keyring {
        &self.ring
    }

    /// The expiry of a token issued at `now` for `rec`.
    fn expiry_for(rec_delivery: Delivery, paid_through: i64, now: i64) -> i64 {
        match rec_delivery {
            Delivery::Online => (now + TOKEN_DAYS * DAY).min(paid_through),
            Delivery::Offline => paid_through,
        }
    }

    /// Open a new licence paid through `paid_through_unix` and issue its first token (not
    /// bound to a device; the app binds a seat on activation). `claims` gives the
    /// licensee, tier, products, features, seats and Stations.
    pub fn open_licence(
        &self,
        claims: &LicenseClaims,
        delivery: Delivery,
        paid_through_unix: i64,
        source: &str,
        note: &str,
        now: i64,
    ) -> Result<Issued, KshanaError> {
        if paid_through_unix <= now {
            return Err(bad("a licence must be paid through a time after now"));
        }
        if claims.licensee.trim().is_empty() {
            return Err(bad("a licence needs a licensee"));
        }
        let mut template = claims.clone();
        template.id = String::new();
        template.machine = None;
        template.arksa = false;
        template.expires_unix = Self::expiry_for(delivery, paid_through_unix, now);
        let (token, entry) = self
            .register
            .issue(&self.seed, &template, source, note, now)?;
        template.expires_unix = 0;
        let mut all = self.ledger.load()?;
        all.insert(
            entry.token_id.clone(),
            LicenceRecord {
                root: entry.token_id.clone(),
                claims: template,
                delivery,
                paid_through_unix,
                seats: Vec::new(),
                tokens: vec![entry.token_id.clone()],
                revoked: false,
                source: source.to_string(),
            },
        );
        self.ledger.save(&all)?;
        Ok(Issued {
            token,
            token_id: entry.token_id.clone(),
            root: entry.token_id,
            expires_unix: entry.expires_unix,
        })
    }

    /// Check a presented token: signature (any active key), not revoked, known to the
    /// ledger, licence not revoked, not beyond the grace period, and, when the token is
    /// bound to a device, bound to `device`.
    fn presented(
        &self,
        token: &str,
        device: &str,
        now: i64,
    ) -> Result<(String, LicenceRecord, String), KshanaError> {
        if device.len() != 64 || hex::decode(device).is_err() {
            return Err(bad(
                "device must be a 64-hex machine hash (`license machine-id`)",
            ));
        }
        let dev = device.to_string();
        let here = move || Some(dev.clone());
        let lic = verify_with(
            &self.ring,
            token,
            &VerifyOptions {
                now_unix: now,
                product: None,
                machine: Some(&here),
                allow_expired: true,
            },
        )?;
        let exp = lic.claims.expires_unix;
        if exp != 0 && now >= exp + GRACE_DAYS * DAY {
            return Err(bad(
                "the token expired more than 30 days ago; the licence holder must contact the vendor",
            ));
        }
        if self.register.is_revoked(&lic.token_id)? {
            return Err(bad("this licence has been revoked"));
        }
        let root = self
            .ledger
            .root_of(&lic.token_id)?
            .ok_or_else(|| bad("this token is not in the licence ledger"))?;
        let rec = self
            .ledger
            .load()?
            .remove(&root)
            .ok_or_else(|| bad("licence record missing"))?;
        if rec.revoked {
            return Err(bad("this licence has been revoked"));
        }
        Ok((root, rec, lic.token_id))
    }

    fn issue_for_device(
        &self,
        root: &str,
        rec: &mut LicenceRecord,
        renews: &str,
        device: &str,
        source: &str,
        now: i64,
    ) -> Result<Issued, KshanaError> {
        let mut c = rec.claims.clone();
        c.machine = Some(device.to_string());
        c.expires_unix = Self::expiry_for(rec.delivery, rec.paid_through_unix, now);
        let (token, entry) = self.register.reissue(
            &self.seed,
            &c,
            renews,
            source,
            &format!("licence {root}"),
            now,
        )?;
        rec.tokens.push(entry.token_id.clone());
        Ok(Issued {
            token,
            token_id: entry.token_id,
            root: root.to_string(),
            expires_unix: entry.expires_unix,
        })
    }

    /// Bind a seat to `device` and issue its device-bound token. A device that already
    /// holds a seat gets a fresh token for it (idempotent). Refused when every seat is
    /// taken, the licence is revoked, unpaid or the token is not valid.
    pub fn activate(&self, token: &str, device: &str, now: i64) -> Result<Issued, KshanaError> {
        let (root, mut rec, tid) = self.presented(token, device, now)?;
        if rec.paid_through_unix <= now {
            return Err(bad(
                "the licence is not paid through today; renew the subscription",
            ));
        }
        let seated = rec.seats.iter().position(|s| s.device == device);
        if seated.is_none() && rec.seats.len() >= rec.seat_limit() {
            return Err(bad(format!(
                "all {} seat(s) of this licence are in use; release one first",
                rec.seat_limit()
            )));
        }
        let out = self.issue_for_device(&root, &mut rec, &tid, device, "activate", now)?;
        match seated {
            Some(i) => rec.seats[i].token_id = out.token_id.clone(),
            None => rec.seats.push(Seat {
                device: device.to_string(),
                token_id: out.token_id.clone(),
                since_unix: now,
            }),
        }
        self.store(&root, rec)?;
        Ok(out)
    }

    /// C1 online renewal: a new 35-day token for the same device, up to the paid-through
    /// date. Refused for an offline (C2) licence, a device without a seat, a revoked or
    /// unpaid licence, or a token bound to another device.
    pub fn renew(&self, token: &str, device: &str, now: i64) -> Result<Issued, KshanaError> {
        let (root, mut rec, tid) = self.presented(token, device, now)?;
        if rec.delivery == Delivery::Offline {
            return Err(bad(
                "an offline licence is renewed by a new signed file, not online",
            ));
        }
        if rec.paid_through_unix <= now {
            return Err(bad(
                "the subscription is not paid through today; nothing to renew",
            ));
        }
        let Some(i) = rec.seats.iter().position(|s| s.device == device) else {
            return Err(bad(
                "this device holds no seat of the licence; activate it first",
            ));
        };
        let out = self.issue_for_device(&root, &mut rec, &tid, device, "renew", now)?;
        rec.seats[i].token_id = out.token_id.clone();
        self.store(&root, rec)?;
        Ok(out)
    }

    /// Free the seat `device` holds (the device's tokens run out on their own; revoke the
    /// licence to stop them at once).
    pub fn release(&self, root: &str, device: &str) -> Result<bool, KshanaError> {
        let mut all = self.ledger.load()?;
        let rec = all
            .get_mut(root)
            .ok_or_else(|| bad(format!("no licence {root}")))?;
        let n = rec.seats.len();
        rec.seats.retain(|s| s.device != device);
        let freed = rec.seats.len() != n;
        self.ledger.save(&all)?;
        Ok(freed)
    }

    /// A payment arrived: move the licence's paid-through date to `paid_through_unix`
    /// (never backwards).
    pub fn extend(&self, root: &str, paid_through_unix: i64) -> Result<i64, KshanaError> {
        let mut all = self.ledger.load()?;
        let rec = all
            .get_mut(root)
            .ok_or_else(|| bad(format!("no licence {root}")))?;
        if rec.revoked {
            return Err(bad("this licence has been revoked"));
        }
        rec.paid_through_unix = rec.paid_through_unix.max(paid_through_unix);
        let p = rec.paid_through_unix;
        self.ledger.save(&all)?;
        Ok(p)
    }

    /// Issue a fresh unbound token for the licence (after a payment, for the buyer's
    /// outbox), following its newest token.
    pub fn reissue_unbound(
        &self,
        root: &str,
        source: &str,
        now: i64,
    ) -> Result<Issued, KshanaError> {
        let mut all = self.ledger.load()?;
        let rec = all
            .get_mut(root)
            .ok_or_else(|| bad(format!("no licence {root}")))?;
        if rec.revoked {
            return Err(bad("this licence has been revoked"));
        }
        let last = rec
            .tokens
            .last()
            .cloned()
            .unwrap_or_else(|| root.to_string());
        let mut c = rec.claims.clone();
        c.expires_unix = Self::expiry_for(rec.delivery, rec.paid_through_unix, now);
        let (token, entry) = self.register.reissue(
            &self.seed,
            &c,
            &last,
            source,
            &format!("licence {root}"),
            now,
        )?;
        rec.tokens.push(entry.token_id.clone());
        self.ledger.save(&all)?;
        Ok(Issued {
            token,
            token_id: entry.token_id,
            root: root.to_string(),
            expires_unix: entry.expires_unix,
        })
    }

    /// Revoke the licence: every token issued for it goes on the register as revoked.
    /// Returns the new signed revocation list.
    pub fn revoke(
        &self,
        root: &str,
        reason: &str,
        source: &str,
        now: i64,
    ) -> Result<String, KshanaError> {
        let mut all = self.ledger.load()?;
        let rec = all
            .get_mut(root)
            .ok_or_else(|| bad(format!("no licence {root}")))?;
        for t in &rec.tokens {
            self.register.revoke(t, reason, source, now)?;
        }
        rec.revoked = true;
        self.ledger.save(&all)?;
        self.register.revocation_list(&self.seed, &[], now)
    }

    /// The current signed revocation list.
    pub fn revocation_list(&self, now: i64) -> Result<String, KshanaError> {
        self.register.revocation_list(&self.seed, &[], now)
    }

    pub(crate) fn seed(&self) -> &[u8; KEY_LEN] {
        &self.seed
    }

    /// A fresh token for a seat (the automatic check, [`super::online`]).
    pub(crate) fn reissue_seat(
        &self,
        root: &str,
        rec: &mut LicenceRecord,
        renews: &str,
        device: &str,
        now: i64,
    ) -> Result<Issued, KshanaError> {
        self.issue_for_device(root, rec, renews, device, "check", now)
    }

    fn store(&self, root: &str, rec: LicenceRecord) -> Result<(), KshanaError> {
        let mut all = self.ledger.load()?;
        all.insert(root.to_string(), rec);
        self.ledger.save(&all)
    }
}
