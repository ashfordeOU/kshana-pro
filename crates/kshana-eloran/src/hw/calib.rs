// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Output calibration: calibration files, the calibrated sink and the calibration
//! procedures.
//!
//! # Reference point
//!
//! All timing is referred to the **eLoran pulse standard zero crossing** (the positive-going
//! carrier zero crossing 30 µs after the pulse's time origin, COMDTINST M16562.4A) **relative
//! to the 1PPS** of the common time reference. A channel is on time when a pulse whose time
//! origin is nominally at a PPS edge shows its standard zero crossing at the output
//! connector exactly 30 µs after that PPS edge (as seen at the reference plane named in
//! [`ReferencePoint::plane`]).
//!
//! # Conventions
//!
//! * `delay_ns` of a channel is the time from the sample stream's nominal instant to the
//!   same feature at the connector (device latency + DAC + filters + cable to the reference
//!   plane). It is **positive when the connector is late**.
//! * Applying a calibration shifts each channel by **−delay** (it is advanced), so the
//!   connector output is on time ([`CalibratedSink`]).
//! * `full_scale_dbm` is the power into the nominal load of a sine of amplitude 1.0 (full
//!   scale) at the eLoran carrier. Scaling to a wanted level is `gain_dB = target − full_scale`.
//! * Uncertainties are standard uncertainties (k = 1) unless named "expanded".

use super::fir::{fractional_delay_taps, FirState};
use crate::scene::SampleSink;
use crate::signal::PulseSignal;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Current calibration file format version.
pub const CALIBRATION_VERSION: u32 = 1;

/// The timing reference point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferencePoint {
    /// Feature measured: the pulse standard zero crossing.
    pub feature: String,
    /// Its time after the pulse time origin (µs): 30.
    pub zero_crossing_us: f64,
    /// What it is referred to, e.g. "rising edge of the house 1PPS at the TIC input".
    pub relative_to: String,
    /// Physical plane, e.g. "SMA output of the antenna simulator".
    pub plane: String,
}

impl Default for ReferencePoint {
    fn default() -> Self {
        ReferencePoint {
            feature: "eLoran pulse standard zero crossing (positive-going)".into(),
            zero_crossing_us: 30.0,
            relative_to: "1PPS rising edge (50 % point) of the common time reference".into(),
            plane: "channel output connector".into(),
        }
    }
}

/// An instrument used in a calibration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instrument {
    /// Role, e.g. "time interval counter", "oscilloscope", "SDR", "power meter".
    pub role: String,
    pub model: String,
    pub serial: String,
    #[serde(default)]
    pub cal_due: Option<String>,
}

/// Calibration of one output channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelCal {
    pub channel: usize,
    /// Output delay (ns), positive = connector late.
    pub delay_ns: f64,
    /// Standard uncertainty of the delay (ns).
    pub delay_u_ns: f64,
    /// Output power at full scale (dBm into the nominal load).
    pub full_scale_dbm: f64,
    #[serde(default)]
    pub full_scale_u_db: f64,
    #[serde(default)]
    pub load_ohm: Option<f64>,
    #[serde(default)]
    pub notes: String,
}

/// Measured offset between the eLoran output and a GNSS simulator's output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnssOffsetCal {
    /// `t_eLoran − t_GNSS` at the reference planes (ns): positive when eLoran is late.
    pub eloran_minus_gnss_ns: f64,
    pub u_ns: f64,
    /// GNSS simulator identification.
    pub gnss_simulator: String,
    /// The GNSS simulator's own output delay as calibrated (ns), for the record.
    #[serde(default)]
    pub gnss_sim_delay_ns: Option<f64>,
    #[serde(default)]
    pub gnss_sim_delay_u_ns: Option<f64>,
}

/// A versioned calibration file (JSON).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalibrationFile {
    pub version: u32,
    /// Date of the calibration (ISO 8601).
    pub date: String,
    /// Device the calibration belongs to (e.g. SoapySDR args or serial).
    pub device: String,
    /// Sample rate the delays were measured at (Hz).
    pub sample_rate_hz: f64,
    #[serde(default)]
    pub reference_point: ReferencePoint,
    #[serde(default)]
    pub instruments: Vec<Instrument>,
    pub channels: Vec<ChannelCal>,
    #[serde(default)]
    pub gnss_offset: Option<GnssOffsetCal>,
    #[serde(default)]
    pub notes: String,
}

