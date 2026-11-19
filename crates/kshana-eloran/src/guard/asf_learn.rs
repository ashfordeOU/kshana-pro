// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Spoof-safe online ASF learning (patent candidate E3, research 04 §2; gap 01-12).
//! CONFIDENTIAL: Kshana Pro only.
//!
//! While GNSS and eLoran are both available, the receiver refines each station's ASF from
//! GNSS-derived truth (position and clock): the observed ASF of station `i` is
//! `pseudo-delay − PF·d_i/c − b_E`, with `b_E = b_G + δ` from the GNSS clock and the
//! time-scale offset. A slow GNSS spoofer during learning would **poison** the ASF state,
//! and the poison would persist into the GNSS outage the learning exists for. The learner
//! therefore only learns while integrity holds and cannot be dragged far even when the
//! drag goes undetected:
//!
//! 1. **Gate.** No update unless the timing protection level is under its threshold and no
//!    cross-system attribution alarm (E1, `kshana_pro::eloran_attrib`) is active. An
//!    attribution alarm also triggers a **rollback**.
//! 2. **Per-station Kalman filter** (random-walk ASF, process noise `q`) with a step clip of
//!    `rate_cap·Δt + 3·√(K·P)`: the physical ASF rate plus three sigma of the correction a
//!    correct model expects.
//! 3. **Attack-subspace cap.** A GNSS time push moves every observed ASF by the same
//!    amount; a GNSS position push moves them along the station bearing geometry
//!    `[1, −η·sin az_i, −η·cos az_i]`. Learning in that 3-dimensional subspace cannot be
//!    told from an attack, so the accumulated correction's component in it is held inside
//!    its **prior** (the ASF map's uncertainty): Mahalanobis `≤ χ²₃(1 − p_cap)`; beyond it the
//!    component is scaled back and the epoch is flagged. That part of the state is never
//!    treated as verified: its prior variance stays in the integrity budget
//!    ([`AsfLearner::sigma_budget_ns`]). With fewer than four stations everything is in the
//!    subspace and nothing beyond the prior can be learnt.
//! 4. **Common-mode CUSUM.** A two-sided CUSUM on the normalised common-mode innovation
//!    detects drags faster than the filter can follow quietly; an alarm freezes learning
//!    and rolls back.
//! 5. **Verified checkpoints and rollback.** A checkpoint is stored every
//!    `checkpoint_interval_s` and becomes *verified* after `verify_holdoff_s` without an
//!    alarm; an alarm restores the newest verified checkpoint (the drag may have started
//!    before it was detected) and freezes learning for `freeze_s`.
//!
//! # Limits (honest)
//!
//! * MODELLED: checked by internal Monte Carlo (convergence, a slow GNSS time drag, a
//!   position drag, rollback); no field data.
//! * A drag slower than the CUSUM sensitivity and inside the attack-subspace prior is
//!   learnt, up to [`AsfLearner::attack_subspace_bound_ns`]; that is the stated bound.
//! * Random-walk ASF model; diurnal/seasonal harmonics and meteorological regressors (E3's
//!   full form) are not modelled as states.
//! * The TPL itself (A3) and the attribution alarm (E1) are inputs.

use super::chi2_quantile;
use serde::{Deserialize, Serialize};

/// Primary factor (LF groundwave refractive index), as in [`crate::prop::ground::N_ATM`].
const ETA: f64 = crate::prop::ground::N_ATM;
/// ns per metre.
const NS_PER_M: f64 = 1e9 / crate::prop::C;

/// The map prior of one station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationPrior {
    pub id: String,
    /// ASF from the map / propagation model (ns) and its σ (ns).
    pub asf_ns: f64,
    pub sigma_ns: f64,
    /// Azimuth from the receiver to the station (rad, clockwise from north).
    pub bearing_rad: f64,
}

/// Settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsfLearnConfig {
    /// Random-walk process noise of the true ASF (ns²/s).
    pub q_ns2_per_s: f64,
    /// Largest physical ASF rate (ns/s).
    pub rate_cap_ns_per_s: f64,
    /// TPL threshold (ns).
    pub tpl_threshold_ns: f64,
    /// Probability outside the attack-subspace cap under the map prior.
    pub p_cap: f64,
    /// CUSUM slack and decision threshold (σ units).
    pub cusum_k: f64,
    pub cusum_h: f64,
    pub checkpoint_interval_s: f64,
    pub verify_holdoff_s: f64,
    /// Learning stays frozen this long after an alarm (s).
    pub freeze_s: f64,
    /// Disable guards 3–4 (for demonstrating poisoning only).
    pub unguarded: bool,
}

