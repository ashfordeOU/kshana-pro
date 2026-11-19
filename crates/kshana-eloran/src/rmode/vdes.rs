// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! VDES R-Mode (VHF): ranging on UTC-synchronised VDES terrestrial bursts. The signal is
//! specified by IALA (G1139 / G1158 per the research notes); **the burst parameters here are
//! configurable placeholders pending confirmation against those documents**: symbol rate,
//! synchronisation-sequence length, RMS bandwidth.
//!
//! Physics modelled (independent of the burst details):
//! * **Radio horizon** with the 4/3-earth standard atmosphere: d = 4.12 (√h₁ + √h₂) km (h in m).
//! * **Two-ray sea multipath**: direct path plus a surface reflection with reflection
//!   coefficient −ρ (vertical polarisation at grazing incidence ≈ −1 for calm sea, smaller for
//!   rough sea); the correlation peak of a band-limited sequence is biased by the reflected
//!   copy when the excess delay is below the resolution 1/B.
//! * **Timing noise**: Cramér–Rao bound for a known sequence, σ_τ = 1 / (2π B_rms √(2 · SNR_seq)),
//!   SNR_seq = per-symbol SNR × symbols.
//! * **Fix**: least squares on pseudoranges with a receiver clock bias.
//!
//! Evidence: **MODELLED**; not a G1139/G1158 conformance test.

use crate::geo::{inverse, Geo};
use crate::prop::C;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// One VDES R-Mode station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VdesStation {
    pub id: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Antenna height above sea (m).
    pub height_m: f64,
    /// Per-symbol SNR at 50 km (dB); scales as 20 log10 of distance (two-ray far region ~ 40 log;
    /// kept simple and stated).
    pub snr_db_at_50km: f64,
    #[serde(default)]
    pub sync_error_ns: f64,
}

/// Burst parameters (placeholders pending G1139/G1158).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VdesConfig {
    pub stations: Vec<VdesStation>,
    pub rx_lat_deg: f64,
    pub rx_lon_deg: f64,
    pub rx_height_m: f64,
    pub rx_clock_bias_ns: f64,
    /// RMS bandwidth of the synchronisation sequence (Hz).
    pub rms_bandwidth_hz: f64,
    /// Symbols in the synchronisation sequence.
    pub sync_symbols: f64,
    /// Bursts averaged per measurement.
    pub bursts: f64,
    /// Sea reflection magnitude ρ (0 = no multipath).
    pub sea_rho: f64,
    pub seed: u64,
}

impl Default for VdesConfig {
    fn default() -> Self {
        VdesConfig {
            stations: vec![],
            rx_lat_deg: 0.0,
            rx_lon_deg: 0.0,
            rx_height_m: 15.0,
            rx_clock_bias_ns: 0.0,
            rms_bandwidth_hz: 30_000.0,
            sync_symbols: 27.0,
            bursts: 60.0,
            sea_rho: 0.9,
            seed: 1,
        }
    }
}

/// Radio horizon (m) between antennas at h1, h2 (m), 4/3 earth.
pub fn radio_horizon_m(h1: f64, h2: f64) -> f64 {
    4120.0 * (h1.sqrt() + h2.sqrt())
}

/// Excess delay (s) of the sea reflection over the direct path (flat-earth image geometry).
pub fn reflection_excess_s(d_m: f64, h1: f64, h2: f64) -> f64 {
    let direct = (d_m * d_m + (h1 - h2).powi(2)).sqrt();
    let refl = (d_m * d_m + (h1 + h2).powi(2)).sqrt();
    (refl - direct) / C
}

/// Bias (s) of a correlation-peak TOA from a reflected copy of relative amplitude `a` (signed)
/// at excess delay `dt`, for a sequence with RMS bandwidth `b` (Gaussian-shaped correlation):
/// the peak of R(t) + a·R(t − dt) moves by ≈ a·dt·e^{−(2π b dt)²/2} / (1 + a·e^{−(2π b dt)²/2}).
pub fn multipath_bias_s(a: f64, dt: f64, b: f64) -> f64 {
    let g = (-(2.0 * std::f64::consts::PI * b * dt).powi(2) / 2.0).exp();
    a * dt * g / (1.0 + a * g)
}

/// Timing σ (s) from the Cramér–Rao bound.
pub fn crb_sigma_s(b_rms: f64, snr_lin_total: f64) -> f64 {
    1.0 / (2.0 * std::f64::consts::PI * b_rms * (2.0 * snr_lin_total).sqrt())
}

/// Per-station result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VdesMeas {
    pub id: String,
    pub distance_m: f64,
    pub in_horizon: bool,
    pub sigma_ns: f64,
    pub multipath_bias_ns: f64,
    pub pseudorange_m: f64,
}

/// Fix result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VdesReport {
    pub stations: Vec<VdesMeas>,
    pub used: usize,
    pub horizontal_error_m: Option<f64>,
    pub fix_lat_deg: Option<f64>,
    pub fix_lon_deg: Option<f64>,
    pub evidence: String,
}

