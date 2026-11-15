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

use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
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
        .map(|s| parse_key_hex(s.split(',').next().unwrap_or("")))
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

/// The minimum tier that unlocks a given Pro scenario kind or Pro command feature.
/// Unknown kinds default to `Enterprise` (fail closed: an unrecognised feature is
/// never free). Every Pro kind and command has an explicit entry in [`explicit_tier`];
/// a test fails if one falls through to the default (roadmap F10f).
pub fn feature_min_tier(kind: &str) -> Tier {
    explicit_tier(kind).unwrap_or(Tier::Enterprise)
}

/// The tier explicitly mapped for `kind`, or `None` when it has no entry.
pub fn explicit_tier(kind: &str) -> Option<Tier> {
    Some(match kind {
        "pro-seam-check" => Tier::Evaluation,
        "pro-twin" => Tier::Pro,
        "pro-trade-study" => Tier::Pro,
        "pro-quantum-systematics" => Tier::Pro,
        "pro-optimism-gap-audit" => Tier::Pro,
        "pro-conflict-resilience-study" => Tier::Pro,
        "pro-scenario-ci" => Tier::Pro,
        "pro-study-dossier" => Tier::Pro,
        // Signed dossiers (F7): part of "studies and signed dossiers" in the 0.41
        // edition matrix.
        "pro-signed-dossier" => Tier::Pro,
        // Incident forensics (E1, 0.42): a Pro pack kind (crate::forensics), sold to anyone.
        "pro-incident-forensics" => Tier::Pro,
        "pro-batch" => Tier::Enterprise,
        "pro-uncertainty" => Tier::Enterprise,
        // 0.41 edition matrix (F10a): Pro features that are not scenario kinds.
        // More than one Kshana Station under one management (fleet).
        "pro-station-fleet" => Tier::Pro,
        // The service run highly available (several nodes on shared storage, F2a).
        "pro-service-ha" => Tier::Enterprise,
        // Kshana Desktop's Pro build (separate download, F10e).
        "pro-desktop" => Tier::Pro,
        // Timing assurance (roadmap 0.42 T): the fleet holdover kind, the fleet page with
        // its evidence history and metrics, and the signed audit export.
        "pro-timing-holdover" => Tier::Pro,
        "pro-timing-fleet" => Tier::Pro,
        "pro-timing-audit" => Tier::Pro,
        // Threat scenario library (roadmap 0.42 E2): running a library scenario's analogue,
        // and the exports and signed feed.
        "pro-threat-scenario" => Tier::Pro,
        "pro-threat-library" => Tier::Pro,
        // Receiver resilience scorecards and test reports (roadmap 0.42 E3).
        "pro-receiver-scorecard" => Tier::Pro,
        // Timing scorecards under spoofing scenarios (roadmap 0.42 T4).
        "pro-timing-scorecard" => Tier::Pro,
        // A kind or feature of a registered server-side extension (crate::extension): the
        // edition gate decides; the recorded tier is informational.
        other => return crate::extension::min_tier(other),
    })
}

/// Pro features gated by a command rather than a scenario kind. Each must have an
/// explicit tier ([`explicit_tier`]); the coverage test checks it.
/// A registered server-side extension adds its own ([`crate::extension`]);
/// [`pro_command_features`] lists both.
pub const PRO_COMMAND_FEATURES: &[&str] = &[
    "pro-scenario-ci",
    "pro-study-dossier",
    "pro-signed-dossier",
    "pro-uncertainty",
    "pro-station-fleet",
    "pro-service-ha",
    "pro-desktop",
    "pro-timing-fleet",
    "pro-timing-audit",
    "pro-threat-library",
];

/// Every command feature of this build: [`PRO_COMMAND_FEATURES`] and those of a registered
/// extension.
pub fn pro_command_features() -> Vec<&'static str> {
    PRO_COMMAND_FEATURES
        .iter()
        .copied()
        .chain(crate::extension::features().iter().map(|f| f.id))
        .chain(crate::extension::pack_features().iter().map(|f| f.id))
        .collect()
}

