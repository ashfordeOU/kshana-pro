// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Meacon and relay detection from propagation-signature consistency (patent candidate E4,
//! research 04 §2; gap 01-3).
//!
//! A relayed (meaconed) LORAN signal reproduces the pulse and phase-code structure but not
//! the **path-dependent distortion** of the claimed transmitter-to-receiver path. For each
//! tracked station the detector compares what the receiver measured with what the
//! propagation model predicts at the receiver for that station:
//!
//! | feature | prediction | meacon / relay signature |
//! |---|---|---|
//! | arrival delay | `PF + SF + ASF` of the path ([`crate::prop::PathEffect::delay`]), σ from the ASF uncertainty | late: a relay path is never shorter than the direct path, plus processing delay |
//! | ECD | transmitted ECD + propagation ECD ([`crate::prop::PathEffect::ecd_s`]) | a regenerated or locally re-radiated replica lacks the long-path dispersion |
//! | field strength | groundwave field (dBµV/m) | set by the attacker (a capture margin above the real signal) |
//! | skywave | delay after the groundwave and sky/ground ratio of the path | absent or that of the relay site |
//! | bearing (crossed loops) | geodesic azimuth to the station | points to the relay |
//! | phase-code alignment | 0 | an external code-correlation stage reports a misalignment |
//! | cross-station TOA geometry | the other stations' common clock | the station's arrival disagrees with the clock the other stations agree on |
//!
//! Each residual is normalised by the root-sum-square of the model and measurement σ.
//! Two statistics are formed:
//!
//! * the **signature** statistic `S = Σ z²` over the timing-independent features (ECD,
//!   field, skywave, bearing), χ² with `k` degrees of freedom under "consistent";
//! * the **timing** statistic `z_d²` of the arrival delay (two-sided: a transmitter timing
//!   fault can be early or late; a relay is always late).
//!
//! Classification ([`Class`]): **Relay** when `S` exceeds its χ²_k quantile at
//! `p_fa_signature`, or when the timing is anomalous and late *and* `S` exceeds the weaker
//! quantile at `p_weak_signature` (a confirmed timing anomaly lowers the bar for the
//! signature evidence that decides relay versus transmitter fault); **TimingOnly** when the
//! timing (or phase code) is anomalous but the propagation signature is consistent
//! (a transmitter timing fault, or an ideal relay that reproduces field and ECD);
//! **Consistent** otherwise. A generalised log-likelihood ratio for the relay hypothesis
//! ([`StationVerdict::relay_llr`]) is returned for the cross-system attribution (E1).
//!
//! The clock needed for the delay feature is either supplied (e.g. GNSS-derived, with its
//! σ) or estimated per station from the **other** stations (leave-one-out weighted mean of
//! their delay residuals: the cross-station TOA geometry check at the claimed position).
//!
//! # Limits (honest)
//!
//! * MODELLED: checked by internal Monte Carlo and by the crate's own receiver against the
//!   crate's own threat models ([`crate::threat::Meaconer`]); not validated against
//!   recordings of a real meacon.
//! * The receiver position must be known or claimed (survey, GNSS while trusted, or the
//!   eLoran fix): the predictions are evaluated there.
//! * The software receiver measures delay, ECD, field and (beam-steered loops) bearing;
//!   skywave delay/ratio and phase-code alignment are accepted from an external stage
//!   when available.
//! * A relay placed close to the victim with a faithful wideband chain and a field matched
//!   to the prediction leaves only the timing signature; it is then reported as
//!   `TimingOnly` and cannot be told from a transmitter timing fault by this detector alone.
//! * No relay localisation beyond reporting the measured bearing and the excess delay.

use super::chi2_quantile;
use crate::chain::{Chain, Station};
use crate::geo::inverse;
use crate::prop::{PathKind, PropagationModel};
use crate::rx::{nominal_emission_s, Measurement, PositionSolution};
use crate::time::AbsTime;
use crate::traj::RxState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Model (prediction) uncertainties, one sigma. Defaults are MODELLED assumptions for a
/// mapped area; take the delay and field σ from [`crate::prop::uncertainty`] where a run
/// exists ([`Expected::with_uncertainty`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSigmas {
    pub delay_ns: f64,
    pub ecd_us: f64,
    pub field_db: f64,
    pub sky_delay_us: f64,
    pub sky_ratio_db: f64,
    pub bearing_deg: f64,
}

