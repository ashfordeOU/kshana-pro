// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Licensing & build mechanics (PR7)
//!
//! The entitlement layer that makes Kshana Pro a *licensable* product: tiers, a
//! tamper-evident license token, and feature gating that maps each Pro pack to the
//! tier that unlocks it.
//!
//! **Token format:** `payload_hex.signature_hex`, where `payload` is the JSON of
//! [`LicenseClaims`] and `signature` is an Ed25519 signature over it. The vendor
//! signs with a private key that never leaves the vendor ([`issue`]); a build
//! verifies with the matching public key only ([`verify`]), embedded at compile
//! time from `KSHANA_PRO_LICENSE_PUBKEY` ([`embedded_public_key`]). A binary
//! handed to a customer therefore cannot mint tokens: holding it gives the public
//! key, not the signing key. (Until 2026-09-26 tokens were HMAC-SHA256, where the
//! verifying binary had to hold the signing secret; those tokens no longer verify.)
//!
//! **Eval default:** with no license, a Pro build runs in *Evaluation* tier — only
//! the free seam-check pack is entitled; the value packs require a real license.
//!
//! **What this does not do:** it does not bind a token to a machine, and it does
//! not stop someone who has the *source* from removing the check. The protection
//! for the source is that this repository is private and never published.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kshana::api::KshanaError;

/// Length of an Ed25519 private-key seed and of a public key, in bytes.
pub const KEY_LEN: usize = 32;

/// The licence public key compiled into this build, from the
/// `KSHANA_PRO_LICENSE_PUBKEY` environment variable at build time (64 hex
/// characters). `None` when the build was made without one: such a build can
/// verify no token and runs every Pro kind at Evaluation tier. There is
/// deliberately no run-time override, which would let a holder of the binary
/// substitute a key of their own and sign their own tokens.
pub fn embedded_public_key() -> Option<Result<[u8; KEY_LEN], KshanaError>> {
    option_env!("KSHANA_PRO_LICENSE_PUBKEY")
        .filter(|s| !s.trim().is_empty())
        .map(parse_key_hex)
}

/// Parse a 32-byte key written as 64 hex characters.
pub fn parse_key_hex(s: &str) -> Result<[u8; KEY_LEN], KshanaError> {
    let bytes =
        hex::decode(s.trim()).map_err(|_| KshanaError::InvalidInput("key is not hex".into()))?;
    bytes.try_into().map_err(|b: Vec<u8>| {
        KshanaError::InvalidInput(format!(
            "key must be {KEY_LEN} bytes ({} hex characters), got {} bytes",
            2 * KEY_LEN,
            b.len()
        ))
    })
}

/// The public key that verifies tokens signed with the private-key `seed`.
pub fn public_key_for(seed: &[u8; KEY_LEN]) -> [u8; KEY_LEN] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

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
        "pro-quantum-systematics" => Tier::Pro,
        "pro-optimism-gap-audit" => Tier::Pro,
        "pro-lunar-signal-security" => Tier::Pro,
        "pro-beacon-dop" => Tier::Pro,
        "pro-surface-beacon-dop-study" => Tier::Pro,
        "pro-beacon-placement" => Tier::Pro,
        "pro-lunar-time-budget-study" => Tier::Pro,
        "pro-realtime-frame-eop-study" => Tier::Pro,
        "pro-optical-rf-hybrid-study" => Tier::Pro,
        "pro-cislunar-observability-study" => Tier::Pro,
        "pro-conflict-resilience-study" => Tier::Pro,
        "pro-scenario-ci" => Tier::Pro,
        "pro-study-dossier" => Tier::Pro,
        "pro-mbse" => Tier::Enterprise,
        "pro-batch" => Tier::Enterprise,
        _ => Tier::Enterprise,
    }
}

/// Gate the Pack C (scenario-subscription CI) feature: succeed when `ent` permits
/// the `pro-scenario-ci` feature at `now_unix`, else return an [`Unsupported`]
/// error naming the tier the feature requires.
///
/// [`Unsupported`]: KshanaError::Unsupported
pub fn ensure_ci_entitled(ent: &Entitlement, now_unix: i64) -> Result<(), KshanaError> {
    if ent.allows("pro-scenario-ci", now_unix) {
        Ok(())
    } else {
        Err(KshanaError::Unsupported(format!(
            "Pack C (scenario-subscription CI) requires the '{}' tier",
            feature_min_tier("pro-scenario-ci").as_str()
        )))
    }
}

/// Gate the study dossier (`kshana-pro study --dossier`): succeed when `ent` permits
/// the `pro-study-dossier` feature at `now_unix`, else return an [`Unsupported`]
/// error naming the tier the feature requires.
///
/// [`Unsupported`]: KshanaError::Unsupported
pub fn ensure_dossier_entitled(ent: &Entitlement, now_unix: i64) -> Result<(), KshanaError> {
    if ent.allows("pro-study-dossier", now_unix) {
        Ok(())
    } else {
        Err(KshanaError::Unsupported(format!(
            "the study dossier requires the '{}' tier",
            feature_min_tier("pro-study-dossier").as_str()
        )))
    }
}

