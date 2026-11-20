// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! MF R-Mode: ranging on maritime DGNSS beacons (283.5–315 kHz, 500 Hz channels), the
//! terrestrial backup being deployed in the Baltic (research 01 gap 11, 02 gap 3: no
//! simulator or validation suite on the market).
//!
//! Signal (as published for MF R-Mode; IALA G1187 Ed. 1.0 is the specification, not
//! reproduced here): the legacy MSK-modulated DGNSS data at the carrier `fc`, plus two
//! low-level continuous-wave ranging tones at `fc ± Δ` (Δ = 225 Hz in most references; some
//! implementations use 250 Hz, so it is a parameter), phase-locked to UTC at each transmitter.
//! Sources: Grundhöfer & Gewies 2018; Johnson et al. (ACCSEAS feasibility study, IALA);
//! arXiv:2108.09496 (R-Mode simulator); DLR prototype MF R-Mode receiver.
//!
//! * **Synthesis** at complex baseband per channel (stations sit in separate 500 Hz channels;
//!   the receiver processes each channel alone): MSK data, both tones, white noise, optional
//!   night skywave (excess delay, relative amplitude, reflection phase).
//! * **Propagation**: the crate's smooth-earth groundwave evaluated **at the beacon's
//!   frequency** (`prop::smooth_earth::GroundCurve` at `fc`), with the LORAN atmospheric
//!   index; the same physics as eLoran at 300 kHz instead of 100 kHz.
//! * **Receiver**: tone phases φ± = −2π(fc ± Δ)τ by correlation; the beat φ+ − φ− gives an
//!   unambiguous coarse delay (period 1/2Δ ≈ 2.2 ms ≈ 666 km); the upper tone gives the fine
//!   delay (period ≈ 3.3 µs ≈ 1 km); the fine ambiguity is resolved from the coarse one. A
//!   least-squares fix (lat, lon, clock bias) uses the receiver's own propagation model.
//!
//! Evidence: **MODELLED**. The day/night behaviour reproduces the published direction (night
//! skywave degrades accuracy; Baltic testbed: 15.1 m day vs 55.3 m night, 95 %); the numbers
//! here depend on the stated tone level, noise and skywave and are not fitted to the testbed.
//! Not a G1187 conformance test.

pub mod perf;
pub mod vdes;

use crate::geo::{inverse, Geo};
use crate::prop::smooth_earth::{Ground, GroundCurve, SEAWATER};
use crate::prop::C;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

const N_ATM: f64 = 1.000_338;
const K_FACTOR: f64 = 1.24;

/// One R-Mode beacon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beacon {
    pub id: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Carrier (Hz), 283.5–315 kHz.
    pub carrier_hz: f64,
    /// Synchronisation error of this transmitter against UTC (ns).
    #[serde(default)]
    pub sync_error_ns: f64,
    /// Received tone amplitude relative to the noise RMS in the channel per sample
    /// (simple SNR control for the bench).
    #[serde(default = "d_amp")]
    pub tone_amp: f64,
    /// Path ground (σ, ε_r); seawater by default.
    #[serde(default)]
    pub ground_sigma: Option<f64>,
    #[serde(default)]
    pub ground_eps_r: Option<f64>,
}

fn d_amp() -> f64 {
    1.0
}

/// Night skywave on all paths.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skywave {
    /// Excess delay over the groundwave (µs).
    pub excess_delay_us: f64,
    /// Skywave amplitude relative to the groundwave.
    pub rel_amp: f64,
    /// Reflection phase (rad).
    pub phase_rad: f64,
}

