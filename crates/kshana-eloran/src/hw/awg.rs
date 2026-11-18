// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Waveform compiler for arbitrary waveform generators (AWGs): render one **repeating
//! segment** of a scenario whose output is periodic, so a memory-based generator (which
//! cannot stream continuously) can loop it forever.
//!
//! # Periodicity
//!
//! A LORAN-C chain repeats its group every GRI, and its phase code every **two GRIs**
//! (the A/B phase-code interval, COMDTINST M16562.4A §2). The emission schedule is on the
//! Loran epoch grid (`crate::chain::emissions`), so the sum of several chains repeats with
//! the least common multiple of their 2×GRI. On a grid of `fs` samples per second the
//! segment must also be a whole number of samples, so the period is extended to the
//! smallest multiple `k·LCM` for which `k·LCM·fs` is an integer.
//!
//! The scene output is periodic only if nothing in it varies with time. [`analyse`] checks
//! the scenario and lists every reason it is not:
//!
//! * a moving receiver (waypoint trajectory);
//! * station faults (time windows), LDC data channels (the data change group to group);
//! * time references with drift, clock noise or steps (constant offsets are fine);
//! * an ionospheric skywave (day/night, fading) or temporal ground-wave variation
//!   (a fixed skywave and time-invariant ground models are fine);
//! * noise of any kind; CW interference not commensurate with the period, or windowed;
//!   pulsed interference (not analysed — refused).
//!
//! A non-periodic scene is refused with these reasons, or — on request
//! ([`NonPeriodic::Finite`]) — rendered as a finite segment of the requested length, which
//! is then a one-shot (burst) waveform: looping it is not faithful to the scenario.
//!
//! # Output and checks
//!
//! The segment is rendered by the scene engine itself (picosecond-exact pulse timing) and
//! quantised to the device resolution, optionally with TPDF dither. The [`AwgReport`]
//! gives the segment length, memory, the maximum and RMS error of the quantised segment
//! against the exact (f64) model, clipping, and the **repeat-boundary check**: the scene is
//! rendered `guard_samples` past the period and the overhang is compared with the start of
//! the segment — equal (within 1e-9 full scale) means the loop is seamless.
//!
//! Evidence: **MODELLED** — internally verified against the scene engine
//! (`periodic_segment_matches_scene_and_loops_seamlessly`); the device's own playback
//! (sample clock, reconstruction, loop latency) needs hardware to verify.

use crate::interf::InterfConfig;
use crate::prop::ground::GroundConfig;
use crate::prop::sky::SkyConfig;
use crate::scenario::Scenario;
use crate::scene::MemSink;
use crate::traj::Trajectory;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// What to do with a scene that is not periodic (or whose period does not fit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NonPeriodic {
    /// Refuse with the reasons.
    Refuse,
    /// Render the first `samples` samples per channel as a finite (one-shot) segment.
    Finite { samples: u64 },
}

/// Options of [`compile`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AwgOptions {
    /// Device waveform memory (samples per channel).
    pub max_samples: u64,
    /// DAC resolution (bits, 2..=32); codes are signed, full scale ±(2^(bits−1) − 1).
    pub bits: u32,
    /// Add triangular (TPDF, ±1 LSB) dither before rounding.
    pub dither: bool,
    /// Seed of the dither.
    pub seed: u64,
    pub non_periodic: NonPeriodic,
    /// Samples rendered past the end for the repeat-boundary check.
    pub guard_samples: usize,
}

impl Default for AwgOptions {
    fn default() -> Self {
        AwgOptions {
            max_samples: 16 << 20,
            bits: 16,
            dither: false,
            seed: 1,
            non_periodic: NonPeriodic::Refuse,
            guard_samples: 4096,
        }
    }
}

