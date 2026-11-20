// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # RF self-interference forensics from I/Q (E24 B2404; analysis only)
//!
//! A lander or rover carries its own interferers: motor drives, DC-DC converters, camera and
//! data-bus clocks. Given a SigMF recording of the navigation receiver's band taken on the
//! vehicle, this module:
//!
//! 1. estimates the Welch power spectral density of the whole recording (the open engine's
//!    [`kshana::spectrum::welch_psd`]) and its noise floor (the median bin, corrected to the
//!    mean of a Welch bin with `K` equivalent segments by the Wilson-Hilferty median of a
//!    gamma variable, `median ≈ mean·(1 − 1/(9K))³`);
//! 2. finds narrowband spurs: runs of bins above the floor by the excess at which the largest
//!    of all noise bins would exceed it with the stated false-alarm probability
//!    ([`kshana::iq::monitor::stats::welch_excess_threshold_db`]);
//! 3. matches each spur's absolute frequency to a harmonic `n·f₀` of the on-board sources the
//!    user declares, within each source's tolerance;
//! 4. splits the recording into blocks and, with a declared activity log (which source was on
//!    when), gives the Pearson correlation of each spur's block power with each source's
//!    on/off state;
//! 5. integrates the interference above the floor inside the declared signal band: the
//!    interference-to-noise ratio and the C/N0 loss `10·log₁₀(1 + I/N)` it implies when the
//!    interference is treated as white across the band (an upper-bound style screening value
//!    for a narrowband spur that a correlator partly rejects).
//!
//! Analysis only: on-board sources are declared, not located; nothing transmits.
//! **MODELLED** on SYNTHETIC recordings ([`super::synth`]).

use std::collections::BTreeMap;
use std::path::Path;

use kshana::sdr::Cf64;
use serde::{Deserialize, Serialize};

use super::{positive, r, read_rel, read_rel_bytes, EVIDENCE};
use crate::transport::Table;

/// Pro feature.
pub const FEATURE: &str = "pro-lunar-rf-forensics";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-lunar-rf-forensics/1";
/// Method identifier.
pub const METHOD_ID: &str = "kshana-pro/self-interference-iq/v1";

/// A declared on-board source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub name: String,
    pub fundamental_hz: f64,
    #[serde(default = "d_harm")]
    pub max_harmonic: u32,
    /// Match tolerance on each harmonic (Hz).
    pub tolerance_hz: f64,
}

fn d_harm() -> u32 {
    100_000
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// SigMF metadata file.
    pub meta: String,
    /// SigMF data file.
    pub data: String,
    /// Full-scale value for integer sample types.
    #[serde(default = "d_fs")]
    pub full_scale: f64,
    #[serde(default = "d_nfft")]
    pub nfft: usize,
    #[serde(default = "d_overlap")]
    pub overlap: f64,
    /// False-alarm probability for the largest noise bin of the whole-recording spectrum.
    #[serde(default = "d_pfa")]
    pub pfa: f64,
    pub candidates: Vec<Candidate>,
    /// Signal band `[low, high]` in absolute Hz.
    pub signal_band_hz: [f64; 2],
    /// Optional activity log CSV `t_s,source,state` (`on`/`off`), seconds from the start.
    #[serde(default)]
    pub activity: Option<String>,
    /// Block length for the time profile (s).
    #[serde(default = "d_block")]
    pub block_s: f64,
    /// Correlation at or above which a spur is "time-correlated" with a source.
    #[serde(default = "d_corr")]
    pub correlation_threshold: f64,
}

fn d_fs() -> f64 {
    1.0
}
fn d_nfft() -> usize {
    4096
}
fn d_overlap() -> f64 {
    0.5
}
fn d_pfa() -> f64 {
    1e-3
}
fn d_block() -> f64 {
    0.01
}
fn d_corr() -> f64 {
    0.7
}

/// One harmonic match.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Match {
    pub source: String,
    pub harmonic: u32,
    pub offset_hz: f64,
}

