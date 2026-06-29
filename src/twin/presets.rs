// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L1 device preset library
//!
//! Named [`DeviceSpec`] presets the L1 twin framework can calibrate against without
//! the caller hand-typing an Allan-deviation budget. Each preset returns the same
//! public input the framework already accepts — published `(τ, σ_y)` points plus a
//! deterministic aging rate — so calibrating a preset is exactly calibrating any
//! other [`DeviceSpec`] (no special path).
//!
//! ## Honesty of the numbers (read this before quoting one)
//!
//! Two provenance tiers, both honest, never mixed:
//!
//! - **Datasheet-cited** — values that genuinely appear on the manufacturer's public
//!   datasheet. Today that is the **Microchip/Microsemi SA.45s CSAC** short-τ Allan
//!   trio (`3e-10 / 1e-10 / 3e-11` at `1 / 10 / 100 s`), which is the canonical
//!   published spec for that part. The provenance string names the datasheet.
//! - **Representative, datasheet-class / published-curve** — order-of-magnitude
//!   values that match the *published behaviour* of that device class (shape, decade
//!   levels, long-τ branch) without claiming any one unit's exact certified numbers.
//!   This is the same honest framing the open core uses for its
//!   [`kshana::clock_state::ClockClass`] and [`kshana::holdover::QuantumClockClass`]
//!   reference figures, and the existing "representative CSAC-class" demonstrator
//!   spec in [`crate::twin`]. **No fabricated fake-precise datasheet numbers.**
//!
//! Calibrating a preset and reproducing its budget is a **calibration fit (Modelled)**
//! — a fit of the open `ClockModel` to published behaviour, *not* an
//! externally-validated measurement of a physical unit.
//!
//! References for the representative class figures: Riley, *Handbook of Frequency
//! Stability Analysis* (NIST SP 1065); Ludlow et al., *Optical atomic clocks*
//! (Rev. Mod. Phys. 87, 2015); the published Galileo PHM / RAFS on-board-clock Allan
//! curves (ESA/Leonardo-Spectratime class, e.g. Droz et al., EFTF 2009).

use crate::twin::DeviceSpec;

/// The provenance tier of a preset's numbers — surfaced so a caller can tell a
/// genuinely datasheet-cited value from a representative class figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceTier {
    /// At least the short-τ values are taken from the manufacturer's public datasheet.
    DatasheetCited,
    /// Order-of-magnitude values matching the device class's *published behaviour*,
    /// not any one unit's certified numbers.
    RepresentativeClass,
}

impl ProvenanceTier {
    /// Short tag for reports.
    pub fn tag(self) -> &'static str {
        match self {
            ProvenanceTier::DatasheetCited => "DATASHEET-CITED",
            ProvenanceTier::RepresentativeClass => "REPRESENTATIVE-CLASS",
        }
    }
}

/// A named device preset: the [`DeviceSpec`] to calibrate, its provenance tier, and a
/// **stated tolerance** within which the calibrated twin's analytic ADEV budget is
/// expected to reproduce the published points ([`crate::twin::fit_report`]). The
/// tolerance is honest headroom for the model-vs-published-class mismatch, not a
/// precision claim.
#[derive(Debug, Clone)]
pub struct DevicePreset {
    pub key: &'static str,
    pub tier: ProvenanceTier,
    pub spec: DeviceSpec,
    /// Reproduction tolerance for [`crate::twin::fit_report`] (fractional rel-err).
    pub fit_tolerance: f64,
}

/// **CSAC — Microchip/Microsemi SA.45s** chip-scale atomic clock. The short-τ Allan
/// trio (`3e-10 / 1e-10 / 3e-11` at `1 / 10 / 100 s`) is the canonical published
/// datasheet spec; the `1000 s` point continues the ~τ^-1/2 roll toward the flicker
/// floor (representative). Aging is the datasheet's `< 9e-10 / month` (≈ `3.5e-16/s`).
pub fn csac_sa45s() -> DevicePreset {
    DevicePreset {
        key: "csac-sa45s",
        tier: ProvenanceTier::DatasheetCited,
        spec: DeviceSpec {
            device: "CSAC Microchip SA.45s".into(),
            provenance: "Microchip/Microsemi SA.45s CSAC datasheet ADEV (3e-10/1e-10/3e-11 @ 1/10/100 s); 1000 s point representative".into(),
            adev_points: vec![
                (1.0, 3.0e-10),
                (10.0, 1.0e-10),
                (100.0, 3.0e-11),
                (1_000.0, 1.0e-11),
            ],
            drift_per_s: 3.5e-16, // ~9e-10/month aging (datasheet class)
        },
        fit_tolerance: 0.25,
    }
}

