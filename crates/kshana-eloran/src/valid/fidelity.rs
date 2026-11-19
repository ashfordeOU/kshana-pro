// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! D2 — automatic fidelity comparator: recording versus simulation of the same interval.
//!
//! # Inputs
//!
//! * the recording: an [`crate::io::Recording`] or samples plus sample rate;
//! * its capture metadata ([`CaptureMeta`]): receiver position, antenna (E-field or crossed
//!   loops with the heading), start UTC of sample 0, the field scale of a sample value 1.0
//!   and any front-end gain included in the samples;
//! * the reference ([`ReferenceSource`]): a scenario, or a selection of GRI-database chains
//!   with a propagation configuration (and optional interference/noise and time
//!   references).
//!
//! # Procedure
//!
//! 1. [`build_matching_scenario`]: the reference scenario with the receiver made static at
//!    the capture position, the capture start, duration, sample rate and field scale, the
//!    capture antenna as outputs, and every time reference a station names (ideal unless
//!    given).
//! 2. The scene is simulated to memory; both sample sets go through the same software
//!    receiver ([`crate::rx`]) with the capture position as approximate position.
//! 3. Comparison at three levels ([`compare_inputs`]):
//!    * **signal**: per station, the phase-decoded average of every pulse of every tracked
//!      group of the band-passed signal, both referenced to the *simulated* carrier ToA of
//!      each group (so a delay appears as a lag); envelope by complex demodulation and a
//!      one-carrier-period boxcar; normalised cross-correlation over ± `lag_search_us`
//!      (lag with parabolic refinement), least-squares amplitude ratio and relative RMS
//!      difference after alignment. Spectrum: Welch PSD (Hann, 50 % overlap), power in
//!      90–110 kHz and in the configured out-of-band bands. Noise floor: the receiver's
//!      noise estimate (field / 10^(SNR/20), RMS in 20 kHz), median over measurements.
//!    * **observable**: measurements matched by chain, station, channel and group: ToA
//!      difference mean (bias) and standard deviation, ECD, SNR and field-strength
//!      differences.
//!    * **solution**: range-domain position + clock of each run and their difference; time
//!      solution at the capture position per time reference.
//! 4. Every metric is checked against [`Thresholds`]; per-metric score 1 when within the
//!    threshold, decaying as threshold/|value| beyond (for the correlation
//!    (1 − threshold)/(1 − ρ)); overall score = 100 × mean score; pass = all metrics pass.
//!
//! Recording − simulation is the sign of every difference.
//!
//! # Limitations
//!
//! * The noise metrics compare the reference scenario's noise model with the site: configure
//!   the reference scenario's `[[interference]]` to the site's noise, or the noise-floor and
//!   SNR metrics fail by design. Levels below `noise_floor_min_uv_m` are treated as equal.
//! * Batch receiver (see [`crate::rx`]); a moving capture is not supported (the receiver is
//!   made static at the capture position).
//! * MODELLED: verified on simulated "recordings" with known perturbations (tests); no real
//!   recording has been compared yet.

use super::{stats, LedgerRow, Reference};
use crate::antenna::OutputChannel;
use crate::chain::Chain;
use crate::geo::{inverse, Geo};
use crate::gridb::{GriDb, Overrides};
use crate::interf::InterfConfig;
use crate::io::Recording;
use crate::prop::PropConfig;
use crate::rx::{
    self, dsp::fft, Antenna, Corrections, LoopMode, Measurement, RxConfig, RxInput, RxOutput, C64,
};
use crate::scenario::{ChainSpec, OutputFile, Scenario, SceneSection};
use crate::scene::{MemSink, SceneParams};
use crate::signal::Parity;
use crate::time::AbsTime;
use crate::timebase::TimeRefModel;
use crate::traj::Trajectory;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::f64::consts::PI;

/// Report format tag.
pub const FIDELITY_FORMAT: &str = "kshana-eloran-fidelity";

/// Antenna of a capture.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CaptureAntenna {
    /// One vertical E-field channel (channel 0).
    EField,
    /// Crossed H loops on channels 0 (bow axis) and 1 (athwartships), platform heading.
    CrossedLoop { heading_deg: f64 },
}

/// Capture metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureMeta {
    pub position: Geo,
    pub antenna: CaptureAntenna,
    /// UTC of sample 0, "YYYY-MM-DDTHH:MM:SS(.fff)Z".
    pub start_utc: String,
    /// Field (µV/m) at the antenna represented by a sample value 1.0, gain removed.
    pub scale_uv_m: f64,
    /// Front-end gain included in the samples (dB); the effective scale is
    /// `scale_uv_m · 10^(−gain_db/20)`.
    #[serde(default)]
    pub gain_db: f64,
    /// Free text: site, receiver, capture id.
    #[serde(default)]
    pub label: String,
}

impl CaptureMeta {
    /// Field (µV/m) per sample unit of the recorded samples.
    pub fn effective_scale(&self) -> f64 {
        self.scale_uv_m * 10f64.powf(-self.gain_db / 20.0)
    }
    /// Read the metadata from a SigMF `global` object key `"kshana:capture"` (same fields
    /// as this struct).
    pub fn from_sigmf(meta: &serde_json::Value) -> Result<CaptureMeta, String> {
        let v = meta
            .get("global")
            .and_then(|g| g.get("kshana:capture"))
            .ok_or("SigMF metadata has no global \"kshana:capture\" object")?;
        serde_json::from_value(v.clone()).map_err(|e| format!("kshana:capture: {e}"))
    }
    fn validate(&self) -> Result<(), String> {
        AbsTime::from_utc_iso(&self.start_utc)?;
        if !(self.scale_uv_m > 0.0 && self.scale_uv_m.is_finite() && self.gain_db.is_finite()) {
            return Err("capture: scale_uv_m must be positive and gain_db finite".into());
        }
        Ok(())
    }
}

/// A selection of GRI-database chains as the reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GriSelection {
    /// Database chain ids ([`GriDb::get`]).
    pub chain_ids: Vec<String>,
    /// Values missing from the database, per chain id.
    #[serde(default)]
    pub overrides: BTreeMap<String, Overrides>,
    #[serde(default)]
    pub propagation: PropConfig,
    #[serde(default)]
    pub interference: Vec<InterfConfig>,
    /// Time reference models; references a station names but not listed here are ideal.
    #[serde(default)]
    pub time_refs: Vec<TimeRefModel>,
    #[serde(default)]
    pub seed: u64,
}

