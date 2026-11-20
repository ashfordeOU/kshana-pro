// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Programme licences (backlog E39 B3902; docs/PROGRAMME-LICENCES.md)
//!
//! Four licence kinds that grant Pro in return for something other than a subscription.
//! Each is ordinary signed licence data (a `programme` block in a v3 token) plus
//! validation; none adds a new way to unlock a feature.
//!
//! | Kind       | For                                         | Highest tier | Gate                    |
//! |------------|---------------------------------------------|--------------|-------------------------|
//! | `feeder`   | a person whose station feeds observations   | `pro`        | signed feed receipt     |
//! | `host`     | a site hosting one or more stations         | `pro`        | signed feed receipt     |
//! | `teaching` | a course at a named institution             | `pro`        | none (time-limited)     |
//! | `bench`    | a lab bench running Pro bench drivers       | `enterprise` | key binding (TPM or software) |
//!
//! The highest tier, the products, the longest life and the seat rule of each kind are
//! written once, in [`POLICIES`] (the explicit tier entries; a test checks every kind has
//! one). A token whose programme breaks its policy is refused at verification, so an
//! issuing mistake cannot turn a free programme licence into an Enterprise one.
//!
//! **Fail closed.** [`super::verify_with`] returns a *gated* programme token (feeder,
//! host, bench) with an Evaluation entitlement. Only [`feed_entitlement`] (a fresh feed
//! receipt signed by a licence key) or [`bench_entitlement`] (a challenge signed by the
//! bound key) returns the entitlement the token names. Every caller that does not know
//! about programmes therefore sees Evaluation, never more.
//!
//! **Feed receipts.** The vendor's ingest service signs, per station, the time it last
//! received valid observations ([`FeedReceipt`]). Pro holds while the newest receipt for a
//! listed station is no older than [`FEED_GRACE_SECS`] (48 hours) since that last
//! observation; after that the licence drops to Evaluation until a new receipt arrives.
//! The receipt is signed with the licence key, so it cannot be made up locally.
//!
//! **Bench binding.** A bench token names the SHA-256 of a public key ([`bench_key_id`])
//! and the weakest binding class it accepts (`tpm` or `software`). At run time a
//! [`BenchBinder`] signs a fresh challenge with the private key and the signature is
//! checked against the named key. [`SoftwareBinder`] keeps the private key in a file: it
//! is **weaker** (the file can be copied to another machine, which a TPM-held key cannot)
//! and the docs and messages say so. This crate contains **no TPM binder**: the interface
//! takes one (P-256 ECDSA, the curve every TPM 2.0 supports), but until one is built and
//! tested on hardware a `tpm`-class token cannot be satisfied by this build.

use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};
use kshana::api::KshanaError;
use sha2::{Digest, Sha256};

use super::signer::{sign_document, LicenceSigner};
use super::{check_signature, split_signed, Entitlement, Keyring, Licence, LicenseClaims, Tier};

/// The claims version a programme token carries. A build that reads only v2 refuses it
/// ("newer than this build reads"), so an older build can never run a programme token as
/// an ordinary licence without its programme rules.
pub const PROGRAMME_CLAIMS_VERSION: u8 = 3;

/// How long Pro holds after a station's last observation (48 hours, research 09 A4).
pub const FEED_GRACE_SECS: i64 = 48 * 3_600;

/// The tolerance on a receipt issued in the future (clock skew between the ingest service
/// and this machine).
pub const RECEIPT_SKEW_SECS: i64 = 300;

/// Grants that no programme token may carry: the on-premises contract grant of the
/// node-locked methods, synthetic attack IQ and the OEM SDK.
pub const FORBIDDEN_GRANTS: &[&str] = &["onprem-contract", "pro-attack-iq", "pro-ks-inside"];

/// A programme licence kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Feeder,
    Host,
    Teaching,
    Bench,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Feeder, Kind::Host, Kind::Teaching, Kind::Bench];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Feeder => "feeder",
            Kind::Host => "host",
            Kind::Teaching => "teaching",
            Kind::Bench => "bench",
        }
    }

    pub fn parse(s: &str) -> Result<Kind, KshanaError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "feeder" => Ok(Kind::Feeder),
            "host" => Ok(Kind::Host),
            "teaching" => Ok(Kind::Teaching),
            "bench" => Ok(Kind::Bench),
            other => Err(KshanaError::InvalidInput(format!(
                "unknown programme '{other}' (expected feeder|host|teaching|bench)"
            ))),
        }
    }

    /// The policy of this kind (every kind has one).
    pub fn policy(self) -> &'static Policy {
        POLICIES
            .iter()
            .find(|p| p.kind == self)
            .expect("every programme kind has a policy")
    }
}

