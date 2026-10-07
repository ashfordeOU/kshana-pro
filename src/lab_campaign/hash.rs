//! Content hashes of the lab-replay results, as the engine defines them.
//!
//! Canonical JSON is `serde_json`'s compact form of a `serde_json::Value`, with object keys
//! sorted (this crate graph does not enable `preserve_order`) and shortest round-trip floats,
//! matching the engine's `iq::campaign::hash` (public `kshana` `feat/034-campaign`).
//!
//! [`digest`] is **provisional**. LAB-CAMPAIGN.md §3 defines DIGEST as "SHA-256 over the sorted
//! (key, sha256(cell file)) list" without fixing the byte form. Here it is the SHA-256 of the
//! canonical JSON of `[[key, sha256], …]` sorted by key. A request is with the engine side to
//! make its own function public; once it is, this module calls it instead.

use super::model::CampaignCell;
use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The canonical (compact, key-sorted) JSON text of `v`.
pub fn canonical_json(v: &serde_json::Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// SHA-256 of the canonical JSON of `v`.
pub fn canonical_hash(v: &serde_json::Value) -> String {
    sha256_hex(canonical_json(v).as_bytes())
}

/// A cell's key, re-derived from its own inputs (LAB-CAMPAIGN.md §2, "Cells").
pub fn cell_key(c: &CampaignCell) -> String {
    canonical_hash(&serde_json::json!({
        "recording_sha256": c.recording.sha256,
        "conditions_hash": c.recording.conditions_hash,
        "frontend_hash": c.frontend.hash,
        "design_hash": c.design.hash,
        "run_hash": c.run_hash,
        "scoring_hash": c.scoring_hash,
        "engine_version": c.engine_version,
    }))
}

/// The campaign DIGEST over `(key, sha256 of the cell file)` pairs (PROVISIONAL form).
pub fn digest<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut v: Vec<(&str, &str)> = pairs.into_iter().collect();
    v.sort();
    let arr: Vec<serde_json::Value> = v
        .into_iter()
        .map(|(k, s)| serde_json::json!([k, s]))
        .collect();
    canonical_hash(&serde_json::Value::Array(arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_sorts_keys_and_matches_the_engine_form() {
        let a: serde_json::Value = serde_json::from_str(r#"{"b":1,"a":[1.5,2]}"#).unwrap();
        // The engine's own unit test pins this exact text.
        assert_eq!(canonical_json(&a), r#"{"a":[1.5,2],"b":1}"#);
    }

    #[test]
    fn digest_does_not_depend_on_input_order() {
        let x = digest([("b", "2"), ("a", "1")]);
        let y = digest([("a", "1"), ("b", "2")]);
        assert_eq!(x, y);
        assert_ne!(x, digest([("a", "1")]));
    }
}
