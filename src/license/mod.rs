// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # Licensing & build mechanics (PR7)
//!
//! The entitlement layer that makes Kshana Pro a *licensable* product: tiers, a
//! tamper-evident license token, and feature gating that maps each Pro pack to the
//! tier that unlocks it.
//!
//! **Token format (honest):** `payload_hex.signature_hex`, where `payload` is the
//! JSON of [`LicenseClaims`] and `signature` is `HMAC-SHA256(secret, payload)`. The
//! secret is the vendor's signing key (configured per deployment, never committed).
//! HMAC is symmetric — fine for a self-hosted check; a distributed product would
//! move to asymmetric signing (e.g. Ed25519: vendor signs with a private key, the
//! build verifies with an embedded public key) so the verifying binary never holds
//! the signing secret. That swap is isolated to [`issue`]/[`verify`].
//!
//! **Eval default:** with no license, a Pro build runs in *Evaluation* tier — only
//! the free seam-check pack is entitled; the value packs require a real license.

use hmac::{Hmac, Mac};
use kshana::api::KshanaError;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Pro licence tiers, ordered by capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// No license / default: only the free seam-check.
    Evaluation,
    /// The standard Pro tier: the L1 twin + L3 trade-study packs.
    Pro,
    /// Enterprise: everything, including the scale (batch/Monte-Carlo) features.
    Enterprise,
}

impl Tier {
    /// Capability rank (higher = more). Used for tier comparisons.
    pub fn rank(self) -> u8 {
        match self {
            Tier::Evaluation => 0,
            Tier::Pro => 1,
            Tier::Enterprise => 2,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Evaluation => "evaluation",
            Tier::Pro => "pro",
            Tier::Enterprise => "enterprise",
        }
    }
    pub fn parse(s: &str) -> Result<Tier, KshanaError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "evaluation" | "eval" => Ok(Tier::Evaluation),
            "pro" => Ok(Tier::Pro),
            "enterprise" | "ent" => Ok(Tier::Enterprise),
            other => Err(KshanaError::InvalidInput(format!(
                "unknown tier '{other}' (expected evaluation|pro|enterprise)"
            ))),
        }
    }
}

/// The minimum tier that unlocks a given Pro scenario kind. Unknown kinds default
/// to `Enterprise` (fail closed: an unrecognised feature is never free).
pub fn feature_min_tier(kind: &str) -> Tier {
    match kind {
        "pro-seam-check" => Tier::Evaluation,
        "pro-twin" => Tier::Pro,
        "pro-trade-study" => Tier::Pro,
        "pro-batch" => Tier::Enterprise,
        _ => Tier::Enterprise,
    }
}

/// The signed claims carried by a license token.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LicenseClaims {
    pub licensee: String,
    pub tier: String,
    /// Unix seconds; `0` means perpetual (never expires).
    pub expires_unix: i64,
    /// Explicit extra feature kinds granted regardless of tier (e.g. a Pro-tier
    /// licence with one Enterprise feature unlocked).
    #[serde(default)]
    pub features: Vec<String>,
}

/// A verified entitlement derived from a valid token.
#[derive(Debug, Clone)]
pub struct Entitlement {
    pub licensee: String,
    pub tier: Tier,
    pub expires_unix: i64,
    pub features: Vec<String>,
}

impl Entitlement {
    /// The default (no-license) entitlement: Evaluation tier, never expires.
    pub fn evaluation() -> Self {
        Entitlement {
            licensee: "(evaluation — no license)".into(),
            tier: Tier::Evaluation,
            expires_unix: 0,
            features: Vec::new(),
        }
    }

    pub fn is_expired(&self, now_unix: i64) -> bool {
        self.expires_unix != 0 && now_unix >= self.expires_unix
    }

    /// Whether this entitlement permits running `kind` at `now_unix`. Expired
    /// licenses permit nothing. Open (non-`pro-`) kinds are always free and are not
    /// gated here (Pro embeds the Apache-2.0 engine).
    pub fn allows(&self, kind: &str, now_unix: i64) -> bool {
        if !kind.starts_with("pro-") {
            return true; // open features are free
        }
        if self.is_expired(now_unix) {
            return false;
        }
        if self.features.iter().any(|f| f == kind) {
            return true; // explicitly granted
        }
        self.tier.rank() >= feature_min_tier(kind).rank()
    }
}

/// Sign claims into a license token with `secret`.
pub fn issue(secret: &[u8], claims: &LicenseClaims) -> Result<String, KshanaError> {
    let payload = serde_json::to_vec(claims)
        .map_err(|e| KshanaError::InvalidInput(format!("could not serialise claims: {e}")))?;
    let sig = sign(secret, &payload);
    Ok(format!("{}.{}", hex::encode(&payload), hex::encode(sig)))
}