/// Whether `ent` may run the Pro kind or feature `kind` at `now_unix`; the error says
/// why not. An Arksa-exclusive or Core kind is refused with [`tiers::NOT_IN_THIS_EDITION`],
/// which names no kind and no method; a kind the tier manifest does not list is refused
/// as unknown; any other kind with the licence tier it needs.
pub fn check(ent: &Entitlement, kind: &str, now_unix: i64) -> Result<(), KshanaError> {
    if ent.allows(kind, now_unix) {
        return Ok(());
    }
    match tiers::edition_of(kind) {
        Some(tiers::Edition::Arksa | tiers::Edition::Core) => {
            Err(KshanaError::Unsupported(tiers::NOT_IN_THIS_EDITION.into()))
        }
        None => Err(KshanaError::Unsupported(format!(
            "'{kind}' carries the pro- prefix but is not a registered Pro pack"
        ))),
        Some(_) => Err(KshanaError::Unsupported(format!(
            "'{kind}' requires the '{}' tier; current license tier is '{}'{}",
            feature_min_tier(kind).as_str(),
            ent.tier.as_str(),
            if ent.is_expired(now_unix) {
                " (and it has expired)"
            } else {
                ""
            },
        ))),
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

/// The signed claims carried by a license token.
///
/// Version 1 tokens carry the first four fields only. Version 2 (roadmap F10b) adds the
/// rest; every v2 field is optional on the wire and left out when empty, so a v1 token
/// still decodes (and a claims value with no v2 field set serialises exactly as v1 did).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LicenseClaims {
    pub licensee: String,
    pub tier: String,
    /// Unix seconds; `0` means perpetual (never expires).
    pub expires_unix: i64,
    /// Explicit extra feature kinds granted regardless of tier (e.g. a Pro-tier
    /// licence with one Enterprise feature unlocked).
    #[serde(default)]
    pub features: Vec<String>,
    /// Claims version: absent (v1) or `2`.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub v: u8,
    /// Token ID, unique per issued token; what a revocation list names. A v1 token has
    /// none and is named by the SHA-256 of its text instead ([`token_id`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Products the token unlocks (`cli`, `desktop`, `station`, `service`). Empty (v1)
    /// means every product.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<String>,
    /// Seats bought (people or installations). `0`: not counted.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub seats: u32,
    /// Kshana Stations the licence may run at once. `0`: not set (one Station, the
    /// open edition's allowance).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub stations: u32,
    /// Machine binding: [`machine::hash_machine_id`] of the one machine the token runs
    /// on. Absent: not bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// Key ID of the signing key ([`key_id`] of its public key), for key rotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,
    /// Unix seconds before which the token is not valid. `0`: no lower bound.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub not_before_unix: i64,
    /// Unix seconds when the token was issued (informational).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub issued_unix: i64,
    /// The Arksa entitlement (roadmap 0.42 X1): unlocks the Arksa-exclusive kinds. Honoured
    /// only when the token is signed by an Arksa issuer key ([`Keyring::add_arksa_issuer`]);
    /// a token that carries it under any other key is refused outright. Absent (false) on
    /// every token issued before it existed, which verify as before.
    #[serde(default, skip_serializing_if = "is_false")]
    pub arksa: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}
fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}
fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}
fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

/// A verified entitlement derived from a valid token.
#[derive(Debug, Clone)]
pub struct Entitlement {
    pub licensee: String,
    pub tier: Tier,
    pub expires_unix: i64,
    pub features: Vec<String>,
    /// Holds a verified `arksa` claim from Arksa's issuer: the only way to run an
    /// Arksa-exclusive kind ([`tiers`]).
    pub arksa: bool,
}

impl Entitlement {
    /// The default (no-license) entitlement: Evaluation tier, never expires.
    pub fn evaluation() -> Self {
        Entitlement {
            licensee: "(evaluation — no license)".into(),
            tier: Tier::Evaluation,
            expires_unix: 0,
            features: Vec::new(),
            arksa: false,
        }
    }

    pub fn is_expired(&self, now_unix: i64) -> bool {
        self.expires_unix != 0 && now_unix >= self.expires_unix
    }

    /// Whether this entitlement permits running `kind` at `now_unix`. Expired
    /// licenses permit nothing. Open (non-`pro-`) kinds are always free and are not
    /// gated here (Pro embeds the open AGPL-3.0 engine).
    ///
    /// The edition tier comes from the signed tier manifest ([`tiers`]): a `pro-` kind it
    /// does not list is refused; an Arksa-exclusive kind needs the `arksa` entitlement
    /// (no licence tier and no `features` grant unlocks it); a Core kind never runs.
    pub fn allows(&self, kind: &str, now_unix: i64) -> bool {
        if !kind.starts_with("pro-") {
            return true; // open features are free
        }
        let edition = match tiers::edition_of(kind) {
            None | Some(tiers::Edition::Core) => return false,
            Some(e) => e,
        };
        if self.is_expired(now_unix) {
            return false;
        }
        if edition == tiers::Edition::Arksa {
            return self.arksa;
        }
        if self.features.iter().any(|f| f == kind) {
            return true; // explicitly granted
        }
        self.tier.rank() >= feature_min_tier(kind).rank()
    }
}

