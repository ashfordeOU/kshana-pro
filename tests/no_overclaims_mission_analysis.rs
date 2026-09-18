// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
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

/// INVARIANT: the position-domain rows are classified against what the engine
/// actually does.
///
/// This guard used to assert `positioning_performance()` was still an honest `Err`.
/// The 0.26 engine implements it, so that assertion fired exactly as designed — it
/// was the MRD demanding to be revisited, not a broken test. Silencing it would have
/// left the deliverable marking a live, externally-validated capability as
/// "not available". The invariant is now the one that survives the capability
/// landing: a position row is measured only if it names a field the struct really
/// has AND resolves to a live verification row; a follow-on row claims neither.
#[test]
fn position_domain_rows_are_classified_against_the_live_engine() {
    // The capability is live: a unit ENU covariance yields the textbook medians.
    let p = kshana::fom::positioning_performance(
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        7.5,
    );
    assert!(
        p.cep_m.is_finite() && p.sep_m.is_finite() && p.drms2_m.is_finite(),
        "positioning_performance() must return finite position-domain FoMs"
    );
    assert!(
        p.sep_m >= p.cep_m,
        "SEP ({}) must not fall below CEP ({})",
        p.sep_m,
        p.cep_m
    );

    let position_sources = [FomSource::PositioningFom, FomSource::SbasProtectionLevel];
    let mut measured = 0usize;
    for row in mrd_table() {
        if !position_sources.contains(&row.source) {
            continue;
        }
        match row.availability {
            Availability::Measured => {
                measured += 1;
                assert!(
                    !row.field.is_empty(),
                    "measured position row {} names no field",
                    row.id
                );
                assert!(
                    resolve_verification(row.verification_requirement).is_some(),
                    "measured position row {} does not resolve to a live verification row ('{}')",
                    row.id,
                    row.verification_requirement
                );
            }
            Availability::PartnerFollowOn => {
                assert!(
                    row.field.is_empty() && row.verification_requirement.is_empty(),
                    "follow-on position row {} must claim no field and no oracle",
                    row.id
                );
            }
        }
    }
    assert!(
        measured >= 5,
        "the CEP/SEP/2DRMS/HPL/VPL rows must be measured against the 0.26 engine, got {measured}"
    );
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

/// INVARIANT (defence in depth): the MRD never credits a pass-through as a
/// computation.
///
/// `PositioningFom.hpl_m` does not compute a protection level — it carries whatever
/// the caller passed in. So no row may claim HPL or VPL *from `PositioningFom`*; the
/// HPL/VPL rows must be sourced to `SbasProtectionLevel`, the DO-229E monitor that
/// actually derives them. This is the specific overclaim the old guard was reaching
/// for, stated precisely enough to survive the capability landing.
#[test]
fn no_row_credits_a_pass_through_as_a_computation() {
    for row in mrd_table() {
        if row.source == FomSource::PositioningFom {
            assert!(
                !matches!(row.field, "hpl_m" | "vpl_m"),
                "MRD row {} sources '{}' from PositioningFom, which only passes a caller-supplied \
                 protection level through; source it to SbasProtectionLevel instead",
                row.id,
                row.field
            );
        }
    }
}