impl Default for ModelSigmas {
    fn default() -> Self {
        ModelSigmas {
            delay_ns: 100.0,
            ecd_us: 0.15,
            field_db: 2.5,
            sky_delay_us: 5.0,
            sky_ratio_db: 6.0,
            bearing_deg: 5.0,
        }
    }
}

/// Predicted skywave structure of a path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkyExpected {
    /// Skywave delay after the groundwave (µs) and sky/ground ratio (dB).
    pub delay_after_ground_us: f64,
    pub ratio_db: f64,
}

/// The propagation signature predicted for one station at the receiver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Expected {
    /// `chain/station`.
    pub station: String,
    pub delay_ns: f64,
    pub ecd_us: f64,
    pub field_dbuv_m: f64,
    pub sky: Option<SkyExpected>,
    /// Geodesic azimuth from the receiver to the station (deg, clockwise from north).
    pub bearing_deg: f64,
    pub distance_km: f64,
    pub sigma: ModelSigmas,
}

impl Expected {
    /// Predict from a propagation model: the groundwave path (and the first skywave hop,
    /// when the model has one) from `station` (of chain `chain`) to the receiver state `rx`.
    pub fn from_model(
        model: &dyn PropagationModel,
        chain: &str,
        station: &Station,
        rx: &RxState,
        sigma: ModelSigmas,
    ) -> Result<Expected, String> {
        let paths = model.paths(station, rx);
        let g = paths
            .iter()
            .find(|p| p.kind == PathKind::Ground)
            .ok_or("meacon: the model returned no groundwave path")?;
        let sky = paths
            .iter()
            .find(|p| matches!(p.kind, PathKind::Sky { .. }))
            .map(|s| SkyExpected {
                delay_after_ground_us: (s.delay - g.delay).as_us(),
                ratio_db: 20.0 * (s.field_uv_m.max(1e-30) / g.field_uv_m.max(1e-30)).log10(),
            });
        let inv = inverse(rx.pos, station.position);
        Ok(Expected {
            station: format!("{chain}/{}", station.id),
            delay_ns: g.delay.as_s() * 1e9,
            ecd_us: station.ecd_us + g.ecd_s * 1e6,
            field_dbuv_m: 20.0 * g.field_uv_m.max(1e-30).log10(),
            sky,
            bearing_deg: inv.az1_rad.to_degrees().rem_euclid(360.0),
            distance_km: inv.distance_m / 1000.0,
            sigma,
        })
    }

    /// Take the delay and field σ (and the nominal delay/field) from a Monte Carlo
    /// propagation-uncertainty result for this path.
    pub fn with_uncertainty(mut self, r: &crate::prop::uncertainty::PointResult) -> Expected {
        self.sigma.delay_ns = r.delay_ns.sd;
        self.sigma.field_db = r.field_dbuv_m.sd.max(0.5);
        self
    }
}

/// What the receiver measured for one station (any feature may be absent).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Observed {
    /// `chain/station`.
    pub station: String,
    /// Pseudo-delay: arrival minus nominal emission (ns), including the receiver clock and
    /// the station's time-scale offset.
    pub pseudo_delay_ns: Option<f64>,
    pub sigma_delay_ns: f64,
    pub ecd_us: Option<f64>,
    pub sigma_ecd_us: f64,
    pub field_dbuv_m: Option<f64>,
    pub sigma_field_db: f64,
    /// Measured skywave delay after the groundwave (µs) and sky/ground ratio (dB), from an
    /// external skywave estimator.
    pub sky_delay_after_ground_us: Option<f64>,
    pub sky_ratio_db: Option<f64>,
    /// Phase-code alignment offset reported by an external code-correlation stage (µs);
    /// expected 0.
    pub phase_code_offset_us: Option<f64>,
    /// Measured true bearing (deg) from a crossed-loop antenna (heading applied).
    pub bearing_deg: Option<f64>,
}