/// Sign claims into a license token with the vendor's private-key `seed`.
///
/// `signer` is any [`LicenceSigner`]: a raw seed (`&[u8; 32]`) or, with the `kms` feature,
/// an AWS KMS key (`kms::KmsSigner`). The token bytes are the same either way.
pub fn issue(signer: &dyn LicenceSigner, claims: &LicenseClaims) -> Result<String, KshanaError> {
    let payload = serde_json::to_vec(claims)
        .map_err(|e| KshanaError::InvalidInput(format!("could not serialise claims: {e}")))?;
    signer::sign_document(signer, &payload)
}

/// Verify a token against the vendor `public_key` at `now_unix`, returning the
/// entitlement. Fails on a malformed token, a bad signature (tamper or wrong key),
/// an unknown tier, a token not yet valid, a token bound to a machine (this entry point
/// knows no machine), or expiry. Takes v1 and v2 tokens and checks no product.
pub fn verify(
    public_key: &[u8; KEY_LEN],
    token: &str,
    now_unix: i64,
) -> Result<Entitlement, KshanaError> {
    let ring = Keyring::single(*public_key);
    verify_with(&ring, token, &VerifyOptions::at(now_unix)).map(|l| l.entitlement)
}

// ---------------------------------------------------------------------------------
// Licence token v2 (roadmap F10b): products, seats, stations, machine binding, key
// IDs with several public keys for rotation, not-before.
// ---------------------------------------------------------------------------------

/// A Kshana product a token can unlock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Product {
    /// The `kshana-pro` command line.
    Cli,
    /// Kshana Desktop (Pro build).
    Desktop,
    /// Kshana Station, the headless site service.
    Station,
    /// The hosted or on-premises Pro service (`kshana-pro serve`).
    Service,
}

impl Product {
    /// Every product, in a fixed order.
    pub const ALL: [Product; 4] = [
        Product::Cli,
        Product::Desktop,
        Product::Station,
        Product::Service,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Product::Cli => "cli",
            Product::Desktop => "desktop",
            Product::Station => "station",
            Product::Service => "service",
        }
    }
    pub fn parse(s: &str) -> Result<Product, KshanaError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cli" => Ok(Product::Cli),
            "desktop" => Ok(Product::Desktop),
            "station" => Ok(Product::Station),
            "service" => Ok(Product::Service),
            other => Err(KshanaError::InvalidInput(format!(
                "unknown product '{other}' (expected cli|desktop|station|service)"
            ))),
        }
    }
}

/// The key ID of a public key: the first 16 hex characters of its SHA-256. A token
/// names the key that signed it by this ID, so a build holding several keys picks the
/// right one, and a retired key is refused by name.
pub fn key_id(public_key: &[u8; KEY_LEN]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(public_key))[..16].to_string()
}

/// The ID a revocation list uses for `token`: its `id` claim when set (v2), else the
/// SHA-256 of the token text (v1 tokens carry no ID).
pub fn token_id(token: &str, claims: &LicenseClaims) -> String {
    if claims.id.is_empty() {
        use sha2::{Digest, Sha256};
        format!(
            "sha256:{}",
            hex::encode(Sha256::digest(token.trim().as_bytes()))
        )
    } else {
        claims.id.clone()
    }
}

/// The public keys a build trusts, and the key IDs it refuses.
///
/// Rotation: a new key is added to the active set and new tokens are signed with it;
/// tokens signed by the old key keep verifying until the old key is retired. A retired
/// key's ID goes on the retired list (compiled in, or delivered in a signed revocation
/// list, [`revocation`]), and from then on every token it signed is refused.
#[derive(Debug, Clone, Default)]
pub struct Keyring {
    active: Vec<(String, [u8; KEY_LEN])>,
    retired: Vec<String>,
    /// Arksa issuer keys (roadmap 0.42 X1): the only keys whose tokens may carry the
    /// `arksa` claim. Kept apart from `active`, so a Pro key can never grant it.
    arksa: Vec<(String, [u8; KEY_LEN])>,
    /// Root key IDs (key custody S7-b, `KSHANA_PRO_ROOT_KID`): only a revocation list
    /// signed by a root may retire keys. Empty: no root is configured (builds before S7-b).
    roots: Vec<String>,
}

