// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The versioned **scenario-pack manifest** — the addressable unit of Pack C.
//!
//! A [`ScenarioPack`] names an ordered, labelled set of scenario configs (its
//! [`PackEntry`] list) under a stable `id` + `version`. A customer subscribes to
//! a pack by id/version and runs their own config against it in CI; the
//! [`pack_hash`](ScenarioPack::pack_hash) gives a deterministic content digest so
//! a run report can pin exactly which pack contents produced it (and a regression
//! gate can detect when the pack itself changed).
//!
//! This file owns only the manifest type, its TOML parser, and the content hash.
//! Metric extraction, the regression gate, the run orchestrator, the branded
//! report, and the CLI are separate later concerns and are *not* built here.

use crate::pack_c::watch::Watch;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One scenario config referenced by a pack, in declaration order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackEntry {
    /// Path (relative to the pack) of the scenario config file, e.g. `araim.toml`.
    pub path: String,
    /// Optional human-facing label; falls back to the file stem of `path`.
    pub label: Option<String>,
}

impl PackEntry {
    /// The display label for this entry: the explicit `label` if present, else the
    /// file stem of `path` (filename without extension), else the raw `path`.
    pub fn label(&self) -> String {
        if let Some(label) = &self.label {
            return label.clone();
        }
        std::path::Path::new(&self.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.path.clone())
    }
}

/// A versioned, subscribable library of resilience scenarios.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioPack {
    /// Stable identifier for the pack, e.g. `resilience-baseline`.
    pub id: String,
    /// Semantic version of the pack contents, e.g. `1.0.0`.
    pub version: String,
    /// Optional human-facing description of what the pack covers.
    pub description: Option<String>,
    /// Ordered scenario entries; at least one is required.
    pub entries: Vec<PackEntry>,
    /// The `[[watch]]` tables: the figures that matter, what worse means for each, and
    /// optional absolute requirements (see [`crate::pack_c::watch`]). A pack with none
    /// behaves exactly as before campaign watch existed.
    #[serde(default, rename = "watch", skip_serializing_if = "Vec::is_empty")]
    pub watches: Vec<Watch>,
}

impl ScenarioPack {
    /// A deterministic SHA-256 content digest (lowercase hex) over the pack's
    /// `id`, `version`, and entries in order. Pinning this in a run report lets a
    /// regression gate detect when the pack contents themselves changed.
    ///
    /// The watches are hashed after the entries, one canonical JSON line each, and only
    /// when there are any, so a manifest without `[[watch]]` tables hashes exactly as it
    /// did before watches existed and its stored baselines stay valid.
    pub fn pack_hash(&self) -> String {
        let mut canonical = format!("{}\n{}\n", self.id, self.version);
        for entry in &self.entries {
            canonical.push_str(&format!(
                "{}\t{}\n",
                entry.path,
                entry.label.clone().unwrap_or_default()
            ));
        }
        for watch in &self.watches {
            let line = serde_json::to_string(watch).unwrap_or_default();
            canonical.push_str(&format!("watch\t{line}\n"));
        }
        let digest = Sha256::digest(canonical.as_bytes());
        hex::encode(digest)
    }
}

