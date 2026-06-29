// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L1 holdover uncertainty-quantification pack
//!
//! The open core answers *"given this clock, what is my coast time to an N-ns error?"*
//! in closed form ([`kshana::holdover`]). This pack **wraps** that physics — it does
//! not re-derive it — into the operational object a PNT/telecom resilience trade asks
//! for: a **banded holdover prediction**
//!
//! ```text
//!   x(t) = x0 + y0·t + ½·D·t²  ±  k·σ(t)
//! ```
//!
//! where `x0` is the time error at coast start, `y0·t + ½·D·t²` is the deterministic
//! residual the synced estimator could not remove ([`kshana::holdover::deterministic_tie`]),
//! `σ(t)` is the stochastic coast 1-σ ([`kshana::holdover::coast_phase_sigma`], the van
//! Loan `q_wf·t + q_rw·t³/3 + q_drift·t⁵/20`), and `k` is a coverage factor (1/2/3-σ).
//! The band optionally folds in **estimation uncertainty** on the deterministic terms
//! (`σ_y0·t`, `½·σ_D·t²`) in quadrature with the stochastic part.
//!
//! From that it reports the **in-spec coast duration** against a telecom/PNT holdover
//! mask (ITU-T G.8272.1 ePRTC, G.8272 PRTC-A, or the classic 1.5 µs / 24 h backhaul
//! budget) and whether the clock holds over a chosen **GNSS-reacquisition interval**.
//!
//! ## Honesty
//!
//! This is a **composition of the open holdover physics** (`Modelled`): the stochastic
//! σ(t) is the open core's van Loan term verbatim, and a unit test pins the
//! zero-deterministic, k = 1 case to [`kshana::holdover::holdover_seconds`] exactly.
//! The mask *thresholds* are the published clock-class limits (named in each mask's
//! provenance); how long a given clock stays inside them is the modelled answer, and —
//! as the open module's own caveat stresses — for a stable clock it is governed by the
//! assumed long-τ red-noise floor (`q_rw`, `q_drift`), not the cited σ_y(1 s). The
//! closed-form coast model carries **no flicker term** (the open van Loan polynomial
//! has none), so a twin's `flicker_floor` is *not* represented here — a deliberate,
//! stated limitation. No flight/in-orbit-validated holdover is claimed.

use kshana::api::KshanaError;
use kshana::holdover::{coast_phase_sigma, phase_to_range_m};

use crate::twin::CalibratedTwin;

/// A telecom/PNT holdover mask: a maximum |time error| budget, with the published
/// clock class it comes from named in `provenance`.
#[derive(Debug, Clone)]
pub struct HoldoverMask {
    pub name: String,
    /// Maximum absolute time error (s) the clock must stay within during holdover.
    pub threshold_s: f64,
    /// The holdover window the class associates with the threshold (s), if any —
    /// informational (e.g. the 24 h of the 1.5 µs/24 h budget).
    pub window_s: Option<f64>,
    pub provenance: String,
}

impl HoldoverMask {
    /// **ITU-T G.8272.1 ePRTC** — enhanced primary reference time clock: a 30 ns
    /// absolute time-error class limit. (The standard's holdover provisions are more
    /// detailed; 30 ns is the published |TE| class budget used here as the mask.)
    pub fn eprtc() -> Self {
        Self {
            name: "ePRTC (G.8272.1)".into(),
            threshold_s: 30.0e-9,
            window_s: None,
            provenance: "ITU-T G.8272.1 ePRTC class |time error| budget (30 ns); published class limit".into(),
        }
    }

    /// **ITU-T G.8272 PRTC-A** — primary reference time clock, 100 ns class limit.
    pub fn prtc_a() -> Self {
        Self {
            name: "PRTC-A (G.8272)".into(),
            threshold_s: 100.0e-9,
            window_s: None,
            provenance: "ITU-T G.8272 PRTC-A class |time error| budget (100 ns); published class limit".into(),
        }
    }