/// One spur.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Spur {
    /// Absolute frequency of the peak bin (Hz).
    pub frequency_hz: f64,
    /// Offset from the recording centre (Hz).
    pub offset_hz: f64,
    pub bins: usize,
    /// Peak above the floor (dB).
    pub peak_above_floor_db: f64,
    /// Power above the floor, integrated over the run (input units², i.e. W if √W).
    pub power: f64,
    pub in_signal_band: bool,
    pub matches: Vec<Match>,
    /// Pearson correlation of block power (dB) with each source's on state.
    pub correlation: BTreeMap<String, f64>,
    /// Sources at or above the correlation threshold.
    pub time_correlated_with: Vec<String>,
}

/// The result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub method: &'static str,
    pub title: String,
    pub evidence: &'static str,
    pub request: Request,
    pub data_sha256: String,
    pub sample_rate_hz: f64,
    pub centre_hz: f64,
    pub samples: usize,
    pub segments: usize,
    pub equivalent_segments: f64,
    pub bin_hz: f64,
    pub floor_density: f64,
    pub threshold_db: f64,
    pub spurs: Vec<Spur>,
    pub unattributed: usize,
    /// In-band interference-to-noise ratio (dB); `None` when no interference in band.
    pub in_band_i_over_n_db: Option<f64>,
    pub cn0_loss_db: f64,
    pub blocks: usize,
    pub verdict: String,
    pub limits: Vec<&'static str>,
}

const LIMITS: &[&str] = &[
    "Harmonic matching is by frequency only: two sources with shared harmonics are both listed.",
    "The C/N0 loss treats in-band interference as white across the band; a correlator rejects part of a narrowband spur, so the loss is a screening value.",
    "Spurs from the recorder itself (DC offset, image, clock) also appear and must be ruled out with a terminated-input recording.",
    "Sources are declared by the user; nothing is located.",
];

struct Activity {
    by_source: BTreeMap<String, Vec<(f64, bool)>>,
}

impl Activity {
    fn parse(text: &str) -> Result<Activity, String> {
        let t = Table::parse(text)?;
        let (ct, cs, cst) = (t.need("t_s")?, t.need("source")?, t.need("state")?);
        let mut by: BTreeMap<String, Vec<(f64, bool)>> = BTreeMap::new();
        for i in 0..t.rows.len() {
            let ts = t
                .num(i, Some(ct))?
                .ok_or_else(|| format!("row {}: empty t_s", i + 2))?;
            let on = match t.text(i, Some(cst)).to_ascii_lowercase().as_str() {
                "on" | "1" | "true" => true,
                "off" | "0" | "false" => false,
                o => return Err(format!("row {}: state '{o}' is not on or off", i + 2)),
            };
            by.entry(t.text(i, Some(cs)).to_string())
                .or_default()
                .push((ts, on));
        }
        for v in by.values_mut() {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        Ok(Activity { by_source: by })
    }

    fn on_at(&self, src: &str, t: f64) -> bool {
        self.by_source
            .get(src)
            .and_then(|v| v.iter().rev().find(|(ts, _)| *ts <= t))
            .is_some_and(|x| x.1)
    }
}

fn pearson(a: &[f64], b: &[f64]) -> Option<f64> {
    let n = a.len() as f64;
    if a.len() < 3 {
        return None;
    }
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma).powi(2);
        sbb += (y - mb).powi(2);
    }
    (saa > 0.0 && sbb > 0.0).then(|| sab / (saa * sbb).sqrt())
}

fn validate(q: &Request) -> Result<(), String> {
    positive("full_scale", q.full_scale)?;
    positive("block_s", q.block_s)?;
    if q.nfft < 64 || !q.nfft.is_power_of_two() {
        return Err("nfft must be a power of two of at least 64".into());
    }
    if !(0.0..=0.95).contains(&q.overlap) {
        return Err("overlap must be in [0, 0.95]".into());
    }
    if !(q.pfa > 0.0 && q.pfa < 1.0) {
        return Err("pfa must be in (0, 1)".into());
    }
    if q.signal_band_hz[1].partial_cmp(&q.signal_band_hz[0]) != Some(std::cmp::Ordering::Greater) {
        return Err("signal_band_hz must be [low, high] with low < high".into());
    }
    for c in &q.candidates {
        positive(
            &format!("candidate '{}' fundamental_hz", c.name),
            c.fundamental_hz,
        )?;
        positive(
            &format!("candidate '{}' tolerance_hz", c.name),
            c.tolerance_hz,
        )?;
        if c.max_harmonic == 0 {
            return Err(format!(
                "candidate '{}': max_harmonic must be at least 1",
                c.name
            ));
        }
    }
    Ok(())
}

