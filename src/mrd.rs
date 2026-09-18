// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Machine-checked Mission Requirements Document skeleton (A03, D1.4)
//!
//! A small, *checked* requirement table that maps each PNT requirement to:
//! 1. the tender's own figure-of-merit vocabulary,
//! 2. the concrete open-core field that **measures** it
//!    ([`kshana::fom::FoMScores`] / [`kshana::walker::CoverageReport`] /
//!    [`kshana::orbit::Dop`] / [`kshana::fom::PositioningFom`] /
//!    [`kshana::sbas::SbasProtectionLevel`]), and
//! 3. the [`kshana::verification`] matrix row + its `OracleKind` that backs it.
//!
//! The table is encoded in code and guarded by unit tests, so the MRD **cannot
//! drift from the engine**: every measured requirement is asserted to resolve to a
//! real, serialised FoM field and to a live verification-matrix row, and every
//! follow-on requirement is asserted to claim no field and no oracle.
//!
//! ## Position-domain status (revised against the 0.26 engine)
//! Position-domain FoMs used to be a flat gap: `positioning_performance()` was a
//! deliberate not-implemented `Err` and all four position rows were marked
//! [`Availability::PartnerFollowOn`]. **That is no longer true.** The open core now
//! ships `positioning_performance(cov_enu, hpl_m) -> PositioningFom`, computing CEP
//! and SEP as the *exact* median radial errors (elliptical / ellipsoidal Gaussian
//! quantiles by quadrature + bisection — not the `0.589·(σ₁+σ₂)` linear rule) and
//! 2DRMS in closed form. That kernel is externally validated in the open core's
//! `tests/positioning_fom_reference.rs` against SciPy (`rayleigh` / `maxwell`
//! medians, isotropic) and an independent 6 M-draw NumPy Monte-Carlo median
//! (anisotropic).
//!
//! The rows are therefore now `Measured` — but each resolves to the evidence that
//! actually *binds* it, which is the honest part:
//!
//! * **CEP / SEP / 2DRMS** are a transform *of a covariance*. The transform is
//!   externally validated; the 3×3 ENU covariance feeding it comes from the GNSS/INS
//!   fusion EKF, whose verification row is **MODELLED** (its trajectory truth and
//!   sensor calibration are not externally validated). These rows therefore resolve
//!   to the fusion row — the weaker, binding evidence — because resolving them to
//!   the SciPy-validated kernel would dress a modelled input in a validated halo.
//! * **HPL / VPL** are not computed by `PositioningFom` at all:
//!   `PositioningFom.hpl_m` is a *pass-through* of a protection level the caller
//!   supplies. The real producer is the DO-229E weighted-LS monitor
//!   [`kshana::sbas::sbas_protection_level`], so those rows are sourced to
//!   [`kshana::sbas::SbasProtectionLevel`] and resolve to its **VALIDATED** row.
//!
//! ## The gap that remains (enforced by tests)
//! Every protection level in the engine — SBAS DO-229E, ARAIM MHSS, the lunar
//! service-volume PL — is computed from **snapshot GNSS geometry**. None of them
//! bounds the error of the *fused* INS+GNSS solution while it coasts through a GNSS
//! outage. `MRD-INT-04` records that honestly, and a test fails if any follow-on row
//! ever acquires a field or an oracle without being re-classified.

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
    /// Position-domain accuracy ([`kshana::fom::PositioningFom`]) — CEP / SEP /
    /// 2DRMS from a 3×3 ENU position-error covariance.
    PositioningFom,
    /// DO-229E weighted-least-squares protection levels
    /// ([`kshana::sbas::SbasProtectionLevel`]) — the actual HPL/VPL producer.
    SbasProtectionLevel,
}

