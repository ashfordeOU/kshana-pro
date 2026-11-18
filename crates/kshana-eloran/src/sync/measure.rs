// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Automatic eLoran-vs-GNSS-simulator synchronisation measurement.
//!
//! A time-interval counter (TIC) measures between the eLoran reference point (e.g. the
//! standard zero crossing marker or the eLoran device's 1PPS-aligned output) and the GNSS
//! simulator's 1PPS, once per `tau0_s`. From the readings this module computes, in terms
//! of `y = t_eLoran − t_GNSS` at the reference planes:
//!
//! * mean, standard deviation, min/max, a least-squares drift fit;
//! * TDEV(τ) (NIST SP 1065 eq. 17, via the second difference of phase averaged over n
//!   samples) and MTIE(τ) (largest peak-to-peak phase in any window of span τ) on octave
//!   τ = n·τ0;
//! * alarms when `|mean − target|` or TDEV exceeds its limit (default 10 ns each);
//! * the correction to store in [`CalibrationFile::gnss_offset`] and a JSON report.
//!
//! Sign convention: with [`TicPolarity::StartEloranStopGnss`] a reading is
//! `R = (t_GNSS + d_gnss_cable) − (t_eLoran + d_eloran_cable)`, so
//! `y = d_gnss_cable − d_eloran_cable − R`; the other polarity flips `R`.
//!
//! The stored correction is `applied_correction_ns + mean(y) − target_ns`, where
//! `applied_correction_ns` is the `eloran_minus_gnss_ns` that was already compensated on
//! the eLoran side while measuring: compensating the new value moves `y` to the target.
//!
//! [`SyncMonitor`] ingests readings one by one (sliding window) and raises/clears alarms.

use crate::hw::calib::{parse_tic_csv, CalibrationFile, GnssOffsetCal, TicUnit};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// TIC channel assignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicPolarity {
    /// Start = eLoran reference point, stop = GNSS 1PPS.
    #[default]
    StartEloranStopGnss,
    /// Start = GNSS 1PPS, stop = eLoran reference point.
    StartGnssStopEloran,
}

/// Measurement configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MeasureConfig {
    /// Reading interval (s).
    pub tau0_s: f64,
    pub polarity: TicPolarity,
    /// Wanted `t_eLoran − t_GNSS` (ns).
    pub target_ns: f64,
    /// Alarm limit on `|mean − target|` (ns).
    pub offset_limit_ns: f64,
    /// Alarm limit on TDEV (ns).
    pub tdev_limit_ns: f64,
    /// Check TDEV only up to this τ (s); `None` = all computed τ.
    pub tdev_max_tau_s: Option<f64>,
    /// Delay from the eLoran reference plane to its TIC input (ns).
    pub eloran_cable_ns: f64,
    /// Delay from the GNSS 1PPS reference plane to its TIC input (ns).
    pub gnss_cable_ns: f64,
    /// Standard uncertainty of the two cable delays combined (ns).
    pub cable_u_ns: f64,
    /// TIC type-B standard uncertainty (resolution, trigger level, offset) (ns).
    pub tic_u_ns: f64,
    /// `eloran_minus_gnss_ns` compensated during the measurement (ns).
    pub applied_correction_ns: f64,
}

impl Default for MeasureConfig {
    fn default() -> Self {
        MeasureConfig {
            tau0_s: 1.0,
            polarity: TicPolarity::StartEloranStopGnss,
            target_ns: 0.0,
            offset_limit_ns: 10.0,
            tdev_limit_ns: 10.0,
            tdev_max_tau_s: None,
            eloran_cable_ns: 0.0,
            gnss_cable_ns: 0.0,
            cable_u_ns: 0.0,
            tic_u_ns: 0.0,
            applied_correction_ns: 0.0,
        }
    }
}

