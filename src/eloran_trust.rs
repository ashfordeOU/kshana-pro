// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # eLoran timing observations for multi-source trust
//!
//! Feeds the software eLoran receiver's time solutions ([`kshana_eloran::rx::TimeSolution`])
//! into the Pro multi-source trust selector ([`crate::multi_source`]) as an `eloran` source.
//!
//! ## Conversion
//!
//! A time solution gives, per tracked station, `ref_minus_rx_ns` = the station's time scale
//! (the UTC(k) it is steered to, plus any transmitter error) minus the receiver clock, with the
//! spread of the individual windows (`std_ns`, `n`). The selector wants, per epoch, the
//! **offset of the local clock against the source** (local − source, in ns) and a status.
//! This adapter forms, per station,
//!
//! ```text
//! local − common reference = −(ref_minus_rx + receiver_delay) + (time_ref − common reference)
//! ```
//!
//! where `receiver_delay_ns` is the calibrated antenna/cable/receiver delay (a delay makes the
//! arrival late, so the measured `ref_minus_rx` is too small by it) and
//! `time_ref_offsets_ns[ref]` is the known offset of each station time reference against the
//! common reference the other sources use (e.g. UTC(k) − UTC from BIPM Circular T, or a
//! steering residual published by the eLoran operator). Station estimates are combined with
//! inverse-variance weights after rejecting stations further than `K·σ` from the median
//! (a cross-station consistency check); the per-station σ is `std/√n`, floored at
//! `station_sigma_floor_ns`.
//!
//! An epoch is `ok` when at least `min_stations` stations survive and the combined σ is at
//! most `max_sigma_ns`; otherwise it is `unavailable` (eLoran does not authenticate in this
//! adapter, so it never reports `failed`).
//!
//! ## Table
//!
//! [`merge_table`] appends `<id>_status,<id>_offset_ns` columns to an existing source table
//! (the [`crate::multi_source::parse_table`] format, e.g. one carrying GNSS and PTP columns),
//! matching each row to the nearest eLoran epoch within a tolerance. [`source_def`] gives the
//! matching [`crate::multi_source::SourceDef`] (group `eloran`, the stated σ).
//!
//! MODELLED: the conversion is exact arithmetic on the receiver's outputs; the stated σ is the
//! user's (the selector's rule decides from stated noise levels, see `multi_source`).

use std::collections::BTreeMap;

use kshana_eloran::chain::Chain;
use kshana_eloran::geo::Geo;
use kshana_eloran::rx::{time_solution, Corrections, Measurement, TimeSolution};
use kshana_eloran::time::AbsTime;
use serde::{Deserialize, Serialize};

use crate::multi_source::{Obs, SourceDef, Status};

/// The default source identifier and group.
pub const SOURCE_ID: &str = "eloran";

/// How eLoran time solutions become selector readings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EloranTrustConfig {
    /// Source identifier in the table (`eloran`).
    pub id: String,
    /// Selector group (`eloran`).
    pub group: String,
    /// The σ stated to the selector (ns): what the source is trusted to, not one epoch's σ.
    pub sigma_ns: f64,
    /// Epochs whose combined σ exceeds this are reported `unavailable` (ns).
    pub max_sigma_ns: f64,
    /// Fewest consistent stations for an `ok` epoch.
    pub min_stations: usize,
    /// Floor of a station's σ (ns): carrier-phase ToA noise underestimates propagation errors.
    pub station_sigma_floor_ns: f64,
    /// Consistency multiplier: a station further than `k·σ` from the median is rejected.
    pub k: f64,
    /// Calibrated receiver (antenna, cable, front end) delay (ns).
    pub receiver_delay_ns: f64,
    /// Offset of each station time reference against the common reference (ns), keyed by
    /// the time-reference id of the scenario (`UTC(A)` …). Missing ids count as 0.
    pub time_ref_offsets_ns: BTreeMap<String, f64>,
}

impl Default for EloranTrustConfig {
    fn default() -> Self {
        EloranTrustConfig {
            id: SOURCE_ID.into(),
            group: SOURCE_ID.into(),
            sigma_ns: 60.0,
            max_sigma_ns: 200.0,
            min_stations: 1,
            station_sigma_floor_ns: 10.0,
            k: 4.0,
            receiver_delay_ns: 0.0,
            time_ref_offsets_ns: BTreeMap::new(),
        }
    }
}