impl CalibrationFile {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != CALIBRATION_VERSION {
            return Err(format!(
                "calibration file version {} not supported (expected {CALIBRATION_VERSION})",
                self.version
            ));
        }
        if (self.reference_point.zero_crossing_us - 30.0).abs() > 1e-9 {
            return Err("reference point must be the standard zero crossing at 30 µs".into());
        }
        for (i, c) in self.channels.iter().enumerate() {
            if c.channel != i {
                return Err("channels must be listed in order 0, 1, 2, …".into());
            }
            if !c.delay_ns.is_finite()
                || c.delay_u_ns.is_nan()
                || c.delay_u_ns < 0.0
                || !c.full_scale_dbm.is_finite()
            {
                return Err(format!("channel {i}: invalid values"));
            }
        }
        Ok(())
    }
    pub fn from_json(s: &str) -> Result<CalibrationFile, String> {
        let c: CalibrationFile =
            serde_json::from_str(s).map_err(|e| format!("calibration: {e}"))?;
        c.validate()?;
        Ok(c)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
    pub fn load(path: &Path) -> Result<CalibrationFile, String> {
        CalibrationFile::from_json(
            &std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        std::fs::write(path, self.to_json()).map_err(|e| format!("{}: {e}", path.display()))
    }
    /// Per-channel gain (dB) that makes full scale come out at `target_full_scale_dbm`.
    pub fn gains_db(&self, target_full_scale_dbm: Option<f64>) -> Vec<f64> {
        self.channels
            .iter()
            .map(|c| {
                target_full_scale_dbm
                    .map(|t| t - c.full_scale_dbm)
                    .unwrap_or(0.0)
            })
            .collect()
    }
}

/// FIR half length of the calibration fractional-delay filter (taps = 2·half + 1).
pub const DELAY_FIR_HALF: usize = 32;
/// Kaiser β of the calibration fractional-delay filter.
pub const DELAY_FIR_BETA: f64 = 12.0;

/// A [`SampleSink`] wrapper applying per-channel time shifts of **−delay** and gains.
///
/// A negative shift (an advance) is not causal, so the sink delays every channel by a
/// common whole number of samples `L` ([`CalibratedSink::lead_samples`]) minus its own
/// delay with a windowed-sinc fractional-delay FIR, then drops the first `L` output samples
/// and pads `L` samples at the end: the output has the same length as the input and each
/// channel is shifted by exactly −delay. **The first `L` input samples (a few tens of µs)
/// are consumed as look-ahead**, so start scenes at least that long before the first
/// event that matters.
pub struct CalibratedSink<S: SampleSink> {
    inner: S,
    firs: Vec<FirState>,
    gains: Vec<f64>,
    lead: usize,
    to_drop: usize,
    /// Samples beyond ±1.0 after the gain.
    pub clipped: u64,
}

impl<S: SampleSink> CalibratedSink<S> {
    /// `delays_ns[c]` and `gains_db[c]` per channel (missing gains = 0 dB).
    pub fn new(
        inner: S,
        fs_hz: f64,
        delays_ns: &[f64],
        gains_db: &[f64],
    ) -> Result<CalibratedSink<S>, String> {
        if delays_ns.is_empty() {
            return Err("at least one channel".into());
        }
        let s: Vec<f64> = delays_ns.iter().map(|d| d * 1e-9 * fs_hz).collect();
        if s.iter().any(|v| !v.is_finite()) {
            return Err("invalid delay".into());
        }
        let max_s = s.iter().cloned().fold(0.0f64, f64::max);
        let lead = max_s.ceil() as usize + DELAY_FIR_HALF;
        let firs = s
            .iter()
            .map(|sc| {
                let d = lead as f64 - sc; // ≥ half
                let i = d.floor();
                let frac = d - i;
                FirState::new(
                    fractional_delay_taps(frac, DELAY_FIR_HALF, DELAY_FIR_BETA),
                    i as usize - DELAY_FIR_HALF,
                )
            })
            .collect();
        let gains = (0..delays_ns.len())
            .map(|c| 10f64.powf(gains_db.get(c).copied().unwrap_or(0.0) / 20.0))
            .collect();
        Ok(CalibratedSink {
            inner,
            firs,
            gains,
            lead,
            to_drop: lead,
            clipped: 0,
        })
    }

    /// From a calibration file; `target_full_scale_dbm` sets the gains (None = unity).
    /// `extra_delay_ns` adds to every channel's delay (e.g. a measured eLoran-vs-GNSS offset).
    pub fn from_calibration(
        inner: S,
        fs_hz: f64,
        cal: &CalibrationFile,
        target_full_scale_dbm: Option<f64>,
        extra_delay_ns: f64,
    ) -> Result<CalibratedSink<S>, String> {
        cal.validate()?;
        let d: Vec<f64> = cal
            .channels
            .iter()
            .map(|c| c.delay_ns + extra_delay_ns)
            .collect();
        CalibratedSink::new(inner, fs_hz, &d, &cal.gains_db(target_full_scale_dbm))
    }

    /// Look-ahead consumed at the start (samples).
    pub fn lead_samples(&self) -> usize {
        self.lead
    }
    pub fn inner(&self) -> &S {
        &self.inner
    }
    pub fn into_inner(self) -> S {
        self.inner
    }

    fn push(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        if channels.len() != self.firs.len() {
            return Err(format!(
                "calibration has {} channels, scene has {}",
                self.firs.len(),
                channels.len()
            ));
        }
        let mut out: Vec<Vec<f64>> = Vec::with_capacity(channels.len());
        for (c, x) in channels.iter().enumerate() {
            let mut y = self.firs[c].process(x);
            let g = self.gains[c];
            for v in y.iter_mut() {
                *v *= g;
                if v.abs() > 1.0 {
                    self.clipped += 1;
                }
            }
            out.push(y);
        }
        let n = out[0].len();
        let drop = self.to_drop.min(n);
        self.to_drop -= drop;
        if drop == n {
            return Ok(());
        }
        if drop > 0 {
            for y in out.iter_mut() {
                y.drain(..drop);
            }
        }
        self.inner.write_block(&out)
    }
}

impl<S: SampleSink> SampleSink for CalibratedSink<S> {
    fn write_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        self.push(channels)
    }
    fn finish(&mut self) -> Result<(), String> {
        let pad = vec![vec![0.0; self.lead]; self.firs.len()];
        self.push(&pad)?;
        self.inner.finish()
    }
}