impl MeasureConfig {
    /// `y = t_eLoran − t_GNSS` (ns) from a raw reading (ns).
    pub fn eloran_minus_gnss(&self, reading_ns: f64) -> f64 {
        let r = match self.polarity {
            TicPolarity::StartEloranStopGnss => reading_ns,
            TicPolarity::StartGnssStopEloran => -reading_ns,
        };
        self.gnss_cable_ns - self.eloran_cable_ns - r
    }
}

/// Raw TIC readings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TicSeries {
    pub readings_ns: Vec<f64>,
}

impl TicSeries {
    pub fn from_ns(v: &[f64]) -> TicSeries {
        TicSeries {
            readings_ns: v.to_vec(),
        }
    }
    pub fn from_seconds(v: &[f64]) -> TicSeries {
        TicSeries {
            readings_ns: v.iter().map(|x| x * 1e9).collect(),
        }
    }
    /// From TIC CSV (see [`parse_tic_csv`]).
    pub fn from_csv(text: &str, column: Option<usize>, unit: TicUnit) -> Result<TicSeries, String> {
        Ok(TicSeries {
            readings_ns: parse_tic_csv(text, column, unit)?,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------------------

/// One point of a stability curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StabilityPoint {
    pub tau_s: f64,
    /// Averaging factor n (τ = n·τ0).
    pub n: usize,
    pub value_ns: f64,
    /// Number of terms in the estimate.
    pub terms: usize,
}

/// TDEV at τ = n·τ0 of phase `x` (any unit; result in the same unit).
/// `TVAR(nτ0) = 1/(6 n² M) Σ_{j=0}^{M−1} [Σ_{i=j}^{j+n−1} (x_{i+2n} − 2x_{i+n} + x_i)]²`,
/// `M = N − 3n + 1`. `None` when `3n > N` or `n = 0`.
pub fn tdev(x: &[f64], n: usize) -> Option<f64> {
    let len = x.len();
    if n == 0 || 3 * n > len {
        return None;
    }
    let m = len - 3 * n + 1;
    // second differences d_i, i = 0..len−2n, and their prefix sums
    let mut pre = Vec::with_capacity(len - 2 * n + 1);
    pre.push(0.0);
    let mut acc = 0.0;
    for i in 0..len - 2 * n {
        acc += x[i + 2 * n] - 2.0 * x[i + n] + x[i];
        pre.push(acc);
    }
    let s: f64 = (0..m)
        .map(|j| {
            let v = pre[j + n] - pre[j];
            v * v
        })
        .sum();
    Some((s / (6.0 * (n * n) as f64 * m as f64)).sqrt())
}

/// MTIE at τ = n·τ0: max over windows of n+1 consecutive samples of (max − min).
pub fn mtie(x: &[f64], n: usize) -> Option<f64> {
    let w = n + 1;
    if n == 0 || w > x.len() {
        return None;
    }
    let mut mx: VecDeque<usize> = VecDeque::new();
    let mut mn: VecDeque<usize> = VecDeque::new();
    let mut best = 0.0f64;
    for i in 0..x.len() {
        while mx.back().is_some_and(|&j| x[j] <= x[i]) {
            mx.pop_back();
        }
        mx.push_back(i);
        while mn.back().is_some_and(|&j| x[j] >= x[i]) {
            mn.pop_back();
        }
        mn.push_back(i);
        if i + 1 >= w {
            let lo = i + 1 - w;
            while mx.front().is_some_and(|&j| j < lo) {
                mx.pop_front();
            }
            while mn.front().is_some_and(|&j| j < lo) {
                mn.pop_front();
            }
            best = best.max(x[*mx.front().unwrap()] - x[*mn.front().unwrap()]);
        }
    }
    Some(best)
}

/// Octave averaging factors 1, 2, 4, … up to `max_n`.
pub fn octave_ns(max_n: usize) -> Vec<usize> {
    let mut v = Vec::new();
    let mut n = 1;
    while n <= max_n {
        v.push(n);
        n *= 2;
    }
    v
}

/// TDEV on octave τ.
pub fn tdev_curve(x: &[f64], tau0_s: f64) -> Vec<StabilityPoint> {
    octave_ns(x.len() / 3)
        .into_iter()
        .filter_map(|n| {
            tdev(x, n).map(|v| StabilityPoint {
                tau_s: n as f64 * tau0_s,
                n,
                value_ns: v,
                terms: x.len() - 3 * n + 1,
            })
        })
        .collect()
}

/// MTIE on octave τ.
pub fn mtie_curve(x: &[f64], tau0_s: f64) -> Vec<StabilityPoint> {
    octave_ns(x.len().saturating_sub(1))
        .into_iter()
        .filter_map(|n| {
            mtie(x, n).map(|v| StabilityPoint {
                tau_s: n as f64 * tau0_s,
                n,
                value_ns: v,
                terms: x.len() - n,
            })
        })
        .collect()
}

/// Least-squares line `y = intercept + slope·t`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DriftFit {
    /// Value at t = 0 (first reading) (ns).
    pub intercept_ns: f64,
    pub slope_ns_per_s: f64,
    pub slope_ns_per_day: f64,
    /// Standard uncertainty of the slope (ns/s).
    pub slope_u_ns_per_s: f64,
    /// Standard deviation of the residuals (ns).
    pub residual_std_ns: f64,
}

/// Fit `y_k` at `t_k = k·tau0`.
pub fn drift_fit(y: &[f64], tau0_s: f64) -> Option<DriftFit> {
    let n = y.len();
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let tm = (nf - 1.0) / 2.0 * tau0_s;
    let ym = y.iter().sum::<f64>() / nf;
    let (mut stt, mut sty) = (0.0, 0.0);
    for (k, v) in y.iter().enumerate() {
        let dt = k as f64 * tau0_s - tm;
        stt += dt * dt;
        sty += dt * (v - ym);
    }
    let b = sty / stt;
    let a = ym - b * tm;
    let ss: f64 = y
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let r = v - (a + b * k as f64 * tau0_s);
            r * r
        })
        .sum();
    let rs = if n > 2 { (ss / (nf - 2.0)).sqrt() } else { 0.0 };
    Some(DriftFit {
        intercept_ns: a,
        slope_ns_per_s: b,
        slope_ns_per_day: b * 86_400.0,
        slope_u_ns_per_s: rs / stt.sqrt(),
        residual_std_ns: rs,
    })
}