/// What the recording is compared with.
#[derive(Clone, Debug, PartialEq)]
pub enum ReferenceSource {
    Scenario(Box<Scenario>),
    GriDb(GriSelection),
}

/// Pass/fail thresholds (absolute values unless stated).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    /// Minimum envelope correlation.
    pub envelope_corr_min: f64,
    /// Maximum envelope RMS difference relative to the recorded envelope RMS.
    pub envelope_rms_rel_max: f64,
    pub envelope_lag_us_max: f64,
    pub envelope_amp_db_max: f64,
    pub band_power_db_max: f64,
    pub oob_power_db_max: f64,
    pub noise_floor_db_max: f64,
    pub toa_bias_ns_max: f64,
    pub toa_std_ns_max: f64,
    pub ecd_us_max: f64,
    pub snr_db_max: f64,
    pub field_db_max: f64,
    pub position_m_max: f64,
    pub time_ns_max: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            envelope_corr_min: 0.99,
            envelope_rms_rel_max: 0.1,
            envelope_lag_us_max: 0.5,
            envelope_amp_db_max: 1.0,
            band_power_db_max: 1.0,
            oob_power_db_max: 3.0,
            noise_floor_db_max: 3.0,
            toa_bias_ns_max: 20.0,
            toa_std_ns_max: 20.0,
            ecd_us_max: 0.5,
            snr_db_max: 3.0,
            field_db_max: 1.0,
            position_m_max: 50.0,
            time_ns_max: 20.0,
        }
    }
}

/// Comparator configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FidelityConfig {
    pub thresholds: Thresholds,
    /// GRIs per receiver tracking window.
    pub track_gris: usize,
    /// Averaged-pulse window relative to the carrier ToA (µs).
    pub pulse_window_us: (f64, f64),
    /// Envelope lag search half-range (µs).
    pub lag_search_us: f64,
    /// Out-of-band bands for the spectrum comparison (Hz).
    pub oob_bands_hz: Vec<(f64, f64)>,
    /// Welch segment length (power of two).
    pub welch_n: usize,
    /// Noise levels below this (µV/m, RMS in 20 kHz) are clamped to it.
    pub noise_floor_min_uv_m: f64,
    /// Per-station propagation corrections used by the solutions (ns, key "chain/station").
    #[serde(default)]
    pub corrections: BTreeMap<String, f64>,
}

impl Default for FidelityConfig {
    fn default() -> Self {
        FidelityConfig {
            thresholds: Thresholds::default(),
            track_gris: 4,
            pulse_window_us: (-30.0, 300.0),
            lag_search_us: 20.0,
            oob_bands_hz: vec![(40_000.0, 80_000.0), (120_000.0, 160_000.0)],
            welch_n: 4096,
            noise_floor_min_uv_m: 0.01,
            corrections: BTreeMap::new(),
        }
    }
}

/// Level of a metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Signal,
    Observable,
    Solution,
}

/// Direction of a threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bound {
    /// `|value| ≤ threshold`.
    MaxAbs,
    /// `value ≥ threshold` (ideal 1, e.g. a correlation).
    Min,
}

/// One checked metric.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub level: Level,
    /// "chain/station", or `None` for whole-capture metrics.
    pub station: Option<String>,
    pub name: String,
    #[serde(deserialize_with = "super::de_f64")]
    pub value: f64,
    pub unit: String,
    #[serde(deserialize_with = "super::de_f64")]
    pub threshold: f64,
    pub bound: Bound,
    pub pass: bool,
    #[serde(deserialize_with = "super::de_f64")]
    pub score: f64,
}

impl Metric {
    fn new(
        level: Level,
        station: Option<String>,
        name: &str,
        value: f64,
        unit: &str,
        threshold: f64,
        bound: Bound,
    ) -> Metric {
        let (pass, score) = if !value.is_finite() {
            (false, 0.0)
        } else {
            match bound {
                Bound::MaxAbs => {
                    if value.abs() <= threshold {
                        (true, 1.0)
                    } else {
                        (false, threshold / value.abs())
                    }
                }
                Bound::Min => {
                    if value >= threshold {
                        (true, 1.0)
                    } else {
                        (false, ((1.0 - threshold) / (1.0 - value)).clamp(0.0, 1.0))
                    }
                }
            }
        };
        Metric {
            level,
            station,
            name: name.into(),
            value,
            unit: unit.into(),
            threshold,
            bound,
            pass,
            score,
        }
    }
}

/// A phase-decoded average of a station's pulses (band-passed signal).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AveragedPulse {
    pub chain: String,
    pub station: String,
    pub fs_hz: f64,
    /// Time of `samples[0]` relative to the reference carrier ToA (s).
    pub tau0_s: f64,
    pub samples: Vec<f64>,
    pub pulses: usize,
}

impl AveragedPulse {
    /// Time of sample `j` relative to the reference carrier ToA (s).
    pub fn tau(&self, j: usize) -> f64 {
        self.tau0_s + j as f64 / self.fs_hz
    }
    /// Envelope by complex demodulation at `carrier_hz` and a one-period boxcar.
    pub fn envelope(&self, carrier_hz: f64) -> Vec<f64> {
        let w = 2.0 * PI * carrier_hz;
        let z: Vec<C64> = self
            .samples
            .iter()
            .enumerate()
            .map(|(j, a)| C64::cis(-w * self.tau(j)).scale(2.0 * a))
            .collect();
        let l = ((self.fs_hz / carrier_hz).round() as usize).max(1);
        let h = l / 2;
        let n = z.len();
        let mut pre = vec![C64::ZERO; n + 1];
        for i in 0..n {
            pre[i + 1] = pre[i] + z[i];
        }
        (0..n)
            .map(|i| {
                let a = i.saturating_sub(h);
                let b = (a + l).min(n);
                let a = b.saturating_sub(l);
                (pre[b] - pre[a]).scale(1.0 / (b - a) as f64).abs()
            })
            .collect()
    }
}

