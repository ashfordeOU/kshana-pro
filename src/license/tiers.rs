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
//! built binary; the defence there is that Arksa-exclusive code is not in this crate at
//! all: it lives in a separate private crate that registers it through
//! [`crate::extension`] on Arksa servers only (docs/PROTECTION.md).

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

/// The hidden-tier entries of the manifest, by the SHA-256 of their name and their type
/// (`kind` or `feature`), all at the `arksa` tier. This crate does not contain their code
/// or their names; a server-side extension registers them ([`crate::extension`]), and
/// [`startup_check`] then requires each registered id to be one of these. The manifest is
/// regenerated from this list and the code (`tests/tier_manifest.rs`).
pub const ARKSA_ENTRIES: &[(&str, &str)] = &[
    (
        "17fbdf8fab325a9aafa27ee63d5fbd4b16492792b86c386959d5c8e591b34667",
        "feature",
    ),
    (
        "2302247b261f2d2e82c5e2cba3f64a35f5839bc8d90615ab83424dbc2855059f",
        "feature",
    ),
    (
        "4ddf8f80413304e353c1ba9ca8ff9b8cfe268c7eed082b62b0ad0ee8e3366f6a",
        "feature",
    ),
    (
        "64f0b0f231578148fae073eecde0e2ba064170c1ea55a5795b3e22c82d244b78",
        "feature",
    ),
    (
        "e36e88f761e240e2d6a40ff582ff45121918e6bbf10cc7c342024e9fcbe9ed60",
        "feature",
    ),
    (
        "f52968cb2703f58be986d2370c7bc262cd2335919f0a454b2a35d63ecce02213",
        "feature",
    ),
    (
        "fb4c10297445d385975d2f9797c3932a8dc62be33248c6840112c5308081d984",
        "feature",
    ),
    (
        "3b296fd95f942821e84fcd2c81589d780888057d8f072f0d09d6b3bbc8a0a2be",
        "kind",
    ),
    (
        "406cd104a2b12b29aa7e3a65a039d6b7487698d7722bcc0c4c2195183a869190",
        "kind",
    ),
    (
        "64f0b0f231578148fae073eecde0e2ba064170c1ea55a5795b3e22c82d244b78",
        "kind",
    ),
    (
        "7e052860f533fe43e38025c9ec766ed16dc6576c449361dece09159c964dfa5d",
        "kind",
    ),
    (
        "847c1921f266f745dc49f1a577a867f83c5f761497817caa123ea5733e43e98a",
        "kind",
    ),
    (
        "a45f6206d4e4d8e74ed429220b926d0a9f874d0389c1343b0c42d2871d347609",
        "kind",
    ),
    (
        "a849e2191cd7cf76f7d387b404547667f3f17a2f9f195a8bfa8deb3df15d2875",
        "kind",
    ),
    (
        "c0df40881e489d6234e90a098ef19717f6c947bc24e5d498101fe5dde7a7a555",
        "kind",
    ),
    (
        "cd7e35ae0d77db5dc91e7ce5b81c0425c5d7c94120b816ebdf134ce30b5fde9b",
        "kind",
    ),
    (
        "e36e88f761e240e2d6a40ff582ff45121918e6bbf10cc7c342024e9fcbe9ed60",
        "kind",
    ),
    (
        "fb4c10297445d385975d2f9797c3932a8dc62be33248c6840112c5308081d984",
        "kind",
    ),
    (
        "3dd23d8da9ae741dfc55a56b065d6bf330c20eeeac1ec846dd94760dc26a5a58",
        "kind",
    ),
    (
        "3dd23d8da9ae741dfc55a56b065d6bf330c20eeeac1ec846dd94760dc26a5a58",
        "feature",
    ),
    (
        "e38ac47b6775739c7a123902edfb7deef71b1fb391f67c9d2b73b2f7ecc3b778",
        "kind",
    ),
    // Registered 20 Nov 2026 (kind and feature).
    (
        "a06ac1042f0d90924a98605c15407bf8851a0a7b712d997951dd440576b37996",
        "kind",
    ),
    (
        "a06ac1042f0d90924a98605c15407bf8851a0a7b712d997951dd440576b37996",
        "feature",
    ),
];

/// Pro kinds this build knows but registers only when a verified Pro pack that provides them
/// is loaded (roadmap 0.42 C10): they are listed at the `pro` edition in the manifest, and
/// refused with a "comes in a Pro pack" message until the pack is loaded.
pub const PACK_DELIVERED_KINDS: &[&str] = &[crate::forensics::KIND];

/// The edition this build's code gives a Pro kind or feature, or `None` when the build
/// does not contain it.
pub fn compiled_edition(id: &str) -> Option<Edition> {
    if id == "pro-seam-check" {
        return Some(Edition::OpenSeam);
    }
    if PACK_DELIVERED_KINDS.contains(&id) {
        return Some(Edition::Pro);
    }
    if crate::extension::owns(id) {
        return Some(Edition::Arksa);
    }
    if crate::extension::pack_owns(id) {
        return Some(Edition::Pro);
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
    let m = manifest().ok()?;
    m.edition_of(id).or_else(|| {
        // A kind of a loaded Pro pack (roadmap 0.42 C10): the pack's own signed manifest
        // lists it, and registration refused any id the tier manifest names at another
        // tier, so it is at `pro`. Only while the tier manifest itself verifies.
        crate::extension::pack_owns(id).then_some(Edition::Pro)
    })
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
        .chain(PACK_DELIVERED_KINDS.iter().copied())
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

/// The manifest document this crate implies: every kind and feature it contains, at the
/// tier the code gives, plus the hidden entries of [`ARKSA_ENTRIES`] by hash. A registered
/// extension's ids are not added by name (they are hidden; [`startup_check`] matches
/// them against the hashes). The manifest is regenerated from this (`tests/tier_manifest.rs`).
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
    for k in crate::registry::builtin_pro_kinds() {
        push(k.name, "kind");
    }
    for k in PACK_DELIVERED_KINDS {
        push(k, "kind");
    }
    for f in super::PRO_COMMAND_FEATURES {
        push(f, "feature");
    }
    for (hash, ty) in ARKSA_ENTRIES {
        entries.push(ManifestEntry {
            kind: None,
            kind_sha256: Some((*hash).into()),
            entry_type: (*ty).into(),
            tier: Edition::Arksa.as_str().into(),
        });
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