/// Whether the requirement is measured by the current engine, or deferred to a
/// partner / follow-on because no engine path produces it. A `PartnerFollowOn` row
/// is asserted by test to claim no measuring field and no verification row.
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
/// `PartnerFollowOn` row must claim no field and no oracle — an honest gap.
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
        // ── Position-domain requirements ────────────────────────────────────────
        // Measured since the 0.26 engine. Each row resolves to the evidence that
        // BINDS it, not to the strongest evidence anywhere in its call chain — see
        // the module docs for why CEP/SEP/2DRMS point at the MODELLED fusion row
        // even though their kernel is externally validated.
        MrdRequirement {
            id: "MRD-POS-01",
            requirement: "Position accuracy: Circular Error Probable (CEP, horizontal)",
            tender_fom: "horizontal position accuracy",
            source: FomSource::PositioningFom,
            field: "cep_m",
            verification_requirement: "GNSS/INS sensor fusion",
            availability: Measured,
            note:
                "Exact elliptical-Gaussian median radial error (reduces to the Rayleigh median \
                   σ√(2ln2) when isotropic), NOT the 0.589·(σ₁+σ₂) linear rule. The kernel is \
                   externally validated vs SciPy/NumPy; this row resolves to the fusion row \
                   because the covariance it transforms is MODELLED — that is the binding evidence.",
        },
        MrdRequirement {
            id: "MRD-POS-02",
            requirement: "Position accuracy: Spherical Error Probable (SEP, 3-D)",
            tender_fom: "3-D position accuracy",
            source: FomSource::PositioningFom,
            field: "sep_m",
            verification_requirement: "GNSS/INS sensor fusion",
            availability: Measured,
            note: "Exact ellipsoidal-Gaussian median 3-D radial error (reduces to the Maxwell \
                   median ≈1.5382σ when isotropic). Same externally-validated kernel, same \
                   MODELLED covariance input.",
        },
        MrdRequirement {
            id: "MRD-POS-03",
            requirement: "Position accuracy: twice distance RMS (2DRMS, ~95% horizontal)",
            tender_fom: "horizontal position accuracy (95%)",
            source: FomSource::PositioningFom,
            field: "drms2_m",
            verification_requirement: "GNSS/INS sensor fusion",
            availability: Measured,
            note: "Closed form 2√(σ_E²+σ_N²). It is a ~95% horizontal bound only for a \
                   near-circular error ellipse — for an eccentric ellipse the true 95% radius \
                   differs. Inherits the MODELLED covariance.",
        },
        MrdRequirement {
            id: "MRD-INT-02",
            requirement: "Aviation integrity: Horizontal Protection Level (HPL)",
            tender_fom: "integrity (HPL)",
            source: FomSource::SbasProtectionLevel,
            field: "hpl_m",
            verification_requirement: "SBAS protection level (DO-229E weighted-LS HPL/VPL)",
            availability: Measured,
            note: "Sourced to the PRODUCER, not to PositioningFom.hpl_m — that field merely \
                   carries a protection level the caller supplies, so claiming it would credit a \
                   pass-through as a computation. Snapshot GNSS geometry; see MRD-INT-04 for the \
                   fused/coasting gap.",
        },
        MrdRequirement {
            id: "MRD-INT-03",
            requirement: "Aviation integrity: Vertical Protection Level (VPL)",
            tender_fom: "integrity (VPL)",
            source: FomSource::SbasProtectionLevel,
            field: "vpl_m",
            verification_requirement: "SBAS protection level (DO-229E weighted-LS HPL/VPL)",
            availability: Measured,
            note: "Optional: None in En-Route/NPA mode, where DO-229E defines no vertical \
                   protection level. PositioningFom carries no VPL field at all, which is why \
                   this row exists separately. Snapshot GNSS geometry; see MRD-INT-04.",
        },
        // ── The honest position-domain gap that remains ─────────────────────────
        MrdRequirement {
            id: "MRD-INT-04",
            requirement: "Aviation integrity: protection level on the fused INS+GNSS solution \
                          while coasting through a GNSS outage",
            tender_fom: "integrity (HPL/VPL) under GNSS denial",
            source: FomSource::PositioningFom,
            field: "",
            verification_requirement: "",
            availability: PartnerFollowOn,
            note: "Every protection level in the engine (SBAS DO-229E, ARAIM MHSS, lunar \
                   service-volume) is computed from snapshot GNSS geometry. None bounds the error \
                   of the coasting fused solution, so during an outage there is no engine path to \
                   fill PositioningFom.hpl_m at all. Partner/follow-on.",
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
         every follow-on row claims no field and no oracle. The bracketed tag is the \
         evidence class of the verification row that BINDS the requirement, not the \
         strongest evidence anywhere in its call chain._\n\n",
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
        "\n_Position-domain accuracy (CEP/SEP/2DRMS) is measured by \
         `kshana::fom::positioning_performance`. Its kernel is externally validated (SciPy/NumPy), \
         but each row resolves to the MODELLED fusion row because the covariance it transforms is \
         modelled — the binding evidence, not the strongest in the chain. HPL/VPL are sourced to \
         the DO-229E monitor that actually produces them, since `PositioningFom.hpl_m` is a \
         pass-through. A protection level on the coasting fused INS+GNSS solution remains an \
         honest follow-on (MRD-INT-04). \
         This MRD is a DESIGN skeleton pending the real FPD-10030 SRD, not an agreed requirement set._\n",
    );
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use kshana::fom::{positioning_performance, FoMScores, PositioningFom};
    use kshana::orbit::Dop;
    use kshana::sbas::SbasProtectionLevel;
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

    fn sbas_sample() -> serde_json::Value {
        serde_json::to_value(SbasProtectionLevel {
            n_used: 8,
            d_major_m: 1.0,
            d_u_m: 1.0,
            hpl_m: 1.0,
            vpl_m: Some(1.0),
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
                FomSource::SbasProtectionLevel => sbas_sample(),
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
    fn follow_on_rows_claim_no_field_and_no_oracle() {
        // The honesty guard, re-pointed for the 0.26 engine. The old version froze the
        // position rows as partner/follow-on, which became a lie once the engine shipped
        // positioning_performance(). The invariant that actually matters is symmetric and
        // survives the capability landing: a row may be marked follow-on ONLY if it claims
        // nothing. That stops a real capability being parked as a gap, and stops a gap row
        // quietly acquiring a field without being re-classified.
        for r in mrd_table() {
            if r.availability == Availability::PartnerFollowOn {
                assert!(
                    r.field.is_empty(),
                    "follow-on MRD row {} must not name a measuring field (got '{}')",
                    r.id,
                    r.field
                );
                assert!(
                    r.verification_requirement.is_empty(),
                    "follow-on MRD row {} must not claim a verification row (got '{}')",
                    r.id,
                    r.verification_requirement
                );
            }
        }
    }

    #[test]
    fn positioning_performance_is_live_and_matches_its_documented_quantiles() {
        // The replacement tripwire. The old test asserted positioning_performance() was
        // an honest Err; the 0.26 engine implements it, so the guard now pins the exact
        // SEMANTICS the MRD rows claim. It does the same job in the other direction: if
        // the engine reverts to a stub, redefines CEP/SEP as the 0.589·(σ₁+σ₂) linear
        // approximation, or stops passing HPL through, this fails and the MRD is revisited.
        let iso = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let p = positioning_performance(iso, 12.5);

        // Isotropic unit covariance collapses to the two textbook medians. The linear
        // 0.589·(σ₁+σ₂) rule would give 1.178 for CEP — close — but 1.178 for SEP too,
        // which is 23% low, so the SEP assertion is what actually catches a regression
        // to the approximation.
        assert!(
            (p.cep_m - 1.177_410_02).abs() < 5e-3,
            "CEP must reduce to the Rayleigh median σ√(2ln2) ≈ 1.1774σ, got {}",
            p.cep_m
        );
        assert!(
            (p.sep_m - 1.538_17).abs() < 5e-3,
            "SEP must reduce to the Maxwell median ≈ 1.5382σ, got {}",
            p.sep_m
        );
        // 2DRMS is the closed form 2√(σ_E²+σ_N²) = 2√2 for unit variances.
        assert!(
            (p.drms2_m - 2.0 * std::f64::consts::SQRT_2).abs() < 1e-12,
            "2DRMS must be the closed form 2√(σ_E²+σ_N²), got {}",
            p.drms2_m
        );
        // HPL is a PASS-THROUGH, not a computation. This is precisely why MRD-INT-02 is
        // sourced to SbasProtectionLevel rather than to PositioningFom.
        assert_eq!(
            p.hpl_m, 12.5,
            "PositioningFom.hpl_m must pass the caller's protection level through unchanged"
        );
        // Ordering sanity: a 3-D median radial error is never below the horizontal one.
        assert!(
            p.sep_m >= p.cep_m,
            "SEP ({}) must not fall below CEP ({})",
            p.sep_m,
            p.cep_m
        );
    }

    #[test]
    fn every_measured_row_names_a_serialised_field() {
        // Defence in depth for the replacement of no_measured_row_uses_positioning_fom:
        // PositioningFom is a legitimate source now, so the guard that matters is that a
        // Measured row always names a real field (checked against the struct above) rather
        // than claiming availability with nothing behind it.
        for r in mrd_table() {
            if r.availability == Availability::Measured {
                assert!(
                    !r.field.is_empty(),
                    "measured MRD row {} names no serialised FoM field",
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
        assert!(
            measured >= 14,
            "expected timing + geometry + position-domain FoMs measured, got {measured}"
        );
        assert_eq!(
            followon, 1,
            "exactly one honest gap remains: a PL on the coasting fused solution (MRD-INT-04)"
        );
        // The position domain must stay represented — catches a row being deleted rather
        // than re-classified.
        for id in [
            "MRD-POS-01",
            "MRD-POS-02",
            "MRD-POS-03",
            "MRD-INT-02",
            "MRD-INT-03",
        ] {
            let r = table
                .iter()
                .find(|r| r.id == id)
                .expect("position row present");
            assert_eq!(
                r.availability,
                Availability::Measured,
                "{id} must be measured against the 0.26 engine"
            );
        }
    }
}