/// Average the pulses of station `si` of `chain` in the combined band-passed signal `y`.
/// `refs` gives (group, carrier ToA scene s) of measured groups; every group from the first
/// to the last reference (± `extend` groups) is used, its ToA taken from the nearest
/// reference moved by whole GRIs (static receiver). Samples outside `n_valid` are skipped
/// pulse by pulse.
#[allow(clippy::too_many_arguments)]
pub fn averaged_pulse(
    y: &[f64],
    fs: f64,
    n_valid: (usize, usize),
    chain: &Chain,
    si: usize,
    refs: &[(i64, f64)],
    window_us: (f64, f64),
    extend: i64,
) -> Option<AveragedPulse> {
    if refs.is_empty() {
        return None;
    }
    let st = &chain.stations[si];
    let gri = chain.gri_us as f64 * 1e-6;
    let g_lo = refs.iter().map(|r| r.0).min()? - extend;
    let g_hi = refs.iter().map(|r| r.0).max()? + extend;
    let j0 = (window_us.0 * 1e-6 * fs).floor() as i64;
    let len = ((window_us.1 - window_us.0) * 1e-6 * fs).ceil() as usize + 1;
    let mut acc = vec![0.0; len];
    let mut frac_sum = 0.0;
    let mut pulses = 0usize;
    for g in g_lo..=g_hi {
        let r = refs.iter().min_by_key(|r| (r.0 - g).abs())?;
        let toa = r.1 + (g - r.0) as f64 * gri;
        for p in chain.signal.group(st.role, Parity::of_group(g)) {
            let t_ref = toa + p.offset.as_s();
            let n_ref = (t_ref * fs).round() as i64;
            let first = n_ref + j0;
            let last = first + len as i64 - 1;
            if first < n_valid.0 as i64 || last >= n_valid.1 as i64 {
                continue;
            }
            let sgn = if p.phase_rad.abs() > 1.0 { -1.0 } else { 1.0 };
            for (j, a) in acc.iter_mut().enumerate() {
                *a += sgn * y[(first + j as i64) as usize];
            }
            frac_sum += n_ref as f64 / fs - t_ref;
            pulses += 1;
        }
    }
    if pulses == 0 {
        return None;
    }
    let k = 1.0 / pulses as f64;
    Some(AveragedPulse {
        chain: chain.name.clone(),
        station: st.id.clone(),
        fs_hz: fs,
        tau0_s: j0 as f64 / fs + frac_sum * k,
        samples: acc.into_iter().map(|a| a * k).collect(),
        pulses,
    })
}

/// Envelope comparison of one station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeComparison {
    /// Normalised envelope correlation at the best lag.
    #[serde(deserialize_with = "super::de_f64")]
    pub correlation: f64,
    /// Lag of the recorded envelope behind the simulated one (µs).
    #[serde(deserialize_with = "super::de_f64")]
    pub lag_us: f64,
    /// Least-squares amplitude ratio recorded/simulated after alignment (dB).
    #[serde(deserialize_with = "super::de_f64")]
    pub amplitude_db: f64,
    /// RMS of (recorded − gain·simulated) over RMS recorded, after alignment.
    #[serde(deserialize_with = "super::de_f64")]
    pub rms_rel: f64,
    pub pulses_rec: usize,
    pub pulses_sim: usize,
}

/// Compare two envelopes on the same grid (spacing `1/fs`).
pub fn compare_envelopes(
    rec: &[f64],
    sim: &[f64],
    fs: f64,
    max_lag_s: f64,
) -> (f64, f64, f64, f64) {
    let n = rec.len().min(sim.len());
    let ml = ((max_lag_s * fs).round() as i64).min(n as i64 / 2);
    let corr_at = |l: i64| -> (f64, f64, f64, f64) {
        // rec[j] vs sim[j − l]
        let (mut xy, mut xx, mut yy) = (0.0, 0.0, 0.0);
        for j in 0..n as i64 {
            let k = j - l;
            if k < 0 || k >= n as i64 {
                continue;
            }
            let (x, y) = (rec[j as usize], sim[k as usize]);
            xy += x * y;
            xx += x * x;
            yy += y * y;
        }
        let c = if xx > 0.0 && yy > 0.0 {
            xy / (xx * yy).sqrt()
        } else {
            0.0
        };
        (c, xy, xx, yy)
    };
    let mut best = (f64::NEG_INFINITY, 0i64);
    for l in -ml..=ml {
        let c = corr_at(l).0;
        if c > best.0 {
            best = (c, l);
        }
    }
    let l = best.1;
    let (c0, xy, xx, yy) = corr_at(l);
    let mut frac = 0.0;
    if l > -ml && l < ml {
        let cm = corr_at(l - 1).0;
        let cp = corr_at(l + 1).0;
        let den = cm - 2.0 * c0 + cp;
        if den < 0.0 {
            frac = (0.5 * (cm - cp) / den).clamp(-0.5, 0.5);
        }
    }
    let gain = if yy > 0.0 { xy / yy } else { 0.0 };
    let res = (xx - 2.0 * gain * xy + gain * gain * yy).max(0.0);
    let rms_rel = if xx > 0.0 {
        (res / xx).sqrt()
    } else {
        f64::NAN
    };
    (c0, (l as f64 + frac) / fs, 20.0 * gain.log10(), rms_rel)
}

/// Spectrum summary of one input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectrumSummary {
    /// Mean power in 90–110 kHz, dB re 1 (µV/m)².
    #[serde(deserialize_with = "super::de_f64")]
    pub band_power_db: f64,
    /// Mean power in the out-of-band bands, dB re 1 (µV/m)².
    #[serde(deserialize_with = "super::de_f64")]
    pub oob_power_db: f64,
    /// Noise floor: receiver noise estimate, RMS in 20 kHz (µV/m) and in dB(µV/m).
    #[serde(deserialize_with = "super::de_f64")]
    pub noise_floor_uv_m: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub noise_floor_db: f64,
}

/// Spectrum comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectrumComparison {
    pub rec: SpectrumSummary,
    pub sim: SpectrumSummary,
    #[serde(deserialize_with = "super::de_f64")]
    pub band_diff_db: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub oob_diff_db: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub noise_floor_diff_db: f64,
}

