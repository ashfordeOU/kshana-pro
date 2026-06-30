// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — Tamper-evident provenance for a completed trade study
//!
//! Wraps a [`TradeReport`](crate::suite::TradeReport) in the open core's W3C-PROV
//! record ([`kshana::assurance::provenance::ProvRecord`]) and folds the
//! {inputs, per-variant outcomes, weights, decision} into the record's deterministic
//! SHA-256 **Merkle root**. The root is stable for fixed inputs and changes if and
//! only if any of those quantities changes — so an evidence pack is reproducible and
//! any post-hoc edit to a score, weight, or input is detectable.
//!
//! Honest scope: this is *bookkeeping* — a hash over the decision, not a measurement.
//! It composes the open core's (deterministic, `wasm32`-safe) provenance primitive;
//! nothing here is Validated.

use kshana::assurance::provenance::ProvRecord;
use sha2::{Digest, Sha256};

use super::{DecisionReport, TradeReport};

/// A provenance attestation: the W3C-PROV record and its Merkle root.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DecisionProvenance {
    pub record: ProvRecord,
    /// 64-hex SHA-256 Merkle root over the record's fields.
    pub merkle_root: String,
}

/// Deterministic scientific-notation rendering of an `f64` (Rust's shortest
/// round-tripping representation), so identical bits hash identically.
fn fnum(x: f64) -> String {
    format!("{x:e}")
}

fn opt_num(x: Option<f64>) -> String {
    x.map(fnum).unwrap_or_else(|| "∅".to_string())
}

/// SHA-256 (hex) of a string — used to turn each canonical segment into a content
/// hash, mirroring the open core's `study_ledger` hashing.
fn sha_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

/// Canonical, order-stable digest of one variant's input + outcome.
fn variant_digest(o: &super::VariantOutcome) -> String {
    let mut s = String::new();
    s.push_str("label=");
    s.push_str(&o.label);
    s.push_str(";hash=");
    s.push_str(o.scenario_hash.as_deref().unwrap_or("∅"));
    s.push_str(";metric=");
    s.push_str(&opt_num(o.metric_value));
    s.push_str(";rank=");
    s.push_str(&o.rank.map(|r| r.to_string()).unwrap_or_else(|| "∅".into()));
    s.push_str(";crit=[");
    for (j, v) in o.criterion_values.iter().enumerate() {
        if j > 0 {
            s.push(',');
        }
        s.push_str(&opt_num(*v));
    }
    s.push_str("];err=");
    s.push_str(o.error.as_deref().unwrap_or("ok"));
    sha_hex(&s)
}

/// Canonical digest of the decision (criteria, weights, ranking, scores, sensitivity).
fn decision_digest(dr: &DecisionReport) -> String {
    let mut s = String::new();
    s.push_str("weighting=");
    s.push_str(&dr.weighting_method);
    if let Some(a) = &dr.ahp {
        s.push_str(&format!(
            ";ahp(lmax={},ci={},cr={},thr={},ok={})",
            fnum(a.lambda_max),
            fnum(a.consistency_index),
            opt_num(a.consistency_ratio),
            fnum(a.threshold),
            a.acceptable
        ));
    }
    s.push_str(";criteria=[");
    for (i, c) in dr.criteria.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{}:{}:{}:{}", c.name, c.direction, c.value_fn, fnum(c.weight)));
    }
    s.push_str("];ranking=[");
    for (i, r) in dr.ranking.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&r.to_string());
    }
    s.push_str("];winner=");
    s.push_str(&dr.winner.map(|w| w.to_string()).unwrap_or_else(|| "∅".into()));
    s.push_str(";pareto=[");
    for (i, p) in dr.pareto_front.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&p.to_string());
    }
    s.push_str("];scores=[");
    for (i, sc) in dr.scores.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{}={}", sc.variant_index, fnum(sc.score)));
    }
    s.push_str("];smaa=[");
    for (i, e) in dr.smaa.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{}={}", e.variant_index, fnum(e.p_rank1)));
    }
    s.push(']');
    if let Some(f) = &dr.min_flip {
        s.push_str(&format!(";flip(l1={},to={})", fnum(f.l1_change), f.new_winner_index));
    } else {
        s.push_str(";flip=none");
    }
    sha_hex(&s)
}

