// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Multi-time-reference timebase.
//!
//! The simulator runs on one **truth** timeline (TAI, picoseconds). Every eLoran chain is
//! steered to a national UTC(k) realisation, every GNSS to its system time. Each reference
//! is a [`TimeRefModel`]: `x(t) = UTC(k) − UTC` (s) as offset + drift + steps + seeded
//! clock noise (white and random-walk frequency modulation). A transmitter emits when *its*
//! clock reads the nominal schedule, so the true emission instant is
//! `nominal − x_ref(t) − x_tx(t)` ([`Timebase::true_emission`]).
//!
//! GNSS system times ([`GnssTime`]) are offset from TAI by their defining constants (GPST
//! and GST: TAI − 19 s; BDT: TAI − 33 s; GLONASST: UTC(SU) + 3 h) plus a configurable
//! steering residual, so inter-system and chain-to-chain offsets are explicit in a scene.

use crate::time::Ps;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A scheduled phase step of a time reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeStep {
    /// Scene time of the step (s).
    pub at_s: f64,
    /// Size of the step (ns), added to the offset from then on.
    pub step_ns: f64,
}

/// A clock / time-scale model `x(t) = reference − UTC` (or − its parent).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeRefModel {
    /// Identifier, e.g. "UTC(NPL)", "UTC(KRIS)", "tx:Anthorn".
    pub id: String,
    /// Offset at scene start (ns).
    #[serde(default)]
    pub offset_ns: f64,
    /// Linear drift (ns/day).
    #[serde(default)]
    pub drift_ns_per_day: f64,
    /// White-FM noise level: Allan deviation at τ = 1 s (dimensionless). 0 = none.
    #[serde(default)]
    pub wfm_adev_1s: f64,
    /// Random-walk-FM level: Allan deviation contribution at τ = 1 s (dimensionless).
    #[serde(default)]
    pub rwfm_adev_1s: f64,
    #[serde(default)]
    pub steps: Vec<TimeStep>,
    /// Seed of the noise realisation.
    #[serde(default)]
    pub seed: u64,
}

impl TimeRefModel {
    pub fn ideal(id: &str) -> TimeRefModel {
        TimeRefModel {
            id: id.into(),
            offset_ns: 0.0,
            drift_ns_per_day: 0.0,
            wfm_adev_1s: 0.0,
            rwfm_adev_1s: 0.0,
            steps: vec![],
            seed: 0,
        }
    }
}

/// A realised time reference: the model plus its precomputed noise on a 1 s grid.
#[derive(Clone, Debug)]
pub struct TimeRef {
    pub model: TimeRefModel,
    noise_s: Vec<f64>,
}

impl TimeRef {
    /// Realise `model` over `duration_s` (noise sampled every second, linearly interpolated).
    pub fn new(model: TimeRefModel, duration_s: f64) -> TimeRef {
        let n = duration_s.max(0.0).ceil() as usize + 2;
        let mut noise_s = vec![0.0; n];
        if model.wfm_adev_1s > 0.0 || model.rwfm_adev_1s > 0.0 {
            let mut rng = ChaCha8Rng::seed_from_u64(model.seed ^ 0x5eed_7135);
            // White FM: fractional frequency y_k ~ N(0, σ_y(1)²) per 1 s; phase = Σ y.
            // Random-walk FM: frequency itself random walks with step σ·√3 per s (so that
            // its Allan deviation at 1 s is about rwfm_adev_1s).
            let mut x = 0.0;
            let mut f_rw = 0.0;
            for v in noise_s.iter_mut().skip(1) {
                let y_w = model.wfm_adev_1s * gauss(&mut rng);
                f_rw += model.rwfm_adev_1s * 3f64.sqrt() * gauss(&mut rng);
                x += y_w + f_rw;
                *v = x;
            }
        }
        TimeRef { model, noise_s }
    }

    /// `x(t)` in seconds at scene time `t_s`.
    pub fn offset_s(&self, t_s: f64) -> f64 {
        let m = &self.model;
        let mut x = m.offset_ns * 1e-9 + m.drift_ns_per_day * 1e-9 * t_s / 86400.0;
        for st in &m.steps {
            if t_s >= st.at_s {
                x += st.step_ns * 1e-9;
            }
        }
        if t_s >= 0.0 && self.noise_s.len() > 1 {
            let p = t_s.min((self.noise_s.len() - 2) as f64);
            let i = p.floor() as usize;
            let f = p - i as f64;
            x += self.noise_s[i] * (1.0 - f) + self.noise_s[i + 1] * f;
        }
        x
    }
}