impl Default for AsfLearnConfig {
    fn default() -> Self {
        AsfLearnConfig {
            q_ns2_per_s: 1e-2,
            rate_cap_ns_per_s: 0.01,
            tpl_threshold_ns: 100.0,
            p_cap: 1e-3,
            cusum_k: 0.5,
            cusum_h: 12.0,
            checkpoint_interval_s: 600.0,
            verify_holdoff_s: 3600.0,
            freeze_s: 1800.0,
            unguarded: false,
        }
    }
}

/// One station's observed ASF from GNSS-derived truth.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsfObs {
    /// Index into the learner's stations.
    pub index: usize,
    pub asf_ns: f64,
    pub sigma_ns: f64,
}

/// Integrity inputs of an epoch.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    pub tpl_ns: f64,
    pub attribution_alarm: bool,
}

/// What an update did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Learned {
        /// The attack-subspace component was scaled back to its cap.
        capped: bool,
    },
    Frozen {
        reason: String,
    },
    RolledBack {
        reason: String,
        /// Time of the restored checkpoint (s); `None`: back to the map prior.
        to_t_s: Option<f64>,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    t_s: f64,
    asf: Vec<f64>,
    p: Vec<f64>,
}

/// The learner.
#[derive(Clone, Debug)]
pub struct AsfLearner {
    pub cfg: AsfLearnConfig,
    stations: Vec<StationPrior>,
    asf: Vec<f64>,
    p: Vec<f64>,
    last_t: Option<f64>,
    cusum: (f64, f64),
    checkpoints: Vec<Snapshot>,
    last_alarm_t: Option<f64>,
    /// Attack-subspace prior: basis A (n×3) and Σθ = (AᵀWA)⁻¹ (None if singular).
    basis: Vec<[f64; 3]>,
    sigma_theta: Option<[[f64; 3]; 3]>,
    cap: f64,
}

fn inv3(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if d.abs() < 1e-300 {
        return None;
    }
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let (a, b) = ((j + 1) % 3, (j + 2) % 3);
            let (c, e) = ((i + 1) % 3, (i + 2) % 3);
            r[i][j] = (m[a][c] * m[b][e] - m[a][e] * m[b][c]) / d;
        }
    }
    Some(r)
}

impl AsfLearner {
    pub fn new(stations: Vec<StationPrior>, cfg: AsfLearnConfig) -> Result<AsfLearner, String> {
        if stations.is_empty() || stations.iter().any(|s| !(s.sigma_ns > 0.0)) {
            return Err("asf_learn: stations with a positive prior σ are needed".into());
        }
        // ns per metre of position along each station's bearing (pseudo-delay sensitivity)
        let basis: Vec<[f64; 3]> = stations
            .iter()
            .map(|s| {
                [
                    1.0,
                    -ETA * s.bearing_rad.sin() * NS_PER_M,
                    -ETA * s.bearing_rad.cos() * NS_PER_M,
                ]
            })
            .collect();
        let mut atwa = [[0.0; 3]; 3];
        for (a, s) in basis.iter().zip(&stations) {
            let w = 1.0 / (s.sigma_ns * s.sigma_ns);
            for i in 0..3 {
                for j in 0..3 {
                    atwa[i][j] += w * a[i] * a[j];
                }
            }
        }
        let sigma_theta = if stations.len() >= 3 {
            inv3(atwa)
        } else {
            None
        };
        Ok(AsfLearner {
            asf: stations.iter().map(|s| s.asf_ns).collect(),
            p: stations.iter().map(|s| s.sigma_ns * s.sigma_ns).collect(),
            cap: chi2_quantile(1.0 - cfg.p_cap, 3),
            cfg,
            stations,
            last_t: None,
            cusum: (0.0, 0.0),
            checkpoints: Vec::new(),
            last_alarm_t: None,
            basis,
            sigma_theta,
        })
    }

    /// Current ASF estimates (ns).
    pub fn asf_ns(&self) -> &[f64] {
        &self.asf
    }

    /// Kalman σ of each station (ns), excluding the unverifiable part.
    pub fn sigma_kalman_ns(&self) -> Vec<f64> {
        self.p.iter().map(|p| p.sqrt()).collect()
    }