/// Bench configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RModeConfig {
    pub beacons: Vec<Beacon>,
    pub rx_lat_deg: f64,
    pub rx_lon_deg: f64,
    /// Receiver clock bias (ns).
    pub rx_clock_bias_ns: f64,
    /// Tone offset Δ (Hz).
    pub tone_offset_hz: f64,
    /// Relative amplitude of the MSK data component (to the tone amplitude).
    pub msk_rel_amp: f64,
    pub msk_bit_rate: f64,
    /// Baseband sample rate per channel (Hz).
    pub fs_hz: f64,
    /// Integration time per measurement (s).
    pub integration_s: f64,
    pub noise_rms: f64,
    pub skywave: Option<Skywave>,
    /// Ground the receiver assumes for its propagation correction (σ, ε_r): seawater.
    pub rx_model_sigma: f64,
    pub rx_model_eps_r: f64,
    /// Approximate receiver position used to resolve the fine tone cycles (the last GNSS fix,
    /// or dead reckoning), as R-Mode receivers initialise their cycle count. `None`: resolve
    /// from the tone beat alone (needs a high tone SNR: the beat period is 1/2Δ).
    pub prior_lat_deg: Option<f64>,
    pub prior_lon_deg: Option<f64>,
    /// Receiver clock bias at initialisation (ns), from the same GNSS fix.
    pub prior_clock_bias_ns: f64,
    pub seed: u64,
}

impl Default for RModeConfig {
    fn default() -> Self {
        RModeConfig {
            beacons: vec![],
            rx_lat_deg: 0.0,
            rx_lon_deg: 0.0,
            rx_clock_bias_ns: 0.0,
            tone_offset_hz: 225.0,
            msk_rel_amp: 3.0,
            msk_bit_rate: 100.0,
            fs_hz: 2000.0,
            integration_s: 10.0,
            noise_rms: 1.0,
            skywave: None,
            rx_model_sigma: 5.0,
            rx_model_eps_r: 80.0,
            prior_lat_deg: None,
            prior_lon_deg: None,
            prior_clock_bias_ns: 0.0,
            seed: 1,
        }
    }
}

/// One beacon measured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BeaconMeas {
    pub id: String,
    pub distance_m: f64,
    /// True propagation delay (s), groundwave, without receiver clock.
    pub true_delay_s: f64,
    /// Measured pseudodelay (s) after ambiguity resolution (includes the receiver clock).
    pub pseudo_delay_s: f64,
    /// Coarse delay from the beat (s) and fine delay (s).
    pub coarse_s: f64,
    pub fine_s: f64,
    /// Fine-cycle ambiguity resolved consistently with the truth.
    pub ambiguity_ok: bool,
}

/// Bench result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RModeReport {
    pub beacons: Vec<BeaconMeas>,
    pub fix_lat_deg: f64,
    pub fix_lon_deg: f64,
    pub clock_bias_ns: f64,
    pub horizontal_error_m: f64,
    pub residual_rms_m: f64,
    pub evidence: String,
}

fn ground_of(b: &Beacon) -> Ground {
    match (b.ground_sigma, b.ground_eps_r) {
        (Some(s), Some(e)) => Ground { sigma: s, eps_r: e },
        (Some(s), None) => Ground {
            sigma: s,
            eps_r: 15.0,
        },
        _ => SEAWATER,
    }
}

/// Groundwave delay (s) at `fc` over a homogeneous ground: geometric time with the
/// atmospheric index plus the smooth-earth phase lag.
pub fn groundwave_delay_s(d_m: f64, fc: f64, g: Ground) -> f64 {
    let curve = GroundCurve::new(g, fc, K_FACTOR, N_ATM, (d_m * 1.05).max(10_000.0), 400);
    N_ATM * d_m / C + curve.at(d_m).phase_delay_s
}