/// Verify a token against `secret` at `now_unix`, returning the entitlement.
/// Fails on a malformed token, a bad signature (tamper), an unknown tier, or
/// expiry.
pub fn verify(secret: &[u8], token: &str, now_unix: i64) -> Result<Entitlement, KshanaError> {
    let (payload_hex, sig_hex) = token.split_once('.').ok_or_else(|| {
        KshanaError::InvalidInput("malformed token (expected payload.signature)".into())
    })?;
    let payload = hex::decode(payload_hex)
        .map_err(|_| KshanaError::InvalidInput("malformed token payload (not hex)".into()))?;
    let sig = hex::decode(sig_hex)
        .map_err(|_| KshanaError::InvalidInput("malformed token signature (not hex)".into()))?;

    // Constant-time verification via the MAC's own `verify_slice`.
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|e| KshanaError::InvalidInput(format!("invalid signing key: {e}")))?;
    mac.update(&payload);
    mac.verify_slice(&sig).map_err(|_| {
        KshanaError::InvalidInput("invalid license signature (tampered or wrong key)".into())
    })?;

    let claims: LicenseClaims = serde_json::from_slice(&payload)
        .map_err(|e| KshanaError::InvalidInput(format!("invalid license claims: {e}")))?;
    let tier = Tier::parse(&claims.tier)?;
    let ent = Entitlement {
        licensee: claims.licensee,
        tier,
        expires_unix: claims.expires_unix,
        features: claims.features,
    };
    if ent.is_expired(now_unix) {
        return Err(KshanaError::InvalidInput(format!(
            "license expired (expires_unix={}, now={now_unix})",
            ent.expires_unix
        )));
    }
    Ok(ent)
}

fn sign(secret: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(payload);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-vendor-signing-secret";

    fn claims(tier: &str, expires: i64) -> LicenseClaims {
        LicenseClaims {
            licensee: "ACME Space".into(),
            tier: tier.into(),
            expires_unix: expires,
            features: Vec::new(),
        }
    }

    #[test]
    fn issue_then_verify_round_trips() {
        let tok = issue(SECRET, &claims("pro", 0)).unwrap();
        let ent = verify(SECRET, &tok, 1_000).unwrap();
        assert_eq!(ent.tier, Tier::Pro);
        assert_eq!(ent.licensee, "ACME Space");
    }

    #[test]
    fn tampered_token_fails() {
        let tok = issue(SECRET, &claims("enterprise", 0)).unwrap();
        // Flip a character in the payload portion.
        let mut bad = tok.clone();
        let first = bad.chars().next().unwrap();
        let repl = if first == 'a' { 'b' } else { 'a' };
        bad.replace_range(0..1, &repl.to_string());
        assert!(verify(SECRET, &bad, 1_000).is_err());
    }

    #[test]
    fn wrong_secret_fails() {
        let tok = issue(SECRET, &claims("pro", 0)).unwrap();
        assert!(verify(b"other-secret", &tok, 1_000).is_err());
    }

    #[test]
    fn expired_license_fails_verify() {
        let tok = issue(SECRET, &claims("pro", 5_000)).unwrap();
        assert!(verify(SECRET, &tok, 4_999).is_ok()); // before expiry
        assert!(verify(SECRET, &tok, 5_000).is_err()); // at/after expiry
    }

    #[test]
    fn entitlement_gates_by_tier() {
        let pro = Entitlement {
            licensee: "x".into(),
            tier: Tier::Pro,
            expires_unix: 0,
            features: vec![],
        };
        assert!(pro.allows("pro-twin", 0)); // Pro feature
        assert!(pro.allows("pro-trade-study", 0));
        assert!(!pro.allows("pro-batch", 0)); // Enterprise-only
        assert!(pro.allows("pro-seam-check", 0)); // Eval feature
        assert!(pro.allows("ephemeris", 0)); // open = free

        let ent = Entitlement {
            licensee: "y".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
        };
        assert!(ent.allows("pro-batch", 0)); // Enterprise unlocks scale

        let eval = Entitlement::evaluation();
        assert!(eval.allows("pro-seam-check", 0));
        assert!(!eval.allows("pro-twin", 0)); // value packs need a license
    }

    #[test]
    fn explicit_feature_grant_overrides_tier() {
        let ent = Entitlement {
            licensee: "z".into(),
            tier: Tier::Pro,
            expires_unix: 0,
            features: vec!["pro-batch".into()],
        };
        assert!(
            ent.allows("pro-batch", 0),
            "explicit grant unlocks an above-tier feature"
        );
    }

    #[test]
    fn expired_entitlement_allows_nothing_pro() {
        let ent = Entitlement {
            licensee: "z".into(),
            tier: Tier::Enterprise,
            expires_unix: 100,
            features: vec![],
        };
        assert!(!ent.allows("pro-twin", 200)); // expired
        assert!(ent.allows("ephemeris", 200)); // open still free
    }

    #[test]
    fn unknown_pro_kind_defaults_to_enterprise() {
        assert_eq!(feature_min_tier("pro-mystery"), Tier::Enterprise);
        let pro = Entitlement {
            licensee: "x".into(),
            tier: Tier::Pro,
            expires_unix: 0,
            features: vec![],
        };
        assert!(!pro.allows("pro-mystery", 0));
    }
}