/// Build the provenance attestation for a completed trade study.
///
/// The `input_hashes` are, in stable order: one content hash per variant
/// (input scenario hash ⊕ its outcome), sorted by label, followed by a single
/// decision digest when a multi-criteria decision is present. `timestamp` is left
/// `None` (caller-supplied) so the root stays reproducible; the `lockfile_hash` is a
/// release-pipeline field left empty here and noted as such.
pub fn build_provenance(report: &TradeReport) -> DecisionProvenance {
    let mut by_label: Vec<&super::VariantOutcome> = report.outcomes.iter().collect();
    by_label.sort_by(|a, b| a.label.cmp(&b.label));
    let mut input_hashes: Vec<String> = by_label.iter().map(|o| variant_digest(o)).collect();

    let seed = report.decision.as_ref().map(|_| 0u64); // SMAA seed is config; recorded in notes
    if let Some(dr) = &report.decision {
        input_hashes.push(decision_digest(dr));
    }

    let mut notes = vec![
        "MCDA aggregation (WSM/AHP/Pareto) is closed-form and deterministic.".to_string(),
        "SMAA acceptability uses a seeded Dirichlet RNG — deterministic for a fixed seed."
            .to_string(),
    ];
    if report.decision.is_none() {
        notes.push("Single-figure-of-merit study (no multi-criteria decision block).".to_string());
    }

    let record = ProvRecord {
        engine_commit: format!("kshana-pro/{} on open kshana core", crate::version()),
        input_hashes,
        seed,
        nondeterminism_notes: notes,
        // Release pipeline stamps the resolved Cargo.lock hash; empty here keeps the
        // root reproducible from the report alone.
        lockfile_hash: String::new(),
        timestamp: None,
        tolerance: Some("scores hashed at f64 shortest-round-trip precision".to_string()),
    };
    let merkle_root = record.merkle_root();
    DecisionProvenance { record, merkle_root }
}

impl DecisionProvenance {
    /// Markdown rendering of the provenance record + Merkle root.
    pub fn markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("## Provenance (W3C-PROV)\n\n");
        md.push_str(&format!("- Engine: `{}`\n", self.record.engine_commit));
        md.push_str(&format!(
            "- Inputs hashed: {} content hash(es)\n",
            self.record.input_hashes.len()
        ));
        if let Some(s) = self.record.seed {
            md.push_str(&format!("- Seed recorded: {s}\n"));
        }
        md.push_str(&format!(
            "- Tolerance: {}\n",
            self.record.tolerance.as_deref().unwrap_or("—")
        ));
        for n in &self.record.nondeterminism_notes {
            md.push_str(&format!("- Note: {n}\n"));
        }
        md.push_str(&format!("\n**Merkle root:** `{}`\n\n", self.merkle_root));
        md
    }
}

#[cfg(test)]
mod tests {
    use super::super::{run_trade_study, TradeSpace, Variant};
    use super::*;

    fn ephem(duration_s: f64) -> String {
        format!(
            "kind = \"ephemeris\"\n\
             tle = \"\"\"\n\
             1 25544U 98067A   20045.18587073  .00000950  00000-0  25302-4 0  9990\n\
             2 25544  51.6443 242.0161 0004885 264.6060 207.3845 15.49165514212791\n\
             \"\"\"\n\
             step_s = 30.0\n\
             duration_s = {duration_s}\n"
        )
    }

    fn space() -> TradeSpace {
        TradeSpace::single_fom(
            "prov trade",
            "n_samples",
            false,
            vec![
                Variant {
                    label: "short".into(),
                    scenario_toml: ephem(60.0),
                },
                Variant {
                    label: "long".into(),
                    scenario_toml: ephem(300.0),
                },
            ],
        )
    }

    /// Golden: the Merkle root is a 64-hex string and stable for fixed inputs.
    #[test]
    fn merkle_root_is_stable_for_fixed_inputs() {
        let r1 = run_trade_study(&space()).unwrap();
        let r2 = run_trade_study(&space()).unwrap();
        let p1 = build_provenance(&r1);
        let p2 = build_provenance(&r2);
        assert_eq!(p1.merkle_root.len(), 64);
        assert!(p1.merkle_root.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(
            p1.merkle_root, p2.merkle_root,
            "fixed inputs must reproduce the same root"
        );
    }

    /// Tamper-evidence: perturbing an outcome changes the root.
    #[test]
    fn editing_an_outcome_changes_the_root() {
        let r = run_trade_study(&space()).unwrap();
        let base = build_provenance(&r).merkle_root;

        let mut tampered = r.clone();
        // Flip a metric value — a post-hoc edit to the evidence.
        tampered.outcomes[0].metric_value = tampered.outcomes[0].metric_value.map(|v| v + 1.0);
        assert_ne!(
            build_provenance(&tampered).merkle_root,
            base,
            "a changed score must change the Merkle root"
        );
    }

    #[test]
    fn round_trips_through_serde() {
        let r = run_trade_study(&space()).unwrap();
        let p = build_provenance(&r);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("merkle_root"));
        assert!(json.contains(&p.merkle_root));
    }
}