    /// Integrity budget σ of each station (ns): the Kalman variance plus the prior variance
    /// of the attack-subspace component, which learning cannot verify.
    pub fn sigma_budget_ns(&self) -> Vec<f64> {
        let unver = self.unverifiable_var();
        self.p
            .iter()
            .zip(unver)
            .map(|(p, u)| (p + u).sqrt())
            .collect()
    }

    fn unverifiable_var(&self) -> Vec<f64> {
        match &self.sigma_theta {
            Some(st) => self
                .basis
                .iter()
                .map(|a| {
                    let mut v = 0.0;
                    for i in 0..3 {
                        for j in 0..3 {
                            v += a[i] * st[i][j] * a[j];
                        }
                    }
                    v
                })
                .collect(),
            None => self.stations.iter().map(|s| s.sigma_ns.powi(2)).collect(),
        }
    }

    /// Largest common-mode shift (ns) the attack-subspace cap allows relative to the map
    /// prior: `√(χ²₃(1 − p_cap) · Σθ₀₀)`. With fewer than three stations: 3 × the largest σ.
    pub fn attack_subspace_bound_ns(&self) -> f64 {
        match &self.sigma_theta {
            Some(st) => (self.cap * st[0][0]).sqrt(),
            None => 3.0 * self.stations.iter().map(|s| s.sigma_ns).fold(0.0, f64::max),
        }
    }

    /// Per-station bound (ns) on the learnt correction's attack-subspace component:
    /// `√(χ²₃(1 − p_cap) · aᵢᵀ Σθ aᵢ)` (time and position pushes together).
    pub fn attack_subspace_bound_station_ns(&self) -> Vec<f64> {
        self.unverifiable_var()
            .iter()
            .map(|v| (self.cap * v).sqrt())
            .collect()
    }

    /// Restore the newest verified checkpoint at time `t_s` (or the map prior when none is
    /// verified); returns the restored checkpoint time.
    pub fn rollback(&mut self, t_s: f64) -> Option<f64> {
        let holdoff = self.cfg.verify_holdoff_s;
        // verified: stored at least the holdoff ago (checkpoints newer than a restored one
        // were discarded at that restore, so every older one survived an alarm-free holdoff)
        let pos = self
            .checkpoints
            .iter()
            .rposition(|c| t_s - c.t_s >= holdoff);
        let to = match pos {
            Some(i) => {
                let c = self.checkpoints[i].clone();
                self.asf = c.asf;
                self.p = c.p;
                self.checkpoints.truncate(i + 1);
                Some(c.t_s)
            }
            None => {
                self.asf = self.stations.iter().map(|s| s.asf_ns).collect();
                self.p = self.stations.iter().map(|s| s.sigma_ns.powi(2)).collect();
                self.checkpoints.clear();
                None
            }
        };
        self.cusum = (0.0, 0.0);
        self.last_alarm_t = Some(t_s);
        to
    }

    /// Process one epoch at `t_s`.
    pub fn update(&mut self, t_s: f64, obs: &[AsfObs], gate: Gate) -> Status {
        let dt = self.last_t.map(|t| (t_s - t).max(0.0)).unwrap_or(0.0);
        self.last_t = Some(t_s);
        for p in self.p.iter_mut() {
            *p += self.cfg.q_ns2_per_s * dt;
        }
        if gate.attribution_alarm {
            let to = self.rollback(t_s);
            return Status::RolledBack {
                reason: "cross-system attribution alarm".into(),
                to_t_s: to,
            };
        }
        if let Some(ta) = self.last_alarm_t {
            if t_s - ta < self.cfg.freeze_s {
                return Status::Frozen {
                    reason: "post-alarm freeze".into(),
                };
            }
        }
        if !(gate.tpl_ns <= self.cfg.tpl_threshold_ns) {
            return Status::Frozen {
                reason: format!(
                    "timing protection level {:.1} ns above {:.1} ns",
                    gate.tpl_ns, self.cfg.tpl_threshold_ns
                ),
            };
        }
        let obs: Vec<&AsfObs> = obs
            .iter()
            .filter(|o| o.index < self.asf.len() && o.sigma_ns > 0.0)
            .collect();
        if obs.is_empty() {
            return Status::Frozen {
                reason: "no observations".into(),
            };
        }
        // common-mode CUSUM on the normalised weighted-mean innovation
        if !self.cfg.unguarded {
            let (mut sw, mut swv) = (0.0, 0.0);
            for o in &obs {
                let v = o.sigma_ns.powi(2) + self.p[o.index];
                sw += 1.0 / v;
                swv += (o.asf_ns - self.asf[o.index]) / v;
            }
            let z = (swv / sw) * sw.sqrt();
            let k = self.cfg.cusum_k;
            self.cusum.0 = (self.cusum.0 + z - k).max(0.0);
            self.cusum.1 = (self.cusum.1 - z - k).max(0.0);
            if self.cusum.0 > self.cfg.cusum_h || self.cusum.1 > self.cfg.cusum_h {
                let to = self.rollback(t_s);
                return Status::RolledBack {
                    reason: "common-mode drift (CUSUM)".into(),
                    to_t_s: to,
                };
            }
        }
        // per-station Kalman update with the step clip
        for o in &obs {
            let i = o.index;
            let r = o.sigma_ns * o.sigma_ns;
            let k = self.p[i] / (self.p[i] + r);
            let clip = self.cfg.rate_cap_ns_per_s * dt + 3.0 * (k * self.p[i]).sqrt();
            let step = (k * (o.asf_ns - self.asf[i])).clamp(-clip, clip);
            self.asf[i] += step;
            self.p[i] *= 1.0 - k;
        }
        let capped = !self.cfg.unguarded && self.apply_cap();
        // checkpoints
        let due = self
            .checkpoints
            .last()
            .is_none_or(|c| t_s - c.t_s >= self.cfg.checkpoint_interval_s);
        if due {
            self.checkpoints.push(Snapshot {
                t_s,
                asf: self.asf.clone(),
                p: self.p.clone(),
            });
        }
        Status::Learned { capped }
    }

