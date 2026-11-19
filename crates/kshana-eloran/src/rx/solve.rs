// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Measurements to solutions: time (known position), range-domain position and clock,
//! and hyperbolic (TDoA) fixes.
//!
//! # Observation
//!
//! Each [`Measurement`] gives the arrival (scene time, the receiver's sample clock) of a
//! station's group `k`. Its nominal emission is computed exactly from the chain's schedule:
//! group `k` starts at `k·GRI` after the Loran epoch (TAI), the station transmits its group
//! `emission_delay` after that ([`crate::chain`]). The **pseudo-delay** is
//! `toa − nominal emission`, which equals
//!
//! ```text
//! propagation delay + (receiver clock − station time scale)
//! ```
//!
//! # Prediction
//!
//! The predicted propagation delay is the Primary Factor `η·d/c` (η = 1.000338, `d` the
//! WGS-84 geodesic distance from [`crate::geo::inverse`]) plus a user correction per
//! station in nanoseconds ([`Corrections`]: ASF, and SF when the user's table includes it).
//! With the simulator's `simple` groundwave model (PF only) the prediction is exact.
//!
//! # Solutions
//!
//! * [`time_solution`]: known position; `ref_minus_rx_ns = −(pseudo-delay − predicted)` per
//!   station, i.e. the station's time scale (UTC(k) plus transmitter error) minus the
//!   receiver clock; means per time reference.
//! * [`position_solution`]: iterative (Gauss–Newton) least squares for latitude, longitude
//!   and one clock bias (or one per chain), all-in-view, multi-chain; geometry from the
//!   geodesic azimuths, HDOP from `(HᵀH)⁻¹`, residuals per station.
//! * [`tdoa_fix`]: classic hyperbolic fix from master–secondary time differences of each
//!   chain (no clock unknown), for comparison.
//!
//! Limitations: equal weights; heights ignored (LF groundwave on the ellipsoid); several
//! measurements of a station are averaged into one observation, so for a moving receiver
//! pass only the measurements of one epoch.

use super::dsp::{invert, solve};
use super::Measurement;
use crate::chain::{Chain, Station};
use crate::geo::{inverse, Geo, WGS84_A, WGS84_F};
use crate::prop::{ground::N_ATM, C};
use crate::time::{AbsTime, Ps};
use std::collections::BTreeMap;

/// Per-station propagation corrections beyond the Primary Factor (ns), keyed
/// `"chain/station"` (e.g. `"9930/M"`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Corrections {
    pub asf_ns: BTreeMap<String, f64>,
}

impl Corrections {
    pub fn key(chain: &str, station: &str) -> String {
        format!("{chain}/{station}")
    }
    pub fn get(&self, chain: &str, station: &str) -> f64 {
        self.asf_ns
            .get(&Self::key(chain, station))
            .copied()
            .unwrap_or(0.0)
    }
}

/// Predicted groundwave delay (s): PF plus the station's correction.
pub fn predicted_delay_s(st: &Station, rx: Geo, chain: &str, corr: &Corrections) -> f64 {
    let d = inverse(st.position, rx).distance_m;
    N_ATM * d / C + corr.get(chain, &st.id) * 1e-9
}

/// Nominal emission (scene s) of pulse 0 of station `si`'s group `group`.
pub fn nominal_emission_s(chain: &Chain, si: usize, group: i64, start: AbsTime) -> f64 {
    let gri = chain.gri().0 as i128;
    let abs = group as i128 * gri;
    (Ps((abs - start.0) as i64) + Ps::from_us(chain.stations[si].emission_delay_us)).as_s()
}

/// One station's observation: the mean pseudo-delay of its measurements.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub chain_index: usize,
    pub station_index: usize,
    pub chain: String,
    pub station: String,
    /// Mean of `toa − nominal emission` (s).
    pub pseudo_delay_s: f64,
    /// Spread (std) of the individual pseudo-delays (s) and their count.
    pub std_s: f64,
    pub n: usize,
}