/// One station's contribution to an epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationOffset {
    pub chain: String,
    pub station: String,
    pub time_ref: String,
    /// Local clock minus the common reference through this station (ns).
    pub offset_ns: f64,
    pub sigma_ns: f64,
    /// Whether the station passed the consistency check.
    pub used: bool,
}

/// One epoch of the `eloran` source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EloranEpoch {
    pub t_s: f64,
    pub status: Status,
    /// Local clock minus the common reference (ns), when `ok`.
    pub offset_ns: Option<f64>,
    /// Combined one-sigma of the epoch (ns).
    pub sigma_ns: Option<f64>,
    pub stations: Vec<StationOffset>,
}

impl EloranEpoch {
    /// The selector reading of this epoch.
    pub fn obs(&self) -> Obs {
        Obs {
            status: self.status,
            offset_s: self.offset_ns.unwrap_or(0.0) * 1e-9,
        }
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// Convert one time solution (taken at epoch `t_s`) into a selector epoch.
pub fn epoch_from_time_solution(
    t_s: f64,
    ts: &TimeSolution,
    cfg: &EloranTrustConfig,
) -> EloranEpoch {
    let mut stations: Vec<StationOffset> = ts
        .stations
        .iter()
        .filter(|s| s.ref_minus_rx_ns.is_finite())
        .map(|s| {
            let off_ref = cfg
                .time_ref_offsets_ns
                .get(&s.time_ref)
                .copied()
                .unwrap_or(0.0);
            let sigma = if s.n > 1 {
                s.std_ns / (s.n as f64).sqrt()
            } else {
                0.0
            }
            .max(cfg.station_sigma_floor_ns);
            StationOffset {
                chain: s.chain.clone(),
                station: s.station.clone(),
                time_ref: s.time_ref.clone(),
                offset_ns: -(s.ref_minus_rx_ns + cfg.receiver_delay_ns) + off_ref,
                sigma_ns: sigma,
                used: false,
            }
        })
        .collect();
    let med = median(stations.iter().map(|s| s.offset_ns).collect());
    let (mut sw, mut swx) = (0.0, 0.0);
    let mut used = 0usize;
    for s in &mut stations {
        if (s.offset_ns - med).abs() <= cfg.k * s.sigma_ns.max(1e-3) * std::f64::consts::SQRT_2 {
            s.used = true;
            let w = 1.0 / (s.sigma_ns * s.sigma_ns);
            sw += w;
            swx += w * s.offset_ns;
            used += 1;
        }
    }
    let (offset, sigma) = if used > 0 {
        (Some(swx / sw), Some((1.0 / sw).sqrt()))
    } else {
        (None, None)
    };
    let ok = used >= cfg.min_stations.max(1) && sigma.is_some_and(|s| s <= cfg.max_sigma_ns);
    EloranEpoch {
        t_s,
        status: if ok { Status::Ok } else { Status::Unavailable },
        offset_ns: if ok { offset } else { None },
        sigma_ns: sigma,
        stations,
    }
}

/// Split a recording's measurements into epochs of `window_s` (by arrival time on the sample
/// clock) and take a time solution at the known position `rx` in each. Returns
/// `(epoch centre time, solution)` for every window that holds a measurement.
pub fn time_solutions_by_window(
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
    rx: Geo,
    corr: &Corrections,
    window_s: f64,
) -> Vec<(f64, TimeSolution)> {
    let mut bins: BTreeMap<i64, Vec<Measurement>> = BTreeMap::new();
    for m in meas {
        bins.entry((m.toa_s / window_s).floor() as i64)
            .or_default()
            .push(m.clone());
    }
    bins.into_iter()
        .map(|(k, ms)| {
            (
                (k as f64 + 0.5) * window_s,
                time_solution(&ms, chains, start, rx, corr),
            )
        })
        .collect()
}

/// The selector source definition of the `eloran` source.
pub fn source_def(cfg: &EloranTrustConfig) -> SourceDef {
    SourceDef {
        id: cfg.id.clone(),
        group: cfg.group.clone(),
        sigma_ns: cfg.sigma_ns,
        authenticates_group: false,
    }
}

/// The two table columns of the source.
pub fn columns(cfg: &EloranTrustConfig) -> [String; 2] {
    [
        format!("{}_status", cfg.id),
        format!("{}_offset_ns", cfg.id),
    ]
}

/// The epoch nearest `t` within `tol_s` (epochs sorted by time).
fn nearest(epochs: &[EloranEpoch], t: f64, tol_s: f64) -> Option<&EloranEpoch> {
    let i = epochs.partition_point(|e| e.t_s < t);
    [i.checked_sub(1), Some(i)]
        .into_iter()
        .flatten()
        .filter_map(|j| epochs.get(j))
        .filter(|e| (e.t_s - t).abs() <= tol_s)
        .min_by(|a, b| (a.t_s - t).abs().total_cmp(&(b.t_s - t).abs()))
}

/// Append the `eloran` columns to a source table: each row takes the eLoran epoch nearest its
/// `t_s` within `tol_s`, or `unavailable`. Comment and blank lines are kept.
pub fn merge_table(
    table: &str,
    epochs: &[EloranEpoch],
    cfg: &EloranTrustConfig,
    tol_s: f64,
) -> Result<String, String> {
    let mut sorted = epochs.to_vec();
    sorted.sort_by(|a, b| a.t_s.total_cmp(&b.t_s));
    let [cs, co] = columns(cfg);
    let mut out = String::with_capacity(table.len() + 32 * epochs.len());
    let mut header_seen = false;
    for (ln, line) in table.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if !header_seen {
            header_seen = true;
            if line.split(',').any(|c| c.trim() == cs || c.trim() == co) {
                return Err(format!("the table already has a {} column", cfg.id));
            }
            out.push_str(&format!("{line},{cs},{co}\n"));
            continue;
        }
        let t: f64 = line
            .split(',')
            .next()
            .and_then(|f| f.trim().parse().ok())
            .ok_or(format!("line {}: t_s is not a number", ln + 1))?;
        match nearest(&sorted, t, tol_s) {
            Some(e) if e.status == Status::Ok => {
                out.push_str(&format!("{line},ok,{:.3}\n", e.offset_ns.unwrap_or(0.0)))
            }
            _ => out.push_str(&format!("{line},unavailable,\n")),
        }
    }
    if !header_seen {
        return Err("the source table is empty".into());
    }
    Ok(out)
}

/// A table of the `eloran` source alone (`t_s,<id>_status,<id>_offset_ns`).
pub fn table(epochs: &[EloranEpoch], cfg: &EloranTrustConfig) -> String {
    let [cs, co] = columns(cfg);
    let mut out = format!("t_s,{cs},{co}\n");
    for e in epochs {
        match (e.status, e.offset_ns) {
            (Status::Ok, Some(o)) => out.push_str(&format!("{},ok,{o:.3}\n", e.t_s)),
            _ => out.push_str(&format!("{},unavailable,\n", e.t_s)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kshana_eloran::rx::StationClock;

    fn sc(st: &str, r: f64, std: f64, n: usize) -> StationClock {
        StationClock {
            chain: "c".into(),
            station: st.into(),
            time_ref: "UTC(A)".into(),
            ref_minus_rx_ns: r,
            std_ns: std,
            n,
        }
    }

    #[test]
    fn sign_convention_and_outlier_rejection() {
        let ts = TimeSolution {
            stations: vec![
                sc("M", 50.0, 0.0, 1),
                sc("X", 52.0, 0.0, 1),
                sc("Y", 900.0, 0.0, 1),
            ],
            by_time_ref: vec![],
        };
        let mut cfg = EloranTrustConfig::default();
        cfg.time_ref_offsets_ns.insert("UTC(A)".into(), 50.0);
        let e = epoch_from_time_solution(10.0, &ts, &cfg);
        assert_eq!(e.status, Status::Ok);
        // UTC(A) − rx = 51 ns and UTC(A) − UTC = 50 ns: rx − UTC = −1 ns.
        assert!((e.offset_ns.unwrap() + 1.0).abs() < 1e-9, "{e:?}");
        assert!(!e.stations[2].used);
        assert!((e.sigma_ns.unwrap() - 10.0 / 2f64.sqrt()).abs() < 1e-9);
    }
}
