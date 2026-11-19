// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D3. Earth–Moon two-way time-transfer (TWTT) holdover calculator
//!
//! NASA's open question 6 (research note §0 item 3, gap 4) asks for rules of thumb linking the
//! Earth–Moon TWTT contact cadence to holdover. Inputs: the contact interval `T`, the lunar
//! clock's FM noise model (white FM `h_0`, flicker FM `h_{-1}`, random-walk FM `h_{-2}`; from a
//! named class, IEEE 1139 coefficients or Allan-deviation points) and the TWTT link noise.
//! Outputs: the time error between contacts and the longest contact interval that keeps the
//! time error inside a budget at a coverage factor `k`.
//!
//! ## The model
//!
//! At each contact the lunar clock's offset is measured against UTC(k) with `n` TWTT
//! measurements of white phase noise `σ_link` each, so the phase estimate has variance
//! `σ_L² = σ_link²/n`. Between contacts the clock coasts. With `q_wf = h_0/2` and
//! `q_rw = 2π² h_{-2}` (so that `σ_y²(τ) = q_wf/τ + q_rw τ/3`, IEEE 1139), the time-error
//! variance `u` seconds after a contact is (van Loan coast variance from the open engine,
//! [`kshana::holdover::coast_phase_variance`]):
//!
//! * **phase only** (offset reset, frequency left alone):
//!   `σ²(u) = σ_L² + q_wf u + q_rw u³/3 + σ_F² u²`;
//! * **phase and frequency** (frequency estimated from the last two contacts, `T` apart):
//!   `σ²(u) = σ_L²[(1+u/T)² + (u/T)²] + q_wf(u + u²/T) + q_rw(u³/3 + u²T/3) + 2σ_F² u²`;
//!
//! plus a constant link-calibration term `u_cal²`. `σ_F² = 2 ln 2 · h_{-1}` is the flicker-FM
//! Allan variance; for flicker FM the time error is taken as `τ·σ_y(τ)` (the IEEE 1139 /
//! ITU-T rule of thumb), which is an approximation, not an exact prediction variance. For white
//! and random-walk FM with phase-only correction and a noise-free link, `σ(τ) = τ·σ_y(τ)`
//! exactly; the tests check that against the open engine's Allan deviation
//! ([`kshana::powerlaw::allan_deviation`]) and the phase-and-frequency variance by Monte Carlo.
//!
//! The worst time error in a contact cycle is at `u = T` (just before the next contact), so the
//! required interval for a budget `B` solves `k σ(T) = B` (bisection; closed form for white FM).
//!
//! Evidence label: **MODELLED**. The named clock classes are the open engine's
//! ([`kshana::clock_specs::LunarClock`], calibrated to published one-day rows). No lunar clock
//! or Earth–Moon TWTT link has been measured here.

use kshana::powerlaw::PowerLaw;
use serde::{Deserialize, Serialize};

/// A clock's FM noise (IEEE 1139 coefficients).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClockNoise {
    /// `h_{-2}`, random-walk FM.
    pub h_m2: f64,
    /// `h_{-1}`, flicker FM.
    pub h_m1: f64,
    /// `h_0`, white FM.
    pub h_0: f64,
}

impl ClockNoise {
    pub fn from_powerlaw(p: &PowerLaw) -> Self {
        ClockNoise {
            h_m2: p.h_m2,
            h_m1: p.h_m1,
            h_0: p.h_0,
        }
    }
    pub fn powerlaw(&self) -> PowerLaw {
        PowerLaw {
            h_m2: self.h_m2,
            h_m1: self.h_m1,
            h_0: self.h_0,
            ..Default::default()
        }
    }
    /// From Allan-deviation points, by the open engine's non-negative FM-family fit.
    pub fn from_adev_points(taus: &[f64], adevs: &[f64]) -> Result<Self, String> {
        if taus.len() != adevs.len() || taus.len() < 2 {
            return Err("give at least two (tau, adev) points".into());
        }
        let (h_m2, h_m1, h_0) = kshana::powerlaw::fit_fm_family(taus, adevs);
        Ok(ClockNoise { h_m2, h_m1, h_0 })
    }
    /// A named class: the open engine's lunar clocks (`optical-master`, `passive-h-maser`,
    /// `rafs`, `mini-rafs`) or its clock classes (`tcxo`, `ocxo`, `csac`, `uso`, `dsac`).
    pub fn named(name: &str) -> Result<Self, String> {
        use kshana::clock_specs::LunarClock;
        if let Some(c) = LunarClock::all().into_iter().find(|c| c.name() == name) {
            return Ok(Self::from_powerlaw(&c.powerlaw()));
        }
        if let Some(n) = crate::timing::model::class_noise(name) {
            return Ok(ClockNoise {
                h_0: 2.0 * n.q_wf,
                h_m1: 0.0,
                h_m2: n.q_rw / (2.0 * std::f64::consts::PI.powi(2)),
            });
        }
        Err(format!(
            "unknown clock '{name}' (optical-master|passive-h-maser|rafs|mini-rafs|tcxo|ocxo|csac|uso|dsac)"
        ))
    }
    /// `q_wf = h_0 / 2` (s²/s).
    pub fn q_wf(&self) -> f64 {
        self.h_0 / 2.0
    }
    /// `q_rw = 2π² h_{-2}` (s²/s³).
    pub fn q_rw(&self) -> f64 {
        2.0 * std::f64::consts::PI.powi(2) * self.h_m2
    }
    /// The flicker-FM Allan variance `2 ln 2 · h_{-1}`.
    pub fn ffm_avar(&self) -> f64 {
        2.0 * std::f64::consts::LN_2 * self.h_m1
    }
    /// `σ_y(τ)` from the open engine.
    pub fn adev(&self, tau: f64) -> f64 {
        kshana::clock_specs::sigma_y(&self.powerlaw(), tau)
    }
}