/// Simulate measurements and fix.
pub fn run(cfg: &VdesConfig) -> VdesReport {
    let mut rng = ChaCha8Rng::seed_from_u64(cfg.seed);
    let rx = Geo::new(cfg.rx_lat_deg, cfg.rx_lon_deg);
    let mut meas = vec![];
    for s in &cfg.stations {
        let d = inverse(Geo::new(s.lat_deg, s.lon_deg), rx).distance_m;
        let inh = d <= radio_horizon_m(s.height_m, cfg.rx_height_m);
        let snr_db = s.snr_db_at_50km - 20.0 * (d / 50_000.0).max(1e-3).log10();
        let snr = 10f64.powf(snr_db / 10.0) * cfg.sync_symbols * cfg.bursts;
        let sig = crb_sigma_s(cfg.rms_bandwidth_hz, snr);
        let bias = multipath_bias_s(
            -cfg.sea_rho,
            reflection_excess_s(d, s.height_m, cfg.rx_height_m),
            cfg.rms_bandwidth_hz,
        );
        let (u1, u2): (f64, f64) = (rng.gen::<f64>().max(1e-300), rng.gen());
        let n = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let pr = d + C * (bias + sig * n + (s.sync_error_ns + cfg.rx_clock_bias_ns) * 1e-9);
        meas.push(VdesMeas {
            id: s.id.clone(),
            distance_m: d,
            in_horizon: inh,
            sigma_ns: sig * 1e9,
            multipath_bias_ns: bias * 1e9,
            pseudorange_m: pr,
        });
    }
    let used: Vec<(&VdesStation, &VdesMeas)> = cfg
        .stations
        .iter()
        .zip(&meas)
        .filter(|(_, m)| m.in_horizon)
        .collect();
    let (mut lat, mut lon, mut b) = (0.0, 0.0, 0.0);
    let ok = used.len() >= 3 && {
        lat = used.iter().map(|(s, _)| s.lat_deg).sum::<f64>() / used.len() as f64;
        lon = used.iter().map(|(s, _)| s.lon_deg).sum::<f64>() / used.len() as f64;
        let mut conv = false;
        for _ in 0..20 {
            let mut a = [[0.0f64; 3]; 3];
            let mut g = [0.0f64; 3];
            for (s, m) in &used {
                let st = Geo::new(s.lat_deg, s.lon_deg);
                let r0 = inverse(st, Geo::new(lat, lon)).distance_m + b;
                let dl = 1e-5;
                let rl = inverse(st, Geo::new(lat + dl, lon)).distance_m + b;
                let rn = inverse(st, Geo::new(lat, lon + dl)).distance_m + b;
                let h = [(rl - r0) / dl, (rn - r0) / dl, 1.0];
                let e = m.pseudorange_m - r0;
                for i in 0..3 {
                    g[i] += h[i] * e;
                    for j in 0..3 {
                        a[i][j] += h[i] * h[j];
                    }
                }
            }
            let Some(x) = solve3(a, g) else { break };
            lat += x[0];
            lon += x[1];
            b += x[2];
            if x[0].abs() < 1e-11 && x[1].abs() < 1e-11 {
                conv = true;
                break;
            }
        }
        conv
    };
    VdesReport {
        used: used.len(),
        horizontal_error_m: ok.then(|| inverse(Geo::new(lat, lon), rx).distance_m),
        fix_lat_deg: ok.then_some(lat),
        fix_lon_deg: ok.then_some(lon),
        stations: meas,
        evidence: "MODELLED: radio horizon, two-ray sea multipath, Cramér–Rao timing; burst parameters are placeholders pending IALA G1139/G1158".into(),
    }
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

    fn cfg() -> VdesConfig {
        let s = |id: &str, la: f64, lo: f64| VdesStation {
            id: id.into(),
            lat_deg: la,
            lon_deg: lo,
            height_m: 60.0,
            snr_db_at_50km: 10.0,
            sync_error_ns: 0.0,
        };
        VdesConfig {
            stations: vec![
                s("A", 54.4, 11.0),
                s("B", 54.0, 11.9),
                s("C", 54.5, 11.9),
                s("D", 54.25, 11.0),
            ],
            rx_lat_deg: 54.3,
            rx_lon_deg: 11.5,
            ..Default::default()
        }
    }

    #[test]
    fn horizon_multipath_and_crb_closed_forms() {
        // 60 m and 15 m antennas: 4.12(√60 + √15) km ≈ 47.9 km.
        assert!((radio_horizon_m(60.0, 15.0) / 1000.0 - 47.87).abs() < 0.05);
        // Reflection excess ≈ 2 h1 h2 / (c d) at long range.
        let dt = reflection_excess_s(30_000.0, 60.0, 15.0);
        assert!((dt - 2.0 * 60.0 * 15.0 / (C * 30_000.0)).abs() / dt < 1e-3);
        // Unresolved reflection (dt ≪ 1/B) with a = −0.9 pulls the peak early by ≈ a·dt/(1+a).
        let b = multipath_bias_s(-0.9, 1e-9, 30_000.0);
        assert!((b - (-0.9e-9 / 0.1)).abs() < 1e-11, "{b}");
        // CRB halves when SNR quadruples.
        assert!((crb_sigma_s(3e4, 400.0) / crb_sigma_s(3e4, 1600.0) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn fix_inside_coverage_multipath_bias_and_horizon() {
        let calm = run(&VdesConfig {
            sea_rho: 0.0,
            ..cfg()
        });
        assert_eq!(calm.used, 4);
        let e0 = calm.horizontal_error_m.unwrap();
        assert!(e0 < 30.0, "no multipath: {e0} m");
        // Sea reflection: every bias is early (negative) and follows the closed form.
        let sea = run(&cfg());
        for (m, s) in sea.stations.iter().zip(&cfg().stations) {
            let want = multipath_bias_s(
                -0.9,
                reflection_excess_s(m.distance_m, s.height_m, 15.0),
                30_000.0,
            ) * 1e9;
            assert!(
                m.multipath_bias_ns < 0.0 && (m.multipath_bias_ns - want).abs() < 1e-9,
                "{m:?}"
            );
        }
        // Beyond the horizon a station is not used; with two left there is no fix.
        let mut far = cfg();
        far.rx_lat_deg = 55.5;
        let r = run(&far);
        assert!(
            r.used < 3 && r.horizontal_error_m.is_none(),
            "used {}",
            r.used
        );
    }
}