fn gauss(r: &mut ChaCha8Rng) -> f64 {
    let (u1, u2): (f64, f64) = (r.gen::<f64>().max(1e-300), r.gen());
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

/// Synthesize one channel's complex baseband and measure the tone phases → (φ+, φ−).
fn channel_phases(cfg: &RModeConfig, b: &Beacon, tau: f64, rng: &mut ChaCha8Rng) -> (f64, f64) {
    let fs = cfg.fs_hz;
    let n = (cfg.integration_s * fs) as usize;
    let d = cfg.tone_offset_hz;
    let fc = b.carrier_hz;
    // Paths: groundwave (1, τ) and optional skywave.
    let mut paths = vec![(1.0, tau, 0.0)];
    if let Some(s) = &cfg.skywave {
        paths.push((s.rel_amp, tau + s.excess_delay_us * 1e-6, s.phase_rad));
    }
    let bits: Vec<f64> = (0..((cfg.integration_s * cfg.msk_bit_rate) as usize + 4))
        .map(|_| if rng.gen::<bool>() { 1.0 } else { -1.0 })
        .collect();
    let (mut up_re, mut up_im, mut lo_re, mut lo_im) = (0.0, 0.0, 0.0, 0.0);
    for k in 0..n {
        let t = k as f64 / fs;
        let (mut re, mut im) = (0.0, 0.0);
        for &(a, tp, ph0) in &paths {
            let te = t - tp;
            let carrier = -2.0 * PI * fc * tp + ph0;
            // tones
            for sgn in [1.0, -1.0] {
                let ph = 2.0 * PI * sgn * d * te + carrier - 2.0 * PI * sgn * d * 0.0;
                re += a * b.tone_amp * ph.cos();
                im += a * b.tone_amp * ph.sin();
            }
            // MSK: frequency ±br/4 per bit, continuous phase (approximate by bit-wise phase)
            let bi = ((te * cfg.msk_bit_rate).floor().max(0.0) as usize).min(bits.len() - 1);
            let ph = 2.0 * PI * bits[bi] * cfg.msk_bit_rate / 4.0 * te + carrier;
            re += a * cfg.msk_rel_amp * b.tone_amp * ph.cos();
            im += a * cfg.msk_rel_amp * b.tone_amp * ph.sin();
        }
        re += cfg.noise_rms * gauss(rng) / std::f64::consts::SQRT_2;
        im += cfg.noise_rms * gauss(rng) / std::f64::consts::SQRT_2;
        // Correlate with the tone references exp(∓j2πΔt).
        let (cu, su) = (2.0 * PI * d * t).sin_cos();
        // upper: multiply by exp(−j2πΔt)
        up_re += re * su + im * cu;
        up_im += im * su - re * cu;
        // lower: multiply by exp(+j2πΔt)
        lo_re += re * su - im * cu;
        lo_im += im * su + re * cu;
    }
    (up_im.atan2(up_re), lo_im.atan2(lo_re))
}

fn wrap(x: f64, period: f64) -> f64 {
    x - period * (x / period).round()
}

/// Run the bench: synthesize, measure, resolve, fix.
pub fn run(cfg: &RModeConfig) -> Result<RModeReport, String> {
    if cfg.beacons.len() < 3 {
        return Err("an R-Mode fix needs at least three beacons".into());
    }
    let mut rng = ChaCha8Rng::seed_from_u64(cfg.seed);
    let rx = Geo::new(cfg.rx_lat_deg, cfg.rx_lon_deg);
    let d = cfg.tone_offset_hz;
    let model = Ground {
        sigma: cfg.rx_model_sigma,
        eps_r: cfg.rx_model_eps_r,
    };
    let mut raw = vec![];
    for b in &cfg.beacons {
        let dist = inverse(Geo::new(b.lat_deg, b.lon_deg), rx).distance_m;
        let true_delay = groundwave_delay_s(dist, b.carrier_hz, ground_of(b));
        let tau = true_delay + b.sync_error_ns * 1e-9 + cfg.rx_clock_bias_ns * 1e-9;
        let (pu, pl) = channel_phases(cfg, b, tau, &mut rng);
        let fu = b.carrier_hz + d;
        // Coarse from the beat: φ+ − φ− = −2π(2Δ)τ + noise.
        let tb = 1.0 / (2.0 * d);
        let coarse = (-(wrap(pu - pl, 2.0 * PI)) / (2.0 * PI * 2.0 * d)).rem_euclid(tb);
        // Fine from the upper tone: φ+ = −2π fu τ (mod 2π) → τ mod 1/fu.
        let tf = 1.0 / fu;
        let fine = (-pu / (2.0 * PI * fu)).rem_euclid(tf);
        raw.push((b, dist, true_delay, tau, coarse, fine, tf));
    }
    // Resolve the fine cycles: from the prior (position + clock, as at initialisation from
    // GNSS) when given, otherwise from the beat.
    let prior = match (cfg.prior_lat_deg, cfg.prior_lon_deg) {
        (Some(a), Some(o)) => Some(Geo::new(a, o)),
        _ => None,
    };
    let mut meas = vec![];
    for &(b, dist, true_delay, _tau, coarse, fine, tf) in raw.iter() {
        let target = match prior {
            Some(p) => {
                groundwave_delay_s(
                    inverse(Geo::new(b.lat_deg, b.lon_deg), p).distance_m,
                    b.carrier_hz,
                    model,
                ) + cfg.prior_clock_bias_ns * 1e-9
            }
            None => coarse,
        };
        let pseudo = fine + ((target - fine) / tf).round() * tf;
        meas.push(BeaconMeas {
            id: b.id.clone(),
            distance_m: dist,
            true_delay_s: true_delay,
            pseudo_delay_s: pseudo,
            coarse_s: coarse,
            fine_s: fine,
            ambiguity_ok: false,
        });
    }
    for (m, r) in meas.iter_mut().zip(&raw) {
        m.ambiguity_ok = (m.pseudo_delay_s - r.3).abs() < r.6 / 2.0;
    }
    // Least squares (lat, lon, bias) with the receiver's propagation model.
    let pred = |p: Geo, bias: f64, b: &Beacon| {
        groundwave_delay_s(
            inverse(Geo::new(b.lat_deg, b.lon_deg), p).distance_m,
            b.carrier_hz,
            model,
        ) + bias
    };
    let mut lat = cfg.beacons.iter().map(|b| b.lat_deg).sum::<f64>() / cfg.beacons.len() as f64;
    let mut lon = cfg.beacons.iter().map(|b| b.lon_deg).sum::<f64>() / cfg.beacons.len() as f64;
    let mut bias = meas[0].pseudo_delay_s - pred(Geo::new(lat, lon), 0.0, &cfg.beacons[0]);
    for _ in 0..12 {
        let p = Geo::new(lat, lon);
        let mut rows = vec![];
        let mut res = vec![];
        for (b, m) in cfg.beacons.iter().zip(&meas) {
            let r0 = pred(p, bias, b);
            let dl = 1e-5;
            let rl = pred(Geo::new(lat + dl, lon), bias, b);
            let rn = pred(Geo::new(lat, lon + dl), bias, b);
            rows.push([(rl - r0) / dl, (rn - r0) / dl, 1.0]);
            res.push(m.pseudo_delay_s - r0);
        }
        // Normal equations 3×3.
        let mut a = [[0.0f64; 3]; 3];
        let mut g = [0.0f64; 3];
        for (r, e) in rows.iter().zip(&res) {
            for i in 0..3 {
                g[i] += r[i] * e;
                for j in 0..3 {
                    a[i][j] += r[i] * r[j];
                }
            }
        }
        let x = solve3(a, g).ok_or("singular geometry")?;
        lat += x[0];
        lon += x[1];
        bias += x[2];
        if x[0].abs() < 1e-10 && x[1].abs() < 1e-10 {
            break;
        }
    }
    let fix = Geo::new(lat, lon);
    let res_rms = (cfg
        .beacons
        .iter()
        .zip(&meas)
        .map(|(b, m)| ((m.pseudo_delay_s - pred(fix, bias, b)) * C).powi(2))
        .sum::<f64>()
        / meas.len() as f64)
        .sqrt();
    Ok(RModeReport {
        horizontal_error_m: inverse(fix, rx).distance_m,
        fix_lat_deg: lat,
        fix_lon_deg: lon,
        clock_bias_ns: bias * 1e9,
        residual_rms_m: res_rms,
        beacons: meas,
        evidence: "MODELLED: MF R-Mode tones per published descriptions (IALA G1187 is the specification); smooth-earth groundwave at the beacon frequency".into(),
    })
}

fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    if d.abs() < 1e-300 {
        return None;
    }
    let mut x = [0.0; 3];
    for (i, xi) in x.iter_mut().enumerate() {
        let mut m = a;
        for r in 0..3 {
            m[r][i] = b[r];
        }
        *xi = det(m) / d;
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Baltic-like layout: four beacons around a receiver at sea (illustrative positions).
    fn cfg() -> RModeConfig {
        let bk = |id: &str, lat: f64, lon: f64, f: f64| Beacon {
            id: id.into(),
            lat_deg: lat,
            lon_deg: lon,
            carrier_hz: f,
            sync_error_ns: 0.0,
            tone_amp: 0.3,
            ground_sigma: None,
            ground_eps_r: None,
        };
        RModeConfig {
            beacons: vec![
                bk("A", 55.6, 12.6, 296.5e3),
                bk("B", 54.2, 12.1, 298.5e3),
                bk("C", 54.7, 14.9, 303.5e3),
                bk("D", 56.1, 15.6, 309.5e3),
            ],
            rx_lat_deg: 55.0,
            rx_lon_deg: 13.8,
            rx_clock_bias_ns: 1234.0,
            // Last GNSS fix about 200 m from the true position.
            prior_lat_deg: Some(55.0015),
            prior_lon_deg: Some(13.8015),
            prior_clock_bias_ns: 1234.0 + 80.0,
            ..Default::default()
        }
    }

    #[test]
    fn groundwave_delay_at_300_khz_exceeds_free_space_by_the_secondary_factor() {
        let d = 150_000.0;
        let t = groundwave_delay_s(d, 300e3, SEAWATER);
        let extra_ns = (t - N_ATM * d / C) * 1e9;
        // Seawater secondary factor at 150 km is of order 0.1–1 µs at LF/MF.
        assert!(extra_ns > 50.0 && extra_ns < 2000.0, "{extra_ns} ns");
        // Poorer ground → larger phase lag.
        let land = groundwave_delay_s(
            d,
            300e3,
            Ground {
                sigma: 0.005,
                eps_r: 15.0,
            },
        );
        assert!(land > t);
    }

    #[test]
    fn day_fix_resolves_ambiguities_and_is_accurate() {
        let r = run(&cfg()).unwrap();
        assert!(r.beacons.iter().all(|b| b.ambiguity_ok), "{:?}", r.beacons);
        assert!(
            r.horizontal_error_m < 10.0,
            "error {} m",
            r.horizontal_error_m
        );
        assert!(
            (r.clock_bias_ns - 1234.0).abs() < 30.0,
            "bias {}",
            r.clock_bias_ns
        );
    }

    #[test]
    fn beat_only_resolution_works_at_high_tone_snr() {
        let mut c = cfg();
        c.prior_lat_deg = None;
        c.prior_lon_deg = None;
        for b in c.beacons.iter_mut() {
            b.tone_amp = 30.0;
        }
        c.integration_s = 20.0;
        let r = run(&c).unwrap();
        assert!(r.beacons.iter().all(|b| b.ambiguity_ok), "{:?}", r.beacons);
        assert!(r.horizontal_error_m < 5.0, "{}", r.horizontal_error_m);
        // At the default tone level the beat alone cannot resolve the 3.3 µs cycles.
        let mut lo = cfg();
        lo.prior_lat_deg = None;
        lo.prior_lon_deg = None;
        let r = run(&lo).unwrap();
        assert!(r.beacons.iter().any(|b| !b.ambiguity_ok));
    }

    #[test]
    fn night_skywave_and_wrong_ground_degrade_the_fix() {
        let day = run(&cfg()).unwrap().horizontal_error_m;
        let mut n = cfg();
        n.skywave = Some(Skywave {
            excess_delay_us: 60.0,
            rel_amp: 0.5,
            phase_rad: 1.0,
        });
        let night = run(&n).unwrap();
        assert!(
            night.horizontal_error_m > 3.0 * day.max(1.0),
            "day {day} m night {} m",
            night.horizontal_error_m
        );
        // Land paths the receiver models as sea: an ASF-like error.
        let mut l = cfg();
        l.beacons[0].ground_sigma = Some(0.01);
        l.beacons[1].ground_sigma = Some(0.01);
        let land = run(&l).unwrap();
        assert!(
            land.horizontal_error_m > day + 20.0,
            "day {day} land {}",
            land.horizontal_error_m
        );
        // A 30 ns sync error at one transmitter moves the fix by metres.
        let mut s = cfg();
        s.beacons[2].sync_error_ns = 30.0;
        let sync = run(&s).unwrap();
        assert!(sync.horizontal_error_m > 2.0, "{}", sync.horizontal_error_m);
    }
}