impl Keyring {
    /// Declare `kid` a root key: only lists it signs may retire keys
    /// ([`revocation::verify`]). Several roots may be set during a root rotation.
    pub fn add_root(&mut self, kid: &str) {
        let kid = kid.trim().to_ascii_lowercase();
        if !kid.is_empty() && !self.roots.contains(&kid) {
            self.roots.push(kid);
        }
    }

    /// The root key IDs ([`Keyring::add_root`]).
    pub fn root_ids(&self) -> &[String] {
        &self.roots
    }

    /// Whether `kid` is a root key.
    pub fn is_root(&self, kid: &str) -> bool {
        self.roots
            .iter()
            .any(|r| r.eq_ignore_ascii_case(kid.trim()))
    }

    /// A keyring holding one key.
    pub fn single(public_key: [u8; KEY_LEN]) -> Keyring {
        let mut k = Keyring::default();
        k.add(public_key);
        k
    }

    /// Add an active key (a duplicate is ignored).
    pub fn add(&mut self, public_key: [u8; KEY_LEN]) {
        let kid = key_id(&public_key);
        if !self.active.iter().any(|(k, _)| *k == kid) {
            self.active.push((kid, public_key));
        }
    }

    /// Add an Arksa issuer key: tokens it signs carry the `arksa` claim (a duplicate is
    /// ignored). Its tokens verify only through this slot, never as Pro keys.
    pub fn add_arksa_issuer(&mut self, public_key: [u8; KEY_LEN]) {
        let kid = key_id(&public_key);
        if !self.arksa.iter().any(|(k, _)| *k == kid) {
            self.arksa.push((kid, public_key));
        }
    }

    /// IDs of the Arksa issuer keys (not retired).
    pub fn arksa_issuer_ids(&self) -> Vec<String> {
        self.arksa
            .iter()
            .filter(|(k, _)| !self.is_retired(k))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Retire a key by its ID: tokens it signed stop verifying.
    pub fn retire(&mut self, kid: &str) {
        let kid = kid.trim().to_ascii_lowercase();
        if !kid.is_empty() && !self.retired.contains(&kid) {
            self.retired.push(kid);
        }
    }

    /// Parse a comma- or whitespace-separated list of 64-hex public keys.
    pub fn parse_keys(list: &str) -> Result<Keyring, KshanaError> {
        let mut k = Keyring::default();
        for item in list.split(|c: char| c == ',' || c.is_whitespace()) {
            if !item.is_empty() {
                k.add(parse_key_hex(item)?);
            }
        }
        Ok(k)
    }

    /// The keyring compiled into this build: `KSHANA_PRO_LICENSE_PUBKEY` (one key, or
    /// several separated by commas) and `KSHANA_PRO_LICENSE_RETIRED` (key IDs or
    /// public keys, comma separated). `None` when the build has no key. A build with the
    /// `arksa` feature also takes the Arksa issuer slot, `KSHANA_PRO_ARKSA_ISSUER_PUBKEY`
    /// ([`embedded_arksa_issuers`]); a default build has no Arksa issuer, so it accepts no
    /// token carrying the `arksa` claim.
    pub fn embedded() -> Option<Result<Keyring, KshanaError>> {
        let keys = option_env!("KSHANA_PRO_LICENSE_PUBKEY").filter(|s| !s.trim().is_empty())?;
        let ring = Keyring::parse_keys(keys).and_then(|mut ring| {
            for k in embedded_arksa_issuers()? {
                ring.add_arksa_issuer(k);
            }
            Ok(ring)
        });
        Some(ring.map(|mut ring| {
            for r in option_env!("KSHANA_PRO_LICENSE_RETIRED")
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                match parse_key_hex(r) {
                    Ok(pk) => ring.retire(&key_id(&pk)),
                    Err(_) => ring.retire(r),
                }
            }
            // Key custody S7-b: the root key ID(s), comma separated (key IDs or public keys).
            for r in option_env!("KSHANA_PRO_ROOT_KID")
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                match parse_key_hex(r) {
                    Ok(pk) => ring.add_root(&key_id(&pk)),
                    Err(_) => ring.add_root(r),
                }
            }
            ring
        }))
    }

    /// Whether the key with this ID is retired.
    pub fn is_retired(&self, kid: &str) -> bool {
        self.retired.iter().any(|r| r.eq_ignore_ascii_case(kid))
    }

    /// IDs of the active (not retired) keys.
    pub fn active_ids(&self) -> Vec<String> {
        self.active
            .iter()
            .filter(|(k, _)| !self.is_retired(k))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// The retired key IDs.
    pub fn retired_ids(&self) -> &[String] {
        &self.retired
    }

    fn candidates_in<'a>(
        &self,
        set: &'a [(String, [u8; KEY_LEN])],
        kid: Option<&str>,
    ) -> Result<Vec<(&'a str, &'a [u8; KEY_LEN])>, KshanaError> {
        if let Some(kid) = kid {
            if self.is_retired(kid) {
                return Err(KshanaError::InvalidInput(format!(
                    "license signed by retired key {kid}; ask the vendor for a token signed by a current key"
                )));
            }
            let found: Vec<_> = set
                .iter()
                .filter(|(k, _)| k.eq_ignore_ascii_case(kid))
                .map(|(k, pk)| (k.as_str(), pk))
                .collect();
            if found.is_empty() {
                return Err(KshanaError::InvalidInput(format!(
                    "license signed by key {kid}, which this build does not know"
                )));
            }
            Ok(found)
        } else {
            Ok(set
                .iter()
                .filter(|(k, _)| !self.is_retired(k))
                .map(|(k, pk)| (k.as_str(), pk))
                .collect())
        }
    }
}