/// **OCXO class** — an oven-controlled crystal oscillator. Representative bathtub
/// curve: white FM down from `~1e-12 @ 1 s` to a `~3e-13` flicker floor near
/// `10–100 s`, then a random-walk-FM / aging upturn back up by `1e4 s`. Order-of-
/// magnitude, not a specific part — exercises the 3-parameter long-τ term.
pub fn ocxo_class() -> DevicePreset {
    DevicePreset {
        key: "ocxo-class",
        tier: ProvenanceTier::RepresentativeClass,
        spec: DeviceSpec {
            device: "OCXO (representative class)".into(),
            provenance: "representative OCXO-class published-behaviour ADEV (white FM + flicker floor + long-tau RWFM upturn); order-of-magnitude, not a specific part".into(),
            adev_points: vec![
                (1.0, 1.0e-12),
                (10.0, 3.8e-13),
                (100.0, 3.0e-13),
                (1_000.0, 6.6e-13),
                (10_000.0, 2.0e-12),
            ],
            drift_per_s: 1.0e-13, // representative crystal aging
        },
        fit_tolerance: 0.10,
    }
}

/// **Rb — SRS PRS10** rubidium frequency standard. Datasheet-class representative
/// Allan budget: `~2e-11 @ 1 s` rolling toward a `~1e-12` floor by `100–1000 s`,
/// with a long-τ magnetometer/aging branch. Marked representative datasheet-class —
/// the short-τ level matches the published PRS10 behaviour without claiming a
/// certified per-unit curve.
pub fn rb_prs10() -> DevicePreset {
    DevicePreset {
        key: "rb-prs10",
        tier: ProvenanceTier::RepresentativeClass,
        spec: DeviceSpec {
            device: "Rb SRS PRS10".into(),
            provenance: "SRS PRS10 Rb standard, representative datasheet-class ADEV (~2e-11 @ 1 s toward ~1e-12 floor + long-tau branch); order-of-magnitude".into(),
            adev_points: vec![
                (1.0, 2.0e-11),
                (10.0, 6.0e-12),
                (100.0, 2.0e-12),
                (1_000.0, 1.2e-12),
                (10_000.0, 2.0e-12),
            ],
            drift_per_s: 4.0e-18, // ~5e-11/month aging class
        },
        fit_tolerance: 0.15,
    }
}

/// **Galileo PHM** — the passive hydrogen maser flown on Galileo. Representative
/// published-curve ADEV: `~7e-13 @ 1 s` averaging down to `~1e-14` by `1e4 s` (the
/// famous ~10⁻¹⁴-over-10⁴-s maser stability). Representative class, not a flight
/// unit's certified curve.
pub fn galileo_phm() -> DevicePreset {
    DevicePreset {
        key: "galileo-phm",
        tier: ProvenanceTier::RepresentativeClass,
        spec: DeviceSpec {
            device: "Galileo Passive Hydrogen Maser".into(),
            provenance: "Galileo PHM, representative published-curve ADEV (~7e-13 @ 1 s to ~1e-14 @ 1e4 s; ESA/Leonardo-Spectratime class); order-of-magnitude".into(),
            adev_points: vec![
                (1.0, 7.0e-13),
                (10.0, 2.2e-13),
                (100.0, 7.1e-14),
                (1_000.0, 2.4e-14),
                (10_000.0, 1.2e-14),
                (100_000.0, 1.0e-14),
            ],
            drift_per_s: 0.0,
        },
        fit_tolerance: 0.15,
    }
}

/// **Galileo RAFS** — the rubidium atomic frequency standard flown on Galileo.
/// Representative published-curve ADEV: `~3e-12 @ 1 s` averaging to a few `1e-14` by
/// `1e4 s`. Representative class.
pub fn galileo_rafs() -> DevicePreset {
    DevicePreset {
        key: "galileo-rafs",
        tier: ProvenanceTier::RepresentativeClass,
        spec: DeviceSpec {
            device: "Galileo Rb Atomic Frequency Standard".into(),
            provenance: "Galileo RAFS, representative published-curve ADEV (~3e-12 @ 1 s to ~5e-14 @ 1e4 s; ESA/Leonardo-Spectratime class); order-of-magnitude".into(),
            adev_points: vec![
                (1.0, 3.0e-12),
                (10.0, 1.0e-12),
                (100.0, 3.0e-13),
                (1_000.0, 1.0e-13),
                (10_000.0, 5.0e-14),
            ],
            drift_per_s: 0.0,
        },
        fit_tolerance: 0.10,
    }
}

/// **Sr/Yb optical-lattice class** — a neutral-atom optical-lattice clock. Open-
/// literature order-of-magnitude ADEV: `~5e-16 @ 1 s` averaging as τ^-1/2 toward the
/// `~1e-18` regime by `1e4 s` (Ludlow et al., Rev. Mod. Phys. 2015; BACON, Nature
/// 2021). Matches the open core's [`kshana::holdover::QuantumClockClass::OpticalLattice`]
/// `5e-16 @ 1 s`. Representative class — not any one apparatus.
pub fn optical_lattice_class() -> DevicePreset {
    DevicePreset {
        key: "optical-lattice",
        tier: ProvenanceTier::RepresentativeClass,
        spec: DeviceSpec {
            device: "Sr/Yb optical-lattice clock (representative class)".into(),
            provenance: "Sr/Yb optical-lattice class, representative published order-of-magnitude ADEV (~5e-16 @ 1 s averaging ~tau^-1/2; Ludlow et al. RMP 2015, BACON Nature 2021)".into(),
            adev_points: vec![
                (1.0, 5.0e-16),
                (10.0, 1.6e-16),
                (100.0, 5.0e-17),
                (1_000.0, 1.6e-17),
                (10_000.0, 5.0e-18),
            ],
            drift_per_s: 0.0,
        },
        fit_tolerance: 0.15,
    }
}

