// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! [`SoftwareGnssBridge`]: the data needed to drive the open Kshana engine's software GNSS
//! IQ scene (`kshana::iq::scene`) with the same trajectory and time base as an eLoran scene.
//!
//! This crate does **not** depend on the open engine; the integration lives in kshana-pro.
//! What crosses the boundary is [`GnssBridgeData`] (JSON-serialisable), produced from a
//! scenario by [`bridge_data`]. The kshana-pro integration is expected to:
//!
//! 1. set the GNSS scene's start to `gps_week` / `gps_tow_s` (the same instant as the
//!    eLoran scene's `start_utc`, so both scenes share one truth timeline);
//! 2. drive the GNSS receiver antenna along `trajectory` (same samples the eLoran scene
//!    evaluates, interpolated linearly between them);
//! 3. offset the GPS system time of the GNSS scene from truth by `gpst_residual_ns` (the
//!    scenario's `gnss_time_residual_ns.Gpst`), mirroring how the eLoran chains carry their
//!    UTC(k) offsets, so inter-system offsets in the truth are consistent;
//! 4. when both are output through hardware, run both from the common 10 MHz/1PPS and
//!    apply `eloran_minus_gnss_ns` on the eLoran side (see [`super::SyncPlan`]).

use super::export::{sample_trajectory, TrajSample};
use super::UtcStamp;
use crate::hw::calib::CalibrationFile;
use crate::scenario::Scenario;
use crate::timebase::GnssTime;
use serde::{Deserialize, Serialize};

/// Stub type naming the bridge; see the module documentation.
#[derive(Clone, Copy, Debug, Default)]
pub struct SoftwareGnssBridge;

impl SoftwareGnssBridge {
    /// Same as [`bridge_data`].
    pub fn data(&self, sc: &Scenario, rate_hz: f64, cal: Option<&CalibrationFile>) -> Result<GnssBridgeData, String> {
        bridge_data(sc, rate_hz, cal)
    }
}

/// Everything the open engine's IQ scene needs to run alongside an eLoran scene.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnssBridgeData {
    /// Scenario hash of the eLoran scenario this was derived from.
    pub scenario_hash: String,
    pub start_utc: String,
    pub gps_week: i64,
    pub gps_tow_s: f64,
    /// GPS − UTC at the start (s).
    pub gps_minus_utc_s: f64,
    pub duration_s: f64,
    /// GPST steering residual of the scene (ns).
    pub gpst_residual_ns: f64,
    /// Trajectory samples (scene time, UTC, position, heading, speed).
    pub trajectory_rate_hz: f64,
    pub trajectory: Vec<TrajSample>,
    /// Measured eLoran − GNSS output offset (ns) to remove on the eLoran side (0 when the
    /// GNSS output is software only).
    pub eloran_minus_gnss_ns: f64,
}

/// Produce the bridge data of a scenario (trajectory sampled at `rate_hz`).
pub fn bridge_data(sc: &Scenario, rate_hz: f64, cal: Option<&CalibrationFile>) -> Result<GnssBridgeData, String> {
    let start = UtcStamp::parse(&sc.scene.start_utc)?;
    let (w, tow) = start.gps_week_tow();
    Ok(GnssBridgeData {
        scenario_hash: sc.hash(),
        start_utc: sc.scene.start_utc.clone(),
        gps_week: w,
        gps_tow_s: tow,
        gps_minus_utc_s: start.leap_s() - 19.0,
        duration_s: sc.scene.params.duration_s,
        gpst_residual_ns: sc.gnss_time_residual_ns.get(&GnssTime::Gpst).copied().unwrap_or(0.0),
        trajectory_rate_hz: rate_hz,
        trajectory: sample_trajectory(sc, rate_hz)?,
        eloran_minus_gnss_ns: cal.and_then(|c| c.gnss_offset.as_ref()).map(|g| g.eloran_minus_gnss_ns).unwrap_or(0.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_data_from_scenario() {
        let src = r#"
gnss_time_residual_ns = { Gpst = 3.0 }
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 1.0
fs_hz = 1000000
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 1.0
[[outputs]]
type = "e_field"
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.0, lon_deg = 0.0 }
"#;
        let sc = Scenario::from_toml(src).unwrap();
        let d = SoftwareGnssBridge.data(&sc, 10.0, None).unwrap();
        assert_eq!(d.trajectory.len(), 11);
        assert_eq!(d.gps_minus_utc_s, 18.0);
        assert_eq!(d.gpst_residual_ns, 3.0);
        // 2026-11-18 is a Wednesday: TOW = 3 days + 18 s
        assert!((d.gps_tow_s - (3.0 * 86400.0 + 18.0)).abs() < 1e-6);
        assert!(serde_json::to_string(&d).unwrap().contains("gps_week"));
    }
}
