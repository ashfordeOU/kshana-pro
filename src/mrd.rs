// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # Machine-checked Mission Requirements Document skeleton (A03, D1.4)
//!
//! A small, *checked* requirement table that maps each PNT requirement to:
//! 1. the tender's own figure-of-merit vocabulary,
//! 2. the concrete open-core field that **measures** it
//!    ([`kshana::fom::FoMScores`] / [`kshana::walker::CoverageReport`] /
//!    [`kshana::orbit::Dop`] / [`kshana::fom::PositioningFom`]), and
//! 3. the [`kshana::verification`] matrix row + its `OracleKind` that backs it.
//!
//! The table is encoded in code and guarded by unit tests, so the MRD **cannot
//! drift from the engine**: every measured requirement is asserted to resolve to a
//! real, serialised FoM field and to a live verification-matrix row, and every
//! position-domain requirement is asserted to stay flagged *partner/follow-on*
//! (because [`kshana::fom::positioning_performance`] is a deliberate not-implemented
//! `Err`).
//!
//! ## Honesty guard (enforced by tests)
//! Position-domain FoMs (CEP / SEP / 2DRMS / HPL) are **not implemented** in the
//! open core — they need a 2-D/3-D INS fusion solution and a multi-measurement
//! integrity monitor. The MRD marks them [`Availability::PartnerFollowOn`] and a
//! test fails if any of them is ever upgraded to [`Availability::Measured`] or if
//! `positioning_performance()` stops returning `Err`. Only timing-domain and
//! geometry FoMs may be claimed as measured.

use kshana::verification::{verification_matrix, OracleKind, VerificationStatus};
use serde::Serialize;

/// Which open-core struct carries the measuring field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FomSource {
    /// Timing-domain figures of merit ([`kshana::fom::FoMScores`]).
    FoMScores,
    /// Geometry coverage/revisit ([`kshana::walker::CoverageReport`]).
    CoverageReport,
    /// Geometry dilution of precision ([`kshana::orbit::Dop`]).
    Dop,
    /// Position-domain accuracy/integrity ([`kshana::fom::PositioningFom`]) — a
    /// deliberate not-implemented stub.
    PositioningFom,
}

/// Whether the requirement is measured by the current engine, or deferred to a
/// partner / follow-on because the capability is not implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Availability {
    /// The engine produces this FoM today.
    Measured,
    /// Not available in this release; partner-owned or a documented follow-on.
    PartnerFollowOn,
}

/// One MRD row.
#[derive(Debug, Clone, Serialize)]
pub struct MrdRequirement {
    pub id: &'static str,
    /// The PNT requirement, in plain language.
    pub requirement: &'static str,
    /// The tender's own FoM-vocabulary term this maps to.
    pub tender_fom: &'static str,
    /// Which open-core struct measures it.
    pub source: FomSource,
    /// The exact serialised field name in that struct (empty for follow-on rows
    /// with no measuring field).
    pub field: &'static str,
    /// The verification-matrix `requirement` row that backs it (empty for a
    /// follow-on capability that has no validated/modelled row yet).
    pub verification_requirement: &'static str,
    pub availability: Availability,
    pub note: &'static str,
}

