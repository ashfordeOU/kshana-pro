// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Vendor issuance register (roadmap F10d).
//!
//! Every token the vendor issues, renews or revokes is one line of an append-only JSON
//! Lines file, each line carrying the SHA-256 of the line before it, so an edit or a
//! deleted line breaks the chain ([`Register::verify`]). The register is the vendor's
//! record of who holds which licence (licensee, tier, products, seats, Stations,
//! expiry); the signed revocation list is built from it ([`Register::revocation_list`]),
//! and so are renewal reminders ([`Register::reminders`]).
//!
//! The register holds each issued token so it can be sent again; it lives with the
//! vendor (next to the offline signing key, `docs/LICENSING-KEYS.md`) and is never
//! shipped. Payments (roadmap F10g) write to it through the same functions.

use std::io::Write;
use std::path::{Path, PathBuf};

use kshana::api::KshanaError;
use sha2::{Digest, Sha256};

use super::revocation::{self, RevocationList};
use super::{issue_v2, key_id, public_key_for, LicenseClaims, Tier, KEY_LEN};

/// Days in a trial token.
pub const TRIAL_DAYS: i64 = 30;
const DAY: i64 = 86_400;

/// What a register line records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Issue,
    Trial,
    Renew,
    Revoke,
}

/// One register line.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub utc_unix: i64,
    pub action: Option<Action>,
    pub token_id: String,
    #[serde(default)]
    pub licensee: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub products: Vec<String>,
    #[serde(default)]
    pub seats: u32,
    #[serde(default)]
    pub stations: u32,
    #[serde(default)]
    pub expires_unix: i64,
    #[serde(default)]
    pub kid: String,
    /// For a renewal: the token it renews.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renews: Option<String>,
    /// Where the action came from: `manual`, `trial`, or `<provider>:<event id>`.
    #[serde(default)]
    pub source: String,
    /// Free text: a revocation reason, an order reference.
    #[serde(default)]
    pub note: String,
    /// The issued token (issue, trial, renew).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    pub prev: String,
    pub hash: String,
}

impl Entry {
    fn digest(&self) -> String {
        let mut e = self.clone();
        e.hash = String::new();
        let body = serde_json::to_string(&e).unwrap_or_default();
        hex::encode(Sha256::digest(format!("{}\n{body}", self.prev).as_bytes()))
    }

    /// Whether this line issued a token (issue, trial or renew).
    pub fn issues_token(&self) -> bool {
        matches!(
            self.action,
            Some(Action::Issue) | Some(Action::Trial) | Some(Action::Renew)
        )
    }
}

/// How a token came to be issued (one register line's action and origin).
struct How<'a> {
    action: Action,
    renews: Option<String>,
    source: &'a str,
    note: &'a str,
}

/// The register file.
#[derive(Debug, Clone)]
pub struct Register {
    path: PathBuf,
}

fn io_err(path: &Path) -> impl Fn(std::io::Error) -> KshanaError + '_ {
    move |e| KshanaError::InvalidInput(format!("register {}: {e}", path.display()))
}