fn mean_std(y: &[f64]) -> (f64, f64) {
    let n = y.len() as f64;
    let m = y.iter().sum::<f64>() / n;
    let v = if y.len() > 1 {
        y.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (n - 1.0)
    } else {
        0.0
    };
    (m, v.sqrt())
}

fn lag1(y: &[f64], m: f64) -> f64 {
    let den: f64 = y.iter().map(|v| (v - m) * (v - m)).sum();
    if den <= 0.0 || y.len() < 3 {
        return 0.0;
    }
    let num: f64 = y.windows(2).map(|w| (w[0] - m) * (w[1] - m)).sum();
    (num / den).clamp(-0.99, 0.99)
}

// ---------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlarmKind {
    Offset,
    Tdev,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alarm {
    pub kind: AlarmKind,
    pub value_ns: f64,
    pub limit_ns: f64,
    /// τ of a TDEV alarm (s).
    pub tau_s: Option<f64>,
    pub message: String,
}

/// The correction for the calibration file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnssCorrection {
    /// Value for [`GnssOffsetCal::eloran_minus_gnss_ns`].
    pub eloran_minus_gnss_ns: f64,
    /// Standard uncertainty (ns): type A (σ/√n_eff), TIC and cable type B, RSS.
    pub u_ns: f64,
    pub u_a_ns: f64,
    pub n_effective: f64,
}

