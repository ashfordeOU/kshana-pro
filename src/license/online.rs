// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Automatic licence check and renewal (roadmap 0.42 C1, owner 15 November 2026).
//!
//! Subscriptions renew by themselves. The app asks the vendor's licence server at start
//! and about once a day ([`check_if_due`]); while the subscription is active the server
//! answers with a fresh 35-day token bound to this device, which replaces the stored one
//! without the user doing anything. When the server says the subscription has ended
//! (cancelled, payment failed and not recovered, refunded, revoked), the answer is
//! recorded and the licence ends at once: Pro locks, the 30-day renewal window starts,
//! and after it the Pro pack is deleted ([`crate::propack::loader`]). Offline, the stored
//! token keeps Pro working until its own expiry (at most 35 days), then Pro locks the same
//! way; the next check that reaches the server puts it right.
//!
//! * **What is sent**: the licence id (the token id), the device's machine hash, the app
//!   version, the pack version and a random nonce. No name, no e-mail, no token text.
//! * **What comes back** is signed with the licence signing key ([`CheckResponse`],
//!   `payload_hex.signature_hex`) and carries the request's nonce, so a forged or replayed
//!   answer is refused. A new token must verify under the build's keys, name the same
//!   licensee and be bound to this device.
//! * An answer that cannot be checked, or no answer, changes nothing ("offline").

use std::path::PathBuf;

use ed25519_dalek::{Signer, SigningKey};
use kshana::api::KshanaError;

use super::activation::{evaluate, Store};
use super::lifecycle::{Delivery, LicenceService, TOKEN_DAYS};
use super::{
    check_signature, key_id, public_key_for, split_signed, Keyring, VerifyOptions, KEY_LEN,
};

/// The response format.
pub const FORMAT: &str = "kshana-licence-check/1";
/// How often the app checks (seconds): about daily.
pub const CHECK_INTERVAL_SECS: i64 = 20 * 3600;
const DAY: i64 = 86_400;

/// The request body (JSON). Nothing personal.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRequest {
    pub licence_id: String,
    pub device: String,
    pub app_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_version: Option<String>,
    pub nonce: String,
}

/// The signed answer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CheckResponse {
    pub format: String,
    pub nonce: String,
    pub licence_id: String,
    pub device: String,
    pub server_unix: i64,
    /// `active` or `ended`.
    pub status: String,
    #[serde(default)]
    pub reason: String,
    /// A fresh token for this device (active only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// When the licence ended (ended only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_unix: Option<i64>,
    #[serde(default)]
    pub kid: String,
}

fn sign_response(seed: &[u8; KEY_LEN], r: &CheckResponse) -> Result<String, KshanaError> {
    let mut r = r.clone();
    r.format = FORMAT.into();
    r.kid = key_id(&public_key_for(seed));
    let payload = serde_json::to_vec(&r)
        .map_err(|e| KshanaError::InvalidInput(format!("could not serialise: {e}")))?;
    let sig = SigningKey::from_bytes(seed).sign(&payload);
    Ok(format!(
        "{}.{}",
        hex::encode(&payload),
        hex::encode(sig.to_bytes())
    ))
}

/// Verify a signed answer against `ring`.
pub fn verify_response(ring: &Keyring, text: &str) -> Result<CheckResponse, KshanaError> {
    let (payload, sig) = split_signed(text)?;
    let r: CheckResponse = serde_json::from_slice(&payload)
        .map_err(|e| KshanaError::InvalidInput(format!("licence check answer: {e}")))?;
    check_signature(
        ring,
        Some(&r.kid).filter(|k| !k.is_empty()).map(|s| s.as_str()),
        &payload,
        &sig,
    )
    .map_err(|e| KshanaError::InvalidInput(format!("licence check answer: {e}")))?;
    if r.format != FORMAT {
        return Err(KshanaError::InvalidInput(
            "licence check answer: unknown format".into(),
        ));
    }
    Ok(r)
}