/// Every preset in the library, in a stable order (for sweeps and coverage checks).
pub fn all_presets() -> Vec<DevicePreset> {
    vec![
        csac_sa45s(),
        ocxo_class(),
        rb_prs10(),
        galileo_phm(),
        galileo_rafs(),
        optical_lattice_class(),
    ]
}

/// Look a preset up by its stable `key` (the id used in scenarios/registry wiring).
pub fn preset_by_key(key: &str) -> Option<DevicePreset> {
    all_presets().into_iter().find(|p| p.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twin::{calibrate, calibrate3, fit_report, validate};

    #[test]
    fn every_preset_calibrates_and_reproduces_its_budget() {
        // Per-device contract: the 3-parameter calibration reproduces each preset's
        // published ADEV budget (including its long-τ branch) within the preset's
        // stated tolerance, checked analytically (model_adev vs published). This is a
        // calibration FIT (Modelled), not a Validated measurement of a physical unit.
        for p in all_presets() {
            let twin = calibrate3(&p.spec).expect(p.key);
            // Physical: every fitted PSD non-negative and finite.
            assert!(twin.q_wf >= 0.0 && twin.q_wf.is_finite(), "{} q_wf", p.key);
            assert!(twin.q_rw >= 0.0 && twin.q_rw.is_finite(), "{} q_rw", p.key);
            assert!(
                twin.flicker_floor >= 0.0 && twin.flicker_floor.is_finite(),
                "{} flicker",
                p.key
            );
            let report = fit_report(&twin, &p.spec, p.fit_tolerance).expect(p.key);
            assert!(
                report.passed,
                "{}: twin should reproduce its ADEV budget within {:.0}%: max_rel_err={:.3} points={:?}",
                p.key,
                p.fit_tolerance * 100.0,
                report.max_rel_err,
                report.points
            );
            // Provenance must be non-empty and self-describing.
            assert!(!p.spec.provenance.is_empty(), "{} provenance", p.key);
        }
    }

    #[test]
    fn presets_with_long_tau_upturn_need_three_params() {
        // The OCXO bathtub has a genuine long-τ RWFM upturn: the 3-parameter fit must
        // recover a non-zero q_rw, and must reproduce the budget materially better
        // than the 2-parameter fit (which structurally cannot bend back up).
        let p = ocxo_class();
        let twin3 = calibrate3(&p.spec).unwrap();
        assert!(
            twin3.q_rw > 0.0,
            "OCXO long-τ upturn must map to a positive q_rw, got {}",
            twin3.q_rw
        );
        let twin2 = calibrate(&p.spec).unwrap();
        let err3 = fit_report(&twin3, &p.spec, 1.0).unwrap().max_rel_err;
        let err2 = fit_report(&twin2, &p.spec, 1.0).unwrap().max_rel_err;
        assert!(
            err3 < err2,
            "3-param fit ({err3:.3}) should beat 2-param ({err2:.3}) on an upturn curve"
        );
    }

    #[test]
    fn csac_is_datasheet_cited_the_rest_are_representative() {
        // Honesty firewall: only the SA.45s is tagged datasheet-cited; the class
        // figures are explicitly representative.
        assert_eq!(csac_sa45s().tier, ProvenanceTier::DatasheetCited);
        for p in all_presets().into_iter().filter(|p| p.key != "csac-sa45s") {
            assert_eq!(
                p.tier,
                ProvenanceTier::RepresentativeClass,
                "{} must be representative-class",
                p.key
            );
        }
    }

    #[test]
    fn csac_preset_reproduces_through_the_open_engine_synthesis() {
        // Stronger route for one preset: synthesise the open ClockModel and re-estimate
        // its ADEV with kshana::allan (the same MC validate() the framework ships).
        // Still Modelled — it is the open engine reproducing its own calibrated curve,
        // not an external oracle. Kept to the short-τ datasheet trio so the MC has
        // enough averaging at modest sample counts.
        let p = csac_sa45s();
        let twin = calibrate3(&p.spec).unwrap();
        let report = validate(&twin, &p.spec, 0.25, 1.0, 200_000, 7).unwrap();
        assert!(
            report.passed,
            "CSAC twin should reproduce its budget through engine synthesis within 25%: max_rel_err={:.3}",
            report.max_rel_err
        );
    }

    #[test]
    fn preset_lookup_round_trips() {
        for p in all_presets() {
            assert_eq!(preset_by_key(p.key).unwrap().key, p.key);
        }
        assert!(preset_by_key("no-such-device").is_none());
    }
}
