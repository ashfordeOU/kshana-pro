//! Content hashes of the lab-replay results, exactly as the engine computes them.
//!
//! Canonical JSON is `serde_json`'s compact form of a `serde_json::Value`, with object keys
//! sorted (this crate graph does not enable `preserve_order`) and shortest round-trip floats,
//! matching the engine's `iq::campaign::hash` (public `kshana`, `feat/034-campaign` @ `d8f3c52a` (carries #45 `010b30b9`, main merged)).
//! The cell key is `runner::cell_key` and the DIGEST is `report::build` on that branch. Once the
//! engine is published at 0.34.0 and pinned here, these call the engine's own functions.

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

/// A cell's key, re-derived from the fields the cell itself records (engine `runner::cell_key`).
pub fn cell_key(c: &CampaignCell) -> String {
    canonical_hash(&serde_json::json!({
        "schema": c.schema,
        "recording_id": c.recording.id,
        "recording_sha256": c.recording.sha256,
        "conditions_hash": c.recording.conditions_hash,
        "frontend": c.frontend.name,
        "frontend_hash": c.frontend.hash,
        "design": c.design.name,
        "design_hash": c.design.hash,
        "run_hash": c.run_hash,
        "scoring_hash": c.scoring_hash,
        "data_class": c.data_class.as_str(),
        "engine_version": c.engine_version,
    }))
}

/// The campaign DIGEST (engine `report::build`): SHA-256 of the concatenation, in sorted order,
/// of one line `"<cell key> <sha256 of the cell file's bytes>\n"` per cell. The engine writes it
/// only when every planned cell is present.
pub fn digest<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut lines: Vec<String> = pairs
        .into_iter()
        .map(|(k, s)| format!("{k} {s}\n"))
        .collect();
    lines.sort();
    sha256_hex(lines.concat().as_bytes())
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
    fn digest_is_order_free_and_uses_the_engine_line_form() {
        let x = digest([("b", "2"), ("a", "1")]);
        assert_eq!(x, digest([("a", "1"), ("b", "2")]));
        // The byte form is the sorted lines, concatenated.
        assert_eq!(x, sha256_hex(b"a 1\nb 2\n"));
    }
}