/// Welch power spectral density (one-sided, units²/Hz) of `x`; returns (df, psd).
pub fn welch_psd(x: &[f64], fs: f64, nseg: usize) -> (f64, Vec<f64>) {
    let mut n = nseg.max(16).next_power_of_two();
    while n > x.len().max(16) && n > 16 {
        n /= 2;
    }
    let win: Vec<f64> = (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos())
        .collect();
    let u: f64 = win.iter().map(|w| w * w).sum();
    let mut psd = vec![0.0; n / 2 + 1];
    let mut segs = 0usize;
    let mut s = 0usize;
    let mut buf = vec![C64::ZERO; n];
    while s + n <= x.len() {
        for i in 0..n {
            buf[i] = C64::new(x[s + i] * win[i], 0.0);
        }
        fft(&mut buf, false);
        for (k, p) in psd.iter_mut().enumerate() {
            let m = if k == 0 || k == n / 2 { 1.0 } else { 2.0 };
            *p += m * buf[k].norm_sqr() / (fs * u);
        }
        segs += 1;
        s += n / 2;
    }
    if segs > 0 {
        for p in psd.iter_mut() {
            *p /= segs as f64;
        }
    }
    (fs / n as f64, psd)
}

fn band_power(df: f64, psd: &[f64], lo: f64, hi: f64) -> f64 {
    psd.iter()
        .enumerate()
        .filter(|(k, _)| {
            let f = *k as f64 * df;
            f >= lo && f <= hi
        })
        .map(|(_, p)| p * df)
        .sum()
}

fn db10(p: f64) -> f64 {
    10.0 * p.max(1e-30).log10()
}

/// Observable and signal comparison of one station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationFidelity {
    pub chain: String,
    pub station: String,
    pub channel: Option<usize>,
    pub found_rec: bool,
    pub found_sim: bool,
    /// Matched measurements (same group).
    pub n_matched: usize,
    pub envelope: Option<EnvelopeComparison>,
    #[serde(deserialize_with = "super::de_f64")]
    pub toa_bias_ns: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub toa_std_ns: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub ecd_diff_us: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub snr_diff_db: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub field_diff_db: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub snr_rec_db: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub snr_sim_db: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub field_rec_uv_m: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub field_sim_uv_m: f64,
}

/// Solution comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolutionComparison {
    pub rec_position: Option<Geo>,
    pub sim_position: Option<Geo>,
    pub horizontal_diff_m: Option<f64>,
    pub rec_bias_ns: Option<f64>,
    pub sim_bias_ns: Option<f64>,
    pub bias_diff_ns: Option<f64>,
    /// Per time reference: (id, recorded − simulated time solution at the capture position, ns).
    pub time_diff_ns: Vec<(String, f64)>,
}

/// The fidelity report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FidelityReport {
    pub format: String,
    pub version: u32,
    pub capture: CaptureMeta,
    pub scenario_hash: String,
    pub propagation_model: String,
    #[serde(deserialize_with = "super::de_f64")]
    pub fs_hz: f64,
    #[serde(deserialize_with = "super::de_f64")]
    pub duration_s: f64,
    pub stations: Vec<StationFidelity>,
    pub spectrum: SpectrumComparison,
    pub solution: SolutionComparison,
    pub metrics: Vec<Metric>,
    /// 0–100.
    #[serde(deserialize_with = "super::de_f64")]
    pub score: f64,
    pub pass: bool,
    pub notes: Vec<String>,
}

impl FidelityReport {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
    pub fn from_json(s: &str) -> Result<FidelityReport, String> {
        serde_json::from_str(s).map_err(|e| format!("fidelity report: {e}"))
    }
    /// Metrics that failed.
    pub fn failures(&self) -> Vec<&Metric> {
        self.metrics.iter().filter(|m| !m.pass).collect()
    }
    /// The metric `name` of `station` (or whole-capture with `None`).
    pub fn metric(&self, station: Option<&str>, name: &str) -> Option<&Metric> {
        self.metrics
            .iter()
            .find(|m| m.name == name && m.station.as_deref() == station)
    }
    /// A ledger row for `model` (e.g. "End-to-end scene vs live capture, site X").
    pub fn ledger_row(&self, model: &str, reference: &Reference) -> LedgerRow {
        let fails = self.failures().len();
        LedgerRow::new(
            model,
            reference,
            &format!(
                "valid::fidelity comparator ({} metrics, capture {})",
                self.metrics.len(),
                self.capture.label
            ),
            "per-metric thresholds of the report (valid::fidelity::Thresholds)",
            &format!(
                "score {:.1}/100, {} of {} metrics pass",
                self.score,
                self.metrics.len() - fails,
                self.metrics.len()
            ),
            self.pass,
            self.stations.iter().filter(|s| s.n_matched > 0).count(),
        )
    }
}