// ---------------------------------------------------------------------------------------
// Delay characterisation from a looped-back capture
// ---------------------------------------------------------------------------------------

/// Calibration pulse train: one standard LORAN-C pulse (phase 0, ECD 0) whose time origin is
/// at each PPS instant `pps_times_s` (plus `delay_ns`), evaluated analytically at `fs_hz`
/// for `n` samples, amplitude `amp`. The standard zero crossing is therefore at PPS + 30 µs.
pub fn calibration_pulse_train(
    fs_hz: f64,
    n: usize,
    pps_times_s: &[f64],
    delay_ns: f64,
    amp: f64,
) -> Vec<f64> {
    let sig = PulseSignal::loran_c();
    let ext = sig.envelope.extent_s();
    let mut x = vec![0.0; n];
    for &p in pps_times_s {
        let t0 = p + delay_ns * 1e-9;
        let a = ((t0 * fs_hz).floor().max(0.0)) as usize;
        let b = (((t0 + ext) * fs_hz).ceil() as usize + 1).min(n);
        for (i, v) in x.iter_mut().enumerate().take(b).skip(a) {
            *v += amp * sig.pulse(i as f64 / fs_hz - t0, 0.0, 0.0);
        }
    }
    x
}

/// Write a calibration pulse train (one pulse per PPS, period `pps_period_s`, first PPS at
/// sample 0) of `duration_s` into `sink` on `channels` channels — a producer for driving a
/// device during a loop-back calibration.
pub fn write_calibration_train(
    sink: &mut dyn SampleSink,
    fs_hz: f64,
    duration_s: f64,
    pps_period_s: f64,
    channels: usize,
    amp: f64,
) -> Result<(), String> {
    let n_total = (duration_s * fs_hz).round() as usize;
    let block = (0.01 * fs_hz) as usize;
    let mut n0 = 0;
    while n0 < n_total {
        let n = block.min(n_total - n0);
        let t_a = n0 as f64 / fs_hz;
        // PPS instants whose pulse overlaps this block, relative to the block start
        let k0 = ((t_a - 400e-6) / pps_period_s).floor().max(0.0) as i64;
        let k1 = ((t_a + n as f64 / fs_hz) / pps_period_s).ceil() as i64;
        let pps: Vec<f64> = (k0..=k1).map(|k| k as f64 * pps_period_s - t_a).collect();
        let x = calibration_pulse_train(fs_hz, n, &pps, 0.0, amp);
        sink.write_block(&vec![x; channels])?;
        n0 += n;
    }
    sink.finish()
}

/// Options of [`characterise_from_capture`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacteriseOptions {
    /// Centre of the delay search (ns).
    pub expected_delay_ns: f64,
    /// Half-width of the coarse search (ns). The coarse search correlates the whole pulse,
    /// so the right carrier cycle is found at good SNR.
    pub search_ns: f64,
}

impl Default for CharacteriseOptions {
    fn default() -> Self {
        CharacteriseOptions {
            expected_delay_ns: 0.0,
            search_ns: 20_000.0,
        }
    }
}

/// Result of a delay characterisation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DelayEstimate {
    /// Mean delay (ns) of the standard zero crossing after PPS + 30 µs.
    pub delay_ns: f64,
    /// Standard deviation of the per-PPS estimates (ns).
    pub std_ns: f64,
    /// Standard uncertainty of the mean (ns) = std / √n.
    pub u_ns: f64,
    pub n: usize,
    pub per_pps_ns: Vec<f64>,
}