    /// **1.5 µs / 24 h** — the classic mobile-backhaul / legacy telecom holdover
    /// budget: stay within 1.5 µs of time error over a 24 h coast.
    pub fn telecom_1_5us_24h() -> Self {
        Self {
            name: "1.5 µs / 24 h".into(),
            threshold_s: 1.5e-6,
            window_s: Some(24.0 * 3600.0),
            provenance: "classic telecom/PNT holdover budget: 1.5 µs maximum time error over 24 h".into(),
        }
    }

    /// A custom mask (e.g. a mission-specific timing budget). Threshold in seconds.
    pub fn custom(name: &str, threshold_s: f64, window_s: Option<f64>, provenance: &str) -> Self {
        Self {
            name: name.into(),
            threshold_s,
            window_s,
            provenance: provenance.into(),
        }
    }
}

/// Inputs to a banded holdover prediction: the open power-law PSDs plus the coast
/// initial condition, the deterministic residuals the estimator left behind, their
/// estimation uncertainties, and the coverage factor.
#[derive(Debug, Clone)]
pub struct HoldoverInputs {
    /// White-FM PSD (s²/s).
    pub q_wf: f64,
    /// Random-walk-FM PSD ((1/s)²/s).
    pub q_rw: f64,
    /// Random-run/drift PSD ((1/s²)²/s) — the open core's `q_drift`.
    pub q_drift: f64,
    /// Time error at coast start (s) — the post-sync residual phase.
    pub x0: f64,
    /// Residual fractional-frequency offset (dimensionless `y0`) the estimator left.
    pub y0: f64,
    /// Residual aging / drift `D` (1/s²).
    pub drift: f64,
    /// 1-σ estimation uncertainty on `y0` (dimensionless).
    pub sigma_y0: f64,
    /// 1-σ estimation uncertainty on `drift` (1/s²).
    pub sigma_drift: f64,
    /// Coverage factor for the band (1, 2, 3 → 1/2/3-σ).
    pub k: f64,
}

impl HoldoverInputs {
    /// Build from a [`CalibratedTwin`]: take its white-FM and random-walk-FM PSDs and
    /// deterministic aging; the caller supplies the coast initial condition and the
    /// post-sync frequency residual / its uncertainty. `q_drift` is 0 (the twin fit
    /// has no random-run state). **Note:** the twin's `flicker_floor` is intentionally
    /// not carried — the open closed-form coast model has no flicker term (see module
    /// docs), so a flicker-dominated clock's holdover is *under*-modelled here and a
    /// full Monte-Carlo through [`crate::twin::validate`] should be used instead.
    pub fn from_twin(twin: &CalibratedTwin, x0: f64, y0: f64, sigma_y0: f64, k: f64) -> Self {
        Self {
            q_wf: twin.q_wf,
            q_rw: twin.q_rw,
            q_drift: 0.0,
            x0,
            y0,
            drift: twin.drift_per_s,
            sigma_y0,
            sigma_drift: 0.0,
            k,
        }
    }

    /// Deterministic mean time error `x(t) = x0 + y0·t + ½·D·t²` (s).
    pub fn mean_s(&self, t: f64) -> f64 {
        let t = t.max(0.0);
        self.x0 + self.y0 * t + 0.5 * self.drift * t * t
    }

    /// Stochastic coast 1-σ (s) — the open core's van Loan term verbatim.
    pub fn stochastic_sigma_s(&self, t: f64) -> f64 {
        coast_phase_sigma(self.q_wf, self.q_rw, self.q_drift, t)
    }

    /// Deterministic-parameter 1-σ (s): `√((σ_y0·t)² + (½·σ_D·t²)²)` — the time-error
    /// uncertainty contributed by imperfect knowledge of `y0` and `D`.
    pub fn deterministic_sigma_s(&self, t: f64) -> f64 {
        let t = t.max(0.0);
        let from_y0 = self.sigma_y0 * t;
        let from_drift = 0.5 * self.sigma_drift * t * t;
        (from_y0 * from_y0 + from_drift * from_drift).sqrt()
    }

