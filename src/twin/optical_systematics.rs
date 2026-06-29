// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L1 optical-lattice systematic-shift budget
//!
//! An optical-lattice clock's accuracy is set not by its short-term Allan deviation
//! but by how well every **systematic frequency shift** is evaluated and corrected.
//! This module is the per-effect budget table — each effect's fractional-frequency
//! shift `Δν/ν` and its 1-σ uncertainty — summed in quadrature into a total
//! systematic uncertainty, alongside the averaging-limited **statistical** floor
//! (Dick effect + quantum projection noise).
//!
//! ## Honesty (read before quoting a number)
//!
//! Every value here is a **representative published-budget order of magnitude** for a
//! Sr/Yb neutral-atom lattice clock — the genre of numbers in the field's review and
//! comparison literature (Ludlow et al., *Optical atomic clocks*, Rev. Mod. Phys. 87,
//! 2015; the BACON collaboration clock comparison, *Nature* 591, 2021; Bloom et al.,
//! *Nature* 506, 2014). They are **not** any one apparatus's certified evaluation, and
//! must never be quoted as such. The structure is real physics (the effects, their
//! relative sizes, the quadrature sum); the magnitudes are illustrative-but-honest
//! orders of magnitude. This is a **physics/budget model (`Modelled`)** — not an
//! externally-validated measurement and not a metrology-grade uncertainty budget.
//!
//! Micromotion and the static quadrupole shift are primarily **trapped-ion** clock
//! systematics; they are included (small/representative) for completeness with that
//! noted, since a neutral-atom lattice does not have a Paul-trap micromotion term.

/// The physical category of a systematic effect — so a budget can be filtered or
/// grouped (e.g. "all Stark-type shifts") and so the statistical floor is never
/// quadrature-summed into the *systematic* total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    /// Lattice light shift: scalar + vector + tensor AC-Stark from the trapping field.
    LatticeAcStark,
    /// Lattice hyperpolarizability (higher-order, two-photon light shift).
    Hyperpolarizability,
    /// Blackbody-radiation (DC + dynamic) Stark shift — usually the dominant shift.
    BlackbodyStark,
    /// Stray DC electric-field Stark shift.
    DcStark,
    /// Density / cold-collision (atom-atom interaction) shift.
    DensityCollisional,
    /// Second-order Zeeman shift.
    SecondOrderZeeman,
    /// Probe-beam AC-Stark / line-pulling / servo offsets.
    ProbeAcStark,
    /// Background-gas collisional shift.
    BackgroundGas,
    /// Micromotion + static quadrupole (ion-clock systematics; ~absent for a lattice).
    MicromotionQuadrupole,
    /// Statistical floor: Dick effect + quantum projection noise (averaging-limited,
    /// **not** a systematic shift — kept separate from the systematic quadrature sum).
    DickQpnStatistical,
}

impl EffectKind {
    /// Whether this contributes to the **statistical** floor rather than the
    /// **systematic** shift budget.
    pub fn is_statistical(self) -> bool {
        matches!(self, EffectKind::DickQpnStatistical)
    }
}

/// One row of the budget: an effect, its representative fractional-frequency shift,
/// its 1-σ fractional uncertainty, and the provenance of the order of magnitude.
#[derive(Debug, Clone)]
pub struct SystematicEffect {
    pub kind: EffectKind,
    pub name: String,
    /// Representative fractional-frequency shift `Δν/ν` (signed, dimensionless).
    pub fractional_shift: f64,
    /// Representative 1-σ fractional-frequency uncertainty (dimensionless, ≥ 0).
    pub fractional_uncertainty: f64,
    pub provenance: String,
}

/// The assembled optical-lattice systematic budget.
#[derive(Debug, Clone)]
pub struct OpticalSystematicsBudget {
    pub device: String,
    pub effects: Vec<SystematicEffect>,
    pub provenance: String,
}

