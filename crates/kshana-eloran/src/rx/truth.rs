// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Comparison of receiver outputs with the simulator's [`TruthRecord`]s.
//!
//! Measurements are matched to truth by chain name, station id and group number. A ToA
//! error beyond a quarter carrier period (2.5 µs) is counted as a cycle (or half-cycle)
//! error; such measurements still enter the statistics, so a cycle error is never hidden.

use super::solve::TimeSolution;
use super::Measurement;
use crate::geo::{inverse, Geo};
use crate::scene::TruthRecord;
use std::collections::HashMap;

/// ToA / ECD error statistics of one station (and channel, for per-loop processing).
#[derive(Clone, Debug, PartialEq)]
pub struct StationErrors {
    pub chain: String,
    pub station: String,
    pub channel: Option<usize>,
    /// Matched measurements.
    pub n: usize,
    pub toa_mean_ns: f64,
    pub toa_std_ns: f64,
    pub toa_rms_ns: f64,
    pub toa_max_abs_ns: f64,
    pub ecd_mean_err_us: f64,
    pub ecd_rms_err_us: f64,
    /// Measurements whose ToA error exceeds 2.5 µs (wrong cycle or half cycle).
    pub cycle_errors: usize,
    /// Mean measured SNR and truth SNR (dB, 20 kHz).
    pub snr_meas_db: f64,
    pub snr_truth_db: f64,
    /// Bearing error (deg) for crossed-loop beam steering (measured relative bearing vs
    /// truth bearing − heading), mean absolute.
    pub bearing_err_deg: Option<f64>,
}

fn truth_index(truth: &[TruthRecord]) -> HashMap<(&str, &str, i64), &TruthRecord> {
    truth.iter().map(|r| ((r.chain.as_str(), r.station.as_str(), r.group), r)).collect()
}

/// The truth record matching a measurement.
pub fn matching_truth<'a>(m: &Measurement, truth: &'a [TruthRecord]) -> Option<&'a TruthRecord> {
    truth.iter().find(|r| r.chain == m.chain && r.station == m.station && r.group == m.group)
}

/// Per-station ToA, ECD, SNR and bearing errors.
pub fn toa_errors(meas: &[Measurement], truth: &[TruthRecord]) -> Vec<StationErrors> {
    let idx = truth_index(truth);
    let mut keys: Vec<(String, String, Option<usize>)> = Vec::new();
    for m in meas {
        let k = (m.chain.clone(), m.station.clone(), m.channel);
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys.into_iter()
        .map(|(c, s, ch)| {
            let pairs: Vec<(&Measurement, &TruthRecord)> = meas
                .iter()
                .filter(|m| m.chain == c && m.station == s && m.channel == ch)
                .filter_map(|m| idx.get(&(m.chain.as_str(), m.station.as_str(), m.group)).map(|r| (m, *r)))
                .collect();
            let n = pairs.len();
            let e: Vec<f64> = pairs.iter().map(|(m, r)| (m.toa_s - r.toa_s) * 1e9).collect();
            let ecd: Vec<f64> = pairs.iter().map(|(m, r)| m.ecd_us - r.ecd_us).collect();
            let nf = n.max(1) as f64;
            let mean = e.iter().sum::<f64>() / nf;
            let std = if n > 1 { (e.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt() } else { 0.0 };
            let bearing: Vec<f64> = pairs
                .iter()
                .filter_map(|(m, r)| {
                    m.bearing_deg.map(|b| {
                        let d = (b - (r.bearing_deg - r.rx_heading_deg)).rem_euclid(360.0);
                        if d > 180.0 { 360.0 - d } else { d }
                    })
                })
                .collect();
            StationErrors {
                chain: c,
                station: s,
                channel: ch,
                n,
                toa_mean_ns: mean,
                toa_std_ns: std,
                toa_rms_ns: (e.iter().map(|x| x * x).sum::<f64>() / nf).sqrt(),
                toa_max_abs_ns: e.iter().fold(0.0, |a, x| a.max(x.abs())),
                ecd_mean_err_us: ecd.iter().sum::<f64>() / nf,
                ecd_rms_err_us: (ecd.iter().map(|x| x * x).sum::<f64>() / nf).sqrt(),
                cycle_errors: e.iter().filter(|x| x.abs() > 2500.0).count(),
                snr_meas_db: pairs.iter().map(|(m, _)| m.snr_db).sum::<f64>() / nf,
                snr_truth_db: pairs.iter().map(|(_, r)| r.snr_db).sum::<f64>() / nf,
                bearing_err_deg: if bearing.is_empty() {
                    None
                } else {
                    Some(bearing.iter().sum::<f64>() / bearing.len() as f64)
                },
            }
        })
        .collect()
}

/// Horizontal error (m) of `pos` against the truth receiver position (mean over records).
pub fn position_error_m(pos: Geo, truth: &[TruthRecord]) -> Option<f64> {
    if truth.is_empty() {
        return None;
    }
    let n = truth.len() as f64;
    let lat = truth.iter().map(|r| r.rx_lat_deg).sum::<f64>() / n;
    let lon = truth.iter().map(|r| r.rx_lon_deg).sum::<f64>() / n;
    Some(inverse(pos, Geo::new(lat, lon)).distance_m)
}

/// Clock error of a time solution against one station.
#[derive(Clone, Debug, PartialEq)]
pub struct ClockError {
    pub chain: String,
    pub station: String,
    /// Estimated station-time-scale minus receiver clock (ns).
    pub estimated_ns: f64,
    /// Truth: −(true − nominal emission) (ns), i.e. the station's time-reference offset plus
    /// transmitter clock and timing faults; and the time reference offset alone.
    pub truth_ns: f64,
    pub time_ref_offset_ns: f64,
    pub error_ns: f64,
}

/// Per-station clock errors (the receiver's sample clock is the truth timeline).
pub fn clock_errors(ts: &TimeSolution, truth: &[TruthRecord]) -> Vec<ClockError> {
    ts.stations
        .iter()
        .filter_map(|s| {
            let rs: Vec<&TruthRecord> = truth.iter().filter(|r| r.chain == s.chain && r.station == s.station).collect();
            if rs.is_empty() {
                return None;
            }
            let n = rs.len() as f64;
            let t = -rs.iter().map(|r| r.emission_error_ns).sum::<f64>() / n;
            let x = rs.iter().map(|r| r.time_ref_offset_ns).sum::<f64>() / n;
            Some(ClockError {
                chain: s.chain.clone(),
                station: s.station.clone(),
                estimated_ns: s.ref_minus_rx_ns,
                truth_ns: t,
                time_ref_offset_ns: x,
                error_ns: s.ref_minus_rx_ns - t,
            })
        })
        .collect()
}