/// What must hold before a programme token's entitlement is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// None beyond the token itself.
    None,
    /// A fresh signed feed receipt for one of the listed stations.
    Feed,
    /// A challenge signed by the bound bench key.
    Bench,
}

/// How many seats a programme token may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seats {
    /// At most this many (0 in the token means "not counted").
    AtMost(u32),
    /// At most one per listed station (host rewards: one seat per active station).
    PerStation,
    /// Not limited by the programme (a course's class size is the vendor's call).
    Any,
}

/// The explicit rules of one programme kind.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub kind: Kind,
    /// The highest licence tier the token may name; grants above it are refused too.
    pub max_tier: Tier,
    /// The products the token may name (it must name at least one).
    pub products: &'static [&'static str],
    /// The longest life from not-before (or issue) to expiry, in days; `None`: perpetual
    /// tokens allowed.
    pub max_days: Option<i64>,
    pub seats: Seats,
    pub needs_stations: bool,
    pub needs_course: bool,
    pub needs_bench_key: bool,
    pub gate: Gate,
}

/// The explicit tier entries of the programme kinds. Owner decisions live here: changing a
/// kind's highest tier, products or life is a one-line, reviewable change.
pub const POLICIES: &[Policy] = &[
    Policy {
        kind: Kind::Feeder,
        max_tier: Tier::Pro,
        products: &["cli", "desktop", "station"],
        max_days: Some(366),
        seats: Seats::AtMost(1),
        needs_stations: true,
        needs_course: false,
        needs_bench_key: false,
        gate: Gate::Feed,
    },
    Policy {
        kind: Kind::Host,
        max_tier: Tier::Pro,
        products: &["cli", "desktop", "station"],
        max_days: Some(366),
        seats: Seats::PerStation,
        needs_stations: true,
        needs_course: false,
        needs_bench_key: false,
        gate: Gate::Feed,
    },
    Policy {
        kind: Kind::Teaching,
        max_tier: Tier::Pro,
        products: &["cli", "desktop"],
        max_days: Some(366),
        seats: Seats::Any,
        needs_stations: false,
        needs_course: true,
        needs_bench_key: false,
        gate: Gate::None,
    },
    Policy {
        kind: Kind::Bench,
        max_tier: Tier::Enterprise,
        products: &["cli", "station"],
        max_days: None,
        seats: Seats::AtMost(1),
        needs_stations: false,
        needs_course: false,
        needs_bench_key: true,
        gate: Gate::Bench,
    },
];

/// The `programme` block of a v3 token.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Programme {
    /// `feeder`, `host`, `teaching` or `bench`.
    pub kind: String,
    /// Feeder and host: the station IDs whose feed keeps the licence active.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stations: Vec<String>,
    /// Teaching: the institution and the course.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub institution: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub course: String,
    /// Bench: the bound key ([`bench_key_id`]), `<algorithm>:<64 hex>`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bench_key: String,
    /// Bench: the weakest binding class accepted, `tpm` or `software`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bench_class: String,
}

fn invalid(msg: String) -> KshanaError {
    KshanaError::InvalidInput(format!("programme licence: {msg}"))
}

