// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! GBAS airborne test automation: event timelines for procedures of kind `gbas-events`
//! (research 2026-11-20 gaps, B1204).
//!
//! A GBAS procedure states a timeline of stimulus **events** as the test bench (a GBAS VDB
//! simulator driven by the scenario runner) produces them, and this module turns it into the
//! **expected-behaviour timeline** the equipment log is scored against with the same checks as
//! the SBAS procedures ([`super::procedure::sbas_checks`]). No VDB message is encoded or decoded
//! here: the VDB decoder belongs in the open engine, and the bench produces the signal. The
//! limits are named parameters the applicant supplies from their copy of the MOPS (RTCA DO-253,
//! revision in use); a check whose limit is not supplied is **not assessed**.
//!
//! | event | expectation |
//! |-------|-------------|
//! | `vdb-loss` | the GBAS approach mode is left (mode at most `gnss-only`) by `vdb_loss_limit_s` after the loss and stays left while the loss lasts |
//! | `correction-stale` | left by `correction_timeout_s` after the last fresh correction message |
//! | `fas-data-invalid` | left by `fas_alert_s` after the invalid final-approach-segment data is first broadcast |
//! | `protection-over-alert` | left by `alert_time_s` after the stimulus makes the protection level exceed the alert limit |
//! | `ssid-mismatch` | informational: the bench broadcasts under another station identifier |
//!
//! MODELLED: a test procedure structure; the event semantics and limits are to confirm against
//! the applicant's copy of DO-253 and the bench's own documentation.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::sbas_seq::{ExpectKind, Expectation, Stream};

/// Format of a generated GBAS timeline.
pub const OUTPUT_FORMAT: &str = "kshana-gbas-timeline/1";

/// One stimulus event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    /// `vdb-loss`, `correction-stale`, `fas-data-invalid`, `protection-over-alert`,
    /// `ssid-mismatch`.
    pub kind: String,
    pub t_s: f64,
    pub duration_s: f64,
}

/// The `[gbas]` table of a procedure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GbasSpec {
    pub duration_s: u32,
    #[serde(default, rename = "event")]
    pub events: Vec<Event>,
}

const KINDS: [(&str, &str, &str); 5] = [
    (
        "vdb-loss",
        "vdb_loss_limit_s",
        "VDB lost: GBAS approach left",
    ),
    (
        "correction-stale",
        "correction_timeout_s",
        "corrections no longer refreshed: GBAS approach left",
    ),
    (
        "fas-data-invalid",
        "fas_alert_s",
        "final-approach-segment data invalid: GBAS approach left",
    ),
    (
        "protection-over-alert",
        "alert_time_s",
        "protection level above the alert limit: GBAS approach left",
    ),
    (
        "ssid-mismatch",
        "",
        "broadcast under another station identifier (informational)",
    ),
];

impl GbasSpec {
    /// Check the timeline.
    pub fn validate(&self) -> Result<(), String> {
        if self.duration_s == 0 {
            return Err("gbas.duration_s must be positive".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for e in &self.events {
            if !KINDS.iter().any(|k| k.0 == e.kind) {
                return Err(format!(
                    "gbas event '{}': kind '{}' (expected {})",
                    e.id,
                    e.kind,
                    KINDS.iter().map(|k| k.0).collect::<Vec<_>>().join(", ")
                ));
            }
            if !ids.insert(&e.id) {
                return Err(format!("gbas event '{}' declared twice", e.id));
            }
            if !(e.t_s >= 0.0
                && e.duration_s > 0.0
                && e.t_s + e.duration_s <= self.duration_s as f64)
            {
                return Err(format!(
                    "gbas event '{}': needs 0 <= t_s, a positive duration_s, and to end within duration_s",
                    e.id
                ));
            }
        }
        Ok(())
    }
}

/// The expected-behaviour timeline of a GBAS spec (no frames).
pub fn generate(spec: &GbasSpec) -> Result<Stream, String> {
    spec.validate()?;
    let spec_json = serde_json::to_string(spec).map_err(|e| e.to_string())?;
    let mut ex = Vec::new();
    for e in &spec.events {
        let (_, param, what) = KINDS.iter().find(|k| k.0 == e.kind).expect("validated");
        let end = e.t_s + e.duration_s;
        if param.is_empty() {
            ex.push(Expectation {
                id: format!("{}-info", e.id),
                event: e.id.clone(),
                kind: ExpectKind::Informational,
                prns: vec![],
                mode: None,
                t_ref_s: e.t_s,
                until_s: Some(end),
                limit_param: String::new(),
                what: (*what).into(),
            });
            continue;
        }
        ex.push(Expectation {
            id: format!("{}-leave-gbas", e.id),
            event: e.id.clone(),
            kind: ExpectKind::ModeAtMost,
            prns: vec![],
            mode: Some("gnss-only".into()),
            t_ref_s: e.t_s,
            until_s: Some(end),
            limit_param: (*param).into(),
            what: (*what).into(),
        });
        ex.push(Expectation {
            id: format!("{}-after", e.id),
            event: e.id.clone(),
            kind: ExpectKind::Informational,
            prns: vec![],
            mode: None,
            t_ref_s: end,
            until_s: None,
            limit_param: String::new(),
            what: "stimulus ended: whether and when the approach is offered again is the procedure's operational question, recorded not scored".into(),
        });
    }
    Ok(Stream {
        format: OUTPUT_FORMAT.into(),
        label: "MODELLED: GBAS event timeline; no VDB messages encoded".into(),
        spec_sha256: hex::encode(Sha256::digest(spec_json.as_bytes())),
        frames: vec![],
        expectations: ex,
        frames_sha256: hex::encode(Sha256::digest(spec_json.as_bytes())),
    })
}
