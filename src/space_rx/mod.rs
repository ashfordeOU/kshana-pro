// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Space-segment GNSS receiver evidence (research 2026-11-20 gaps, epic E20)
//!
//! The receiver-side view of a GNSS receiver flown on a spacecraft: what it meets in orbit,
//! what its test campaign shows, and how its faults look in its own output.
//!
//! * [`exposure`] (B2001): the **ground-interference envelope seen by a spacecraft
//!   receiver**. The orbit (a TLE, propagated with the open engine's SGP4) is flown over
//!   interference areas (declared, or the degraded cells of an open-engine interference map);
//!   per pass the modelled J/S and effective C/N0 at the receiver, the time above the
//!   receiver's tested J/S tolerance, and the Doppler and Doppler-rate offsets a ground
//!   transmitter imposes as seen from orbit. The result is a **test envelope** (percentiles of
//!   J/S, dwell, Doppler offset and rate) for jamming and spoofing-resilience tests of the
//!   receiver on a simulator. Signal-parameter level only; nothing here locates an emitter or
//!   produces a waveform.
//! * [`qual`] (B2002): the **qualification campaign format** (`kshana-space-rx-campaign/1`:
//!   TID steps with navigation outputs, SEE runs, vibration, thermal and tracking cases), the
//!   per-chip **scorecard** against a mission profile, the **selection database**
//!   (`kshana-space-rx-db/1`, schema published by `space-rx db schema`) and a **qualification
//!   process template** populated from a campaign.
//! * [`see`] (B2003): **radiation-fault signatures** in a receiver's output trace (latch-up
//!   suspect, reset, functional interrupt, frozen output, single-epoch upsets with a bit-flip
//!   test, time-tag corruption, clock-bias step) and an **injector** that writes fixtures with
//!   ground truth.
//! * [`obc`] (B2003): **corrupt-data tolerance vectors** for the receiver-to-OBC interface
//!   (NMEA 0183 sentences built from a trace and mutated by class) with the expected OBC
//!   behaviour, a reference parser and the scoring of an OBC's accept/reject log.
//! * [`see_rate`] (B2003): a **SEE rate forecast** from a Weibull cross-section and an
//!   integral LET spectrum (plus an optional proton term).
//!
//! **Evidence label: MODELLED.** No receiver has been tested with these tools in this build.
//! The sample campaign, environment and traces are SYNTHETIC. Scorecards are evidence for a
//! selection or review against the limits a mission states; they are never a statement that a
//! part is fit for flight. Radiation and interface robustness are modelling and test planning
//! only.

pub mod exposure;
pub mod obc;
pub mod qual;
pub mod resilience;
pub mod see;
pub mod see_rate;

/// The licence feature of the ground-interference envelope.
pub const FEATURE_EXPOSURE: &str = "pro-space-rx-exposure";
/// The licence feature of campaign scorecards, the selection database and templates.
pub const FEATURE_QUAL: &str = "pro-space-rx-qual";
/// The licence feature of the radiation-fault detector, interface vectors and SEE rates.
pub const FEATURE_SEE: &str = "pro-space-rx-see";

/// The evidence label of every output.
pub const LABEL: &str = "MODELLED: computed from the inputs supplied (orbits, areas, test \
results, traces) with the models and limits stated in the output. Not measured on board; not a \
statement that any receiver is fit for flight or meets a standard.";

/// Lower-case hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// Round `x` to `d` decimals (values written to files).
pub fn round(x: f64, d: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let s = 10f64.powi(d);
    (x * s).round() / s
}

/// The `p`-quantile (0..=1) of `v` by linear interpolation; `None` when empty.
pub fn quantile(v: &[f64], p: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() {
        return None;
    }
    s.sort_by(|a, b| a.total_cmp(b));
    let pos = p.clamp(0.0, 1.0) * (s.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    Some(s[lo] + (s[hi] - s[lo]) * (pos - lo as f64))
}

/// One check of a scorecard or a tolerance score.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Check {
    pub id: String,
    /// `met`, `not_met` or `not_assessed`.
    pub verdict: String,
    pub detail: String,
}

impl Check {
    pub fn new(id: &str, verdict: Option<bool>, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            verdict: match verdict {
                Some(true) => "met",
                Some(false) => "not_met",
                None => "not_assessed",
            }
            .into(),
            detail: detail.into(),
        }
    }
}
