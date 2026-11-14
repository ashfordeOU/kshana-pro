// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Edition tiers and the signed tier manifest (roadmap 0.42, items X1 and X2)
//!
//! Every Pro scenario kind and Pro command feature belongs to one **edition tier**:
//!
//! | Tier        | Who runs it                                                        |
//! |-------------|--------------------------------------------------------------------|
//! | `open-seam` | anyone, no licence (the free seam check)                           |
//! | `pro`       | a Pro licence at the licence tier [`super::explicit_tier`] names    |
//! | `arksa`     | only a token carrying the `arksa` claim, signed by Arksa's issuer  |
//! | `core`      | nobody: Core never ships in this crate                             |
//!
//! The tier of each kind is written in a **signed manifest**
//! (`commercial/tier-manifest.json`, Ed25519 signature in `tier-manifest.json.sig`),
//! embedded in the build and verified before any Pro kind runs. A kind the manifest
//! does not list is refused. Arksa-exclusive and Core entries are listed by the SHA-256
//! of their name only, so a build that does not contain them (the default build, Desktop
//! Pro) does not carry their names either; it still refuses them by hash.
//!
//! The manifest is embedded at compile time, so its signature guards the file between
//! the vendor and the build (an edit to the repository copy without the signing key is
//! caught at start-up and in CI). It is not a defence against someone who patches a
//! built binary; the defence there is that Arksa-exclusive code is not in the binary at
//! all (cargo feature `arksa`, off by default; see docs/PROTECTION.md).

use std::sync::OnceLock;

use ed25519_dalek::{Signature, VerifyingKey};
use kshana::api::KshanaError;
use sha2::{Digest, Sha256};

use super::KEY_LEN;

/// The edition tier of a Pro kind or feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Edition {
    /// Free: runs with no licence.
    OpenSeam,
    /// Sold to anyone holding a Pro licence of the right licence tier.
    Pro,
    /// Runs only under a token with the `arksa` claim from Arksa's issuer.
    Arksa,
    /// Never runs from this crate.
    Core,
}

impl Edition {
    pub fn as_str(self) -> &'static str {
        match self {
            Edition::OpenSeam => "open-seam",
            Edition::Pro => "pro",
            Edition::Arksa => "arksa",
            Edition::Core => "core",
        }
    }
    pub fn parse(s: &str) -> Result<Edition, KshanaError> {
        match s {
            "open-seam" => Ok(Edition::OpenSeam),
            "pro" => Ok(Edition::Pro),
            "arksa" => Ok(Edition::Arksa),
            "core" => Ok(Edition::Core),
            other => Err(KshanaError::InvalidInput(format!(
                "unknown edition tier '{other}' (expected open-seam|pro|arksa|core)"
            ))),
        }
    }
    /// Whether the manifest lists this tier by hash only.
    pub fn is_hidden(self) -> bool {
        matches!(self, Edition::Arksa | Edition::Core)
    }
}

/// The refusal for an Arksa-exclusive or Core kind. It names no kind and no method.
pub const NOT_IN_THIS_EDITION: &str =
    "this capability is not available under this licence; it is provided only within Arksa";

/// Arksa-exclusive scenario kinds compiled into this build (feature `arksa` only; empty
/// otherwise, so the names are not in a default build).
#[cfg(feature = "arksa")]
pub const ARKSA_KINDS: &[&str] = &[
    "pro-mbse",
    "pro-design-optimise",
    "pro-spectrum-coexistence",
    "pro-lunar-signal-security",
    "pro-beacon-placement",
    "pro-lunar-time-budget-study",
    "pro-cislunar-observability-study",
    "pro-optical-rf-hybrid-study",
    "pro-realtime-frame-eop-study",
];
#[cfg(not(feature = "arksa"))]
pub const ARKSA_KINDS: &[&str] = &[];

/// Arksa-exclusive command features compiled into this build (feature `arksa` only). The
/// commands `mbse`, `design` and `coexist` share the feature id of their scenario kind.
#[cfg(feature = "arksa")]
pub const ARKSA_FEATURES: &[&str] = &[
    "pro-mbse",
    "pro-design-optimise",
    "pro-spectrum-coexistence",
    "pro-mission-dossier",
    "pro-navdt",
    "pro-spacestream",
    "pro-mrd",
];
#[cfg(not(feature = "arksa"))]
pub const ARKSA_FEATURES: &[&str] = &[];

/// The edition this build's code gives a Pro kind or feature, or `None` when the build
/// does not contain it.
pub fn compiled_edition(id: &str) -> Option<Edition> {
    if id == "pro-seam-check" {
        return Some(Edition::OpenSeam);
    }
    if ARKSA_KINDS.contains(&id) || ARKSA_FEATURES.contains(&id) {
        return Some(Edition::Arksa);
    }
    let is_kind = crate::registry::pro_kinds().iter().any(|m| m.name == id);
    if is_kind || super::pro_command_features().contains(&id) {
        return Some(Edition::Pro);
    }
    None
}

/// The SHA-256 of a kind name, as the manifest writes hidden entries.
pub fn name_hash(id: &str) -> String {
    hex::encode(Sha256::digest(id.as_bytes()))
}

