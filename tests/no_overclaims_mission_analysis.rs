// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! No-overclaim regression guard for the mission-analysis deliverables (A15).
//!
//! Extends the open core's `no_overclaims.rs` honesty pattern to the new Pro
//! artifacts — the benefit-vs-baseline engine ([`kshana_pro::benefits`]) and the
//! machine-checked MRD ([`kshana_pro::mrd`]). These are CI-level, cross-module
//! invariants: a violation fails the pipeline. They check *classification/labelling*
//! only — they never fabricate validation.

use kshana::verification::{OracleKind, VerificationStatus};
use kshana_pro::benefits::{benefits_case, fom_provenance, FomSpec};
use kshana_pro::mrd::{mrd_table, resolve_verification, Availability, FomSource};
use kshana_pro::suite::Variant;

/// The FoM keys the mission-analysis artifacts actually classify.
const FOM_KEYS: &[&str] = &[
    "quantum.fom.timing_rms_ns",
    "quantum.fom.timing_p95_ns",
    "quantum.fom.holdover_s",
    "quantum.fom.resilience_slope_ns_per_s",
    "quantum.fom.availability",
    "quantum.fom.integrity",
    "quantum.fom.security",
    "classical.fom.holdover_s",
    "user.hpl_m",
    "user.vpl_m",
    "pos_drift_m",
    "some.unmapped.metric",
];

/// INVARIANT: a FoM may carry the `Validated` label ONLY if its oracle is an
/// `ExternalDataset`. No mission-analysis number is upgraded to validated without a
/// genuine external oracle.
#[test]
fn validated_label_requires_an_external_dataset_oracle() {
    for key in FOM_KEYS {
        let (status, oracle) = fom_provenance(key);
        if status == VerificationStatus::Validated {
            assert_eq!(
                oracle,
                OracleKind::ExternalDataset,
                "FoM '{key}' is labelled Validated but its oracle is {oracle:?}, not ExternalDataset"
            );
        }
    }
}

/// INVARIANT: a benefit case over modelled clock FoMs produces only MODELLED rows —
/// a modelled quantum number is never silently emitted as validated.
#[test]
fn benefit_case_over_modelled_foms_emits_no_validated_row() {
    let scn = "seed = 42\n\
               threshold_ns = 20.0\n\
               [time]\n\
               step_s = 10.0\n\
               duration_s = 3600.0\n\
               [gnss]\n\
               windows = [ { t0 = 0.0, t1 = 600.0, state = \"nominal\" }, { t0 = 600.0, t1 = 3600.0, state = \"denied\" } ]\n\
               [clock_quantum]\n\
               id = \"q\"\nprovenance = \"demo\"\ny0 = 5.0e-17\nq_wf = 1.0e-30\nq_rw = 0.0\n\
               [clock_classical]\n\
               id = \"c\"\nprovenance = \"demo\"\ny0 = 5.0e-10\nq_wf = 9.0e-20\nq_rw = 0.0\n";
    let cand = Variant {
        label: "optical".into(),
        scenario_toml: scn.into(),
    };
    let base = Variant {
        label: "csac".into(),
        scenario_toml: scn.into(),
    };
    let foms = vec![
        FomSpec {
            name: "holdover".into(),
            candidate_key: "quantum.fom.holdover_s".into(),
            baseline_key: "classical.fom.holdover_s".into(),
            lower_better: false,
        },
        FomSpec {
            name: "timing rms".into(),
            candidate_key: "quantum.fom.timing_rms_ns".into(),
            baseline_key: "classical.fom.timing_rms_ns".into(),
            lower_better: true,
        },
    ];
    let case = benefits_case("modelled trade", &cand, &base, &foms).unwrap();
    for row in &case.rows {
        assert_eq!(
            row.candidate_label, "MODELLED",
            "candidate {} should be modelled",
            row.name
        );
        assert_eq!(
            row.baseline_label, "MODELLED",
            "baseline {} should be modelled",
            row.name
        );
        assert!(!row.crosses_label, "same-FoM rows must not cross tiers");
    }
    assert!(!case.any_crosses_label);
}

/// INVARIANT: position-domain FoMs stay not-implemented / partner-follow-on. If the
/// open core ever implements `positioning_performance`, this fails so the MRD is
/// revisited rather than silently claiming the capability.
#[test]
fn position_domain_capability_remains_unavailable() {
    assert!(
        kshana::fom::positioning_performance().is_err(),
        "positioning_performance() now returns Ok — the MRD position rows must be revisited"
    );
    for row in mrd_table() {
        if row.source == FomSource::PositioningFom {
            assert_eq!(
                row.availability,
                Availability::PartnerFollowOn,
                "MRD row {} (PositioningFom) must stay partner/follow-on",
                row.id
            );
        }
    }
}

/// INVARIANT: every measured MRD row resolves to a real FoM field + a live oracle tag.
#[test]
fn every_measured_mrd_row_resolves_to_field_and_oracle() {
    for row in mrd_table() {
        if row.availability == Availability::Measured {
            assert!(
                !row.field.is_empty(),
                "measured MRD row {} has no FoM field",
                row.id
            );
            assert!(
                resolve_verification(row.verification_requirement).is_some(),
                "measured MRD row {} does not resolve to a live verification row",
                row.id
            );
        }
    }
}

/// INVARIANT (defence in depth): no measured MRD row is backed by the not-implemented
/// PositioningFom — i.e. nothing claims a position-domain number as measured.
#[test]
fn no_measured_row_is_backed_by_unimplemented_positioning() {
    for row in mrd_table() {
        if row.availability == Availability::Measured {
            assert_ne!(
                row.source,
                FomSource::PositioningFom,
                "measured MRD row {} must not use the not-implemented PositioningFom",
                row.id
            );
        }
    }
}