/// Build the scenario matching a capture of `n_samples` at `fs_hz`.
pub fn build_matching_scenario(
    meta: &CaptureMeta,
    source: &ReferenceSource,
    n_samples: usize,
    fs_hz: f64,
) -> Result<Scenario, String> {
    meta.validate()?;
    if fs_hz.fract() != 0.0 || fs_hz < 400_000.0 {
        return Err(format!(
            "capture sample rate {fs_hz} Hz: the simulator needs a whole number ≥ 400 kHz (resample the capture)"
        ));
    }
    let duration_s = n_samples as f64 / fs_hz;
    let outputs = match meta.antenna {
        CaptureAntenna::EField => vec![OutputChannel::EField { gain: 1.0 }],
        CaptureAntenna::CrossedLoop { .. } => OutputChannel::crossed_loops(),
    };
    let heading_deg = match meta.antenna {
        CaptureAntenna::CrossedLoop { heading_deg } => heading_deg,
        CaptureAntenna::EField => 0.0,
    };
    let receiver = Trajectory::Static {
        lat_deg: meta.position.lat_deg,
        lon_deg: meta.position.lon_deg,
        h_m: meta.position.h_m,
        heading_deg,
    };
    let mut sc = match source {
        ReferenceSource::Scenario(s) => {
            let mut s = (**s).clone();
            s.scene.start_utc = meta.start_utc.clone();
            s.scene.params.fs_hz = fs_hz;
            s.scene.params.duration_s = duration_s;
            s.scene.params.output_scale_uv_m = meta.effective_scale();
            s.receiver = receiver;
            s.outputs = outputs;
            s
        }
        ReferenceSource::GriDb(sel) => {
            if sel.chain_ids.is_empty() {
                return Err("GRI selection: no chain ids".into());
            }
            let db = GriDb::embedded()?;
            let chains = sel
                .chain_ids
                .iter()
                .map(|id| {
                    let c = db
                        .get(id)
                        .ok_or_else(|| format!("GRI database has no chain {id:?}"))?;
                    let ov = sel.overrides.get(id).cloned().unwrap_or_default();
                    Ok(ChainSpec {
                        chain: c.to_chain(&ov)?,
                        ldc: None,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Scenario {
                scene: SceneSection {
                    start_utc: meta.start_utc.clone(),
                    params: SceneParams {
                        fs_hz,
                        duration_s,
                        block_s: 0.01,
                        output_scale_uv_m: meta.effective_scale(),
                        seed: sel.seed,
                    },
                },
                receiver,
                outputs,
                time_refs: sel.time_refs.clone(),
                gnss_time_residual_ns: BTreeMap::new(),
                propagation: sel.propagation.clone(),
                interference: sel.interference.clone(),
                chains,
                file: OutputFile {
                    format: "none".into(),
                },
            }
        }
    };
    // every referenced time reference must exist: ideal unless given
    let mut need: Vec<String> = Vec::new();
    for c in &sc.chains {
        for s in &c.chain.stations {
            if !s.time_ref.is_empty()
                && !sc.time_refs.iter().any(|t| t.id == s.time_ref)
                && !need.contains(&s.time_ref)
            {
                need.push(s.time_ref.clone());
            }
        }
    }
    for id in need {
        sc.time_refs.push(TimeRefModel::ideal(&id));
    }
    Ok(sc)
}

fn rx_config(meta: &CaptureMeta, cfg: &FidelityConfig) -> RxConfig {
    RxConfig {
        antenna: match meta.antenna {
            CaptureAntenna::EField => Antenna::EField { channel: 0 },
            CaptureAntenna::CrossedLoop { .. } => Antenna::CrossedLoop {
                a: 0,
                b: 1,
                mode: LoopMode::BeamSteer,
            },
        },
        track_gris: cfg.track_gris,
        approx_position: Some(meta.position),
        corrections: Corrections {
            asf_ns: cfg.corrections.clone(),
        },
        output_scale_uv_m: meta.effective_scale(),
        ..RxConfig::default()
    }
}

/// Compare a recording with a simulation built from `source` (see the module docs).
pub fn compare(
    recording: &Recording,
    meta: &CaptureMeta,
    source: &ReferenceSource,
    cfg: &FidelityConfig,
) -> Result<FidelityReport, String> {
    compare_samples(&recording.channels, recording.fs_hz, meta, source, cfg)
}

/// [`compare`] for samples in memory (`channels[c][n]`, sample units of the capture).
pub fn compare_samples(
    channels: &[Vec<f64>],
    fs_hz: f64,
    meta: &CaptureMeta,
    source: &ReferenceSource,
    cfg: &FidelityConfig,
) -> Result<FidelityReport, String> {
    let need = match meta.antenna {
        CaptureAntenna::EField => 1,
        CaptureAntenna::CrossedLoop { .. } => 2,
    };
    if channels.len() < need {
        return Err(format!(
            "capture has {} channels, the antenna needs {need}",
            channels.len()
        ));
    }
    let n = channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let sc = build_matching_scenario(meta, source, n, fs_hz)?;
    let mut scene = sc.build()?;
    let model = scene.prop.name();
    let mut sink = MemSink::default();
    let mut truth = Vec::new();
    scene.run(&mut sink, &mut truth)?;
    let start = AbsTime::from_utc_iso(&meta.start_utc)?;
    let rec = RxInput::new(
        channels.iter().map(|c| c[..n].to_vec()).collect(),
        fs_hz,
        start,
    );
    let sim = RxInput::new(sink.channels, fs_hz, start);
    let chains: Vec<Chain> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let mut rep = compare_inputs(&rec, &sim, &chains, meta, cfg)?;
    rep.scenario_hash = sc.hash();
    rep.propagation_model = model;
    Ok(rep)
}

struct RxRun {
    rx: rx::Receiver,
    out: RxOutput,
}

impl RxRun {
    fn combined(&self, weights: &[(usize, f64)]) -> Vec<f64> {
        let n = self.rx.filtered.first().map(|c| c.len()).unwrap_or(0);
        (0..n)
            .map(|i| {
                weights
                    .iter()
                    .map(|(c, w)| self.rx.filtered[*c][i] * w)
                    .sum()
            })
            .collect()
    }
    fn meas(&self, chain: &str, station: &str) -> Vec<&Measurement> {
        self.out
            .measurements
            .iter()
            .filter(|m| m.chain == chain && m.station == station)
            .collect()
    }
    /// Median receiver noise estimate, RMS in 20 kHz (µV/m).
    fn noise_uv_m(&self) -> f64 {
        let mut v: Vec<f64> = self
            .out
            .measurements
            .iter()
            .map(|m| m.field_uv_m / 10f64.powf(m.snr_db / 20.0))
            .filter(|x| x.is_finite())
            .collect();
        if v.is_empty() {
            return 0.0;
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    }
}

fn weights_for(ms: &[&Measurement], antenna: CaptureAntenna) -> Vec<(usize, f64)> {
    match antenna {
        CaptureAntenna::EField => vec![(0, 1.0)],
        CaptureAntenna::CrossedLoop { .. } => {
            let b = ms
                .iter()
                .filter_map(|m| m.bearing_deg)
                .next()
                .unwrap_or(0.0)
                .to_radians();
            vec![(0, b.cos()), (1, b.sin())]
        }
    }
}

/// Compare two receiver inputs (recording, simulation) of the same chains. The scenario
/// hash and model name of the report are left empty.
pub fn compare_inputs(
    rec: &RxInput,
    sim: &RxInput,
    chains: &[Chain],
    meta: &CaptureMeta,
    cfg: &FidelityConfig,
) -> Result<FidelityReport, String> {
    let rcfg = rx_config(meta, cfg);
    let run = |input: &RxInput| -> Result<RxRun, String> {
        let (rx, out) = rx::process(input, chains, rcfg.clone())?;
        Ok(RxRun { rx, out })
    };
    let r = run(rec)?;
    let s = run(sim)?;
    let fs = rec.fs_hz;
    let th = &cfg.thresholds;
    let mut metrics = Vec::new();
    let mut notes = Vec::new();
    let mut stations = Vec::new();

    for ch in chains {
        for (si, st) in ch.stations.iter().enumerate() {
            let key = format!("{}/{}", ch.name, st.id);
            let mr = r.meas(&ch.name, &st.id);
            let msim = s.meas(&ch.name, &st.id);
            let found_rec = !mr.is_empty();
            let found_sim = !msim.is_empty();
            if !found_rec && !found_sim {
                notes.push(format!("{key}: tracked in neither input"));
                continue;
            }
            metrics.push(Metric::new(
                Level::Observable,
                Some(key.clone()),
                "station_tracked_in_both",
                if found_rec && found_sim { 0.0 } else { 1.0 },
                "flag",
                0.0,
                Bound::MaxAbs,
            ));
            // observables, matched by channel and group
            let pairs: Vec<(&Measurement, &Measurement)> = mr
                .iter()
                .filter_map(|a| {
                    msim.iter()
                        .find(|b| b.group == a.group && b.channel == a.channel)
                        .map(|b| (*a, *b))
                })
                .collect();
            let dt: Vec<f64> = pairs
                .iter()
                .map(|(a, b)| (a.toa_s - b.toa_s) * 1e9)
                .collect();
            let st_t = stats(&dt);
            let mean = |f: &dyn Fn(&Measurement, &Measurement) -> f64| -> f64 {
                if pairs.is_empty() {
                    f64::NAN
                } else {
                    pairs.iter().map(|(a, b)| f(a, b)).sum::<f64>() / pairs.len() as f64
                }
            };
            let snr_diff = mean(&|a, b| {
                let floor = |m: &Measurement| {
                    // clamp noise at the floor so noise-free inputs compare finitely
                    let noise =
                        (m.field_uv_m / 10f64.powf(m.snr_db / 20.0)).max(cfg.noise_floor_min_uv_m);
                    20.0 * (m.field_uv_m / noise).log10()
                };
                floor(a) - floor(b)
            });
            let field_diff = mean(&|a, b| 20.0 * (a.field_uv_m / b.field_uv_m).log10());
            let ecd_diff = mean(&|a, b| a.ecd_us - b.ecd_us);
            let avg = |v: &[&Measurement], f: fn(&Measurement) -> f64| -> f64 {
                if v.is_empty() {
                    f64::NAN
                } else {
                    v.iter().map(|m| f(m)).sum::<f64>() / v.len() as f64
                }
            };
            let mut sf = StationFidelity {
                chain: ch.name.clone(),
                station: st.id.clone(),
                channel: mr.first().and_then(|m| m.channel),
                found_rec,
                found_sim,
                n_matched: pairs.len(),
                envelope: None,
                toa_bias_ns: if pairs.is_empty() {
                    f64::NAN
                } else {
                    st_t.mean
                },
                toa_std_ns: if pairs.is_empty() { f64::NAN } else { st_t.std },
                ecd_diff_us: ecd_diff,
                snr_diff_db: snr_diff,
                field_diff_db: field_diff,
                snr_rec_db: avg(&mr, |m| m.snr_db),
                snr_sim_db: avg(&msim, |m| m.snr_db),
                field_rec_uv_m: avg(&mr, |m| m.field_uv_m),
                field_sim_uv_m: avg(&msim, |m| m.field_uv_m),
            };
            if found_rec && found_sim {
                let k = Some(key.clone());
                for (name, v, unit, t) in [
                    ("toa_bias", sf.toa_bias_ns, "ns", th.toa_bias_ns_max),
                    ("toa_std", sf.toa_std_ns, "ns", th.toa_std_ns_max),
                    ("ecd_diff", sf.ecd_diff_us, "us", th.ecd_us_max),
                    ("snr_diff", sf.snr_diff_db, "dB", th.snr_db_max),
                    ("field_diff", sf.field_diff_db, "dB", th.field_db_max),
                ] {
                    metrics.push(Metric::new(
                        Level::Observable,
                        k.clone(),
                        name,
                        v,
                        unit,
                        t,
                        Bound::MaxAbs,
                    ));
                }
                // signal level: averaged pulse envelopes, both on the simulated ToA
                let refs: Vec<(i64, f64)> = msim.iter().map(|m| (m.group, m.toa_s)).collect();
                let ext = (cfg.track_gris as i64) / 2;
                let yr = r.combined(&weights_for(&mr, meta.antenna));
                let ys = s.combined(&weights_for(&msim, meta.antenna));
                let pr = averaged_pulse(
                    &yr,
                    fs,
                    r.rx.n_valid,
                    ch,
                    si,
                    &refs,
                    cfg.pulse_window_us,
                    ext,
                );
                let ps = averaged_pulse(
                    &ys,
                    fs,
                    s.rx.n_valid,
                    ch,
                    si,
                    &refs,
                    cfg.pulse_window_us,
                    ext,
                );
                if let (Some(pr), Some(ps)) = (pr, ps) {
                    let er = pr.envelope(ch.signal.carrier_hz);
                    let es = ps.envelope(ch.signal.carrier_hz);
                    let (c, lag, amp, rms) =
                        compare_envelopes(&er, &es, fs, cfg.lag_search_us * 1e-6);
                    let lag_us = lag * 1e6 + (pr.tau0_s - ps.tau0_s) * 1e6;
                    sf.envelope = Some(EnvelopeComparison {
                        correlation: c,
                        lag_us,
                        amplitude_db: amp,
                        rms_rel: rms,
                        pulses_rec: pr.pulses,
                        pulses_sim: ps.pulses,
                    });
                    for (name, v, unit, t, b) in [
                        ("envelope_corr", c, "", th.envelope_corr_min, Bound::Min),
                        (
                            "envelope_lag",
                            lag_us,
                            "us",
                            th.envelope_lag_us_max,
                            Bound::MaxAbs,
                        ),
                        (
                            "envelope_amp",
                            amp,
                            "dB",
                            th.envelope_amp_db_max,
                            Bound::MaxAbs,
                        ),
                        (
                            "envelope_rms_rel",
                            rms,
                            "",
                            th.envelope_rms_rel_max,
                            Bound::MaxAbs,
                        ),
                    ] {
                        metrics.push(Metric::new(Level::Signal, k.clone(), name, v, unit, t, b));
                    }
                } else {
                    notes.push(format!("{key}: no complete pulse window for the envelope"));
                }
            }
            stations.push(sf);
        }
    }

    // spectrum and noise floor
    let summary = |input: &RxInput, run: &RxRun| -> SpectrumSummary {
        let scale = meta.effective_scale();
        let (mut band, mut oob) = (0.0, 0.0);
        for c in &input.channels {
            let x: Vec<f64> = c.iter().map(|v| v * scale).collect();
            let (df, psd) = welch_psd(&x, fs, cfg.welch_n);
            band += band_power(df, &psd, 90_000.0, 110_000.0);
            for (lo, hi) in &cfg.oob_bands_hz {
                oob += band_power(df, &psd, *lo, *hi);
            }
        }
        let nf = run.noise_uv_m().max(cfg.noise_floor_min_uv_m);
        SpectrumSummary {
            band_power_db: db10(band),
            oob_power_db: db10(oob),
            noise_floor_uv_m: nf,
            noise_floor_db: 20.0 * nf.log10(),
        }
    };
    let sr = summary(rec, &r);
    let ss = summary(sim, &s);
    let spectrum = SpectrumComparison {
        band_diff_db: sr.band_power_db - ss.band_power_db,
        oob_diff_db: sr.oob_power_db - ss.oob_power_db,
        noise_floor_diff_db: sr.noise_floor_db - ss.noise_floor_db,
        rec: sr,
        sim: ss,
    };
    for (name, v, t) in [
        (
            "band_power_diff",
            spectrum.band_diff_db,
            th.band_power_db_max,
        ),
        ("oob_power_diff", spectrum.oob_diff_db, th.oob_power_db_max),
        (
            "noise_floor_diff",
            spectrum.noise_floor_diff_db,
            th.noise_floor_db_max,
        ),
    ] {
        metrics.push(Metric::new(
            Level::Signal,
            None,
            name,
            v,
            "dB",
            t,
            Bound::MaxAbs,
        ));
    }

    // solutions
    let corr = Corrections {
        asf_ns: cfg.corrections.clone(),
    };
    let pos = |run: &RxRun| {
        rx::position_solution(
            &run.out.measurements,
            chains,
            rec.start,
            &corr,
            &Default::default(),
        )
        .ok()
        .filter(|p| p.converged)
    };
    let (pr, ps) = (pos(&r), pos(&s));
    let mut solution = SolutionComparison {
        rec_position: pr.as_ref().map(|p| p.position),
        sim_position: ps.as_ref().map(|p| p.position),
        horizontal_diff_m: None,
        rec_bias_ns: pr.as_ref().map(|p| p.clock_bias_ns[0].1),
        sim_bias_ns: ps.as_ref().map(|p| p.clock_bias_ns[0].1),
        bias_diff_ns: None,
        time_diff_ns: Vec::new(),
    };
    match (&pr, &ps) {
        (Some(a), Some(b)) => {
            let d = inverse(a.position, b.position).distance_m;
            let db = a.clock_bias_ns[0].1 - b.clock_bias_ns[0].1;
            solution.horizontal_diff_m = Some(d);
            solution.bias_diff_ns = Some(db);
            metrics.push(Metric::new(
                Level::Solution,
                None,
                "position_diff",
                d,
                "m",
                th.position_m_max,
                Bound::MaxAbs,
            ));
            metrics.push(Metric::new(
                Level::Solution,
                None,
                "clock_bias_diff",
                db,
                "ns",
                th.time_ns_max,
                Bound::MaxAbs,
            ));
        }
        (None, None) => notes.push("no position solution in either input".into()),
        _ => metrics.push(Metric::new(
            Level::Solution,
            None,
            "position_solved_in_both",
            1.0,
            "flag",
            0.0,
            Bound::MaxAbs,
        )),
    }
    let tsol = |run: &RxRun| {
        rx::time_solution(
            &run.out.measurements,
            chains,
            rec.start,
            meta.position,
            &corr,
        )
    };
    let (tr, ts) = (tsol(&r), tsol(&s));
    for (id, v, _) in &tr.by_time_ref {
        if let Some((_, w, _)) = ts.by_time_ref.iter().find(|x| &x.0 == id) {
            let d = v - w;
            solution.time_diff_ns.push((id.clone(), d));
            metrics.push(Metric::new(
                Level::Solution,
                Some(format!("time_ref:{id}")),
                "time_diff",
                d,
                "ns",
                th.time_ns_max,
                Bound::MaxAbs,
            ));
        }
    }

    let score = if metrics.is_empty() {
        0.0
    } else {
        100.0 * metrics.iter().map(|m| m.score).sum::<f64>() / metrics.len() as f64
    };
    let pass = !metrics.is_empty() && metrics.iter().all(|m| m.pass);
    Ok(FidelityReport {
        format: FIDELITY_FORMAT.into(),
        version: 1,
        capture: meta.clone(),
        scenario_hash: String::new(),
        propagation_model: String::new(),
        fs_hz: fs,
        duration_s: rec.channels.first().map(|c| c.len()).unwrap_or(0) as f64 / fs,
        stations,
        spectrum,
        solution,
        metrics,
        score,
        pass,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop::ground::GroundConfig;

    /// Synthetic four-station geometry (not real stations), PF-only reference model.
    const BASE: &str = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.8
fs_hz = 1000000
seed = 3
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
offset_ns = 20.0
[propagation.ground]
model = "simple"
[[chains]]
name = "fid"
gri_us = 79800
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.5, lon_deg = 1.0 }
time_ref = "UTC(A)"
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 53.5, lon_deg = 1.5 }
emission_delay_us = 13000.0
time_ref = "UTC(A)"
[[chains.stations]]
id = "Y"
role = "secondary"
position = { lat_deg = 55.5, lon_deg = 6.5 }
emission_delay_us = 29000.0
time_ref = "UTC(A)"
[[chains.stations]]
id = "Z"
role = "secondary"
position = { lat_deg = 53.8, lon_deg = 5.5 }
emission_delay_us = 46000.0
time_ref = "UTC(A)"
"#;

    fn base() -> Scenario {
        Scenario::from_toml(BASE).unwrap()
    }

    fn meta() -> CaptureMeta {
        CaptureMeta {
            position: Geo::new(55.0, 3.0),
            antenna: CaptureAntenna::EField,
            start_utc: "2026-11-18T00:00:00Z".into(),
            scale_uv_m: 1.0e6,
            gain_db: 0.0,
            label: "synthetic".into(),
        }
    }

    /// Simulate `sc` as the "recording"; returns samples and truth.
    fn record(sc: &Scenario) -> (Vec<Vec<f64>>, Vec<crate::TruthRecord>) {
        let mut scene = sc.build().unwrap();
        let mut sink = MemSink::default();
        let mut truth = Vec::new();
        scene.run(&mut sink, &mut truth).unwrap();
        (sink.channels, truth)
    }

    fn report(rec: &Scenario) -> (FidelityReport, Vec<crate::TruthRecord>) {
        let (ch, truth) = record(rec);
        let rep = compare_samples(
            &ch,
            1e6,
            &meta(),
            &ReferenceSource::Scenario(Box::new(base())),
            &FidelityConfig::default(),
        )
        .unwrap();
        (rep, truth)
    }

    fn st<'a>(r: &'a FidelityReport, id: &str) -> &'a StationFidelity {
        r.stations.iter().find(|s| s.station == id).unwrap()
    }

    #[test]
    fn identical_inputs_score_perfectly() {
        let (rep, _) = report(&base());
        assert!(rep.pass, "{:?}", rep.failures());
        assert!((rep.score - 100.0).abs() < 1e-9);
        assert_eq!(rep.stations.len(), 4);
        for s in &rep.stations {
            assert_eq!(s.toa_bias_ns, 0.0);
            let e = s.envelope.as_ref().unwrap();
            assert!(e.correlation > 0.999_999 && e.lag_us.abs() < 1e-6, "{e:?}");
            assert!(e.rms_rel < 1e-9);
        }
        assert_eq!(rep.solution.horizontal_diff_m, Some(0.0));
        // JSON round trip
        let back = FidelityReport::from_json(&rep.to_json()).unwrap();
        assert_eq!(back.metrics.len(), rep.metrics.len());
        assert!(rep
            .ledger_row("x", &Reference::synthetic("t"))
            .to_markdown()
            .contains("MODELLED"));
    }

    #[test]
    fn extra_delay_on_one_station_is_detected_and_quantified() {
        let mut sc = base();
        sc.chains[0].chain.stations[2].emission_delay_us += 1.0; // Y late by 1 µs
        let (rep, _) = report(&sc);
        assert!(!rep.pass);
        let y = st(&rep, "Y");
        assert!((y.toa_bias_ns - 1000.0).abs() < 5.0, "{}", y.toa_bias_ns);
        let lag = y.envelope.as_ref().unwrap().lag_us;
        assert!((lag - 1.0).abs() < 0.1, "lag {lag}");
        for id in ["M", "X", "Z"] {
            assert!(st(&rep, id).toa_bias_ns.abs() < 2.0);
            assert!(
                rep.metric(Some(&format!("fid/{id}")), "toa_bias")
                    .unwrap()
                    .pass
            );
        }
        assert!(!rep.metric(Some("fid/Y"), "toa_bias").unwrap().pass);
        assert!(!rep.metric(Some("fid/Y"), "envelope_lag").unwrap().pass);
        assert!(rep.score < 100.0 && rep.score > 80.0, "{}", rep.score);
        // the position solution moves
        assert!(rep.solution.horizontal_diff_m.unwrap() > 10.0);
    }

    #[test]
    fn different_asf_is_quantified_per_station() {
        let mut sc = base();
        sc.propagation.ground = GroundConfig::SmoothEarth {
            sigma_s_m: 0.01,
            eps_r: None,
            k_factor: crate::prop::ground::K_LORAN,
            temporal: None,
        };
        let (rep, truth) = report(&sc);
        // reference truth: PF only, so the delay difference is SF + ASF of the recording
        for s in &rep.stations {
            let tr: Vec<_> = truth.iter().filter(|r| r.station == s.station).collect();
            let want = tr.iter().map(|r| r.sf_ns + r.asf_ns).sum::<f64>() / tr.len() as f64;
            assert!(want > 100.0);
            assert!(
                (s.toa_bias_ns - want).abs() < 10.0,
                "{}: bias {} want {want}",
                s.station,
                s.toa_bias_ns
            );
            assert!(s.field_diff_db.abs() > 0.5, "{}", s.field_diff_db);
        }
        assert!(!rep.pass);
    }

    #[test]
    fn added_noise_is_detected_and_measured() {
        let mut sc = base();
        sc.interference.push(InterfConfig::Wgn {
            rms_uv_m_20khz: 1000.0,
            seed: 9,
        });
        let (rep, _) = report(&sc);
        let nf = rep.spectrum.rec.noise_floor_uv_m;
        assert!(
            (20.0 * (nf / 1000.0).log10()).abs() < 1.5,
            "noise floor {nf} µV/m"
        );
        assert!(!rep.metric(None, "noise_floor_diff").unwrap().pass);
        assert!(!rep.metric(None, "oob_power_diff").unwrap().pass);
        for s in &rep.stations {
            assert!(s.snr_diff_db < -20.0, "{}", s.snr_diff_db);
            assert!(s.toa_bias_ns.abs() < 15.0, "{}", s.toa_bias_ns);
            assert!(s.field_diff_db.abs() < 0.5);
        }
    }

    #[test]
    fn gri_db_selection_builds_a_scenario() {
        let db = GriDb::embedded().unwrap();
        let c = db
            .chains
            .iter()
            .find(|c| c.is_complete())
            .expect("a complete chain");
        let sel = GriSelection {
            chain_ids: vec![c.id.clone()],
            overrides: BTreeMap::new(),
            propagation: PropConfig::default(),
            interference: vec![],
            time_refs: vec![],
            seed: 0,
        };
        let sc =
            build_matching_scenario(&meta(), &ReferenceSource::GriDb(sel), 400_000, 1e6).unwrap();
        assert!((sc.scene.params.duration_s - 0.4).abs() < 1e-12);
        assert_eq!(sc.scene.params.output_scale_uv_m, 1e6);
        sc.build().unwrap();
        let bad = GriSelection {
            chain_ids: vec!["no-such-chain".into()],
            overrides: BTreeMap::new(),
            propagation: PropConfig::default(),
            interference: vec![],
            time_refs: vec![],
            seed: 0,
        };
        assert!(build_matching_scenario(&meta(), &ReferenceSource::GriDb(bad), 1000, 1e6).is_err());
    }

    #[test]
    fn capture_meta_from_sigmf_and_gain() {
        let v = serde_json::json!({"global": {"kshana:capture": {
            "position": {"lat_deg": 1.0, "lon_deg": 2.0},
            "antenna": {"type": "crossed_loop", "heading_deg": 30.0},
            "start_utc": "2026-01-01T00:00:00Z",
            "scale_uv_m": 1000.0, "gain_db": 20.0}}});
        let m = CaptureMeta::from_sigmf(&v).unwrap();
        assert!((m.effective_scale() - 100.0).abs() < 1e-9);
        assert_eq!(m.antenna, CaptureAntenna::CrossedLoop { heading_deg: 30.0 });
    }
}