/// One manifest entry as written in the file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestEntry {
    /// The kind or feature name (open-seam and pro tiers only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// SHA-256 of the name (arksa and core tiers only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind_sha256: Option<String>,
    /// `kind` (a scenario kind) or `feature` (a command feature).
    #[serde(rename = "type")]
    pub entry_type: String,
    /// `open-seam`, `pro`, `arksa` or `core`.
    pub tier: String,
}

/// The manifest file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestDoc {
    pub format: String,
    pub version: u32,
    pub entries: Vec<ManifestEntry>,
}

/// The `format` value of a tier manifest.
pub const MANIFEST_FORMAT: &str = "kshana-pro-tier-manifest";

/// A verified tier manifest.
#[derive(Debug, Clone)]
pub struct TierManifest {
    doc: ManifestDoc,
}

impl TierManifest {
    /// The edition of `id`, or `None` when the manifest does not list it.
    pub fn edition_of(&self, id: &str) -> Option<Edition> {
        let mut hash: Option<String> = None;
        for e in &self.doc.entries {
            let hit = match (&e.kind, &e.kind_sha256) {
                (Some(k), _) => k == id,
                (None, Some(h)) => *h == *hash.get_or_insert_with(|| name_hash(id)),
                (None, None) => false,
            };
            if hit {
                return Edition::parse(&e.tier).ok();
            }
        }
        None
    }

    /// The names listed in the clear (open-seam and pro entries).
    pub fn named(&self) -> Vec<(&str, &str, Edition)> {
        self.doc
            .entries
            .iter()
            .filter_map(|e| {
                let t = Edition::parse(&e.tier).ok()?;
                Some((e.kind.as_deref()?, e.entry_type.as_str(), t))
            })
            .collect()
    }

    /// Number of entries of type `kind` (scenario kinds), in every tier.
    pub fn kind_count(&self) -> usize {
        self.doc
            .entries
            .iter()
            .filter(|e| e.entry_type == "kind")
            .count()
    }

    /// Number of entries in `tier`.
    pub fn count_in(&self, tier: Edition) -> usize {
        self.doc
            .entries
            .iter()
            .filter(|e| e.tier == tier.as_str())
            .count()
    }

    pub fn doc(&self) -> &ManifestDoc {
        &self.doc
    }
}