/// Check a token's programme against its policy. Called by [`super::verify_with`] on every
/// v3 token and by [`super::issue_v2`] before signing.
pub fn validate(claims: &LicenseClaims) -> Result<Kind, KshanaError> {
    let p = claims
        .programme
        .as_ref()
        .ok_or_else(|| invalid("a v3 token must carry a programme block".into()))?;
    let kind = Kind::parse(&p.kind)?;
    let pol = kind.policy();
    let tier = Tier::parse(&claims.tier)?;
    if tier > pol.max_tier {
        return Err(invalid(format!(
            "a {} licence is at most '{}', the token names '{}'",
            kind.as_str(),
            pol.max_tier.as_str(),
            tier.as_str()
        )));
    }
    if claims.arksa || claims.vetted_buyer || claims.trial {
        return Err(invalid(
            "a programme token carries no special entitlement, vetting or trial flag".into(),
        ));
    }
    for f in &claims.features {
        if FORBIDDEN_GRANTS.contains(&f.as_str()) {
            return Err(invalid(format!(
                "the grant '{f}' is never part of a programme licence"
            )));
        }
        match super::explicit_tier(f) {
            Some(t) if t <= pol.max_tier => {}
            _ => {
                return Err(invalid(format!(
                    "the grant '{f}' is above what a {} licence may carry ('{}')",
                    kind.as_str(),
                    pol.max_tier.as_str()
                )))
            }
        }
    }
    if claims.products.is_empty() {
        return Err(invalid("the token must name its products".into()));
    }
    if let Some(bad) = claims
        .products
        .iter()
        .find(|c| !pol.products.contains(&c.as_str()))
    {
        return Err(invalid(format!(
            "a {} licence covers {} only, not '{bad}'",
            kind.as_str(),
            pol.products.join(", ")
        )));
    }
    if let Some(days) = pol.max_days {
        let from = claims.not_before_unix.max(claims.issued_unix);
        if claims.expires_unix == 0 || from == 0 || claims.expires_unix - from > days * 86_400 {
            return Err(invalid(format!(
                "a {} licence must expire within {days} days of issue",
                kind.as_str()
            )));
        }
    }
    let has_stations = !p.stations.is_empty();
    if pol.needs_stations != has_stations {
        return Err(invalid(if pol.needs_stations {
            format!("a {} licence lists at least one station", kind.as_str())
        } else {
            format!("a {} licence lists no stations", kind.as_str())
        }));
    }
    if p.stations
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 128)
    {
        return Err(invalid("station IDs are 1 to 128 characters".into()));
    }
    if claims.stations as usize > p.stations.len() {
        return Err(invalid(
            "the stations claim may not exceed the stations the programme lists".into(),
        ));
    }
    let course = !p.institution.trim().is_empty() && !p.course.trim().is_empty();
    if pol.needs_course && !course {
        return Err(invalid(
            "a teaching licence names the institution and the course".into(),
        ));
    }
    if !pol.needs_course && (!p.institution.is_empty() || !p.course.is_empty()) {
        return Err(invalid(format!(
            "a {} licence names no institution or course",
            kind.as_str()
        )));
    }
    if pol.needs_bench_key {
        parse_bench_key(&p.bench_key)?;
        BindingClass::parse(&p.bench_class)?;
    } else if !p.bench_key.is_empty() || !p.bench_class.is_empty() {
        return Err(invalid(format!(
            "a {} licence binds no bench key",
            kind.as_str()
        )));
    }
    let seat_cap = match pol.seats {
        Seats::AtMost(n) => Some(n),
        Seats::PerStation => Some(p.stations.len() as u32),
        Seats::Any => None,
    };
    if let Some(cap) = seat_cap {
        if claims.seats > cap {
            return Err(invalid(format!(
                "a {} licence carries at most {cap} seat(s)",
                kind.as_str()
            )));
        }
    }
    Ok(kind)
}

/// The entitlement a programme token names, after its gate has held.
fn named_entitlement(lic: &Licence, now: i64) -> Result<Entitlement, KshanaError> {
    let ent = Entitlement {
        licensee: lic.claims.licensee.clone(),
        tier: Tier::parse(&lic.claims.tier)?,
        expires_unix: lic.claims.expires_unix,
        features: lic.claims.features.clone(),
        arksa: false,
    };
    if ent.is_expired(now) {
        return Err(invalid("the licence has expired".into()));
    }
    Ok(ent)
}

/// The gated entitlement verification returns for a feeder, host or bench token until
/// its gate holds: Evaluation, with the licensee and expiry kept for display.
pub(crate) fn pending_entitlement(claims: &LicenseClaims) -> Entitlement {
    Entitlement {
        licensee: claims.licensee.clone(),
        tier: Tier::Evaluation,
        expires_unix: claims.expires_unix,
        features: Vec::new(),
        arksa: false,
    }
}