/// The curated MRD requirement table. Each `Measured` row is checked by the unit
/// tests to resolve to a real serialised field AND a live verification row; each
/// `PartnerFollowOn` row documents an honest gap.
pub fn mrd_table() -> Vec<MrdRequirement> {
    use Availability::*;
    use FomSource::*;
    vec![
        MrdRequirement {
            id: "MRD-PNT-01",
            requirement: "Timing accuracy: clock-phase error during GNSS outage (RMS)",
            tender_fom: "ranging/timing accuracy",
            source: FoMScores,
            field: "timing_rms_ns",
            verification_requirement: "GNSS-denied clock holdover",
            availability: Measured,
            note: "Timing-domain (nanoseconds), not a position number.",
        },
        MrdRequirement {
            id: "MRD-PNT-02",
            requirement: "Timing accuracy: 95th-percentile clock-phase error",
            tender_fom: "ranging/timing accuracy (95%)",
            source: FoMScores,
            field: "timing_p95_ns",
            verification_requirement: "GNSS-denied clock holdover",
            availability: Measured,
            note: "Timing-domain tail metric.",
        },
        MrdRequirement {
            id: "MRD-RES-01",
            requirement: "Holdover: in-spec coast duration under GNSS denial",
            tender_fom: "resilience / holdover",
            source: FoMScores,
            field: "holdover_s",
            verification_requirement: "GNSS-denied clock holdover",
            availability: Measured,
            note: "Grid-bounded lower bound at the time-step resolution.",
        },
        MrdRequirement {
            id: "MRD-RES-02",
            requirement: "Resilience-vs-time: error growth rate during outage",
            tender_fom: "resilience-vs-time",
            source: FoMScores,
            field: "resilience_slope_ns_per_s",
            verification_requirement:
                "Quantum-vs-classical PNT trade & GNSS-denied resilience (13503)",
            availability: Measured,
            note: "Least-squares |error| slope; modelled.",
        },
        MrdRequirement {
            id: "MRD-AVL-01",
            requirement: "Service availability: fraction of run with an in-spec solution",
            tender_fom: "PNT availability",
            source: FoMScores,
            field: "availability",
            verification_requirement: "GNSS-denied clock holdover",
            availability: Measured,
            note: "Fraction over the whole run.",
        },
        MrdRequirement {
            id: "MRD-INT-01",
            requirement: "Filter self-consistency (Kalman k-sigma containment)",
            tender_fom: "estimator consistency",
            source: FoMScores,
            field: "integrity",
            verification_requirement: "Onboard clock state estimation",
            availability: Measured,
            note: "Self-consistency only — NOT aviation HPL/VPL/RAIM integrity (see MRD-INT-02).",
        },
        MrdRequirement {
            id: "MRD-SEC-01",
            requirement: "Spoof detectability bound from clock stability",
            tender_fom: "security / spoof detection",
            source: FoMScores,
            field: "security",
            verification_requirement: "Spoofing detection",
            availability: Measured,
            note: "Analytic detectability bound; meaningful only with a configured attack.",
        },
        MrdRequirement {
            id: "MRD-COV-01",
            requirement: "Coverage: fraction of time a user has a fix",
            tender_fom: "coverage / continuity",
            source: CoverageReport,
            field: "coverage_fraction",
            verification_requirement: "Orbit propagation & determination",
            availability: Measured,
            note: "Geometry derived from the externally-validated propagator; the coverage \
                   metric itself is a deterministic derivation, not separately dataset-validated.",
        },
        MrdRequirement {
            id: "MRD-COV-02",
            requirement: "Revisit: worst-case gap between fixes",
            tender_fom: "revisit interval",
            source: CoverageReport,
            field: "max_revisit_gap_s",
            verification_requirement: "Orbit propagation & determination",
            availability: Measured,
            note: "Geometry derived from the validated propagator.",
        },
        MrdRequirement {
            id: "MRD-GEO-01",
            requirement: "Geometry: position dilution of precision (PDOP)",
            tender_fom: "DOP / geometry quality",
            source: Dop,
            field: "pdop",
            verification_requirement: "Integrity (RAIM/ARAIM/SBAS)",
            availability: Measured,
            note: "Geometry; PDOP sanity-checked against real IGS data (tests/igs_real_data.rs).",
        },
        // ── Position-domain requirements — honest follow-on gaps ────────────────
        MrdRequirement {
            id: "MRD-POS-01",
            requirement: "Position accuracy: Circular Error Probable (CEP, horizontal)",
            tender_fom: "horizontal position accuracy",
            source: FomSource::PositioningFom,
            field: "cep_m",
            verification_requirement: "",
            availability: PartnerFollowOn,
            note: "Not implemented (positioning_performance() returns Err): needs 2-D/3-D INS \
                   fusion. Partner/follow-on.",
        },
        MrdRequirement {
            id: "MRD-POS-02",
            requirement: "Position accuracy: Spherical Error Probable (SEP, 3-D)",
            tender_fom: "3-D position accuracy",
            source: FomSource::PositioningFom,
            field: "sep_m",
            verification_requirement: "",
            availability: PartnerFollowOn,
            note: "Not implemented. Partner/follow-on.",
        },
        MrdRequirement {
            id: "MRD-POS-03",
            requirement: "Position accuracy: twice distance RMS (2DRMS, ~95% horizontal)",
            tender_fom: "horizontal position accuracy (95%)",
            source: FomSource::PositioningFom,
            field: "drms2_m",
            verification_requirement: "",
            availability: PartnerFollowOn,
            note: "Not implemented. Partner/follow-on.",
        },
        MrdRequirement {
            id: "MRD-INT-02",
            requirement: "Aviation integrity: Horizontal Protection Level (HPL)",
            tender_fom: "integrity (HPL/VPL)",
            source: FomSource::PositioningFom,
            field: "hpl_m",
            verification_requirement: "",
            availability: PartnerFollowOn,
            note: "Position-domain HPL not implemented (needs a multi-measurement integrity \
                   monitor on a position solution). The RAIM/ARAIM engine is validated for \
                   geometry availability separately, but a position-domain HPL is follow-on.",
        },
    ]
}