    /// Band half-width (s): `k·√(σ_stochastic² + σ_deterministic²)`.
    pub fn band_half_s(&self, t: f64) -> f64 {
        let s = self.stochastic_sigma_s(t);
        let d = self.deterministic_sigma_s(t);
        self.k * (s * s + d * d).sqrt()
    }

    /// **Conservative (sign-agnostic) error envelope** (s):
    /// `|x0| + |y0|·t + ½·|D|·t² + band_half(t)`. Every term is non-negative and
    /// increasing in `t`, so the envelope is monotone — which is what makes the
    /// in-spec coast time a well-posed root find regardless of the sign of `y0`/`D`.
    pub fn envelope_s(&self, t: f64) -> f64 {
        let t = t.max(0.0);
        self.x0.abs() + self.y0.abs() * t + 0.5 * self.drift.abs() * t * t + self.band_half_s(t)
    }

    /// One banded prediction sample at `t`.
    pub fn predict(&self, t: f64) -> HoldoverPrediction {
        let mean = self.mean_s(t);
        let half = self.band_half_s(t);
        HoldoverPrediction {
            t_s: t,
            mean_s: mean,
            band_half_s: half,
            lower_s: mean - half,
            upper_s: mean + half,
            envelope_s: self.envelope_s(t),
            envelope_range_m: phase_to_range_m(self.envelope_s(t)),
        }
    }

    /// Banded predictions at each `t` in `t_points` (for plotting / a coast table).
    pub fn predict_curve(&self, t_points: &[f64]) -> Vec<HoldoverPrediction> {
        t_points.iter().map(|&t| self.predict(t)).collect()
    }

    /// **In-spec coast duration** (s): the largest coast time whose conservative error
    /// envelope still fits inside `mask.threshold_s`. Inverts the monotone
    /// [`envelope_s`](Self::envelope_s) by bracket-and-bisect (the same shape the open
    /// [`kshana::holdover::holdover_seconds`] uses). Returns `0.0` if the envelope
    /// already exceeds the threshold at `t = 0` (i.e. `|x0| + k·σ(0) > threshold`), and
    /// `f64::INFINITY` for a noiseless, residual-free clock that never exceeds it.
    pub fn in_spec_coast_s(&self, mask: &HoldoverMask) -> Result<f64, KshanaError> {
        if !mask.threshold_s.is_finite() || mask.threshold_s <= 0.0 {
            return Err(KshanaError::InvalidInput(
                "holdover mask threshold must be finite and > 0".into(),
            ));
        }
        if !self.k.is_finite() || self.k < 0.0 {
            return Err(KshanaError::InvalidInput(
                "coverage factor k must be finite and >= 0".into(),
            ));
        }
        let target = mask.threshold_s;
        if self.envelope_s(0.0) > target {
            return Ok(0.0);
        }
        // Bracket: expand hi until the envelope exceeds the threshold.
        let mut hi = 1.0_f64;
        let mut guard = 0;
        while self.envelope_s(hi) <= target {
            hi *= 2.0;
            guard += 1;
            if guard > 200 {
                return Ok(f64::INFINITY); // unreachable within any sane horizon
            }
        }
        // Bisection (envelope is monotone increasing in t).
        let mut lo = 0.0_f64;
        for _ in 0..100 {
            let mid = 0.5 * (lo + hi);
            if self.envelope_s(mid) <= target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Ok(0.5 * (lo + hi))
    }

    /// Whether the clock holds over a **GNSS-reacquisition interval** `reacq_s`: the
    /// error envelope at `reacq_s` stays inside the mask. The `margin_s` is the slack
    /// (threshold − envelope), negative if blown.
    pub fn holds_over_interval(
        &self,
        mask: &HoldoverMask,
        reacq_s: f64,
    ) -> Result<ReacquisitionVerdict, KshanaError> {
        if !reacq_s.is_finite() || reacq_s < 0.0 {
            return Err(KshanaError::InvalidInput(
                "reacquisition interval must be finite and >= 0".into(),
            ));
        }
        let env = self.envelope_s(reacq_s);
        Ok(ReacquisitionVerdict {
            reacq_s,
            envelope_s: env,
            threshold_s: mask.threshold_s,
            margin_s: mask.threshold_s - env,
            holds: env <= mask.threshold_s,
        })
    }
}

/// One banded holdover prediction sample.
#[derive(Debug, Clone)]
pub struct HoldoverPrediction {
    pub t_s: f64,
    /// Deterministic mean time error (s).
    pub mean_s: f64,
    /// Band half-width (s) = `k·σ_total`.
    pub band_half_s: f64,
    /// `mean − band_half` (s).
    pub lower_s: f64,
    /// `mean + band_half` (s).
    pub upper_s: f64,
    /// Conservative sign-agnostic error envelope (s) — what the mask is tested against.
    pub envelope_s: f64,
    /// The envelope as a one-way range error (m), via `c·Δt`.
    pub envelope_range_m: f64,
}

/// The verdict of holding over a GNSS-reacquisition interval.
#[derive(Debug, Clone)]
pub struct ReacquisitionVerdict {
    pub reacq_s: f64,
    pub envelope_s: f64,
    pub threshold_s: f64,
    /// Slack against the mask (s); negative means the budget is exceeded.
    pub margin_s: f64,
    pub holds: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kshana::holdover::holdover_seconds;