/// Result of the periodicity analysis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Periodicity {
    /// True when nothing in the scene varies with time.
    pub periodic: bool,
    /// Every reason the scene is not periodic.
    pub reasons: Vec<String>,
    /// Phase-code cycle (2×GRI, µs) of each chain.
    pub chain_cycles_us: Vec<(String, u64)>,
    /// LCM of the chain cycles (µs), before the sample-grid extension.
    pub lcm_us: Option<u128>,
    /// Segment period (µs) on the sample grid, and its length in samples.
    pub period_us: Option<u128>,
    pub period_samples: Option<u128>,
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Analyse whether the scenario's output is periodic and with which period.
pub fn analyse(sc: &Scenario) -> Periodicity {
    let mut reasons = vec![];
    if let Trajectory::Waypoints { .. } = sc.receiver {
        reasons.push("moving receiver (waypoint trajectory)".to_string());
    }
    let mut cycles = vec![];
    for c in &sc.chains {
        let ch = &c.chain;
        cycles.push((ch.name.clone(), 2 * ch.gri_us as u64));
        if c.ldc.is_some() {
            reasons.push(format!(
                "chain {}: LDC data channel (data change every group)",
                ch.name
            ));
        }
        for s in &ch.stations {
            if !s.faults.is_empty() {
                reasons.push(format!(
                    "chain {} station {}: time-windowed faults",
                    ch.name, s.id
                ));
            }
        }
    }
    let used: Vec<&str> = sc
        .chains
        .iter()
        .flat_map(|c| c.chain.stations.iter())
        .flat_map(|s| std::iter::once(s.time_ref.as_str()).chain(s.tx_clock.as_deref()))
        .collect();
    for m in &sc.time_refs {
        if !used.contains(&m.id.as_str()) {
            continue;
        }
        if m.drift_ns_per_day != 0.0 {
            reasons.push(format!("time reference {}: drift", m.id));
        }
        if m.wfm_adev_1s != 0.0 || m.rwfm_adev_1s != 0.0 {
            reasons.push(format!("time reference {}: clock noise", m.id));
        }
        if !m.steps.is_empty() {
            reasons.push(format!("time reference {}: phase steps", m.id));
        }
    }
    match &sc.propagation.ground {
        GroundConfig::SmoothEarth {
            temporal: Some(_), ..
        }
        | GroundConfig::Millington {
            temporal: Some(_), ..
        } => reasons.push("ground wave: temporal (seasonal/diurnal) variation".into()),
        _ => {}
    }
    if let Some(SkyConfig::Ionospheric(_)) = &sc.propagation.sky {
        reasons.push("ionospheric skywave (day/night, fading)".into());
    }
    let mut lcm: Option<u128> = None;
    for (_, c) in &cycles {
        let c = *c as u128;
        lcm = Some(match lcm {
            None => c,
            Some(l) => l / gcd(l, c) * c,
        });
    }
    let fs = sc.scene.params.fs_hz;
    let (mut period_us, mut period_samples) = (None, None);
    if let Some(l) = lcm {
        if fs.fract() == 0.0 && fs > 0.0 {
            let num = l * fs as u128; // samples × 10⁶
            let k = 1_000_000 / gcd(num, 1_000_000);
            period_us = Some(k * l);
            period_samples = Some(num * k / 1_000_000);
        }
    } else {
        reasons.push("no chains".into());
    }
    let period_s = period_us.map(|p| p as f64 * 1e-6);
    for (i, f) in sc.interference.iter().enumerate() {
        let r = match f {
            InterfConfig::Cw {
                freq_hz,
                from_s,
                to_s,
                ..
            } => {
                if from_s.is_some() || to_s.is_some() {
                    Some("windowed CW".to_string())
                } else if let Some(p) = period_s {
                    let cyc = freq_hz * p;
                    ((cyc - cyc.round()).abs() > 1e-6)
                        .then(|| format!("CW at {freq_hz} Hz not commensurate with {p} s"))
                } else {
                    None
                }
            }
            InterfConfig::Wgn { .. }
            | InterfConfig::Impulsive(_)
            | InterfConfig::Atmospheric(_)
            | InterfConfig::BandNoise { .. } => Some("random noise".into()),
            InterfConfig::Pulsed { .. } => Some("pulsed interference (not analysed)".into()),
        };
        if let Some(r) = r {
            reasons.push(format!("interference {i}: {r}"));
        }
    }
    Periodicity {
        periodic: reasons.is_empty(),
        reasons,
        chain_cycles_us: cycles,
        lcm_us: lcm,
        period_us,
        period_samples,
    }
}