/// The programme kind and its gate, for a verified licence.
pub fn kind_of(lic: &Licence) -> Option<Kind> {
    lic.claims
        .programme
        .as_ref()
        .and_then(|p| Kind::parse(&p.kind).ok())
}

/// A token whose entitlement needs a gate (feed receipt or bench key) before it is used.
pub fn is_gated(claims: &LicenseClaims) -> bool {
    claims
        .programme
        .as_ref()
        .and_then(|p| Kind::parse(&p.kind).ok())
        .is_some_and(|k| k.policy().gate != Gate::None)
}

// ---------------------------------------------------------------------------------------
// Feed receipts (feeder and host)
// ---------------------------------------------------------------------------------------

/// The `format` of a feed receipt.
pub const RECEIPT_FORMAT: &str = "kshana-pro-feed-receipt/1";

/// The ingest service's signed statement that a station fed valid observations.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedReceipt {
    pub format: String,
    /// The token ID of the programme licence ([`super::token_id`]).
    pub licence: String,
    pub station: String,
    /// Unix seconds of the newest valid observation the service received.
    pub last_obs_unix: i64,
    pub issued_unix: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,
}

/// Sign a feed receipt (the vendor's ingest service; `license feed-receipt issue`).
pub fn issue_receipt(
    signer: &dyn LicenceSigner,
    licence_id: &str,
    station: &str,
    last_obs_unix: i64,
    now: i64,
) -> Result<String, KshanaError> {
    if licence_id.trim().is_empty() || station.trim().is_empty() {
        return Err(invalid(
            "a receipt names the licence and the station".into(),
        ));
    }
    if last_obs_unix > now {
        return Err(invalid(
            "the last observation is after the receipt's issue time".into(),
        ));
    }
    let r = FeedReceipt {
        format: RECEIPT_FORMAT.into(),
        licence: licence_id.trim().into(),
        station: station.trim().into(),
        last_obs_unix,
        issued_unix: now,
        kid: Some(signer.kid()?),
    };
    let payload = serde_json::to_vec(&r).map_err(|e| invalid(format!("receipt: {e}")))?;
    sign_document(signer, &payload)
}

/// Verify a receipt's signature against the licence keys.
pub fn verify_receipt(ring: &Keyring, text: &str) -> Result<FeedReceipt, KshanaError> {
    let (payload, sig) = split_signed(text).map_err(|e| invalid(format!("feed receipt: {e}")))?;
    let r: FeedReceipt = serde_json::from_slice(&payload)
        .map_err(|e| invalid(format!("not a feed receipt: {e}")))?;
    check_signature(ring, r.kid.as_deref(), &payload, &sig)
        .map_err(|e| invalid(format!("feed receipt: {e}")))?;
    if r.format != RECEIPT_FORMAT {
        return Err(invalid(format!("unknown receipt format '{}'", r.format)));
    }
    Ok(r)
}

/// What a feed-gated licence grants now.
#[derive(Debug, Clone)]
pub struct FeedGrant {
    pub entitlement: Entitlement,
    /// The listed stations with a fresh receipt (for a host: the seats in use may not
    /// exceed this).
    pub active_stations: Vec<String>,
    /// When Pro drops to Evaluation unless a newer receipt arrives.
    pub holds_until_unix: i64,
}