impl LicenceService {
    /// Answer a licence check (server side). Unknown licences and devices without a seat
    /// are errors (not signed: the app treats them as no answer). An active licence gets
    /// its device's newest token, reissued when it is more than a day old.
    pub fn check(&self, req: &CheckRequest, now: i64) -> Result<String, KshanaError> {
        let bad = |m: &str| KshanaError::InvalidInput(m.to_string());
        if req.nonce.len() < 16 || req.nonce.len() > 128 {
            return Err(bad("nonce must be 16 to 128 characters"));
        }
        let root = self
            .ledger
            .root_of(&req.licence_id)?
            .ok_or_else(|| bad("unknown licence"))?;
        let mut all = self.ledger.load()?;
        let rec = all
            .get(&root)
            .cloned()
            .ok_or_else(|| bad("unknown licence"))?;
        let Some(seat) = rec.seats.iter().find(|s| s.device == req.device).cloned() else {
            return Err(bad("this device holds no seat of the licence"));
        };
        let mut resp = CheckResponse {
            format: FORMAT.into(),
            nonce: req.nonce.clone(),
            licence_id: req.licence_id.clone(),
            device: req.device.clone(),
            server_unix: now,
            status: "active".into(),
            reason: String::new(),
            token: None,
            ended_unix: None,
            kid: String::new(),
        };
        if rec.revoked || self.register.is_revoked(&seat.token_id)? {
            resp.status = "ended".into();
            resp.reason = "revoked (refund, chargeback, cancellation or by the vendor)".into();
            resp.ended_unix = Some(now);
        } else if rec.paid_through_unix <= now {
            resp.status = "ended".into();
            resp.reason = "the subscription has ended".into();
            resp.ended_unix = Some(rec.paid_through_unix);
        } else {
            let current = self
                .register
                .find(&seat.token_id)?
                .ok_or_else(|| bad("seat token missing from the register"))?;
            let fresh = rec.delivery == Delivery::Offline
                || current.expires_unix
                    >= (now + (TOKEN_DAYS - 1) * DAY).min(rec.paid_through_unix);
            if fresh {
                resp.token = Some(current.token);
            } else {
                drop(all);
                let mut rec = rec;
                let out = self.reissue_seat(&root, &mut rec, &seat.token_id, &req.device, now)?;
                all = self.ledger.load()?;
                if let Some(s) = rec.seats.iter_mut().find(|s| s.device == req.device) {
                    s.token_id = out.token_id.clone();
                }
                all.insert(root.clone(), rec);
                self.ledger.save(&all)?;
                resp.token = Some(out.token);
            }
        }
        sign_response(self.seed(), &resp)
    }
}

/// Sends a check to the server. `Err` means no answer (offline, refused, server error).
pub trait CheckTransport {
    fn post(&self, path: &str, body: &str) -> Result<String, String>;
}

/// The system `curl` (HTTPS only), like the opt-in refresh.
pub struct CurlTransport {
    pub base_url: String,
}