/// Gate the MBSE and programme tooling (`kshana-pro mbse ...`): succeed when `ent`
/// permits the `pro-mbse` feature at `now_unix`, else return an [`Unsupported`] error
/// naming the tier the feature requires. It is the same feature id as the `pro-mbse`
/// scenario pack, so one entitlement covers both.
///
/// [`Unsupported`]: KshanaError::Unsupported
pub fn ensure_mbse_entitled(ent: &Entitlement, now_unix: i64) -> Result<(), KshanaError> {
    if ent.allows("pro-mbse", now_unix) {
        Ok(())
    } else {
        Err(KshanaError::Unsupported(format!(
            "the MBSE tooling requires the '{}' tier (or the 'pro-mbse' feature on the licence)",
            feature_min_tier("pro-mbse").as_str()
        )))
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
    /// gated here (Pro embeds the open AGPL-3.0 engine).
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

/// Sign claims into a license token with the vendor's private-key `seed`.
pub fn issue(seed: &[u8; KEY_LEN], claims: &LicenseClaims) -> Result<String, KshanaError> {
    let payload = serde_json::to_vec(claims)
        .map_err(|e| KshanaError::InvalidInput(format!("could not serialise claims: {e}")))?;
    let sig = SigningKey::from_bytes(seed).sign(&payload);
    Ok(format!(
        "{}.{}",
        hex::encode(&payload),
        hex::encode(sig.to_bytes())
    ))
}

/// Verify a token against the vendor `public_key` at `now_unix`, returning the
/// entitlement. Fails on a malformed token, a bad signature (tamper or wrong key),
/// an unknown tier, or expiry.
pub fn verify(
    public_key: &[u8; KEY_LEN],
    token: &str,
    now_unix: i64,
) -> Result<Entitlement, KshanaError> {
    let (payload_hex, sig_hex) = token.trim().split_once('.').ok_or_else(|| {
        KshanaError::InvalidInput("malformed token (expected payload.signature)".into())
    })?;
    let payload = hex::decode(payload_hex)
        .map_err(|_| KshanaError::InvalidInput("malformed token payload (not hex)".into()))?;
    let sig_bytes: [u8; 64] = hex::decode(sig_hex)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| {
            KshanaError::InvalidInput("malformed token signature (not 64 hex bytes)".into())
        })?;
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| KshanaError::InvalidInput("invalid licence public key".into()))?;
    key.verify_strict(&payload, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; KEY_LEN] = [7; KEY_LEN];

    fn public() -> [u8; KEY_LEN] {
        public_key_for(&SEED)
    }

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
        let tok = issue(&SEED, &claims("pro", 0)).unwrap();
        let ent = verify(&public(), &tok, 1_000).unwrap();
        assert_eq!(ent.tier, Tier::Pro);
        assert_eq!(ent.licensee, "ACME Space");
    }

    #[test]
    fn public_key_cannot_sign() {
        // The verifying side holds only the public key. Using those same bytes as a
        // signing seed yields a different key pair, so its tokens do not verify.
        let forged = issue(&public(), &claims("enterprise", 0)).unwrap();
        assert!(verify(&public(), &forged, 1_000).is_err());
    }

    #[test]
    fn hmac_era_token_is_rejected() {
        // A 32-byte signature (the old HMAC-SHA256 format) is malformed now.
        let payload = hex::encode(serde_json::to_vec(&claims("pro", 0)).unwrap());
        let tok = format!("{payload}.{}", hex::encode([0u8; 32]));
        assert!(verify(&public(), &tok, 1_000).is_err());
    }

    #[test]
    fn parse_key_hex_checks_length() {
        assert_eq!(parse_key_hex(&hex::encode(public())).unwrap(), public());
        assert!(parse_key_hex("abcd").is_err());
        assert!(parse_key_hex("zz").is_err());
    }

    #[test]
    fn tampered_token_fails() {
        let tok = issue(&SEED, &claims("enterprise", 0)).unwrap();
        // Flip a character in the payload portion.
        let mut bad = tok.clone();
        let first = bad.chars().next().unwrap();
        let repl = if first == 'a' { 'b' } else { 'a' };
        bad.replace_range(0..1, &repl.to_string());
        assert!(verify(&public(), &bad, 1_000).is_err());
    }

    #[test]
    fn wrong_secret_fails() {
        let tok = issue(&SEED, &claims("pro", 0)).unwrap();
        assert!(verify(&public_key_for(&[9; KEY_LEN]), &tok, 1_000).is_err());
    }

    #[test]
    fn expired_license_fails_verify() {
        let tok = issue(&SEED, &claims("pro", 5_000)).unwrap();
        assert!(verify(&public(), &tok, 4_999).is_ok()); // before expiry
        assert!(verify(&public(), &tok, 5_000).is_err()); // at/after expiry
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

    #[test]
    fn pack_c_feature_is_pro_tier() {
        assert_eq!(feature_min_tier("pro-scenario-ci"), Tier::Pro);
        assert_eq!(feature_min_tier("pro-study-dossier"), Tier::Pro);
        assert!(ensure_dossier_entitled(&Entitlement::evaluation(), 0).is_err());
    }

    #[test]
    fn ensure_ci_entitled_denies_evaluation_and_allows_pro() {
        // Evaluation (no license) is not entitled to the Pack C CI feature.
        assert!(ensure_ci_entitled(&Entitlement::evaluation(), 0).is_err());

        // An unexpired Pro entitlement is entitled.
        let pro = Entitlement {
            licensee: "ACME Space".into(),
            tier: Tier::Pro,
            expires_unix: 0,
            features: vec![],
        };
        assert!(ensure_ci_entitled(&pro, 0).is_ok());

        // An unexpired Pro license also entitles (constructed via the token path).
        let tok = issue(&SEED, &claims("pro", 5_000)).unwrap();
        let ent = verify(&public(), &tok, 4_999).unwrap();
        assert!(ensure_ci_entitled(&ent, 4_999).is_ok());

        // An expired entitlement is denied.
        let expired = Entitlement {
            licensee: "ACME Space".into(),
            tier: Tier::Pro,
            expires_unix: 100,
            features: vec![],
        };
        assert!(ensure_ci_entitled(&expired, 200).is_err());
    }
}