impl Observed {
    /// Aggregate the software receiver's measurements per station (means, and the standard
    /// error of each mean from the spread). `heading_deg` turns the loop-relative bearing
    /// into a true bearing.
    pub fn from_measurements(
        meas: &[Measurement],
        chains: &[Chain],
        start: AbsTime,
        heading_deg: f64,
    ) -> Vec<Observed> {
        let mut by: BTreeMap<(usize, usize), Vec<&Measurement>> = BTreeMap::new();
        for m in meas {
            by.entry((m.chain_index, m.station_index))
                .or_default()
                .push(m);
        }
        let mean_se = |v: &[f64]| -> (f64, f64) {
            let n = v.len().max(1) as f64;
            let m = v.iter().sum::<f64>() / n;
            let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
            (m, (var / n).sqrt())
        };
        by.into_iter()
            .map(|((ci, si), v)| {
                let pd: Vec<f64> = v
                    .iter()
                    .map(|m| (m.toa_s - nominal_emission_s(&chains[ci], si, m.group, start)) * 1e9)
                    .collect();
                let ecd: Vec<f64> = v.iter().map(|m| m.ecd_us).collect();
                let fld: Vec<f64> = v
                    .iter()
                    .map(|m| 20.0 * m.field_uv_m.max(1e-30).log10())
                    .collect();
                let brg: Vec<f64> = v.iter().filter_map(|m| m.bearing_deg).collect();
                let (pd_m, pd_s) = mean_se(&pd);
                let (e_m, e_s) = mean_se(&ecd);
                let (f_m, f_s) = mean_se(&fld);
                Observed {
                    station: format!("{}/{}", v[0].chain, v[0].station),
                    pseudo_delay_ns: Some(pd_m),
                    sigma_delay_ns: pd_s.max(1.0),
                    ecd_us: Some(e_m),
                    sigma_ecd_us: e_s.max(0.01),
                    field_dbuv_m: Some(f_m),
                    sigma_field_db: f_s.max(0.1),
                    bearing_deg: (!brg.is_empty()).then(|| {
                        (brg.iter().sum::<f64>() / brg.len() as f64 + heading_deg).rem_euclid(360.0)
                    }),
                    ..Default::default()
                }
            })
            .collect()
    }
}

/// A clock reference for the delay feature: receiver clock minus the stations' time scale
/// (ns) and its σ.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClockRef {
    pub bias_ns: f64,
    pub sigma_ns: f64,
}

/// Detector settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeaconConfig {
    /// False-alarm probability of the signature test (per station).
    pub p_fa_signature: f64,
    /// False-alarm probability of the timing test (per station).
    pub p_fa_timing: f64,
    /// Weaker signature bar applied once the timing is anomalous and late.
    pub p_weak_signature: f64,
}

impl Default for MeaconConfig {
    fn default() -> Self {
        MeaconConfig {
            p_fa_signature: 5e-4,
            p_fa_timing: 5e-4,
            p_weak_signature: 1e-2,
        }
    }
}

/// One normalised residual.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    /// Measured − predicted (feature units).
    pub residual: f64,
    pub sigma: f64,
    pub z: f64,
}

/// The detector's class for one station.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Consistent,
    /// Timing (or phase-code) anomaly with a consistent propagation signature.
    TimingOnly,
    /// The propagation signature does not match the claimed path: meacon or relay.
    Relay,
}

/// Verdict for one station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationVerdict {
    pub station: String,
    pub class: Class,
    /// Timing-independent signature features and their statistic, dof and threshold.
    pub signature: Vec<Feature>,
    pub signature_stat: f64,
    pub signature_dof: usize,
    pub signature_threshold: f64,
    /// Delay feature (when a clock was available) and phase-code feature.
    pub timing: Option<Feature>,
    pub phase_code: Option<Feature>,
    pub timing_threshold: f64,
    /// Excess arrival delay over the prediction (ns), when timing was available.
    pub excess_delay_ns: Option<f64>,
    /// Bearing the relay appears on (deg), when measured and inconsistent.
    pub relay_bearing_deg: Option<f64>,
    /// Generalised log-likelihood ratio of "relay" against "consistent":
    /// `½·(S + max(z_d, 0)²) − ½·(k + ½)` (the expected value under "consistent" removed).
    pub relay_llr: f64,
    pub explanation: Vec<String>,
}