/// Standard normal deviate (Box–Muller).
pub fn gauss<R: Rng>(rng: &mut R) -> f64 {
    let u1: f64 = rng.gen::<f64>().max(1e-300);
    let u2: f64 = rng.gen();
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// GNSS system times.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum GnssTime {
    Gpst,
    Gst,
    Bdt,
    Glonasst,
}

impl GnssTime {
    /// Defining offset `system − TAI` (s), with `tai_minus_utc` for GLONASST (UTC-based).
    pub fn minus_tai_s(self, tai_minus_utc: f64) -> f64 {
        match self {
            GnssTime::Gpst | GnssTime::Gst => -19.0,
            GnssTime::Bdt => -33.0,
            GnssTime::Glonasst => -tai_minus_utc + 3.0 * 3600.0,
        }
    }
}

/// All time references of a scene.
#[derive(Clone, Debug, Default)]
pub struct Timebase {
    refs: BTreeMap<String, TimeRef>,
    /// Steering residual of each GNSS time against its nominal definition (s).
    pub gnss_residual_s: BTreeMap<GnssTime, f64>,
}

impl Timebase {
    pub fn new(models: Vec<TimeRefModel>, duration_s: f64) -> Timebase {
        let refs = models
            .into_iter()
            .map(|m| (m.id.clone(), TimeRef::new(m, duration_s)))
            .collect();
        Timebase { refs, gnss_residual_s: BTreeMap::new() }
    }
    /// `x(t)` of reference `id` (0 for an unknown or empty id: the ideal UTC).
    pub fn offset_s(&self, id: &str, t_s: f64) -> f64 {
        self.refs.get(id).map(|r| r.offset_s(t_s)).unwrap_or(0.0)
    }
    pub fn has(&self, id: &str) -> bool {
        self.refs.contains_key(id)
    }
    pub fn ids(&self) -> Vec<String> {
        self.refs.keys().cloned().collect()
    }
    /// True emission time (scene time) of an event whose nominal time is `nominal` on a
    /// transmitter steered to `time_ref` with its own clock error model `tx_clock`.
    pub fn true_emission(&self, nominal: Ps, time_ref: &str, tx_clock: Option<&str>) -> Ps {
        let t = nominal.as_s();
        let mut x = self.offset_s(time_ref, t);
        if let Some(tx) = tx_clock {
            x += self.offset_s(tx, t);
        }
        nominal - Ps::from_s(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_drift_and_steps() {
        let mut m = TimeRefModel::ideal("UTC(X)");
        m.offset_ns = 10.0;
        m.drift_ns_per_day = 86.4; // 1 ns per 1000 s
        m.steps.push(TimeStep { at_s: 500.0, step_ns: 25.0 });
        let r = TimeRef::new(m, 2000.0);
        assert!((r.offset_s(0.0) - 10e-9).abs() < 1e-15);
        assert!((r.offset_s(1000.0) - (10e-9 + 1e-9 + 25e-9)).abs() < 1e-15);
    }

    #[test]
    fn true_emission_subtracts_reference_offset() {
        let mut m = TimeRefModel::ideal("UTC(A)");
        m.offset_ns = 100.0; // UTC(A) runs 100 ns ahead
        let tb = Timebase::new(vec![m], 10.0);
        let t = tb.true_emission(Ps::from_s(1.0), "UTC(A)", None);
        assert_eq!(t, Ps::from_s(1.0) - Ps::from_ns(100.0));
    }

    #[test]
    fn noise_is_deterministic_by_seed() {
        let mut m = TimeRefModel::ideal("n");
        m.wfm_adev_1s = 1e-11;
        m.seed = 7;
        let a = TimeRef::new(m.clone(), 100.0);
        let b = TimeRef::new(m, 100.0);
        assert_eq!(a.offset_s(57.3), b.offset_s(57.3));
        assert!(a.offset_s(57.3) != 0.0);
    }

    #[test]
    fn gnss_time_constants() {
        assert_eq!(GnssTime::Gpst.minus_tai_s(37.0), -19.0);
        assert_eq!(GnssTime::Bdt.minus_tai_s(37.0), -33.0);
    }
}