/// How the lunar clock is corrected at each contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    PhaseOnly,
    PhaseAndFrequency,
}

impl Strategy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "phase" | "phase-only" => Ok(Strategy::PhaseOnly),
            "phase-frequency" | "phase-and-frequency" => Ok(Strategy::PhaseAndFrequency),
            o => Err(format!(
                "unknown strategy '{o}' (phase-only|phase-and-frequency)"
            )),
        }
    }
}

/// The TWTT link.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// White phase noise of one TWTT measurement (s, 1σ).
    pub sigma_s: f64,
    /// Measurements averaged per contact.
    pub n_per_contact: u32,
    /// Standard uncertainty of the link calibration (s), constant between contacts.
    pub calibration_u_s: f64,
}

impl Link {
    /// The phase-estimate variance at a contact, `σ_link²/n`.
    pub fn phase_var(&self) -> f64 {
        self.sigma_s.powi(2) / self.n_per_contact.max(1) as f64
    }
}

/// The time-error variance components at `u` after a contact (s²).
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Components {
    pub u_s: f64,
    pub link: f64,
    pub white_fm: f64,
    pub flicker_fm: f64,
    pub random_walk_fm: f64,
    pub calibration: f64,
    pub total: f64,
}

/// A holdover problem.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Holdover {
    pub clock: ClockNoise,
    pub link: Link,
    pub strategy: Strategy,
}

/// One point of the time-error profile.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ProfilePoint {
    pub u_s: f64,
    pub sigma_s: f64,
}

/// The time error over one contact cycle.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub interval_s: f64,
    pub k: f64,
    pub profile: Vec<ProfilePoint>,
    /// `k σ(T)`, the worst time error in the cycle (s).
    pub max_time_error_s: f64,
    /// The rms time error averaged over the cycle (s).
    pub rms_time_error_s: f64,
    pub at_interval: Components,
    pub label: &'static str,
}

impl Holdover {
    /// The variance components `u` after a contact, for contacts `interval` apart.
    pub fn components(&self, u: f64, interval: f64) -> Components {
        let u = u.max(0.0);
        let c = &self.clock;
        let l = self.link.phase_var();
        let coast_wf = kshana::holdover::coast_phase_variance(c.q_wf(), 0.0, 0.0, u);
        let coast_rw = kshana::holdover::coast_phase_variance(0.0, c.q_rw(), 0.0, u);
        let (link, wf, ff, rw) = match self.strategy {
            Strategy::PhaseOnly => (l, coast_wf, c.ffm_avar() * u * u, coast_rw),
            Strategy::PhaseAndFrequency => {
                let t = interval.max(f64::MIN_POSITIVE);
                let a = u / t;
                (
                    l * ((1.0 + a).powi(2) + a * a),
                    coast_wf + c.q_wf() * u * u / t,
                    2.0 * c.ffm_avar() * u * u,
                    coast_rw + c.q_rw() * u * u * t / 3.0,
                )
            }
        };
        let cal = self.link.calibration_u_s.powi(2);
        Components {
            u_s: u,
            link,
            white_fm: wf,
            flicker_fm: ff,
            random_walk_fm: rw,
            calibration: cal,
            total: link + wf + ff + rw + cal,
        }
    }

    /// `σ(u)` for contacts `interval` apart (s).
    pub fn sigma(&self, u: f64, interval: f64) -> f64 {
        self.components(u, interval).total.sqrt()
    }

    /// The report for contacts `interval` apart, at coverage factor `k`.
    pub fn report(&self, interval: f64, k: f64, points: usize) -> Report {
        let n = points.max(2);
        let profile: Vec<ProfilePoint> = (0..n)
            .map(|i| {
                let u = interval * i as f64 / (n - 1) as f64;
                ProfilePoint {
                    u_s: u,
                    sigma_s: self.sigma(u, interval),
                }
            })
            .collect();
        // Trapezoidal mean of σ² over the cycle.
        let mut m = 0.0;
        for w in profile.windows(2) {
            m += 0.5 * (w[0].sigma_s.powi(2) + w[1].sigma_s.powi(2)) * (w[1].u_s - w[0].u_s);
        }
        Report {
            interval_s: interval,
            k,
            max_time_error_s: k * self.sigma(interval, interval),
            rms_time_error_s: (m / interval.max(f64::MIN_POSITIVE)).sqrt(),
            at_interval: self.components(interval, interval),
            profile,
            label: super::LABEL,
        }
    }

    /// The longest contact interval with `k σ(T) ≤ budget`, searched up to `max_s`; `None` when
    /// even an immediate re-contact cannot meet the budget (the link or calibration alone exceeds
    /// it). Returns `max_s` when the budget holds over the whole search range.
    pub fn required_interval(&self, budget_s: f64, k: f64, max_s: f64) -> Option<f64> {
        let f = |t: f64| k * self.sigma(t, t) - budget_s;
        let lo0 = 1e-3;
        if f(lo0) > 0.0 {
            return None;
        }
        if f(max_s) <= 0.0 {
            return Some(max_s);
        }
        let (mut lo, mut hi) = (lo0, max_s);
        for _ in 0..200 {
            let mid = (lo * hi).sqrt();
            if f(mid) <= 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi / lo < 1.0 + 1e-12 {
                break;
            }
        }
        Some(lo)
    }
}