impl Register {
    pub fn open(path: impl Into<PathBuf>) -> Register {
        Register { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every line, in order (an absent file is an empty register).
    pub fn entries(&self) -> Result<Vec<Entry>, KshanaError> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_err(&self.path)(e)),
        };
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| {
                serde_json::from_str(l)
                    .map_err(|e| KshanaError::InvalidInput(format!("register line {}: {e}", i + 1)))
            })
            .collect()
    }

    /// Check the hash chain; returns the number of lines.
    pub fn verify(&self) -> Result<usize, KshanaError> {
        let entries = self.entries()?;
        let mut prev = String::new();
        for (i, e) in entries.iter().enumerate() {
            if e.seq != i as u64 + 1 || e.prev != prev || e.hash != e.digest() {
                return Err(KshanaError::InvalidInput(format!(
                    "register chain broken at line {} (edited, reordered or a line removed)",
                    i + 1
                )));
            }
            prev = e.hash.clone();
        }
        Ok(entries.len())
    }

    /// Append a line (sets `seq`, `prev` and `hash`). The chain is checked first, so
    /// nothing is added to a damaged register.
    pub fn append(&self, mut entry: Entry) -> Result<Entry, KshanaError> {
        self.verify()?;
        let entries = self.entries()?;
        entry.seq = entries.len() as u64 + 1;
        entry.prev = entries.last().map(|e| e.hash.clone()).unwrap_or_default();
        entry.hash = entry.digest();
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(io_err(&self.path))?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(io_err(&self.path))?;
        let line = serde_json::to_string(&entry)
            .map_err(|e| KshanaError::InvalidInput(format!("register: {e}")))?;
        writeln!(f, "{line}").map_err(io_err(&self.path))?;
        f.sync_all().map_err(io_err(&self.path))?;
        Ok(entry)
    }

    /// The line that issued `token_id`.
    pub fn find(&self, token_id: &str) -> Result<Option<Entry>, KshanaError> {
        Ok(self
            .entries()?
            .into_iter()
            .find(|e| e.issues_token() && e.token_id == token_id))
    }

    /// Whether `token_id` has been revoked.
    pub fn is_revoked(&self, token_id: &str) -> Result<bool, KshanaError> {
        Ok(self
            .entries()?
            .iter()
            .any(|e| e.action == Some(Action::Revoke) && e.token_id == token_id))
    }

    fn issue_action(
        &self,
        seed: &[u8; KEY_LEN],
        claims: &LicenseClaims,
        how: How<'_>,
        now: i64,
    ) -> Result<(String, Entry), KshanaError> {
        let How {
            action,
            renews,
            source,
            note,
        } = how;
        let mut c = claims.clone();
        c.id = String::new();
        c.issued_unix = now;
        let token = issue_v2(seed, &c)?;
        let (payload, _) = super::split_signed(&token)?;
        let signed: LicenseClaims = serde_json::from_slice(&payload)
            .map_err(|e| KshanaError::InvalidInput(format!("{e}")))?;
        let entry = self.append(Entry {
            utc_unix: now,
            action: Some(action),
            token_id: signed.id.clone(),
            licensee: signed.licensee.clone(),
            tier: signed.tier.clone(),
            products: signed.products.clone(),
            seats: signed.seats,
            stations: signed.stations,
            expires_unix: signed.expires_unix,
            kid: signed.kid.clone().unwrap_or_default(),
            renews,
            source: source.to_string(),
            note: note.to_string(),
            token: token.clone(),
            ..Default::default()
        })?;
        Ok((token, entry))
    }

    /// Issue a token and record it.
    pub fn issue(
        &self,
        seed: &[u8; KEY_LEN],
        claims: &LicenseClaims,
        source: &str,
        note: &str,
        now: i64,
    ) -> Result<(String, Entry), KshanaError> {
        self.issue_action(
            seed,
            claims,
            How {
                action: Action::Issue,
                renews: None,
                source,
                note,
            },
            now,
        )
    }

    /// Issue a 30-day Pro trial token for `products` and record it. One trial per
    /// licensee: a second is refused.
    pub fn trial(
        &self,
        seed: &[u8; KEY_LEN],
        licensee: &str,
        products: &[String],
        now: i64,
    ) -> Result<(String, Entry), KshanaError> {
        let name = licensee.trim();
        if name.is_empty() {
            return Err(KshanaError::InvalidInput("a trial needs a licensee".into()));
        }
        if self
            .entries()?
            .iter()
            .any(|e| e.action == Some(Action::Trial) && e.licensee.eq_ignore_ascii_case(name))
        {
            return Err(KshanaError::InvalidInput(format!(
                "'{name}' already had a trial"
            )));
        }
        let claims = LicenseClaims {
            licensee: name.to_string(),
            tier: Tier::Pro.as_str().into(),
            expires_unix: now + TRIAL_DAYS * DAY,
            products: products.to_vec(),
            seats: 1,
            ..Default::default()
        };
        self.issue_action(
            seed,
            &claims,
            How {
                action: Action::Trial,
                renews: None,
                source: "trial",
                note: "",
            },
            now,
        )
    }

    /// Renew `token_id` by `days`: a new token with the same claims, valid from the
    /// later of now and the old expiry. A revoked token is not renewed.
    pub fn renew(
        &self,
        seed: &[u8; KEY_LEN],
        token_id: &str,
        days: i64,
        source: &str,
        now: i64,
    ) -> Result<(String, Entry), KshanaError> {
        if days <= 0 || days > 3660 {
            return Err(KshanaError::InvalidInput(
                "renewal days must be 1 to 3660".into(),
            ));
        }
        let old = self.find(token_id)?.ok_or_else(|| {
            KshanaError::InvalidInput(format!("no issued token {token_id} in the register"))
        })?;
        if self.is_revoked(token_id)? {
            return Err(KshanaError::InvalidInput(format!(
                "{token_id} is revoked; issue a new licence instead"
            )));
        }
        let (payload, _) = super::split_signed(&old.token)?;
        let mut claims: LicenseClaims = serde_json::from_slice(&payload)
            .map_err(|e| KshanaError::InvalidInput(format!("{e}")))?;
        let base = if claims.expires_unix == 0 {
            return Err(KshanaError::InvalidInput(format!(
                "{token_id} is perpetual; nothing to renew"
            )));
        } else {
            claims.expires_unix.max(now)
        };
        claims.expires_unix = base + days * DAY;
        claims.not_before_unix = 0;
        self.issue_action(
            seed,
            &claims,
            How {
                action: Action::Renew,
                renews: Some(token_id.to_string()),
                source,
                note: "",
            },
            now,
        )
    }

    /// Revoke `token_id` (it must be in the register; revoking twice is a no-op).
    pub fn revoke(
        &self,
        token_id: &str,
        reason: &str,
        source: &str,
        now: i64,
    ) -> Result<Option<Entry>, KshanaError> {
        let issued = self.find(token_id)?.ok_or_else(|| {
            KshanaError::InvalidInput(format!("no issued token {token_id} in the register"))
        })?;
        if self.is_revoked(token_id)? {
            return Ok(None);
        }
        self.append(Entry {
            utc_unix: now,
            action: Some(Action::Revoke),
            token_id: token_id.to_string(),
            licensee: issued.licensee,
            tier: issued.tier,
            products: issued.products,
            expires_unix: issued.expires_unix,
            kid: issued.kid,
            source: source.to_string(),
            note: reason.to_string(),
            ..Default::default()
        })
        .map(Some)
    }

    /// The signed revocation list of every revoked token, plus `retired_kids`. Its
    /// sequence is the register length, which only grows.
    pub fn revocation_list(
        &self,
        seed: &[u8; KEY_LEN],
        retired_kids: &[String],
        now: i64,
    ) -> Result<String, KshanaError> {
        let n = self.verify()?;
        let entries = self.entries()?;
        let mut revoked: Vec<String> = entries
            .iter()
            .filter(|e| e.action == Some(Action::Revoke))
            .map(|e| e.token_id.clone())
            .collect();
        revoked.sort();
        revoked.dedup();
        let own = key_id(&public_key_for(seed));
        if retired_kids.iter().any(|k| k.eq_ignore_ascii_case(&own)) {
            return Err(KshanaError::InvalidInput(
                "a list cannot retire the key that signs it".into(),
            ));
        }
        revocation::sign(
            seed,
            &RevocationList {
                sequence: n as u64,
                issued_unix: now,
                revoked,
                retired_kids: retired_kids.to_vec(),
                ..Default::default()
            },
        )
    }

    /// Tokens due for renewal: issued, not revoked, not already renewed, expiring
    /// between `now` and `now + within_days` (or expired within the grace period).
    pub fn reminders(&self, now: i64, within_days: i64) -> Result<Vec<Entry>, KshanaError> {
        let entries = self.entries()?;
        let revoked = |id: &str| {
            entries
                .iter()
                .any(|e| e.action == Some(Action::Revoke) && e.token_id == id)
        };
        let renewed = |id: &str| entries.iter().any(|e| e.renews.as_deref() == Some(id));
        let from = now - super::activation::GRACE_DAYS * DAY;
        Ok(entries
            .iter()
            .filter(|e| e.issues_token() && e.expires_unix != 0)
            .filter(|e| e.expires_unix > from && e.expires_unix <= now + within_days * DAY)
            .filter(|e| !revoked(&e.token_id) && !renewed(&e.token_id))
            .cloned()
            .collect())
    }
}