/// Report of a compiled segment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AwgReport {
    /// The segment is one exact period of a periodic scene (loop it).
    pub periodic: bool,
    /// Why the scene is not periodic (empty when periodic).
    pub reasons: Vec<String>,
    pub samples: u64,
    pub channels: usize,
    pub sample_rate_hz: f64,
    pub duration_s: f64,
    pub bits: u32,
    /// Bytes per stored sample (1, 2 or 4) and total memory (all channels).
    pub bytes_per_sample: usize,
    pub memory_bytes: u64,
    /// Full-scale code, 2^(bits−1) − 1.
    pub full_scale_code: i64,
    pub dither: bool,
    /// Largest |quantised − exact| (full-scale units and LSB), clipping included.
    pub max_error_fs: f64,
    pub max_error_lsb: f64,
    pub rms_error_fs: f64,
    pub clipped: u64,
    pub peak_abs: f64,
    /// Largest |x[N + i] − x[i]| over the guard (exact model, full-scale units).
    pub boundary_max_diff_fs: f64,
    /// Jump at the wrap |x[0] − x[N−1]| compared with the largest step inside (both exact).
    pub wrap_step_fs: f64,
    pub max_inner_step_fs: f64,
    /// Seamless loop: periodic scene and boundary difference ≤ 1e-9.
    pub repeatable: bool,
}

/// A compiled AWG segment.
#[derive(Clone, Debug, PartialEq)]
pub struct AwgSegment {
    /// Signed DAC codes per channel.
    pub codes: Vec<Vec<i32>>,
    pub report: AwgReport,
}

impl AwgSegment {
    /// Channel `c` as normalised values (code / full-scale code), ±1.0 = full scale.
    pub fn normalised(&self, c: usize) -> Vec<f64> {
        let l = self.report.full_scale_code as f64;
        self.codes[c].iter().map(|&q| q as f64 / l).collect()
    }
    /// Channel `c` as 16-bit codes (rescaled when `bits` ≠ 16).
    pub fn codes_i16(&self, c: usize) -> Vec<i16> {
        let b = self.report.bits as i32;
        self.codes[c]
            .iter()
            .map(|&q| {
                if b >= 16 {
                    (q >> (b - 16)) as i16
                } else {
                    (q << (16 - b)) as i16
                }
            })
            .collect()
    }
}

/// A device that plays a compiled segment from its memory.
pub trait SegmentPlayer {
    /// Upload `segment` and arm playback: looped when `segment.report.periodic`, once
    /// otherwise; start at `at` (a PPS edge needs the device's trigger input on the PPS).
    fn play_segment(&mut self, segment: &AwgSegment, at: super::StartTime) -> Result<(), String>;
    /// Stop playback and switch the output off.
    fn stop_playback(&mut self) -> Result<(), String>;
}