/// What a token is checked against besides its signature.
pub struct VerifyOptions<'a> {
    /// Time of the check, Unix seconds.
    pub now_unix: i64,
    /// The product doing the check; `None` checks no product.
    pub product: Option<Product>,
    /// The machine hash of this machine ([`machine::this_machine_hash`]), asked for
    /// only when the token is bound to a machine.
    pub machine: Option<&'a dyn Fn() -> Option<String>>,
    /// Accept a token whose expiry has passed (the grace period is applied by the
    /// caller, [`activation`]); every other check still applies.
    pub allow_expired: bool,
}

impl VerifyOptions<'_> {
    /// Check at `now_unix`, no product, no machine.
    pub fn at(now_unix: i64) -> VerifyOptions<'static> {
        VerifyOptions {
            now_unix,
            product: None,
            machine: None,
            allow_expired: false,
        }
    }
}

/// A token that verified: its claims, the entitlement it grants, the ID of the key
/// that signed it and the token's ID for revocation.
#[derive(Debug, Clone)]
pub struct Licence {
    pub claims: LicenseClaims,
    pub entitlement: Entitlement,
    pub kid: String,
    pub token_id: String,
}

/// Split and decode a signed `payload_hex.signature_hex` document.
pub(crate) fn split_signed(token: &str) -> Result<(Vec<u8>, [u8; 64]), KshanaError> {
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
    Ok((payload, sig_bytes))
}

/// Check the signature of `payload` against the keys `kid` allows; returns the ID of
/// the key that verified it.
pub(crate) fn check_signature(
    ring: &Keyring,
    kid: Option<&str>,
    payload: &[u8],
    sig: &[u8; 64],
) -> Result<String, KshanaError> {
    check_signature_in(ring, false, kid, payload, sig)
}

/// [`check_signature`] against the Pro keys (`arksa` false) or the Arksa issuer keys.
fn check_signature_in(
    ring: &Keyring,
    arksa: bool,
    kid: Option<&str>,
    payload: &[u8],
    sig: &[u8; 64],
) -> Result<String, KshanaError> {
    let sig = Signature::from_bytes(sig);
    let set = if arksa { &ring.arksa } else { &ring.active };
    for (id, pk) in ring.candidates_in(set, kid)? {
        let Ok(key) = VerifyingKey::from_bytes(pk) else {
            continue;
        };
        if key.verify_strict(payload, &sig).is_ok() {
            return Ok(id.to_string());
        }
    }
    Err(KshanaError::InvalidInput(
        "invalid license signature (tampered or wrong key)".into(),
    ))
}