impl OpticalSystematicsBudget {
    /// A **representative Sr/Yb optical-lattice** systematic budget at the low-10⁻¹⁸
    /// level. Magnitudes are published-budget orders of magnitude (see module docs),
    /// not any one clock's evaluation.
    pub fn sr_lattice_representative() -> Self {
        let rep = "representative Sr/Yb-lattice published-budget order of magnitude (Ludlow et al. RMP 2015; BACON Nature 2021; Bloom et al. Nature 2014); not a specific apparatus";
        let effects = vec![
            SystematicEffect {
                kind: EffectKind::BlackbodyStark,
                name: "Blackbody-radiation Stark shift".into(),
                fractional_shift: -5.0e-15, // the dominant shift; ~5e-15 at room temperature
                fractional_uncertainty: 3.0e-18,
                provenance: rep.into(),
            },
            SystematicEffect {
                kind: EffectKind::SecondOrderZeeman,
                name: "Second-order Zeeman shift".into(),
                fractional_shift: -1.5e-16,
                fractional_uncertainty: 1.0e-18,
                provenance: rep.into(),
            },
            SystematicEffect {
                kind: EffectKind::LatticeAcStark,
                name: "Lattice AC-Stark (scalar/vector/tensor)".into(),
                fractional_shift: 1.0e-18, // near-cancelled at the magic wavelength
                fractional_uncertainty: 1.0e-18,
                provenance: rep.into(),
            },
            SystematicEffect {
                kind: EffectKind::Hyperpolarizability,
                name: "Lattice hyperpolarizability".into(),
                fractional_shift: -1.0e-19,
                fractional_uncertainty: 5.0e-19,
                provenance: rep.into(),
            },
            SystematicEffect {
                kind: EffectKind::DcStark,
                name: "DC-Stark (stray fields)".into(),
                fractional_shift: -2.0e-18,
                fractional_uncertainty: 1.0e-18,
                provenance: rep.into(),
            },
            SystematicEffect {
                kind: EffectKind::DensityCollisional,
                name: "Density / cold-collision shift".into(),
                fractional_shift: -3.0e-18,
                fractional_uncertainty: 2.0e-18,
                provenance: rep.into(),
            },
            SystematicEffect {
                kind: EffectKind::ProbeAcStark,
                name: "Probe AC-Stark / line pulling".into(),
                fractional_shift: 1.0e-19,
                fractional_uncertainty: 3.0e-19,
                provenance: rep.into(),
            },
            SystematicEffect {
                kind: EffectKind::BackgroundGas,
                name: "Background-gas collisions".into(),
                fractional_shift: -3.0e-18,
                fractional_uncertainty: 2.0e-18,
                provenance: rep.into(),
            },
            SystematicEffect {
                kind: EffectKind::MicromotionQuadrupole,
                name: "Micromotion + quadrupole (ion-class; ~absent for lattice)".into(),
                fractional_shift: 0.0,
                fractional_uncertainty: 1.0e-19,
                provenance: "ion-clock systematic, included for completeness; negligible for a neutral-atom lattice".into(),
            },
            SystematicEffect {
                kind: EffectKind::DickQpnStatistical,
                name: "Dick effect + quantum projection noise (statistical floor)".into(),
                fractional_shift: 0.0,
                fractional_uncertainty: 2.0e-18, // averaging-limited; the σ_y reached at the stated averaging time
                provenance: "statistical (averaging-limited) instability floor, representative; reached only after sufficient averaging".into(),
            },
        ];
        Self {
            device: "Sr/Yb optical-lattice clock (representative class)".into(),
            effects,
            provenance: "representative published-budget physics model (Modelled); orders of magnitude, not a specific lab's evaluation".into(),
        }
    }

    /// Total **net** systematic frequency shift `Σ Δν/ν` (signed). Excludes the
    /// statistical floor (which has no shift).
    pub fn total_shift(&self) -> f64 {
        self.effects
            .iter()
            .filter(|e| !e.kind.is_statistical())
            .map(|e| e.fractional_shift)
            .sum()
    }

    /// Total **systematic** uncertainty: the quadrature sum `√(Σ u_i²)` over the
    /// non-statistical effects (uncorrelated-uncertainty assumption, the standard
    /// budget convention).
    pub fn total_systematic_uncertainty(&self) -> f64 {
        self.effects
            .iter()
            .filter(|e| !e.kind.is_statistical())
            .map(|e| e.fractional_uncertainty * e.fractional_uncertainty)
            .sum::<f64>()
            .sqrt()
    }