/// Estimate the output delay from a looped-back capture of the calibration pulse train.
///
/// `capture` is one channel at `fs_hz`; `pps_times_s` are the PPS instants in the capture's
/// time (s from its first sample — for a PPS-aligned capture these are 0, 1, 2, …). For
/// each PPS: a coarse delay from the correlation with the known pulse over the search
/// window (sample resolution); the standard zero crossing nearest to it, located with a
/// cubic (4-point Lagrange) interpolant, which fixes the carrier cycle; then the delay that
/// maximises the continuous-time matched-filter output (the analytic pulse evaluated at
/// the trial delay), found with Newton's method — this uses the whole pulse's energy, so the
/// estimate is far less noisy than a single crossing. The template's standard zero crossing
/// is at 30 µs, so the result is the delay of the standard zero crossing after PPS + 30 µs.
pub fn characterise_from_capture(
    capture: &[f64],
    fs_hz: f64,
    pps_times_s: &[f64],
    opt: &CharacteriseOptions,
) -> Result<DelayEstimate, String> {
    let sig = PulseSignal::loran_c();
    let ext = sig.envelope.extent_s();
    let n_t = (ext * fs_hz).ceil() as usize;
    let mut per = Vec::new();
    for &p in pps_times_s {
        let centre = p + opt.expected_delay_ns * 1e-9;
        let lag_lo = ((centre - opt.search_ns * 1e-9) * fs_hz).floor() as i64;
        let lag_hi = ((centre + opt.search_ns * 1e-9) * fs_hz).ceil() as i64;
        if lag_lo < 0 || (lag_hi as usize + n_t + 4) >= capture.len() {
            continue; // pulse not fully inside the capture
        }
        // template at sample-aligned origin
        let tmpl: Vec<f64> = (0..n_t)
            .map(|i| sig.pulse(i as f64 / fs_hz, 0.0, 0.0))
            .collect();
        let mut best = (f64::MIN, lag_lo);
        for lag in lag_lo..=lag_hi {
            let o = lag as usize;
            let c: f64 = tmpl
                .iter()
                .enumerate()
                .map(|(i, t)| t * capture[o + i])
                .sum();
            if c > best.0 {
                best = (c, lag);
            }
        }
        let t_guess = best.1 as f64 / fs_hz + 30e-6;
        let Some(tz) = zero_crossing(capture, fs_hz, t_guess) else {
            continue;
        };
        // fine: maximise the continuous-delay matched-filter output c(τ) by Newton's method
        // from the zero-crossing estimate (which fixes the carrier cycle).
        let c = |tau: f64| -> f64 {
            let t0 = p + tau;
            let a = (t0 * fs_hz).floor().max(0.0) as usize;
            let b = (((t0 + ext) * fs_hz).ceil() as usize + 1).min(capture.len());
            (a..b)
                .map(|i| capture[i] * sig.pulse(i as f64 / fs_hz - t0, 0.0, 0.0))
                .sum()
        };
        let mut tau = tz - (p + 30e-6);
        let h = 2e-9;
        for _ in 0..8 {
            let (cm, c0, cp) = (c(tau - h), c(tau), c(tau + h));
            let d1 = (cp - cm) / (2.0 * h);
            let d2 = (cp - 2.0 * c0 + cm) / (h * h);
            if d2.is_nan() || d2 >= 0.0 {
                break;
            }
            let step = (-d1 / d2).clamp(-1e-6, 1e-6);
            tau += step;
            if step.abs() < 1e-14 {
                break;
            }
        }
        per.push(tau * 1e9);
    }
    if per.is_empty() {
        return Err("no calibration pulse found inside the capture".into());
    }
    let n = per.len();
    let mean = per.iter().sum::<f64>() / n as f64;
    let std = if n > 1 {
        (per.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
    } else {
        0.0
    };
    Ok(DelayEstimate {
        delay_ns: mean,
        std_ns: std,
        u_ns: std / (n as f64).sqrt(),
        n,
        per_pps_ns: per,
    })
}

/// Positive-going zero crossing nearest `t_guess` (within ±0.4 carrier cycle), refined with a
/// 4-point Lagrange cubic and Newton iteration. Returns seconds.
pub fn zero_crossing(x: &[f64], fs: f64, t_guess: f64) -> Option<f64> {
    let c = (t_guess * fs).round() as i64;
    let reach = (4e-6 * fs).ceil() as i64;
    let mut cand: Option<usize> = None;
    for d in 0..=reach {
        for i in [c + d, c - d] {
            if i >= 1
                && (i as usize) + 2 < x.len()
                && x[i as usize] <= 0.0
                && x[i as usize + 1] > 0.0
            {
                cand = Some(i as usize);
                break;
            }
        }
        if cand.is_some() {
            break;
        }
    }
    let i = cand?;
    let y = [x[i - 1], x[i], x[i + 1], x[i + 2]];
    let nodes = [-1.0, 0.0, 1.0, 2.0];
    let f = |u: f64| -> (f64, f64) {
        let (mut v, mut dv) = (0.0, 0.0);
        for j in 0..4 {
            let (mut l, mut dl) = (1.0, 0.0);
            for m in 0..4 {
                if m == j {
                    continue;
                }
                let den = nodes[j] - nodes[m];
                dl = dl * (u - nodes[m]) / den + l / den;
                l *= (u - nodes[m]) / den;
            }
            v += y[j] * l;
            dv += y[j] * dl;
        }
        (v, dv)
    };
    let mut u = -y[1] / (y[2] - y[1]);
    for _ in 0..20 {
        let (v, dv) = f(u);
        if dv == 0.0 {
            break;
        }
        let step = v / dv;
        u -= step;
        if step.abs() < 1e-12 {
            break;
        }
    }
    Some((i as f64 + u) / fs)
}

// ---------------------------------------------------------------------------------------
// Uncertainty budgets (GUM)
// ---------------------------------------------------------------------------------------

/// Probability distribution of a Type B input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Distribution {
    /// `value` is a standard uncertainty.
    #[default]
    Normal,
    /// `value` is a half-width `a`: u = a/√3.
    Rectangular,
    /// `value` is a half-width `a`: u = a/√6.
    Triangular,
    /// `value` is a half-width `a`: u = a/√2.
    UShaped,
}