/// The entitlement of a feeder or host licence given the receipts at hand (any order; bad,
/// foreign and stale receipts are skipped). Fails, with the reason, when no listed station
/// has a receipt younger than [`FEED_GRACE_SECS`] since its last observation.
pub fn feed_entitlement(
    ring: &Keyring,
    lic: &Licence,
    receipts: &[&str],
    now: i64,
) -> Result<FeedGrant, KshanaError> {
    let p = lic
        .claims
        .programme
        .as_ref()
        .ok_or_else(|| invalid("not a programme licence".into()))?;
    let kind = Kind::parse(&p.kind)?;
    if kind.policy().gate != Gate::Feed {
        return Err(invalid(format!(
            "a {} licence is not feed-gated",
            kind.as_str()
        )));
    }
    let mut newest: std::collections::BTreeMap<String, i64> = Default::default();
    let mut why = if receipts.is_empty() {
        "no feed receipt (install one with `kshana-pro license feed-receipt install`)".to_string()
    } else {
        String::new()
    };
    for text in receipts {
        let r = match verify_receipt(ring, text) {
            Ok(r) => r,
            Err(e) => {
                why = e.to_string();
                continue;
            }
        };
        if r.licence != lic.token_id {
            why = "the feed receipt is for another licence".into();
            continue;
        }
        if !p.stations.contains(&r.station) {
            why = format!("station '{}' is not one this licence lists", r.station);
            continue;
        }
        if r.issued_unix > now + RECEIPT_SKEW_SECS || r.last_obs_unix > r.issued_unix {
            why = "the feed receipt is dated in the future".into();
            continue;
        }
        let e = newest.entry(r.station).or_insert(i64::MIN);
        *e = (*e).max(r.last_obs_unix);
    }
    if !newest.is_empty() {
        why = "the feed stopped more than 48 hours ago".into();
    }
    let fresh: Vec<(String, i64)> = newest
        .into_iter()
        .filter(|(_, t)| now - t <= FEED_GRACE_SECS)
        .collect();
    if fresh.is_empty() {
        return Err(invalid(format!(
            "{} licence at Evaluation tier: {why}",
            kind.as_str()
        )));
    }
    let holds_until_unix = fresh
        .iter()
        .map(|(_, t)| t + FEED_GRACE_SECS)
        .max()
        .unwrap_or(now);
    Ok(FeedGrant {
        entitlement: named_entitlement(lic, now)?,
        active_stations: fresh.into_iter().map(|(s, _)| s).collect(),
        holds_until_unix,
    })
}

// ---------------------------------------------------------------------------------------
// Bench binding (TPM interface, software fallback)
// ---------------------------------------------------------------------------------------

/// How a bench key is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindingClass {
    /// A private key in a file. Weaker: the file can be copied to another machine.
    Software,
    /// A non-exportable key inside a TPM 2.0. No binder of this class is in this build.
    Tpm,
}

impl BindingClass {
    pub fn as_str(self) -> &'static str {
        match self {
            BindingClass::Software => "software",
            BindingClass::Tpm => "tpm",
        }
    }
    pub fn parse(s: &str) -> Result<BindingClass, KshanaError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "software" => Ok(BindingClass::Software),
            "tpm" => Ok(BindingClass::Tpm),
            other => Err(invalid(format!(
                "unknown bench binding class '{other}' (expected tpm|software)"
            ))),
        }
    }
}

/// The signature algorithms a bench key may use: Ed25519 (the software binder) and P-256
/// ECDSA with SHA-256 (what a TPM 2.0 signs with).
pub const BENCH_ALGORITHMS: &[&str] = &["ed25519", "p256"];

/// Something holding a bench key. A TPM-backed implementation keeps the key inside the TPM
/// and reports [`BindingClass::Tpm`]; none is part of this build.
pub trait BenchBinder {
    fn class(&self) -> BindingClass;
    /// `ed25519` or `p256`.
    fn algorithm(&self) -> &'static str;
    /// Ed25519: the 32-byte key. P-256: the SEC1 encoding.
    fn public_key(&self) -> Result<Vec<u8>, KshanaError>;
    /// Ed25519: 64-byte signature. P-256: the 64-byte r‖s signature over SHA-256(msg).
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, KshanaError>;
}

/// The ID a bench token names: `<algorithm>:<SHA-256 of the public key>`.
pub fn bench_key_id(algorithm: &str, public_key: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"kshana-bench-key-v1:");
    h.update(public_key);
    format!("{algorithm}:{}", hex::encode(h.finalize()))
}

fn parse_bench_key(id: &str) -> Result<(), KshanaError> {
    let (alg, hash) = id
        .split_once(':')
        .ok_or_else(|| invalid("a bench licence names its key as <algorithm>:<64 hex>".into()))?;
    if !BENCH_ALGORITHMS.contains(&alg) {
        return Err(invalid(format!("unknown bench key algorithm '{alg}'")));
    }
    if hash.len() != 64 || hex::decode(hash).is_err() {
        return Err(invalid(
            "the bench key hash must be 64 hex characters".into(),
        ));
    }
    Ok(())
}

