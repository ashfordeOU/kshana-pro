// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Lunar surface operations evidence, second slice (research 2026-11-20 gaps, E24 B2404)
//!
//! The first B2404 slice (FDIR plan results, configuration attestation, datum tie, tip-over)
//! is in `lunar-evidence`. This slice adds:
//!
//! * [`plume`]: **dust-plume visibility**, a MODELLED parametric model of the ejecta sheet a
//!   descending or ascending engine raises, with every assumption stated in the output:
//!   optical and RF extinction along declared lines of sight (a surface beacon, a camera, a
//!   relay) over the descent, onset altitude and duration of obscuration.
//! * [`traverse`]: **LTV / EVA navigation safety evidence** from a traverse log: position
//!   error against the reported sigma (Stanford regions, Gaussian overbound) where truth is
//!   logged, hazard keep-out margins with the protection level, and the walk-back margin
//!   against the consumables the user declares.
//! * [`rf`]: **RF self-interference forensics from I/Q** (analysis only): narrowband spurs in a
//!   SigMF recording of the receiver band, matched to harmonics of on-board sources the user
//!   declares (motor drives, converters, clocks), their time correlation with a declared
//!   activity log, and the in-band interference-to-noise ratio with the C/N0 loss it implies.
//!
//! **Evidence label: MODELLED.** Exercised on SYNTHETIC inputs with known truth ([`synth`],
//! `tests/lunar_surface.rs`). The plume model is parametric and is not validated against
//! plume measurements; its physical parameters are the user's and have no defaults. Nothing
//! here locates an emitter (on-board sources are declared, not found), fuses sensors for
//! navigation, or transmits.

pub mod plume;
pub mod rf;
pub mod synth;
pub mod traverse;

/// Evidence label.
pub const EVIDENCE: &str = "MODELLED: exercised on synthetic inputs with known truth; not \
validated against flight or surface measurements.";

/// Read a file relative to the request's directory.
pub(crate) fn read_rel(base: &std::path::Path, p: &str) -> Result<String, String> {
    crate::timing::grid::read_text(base, p)
}

/// Read bytes relative to the request's directory.
pub(crate) fn read_rel_bytes(base: &std::path::Path, p: &str) -> Result<Vec<u8>, String> {
    let path = if std::path::Path::new(p).is_absolute() {
        p.into()
    } else {
        base.join(p)
    };
    std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A positive, finite number or an error naming it.
pub(crate) fn positive(name: &str, v: f64) -> Result<(), String> {
    if v > 0.0 && v.is_finite() {
        Ok(())
    } else {
        Err(format!("{name} must be positive"))
    }
}

/// Round to `d` decimals.
pub(crate) fn r(x: f64, d: i32) -> f64 {
    crate::forensics::inputs::round(x, d)
}