/// Group measurements per station (and per channel = None or the first channel seen).
pub fn observations(meas: &[Measurement], chains: &[Chain], start: AbsTime) -> Vec<Observation> {
    let mut map: BTreeMap<(usize, usize), Vec<f64>> = BTreeMap::new();
    for m in meas {
        let pd =
            m.toa_s - nominal_emission_s(&chains[m.chain_index], m.station_index, m.group, start);
        map.entry((m.chain_index, m.station_index))
            .or_default()
            .push(pd);
    }
    map.into_iter()
        .map(|((ci, si), v)| {
            let n = v.len();
            let mean = v.iter().sum::<f64>() / n as f64;
            let var = if n > 1 {
                v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64
            } else {
                0.0
            };
            Observation {
                chain_index: ci,
                station_index: si,
                chain: chains[ci].name.clone(),
                station: chains[ci].stations[si].id.clone(),
                pseudo_delay_s: mean,
                std_s: var.sqrt(),
                n,
            }
        })
        .collect()
}

/// Clock estimate against one station.
#[derive(Clone, Debug, PartialEq)]
pub struct StationClock {
    pub chain: String,
    pub station: String,
    pub time_ref: String,
    /// Station time scale minus receiver clock (ns), mean over the measurements.
    pub ref_minus_rx_ns: f64,
    pub std_ns: f64,
    pub n: usize,
}

/// Time solution for a known receiver position.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeSolution {
    pub stations: Vec<StationClock>,
    /// Mean `ref_minus_rx_ns` per time reference id: (id, mean ns, stations).
    pub by_time_ref: Vec<(String, f64, usize)>,
}

/// Receiver clock against every measured station at a known position.
pub fn time_solution(
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
    rx: Geo,
    corr: &Corrections,
) -> TimeSolution {
    let obs = observations(meas, chains, start);
    let stations: Vec<StationClock> = obs
        .iter()
        .map(|o| {
            let st = &chains[o.chain_index].stations[o.station_index];
            let pred = predicted_delay_s(st, rx, &o.chain, corr);
            StationClock {
                chain: o.chain.clone(),
                station: o.station.clone(),
                time_ref: st.time_ref.clone(),
                ref_minus_rx_ns: -(o.pseudo_delay_s - pred) * 1e9,
                std_ns: o.std_s * 1e9,
                n: o.n,
            }
        })
        .collect();
    let mut by: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for s in &stations {
        by.entry(s.time_ref.clone())
            .or_default()
            .push(s.ref_minus_rx_ns);
    }
    let by_time_ref = by
        .into_iter()
        .map(|(k, v)| {
            let n = v.len();
            (k, v.iter().sum::<f64>() / n as f64, n)
        })
        .collect();
    TimeSolution {
        stations,
        by_time_ref,
    }
}

/// Options of the position solution.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionConfig {
    /// Starting point (default: centroid of the measured stations).
    pub initial: Option<Geo>,
    /// One clock bias per chain instead of one common bias.
    pub bias_per_chain: bool,
    pub max_iter: usize,
}

impl Default for PositionConfig {
    fn default() -> Self {
        PositionConfig {
            initial: None,
            bias_per_chain: false,
            max_iter: 30,
        }
    }
}

/// Range-domain position + clock solution.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionSolution {
    pub position: Geo,
    /// Clock bias(es), receiver clock minus station time scale (ns): ("all" or chain name).
    pub clock_bias_ns: Vec<(String, f64)>,
    pub hdop: f64,
    /// Post-fit residuals per station `"chain/station"` (ns).
    pub residuals_ns: Vec<(String, f64)>,
    pub iterations: usize,
    pub converged: bool,
}

fn radii(lat_deg: f64) -> (f64, f64) {
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let s = lat_deg.to_radians().sin();
    let w = (1.0 - e2 * s * s).sqrt();
    (WGS84_A * (1.0 - e2) / (w * w * w), WGS84_A / w)
}