fn feat(name: &str, residual: f64, sigma: f64) -> Feature {
    Feature {
        name: name.into(),
        residual,
        sigma,
        z: residual / sigma,
    }
}

fn wrap180(x: f64) -> f64 {
    (x + 180.0).rem_euclid(360.0) - 180.0
}

/// Test one station given a clock reference (or `None`: no delay feature).
pub fn test_station(
    obs: &Observed,
    exp: &Expected,
    clock: Option<ClockRef>,
    cfg: &MeaconConfig,
) -> StationVerdict {
    let s = &exp.sigma;
    let rss = |a: f64, b: f64| (a * a + b * b).sqrt();
    let mut sig = Vec::new();
    if let Some(e) = obs.ecd_us {
        sig.push(feat(
            "ECD (µs)",
            e - exp.ecd_us,
            rss(s.ecd_us, obs.sigma_ecd_us),
        ));
    }
    if let Some(f) = obs.field_dbuv_m {
        sig.push(feat(
            "field (dB)",
            f - exp.field_dbuv_m,
            rss(s.field_db, obs.sigma_field_db),
        ));
    }
    if let Some(sky) = &exp.sky {
        if let Some(d) = obs.sky_delay_after_ground_us {
            sig.push(feat(
                "skywave delay (µs)",
                d - sky.delay_after_ground_us,
                s.sky_delay_us,
            ));
        }
        if let Some(r) = obs.sky_ratio_db {
            sig.push(feat(
                "sky/ground ratio (dB)",
                r - sky.ratio_db,
                s.sky_ratio_db,
            ));
        }
    }
    if let Some(b) = obs.bearing_deg {
        sig.push(feat(
            "bearing (deg)",
            wrap180(b - exp.bearing_deg),
            s.bearing_deg,
        ));
    }
    let k = sig.len();
    let stat: f64 = sig.iter().map(|f| f.z * f.z).sum();
    let thr = chi2_quantile(1.0 - cfg.p_fa_signature, k);
    let thr_weak = chi2_quantile(1.0 - cfg.p_weak_signature, k);
    let t_thr = chi2_quantile(1.0 - cfg.p_fa_timing, 1);
    let timing = match (obs.pseudo_delay_ns, clock) {
        (Some(pd), Some(c)) => Some(feat(
            "arrival delay (ns)",
            pd - c.bias_ns - exp.delay_ns,
            (s.delay_ns.powi(2) + obs.sigma_delay_ns.powi(2) + c.sigma_ns.powi(2)).sqrt(),
        )),
        _ => None,
    };
    let phase_code = obs
        .phase_code_offset_us
        .map(|p| feat("phase-code alignment (µs)", p, 1.0));
    let timing_anom = timing.as_ref().is_some_and(|t| t.z * t.z > t_thr);
    let late = timing.as_ref().is_some_and(|t| t.z > 0.0);
    let code_anom = phase_code.as_ref().is_some_and(|p| p.z * p.z > t_thr);
    let sig_flag = k > 0 && stat > thr;
    let weak = k > 0 && stat > thr_weak;
    let class = if sig_flag || (timing_anom && late && weak) {
        Class::Relay
    } else if timing_anom || code_anom {
        Class::TimingOnly
    } else {
        Class::Consistent
    };
    let zl = timing.as_ref().map(|t| t.z.max(0.0)).unwrap_or(0.0);
    let relay_llr = 0.5 * (stat + zl * zl) - 0.5 * (k as f64 + 0.5);
    let mut explanation = Vec::new();
    for f in sig.iter().chain(timing.iter()).chain(phase_code.iter()) {
        if f.z.abs() > 3.0 {
            explanation.push(format!(
                "{}: measured − predicted {:+.3} ({:+.1}σ)",
                f.name, f.residual, f.z
            ));
        }
    }
    match class {
        Class::Relay => explanation.push(format!(
            "propagation signature inconsistent with the {:.0} km path (S = {:.1} > {:.1}, {} features){}",
            exp.distance_km,
            stat,
            if sig_flag { thr } else { thr_weak },
            k,
            if late && timing_anom { "; late arrival" } else { "" }
        )),
        Class::TimingOnly => explanation.push(
            "timing anomaly with a consistent propagation signature (transmitter timing fault, \
             or a relay matching field and ECD)"
                .into(),
        ),
        Class::Consistent => {}
    }
    let bearing_bad = sig
        .iter()
        .any(|f| f.name.starts_with("bearing") && f.z.abs() > 3.0);
    StationVerdict {
        station: obs.station.clone(),
        class,
        signature_stat: stat,
        signature_dof: k,
        signature_threshold: thr,
        timing_threshold: t_thr,
        excess_delay_ns: timing.as_ref().map(|t| t.residual),
        relay_bearing_deg: if bearing_bad { obs.bearing_deg } else { None },
        signature: sig,
        timing,
        phase_code,
        relay_llr,
        explanation,
    }
}