    fn white_only(q_wf: f64) -> HoldoverInputs {
        HoldoverInputs {
            q_wf,
            q_rw: 0.0,
            q_drift: 0.0,
            x0: 0.0,
            y0: 0.0,
            drift: 0.0,
            sigma_y0: 0.0,
            sigma_drift: 0.0,
            k: 1.0,
        }
    }

    #[test]
    fn in_spec_coast_matches_open_holdover_seconds_in_the_pure_case() {
        // Cross-check against the open oracle: with no deterministic terms and k = 1,
        // the envelope IS coast_phase_sigma, so in_spec_coast must equal the open
        // kshana::holdover::holdover_seconds to high precision (an internal-consistency
        // check that the wrap is faithful, not an independent validation).
        let h = HoldoverInputs {
            q_wf: 1e-22,
            q_rw: 1e-28,
            q_drift: 1e-34,
            ..white_only(0.0)
        };
        let mask = HoldoverMask::custom("test", 1e-8, None, "unit");
        let ours = h.in_spec_coast_s(&mask).unwrap();
        let theirs = holdover_seconds(h.q_wf, h.q_rw, h.q_drift, mask.threshold_s);
        let rel = (ours - theirs).abs() / theirs;
        assert!(rel < 1e-6, "ours {ours:.6e} vs open {theirs:.6e} (rel {rel})");
    }

    #[test]
    fn deterministic_residual_shortens_the_coast() {
        // Adding a residual frequency offset y0 must shorten the in-spec coast versus
        // the stochastic-only clock — the deterministic ramp eats the budget faster.
        let mask = HoldoverMask::prtc_a();
        let stoch = white_only(1e-22);
        let with_y0 = HoldoverInputs {
            y0: 1e-12,
            ..white_only(1e-22)
        };
        let t_stoch = stoch.in_spec_coast_s(&mask).unwrap();
        let t_with = with_y0.in_spec_coast_s(&mask).unwrap();
        assert!(
            t_with < t_stoch,
            "y0 residual should shorten coast: {t_with:.1} vs {t_stoch:.1}"
        );
    }