/// One input of an uncertainty budget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyComponent {
    pub name: String,
    /// Standard uncertainty (Normal) or half-width (other distributions), ns.
    pub value_ns: f64,
    #[serde(default)]
    pub distribution: Distribution,
    /// Sensitivity coefficient (default 1).
    #[serde(default = "one")]
    pub sensitivity: f64,
}

fn one() -> f64 {
    1.0
}

impl UncertaintyComponent {
    pub fn normal(name: &str, u_ns: f64) -> UncertaintyComponent {
        UncertaintyComponent {
            name: name.into(),
            value_ns: u_ns,
            distribution: Distribution::Normal,
            sensitivity: 1.0,
        }
    }
    /// Standard uncertainty (ns), before the sensitivity coefficient.
    pub fn standard_u_ns(&self) -> f64 {
        let a = self.value_ns.abs();
        match self.distribution {
            Distribution::Normal => a,
            Distribution::Rectangular => a / 3f64.sqrt(),
            Distribution::Triangular => a / 6f64.sqrt(),
            Distribution::UShaped => a / 2f64.sqrt(),
        }
    }
}

/// One line of a reported budget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BudgetLine {
    pub name: String,
    /// "A" or "B".
    pub kind: String,
    pub distribution: Distribution,
    pub standard_u_ns: f64,
    pub sensitivity: f64,
    /// |c|·u (ns).
    pub contribution_ns: f64,
    /// Degrees of freedom (None = infinite).
    pub dof: Option<f64>,
}

impl BudgetLine {
    pub fn type_b(c: &UncertaintyComponent) -> BudgetLine {
        let u = c.standard_u_ns();
        BudgetLine {
            name: c.name.clone(),
            kind: "B".into(),
            distribution: c.distribution,
            standard_u_ns: u,
            sensitivity: c.sensitivity,
            contribution_ns: (c.sensitivity * u).abs(),
            dof: None,
        }
    }
}

/// Combined standard uncertainty (RSS of contributions) and Welch–Satterthwaite dof.
pub fn combine(lines: &[BudgetLine]) -> (f64, Option<f64>) {
    let uc = lines
        .iter()
        .map(|l| l.contribution_ns.powi(2))
        .sum::<f64>()
        .sqrt();
    let den: f64 = lines
        .iter()
        .filter_map(|l| l.dof.map(|v| l.contribution_ns.powi(4) / v))
        .sum();
    let dof = if den > 0.0 {
        Some(uc.powi(4) / den)
    } else {
        None
    };
    (uc, dof)
}

// ---------------------------------------------------------------------------------------
// Receiver internal delay calibration
// ---------------------------------------------------------------------------------------

/// Unit of time-interval-counter readings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicUnit {
    #[default]
    Seconds,
    Nanoseconds,
}

/// Parse TIC readings from CSV text: comment lines (`#`) and non-numeric header lines are
/// skipped; `column` (0-based) selects the column, default the last one. Returns ns.
pub fn parse_tic_csv(text: &str, column: Option<usize>, unit: TicUnit) -> Result<Vec<f64>, String> {
    let mut v = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = l.split([',', ';', '\t']).map(|c| c.trim()).collect();
        let idx = column.unwrap_or(cols.len() - 1);
        let Some(cell) = cols.get(idx) else {
            return Err(format!("line {}: no column {idx}", ln + 1));
        };
        match cell.parse::<f64>() {
            Ok(x) => v.push(match unit {
                TicUnit::Seconds => x * 1e9,
                TicUnit::Nanoseconds => x,
            }),
            Err(_) if v.is_empty() => continue, // header
            Err(_) => return Err(format!("line {}: not a number: {cell}", ln + 1)),
        }
    }
    if v.len() < 2 {
        return Err("need at least two TIC readings".into());
    }
    Ok(v)
}