/// Verify a manifest: the Ed25519 signature `sig_hex` (128 hex characters) over the exact
/// bytes `json`, under `public_key`; then its form: the format and version, one name or
/// hash per entry, names in the clear only for the open-seam and pro tiers, hashes only
/// for arksa and core, a known type, no entry twice.
pub fn verify_manifest(
    json: &[u8],
    sig_hex: &str,
    public_key: &[u8; KEY_LEN],
) -> Result<TierManifest, KshanaError> {
    let bad = |m: &str| KshanaError::InvalidInput(format!("tier manifest: {m}"));
    let sig: [u8; 64] = hex::decode(sig_hex.trim())
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| bad("signature is not 64 hex bytes"))?;
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| bad("public key is invalid"))?;
    key.verify_strict(json, &Signature::from_bytes(&sig))
        .map_err(|_| bad("signature does not verify (tampered or wrong key)"))?;
    let doc: ManifestDoc =
        serde_json::from_slice(json).map_err(|e| bad(&format!("not readable: {e}")))?;
    if doc.format != MANIFEST_FORMAT || doc.version != 1 {
        return Err(bad("unknown format or version"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for e in &doc.entries {
        let tier = Edition::parse(&e.tier)?;
        if e.entry_type != "kind" && e.entry_type != "feature" {
            return Err(bad("an entry type is neither kind nor feature"));
        }
        let key = match (&e.kind, &e.kind_sha256) {
            (Some(k), None) if !tier.is_hidden() => k.clone(),
            (None, Some(h)) if tier.is_hidden() => {
                if h.len() != 64 || hex::decode(h).is_err() {
                    return Err(bad("a hash is not 64 hex characters"));
                }
                h.clone()
            }
            _ => {
                return Err(bad(
                    "an entry is named in the clear in a hidden tier, or hashed in an open one",
                ))
            }
        };
        if !seen.insert((key, e.entry_type.clone())) {
            return Err(bad("an entry is listed twice"));
        }
    }
    Ok(TierManifest { doc })
}

/// The manifest embedded in this build.
pub const EMBEDDED_MANIFEST: &str = include_str!("../../commercial/tier-manifest.json");
/// The signature committed with it (signed with the placeholder test key).
const COMMITTED_SIGNATURE: &str = include_str!("../../commercial/tier-manifest.json.sig");

/// Public half of the placeholder manifest key: a test key whose private half exists only
/// in the tests (`tests/tier_manifest.rs`). A release signs the manifest with the vendor's
/// offline manifest key and sets `KSHANA_PRO_TIER_MANIFEST_PUBKEY` and
/// `KSHANA_PRO_TIER_MANIFEST_SIG` at build time (docs/LICENSING-KEYS.md, section 6).
pub const PLACEHOLDER_MANIFEST_PUBKEY: &str =
    "b0b2d471485aad953b4342c7c03c301984d939736437074e1a978174be44451c";

/// The manifest public key this build trusts.
pub fn manifest_public_key() -> Result<[u8; KEY_LEN], KshanaError> {
    let hex_key = option_env!("KSHANA_PRO_TIER_MANIFEST_PUBKEY")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(PLACEHOLDER_MANIFEST_PUBKEY);
    super::parse_key_hex(hex_key)
}

/// The manifest signature this build carries.
pub fn manifest_signature() -> &'static str {
    option_env!("KSHANA_PRO_TIER_MANIFEST_SIG")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(COMMITTED_SIGNATURE)
}

/// The embedded manifest, verified once per process.
pub fn manifest() -> Result<&'static TierManifest, KshanaError> {
    static M: OnceLock<Result<TierManifest, String>> = OnceLock::new();
    M.get_or_init(|| {
        let key = manifest_public_key().map_err(|e| e.to_string())?;
        verify_manifest(EMBEDDED_MANIFEST.as_bytes(), manifest_signature(), &key)
            .map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(|e| KshanaError::InvalidInput(e.clone()))
}

/// The edition the verified manifest gives `id`; `None` when the manifest does not list
/// it or does not verify (both refuse the kind).
pub fn edition_of(id: &str) -> Option<Edition> {
    manifest().ok().and_then(|m| m.edition_of(id))
}

/// The start-up check: the manifest verifies, and it agrees with the code of this build
/// (every compiled Pro kind and feature is listed at the tier the code gives it; every
/// name in the clear is in this build).
pub fn startup_check() -> Result<(), KshanaError> {
    let m = manifest()?;
    let bad = |m: String| KshanaError::InvalidInput(format!("tier manifest: {m}"));
    let compiled: Vec<&str> = crate::registry::pro_kinds()
        .iter()
        .map(|k| k.name)
        .chain(super::pro_command_features())
        .chain(ARKSA_FEATURES.iter().copied())
        .collect();
    for id in &compiled {
        let want = compiled_edition(id);
        let got = m.edition_of(id);
        if want != got {
            return Err(bad(format!(
                "an entry of this build is missing or at another tier ({})",
                if want.is_some_and(Edition::is_hidden) {
                    "hidden"
                } else {
                    id
                }
            )));
        }
    }
    for (name, _, _) in m.named() {
        if !compiled.contains(&name) {
            return Err(bad(format!("'{name}' is listed but not in this build")));
        }
    }
    Ok(())
}

/// The manifest document this build's code implies (every kind and feature it knows, at
/// the tier the code gives). In a default build the hidden entries are not known, so this
/// is complete only in a build with the `arksa` feature, which is where the manifest is
/// regenerated (`tests/tier_manifest.rs`).
pub fn manifest_from_code() -> ManifestDoc {
    let mut entries: Vec<ManifestEntry> = Vec::new();
    let mut push = |id: &str, ty: &str| {
        let Some(t) = compiled_edition(id) else {
            return;
        };
        let e = if t.is_hidden() {
            ManifestEntry {
                kind: None,
                kind_sha256: Some(name_hash(id)),
                entry_type: ty.into(),
                tier: t.as_str().into(),
            }
        } else {
            ManifestEntry {
                kind: Some(id.into()),
                kind_sha256: None,
                entry_type: ty.into(),
                tier: t.as_str().into(),
            }
        };
        if !entries.contains(&e) {
            entries.push(e);
        }
    };
    for k in crate::registry::pro_kinds() {
        push(k.name, "kind");
    }
    for f in super::pro_command_features()
        .into_iter()
        .chain(ARKSA_FEATURES.iter().copied())
    {
        push(f, "feature");
    }
    entries.sort_by(|a, b| {
        let key = |e: &ManifestEntry| {
            (
                Edition::parse(&e.tier).ok(),
                e.entry_type.clone(),
                e.kind.clone().or_else(|| e.kind_sha256.clone()),
            )
        };
        key(a).cmp(&key(b))
    });
    ManifestDoc {
        format: MANIFEST_FORMAT.into(),
        version: 1,
        entries,
    }
}

/// The manifest as written to the file: pretty JSON and a final newline.
pub fn manifest_text(doc: &ManifestDoc) -> String {
    let mut s = serde_json::to_string_pretty(doc).expect("the manifest is plain data");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_manifest_verifies_and_matches_this_build() {
        startup_check().expect("the embedded manifest verifies and matches the build");
    }

    #[test]
    fn edition_names_round_trip() {
        for e in [
            Edition::OpenSeam,
            Edition::Pro,
            Edition::Arksa,
            Edition::Core,
        ] {
            assert_eq!(Edition::parse(e.as_str()).unwrap(), e);
        }
        assert!(Edition::parse("gold").is_err());
    }

    #[test]
    fn unknown_kinds_have_no_edition() {
        assert_eq!(edition_of("pro-not-a-kind"), None);
        assert_eq!(edition_of("pro-twin"), Some(Edition::Pro));
        assert_eq!(edition_of("pro-seam-check"), Some(Edition::OpenSeam));
    }
}