fn centroid(chains: &[Chain], obs: &[Observation]) -> Geo {
    let n = obs.len().max(1) as f64;
    let (la, lo) = obs.iter().fold((0.0, 0.0), |a, o| {
        let p = chains[o.chain_index].stations[o.station_index].position;
        (a.0 + p.lat_deg, a.1 + p.lon_deg)
    });
    Geo::new(la / n, lo / n)
}

fn step(pos: Geo, dn: f64, de: f64) -> Geo {
    let (rm, rn) = radii(pos.lat_deg);
    Geo {
        lat_deg: pos.lat_deg + (dn / rm).to_degrees(),
        lon_deg: pos.lon_deg + (de / (rn * pos.lat_deg.to_radians().cos())).to_degrees(),
        h_m: pos.h_m,
    }
}

/// All-in-view range-domain least squares on the ellipsoid.
pub fn position_solution(
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
    corr: &Corrections,
    cfg: &PositionConfig,
) -> Result<PositionSolution, String> {
    let obs = observations(meas, chains, start);
    let groups: Vec<String> = if cfg.bias_per_chain {
        let mut g: Vec<String> = obs.iter().map(|o| o.chain.clone()).collect();
        g.dedup();
        g.sort();
        g.dedup();
        g
    } else {
        vec!["all".into()]
    };
    let nu = 2 + groups.len();
    if obs.len() < nu {
        return Err(format!(
            "position solution needs at least {nu} stations, have {}",
            obs.len()
        ));
    }
    let bias_ix = |o: &Observation| {
        if cfg.bias_per_chain {
            2 + groups.iter().position(|g| *g == o.chain).unwrap()
        } else {
            2
        }
    };
    let mut pos = cfg.initial.unwrap_or_else(|| centroid(chains, &obs));
    let mut bias = vec![0.0; groups.len()]; // metres
    let mut converged = false;
    let mut it = 0;
    let mut q = vec![vec![0.0; nu]; nu];
    let mut res = vec![0.0; obs.len()];
    while it < cfg.max_iter {
        it += 1;
        let mut ata = vec![vec![0.0; nu]; nu];
        let mut atb = vec![0.0; nu];
        for (k, o) in obs.iter().enumerate() {
            let st = &chains[o.chain_index].stations[o.station_index];
            let inv = inverse(pos, st.position);
            let bi = bias_ix(o);
            let pred =
                N_ATM * inv.distance_m + corr.get(&o.chain, &o.station) * 1e-9 * C + bias[bi - 2];
            let r = o.pseudo_delay_s * C - pred;
            res[k] = r;
            let mut h = vec![0.0; nu];
            h[0] = -N_ATM * inv.az1_rad.cos();
            h[1] = -N_ATM * inv.az1_rad.sin();
            h[bi] = 1.0;
            for i in 0..nu {
                atb[i] += h[i] * r;
                for j in 0..nu {
                    ata[i][j] += h[i] * h[j];
                }
            }
        }
        let dx = solve(&ata, &atb).ok_or("position solution: singular geometry")?;
        q = invert(&ata).ok_or("position solution: singular geometry")?;
        pos = step(pos, dx[0], dx[1]);
        for (b, d) in bias.iter_mut().zip(&dx[2..]) {
            *b += d;
        }
        if dx[0].hypot(dx[1]) < 1e-4 && dx[2..].iter().all(|d| d.abs() < 1e-4) {
            converged = true;
            break;
        }
    }
    // final residuals at the solution
    for (k, o) in obs.iter().enumerate() {
        let st = &chains[o.chain_index].stations[o.station_index];
        let d = inverse(pos, st.position).distance_m;
        res[k] = o.pseudo_delay_s * C
            - (N_ATM * d + corr.get(&o.chain, &o.station) * 1e-9 * C + bias[bias_ix(o) - 2]);
    }
    Ok(PositionSolution {
        position: pos,
        clock_bias_ns: groups
            .iter()
            .cloned()
            .zip(bias.iter().map(|b| b / C * 1e9))
            .collect(),
        hdop: (q[0][0] + q[1][1]).sqrt(),
        residuals_ns: obs
            .iter()
            .zip(&res)
            .map(|(o, r)| (Corrections::key(&o.chain, &o.station), r / C * 1e9))
            .collect(),
        iterations: it,
        converged,
    })
}

