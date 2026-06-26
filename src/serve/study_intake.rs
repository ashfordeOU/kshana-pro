// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Study-request intake (P6)
//!
//! The schema + validation for a *study request* — a customer asking for a packaged
//! study to be run and delivered — plus a deterministic study id. This is the intake
//! step only: it validates the request against the live set of runnable kinds and
//! acknowledges it. It deliberately does **not** execute the pack (packs are
//! packaged in a later task); the wired `POST /study` handler in [`super`] accepts +
//! acknowledges, gated by the same license tier `/run` uses.
//!
//! Validity of `pack` is checked against the *live* set of kinds a Pro build can run
//! — the union of the embedded open engine's kinds ([`kshana::api::list_scenario_kinds`])
//! and the registered Pro packs ([`crate::registry::pro_kinds`]). There is no second
//! hard-coded list to drift out of sync with the dispatcher.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A customer's request for a packaged study.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyRequest {
    /// The scenario/pack kind to study (an open engine kind or a registered Pro pack).
    pub pack: String,
    /// The requesting customer (organisation). Required, non-empty.
    pub customer: String,
    /// Optional recipient/team the study is prepared for.
    #[serde(default)]
    pub prepared_for: Option<String>,
    /// The study inputs — kept opaque here (validated when the pack runs, later).
    #[serde(default)]
    pub inputs: serde_json::Value,
}

/// Every pack id a Pro build can run: the union of the embedded open engine's kinds
/// and the registered Pro packs. The single source of truth is the dispatcher's own
/// registry, so this never drifts from what `/run` will actually accept.
fn known_pack_ids() -> Vec<String> {
    let mut ids: Vec<String> = kshana::api::list_scenario_kinds()
        .iter()
        .map(|m| m.name.to_string())
        .collect();
    ids.extend(
        crate::registry::pro_kinds()
            .iter()
            .map(|m| m.name.to_string()),
    );
    ids
}

/// Validate a [`StudyRequest`]. Returns the list of human-readable violations; an
/// empty list means the request is valid. Pure and cheap — runs nothing heavy.
pub fn validate(req: &StudyRequest) -> Vec<String> {
    let mut violations = Vec::new();

    if req.customer.trim().is_empty() {
        violations.push("customer must be a non-empty string".to_string());
    }

    if req.pack.trim().is_empty() {
        violations.push("pack must be a non-empty kind".to_string());
    } else {
        let known = known_pack_ids();
        if !known.iter().any(|k| k == &req.pack) {
            violations.push(format!(
                "pack '{}' is not a known kind (expected one of the open or Pro kinds)",
                req.pack
            ));
        }
    }

    violations
}

/// A deterministic, short study id: the first 16 bytes (32 hex chars) of
/// `sha256(canonical(request))`. Stable for identical requests, independent of map
/// key ordering in `inputs` (we re-serialise through `serde_json::Value`, which
/// sorts object keys when the `preserve_order` feature is off — the default here).
pub fn study_id(req: &StudyRequest) -> String {
    let canonical = canonical_request(req);
    let digest = Sha256::digest(canonical.as_bytes());
    hex::encode(&digest[..16])
}

/// The canonical serialisation of a request for hashing: a fixed field order with
/// `inputs` re-serialised through a `Value` so equivalent JSON hashes identically.
fn canonical_request(req: &StudyRequest) -> String {
    let canonical = serde_json::json!({
        "pack": req.pack,
        "customer": req.customer,
        "prepared_for": req.prepared_for,
        "inputs": req.inputs,
    });
    serde_json::to_string(&canonical).expect("study request serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> StudyRequest {
        StudyRequest {
            pack: "pro-twin".into(),
            customer: "ACME".into(),
            prepared_for: None,
            inputs: serde_json::json!({}),
        }
    }

    #[test]
    fn known_ids_include_open_and_pro_kinds() {
        let ids = known_pack_ids();
        assert!(
            ids.iter().any(|k| k == "pro-seam-check"),
            "Pro kinds present"
        );
        // The embedded open engine reports at least one kind.
        assert!(
            ids.len() > crate::registry::pro_kinds().len(),
            "open kinds present too"
        );
    }

    #[test]
    fn valid_request_has_no_violations() {
        assert!(validate(&valid()).is_empty());
    }

    #[test]
    fn unknown_pack_is_flagged() {
        let mut r = valid();
        r.pack = "pro-mystery".into();
        assert!(validate(&r).iter().any(|v| v.contains("not a known kind")));
    }

    #[test]
    fn empty_customer_is_flagged() {
        let mut r = valid();
        r.customer = "   ".into();
        assert!(validate(&r).iter().any(|v| v.contains("customer")));
    }

    #[test]
    fn study_id_is_deterministic_and_short_hex() {
        let id = study_id(&valid());
        assert_eq!(id, study_id(&valid()));
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