/// Test every station. With `clock = None` the delay feature of each station uses the
/// leave-one-out clock of the other stations (cross-station TOA geometry; ≥ 2 others with
/// a delay), weighted by their delay σ; with fewer, that station has no delay feature.
/// Observations without a matching prediction are skipped.
pub fn screen(
    obs: &[Observed],
    exp: &[Expected],
    clock: Option<ClockRef>,
    cfg: &MeaconConfig,
) -> Vec<StationVerdict> {
    let pairs: Vec<(&Observed, &Expected)> = obs
        .iter()
        .filter_map(|o| exp.iter().find(|e| e.station == o.station).map(|e| (o, e)))
        .collect();
    // per-station clock residual and weight
    let resid: Vec<Option<(f64, f64)>> = pairs
        .iter()
        .map(|(o, e)| {
            o.pseudo_delay_ns.map(|pd| {
                let v = e.sigma.delay_ns.powi(2) + o.sigma_delay_ns.powi(2);
                (pd - e.delay_ns, 1.0 / v)
            })
        })
        .collect();
    let loo = |i: usize, skip: Option<usize>| -> Option<ClockRef> {
        let others: Vec<(f64, f64)> = resid
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i && Some(*j) != skip)
            .filter_map(|(_, r)| *r)
            .collect();
        (others.len() >= 2).then(|| {
            let w: f64 = others.iter().map(|x| x.1).sum();
            ClockRef {
                bias_ns: others.iter().map(|x| x.0 * x.1).sum::<f64>() / w,
                sigma_ns: (1.0 / w).sqrt(),
            }
        })
    };
    // Exclusion: the station whose leave-one-out timing is worst, when anomalous, is kept
    // out of the other stations' clocks (so one faulty or relayed station does not make
    // the others look early or late).
    let mut skip = None;
    if clock.is_none() {
        let t_thr = chi2_quantile(1.0 - cfg.p_fa_timing, 1);
        let worst = pairs
            .iter()
            .enumerate()
            .filter_map(|(i, (o, e))| {
                test_station(o, e, loo(i, None), cfg)
                    .timing
                    .map(|t| (i, t.z * t.z))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, z2)) = worst {
            if z2 > t_thr {
                skip = Some(i);
            }
        }
    }
    pairs
        .iter()
        .enumerate()
        .map(|(i, (o, e))| {
            let c = clock.or_else(|| loo(i, skip).or_else(|| loo(i, None)));
            test_station(o, e, c, cfg)
        })
        .collect()
}

/// Predictions for every station of `chains` at the position of an eLoran (or other)
/// solution, convenience for a moving or unsurveyed receiver.
pub fn expected_at_solution(
    model: &dyn PropagationModel,
    chains: &[Chain],
    sol: &PositionSolution,
    t_s: f64,
    sigma: &ModelSigmas,
) -> Result<Vec<Expected>, String> {
    let rx = RxState {
        t_s,
        pos: sol.position,
        heading_rad: 0.0,
        speed_mps: 0.0,
    };
    let mut out = Vec::new();
    for c in chains {
        for st in &c.stations {
            out.push(Expected::from_model(
                model,
                &c.name,
                st,
                &rx,
                sigma.clone(),
            )?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