/// Run on a request whose files are relative to `base`.
pub fn run(q: &Request, base: &Path) -> Result<Report, String> {
    validate(q)?;
    let meta = read_rel(base, &q.meta)?;
    let data = read_rel_bytes(base, &q.data)?;
    let rec = kshana::sigmf::read(&meta, &data, q.full_scale)?;
    let fs = rec.sample_rate_hz()?;
    let centre = rec.meta.centre_hz().unwrap_or(0.0);
    let act = match &q.activity {
        Some(a) => Some(Activity::parse(&read_rel(base, a)?).map_err(|e| format!("{a}: {e}"))?),
        None => None,
    };
    analyse(
        q,
        &rec.samples,
        fs,
        centre,
        act.as_ref(),
        crate::forensics::inputs::sha256_hex(&data),
    )
}

fn analyse(
    q: &Request,
    x: &[Cf64],
    fs: f64,
    centre: f64,
    act: Option<&Activity>,
    sha: String,
) -> Result<Report, String> {
    let w = kshana::spectrum::welch_psd(x, fs, q.nfft, q.overlap)?;
    let step = ((q.nfft as f64) * (1.0 - q.overlap)).round().max(1.0) as usize;
    let k = kshana::iq::monitor::stats::welch_equivalent_segments(q.nfft, step, w.segments);
    let mut sorted = w.psd.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    let floor = median / (1.0 - 1.0 / (9.0 * k)).powi(3);
    let thr_db = kshana::iq::monitor::stats::welch_excess_threshold_db(k, w.psd.len(), q.pfa);
    let thr = floor * 10f64.powf(thr_db / 10.0);
    let bin = fs / q.nfft as f64;
    // Runs of bins above the threshold (ignoring the DC bin, which recorders often spike).
    let dc = q.nfft / 2;
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < w.psd.len() {
        if w.psd[i] > thr && i != dc {
            let s = i;
            while i + 1 < w.psd.len() && w.psd[i + 1] > thr && i + 1 != dc {
                i += 1;
            }
            runs.push((s, i));
        }
        i += 1;
    }
    // Block profile.
    let blk = ((q.block_s * fs).round() as usize).max(q.nfft);
    let nblocks = x.len() / blk;
    let block_psd: Vec<Vec<f64>> = (0..nblocks)
        .filter_map(|b| {
            kshana::spectrum::welch_psd(&x[b * blk..(b + 1) * blk], fs, q.nfft, q.overlap)
                .ok()
                .map(|p| p.psd)
        })
        .collect();
    let [lo, hi] = q.signal_band_hz;
    let mut spurs = Vec::new();
    for (s, e) in runs {
        let peak = (s..=e)
            .max_by(|a, b| w.psd[*a].total_cmp(&w.psd[*b]))
            .expect("non-empty run");
        let off = w.freq_hz[peak];
        let f_abs = centre + off;
        let power: f64 = (s..=e).map(|j| (w.psd[j] - floor).max(0.0) * bin).sum();
        let matches: Vec<Match> = q
            .candidates
            .iter()
            .filter_map(|c| {
                let n = (f_abs / c.fundamental_hz).round();
                if n < 1.0 || n > c.max_harmonic as f64 {
                    return None;
                }
                let d = f_abs - n * c.fundamental_hz;
                (d.abs() <= c.tolerance_hz + bin / 2.0).then(|| Match {
                    source: c.name.clone(),
                    harmonic: n as u32,
                    offset_hz: r(d, 3),
                })
            })
            .collect();
        let mut correlation = BTreeMap::new();
        let mut correlated = Vec::new();
        if let Some(a) = act {
            let lvl: Vec<f64> = block_psd
                .iter()
                .map(|p| {
                    let m = (s.saturating_sub(1)..=(e + 1).min(p.len() - 1))
                        .map(|j| p[j])
                        .fold(0.0, f64::max);
                    10.0 * (m / floor).max(1e-12).log10()
                })
                .collect();
            for src in a.by_source.keys() {
                let on: Vec<f64> = (0..block_psd.len())
                    .map(|b| {
                        let tm = (b as f64 + 0.5) * blk as f64 / fs;
                        if a.on_at(src, tm) {
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .collect();
                if let Some(c) = pearson(&lvl, &on) {
                    correlation.insert(src.clone(), r(c, 4));
                    if c >= q.correlation_threshold {
                        correlated.push(src.clone());
                    }
                }
            }
        }
        spurs.push(Spur {
            frequency_hz: r(f_abs, 3),
            offset_hz: r(off, 3),
            bins: e - s + 1,
            peak_above_floor_db: r(10.0 * (w.psd[peak] / floor).log10(), 2),
            power,
            in_signal_band: f_abs >= lo && f_abs <= hi,
            matches,
            correlation,
            time_correlated_with: correlated,
        });
    }
    // In-band I/N.
    let in_band: Vec<usize> = (0..w.psd.len())
        .filter(|&j| {
            let f = centre + w.freq_hz[j];
            f >= lo && f <= hi && j != dc
        })
        .collect();
    let n_pow = floor * bin * in_band.len() as f64;
    let i_pow: f64 = in_band
        .iter()
        .filter(|&&j| w.psd[j] > thr)
        .map(|&j| (w.psd[j] - floor) * bin)
        .sum();
    let ion = (n_pow > 0.0 && i_pow > 0.0).then(|| 10.0 * (i_pow / n_pow).log10());
    let loss = if n_pow > 0.0 {
        10.0 * (1.0 + i_pow / n_pow).log10()
    } else {
        0.0
    };
    let unattributed = spurs.iter().filter(|s| s.matches.is_empty()).count();
    let verdict = format!(
        "{} spur(s) above the floor at Pfa {}; {} matched to a declared source, {} unattributed; {} time-correlated with the activity log; in-band C/N0 loss {:.2} dB (white-equivalent screening value). Analysis only.",
        spurs.len(),
        q.pfa,
        spurs.len() - unattributed,
        unattributed,
        spurs
            .iter()
            .filter(|s| !s.time_correlated_with.is_empty())
            .count(),
        loss
    );
    Ok(Report {
        format: FORMAT,
        method: METHOD_ID,
        title: q.title.clone(),
        evidence: EVIDENCE,
        request: q.clone(),
        data_sha256: sha,
        sample_rate_hz: fs,
        centre_hz: centre,
        samples: x.len(),
        segments: w.segments,
        equivalent_segments: r(k, 3),
        bin_hz: bin,
        floor_density: floor,
        threshold_db: r(thr_db, 3),
        spurs,
        unattributed,
        in_band_i_over_n_db: ion.map(|v| r(v, 3)),
        cn0_loss_db: r(loss, 4),
        blocks: block_psd.len(),
        verdict,
        limits: LIMITS.to_vec(),
    })
}

/// HTML page.
pub fn html(rep: &Report) -> String {
    let rows = rep
        .spurs
        .iter()
        .map(|s| {
            vec![
                format!("{:.1}", s.frequency_hz),
                format!("{:.1}", s.peak_above_floor_db),
                s.in_signal_band.to_string(),
                s.matches
                    .iter()
                    .map(|m| format!("{} ×{}", m.source, m.harmonic))
                    .collect::<Vec<_>>()
                    .join(", "),
                s.time_correlated_with.join(", "),
            ]
        })
        .collect();
    crate::transport::page(
        &rep.title,
        rep.evidence,
        &rep.verdict,
        &[
            "Analysis of a recording only: sources are declared, not located; nothing transmits."
                .into(),
        ],
        &[(
            "Spurs".into(),
            vec![
                "frequency (Hz)",
                "above floor (dB)",
                "in signal band",
                "harmonic of",
                "time-correlated with",
            ],
            rows,
        )],
    )
}