fn verify_bench_signature(alg: &str, pk: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    match alg {
        "ed25519" => {
            let (Ok(pk), Ok(sig)) = (<[u8; 32]>::try_from(pk), <[u8; 64]>::try_from(sig)) else {
                return false;
            };
            VerifyingKey::from_bytes(&pk)
                .map(|k| {
                    k.verify_strict(msg, &ed25519_dalek::Signature::from_bytes(&sig))
                        .is_ok()
                })
                .unwrap_or(false)
        }
        "p256" => {
            use p256::ecdsa::signature::Verifier as _;
            let (Ok(key), Ok(sig)) = (
                p256::ecdsa::VerifyingKey::from_sec1_bytes(pk),
                p256::ecdsa::Signature::from_slice(sig),
            ) else {
                return false;
            };
            key.verify(msg, &sig).is_ok()
        }
        _ => false,
    }
}

/// A fresh challenge for the binder: bound to the token, the time and 16 random bytes, so
/// a recorded answer cannot be replayed.
fn challenge(token_id: &str, now: i64) -> Vec<u8> {
    use rand::RngCore;
    let mut nonce = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let mut m = b"kshana-bench-challenge-v1:".to_vec();
    m.extend_from_slice(token_id.as_bytes());
    m.push(b':');
    m.extend_from_slice(&now.to_be_bytes());
    m.extend_from_slice(&nonce);
    m
}

/// The entitlement of a bench licence, once `binder` proves it holds the bound key and is
/// of the class the token requires. A software binder never satisfies a `tpm` token.
pub fn bench_entitlement(
    lic: &Licence,
    binder: &dyn BenchBinder,
    now: i64,
) -> Result<Entitlement, KshanaError> {
    let p = lic
        .claims
        .programme
        .as_ref()
        .ok_or_else(|| invalid("not a programme licence".into()))?;
    if Kind::parse(&p.kind)? != Kind::Bench {
        return Err(invalid(format!("a {} licence binds no bench key", p.kind)));
    }
    let need = BindingClass::parse(&p.bench_class)?;
    if binder.class() < need {
        return Err(invalid(format!(
            "this bench licence requires a '{}' key; the key here is '{}' (no TPM binder is part of this build)",
            need.as_str(),
            binder.class().as_str()
        )));
    }
    let pk = binder.public_key()?;
    if bench_key_id(binder.algorithm(), &pk) != p.bench_key {
        return Err(invalid(
            "the bench key here is not the one this licence is bound to (compare `kshana-pro license bench-key show`)"
                .into(),
        ));
    }
    let msg = challenge(&lic.token_id, now);
    let sig = binder.sign(&msg)?;
    if !verify_bench_signature(binder.algorithm(), &pk, &msg, &sig) {
        return Err(invalid("the bench key did not sign the challenge".into()));
    }
    named_entitlement(lic, now)
}

/// The software fallback binder: an Ed25519 key in a file (mode 0600 on Unix).
///
/// **Weaker than a TPM.** Anyone who can read the file can copy the binding to another
/// machine. Use it where a TPM is not available and the vendor accepts `software` class.
pub struct SoftwareBinder {
    seed: [u8; 32],
}

impl SoftwareBinder {
    pub fn from_seed(seed: [u8; 32]) -> SoftwareBinder {
        SoftwareBinder { seed }
    }

    /// Load the key file, or create it with a fresh random key when absent.
    pub fn load_or_create(path: &std::path::Path) -> Result<SoftwareBinder, KshanaError> {
        if path.exists() {
            return Self::load(path);
        }
        use rand::RngCore;
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| invalid(format!("{}: {e}", d.display())))?;
        }
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        use std::io::Write as _;
        o.open(path)
            .and_then(|mut f| f.write_all(format!("{}\n", hex::encode(seed)).as_bytes()))
            .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
        Ok(SoftwareBinder { seed })
    }

    /// Load an existing key file.
    pub fn load(path: &std::path::Path) -> Result<SoftwareBinder, KshanaError> {
        let t = std::fs::read_to_string(path)
            .map_err(|e| invalid(format!("bench key {}: {e}", path.display())))?;
        Ok(SoftwareBinder {
            seed: super::parse_key_hex(&t)?,
        })
    }

    /// The key ID a vendor puts in the token.
    pub fn key_id(&self) -> String {
        bench_key_id(
            "ed25519",
            &SigningKey::from_bytes(&self.seed)
                .verifying_key()
                .to_bytes(),
        )
    }
}