/// Verify a v1 or v2 token against a keyring. Checks, in order: the format, the key
/// (a retired or unknown key ID is refused), the signature, the tier, the products,
/// not-before, the machine binding and the expiry.
pub fn verify_with(
    ring: &Keyring,
    token: &str,
    opts: &VerifyOptions<'_>,
) -> Result<Licence, KshanaError> {
    let (payload, sig) = split_signed(token)?;
    // The key ID is read before the signature is checked only to pick the key; the
    // claims are trusted only after the signature verifies under that key.
    let unverified: LicenseClaims = serde_json::from_slice(&payload)
        .map_err(|e| KshanaError::InvalidInput(format!("invalid license claims: {e}")))?;
    // A token carrying the `arksa` claim verifies only under an Arksa issuer key; any
    // other token only under a Pro key. So a Pro key cannot grant the claim, and adding
    // the claim to a Pro token breaks its signature.
    let kid = check_signature_in(
        ring,
        unverified.arksa,
        unverified.kid.as_deref(),
        &payload,
        &sig,
    )?;
    let claims = unverified;
    let tier = Tier::parse(&claims.tier)?;
    if claims.v > 2 {
        return Err(KshanaError::InvalidInput(format!(
            "license claims version {} is newer than this build reads (2)",
            claims.v
        )));
    }
    if let Some(p) = opts.product {
        if !claims.products.is_empty() && !claims.products.iter().any(|c| c == p.as_str()) {
            return Err(KshanaError::InvalidInput(format!(
                "license is for {} and does not cover {}",
                claims.products.join(", "),
                p.as_str()
            )));
        }
    }
    if claims.not_before_unix != 0 && opts.now_unix < claims.not_before_unix {
        return Err(KshanaError::InvalidInput(format!(
            "license not valid yet (not_before_unix={}, now={})",
            claims.not_before_unix, opts.now_unix
        )));
    }
    if let Some(bound) = &claims.machine {
        let here = opts.machine.and_then(|f| f());
        if here.as_deref() != Some(bound.as_str()) {
            return Err(KshanaError::InvalidInput(
                "license is bound to another machine (compare `kshana-pro license machine-id`)"
                    .into(),
            ));
        }
    }
    let entitlement = Entitlement {
        licensee: claims.licensee.clone(),
        tier,
        expires_unix: claims.expires_unix,
        features: claims.features.clone(),
        arksa: claims.arksa,
    };
    if !opts.allow_expired && entitlement.is_expired(opts.now_unix) {
        return Err(KshanaError::InvalidInput(format!(
            "license expired (expires_unix={}, now={})",
            entitlement.expires_unix, opts.now_unix
        )));
    }
    let token_id = token_id(token, &claims);
    Ok(Licence {
        claims,
        entitlement,
        kid,
        token_id,
    })
}

/// Issue a v2 token: sets `v`, the key ID of `seed`'s public key and, when empty, a
/// random token ID; requires at least one product and a valid tier.
pub fn issue_v2(seed: &dyn LicenceSigner, claims: &LicenseClaims) -> Result<String, KshanaError> {
    let mut c = claims.clone();
    Tier::parse(&c.tier)?;
    if c.products.is_empty() {
        return Err(KshanaError::InvalidInput(
            "a v2 token names at least one product (cli, desktop, station, service)".into(),
        ));
    }
    for p in &c.products {
        Product::parse(p)?;
    }
    if let Some(m) = &c.machine {
        if m.len() != 64 || hex::decode(m).is_err() {
            return Err(KshanaError::InvalidInput(
                "machine binding must be a 64-hex machine hash (`license machine-id`)".into(),
            ));
        }
    }
    c.v = 2;
    c.kid = Some(seed.kid()?);
    if c.id.is_empty() {
        c.id = new_token_id();
    }
    issue(seed, &c)
}

/// Issue a v2 token carrying the `arksa` claim, signed with an Arksa issuer `seed`
/// (roadmap 0.42 X1). Only Arksa's issuer holds such a seed; a build trusts its public
/// key through the Arksa issuer slot ([`Keyring::add_arksa_issuer`]).
pub fn issue_arksa(
    seed: &dyn LicenceSigner,
    claims: &LicenseClaims,
) -> Result<String, KshanaError> {
    let mut c = claims.clone();
    c.arksa = true;
    issue_v2(seed, &c)
}

/// The Arksa issuer public keys this process trusts: those a registered server-side
/// extension supplies ([`crate::extension::Extension::issuer_keys`]). Empty in every
/// Kshana Pro build, which therefore accepts no token carrying the claim.
pub fn embedded_arksa_issuers() -> Result<Vec<[u8; KEY_LEN]>, KshanaError> {
    Ok(crate::extension::issuer_keys())
}

/// A fresh random token ID (`kt_` and 24 hex characters).
pub fn new_token_id() -> String {
    use rand::RngCore;
    let mut b = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut b);
    format!("kt_{}", hex::encode(b))
}

pub mod activation;
pub mod clock;
#[cfg(feature = "kms")]
pub mod kms;
pub mod lifecycle;
pub mod machine;
pub mod online;
pub mod register;
pub mod revocation;
pub mod signer;
pub mod tiers;