/// Inputs of the receiver delay calibration.
///
/// Measurement model (TIC started by the reference 1PPS, stopped by the receiver 1PPS):
///
/// `TIC = d_channel + d_prop + d_rx (+ TIC offset)` ⇒ `d_rx = mean(TIC) − d_channel − d_prop − d_tic`
///
/// * `channel_delay_ns`: residual delay of the simulator channel at the connector relative
///   to the reference point (0 if a [`CalibratedSink`] compensated it; then enter the
///   calibration uncertainty), including the cable to the receiver if not separately listed;
/// * `propagation_delay_ns`: propagation delay present in the simulated signal that the
///   receiver does **not** remove (0 for a timing receiver configured with the simulated
///   position and ASF, the full simulated delay for a raw-TOA comparison);
/// * `tic_offset_ns`: TIC start/stop channel offset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReceiverDelayInputs {
    pub channel_delay_ns: f64,
    pub channel_delay_u_ns: f64,
    pub propagation_delay_ns: f64,
    #[serde(default)]
    pub propagation_delay_u_ns: f64,
    #[serde(default)]
    pub tic_offset_ns: f64,
    /// Further Type B inputs (TIC resolution, trigger level, cable, reference distribution…).
    #[serde(default)]
    pub type_b: Vec<UncertaintyComponent>,
    /// Coverage factor (default 2).
    #[serde(default = "two")]
    pub k: f64,
}

fn two() -> f64 {
    2.0
}

/// Report of a receiver delay calibration (JSON-serialisable).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReceiverDelayReport {
    pub model: String,
    pub n_readings: usize,
    pub mean_tic_ns: f64,
    pub std_tic_ns: f64,
    /// Standard deviation of the mean, s/√n (ns).
    pub sdom_ns: f64,
    /// Lag-1 autocorrelation of the readings.
    pub lag1_autocorrelation: f64,
    /// Effective number of independent readings n·(1−r₁)/(1+r₁) (≤ n; = n for r₁ ≤ 0).
    pub n_effective: f64,
    /// Type A standard uncertainty used (ns): s/√n_eff.
    pub u_a_ns: f64,
    pub autocorrelation_note: String,
    pub receiver_delay_ns: f64,
    pub budget: Vec<BudgetLine>,
    pub combined_u_ns: f64,
    pub effective_dof: Option<f64>,
    pub k: f64,
    pub expanded_u_ns: f64,
}