/// Resolve a verification-matrix `requirement` row to its (status, oracle).
pub fn resolve_verification(req: &str) -> Option<(VerificationStatus, OracleKind)> {
    if req.is_empty() {
        return None;
    }
    verification_matrix()
        .into_iter()
        .find(|it| it.requirement.contains(req))
        .map(|it| (it.status, it.oracle_kind))
}

/// Render the MRD as a Markdown table for the D1.4 deliverable draft.
pub fn to_markdown() -> String {
    let mut md = String::new();
    md.push_str("# Mission Requirements Document (D1.4) — FoM mapping skeleton\n\n");
    md.push_str(&format!(
        "_Machine-checked against Kshana Pro v{} / the open engine. Every measured \
         requirement resolves to a real FoM field and a live verification-matrix row; \
         position-domain requirements are honest follow-on gaps._\n\n",
        crate::version()
    ));
    md.push_str(
        "| ID | Requirement | Tender FoM | Kshana field | Verification row | Oracle | Availability |\n",
    );
    md.push_str(
        "|----|-------------|-----------|--------------|------------------|--------|--------------|\n",
    );
    for r in mrd_table() {
        let (status_tag, oracle) = match resolve_verification(r.verification_requirement) {
            Some((s, o)) => (s.tag(), format!("{o:?}")),
            None => ("—", "—".to_string()),
        };
        let avail = match r.availability {
            Availability::Measured => format!("MEASURED ({status_tag})"),
            Availability::PartnerFollowOn => "PARTNER / FOLLOW-ON".to_string(),
        };
        let field = if r.field.is_empty() {
            "—".to_string()
        } else {
            format!("`{:?}.{}`", r.source, r.field)
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            r.id,
            r.requirement,
            r.tender_fom,
            field,
            if r.verification_requirement.is_empty() {
                "—"
            } else {
                r.verification_requirement
            },
            oracle,
            avail,
        ));
    }
    md.push_str(
        "\n_Position-domain FoMs (CEP/SEP/2DRMS/HPL) are deliberately not implemented \
         (`kshana::fom::positioning_performance` returns `Err`) and are marked partner/follow-on. \
         This MRD is a DESIGN skeleton pending the real FPD-10030 SRD, not an agreed requirement set._\n",
    );
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use kshana::fom::{positioning_performance, FoMScores, PositioningFom};
    use kshana::orbit::Dop;
    use kshana::walker::CoverageReport;

    fn fom_sample() -> serde_json::Value {
        serde_json::to_value(FoMScores {
            timing_rms_ns: 1.0,
            timing_p95_ns: 1.0,
            holdover_s: 1.0,
            resilience_slope_ns_per_s: 1.0,
            availability: 1.0,
            integrity: Some(1.0),
            security: Some(1.0),
        })
        .unwrap()
    }

    fn coverage_sample() -> serde_json::Value {
        serde_json::to_value(CoverageReport {
            samples_total: 1,
            samples_covered: 1,
            coverage_fraction: 1.0,
            max_revisit_gap_s: 0.0,
            mean_revisit_gap_s: 0.0,
        })
        .unwrap()
    }

    fn dop_sample() -> serde_json::Value {
        serde_json::to_value(Dop {
            gdop: 1.0,
            pdop: 1.0,
            hdop: 1.0,
            vdop: 1.0,
            tdop: 1.0,
        })
        .unwrap()
    }

    fn positioning_sample() -> serde_json::Value {
        serde_json::to_value(PositioningFom {
            cep_m: 1.0,
            sep_m: 1.0,
            drms2_m: 1.0,
            hpl_m: 1.0,
        })
        .unwrap()
    }

    #[test]
    fn every_field_name_exists_in_its_source_struct() {
        // The MRD cannot reference a field the engine does not have.
        for r in mrd_table() {
            if r.field.is_empty() {
                continue;
            }
            let sample = match r.source {
                FomSource::FoMScores => fom_sample(),
                FomSource::CoverageReport => coverage_sample(),
                FomSource::Dop => dop_sample(),
                FomSource::PositioningFom => positioning_sample(),
            };
            assert!(
                sample.get(r.field).is_some(),
                "MRD row {} references field '{}' absent from {:?}",
                r.id,
                r.field,
                r.source
            );
        }
    }

    #[test]
    fn every_measured_row_resolves_to_a_live_verification_row() {
        for r in mrd_table() {
            if r.availability == Availability::Measured {
                assert!(
                    resolve_verification(r.verification_requirement).is_some(),
                    "measured MRD row {} does not resolve to a live verification row ('{}')",
                    r.id,
                    r.verification_requirement
                );
            }
        }
    }

    #[test]
    fn position_domain_requirements_stay_partner_followon() {
        // The honesty guard: nothing measured by PositioningFom may be claimed
        // available, because the capability is not implemented.
        for r in mrd_table() {
            if r.source == FomSource::PositioningFom {
                assert_eq!(
                    r.availability,
                    Availability::PartnerFollowOn,
                    "MRD row {} (PositioningFom) must be partner/follow-on, not measured",
                    r.id
                );
            }
        }
    }

    #[test]
    fn positioning_performance_is_still_an_honest_stub() {
        // If this ever returns Ok, the MRD's "not available" marking would be a lie —
        // this test forces the MRD to be revisited at that point.
        assert!(
            positioning_performance().is_err(),
            "positioning_performance() is now implemented — revisit the MRD-POS rows"
        );
    }

    #[test]
    fn no_measured_row_uses_positioning_fom() {
        for r in mrd_table() {
            if r.availability == Availability::Measured {
                assert_ne!(
                    r.source,
                    FomSource::PositioningFom,
                    "measured MRD row {} must not be backed by the not-implemented PositioningFom",
                    r.id
                );
            }
        }
    }

    #[test]
    fn markdown_renders_all_rows_with_availability() {
        let md = to_markdown();
        for r in mrd_table() {
            assert!(md.contains(r.id), "markdown missing row {}", r.id);
        }
        assert!(md.contains("MEASURED"));
        assert!(md.contains("PARTNER / FOLLOW-ON"));
    }

    #[test]
    fn table_covers_the_required_domains() {
        let table = mrd_table();
        let measured = table
            .iter()
            .filter(|r| r.availability == Availability::Measured)
            .count();
        let followon = table
            .iter()
            .filter(|r| r.availability == Availability::PartnerFollowOn)
            .count();
        assert!(measured >= 8, "expected the timing+geometry FoMs measured");
        assert_eq!(followon, 4, "the four position-domain FoMs are follow-on");
    }
}