pub use signer::LicenceSigner;

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
            ..Default::default()
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
            arksa: false,
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
            arksa: false,
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
            arksa: false,
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
            arksa: false,
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
            arksa: false,
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
            arksa: false,
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
            arksa: false,
        };
        assert!(ensure_ci_entitled(&expired, 200).is_err());
    }

    // ---- v2 (F10b) ----

    fn v2(products: &[&str]) -> LicenseClaims {
        LicenseClaims {
            licensee: "ACME Space".into(),
            tier: "pro".into(),
            expires_unix: 10_000,
            products: products.iter().map(|p| p.to_string()).collect(),
            seats: 5,
            stations: 3,
            ..Default::default()
        }
    }

    #[test]
    fn v1_and_v2_both_verify_and_v2_carries_its_claims() {
        let ring = Keyring::single(public());
        let t1 = issue(&SEED, &claims("pro", 0)).unwrap();
        let l1 = verify_with(&ring, &t1, &VerifyOptions::at(1_000)).unwrap();
        assert_eq!(l1.claims.v, 0);
        assert!(l1.token_id.starts_with("sha256:"));

        let t2 = issue_v2(&SEED, &v2(&["desktop", "station"])).unwrap();
        let l2 = verify_with(&ring, &t2, &VerifyOptions::at(1_000)).unwrap();
        assert_eq!(l2.claims.v, 2);
        assert_eq!(l2.claims.seats, 5);
        assert_eq!(l2.claims.stations, 3);
        assert_eq!(l2.kid, key_id(&public()));
        assert_eq!(l2.claims.kid.as_deref(), Some(l2.kid.as_str()));
        assert!(l2.token_id.starts_with("kt_"));
        // The old single-key entry point takes v2 tokens too.
        assert_eq!(verify(&public(), &t2, 1_000).unwrap().tier, Tier::Pro);
    }

    #[test]
    fn v1_claims_serialise_exactly_as_before() {
        // A claims value with no v2 field set must keep the v1 wire form, so tokens
        // issued by older builds and by this one for v1 are the same bytes.
        let json = serde_json::to_string(&claims("pro", 7)).unwrap();
        assert_eq!(
            json,
            r#"{"licensee":"ACME Space","tier":"pro","expires_unix":7,"features":[]}"#
        );
    }

    #[test]
    fn product_claim_is_enforced() {
        let ring = Keyring::single(public());
        let t = issue_v2(&SEED, &v2(&["desktop"])).unwrap();
        let mut o = VerifyOptions::at(1_000);
        o.product = Some(Product::Desktop);
        assert!(verify_with(&ring, &t, &o).is_ok());
        o.product = Some(Product::Station);
        let e = verify_with(&ring, &t, &o).unwrap_err().to_string();
        assert!(e.contains("does not cover station"), "{e}");
        // A v1 token (no products) covers every product.
        let t1 = issue(&SEED, &claims("pro", 0)).unwrap();
        assert!(verify_with(&ring, &t1, &o).is_ok());
        // A v2 token must name a product.
        assert!(issue_v2(&SEED, &v2(&[])).is_err());
        assert!(issue_v2(&SEED, &v2(&["toaster"])).is_err());
    }

    #[test]
    fn not_before_is_enforced() {
        let ring = Keyring::single(public());
        let mut c = v2(&["cli"]);
        c.not_before_unix = 2_000;
        let t = issue_v2(&SEED, &c).unwrap();
        assert!(verify_with(&ring, &t, &VerifyOptions::at(1_999)).is_err());
        assert!(verify_with(&ring, &t, &VerifyOptions::at(2_000)).is_ok());
    }

    #[test]
    fn machine_binding_is_enforced() {
        let ring = Keyring::single(public());
        let here = machine::hash_machine_id("machine-a");
        let mut c = v2(&["cli"]);
        c.machine = Some(here.clone());
        let t = issue_v2(&SEED, &c).unwrap();
        let same = || Some(machine::hash_machine_id("MACHINE-A"));
        let other = || Some(machine::hash_machine_id("machine-b"));
        let unknown = || None;
        let mut o = VerifyOptions::at(1_000);
        o.machine = Some(&same);
        assert!(verify_with(&ring, &t, &o).is_ok());
        o.machine = Some(&other);
        assert!(verify_with(&ring, &t, &o).is_err());
        o.machine = Some(&unknown);
        assert!(verify_with(&ring, &t, &o).is_err());
        // No machine function at all: a bound token fails closed.
        assert!(verify_with(&ring, &t, &VerifyOptions::at(1_000)).is_err());
        // A malformed binding is refused at issue.
        c.machine = Some("abc".into());
        assert!(issue_v2(&SEED, &c).is_err());
    }

    #[test]
    fn key_rotation_and_retired_key() {
        const NEW: [u8; KEY_LEN] = [8; KEY_LEN];
        let mut ring = Keyring::single(public());
        ring.add(public_key_for(&NEW));
        let old_v2 = issue_v2(&SEED, &v2(&["cli"])).unwrap();
        let old_v1 = issue(&SEED, &claims("pro", 0)).unwrap();
        let new_v2 = issue_v2(&NEW, &v2(&["cli"])).unwrap();
        // Both keys active: everything verifies, each against its own key.
        assert_eq!(
            verify_with(&ring, &new_v2, &VerifyOptions::at(1))
                .unwrap()
                .kid,
            key_id(&public_key_for(&NEW))
        );
        assert!(verify_with(&ring, &old_v2, &VerifyOptions::at(1)).is_ok());
        assert!(verify_with(&ring, &old_v1, &VerifyOptions::at(1)).is_ok());
        // Retire the old key: its v2 and v1 tokens fail; the new key's still verify.
        ring.retire(&key_id(&public()));
        let e = verify_with(&ring, &old_v2, &VerifyOptions::at(1))
            .unwrap_err()
            .to_string();
        assert!(e.contains("retired key"), "{e}");
        assert!(verify_with(&ring, &old_v1, &VerifyOptions::at(1)).is_err());
        assert!(verify_with(&ring, &new_v2, &VerifyOptions::at(1)).is_ok());
        assert_eq!(ring.active_ids(), vec![key_id(&public_key_for(&NEW))]);
    }

    #[test]
    fn kid_cannot_be_swapped_to_another_key() {
        // A token whose kid is edited to name another trusted key fails: the kid is
        // inside the signed payload.
        const NEW: [u8; KEY_LEN] = [8; KEY_LEN];
        let mut ring = Keyring::single(public());
        ring.add(public_key_for(&NEW));
        let t = issue_v2(&SEED, &v2(&["cli"])).unwrap();
        let (p, sig) = t.split_once('.').unwrap();
        let json = String::from_utf8(hex::decode(p).unwrap()).unwrap();
        let swapped = json.replace(&key_id(&public()), &key_id(&public_key_for(&NEW)));
        let forged = format!("{}.{sig}", hex::encode(swapped));
        assert!(verify_with(&ring, &forged, &VerifyOptions::at(1)).is_err());
        // A kid nobody knows is refused by name.
        let other = issue_v2(&[1; KEY_LEN], &v2(&["cli"])).unwrap();
        let e = verify_with(&ring, &other, &VerifyOptions::at(1))
            .unwrap_err()
            .to_string();
        assert!(e.contains("does not know"), "{e}");
    }

    #[test]
    fn keyring_parses_several_keys() {
        let list = format!(
            "{}, {}",
            hex::encode(public()),
            hex::encode(public_key_for(&[8; KEY_LEN]))
        );
        let ring = Keyring::parse_keys(&list).unwrap();
        assert_eq!(ring.active_ids().len(), 2);
        assert!(Keyring::parse_keys("zz").is_err());
    }

    #[test]
    fn newer_claims_version_is_refused() {
        let ring = Keyring::single(public());
        let mut c = v2(&["cli"]);
        c.v = 3;
        let t = issue(&SEED, &c).unwrap();
        assert!(verify_with(&ring, &t, &VerifyOptions::at(1)).is_err());
    }

    // ---- F10f: every Pro feature has an explicit tier ----

    #[test]
    fn every_pro_kind_and_command_has_an_explicit_tier() {
        let mut missing: Vec<String> = crate::registry::pro_kinds()
            .iter()
            .map(|m| m.name.to_string())
            .chain(pro_command_features().iter().map(|s| s.to_string()))
            .filter(|k| explicit_tier(k).is_none())
            .collect();
        missing.sort();
        assert!(
            missing.is_empty(),
            "Pro features with no explicit tier (they would fall through to Enterprise): {missing:?}"
        );
        // Every Pro kind is named pro-*, so the open/Pro split in `allows` holds.
        for m in crate::registry::pro_kinds() {
            assert!(m.name.starts_with("pro-"), "{}", m.name);
        }
    }

    #[test]
    fn edition_matrix_pro_features_are_not_free() {
        // 0.41 edition matrix (F10a): these are Pro, never Evaluation.
        for f in [
            "pro-station-fleet",
            "pro-service-ha",
            "pro-signed-dossier",
            "pro-desktop",
        ] {
            assert!(feature_min_tier(f) > Tier::Evaluation, "{f}");
        }
    }
}