    /// Hold the attack-subspace component of the learnt correction inside its prior.
    fn apply_cap(&mut self) -> bool {
        let Some(st) = self.sigma_theta else {
            // < 3 stations: every station inside its own 3σ prior
            let mut c = false;
            for (a, s) in self.asf.iter_mut().zip(&self.stations) {
                let lim = 3.0 * s.sigma_ns;
                let d = (*a - s.asf_ns).clamp(-lim, lim);
                if s.asf_ns + d != *a {
                    *a = s.asf_ns + d;
                    c = true;
                }
            }
            return c;
        };
        // θ = Σθ AᵀW c
        let c: Vec<f64> = self
            .asf
            .iter()
            .zip(&self.stations)
            .map(|(a, s)| a - s.asf_ns)
            .collect();
        let mut atwc = [0.0; 3];
        for ((a, s), ci) in self.basis.iter().zip(&self.stations).zip(&c) {
            let w = 1.0 / s.sigma_ns.powi(2);
            for k in 0..3 {
                atwc[k] += w * a[k] * ci;
            }
        }
        let mut th = [0.0; 3];
        for i in 0..3 {
            for j in 0..3 {
                th[i] += st[i][j] * atwc[j];
            }
        }
        let q: f64 = (0..3).map(|i| th[i] * atwc[i]).sum(); // θᵀ Σθ⁻¹ θ
        if q <= self.cap {
            return false;
        }
        let f = 1.0 - (self.cap / q).sqrt();
        for (a, b) in self.asf.iter_mut().zip(&self.basis) {
            *a -= f * (b[0] * th[0] + b[1] * th[1] + b[2] * th[2]);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timebase::gauss;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    const N: usize = 6;
    const SIG_MAP: f64 = 50.0;
    const SIG_OBS: f64 = 30.0;

    fn priors(rng: &mut ChaCha8Rng) -> (Vec<StationPrior>, Vec<f64>) {
        let mut p = Vec::new();
        let mut truth = Vec::new();
        for i in 0..N {
            let map = 300.0 + 150.0 * i as f64;
            truth.push(map + SIG_MAP * gauss(rng));
            p.push(StationPrior {
                id: format!("S{i}"),
                asf_ns: map,
                sigma_ns: SIG_MAP,
                bearing_rad: i as f64 * std::f64::consts::TAU / N as f64 + 0.2,
            });
        }
        (p, truth)
    }

    /// True ASF at `t`: static truth plus a diurnal term scaled per station (land fraction).
    fn true_asf(truth: &[f64], t: f64) -> Vec<f64> {
        truth
            .iter()
            .enumerate()
            .map(|(i, a)| {
                a + 15.0
                    * (0.3 + 0.12 * i as f64)
                    * (std::f64::consts::TAU * t / 86_400.0 + i as f64).sin()
            })
            .collect()
    }

    fn obs(rng: &mut ChaCha8Rng, asf: &[f64], extra: impl Fn(usize) -> f64) -> Vec<AsfObs> {
        asf.iter()
            .enumerate()
            .map(|(i, a)| AsfObs {
                index: i,
                asf_ns: a + extra(i) + SIG_OBS * gauss(rng),
                sigma_ns: SIG_OBS,
            })
            .collect()
    }

    const CLEAN: Gate = Gate {
        tpl_ns: 40.0,
        attribution_alarm: false,
    };

    fn rms(a: &[f64], b: &[f64]) -> f64 {
        (a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f64>() / a.len() as f64).sqrt()
    }

    /// Clean data: over 20 random maps, 6 h of 10 s epochs bring the RMS ASF error from the
    /// map's 50 ns to ≤ 8 ns (stated), cap the attack subspace in < 1 % of epochs, and
    /// the Kalman σ falls well below the prior.
    #[test]
    fn learning_converges_on_clean_data() {
        let mut worst: f64 = 0.0;
        let mut capped = 0;
        let mut total = 0;
        for seed in 0..20 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let (p, truth) = priors(&mut rng);
            let map: Vec<f64> = p.iter().map(|s| s.asf_ns).collect();
            let mut l = AsfLearner::new(p, AsfLearnConfig::default()).unwrap();
            let e0 = rms(&map, &true_asf(&truth, 0.0));
            for k in 0..2160 {
                let t = 10.0 * k as f64;
                let o = obs(&mut rng, &true_asf(&truth, t), |_| 0.0);
                match l.update(t, &o, CLEAN) {
                    Status::Learned { capped: c } => {
                        total += 1;
                        if c {
                            capped += 1;
                        }
                    }
                    s => panic!("clean data froze learning: {s:?}"),
                }
            }
            let e1 = rms(l.asf_ns(), &true_asf(&truth, 21_600.0));
            worst = worst.max(e1);
            assert!(e1 < e0 || e0 < 12.0, "seed {seed}: {e0} → {e1}");
            assert!(l.sigma_kalman_ns().iter().all(|s| *s < 10.0));
            assert!(l
                .sigma_budget_ns()
                .iter()
                .all(|s| *s > l.sigma_kalman_ns()[0]));
        }
        println!("E3 clean: worst final RMS {worst:.1} ns; capped epochs {capped}/{total}");
        assert!(worst <= 8.0, "worst RMS error {worst}");
        assert!((capped as f64) / (total as f64) < 0.01);
    }

    /// A slow GNSS time drag (0.2 ns/s for 3 h = 2160 ns) after 3 h of clean learning, no
    /// TPL or attribution alarm (worst case: the drag is not caught upstream). Stated: the
    /// learnt ASF's common-mode shift stays ≤ the attack-subspace bound and the CUSUM rolls
    /// it back so that no station moves > 15 ns; an unguarded learner is dragged > 1000 ns.
    /// A GNSS position drag (2 m/min to 360 m, ≈ 1.2 µs of geometry-shaped ASF poison) is
    /// not common-mode: each station's shift stays ≤ its attack-subspace bound + 15 ns
    /// (≈ 120 ns here; E1 is expected to flag such a drag upstream).
    #[test]
    fn slow_gnss_drag_does_not_poison_the_asf() {
        for (label, drag) in [
            (
                "time",
                Box::new(|_i: usize, dt: f64| 0.2 * dt) as Box<dyn Fn(usize, f64) -> f64>,
            ),
            (
                "position",
                Box::new(|i: usize, dt: f64| {
                    // receiver dragged north at 2 m/min: ASF_obs_i = pd − PF·d_true·… gains
                    // −η·cos(az_i)·Δn (ns per m)
                    let az = i as f64 * std::f64::consts::TAU / N as f64 + 0.2;
                    let dn = (dt / 60.0 * 2.0).min(360.0);
                    -ETA * az.cos() * dn * NS_PER_M
                }),
            ),
        ] {
            for unguarded in [false, true] {
                let mut rng = ChaCha8Rng::seed_from_u64(42);
                let (p, truth) = priors(&mut rng);
                let mut cfg = AsfLearnConfig::default();
                cfg.unguarded = unguarded;
                let mut l = AsfLearner::new(p, cfg).unwrap();
                let bound = l.attack_subspace_bound_ns();
                let st_bound = l.attack_subspace_bound_station_ns();
                let t0 = 10_800.0;
                let mut pre = vec![];
                let mut rolled = 0;
                for k in 0..2160 {
                    let t = 10.0 * k as f64;
                    if t == t0 {
                        pre = l.asf_ns().to_vec();
                    }
                    let o = obs(&mut rng, &true_asf(&truth, t), |i| {
                        if t >= t0 {
                            drag(i, t - t0)
                        } else {
                            0.0
                        }
                    });
                    if let Status::RolledBack { .. } = l.update(t, &o, CLEAN) {
                        rolled += 1;
                    }
                }
                let end = true_asf(&truth, 21_590.0);
                let pre_t = true_asf(&truth, t0);
                // shift of the learnt state relative to the true ASF change over the drag
                let shift: Vec<f64> = l
                    .asf_ns()
                    .iter()
                    .zip(&pre)
                    .zip(end.iter().zip(&pre_t))
                    .map(|((a, b), (x, y))| (a - b) - (x - y))
                    .collect();
                let cm = shift.iter().sum::<f64>() / N as f64;
                let max = shift.iter().fold(0.0f64, |m, s| m.max(s.abs()));
                println!(
                    "E3 {label} drag, unguarded={unguarded}: common-mode shift {cm:.1} ns, max station shift {max:.1} ns, rollbacks {rolled}, bound {bound:.1} ns"
                );
                if unguarded {
                    assert!(
                        max > 1000.0 || (label == "position" && max > 300.0),
                        "{label}: unguarded learner was not dragged ({max})"
                    );
                } else {
                    assert!(
                        cm.abs() <= bound,
                        "{label}: common-mode shift {cm} > bound {bound}"
                    );
                    if label == "time" {
                        // caught by the CUSUM and rolled back
                        assert!(rolled > 0 && max <= 15.0, "{label}: station shift {max} ns");
                    } else {
                        // not common-mode: held by the attack-subspace cap per station
                        for (i, sh) in shift.iter().enumerate() {
                            assert!(
                                sh.abs() <= st_bound[i] + 15.0,
                                "{label}: station {i} shift {sh} > {}",
                                st_bound[i]
                            );
                        }
                    }
                }
            }
        }
    }

    /// Rollback: a drag starts at 3 h, the attribution alarm arrives 20 min later; the
    /// learner restores the newest checkpoint verified before the drag (holdoff 1 h) and
    /// stays frozen for `freeze_s`; then learning resumes on clean data.
    #[test]
    fn rollback_restores_the_verified_state() {
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let (p, truth) = priors(&mut rng);
        let mut cfg = AsfLearnConfig::default();
        cfg.cusum_h = 1e9; // isolate the alarm path
        let mut l = AsfLearner::new(p, cfg.clone()).unwrap();
        let t0 = 10_800.0;
        let t_alarm = t0 + 1200.0;
        let mut at_t0 = vec![];
        let mut status_at_alarm = None;
        for k in 0..2160 {
            let t = 10.0 * k as f64;
            if t == t0 {
                at_t0 = l.asf_ns().to_vec();
            }
            let o = obs(&mut rng, &true_asf(&truth, t), |_| {
                if t >= t0 && t < t_alarm + 600.0 {
                    1.0 * (t - t0)
                } else {
                    0.0
                }
            });
            let gate = Gate {
                tpl_ns: 40.0,
                attribution_alarm: t == t_alarm,
            };
            let s = l.update(t, &o, gate);
            if t == t_alarm {
                status_at_alarm = Some((s.clone(), l.asf_ns().to_vec()));
            }
            if t > t_alarm && t < t_alarm + cfg.freeze_s {
                assert!(matches!(s, Status::Frozen { .. }), "{t}: {s:?}");
            }
        }
        let (s, restored) = status_at_alarm.unwrap();
        let to = match s {
            Status::RolledBack { to_t_s, .. } => to_t_s.expect("a verified checkpoint exists"),
            other => panic!("{other:?}"),
        };
        assert!(
            to <= t_alarm - cfg.verify_holdoff_s && to < t0,
            "restored {to}"
        );
        // the restored state is a checkpoint taken before the drag: close to the pre-drag state
        assert!(rms(&restored, &at_t0) < 6.0, "{}", rms(&restored, &at_t0));
        // and learning resumed afterwards: the final state tracks the truth again
        let e = rms(l.asf_ns(), &true_asf(&truth, 21_590.0));
        assert!(e < 15.0, "after rollback {e}");
        // gate: a TPL above threshold freezes
        assert!(matches!(
            l.update(
                30_000.0,
                &obs(&mut rng, &true_asf(&truth, 30_000.0), |_| 0.0),
                Gate {
                    tpl_ns: 500.0,
                    attribution_alarm: false
                }
            ),
            Status::Frozen { .. }
        ));
    }
}