/// Compute the receiver internal delay and its GUM uncertainty budget.
pub fn receiver_delay(
    readings_ns: &[f64],
    inp: &ReceiverDelayInputs,
) -> Result<ReceiverDelayReport, String> {
    let n = readings_ns.len();
    if n < 2 {
        return Err("need at least two readings".into());
    }
    let mean = readings_ns.iter().sum::<f64>() / n as f64;
    let var = readings_ns.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let std = var.sqrt();
    let sdom = std / (n as f64).sqrt();
    let r1 = if var > 0.0 {
        let c1: f64 = readings_ns
            .windows(2)
            .map(|w| (w[0] - mean) * (w[1] - mean))
            .sum::<f64>()
            / n as f64;
        let c0 = var * (n - 1) as f64 / n as f64;
        c1 / c0
    } else {
        0.0
    };
    let n_eff = if r1 > 0.0 {
        (n as f64 * (1.0 - r1) / (1.0 + r1)).max(1.0)
    } else {
        n as f64
    };
    let u_a = std / n_eff.sqrt();
    let note = if r1 > 0.2 {
        format!(
            "Readings are positively autocorrelated (r1 = {r1:.2}): s/sqrt(n) = {sdom:.3} ns would understate the \
             uncertainty; Type A uses n_eff = {n_eff:.1}. For strongly correlated (flicker/random-walk) noise use the \
             time deviation TDEV at the averaging time instead."
        )
    } else {
        format!("Lag-1 autocorrelation r1 = {r1:.2}: readings treated as approximately independent (n_eff = {n_eff:.1}).")
    };
    let mut budget = vec![BudgetLine {
        name: "TIC readings (Type A)".into(),
        kind: "A".into(),
        distribution: Distribution::Normal,
        standard_u_ns: u_a,
        sensitivity: 1.0,
        contribution_ns: u_a,
        dof: Some((n_eff - 1.0).max(1.0)),
    }];
    budget.push(BudgetLine::type_b(&UncertaintyComponent {
        sensitivity: -1.0,
        ..UncertaintyComponent::normal("simulator channel delay", inp.channel_delay_u_ns)
    }));
    if inp.propagation_delay_u_ns > 0.0 {
        budget.push(BudgetLine::type_b(&UncertaintyComponent {
            sensitivity: -1.0,
            ..UncertaintyComponent::normal(
                "simulated propagation delay",
                inp.propagation_delay_u_ns,
            )
        }));
    }
    budget.extend(inp.type_b.iter().map(BudgetLine::type_b));
    let (uc, dof) = combine(&budget);
    Ok(ReceiverDelayReport {
        model: "d_rx = mean(TIC) - d_channel - d_prop - d_tic".into(),
        n_readings: n,
        mean_tic_ns: mean,
        std_tic_ns: std,
        sdom_ns: sdom,
        lag1_autocorrelation: r1,
        n_effective: n_eff,
        u_a_ns: u_a,
        autocorrelation_note: note,
        receiver_delay_ns: mean
            - inp.channel_delay_ns
            - inp.propagation_delay_ns
            - inp.tic_offset_ns,
        budget,
        combined_u_ns: uc,
        effective_dof: dof,
        k: inp.k,
        expanded_u_ns: inp.k * uc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hw::fir::delay_signal;
    use crate::scene::MemSink;
    use crate::timebase::gauss;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    const FS: f64 = 2e6;

    fn pps(n: usize, period: f64) -> Vec<f64> {
        (0..n).map(|k| 50e-6 + k as f64 * period).collect()
    }

    fn sample_cal() -> CalibrationFile {
        CalibrationFile {
            version: CALIBRATION_VERSION,
            date: "2026-11-18".into(),
            device: "driver=test".into(),
            sample_rate_hz: FS,
            reference_point: ReferencePoint::default(),
            instruments: vec![Instrument {
                role: "time interval counter".into(),
                model: "TIC-1".into(),
                serial: "42".into(),
                cal_due: None,
            }],
            channels: vec![
                ChannelCal {
                    channel: 0,
                    delay_ns: 412.3,
                    delay_u_ns: 0.4,
                    full_scale_dbm: 10.0,
                    full_scale_u_db: 0.1,
                    load_ohm: Some(50.0),
                    notes: String::new(),
                },
                ChannelCal {
                    channel: 1,
                    delay_ns: -37.9,
                    delay_u_ns: 0.4,
                    full_scale_dbm: 4.0,
                    full_scale_u_db: 0.1,
                    load_ohm: Some(50.0),
                    notes: String::new(),
                },
            ],
            gnss_offset: Some(GnssOffsetCal {
                eloran_minus_gnss_ns: 3.5,
                u_ns: 1.0,
                gnss_simulator: "sim".into(),
                gnss_sim_delay_ns: None,
                gnss_sim_delay_u_ns: None,
            }),
            notes: String::new(),
        }
    }

    #[test]
    fn calibration_file_round_trips_and_checks_version() {
        let c = sample_cal();
        let j = c.to_json();
        assert_eq!(CalibrationFile::from_json(&j).unwrap(), c);
        let bad = j.replace("\"version\": 1", "\"version\": 9");
        assert!(CalibrationFile::from_json(&bad).is_err());
        assert_eq!(c.gains_db(Some(0.0)), vec![-10.0, -4.0]);
    }

    #[test]
    fn characterise_recovers_known_delay_within_0p1_ns() {
        let p = pps(20, 1e-3);
        let n = (21e-3 * FS) as usize;
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        for &d in &[0.0, 137.42, -2843.7, 9_871.25] {
            let mut x = calibration_pulse_train(FS, n, &p, d, 0.5);
            for v in x.iter_mut() {
                *v += 1e-4 * gauss(&mut rng); // ~74 dB peak SNR
            }
            let e = characterise_from_capture(&x, FS, &p, &CharacteriseOptions::default()).unwrap();
            assert_eq!(e.n, 20);
            assert!(
                (e.delay_ns - d).abs() < 0.1,
                "delay {d}: got {}",
                e.delay_ns
            );
        }
    }

    #[test]
    fn characterise_recovers_fir_delayed_loopback() {
        let p = pps(10, 1e-3);
        let n = (11e-3 * FS) as usize;
        let x = calibration_pulse_train(FS, n, &p, 0.0, 0.5);
        let d_samples = 61.2748;
        let y = delay_signal(&x, d_samples, DELAY_FIR_HALF, DELAY_FIR_BETA);
        let e = characterise_from_capture(
            &y,
            FS,
            &p,
            &CharacteriseOptions {
                expected_delay_ns: 30_000.0,
                search_ns: 8_000.0,
            },
        )
        .unwrap();
        let want = d_samples / FS * 1e9;
        assert!(
            (e.delay_ns - want).abs() < 0.1,
            "want {want}, got {}",
            e.delay_ns
        );
    }

    #[test]
    fn calibrated_sink_advances_by_delay_and_applies_gain() {
        let p = pps(10, 1e-3);
        let n = (11e-3 * FS) as usize;
        let x = calibration_pulse_train(FS, n, &p, 0.0, 0.25);
        let delays = [412.3, -37.9];
        let mut s = CalibratedSink::new(MemSink::default(), FS, &delays, &[0.0, 6.0]).unwrap();
        for k in (0..n).step_by(7000) {
            let e = (k + 7000).min(n);
            s.write_block(&[x[k..e].to_vec(), x[k..e].to_vec()])
                .unwrap();
        }
        s.finish().unwrap();
        let out = s.into_inner().channels;
        assert_eq!(out[0].len(), n);
        for (c, d) in delays.iter().enumerate() {
            let e =
                characterise_from_capture(&out[c], FS, &p[1..], &CharacteriseOptions::default())
                    .unwrap();
            assert!((e.delay_ns + d).abs() < 0.1, "channel {c}: {}", e.delay_ns);
        }
        let energy = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>();
        let ratio = (energy(&out[1]) / energy(&out[0])).sqrt();
        assert!((ratio - 10f64.powf(6.0 / 20.0)).abs() < 1e-3, "{ratio}");
    }

    #[test]
    fn calibrated_then_device_delay_is_on_time() {
        // scene → CalibratedSink(d) → "device" adding d → on time at the connector
        let p = pps(10, 1e-3);
        let n = (11e-3 * FS) as usize;
        let x = calibration_pulse_train(FS, n, &p, 0.0, 0.3);
        let d_ns = 25_637.4;
        let mut s = CalibratedSink::new(MemSink::default(), FS, &[d_ns], &[]).unwrap();
        s.write_block(&[x]).unwrap();
        s.finish().unwrap();
        let pre = s.into_inner().channels.remove(0);
        let dev = delay_signal(&pre, d_ns * 1e-9 * FS, DELAY_FIR_HALF, DELAY_FIR_BETA);
        let e =
            characterise_from_capture(&dev, FS, &p[1..], &CharacteriseOptions::default()).unwrap();
        assert!(e.delay_ns.abs() < 0.1, "{}", e.delay_ns);
    }

    #[test]
    fn write_calibration_train_matches_direct_synthesis() {
        let mut m = MemSink::default();
        write_calibration_train(&mut m, FS, 0.05, 0.004, 1, 0.5).unwrap();
        let p: Vec<f64> = (0..13).map(|k| k as f64 * 0.004).collect();
        let direct = calibration_pulse_train(FS, (0.05 * FS) as usize, &p, 0.0, 0.5);
        let err = m.channels[0]
            .iter()
            .zip(&direct)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        assert!(err < 1e-9, "{err}");
    }

    #[test]
    fn tic_csv_and_receiver_delay_budget() {
        let csv = "# TIC log\ntime,interval_s\n0,1.2501e-6\n1,1.2499e-6\n2,1.2502e-6\n3,1.2498e-6\n4,1.2500e-6\n";
        let r = parse_tic_csv(csv, None, TicUnit::Seconds).unwrap();
        assert_eq!(r.len(), 5);
        assert!((r[0] - 1250.1).abs() < 1e-6);
        let inp = ReceiverDelayInputs {
            channel_delay_ns: 0.0,
            channel_delay_u_ns: 0.5,
            propagation_delay_ns: 1000.0,
            propagation_delay_u_ns: 0.0,
            tic_offset_ns: 0.0,
            type_b: vec![UncertaintyComponent {
                name: "TIC resolution".into(),
                value_ns: 0.05,
                distribution: Distribution::Rectangular,
                sensitivity: 1.0,
            }],
            k: 2.0,
        };
        let rep = receiver_delay(&r, &inp).unwrap();
        assert!((rep.receiver_delay_ns - 250.0).abs() < 1e-6);
        assert_eq!(rep.budget.len(), 3);
        let want = (rep.u_a_ns.powi(2) + 0.25 + (0.05f64 / 3f64.sqrt()).powi(2)).sqrt();
        assert!((rep.combined_u_ns - want).abs() < 1e-12);
        assert!((rep.expanded_u_ns - 2.0 * want).abs() < 1e-12);
        let j = serde_json::to_string(&rep).unwrap();
        assert!(j.contains("receiver_delay_ns"));
    }

    #[test]
    fn autocorrelated_readings_inflate_type_a() {
        // random walk readings: strongly autocorrelated
        let mut rng = ChaCha8Rng::seed_from_u64(9);
        let mut x = 0.0;
        let r: Vec<f64> = (0..500)
            .map(|_| {
                x += gauss(&mut rng);
                100.0 + x
            })
            .collect();
        let inp = ReceiverDelayInputs {
            channel_delay_ns: 0.0,
            channel_delay_u_ns: 0.0,
            propagation_delay_ns: 0.0,
            propagation_delay_u_ns: 0.0,
            tic_offset_ns: 0.0,
            type_b: vec![],
            k: 2.0,
        };
        let rep = receiver_delay(&r, &inp).unwrap();
        assert!(rep.lag1_autocorrelation > 0.8);
        assert!(rep.u_a_ns > 3.0 * rep.sdom_ns);
        assert!(rep.autocorrelation_note.contains("autocorrelated"));
    }
}