impl BenchBinder for SoftwareBinder {
    fn class(&self) -> BindingClass {
        BindingClass::Software
    }
    fn algorithm(&self) -> &'static str {
        "ed25519"
    }
    fn public_key(&self) -> Result<Vec<u8>, KshanaError> {
        Ok(SigningKey::from_bytes(&self.seed)
            .verifying_key()
            .to_bytes()
            .to_vec())
    }
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, KshanaError> {
        Ok(SigningKey::from_bytes(&self.seed)
            .sign(msg)
            .to_bytes()
            .to_vec())
    }
}

// ---------------------------------------------------------------------------------------
// Telemetry opt-in (simulator vendors, research 09 A38)
// ---------------------------------------------------------------------------------------

/// The `format` of a telemetry record.
pub const TELEMETRY_FORMAT: &str = "kshana-pro-telemetry/1";

/// One anonymised scenario outcome. It carries no licensee, no machine, no input values and
/// no times finer than the week: the scenario kind, the outcome word and a duration
/// bucket only.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryRecord {
    pub format: String,
    pub kind: String,
    pub outcome: String,
    /// `<1s`, `1-10s`, `10-60s`, `1-10min` or `>10min`.
    pub duration: String,
    /// Weeks since the Unix epoch.
    pub week: i64,
    pub build: String,
}

/// The anonymised record of one outcome, or `None` when the licence did not opt in (the
/// `telemetry_opt_in` claim). Nothing is sent anywhere: the caller writes the record to a
/// local outbox ([`append_outbox`]) the holder reviews and shares under their agreement.
pub fn telemetry_record(
    claims: &LicenseClaims,
    kind: &str,
    outcome: &str,
    duration_s: f64,
    now: i64,
) -> Option<TelemetryRecord> {
    if !claims.telemetry_opt_in {
        return None;
    }
    let word = |s: &str| {
        let s: String = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .take(48)
            .collect();
        if s.is_empty() {
            "unknown".to_string()
        } else {
            s.to_ascii_lowercase()
        }
    };
    let duration = match duration_s {
        d if d < 1.0 => "<1s",
        d if d < 10.0 => "1-10s",
        d if d < 60.0 => "10-60s",
        d if d < 600.0 => "1-10min",
        _ => ">10min",
    };
    Some(TelemetryRecord {
        format: TELEMETRY_FORMAT.into(),
        kind: word(kind),
        outcome: word(outcome),
        duration: duration.into(),
        week: now.div_euclid(7 * 86_400),
        build: env!("CARGO_PKG_VERSION").into(),
    })
}

/// Append a record to the outbox file (JSON Lines).
pub fn append_outbox(path: &std::path::Path, r: &TelemetryRecord) -> Result<(), KshanaError> {
    use std::io::Write as _;
    let line = serde_json::to_string(r).map_err(|e| invalid(format!("telemetry: {e}")))?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| writeln!(f, "{line}"))
        .map_err(|e| invalid(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_exactly_one_policy() {
        for k in Kind::ALL {
            assert_eq!(POLICIES.iter().filter(|p| p.kind == k).count(), 1, "{k:?}");
            assert_eq!(Kind::parse(k.as_str()).unwrap(), k);
            for p in k.policy().products {
                super::super::Product::parse(p).unwrap();
            }
        }
        assert_eq!(POLICIES.len(), Kind::ALL.len());
    }

    #[test]
    fn telemetry_is_off_unless_opted_in_and_carries_no_identity() {
        let mut c = LicenseClaims {
            licensee: "Sim Vendor GmbH".into(),
            tier: "pro".into(),
            ..Default::default()
        };
        assert!(telemetry_record(&c, "pro-twin", "pass", 3.0, 1_800_000_000).is_none());
        c.telemetry_opt_in = true;
        let r = telemetry_record(&c, "pro-twin", "Pass; rm -rf", 3.0, 1_800_000_000).unwrap();
        assert_eq!(r.outcome, "passrm-rf");
        assert_eq!(r.duration, "1-10s");
        let text = serde_json::to_string(&r).unwrap();
        assert!(!text.contains("Sim Vendor"));
    }
}