/// The measurement report (JSON).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncMeasurementReport {
    pub n: usize,
    pub tau0_s: f64,
    pub duration_s: f64,
    /// Statistics of `y = t_eLoran − t_GNSS` (ns).
    pub mean_ns: f64,
    pub std_ns: f64,
    pub min_ns: f64,
    pub max_ns: f64,
    pub lag1_autocorrelation: f64,
    pub drift: Option<DriftFit>,
    pub tdev: Vec<StabilityPoint>,
    pub mtie: Vec<StabilityPoint>,
    pub target_ns: f64,
    /// `mean − target` (ns).
    pub offset_error_ns: f64,
    pub offset_limit_ns: f64,
    pub tdev_limit_ns: f64,
    pub alarms: Vec<Alarm>,
    pub pass: bool,
    pub correction: GnssCorrection,
}

impl SyncMeasurementReport {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
    /// Write the correction into `cal.gnss_offset` (keeping a recorded GNSS simulator delay).
    pub fn apply_to_calibration(&self, cal: &mut CalibrationFile, gnss_simulator: &str) {
        let (d, du) = cal
            .gnss_offset
            .as_ref()
            .map(|g| (g.gnss_sim_delay_ns, g.gnss_sim_delay_u_ns))
            .unwrap_or((None, None));
        cal.gnss_offset = Some(GnssOffsetCal {
            eloran_minus_gnss_ns: self.correction.eloran_minus_gnss_ns,
            u_ns: self.correction.u_ns,
            gnss_simulator: gnss_simulator.into(),
            gnss_sim_delay_ns: d,
            gnss_sim_delay_u_ns: du,
        });
    }
}

/// Analyse `y` values (already `t_eLoran − t_GNSS`, ns).
pub fn analyse_offsets(y: &[f64], cfg: &MeasureConfig) -> Result<SyncMeasurementReport, String> {
    if y.len() < 2 {
        return Err("need at least two readings".into());
    }
    if y.iter().any(|v| !v.is_finite()) {
        return Err("readings must be finite".into());
    }
    if !(cfg.tau0_s > 0.0) {
        return Err("tau0_s must be > 0".into());
    }
    let (mean, std) = mean_std(y);
    let r1 = lag1(y, mean);
    let n = y.len() as f64;
    let n_eff = (n * (1.0 - r1) / (1.0 + r1)).clamp(1.0, n);
    let u_a = std / n_eff.sqrt();
    let td = tdev_curve(y, cfg.tau0_s);
    let mt = mtie_curve(y, cfg.tau0_s);
    let err = mean - cfg.target_ns;
    let mut alarms = Vec::new();
    if err.abs() > cfg.offset_limit_ns {
        alarms.push(Alarm {
            kind: AlarmKind::Offset,
            value_ns: err,
            limit_ns: cfg.offset_limit_ns,
            tau_s: None,
            message: format!(
                "|offset − target| = {:.3} ns exceeds {} ns",
                err.abs(),
                cfg.offset_limit_ns
            ),
        });
    }
    if let Some(p) = td
        .iter()
        .filter(|p| cfg.tdev_max_tau_s.is_none_or(|m| p.tau_s <= m + 1e-9))
        .filter(|p| p.value_ns > cfg.tdev_limit_ns)
        .max_by(|a, b| a.value_ns.total_cmp(&b.value_ns))
    {
        alarms.push(Alarm {
            kind: AlarmKind::Tdev,
            value_ns: p.value_ns,
            limit_ns: cfg.tdev_limit_ns,
            tau_s: Some(p.tau_s),
            message: format!(
                "TDEV({} s) = {:.3} ns exceeds {} ns",
                p.tau_s, p.value_ns, cfg.tdev_limit_ns
            ),
        });
    }
    Ok(SyncMeasurementReport {
        n: y.len(),
        tau0_s: cfg.tau0_s,
        duration_s: (y.len() - 1) as f64 * cfg.tau0_s,
        mean_ns: mean,
        std_ns: std,
        min_ns: y.iter().copied().fold(f64::INFINITY, f64::min),
        max_ns: y.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        lag1_autocorrelation: r1,
        drift: drift_fit(y, cfg.tau0_s),
        tdev: td,
        mtie: mt,
        target_ns: cfg.target_ns,
        offset_error_ns: err,
        offset_limit_ns: cfg.offset_limit_ns,
        tdev_limit_ns: cfg.tdev_limit_ns,
        pass: alarms.is_empty(),
        alarms,
        correction: GnssCorrection {
            eloran_minus_gnss_ns: cfg.applied_correction_ns + err,
            u_ns: (u_a * u_a + cfg.tic_u_ns * cfg.tic_u_ns + cfg.cable_u_ns * cfg.cable_u_ns)
                .sqrt(),
            u_a_ns: u_a,
            n_effective: n_eff,
        },
    })
}