/// Compile a scenario into an AWG segment (see the module documentation).
pub fn compile(sc: &Scenario, opt: &AwgOptions) -> Result<AwgSegment, String> {
    if !(2..=32).contains(&opt.bits) {
        return Err("bits must be in 2..=32".into());
    }
    let an = analyse(sc);
    let fs = sc.scene.params.fs_hz;
    let fit = an.period_samples.map(|n| n <= opt.max_samples as u128);
    let n: u64 = match (an.periodic, fit, opt.non_periodic) {
        (true, Some(true), _) => an.period_samples.unwrap_or(0) as u64,
        (_, _, NonPeriodic::Finite { samples }) => {
            if samples == 0 || samples > opt.max_samples {
                return Err(format!(
                    "finite segment of {samples} samples does not fit device memory ({})",
                    opt.max_samples
                ));
            }
            samples
        }
        (true, _, NonPeriodic::Refuse) => {
            return Err(format!(
                "periodic scene, but its period of {} samples ({} µs, LCM of 2×GRI {:?}) exceeds \
                 the device memory of {} samples",
                an.period_samples
                    .map(|v| v.to_string())
                    .unwrap_or("?".into()),
                an.period_us.map(|v| v.to_string()).unwrap_or("?".into()),
                an.chain_cycles_us,
                opt.max_samples
            ))
        }
        (false, _, NonPeriodic::Refuse) => {
            return Err(format!("scene is not periodic: {}", an.reasons.join("; ")))
        }
    };
    let mut reasons = an.reasons.clone();
    if an.periodic && fit != Some(true) {
        reasons.push("period exceeds device memory; finite segment rendered".into());
    }
    let periodic = reasons.is_empty();
    // render n + guard samples with the scene engine
    let guard = opt.guard_samples.max(1);
    let total = n + guard as u64;
    let mut s2 = sc.clone();
    s2.scene.params.duration_s = total as f64 / fs;
    let mut scene = s2.build()?;
    let mut sink = MemSink::default();
    let mut truth = vec![];
    scene.run(&mut sink, &mut truth)?;
    let exact = sink.channels;
    if exact.first().map(|c| c.len() as u64) != Some(total) {
        return Err("scene rendered an unexpected length".into());
    }
    let l = ((1u64 << (opt.bits - 1)) - 1) as f64;
    let mut rng = ChaCha8Rng::seed_from_u64(opt.seed);
    let (mut max_e, mut sum_e2, mut clipped, mut peak) = (0.0f64, 0.0f64, 0u64, 0.0f64);
    let (mut bmax, mut wrap, mut inner) = (0.0f64, 0.0f64, 0.0f64);
    let nu = n as usize;
    let mut codes = Vec::with_capacity(exact.len());
    for x in &exact {
        let mut q = Vec::with_capacity(nu);
        for &v in &x[..nu] {
            let d = if opt.dither {
                rng.gen::<f64>() - rng.gen::<f64>()
            } else {
                0.0
            };
            let c = (v * l + d).round();
            if c.abs() > l {
                clipped += 1;
            }
            let c = c.clamp(-l, l);
            let e = (c / l - v).abs();
            max_e = max_e.max(e);
            sum_e2 += e * e;
            peak = peak.max(v.abs());
            q.push(c as i32);
        }
        codes.push(q);
        for i in 0..guard.min(nu) {
            bmax = bmax.max((x[nu + i] - x[i]).abs());
        }
        wrap = wrap.max((x[0] - x[nu - 1]).abs());
        for w in x[..nu].windows(2) {
            inner = inner.max((w[1] - w[0]).abs());
        }
    }
    let channels = codes.len();
    let bps = match opt.bits {
        0..=8 => 1,
        9..=16 => 2,
        _ => 4,
    };
    let report = AwgReport {
        periodic,
        reasons,
        samples: n,
        channels,
        sample_rate_hz: fs,
        duration_s: n as f64 / fs,
        bits: opt.bits,
        bytes_per_sample: bps,
        memory_bytes: n * channels as u64 * bps as u64,
        full_scale_code: l as i64,
        dither: opt.dither,
        max_error_fs: max_e,
        max_error_lsb: max_e * l,
        rms_error_fs: (sum_e2 / (n as f64 * channels as f64)).sqrt(),
        clipped,
        peak_abs: peak,
        boundary_max_diff_fs: bmax,
        wrap_step_fs: wrap,
        max_inner_step_fs: inner,
        repeatable: periodic && bmax <= 1e-9,
    };
    Ok(AwgSegment { codes, report })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scen(extra: &str, gri: &str) -> Scenario {
        let src = format!(
            r#"
[scene]
start_utc = "2026-11-18T00:00:00.123Z"
duration_s = 1.0
fs_hz = 1000000
seed = 1
output_scale_uv_m = 2000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 0.0
[[outputs]]
type = "e_field"
{extra}
[[chains]]
name = "c"
gri_us = {gri}
[[chains.stations]]
id = "M"
role = "master"
position = {{ lat_deg = 55.5, lon_deg = 0.0 }}
[[chains.stations]]
id = "X"
role = "secondary"
emission_delay_us = 13000
position = {{ lat_deg = 54.0, lon_deg = 1.0 }}
"#
        );
        Scenario::from_toml(&src).unwrap()
    }

    #[test]
    fn periodic_segment_matches_scene_and_loops_seamlessly() {
        let sc = scen("", "49900");
        let a = analyse(&sc);
        assert!(a.periodic, "{:?}", a.reasons);
        assert_eq!(a.period_samples, Some(99_800));
        let seg = compile(&sc, &AwgOptions::default()).unwrap();
        let r = &seg.report;
        assert!(r.periodic && r.repeatable, "{r:?}");
        assert_eq!(r.samples, 99_800);
        assert_eq!(r.memory_bytes, 99_800 * 2);
        assert!(r.boundary_max_diff_fs <= 1e-9);
        assert!(r.wrap_step_fs <= r.max_inner_step_fs);
        assert!(
            r.max_error_lsb <= 0.5 + 1e-9,
            "{} (peak {})",
            r.max_error_lsb,
            r.peak_abs
        );
        assert!(r.peak_abs > 0.05 && r.clipped == 0);
        // the segment equals the scene's own output (to quantisation), two periods long
        let mut s2 = sc.clone();
        s2.scene.params.duration_s = 2.0 * 0.0998;
        let mut sink = MemSink::default();
        s2.build().unwrap().run(&mut sink, &mut vec![]).unwrap();
        let q = seg.normalised(0);
        for (i, v) in sink.channels[0].iter().enumerate() {
            assert!(
                (q[i % 99_800] - v).abs() <= 0.5 / 32767.0 + 1e-12,
                "sample {i}"
            );
        }
        // dither: error up to 1.5 LSB, still repeatable
        let mut o = AwgOptions {
            dither: true,
            bits: 12,
            ..Default::default()
        };
        let d = compile(&sc, &o).unwrap();
        assert!(d.report.max_error_lsb <= 1.5 + 1e-9 && d.report.max_error_lsb > 0.5);
        assert!(d.report.repeatable);
        assert_eq!(d.codes_i16(0)[100], (d.codes[0][100] << 4) as i16);
        // too little memory: refused with the period
        o.max_samples = 50_000;
        let e = compile(&sc, &o).unwrap_err();
        assert!(e.contains("99800") && e.contains("50000"), "{e}");
    }

    #[test]
    fn period_is_lcm_of_chain_cycles_on_the_sample_grid() {
        let mut sc = scen("", "99300");
        let mut c2 = sc.chains[0].clone();
        c2.chain.name = "d".into();
        c2.chain.gri_us = 67310;
        sc.chains.push(c2);
        let a = analyse(&sc);
        // lcm(198600, 134620) µs
        let l = 198_600u128 / gcd(198_600, 134_620) * 134_620;
        assert_eq!(a.lcm_us, Some(l));
        assert_eq!(a.period_samples, Some(l)); // 1 MS/s: 1 sample per µs
                                               // a sample rate off the µs grid extends the period
        sc.scene.params.fs_hz = 1_000_003.0;
        sc.chains.truncate(1);
        let a = analyse(&sc);
        let p = a.period_us.unwrap();
        assert_eq!(p, 198_600 * 1_000_000 / gcd(198_600 * 1_000_003, 1_000_000));
        assert_eq!((p * 1_000_003) % 1_000_000, 0);
        assert!(compile(&sc, &AwgOptions::default()).is_err()); // far beyond memory
    }

    #[test]
    fn non_periodic_scenes_are_refused_or_rendered_finite() {
        let extra = r#"
[[time_refs]]
id = "UTC(X)"
drift_ns_per_day = 5.0
[[interference]]
type = "wgn"
rms_uv_m_20khz = 10.0
[[interference]]
type = "cw"
freq_hz = 100005.0
rms_uv_m = 1.0
"#;
        let mut sc = scen(extra, "49900");
        sc.chains[0].chain.stations[0].time_ref = "UTC(X)".into();
        sc.receiver = Trajectory::Waypoints {
            points: vec![
                crate::traj::Waypoint {
                    t_s: 0.0,
                    lat_deg: 55.0,
                    lon_deg: 0.0,
                    h_m: 0.0,
                    heading_deg: None,
                },
                crate::traj::Waypoint {
                    t_s: 10.0,
                    lat_deg: 55.01,
                    lon_deg: 0.0,
                    h_m: 0.0,
                    heading_deg: None,
                },
            ],
        };
        let a = analyse(&sc);
        assert!(!a.periodic);
        let all = a.reasons.join("|");
        for k in ["moving receiver", "drift", "random noise", "CW at 100005"] {
            assert!(all.contains(k), "{k} missing in {all}");
        }
        let e = compile(&sc, &AwgOptions::default()).unwrap_err();
        assert!(e.contains("not periodic") && e.contains("moving receiver"));
        let seg = compile(
            &sc,
            &AwgOptions {
                non_periodic: NonPeriodic::Finite { samples: 20_000 },
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!seg.report.periodic && !seg.report.repeatable);
        assert_eq!(seg.report.samples, 20_000);
        assert!(seg.report.boundary_max_diff_fs > 0.0);
        // a commensurate CW alone keeps the scene periodic
        let sc = scen(
            "[[interference]]\ntype = \"cw\"\nfreq_hz = 100010.0204\nrms_uv_m = 1.0\n",
            "49900",
        );
        assert!(!analyse(&sc).periodic);
        let sc = scen(
            "[[interference]]\ntype = \"cw\"\nfreq_hz = 120000.0\nrms_uv_m = 1.0\n",
            "49900",
        );
        let a = analyse(&sc);
        assert!(a.periodic, "{:?}", a.reasons);
    }
}