    #[test]
    fn higher_coverage_factor_shortens_the_in_spec_coast() {
        // A 3-σ band reaches the mask sooner than a 1-σ band (more conservative).
        let mask = HoldoverMask::eprtc();
        let k1 = white_only(1e-20);
        let k3 = HoldoverInputs {
            k: 3.0,
            ..white_only(1e-20)
        };
        assert!(k3.in_spec_coast_s(&mask).unwrap() < k1.in_spec_coast_s(&mask).unwrap());
    }

    #[test]
    fn band_brackets_the_mean_and_grows_with_time() {
        let h = HoldoverInputs {
            y0: 1e-13,
            ..white_only(1e-22)
        };
        let p1 = h.predict(100.0);
        let p2 = h.predict(1000.0);
        assert!(p1.lower_s <= p1.mean_s && p1.mean_s <= p1.upper_s);
        assert!(
            p2.band_half_s > p1.band_half_s,
            "band must widen with coast time"
        );
        // Envelope maps to a range error via the open c·Δt helper.
        assert!(p2.envelope_range_m > 0.0);
    }

    #[test]
    fn reacquisition_interval_verdict_is_consistent_with_coast() {
        // If the in-spec coast exceeds the reacquisition interval, the verdict must
        // hold (positive margin); if shorter, it must fail.
        let mask = HoldoverMask::telecom_1_5us_24h();
        let h = white_only(1e-20);
        let coast = h.in_spec_coast_s(&mask).unwrap();
        let inside = h.holds_over_interval(&mask, coast * 0.5).unwrap();
        let outside = h.holds_over_interval(&mask, coast * 2.0).unwrap();
        assert!(inside.holds && inside.margin_s >= 0.0);
        assert!(!outside.holds && outside.margin_s < 0.0);
    }

    #[test]
    fn initial_error_already_over_budget_gives_zero_coast() {
        let mask = HoldoverMask::eprtc(); // 30 ns
        let h = HoldoverInputs {
            x0: 100e-9, // already 100 ns at coast start
            ..white_only(1e-22)
        };
        assert_eq!(h.in_spec_coast_s(&mask).unwrap(), 0.0);
    }

    #[test]
    fn from_twin_carries_white_and_random_walk_but_not_flicker() {
        // The twin → holdover bridge must carry q_wf and q_rw (the closed-form coast
        // terms) and the deterministic drift, and must NOT silently invent a q_drift.
        let twin = CalibratedTwin {
            id: "t".into(),
            provenance: "p".into(),
            q_wf: 9e-24,
            flicker_floor: 1e-13,
            q_rw: 3e-30,
            drift_per_s: 1e-15,
        };
        let h = HoldoverInputs::from_twin(&twin, 0.0, 1e-13, 1e-14, 2.0);
        assert_eq!(h.q_wf, 9e-24);
        assert_eq!(h.q_rw, 3e-30);
        assert_eq!(h.q_drift, 0.0); // not fabricated
        assert_eq!(h.drift, 1e-15);
        assert_eq!(h.k, 2.0);
    }

    #[test]
    fn bad_mask_and_inputs_are_errors() {
        let h = white_only(1e-22);
        assert!(h
            .in_spec_coast_s(&HoldoverMask::custom("bad", 0.0, None, "x"))
            .is_err());
        assert!(h.holds_over_interval(&HoldoverMask::eprtc(), -1.0).is_err());
    }

    #[test]
    fn masks_carry_their_published_thresholds_and_provenance() {
        assert_eq!(HoldoverMask::eprtc().threshold_s, 30.0e-9);
        assert_eq!(HoldoverMask::prtc_a().threshold_s, 100.0e-9);
        assert_eq!(HoldoverMask::telecom_1_5us_24h().threshold_s, 1.5e-6);
        for m in [
            HoldoverMask::eprtc(),
            HoldoverMask::prtc_a(),
            HoldoverMask::telecom_1_5us_24h(),
        ] {
            assert!(!m.provenance.is_empty(), "{} needs provenance", m.name);
        }
    }
}