impl CheckTransport for CurlTransport {
    fn post(&self, path: &str, body: &str) -> Result<String, String> {
        let base = self.base_url.trim_end_matches('/');
        if !base.starts_with("https://") {
            return Err("the licence server must be an https:// URL".into());
        }
        let out = std::process::Command::new("curl")
            .args([
                "-sS",
                "--proto",
                "=https",
                "--max-time",
                "30",
                "-f",
                "-H",
                "Content-Type: application/json",
                "--data-binary",
                body,
                &format!("{base}{path}"),
            ])
            .output()
            .map_err(|e| format!("could not run curl: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

/// The licence server this build asks: `KSHANA_PRO_LICENCE_URL` at build time, or the
/// store's `refresh.toml` `url` when set.
pub fn server_url(store: &Store) -> Option<String> {
    let cfg = store.refresh_config();
    if !cfg.url.trim().is_empty() {
        return Some(cfg.url.trim().to_string());
    }
    option_env!("KSHANA_PRO_LICENCE_URL")
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
}

/// What a check did.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckOutcome {
    /// A fresh token replaced the stored one.
    Renewed { expires_unix: i64 },
    /// The server confirmed the licence; the stored token is current.
    Unchanged,
    /// The server says the licence has ended: Pro is locked from `ended_unix`.
    Ended { reason: String, ended_unix: i64 },
    /// No usable answer; nothing changed.
    Offline(String),
    /// No activated licence to check.
    NoLicence,
}

fn ended_path(store: &Store) -> PathBuf {
    store.dir().join("licence-ended.txt")
}

fn last_check_path(store: &Store) -> PathBuf {
    store.dir().join("licence-check.json")
}

/// The verified "ended" answer stored for the activated token, if any.
pub fn ended_record(store: &Store, ring: &Keyring) -> Option<CheckResponse> {
    let text = std::fs::read_to_string(ended_path(store)).ok()?;
    let r = verify_response(ring, &text).ok()?;
    let tok = store.token()?;
    let (payload, _) = split_signed(&tok).ok()?;
    let claims: super::LicenseClaims = serde_json::from_slice(&payload).ok()?;
    let tid = super::token_id(&tok, &claims);
    (r.status == "ended" && r.licence_id == tid).then_some(r)
}

/// Ask the server now. `device` is this machine's hash.
pub fn check_now(
    store: &Store,
    ring: &Keyring,
    transport: &dyn CheckTransport,
    device: &str,
    app_version: &str,
    pack_version: Option<&str>,
    now: i64,
) -> Result<CheckOutcome, KshanaError> {
    let Some(tok) = store.token() else {
        return Ok(CheckOutcome::NoLicence);
    };
    let (payload, _) = split_signed(&tok)?;
    let claims: super::LicenseClaims = serde_json::from_slice(&payload)
        .map_err(|e| KshanaError::InvalidInput(format!("stored token: {e}")))?;
    let licence_id = super::token_id(&tok, &claims);
    let nonce = {
        use rand::RngCore;
        let mut b = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut b);
        hex::encode(b)
    };
    let req = CheckRequest {
        licence_id: licence_id.clone(),
        device: device.to_string(),
        app_version: app_version.to_string(),
        pack_version: pack_version.map(str::to_string),
        nonce: nonce.clone(),
    };
    let body = serde_json::to_string(&req).unwrap_or_default();
    let text = match transport.post("/v1/licence/check", &body) {
        Ok(t) => t,
        Err(e) => return Ok(CheckOutcome::Offline(e)),
    };
    let r = match verify_response(ring, &text) {
        Ok(r) => r,
        Err(e) => return Ok(CheckOutcome::Offline(format!("answer refused: {e}"))),
    };
    if r.nonce != nonce || r.licence_id != licence_id || r.device != device {
        return Ok(CheckOutcome::Offline(
            "answer refused: it does not answer this request (replayed or misdirected)".into(),
        ));
    }
    let _ = std::fs::write(
        last_check_path(store),
        serde_json::json!({ "checked_unix": now, "server_unix": r.server_unix }).to_string(),
    );
    match r.status.as_str() {
        "ended" => {
            write_file(&ended_path(store), &text)?;
            Ok(CheckOutcome::Ended {
                reason: r.reason.clone(),
                ended_unix: r.ended_unix.unwrap_or(r.server_unix),
            })
        }
        "active" => {
            let Some(new) = r.token.as_deref().map(str::trim) else {
                return Ok(CheckOutcome::Unchanged);
            };
            if new == tok {
                return Ok(CheckOutcome::Unchanged);
            }
            let dev = device.to_string();
            let here = move || Some(dev.clone());
            let opts = VerifyOptions {
                now_unix: now,
                product: None,
                machine: Some(&here),
                allow_expired: false,
            };
            let lists: Vec<_> = store.revocations(ring).into_iter().collect();
            let st = evaluate(ring, Some(new), &lists, &opts);
            let ok = st.licence.as_ref().is_some_and(|l| {
                l.claims.licensee == claims.licensee && l.claims.machine.as_deref() == Some(device)
            }) && matches!(
                st.state,
                super::activation::State::Active | super::activation::State::Expiring { .. }
            );
            if !ok {
                return Ok(CheckOutcome::Offline(
                    "answer refused: the new token is not valid for this licence and device".into(),
                ));
            }
            write_file(&store.token_path(), new)?;
            let _ = std::fs::remove_file(ended_path(store));
            Ok(CheckOutcome::Renewed {
                expires_unix: st.licence.map(|l| l.claims.expires_unix).unwrap_or(0),
            })
        }
        _ => Ok(CheckOutcome::Offline(
            "answer refused: unknown status".into(),
        )),
    }
}

/// Check when due: at start (no check yet in this process) and then about daily.
#[allow(clippy::too_many_arguments)]
pub fn check_if_due(
    store: &Store,
    ring: &Keyring,
    transport: &dyn CheckTransport,
    device: &str,
    app_version: &str,
    pack_version: Option<&str>,
    now: i64,
    at_start: bool,
) -> Result<Option<CheckOutcome>, KshanaError> {
    let last = std::fs::read_to_string(last_check_path(store))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["checked_unix"].as_i64())
        .unwrap_or(0);
    if !at_start && now - last < CHECK_INTERVAL_SECS && now >= last {
        return Ok(None);
    }
    check_now(
        store,
        ring,
        transport,
        device,
        app_version,
        pack_version,
        now,
    )
    .map(Some)
}

fn write_file(path: &std::path::Path, text: &str) -> Result<(), KshanaError> {
    let err = |e: std::io::Error| KshanaError::InvalidInput(format!("{}: {e}", path.display()));
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(err)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, text.trim()).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}