/// Analyse raw TIC readings.
pub fn analyse(series: &TicSeries, cfg: &MeasureConfig) -> Result<SyncMeasurementReport, String> {
    let y: Vec<f64> = series
        .readings_ns
        .iter()
        .map(|r| cfg.eloran_minus_gnss(*r))
        .collect();
    analyse_offsets(&y, cfg)
}

// ---------------------------------------------------------------------------------------
// Continuous monitor
// ---------------------------------------------------------------------------------------

/// An alarm raised or cleared by the monitor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmEvent {
    /// Index of the reading (0-based) and its time (s since the first reading).
    pub index: u64,
    pub t_s: f64,
    pub kind: AlarmKind,
    pub raised: bool,
    pub value_ns: f64,
}

/// Incremental monitor over a sliding window of readings.
#[derive(Clone, Debug)]
pub struct SyncMonitor {
    pub cfg: MeasureConfig,
    /// Window length (readings).
    pub window: usize,
    /// Offset alarms are evaluated once the window holds this many readings.
    pub min_samples: usize,
    /// TDEV is re-evaluated every this many readings.
    pub stability_every: usize,
    buf: VecDeque<f64>,
    count: u64,
    active: [bool; 2],
    pub events: Vec<AlarmEvent>,
}

impl SyncMonitor {
    pub fn new(cfg: MeasureConfig, window: usize) -> SyncMonitor {
        SyncMonitor {
            cfg,
            window: window.max(3),
            min_samples: 10,
            stability_every: 10,
            buf: VecDeque::new(),
            count: 0,
            active: [false; 2],
            events: vec![],
        }
    }
    pub fn count(&self) -> u64 {
        self.count
    }
    pub fn alarm_active(&self, k: AlarmKind) -> bool {
        self.active[k as usize]
    }
    fn set(&mut self, k: AlarmKind, on: bool, value: f64, out: &mut Vec<AlarmEvent>) {
        if self.active[k as usize] != on {
            self.active[k as usize] = on;
            let e = AlarmEvent {
                index: self.count - 1,
                t_s: (self.count - 1) as f64 * self.cfg.tau0_s,
                kind: k,
                raised: on,
                value_ns: value,
            };
            self.events.push(e.clone());
            out.push(e);
        }
    }
    /// Ingest one raw TIC reading (ns); returns the alarm transitions it caused.
    pub fn push_reading_ns(&mut self, reading_ns: f64) -> Vec<AlarmEvent> {
        let y = self.cfg.eloran_minus_gnss(reading_ns);
        self.push_offset_ns(y)
    }
    /// Ingest one `t_eLoran − t_GNSS` value (ns).
    pub fn push_offset_ns(&mut self, y: f64) -> Vec<AlarmEvent> {
        let mut out = Vec::new();
        if !y.is_finite() {
            return out;
        }
        self.buf.push_back(y);
        if self.buf.len() > self.window {
            self.buf.pop_front();
        }
        self.count += 1;
        if self.buf.len() >= self.min_samples {
            let m = self.buf.iter().sum::<f64>() / self.buf.len() as f64;
            let e = m - self.cfg.target_ns;
            self.set(AlarmKind::Offset, e.abs() > self.cfg.offset_limit_ns, e, &mut out);
        }
        if self.count % self.stability_every.max(1) as u64 == 0 && self.buf.len() >= 3 {
            let x: Vec<f64> = self.buf.iter().copied().collect();
            let worst = tdev_curve(&x, self.cfg.tau0_s)
                .into_iter()
                .filter(|p| self.cfg.tdev_max_tau_s.is_none_or(|m| p.tau_s <= m + 1e-9))
                .map(|p| p.value_ns)
                .fold(0.0, f64::max);
            self.set(AlarmKind::Tdev, worst > self.cfg.tdev_limit_ns, worst, &mut out);
        }
        out
    }
    /// Full report over the current window.
    pub fn snapshot(&self) -> Result<SyncMeasurementReport, String> {
        let x: Vec<f64> = self.buf.iter().copied().collect();
        analyse_offsets(&x, &self.cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timebase::gauss;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn white(n: usize, sigma: f64, seed: u64) -> Vec<f64> {
        let mut r = ChaCha8Rng::seed_from_u64(seed);
        (0..n).map(|_| sigma * gauss(&mut r)).collect()
    }

    #[test]
    fn tdev_of_white_pm_matches_analytic() {
        // white PM: TVAR(n) = σ²/n exactly in expectation
        let sigma = 2.0;
        let x = white(30_000, sigma, 11);
        for (n, tol) in [(1usize, 0.03), (4, 0.06), (16, 0.12), (64, 0.25)] {
            let t = tdev(&x, n).unwrap();
            let a = sigma / (n as f64).sqrt();
            assert!((t / a - 1.0).abs() < tol, "n = {n}: {t} vs {a}");
        }
        assert!(tdev(&x[..5], 2).is_none());
    }

    #[test]
    fn tdev_ignores_drift_and_mtie_sees_it() {
        let x = white(3000, 1.0, 3);
        let d: Vec<f64> = x
            .iter()
            .enumerate()
            .map(|(k, v)| v + 5.0 + 0.02 * k as f64)
            .collect();
        for n in [1, 8, 64] {
            assert!((tdev(&x, n).unwrap() - tdev(&d, n).unwrap()).abs() < 1e-9);
        }
        let ramp: Vec<f64> = (0..100).map(|k| 0.5 * k as f64).collect();
        assert!((mtie(&ramp, 10).unwrap() - 5.0).abs() < 1e-12);
        assert_eq!(mtie(&[0.0, 3.0, -1.0, 2.0], 1).unwrap(), 4.0);
        assert_eq!(mtie(&[0.0, 3.0, -1.0, 2.0], 3).unwrap(), 4.0);
        let f = drift_fit(&d, 1.0).unwrap();
        assert!((f.slope_ns_per_s - 0.02).abs() < 3.0 * f.slope_u_ns_per_s + 1e-4, "{f:?}");
        assert!((f.intercept_ns - 5.0).abs() < 0.15);
        assert!((f.residual_std_ns - 1.0).abs() < 0.05);
    }

    #[test]
    fn analysis_alarms_and_calibration_correction() {
        // TIC start eLoran, stop GNSS; GNSS cable 12 ns, eLoran cable 4 ns; true y = 3 ns
        let cfg = MeasureConfig {
            gnss_cable_ns: 12.0,
            eloran_cable_ns: 4.0,
            tic_u_ns: 0.3,
            cable_u_ns: 0.4,
            applied_correction_ns: 1.5,
            ..Default::default()
        };
        let noise = white(1000, 0.5, 5);
        let readings: Vec<f64> = noise.iter().map(|w| (12.0 - 4.0 - 3.0) + w).collect();
        let secs: Vec<f64> = readings.iter().map(|r| r * 1e-9).collect();
        let r = analyse(&TicSeries::from_seconds(&secs), &cfg).unwrap();
        assert!((r.mean_ns - 3.0).abs() < 0.1, "{}", r.mean_ns);
        assert!(r.pass, "{:?}", r.alarms);
        assert!((r.correction.eloran_minus_gnss_ns - (1.5 + r.mean_ns)).abs() < 1e-12);
        let ua = r.correction.u_a_ns;
        assert!((r.correction.u_ns - (ua * ua + 0.09 + 0.16).sqrt()).abs() < 1e-12);
        assert!(r.tdev[0].tau_s == 1.0 && (r.tdev[0].value_ns - 0.5).abs() < 0.05);
        // opposite polarity gives the same y
        let cfg2 = MeasureConfig {
            polarity: TicPolarity::StartGnssStopEloran,
            ..cfg.clone()
        };
        let neg: Vec<f64> = readings.iter().map(|r| -r).collect();
        let r2 = analyse(&TicSeries::from_ns(&neg), &cfg2).unwrap();
        assert!((r2.mean_ns - r.mean_ns).abs() < 1e-9);
        // offset beyond 10 ns and noisy series alarm
        let big: Vec<f64> = white(500, 25.0, 9).iter().map(|w| w - 15.0).collect();
        let r3 = analyse_offsets(&big, &MeasureConfig::default()).unwrap();
        assert!(!r3.pass);
        let kinds: Vec<AlarmKind> = r3.alarms.iter().map(|a| a.kind).collect();
        assert_eq!(kinds, vec![AlarmKind::Offset, AlarmKind::Tdev]);
        // JSON report and calibration update
        let j = r.to_json();
        let back: SyncMeasurementReport = serde_json::from_str(&j).unwrap();
        assert_eq!(back.n, 1000);
        let mut cal: CalibrationFile = serde_json::from_value(serde_json::json!({
            "version": 1, "date": "2026-11-18", "device": "dev", "sample_rate_hz": 1e6,
            "channels": [],
            "gnss_offset": {"eloran_minus_gnss_ns": 1.5, "u_ns": 1.0, "gnss_simulator": "old",
                            "gnss_sim_delay_ns": 50.0}
        }))
        .unwrap();
        r.apply_to_calibration(&mut cal, "GNSS-SIM-1");
        let g = cal.gnss_offset.as_ref().unwrap();
        assert_eq!(g.eloran_minus_gnss_ns, r.correction.eloran_minus_gnss_ns);
        assert_eq!(g.gnss_simulator, "GNSS-SIM-1");
        assert_eq!(g.gnss_sim_delay_ns, Some(50.0));
        assert!(CalibrationFile::from_json(&cal.to_json()).is_ok());
    }

    #[test]
    fn csv_series() {
        let s = TicSeries::from_csv("time,tic\n0,1.0e-9\n1,2.0e-9\n2,3.0e-9\n", None, TicUnit::Seconds)
            .unwrap();
        assert_eq!(s.readings_ns.len(), 3);
        assert!((s.readings_ns[2] - 3.0).abs() < 1e-9);
    }

    #[test]
    fn monitor_raises_and_clears_alarms() {
        let mut m = SyncMonitor::new(MeasureConfig::default(), 50);
        let noise = white(400, 0.5, 21);
        let mut ev = Vec::new();
        for (k, w) in noise.iter().enumerate() {
            // offset steps to 20 ns between readings 100 and 199
            let y = if (100..200).contains(&k) { 20.0 } else { 1.0 } + w;
            ev.extend(m.push_offset_ns(y));
        }
        let off: Vec<&AlarmEvent> = ev.iter().filter(|e| e.kind == AlarmKind::Offset).collect();
        assert_eq!(off.len(), 2, "{ev:?}");
        assert!(off[0].raised && !off[1].raised);
        // a 50-reading window mean passes 10 ns after ~25 readings into the step
        assert!((120..135).contains(&off[0].index), "{:?}", off[0]);
        assert!((220..235).contains(&off[1].index), "{:?}", off[1]);
        assert!(!m.alarm_active(AlarmKind::Offset));
        assert_eq!(m.count(), 400);
        let s = m.snapshot().unwrap();
        assert_eq!(s.n, 50);
        assert!(s.pass);
    }
}