/// One time difference of a hyperbolic fix.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeDifference {
    pub chain: String,
    pub secondary: String,
    /// Classic Loran TD: secondary arrival − master arrival of the same group (µs),
    /// emission delay included.
    pub td_us: f64,
    /// Post-fit residual (ns).
    pub residual_ns: f64,
}

/// Hyperbolic (TDoA) fix.
#[derive(Clone, Debug, PartialEq)]
pub struct TdFix {
    pub position: Geo,
    pub tds: Vec<TimeDifference>,
    /// `√(Q_NN + Q_EE)` of the TD geometry (master errors treated as independent).
    pub hdop: f64,
    pub iterations: usize,
    pub converged: bool,
}

/// Hyperbolic fix from the master–secondary TDs of every chain whose master is measured.
pub fn tdoa_fix(
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
    corr: &Corrections,
    initial: Option<Geo>,
) -> Result<TdFix, String> {
    let obs = observations(meas, chains, start);
    // (secondary obs, master obs)
    let mut pairs: Vec<(&Observation, &Observation)> = Vec::new();
    for o in &obs {
        let ch = &chains[o.chain_index];
        if ch.stations[o.station_index].role == crate::signal::Role::Master {
            continue;
        }
        if let Some(m) = obs.iter().find(|m| {
            m.chain_index == o.chain_index
                && ch.stations[m.station_index].role == crate::signal::Role::Master
        }) {
            pairs.push((o, m));
        }
    }
    if pairs.len() < 2 {
        return Err(format!(
            "TDoA fix needs at least 2 time differences, have {}",
            pairs.len()
        ));
    }
    let mut pos = initial.unwrap_or_else(|| centroid(chains, &obs));
    let mut converged = false;
    let mut it = 0;
    let mut q = vec![vec![0.0; 2]; 2];
    let model = |pos: Geo, s: &Observation, m: &Observation| {
        let ss = &chains[s.chain_index].stations[s.station_index];
        let ms = &chains[m.chain_index].stations[m.station_index];
        let a = inverse(pos, ss.position);
        let b = inverse(pos, ms.position);
        let pred = N_ATM * (a.distance_m - b.distance_m)
            + (corr.get(&s.chain, &s.station) - corr.get(&m.chain, &m.station)) * 1e-9 * C;
        let r = (s.pseudo_delay_s - m.pseudo_delay_s) * C - pred;
        let h = [
            -N_ATM * (a.az1_rad.cos() - b.az1_rad.cos()),
            -N_ATM * (a.az1_rad.sin() - b.az1_rad.sin()),
        ];
        (r, h)
    };
    while it < 30 {
        it += 1;
        let mut ata = vec![vec![0.0; 2]; 2];
        let mut atb = vec![0.0; 2];
        for (s, m) in &pairs {
            let (r, h) = model(pos, s, m);
            for i in 0..2 {
                atb[i] += h[i] * r;
                for j in 0..2 {
                    ata[i][j] += h[i] * h[j];
                }
            }
        }
        let dx = solve(&ata, &atb).ok_or("TDoA fix: singular geometry")?;
        q = invert(&ata).ok_or("TDoA fix: singular geometry")?;
        pos = step(pos, dx[0], dx[1]);
        if dx[0].hypot(dx[1]) < 1e-4 {
            converged = true;
            break;
        }
    }
    let tds = pairs
        .iter()
        .map(|(s, m)| {
            let (r, _) = model(pos, s, m);
            let ed = chains[s.chain_index].stations[s.station_index].emission_delay_us;
            TimeDifference {
                chain: s.chain.clone(),
                secondary: s.station.clone(),
                td_us: (s.pseudo_delay_s - m.pseudo_delay_s) * 1e6 + ed,
                residual_ns: r / C * 1e9,
            }
        })
        .collect();
    Ok(TdFix {
        position: pos,
        tds,
        hdop: (q[0][0] + q[1][1]).sqrt(),
        iterations: it,
        converged,
    })
}