/// Parse a [`ScenarioPack`] from a TOML manifest, validating required fields.
///
/// Returns `Err` if the TOML is malformed, if `id` or `version` is empty (after
/// trimming), or if there are no entries.
pub fn parse_pack(src: &str) -> Result<ScenarioPack, String> {
    let pack: ScenarioPack =
        toml::from_str(src).map_err(|e| format!("invalid scenario pack: {e}"))?;
    if pack.id.trim().is_empty() {
        return Err("scenario pack: 'id' must not be empty".to_string());
    }
    if pack.version.trim().is_empty() {
        return Err("scenario pack: 'version' must not be empty".to_string());
    }
    if pack.entries.is_empty() {
        return Err("scenario pack: at least one entry is required".to_string());
    }
    if !pack.watches.is_empty() {
        let labels: Vec<String> = pack.entries.iter().map(PackEntry::label).collect();
        for (i, l) in labels.iter().enumerate() {
            if labels[..i].contains(l) {
                return Err(format!(
                    "scenario pack: two entries share the label '{l}', so a watch cannot name one"
                ));
            }
        }
        let mut ids: Vec<String> = Vec::new();
        for w in &pack.watches {
            w.validate(&labels)
                .map_err(|e| format!("scenario pack: {e}"))?;
            let id = w.id();
            if ids.contains(&id) {
                return Err(format!(
                    "scenario pack: two watches share the identity '{id}'; give one a 'name'"
                ));
            }
            ids.push(id);
        }
    }
    Ok(pack)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPRESENTATIVE: &str = r#"
id = "resilience-baseline"
version = "1.0.0"
description = "Baseline PNT-resilience scenario pack"

[[entries]]
path = "araim.toml"
label = "ARAIM availability"

[[entries]]
path = "holdover.toml"
"#;

    #[test]
    fn parses_representative_manifest() {
        let pack = parse_pack(REPRESENTATIVE).expect("representative manifest must parse");
        assert_eq!(pack.id, "resilience-baseline");
        assert_eq!(pack.version, "1.0.0");
        assert_eq!(
            pack.description.as_deref(),
            Some("Baseline PNT-resilience scenario pack")
        );
        assert_eq!(pack.entries.len(), 2);
        // Explicit label is used verbatim.
        assert_eq!(pack.entries[0].label(), "ARAIM availability");
        // Missing label falls back to the file stem of the path.
        assert_eq!(pack.entries[1].label(), "holdover");
    }

    #[test]
    fn rejects_empty_id() {
        let src = r#"
id = ""
version = "1.0.0"

[[entries]]
path = "araim.toml"
"#;
        assert!(parse_pack(src).is_err());
    }

    #[test]
    fn rejects_empty_version() {
        let src = r#"
id = "resilience-baseline"
version = ""

[[entries]]
path = "araim.toml"
"#;
        assert!(parse_pack(src).is_err());
    }

    #[test]
    fn rejects_no_entries() {
        let src = r#"
id = "resilience-baseline"
version = "1.0.0"
"#;
        assert!(parse_pack(src).is_err());
    }

    #[test]
    fn rejects_explicit_empty_entries_array() {
        // An explicit `entries = []` exercises the `entries.is_empty()` guard
        // (not just the missing-field path covered by `rejects_no_entries`).
        let src = r#"
id = "resilience-baseline"
version = "1.0.0"
entries = []
"#;
        assert!(parse_pack(src).is_err());
    }

    #[test]
    fn pack_hash_is_unchanged_without_watches_and_moves_with_them() {
        // The hash of a watch-free manifest is the pre-watch canonical form, byte for
        // byte, so stored baselines of existing packs stay valid.
        let pack = parse_pack(REPRESENTATIVE).unwrap();
        let canonical =
            "resilience-baseline\n1.0.0\naraim.toml\tARAIM availability\nholdover.toml\t\n";
        assert_eq!(
            pack.pack_hash(),
            hex::encode(Sha256::digest(canonical.as_bytes()))
        );
        let watched = format!(
            "{REPRESENTATIVE}\n[[watch]]\nentry = \"holdover\"\nmetric = \"fom.holdover_s\"\n"
        );
        let w = parse_pack(&watched).expect("watched manifest parses");
        assert_eq!(w.watches.len(), 1);
        assert_ne!(w.pack_hash(), pack.pack_hash());
    }

    #[test]
    fn a_watch_on_an_unknown_entry_is_refused() {
        let src = format!("{REPRESENTATIVE}\n[[watch]]\nentry = \"nope\"\nmetric = \"x\"\n");
        let e = parse_pack(&src).unwrap_err();
        assert!(e.contains("not a label"), "{e}");
    }

    #[test]
    fn pack_hash_is_stable() {
        let pack = parse_pack(REPRESENTATIVE).unwrap();
        assert_eq!(pack.pack_hash(), pack.pack_hash());
    }

    #[test]
    fn pack_hash_changes_on_version_and_path() {
        let base = parse_pack(REPRESENTATIVE).unwrap();
        let base_hash = base.pack_hash();

        let mut bumped = base.clone();
        bumped.version = "1.0.1".to_string();
        assert_ne!(base_hash, bumped.pack_hash());

        let mut repathed = base.clone();
        repathed.entries[0].path = "araim-strict.toml".to_string();
        assert_ne!(base_hash, repathed.pack_hash());
    }
}