    /// The **statistical** (averaging-limited) uncertainty: quadrature of the
    /// statistical rows (Dick + quantum projection noise).
    pub fn statistical_uncertainty(&self) -> f64 {
        self.effects
            .iter()
            .filter(|e| e.kind.is_statistical())
            .map(|e| e.fractional_uncertainty * e.fractional_uncertainty)
            .sum::<f64>()
            .sqrt()
    }

    /// **Combined** uncertainty: systematic ⊕ statistical in quadrature — the total
    /// fractional-frequency uncertainty a comparison at the stated averaging time sees.
    pub fn combined_uncertainty(&self) -> f64 {
        let sys = self.total_systematic_uncertainty();
        let stat = self.statistical_uncertainty();
        (sys * sys + stat * stat).sqrt()
    }

    /// The single largest |shift| row — the dominant systematic to evaluate best.
    pub fn dominant_shift(&self) -> Option<&SystematicEffect> {
        self.effects
            .iter()
            .filter(|e| !e.kind.is_statistical())
            .max_by(|a, b| {
                a.fractional_shift
                    .abs()
                    .partial_cmp(&b.fractional_shift.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_systematic_uncertainty_is_low_1e_minus_18_order() {
        // A representative Sr/Yb-lattice budget should land at the low-10⁻¹⁸ level —
        // the state-of-the-art order of magnitude — not 10⁻¹⁶ or 10⁻²⁰.
        let b = OpticalSystematicsBudget::sr_lattice_representative();
        let u = b.total_systematic_uncertainty();
        assert!(
            (1e-18..1e-17).contains(&u),
            "total systematic uncertainty {u:.3e} should be low-1e-18 order"
        );
    }

    #[test]
    fn quadrature_sum_is_correct() {
        // Recompute the quadrature independently and confirm the method matches.
        let b = OpticalSystematicsBudget::sr_lattice_representative();
        let manual: f64 = b
            .effects
            .iter()
            .filter(|e| !e.kind.is_statistical())
            .map(|e| e.fractional_uncertainty.powi(2))
            .sum::<f64>()
            .sqrt();
        assert!((b.total_systematic_uncertainty() - manual).abs() / manual < 1e-12);
    }

    #[test]
    fn blackbody_is_the_dominant_shift() {
        // The BBR Stark shift is by far the largest systematic shift in a lattice clock.
        let b = OpticalSystematicsBudget::sr_lattice_representative();
        let dom = b.dominant_shift().unwrap();
        assert_eq!(dom.kind, EffectKind::BlackbodyStark);
        // …and it dominates the net shift.
        assert!(b.total_shift().abs() > 1e-15);
    }

    #[test]
    fn statistical_floor_is_separate_from_the_systematic_sum() {
        // The Dick + QPN floor must NOT inflate the systematic quadrature; it enters
        // only the combined total. So combined >= systematic, strictly when a
        // statistical row is present.
        let b = OpticalSystematicsBudget::sr_lattice_representative();
        let sys = b.total_systematic_uncertainty();
        let stat = b.statistical_uncertainty();
        let comb = b.combined_uncertainty();
        assert!(stat > 0.0, "a statistical floor should be present");
        assert!(comb > sys, "combined must exceed systematic-only");
        assert!((comb - (sys * sys + stat * stat).sqrt()).abs() / comb < 1e-12);
    }

    #[test]
    fn every_effect_carries_provenance_and_lists_the_required_effects() {
        // Honesty + coverage: each row has provenance, and the budget covers the
        // effects the L1 deliverable enumerates.
        let b = OpticalSystematicsBudget::sr_lattice_representative();
        for e in &b.effects {
            assert!(!e.provenance.is_empty(), "{} needs provenance", e.name);
            assert!(e.fractional_uncertainty >= 0.0, "{} u >= 0", e.name);
        }
        let kinds: Vec<EffectKind> = b.effects.iter().map(|e| e.kind).collect();
        for required in [
            EffectKind::LatticeAcStark,
            EffectKind::Hyperpolarizability,
            EffectKind::BlackbodyStark,
            EffectKind::DcStark,
            EffectKind::DensityCollisional,
            EffectKind::MicromotionQuadrupole,
            EffectKind::SecondOrderZeeman,
            EffectKind::DickQpnStatistical,
        ] {
            assert!(kinds.contains(&required), "budget missing {required:?}");
        }
    }
}
