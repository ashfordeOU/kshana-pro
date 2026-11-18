// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Receiver motion samples for GNSS simulators that take position / velocity / attitude
//! (streamed hardware-in-the-loop style, or as ECEF/LLH motion files).
//!
//! [`motion_from_traj`] turns the scenario trajectory samples of
//! [`super::export::sample_trajectory`] into [`MotionSample`]s carrying WGS-84 ECEF
//! position, ENU and ECEF velocity and a simple attitude (yaw = heading, pitch = flight
//! path angle, roll = 0). Vertical velocity is a finite difference of the sampled heights.

use super::export::TrajSample;
use super::UtcStamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// WGS-84 semi-major axis (m).
pub const WGS84_A: f64 = 6_378_137.0;
/// WGS-84 flattening.
pub const WGS84_F: f64 = 1.0 / 298.257_223_563;

/// Geodetic (deg, deg, m above the ellipsoid) to WGS-84 ECEF (m).
pub fn llh_to_ecef(lat_deg: f64, lon_deg: f64, h_m: f64) -> [f64; 3] {
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let (sl, cl) = lat_deg.to_radians().sin_cos();
    let (so, co) = lon_deg.to_radians().sin_cos();
    let n = WGS84_A / (1.0 - e2 * sl * sl).sqrt();
    [
        (n + h_m) * cl * co,
        (n + h_m) * cl * so,
        (n * (1.0 - e2) + h_m) * sl,
    ]
}

/// WGS-84 ECEF (m) to geodetic (deg, deg, m), iterative (converges to < 1 mm).
pub fn ecef_to_llh(p: [f64; 3]) -> (f64, f64, f64) {
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let lon = p[1].atan2(p[0]);
    let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
    let mut lat = p[2].atan2(r * (1.0 - e2));
    let mut h = 0.0;
    for _ in 0..10 {
        let sl = lat.sin();
        let n = WGS84_A / (1.0 - e2 * sl * sl).sqrt();
        h = if lat.cos().abs() > 1e-12 {
            r / lat.cos() - n
        } else {
            p[2].abs() - n * (1.0 - e2)
        };
        lat = p[2].atan2(r * (1.0 - e2 * n / (n + h)));
    }
    (lat.to_degrees(), lon.to_degrees(), h)
}

/// Rotate a local East-North-Up vector at (lat, lon) into ECEF axes.
pub fn enu_to_ecef_vec(lat_deg: f64, lon_deg: f64, enu: [f64; 3]) -> [f64; 3] {
    let (sl, cl) = lat_deg.to_radians().sin_cos();
    let (so, co) = lon_deg.to_radians().sin_cos();
    let [e, n, u] = enu;
    [
        -so * e - sl * co * n + cl * co * u,
        co * e - sl * so * n + cl * so * u,
        cl * n + sl * u,
    ]
}

/// One receiver motion sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionSample {
    /// Scene time (s).
    pub t_s: f64,
    pub utc: UtcStamp,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub h_m: f64,
    /// WGS-84 ECEF position (m).
    pub ecef_m: [f64; 3],
    /// Velocity, East-North-Up (m/s).
    pub vel_enu_mps: [f64; 3],
    /// Velocity, ECEF axes (m/s).
    pub vel_ecef_mps: [f64; 3],
    /// Yaw = heading (deg clockwise from north), pitch = flight-path angle (deg), roll (deg).
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub roll_deg: f64,
}

impl MotionSample {
    /// Template placeholder values: `t_s`, `t_hms`, `utc_iso`, `gps_week`, `gps_tow`,
    /// `lat_deg`, `lon_deg`, `lat_rad`, `lon_rad`, `h_m`, `x`, `y`, `z`, `vx`, `vy`, `vz`,
    /// `ve`, `vn`, `vu`, `yaw_deg`, `pitch_deg`, `roll_deg`, `yaw_rad`, `pitch_rad`,
    /// `roll_rad`.
    pub fn fields(&self) -> BTreeMap<String, String> {
        let mut f = BTreeMap::new();
        let (w, tow) = self.utc.gps_week_tow();
        f.insert("t_s".into(), format!("{:.3}", self.t_s));
        f.insert("t_hms".into(), super::profile::hms(self.t_s));
        f.insert("utc_iso".into(), self.utc.iso());
        f.insert("gps_week".into(), w.to_string());
        f.insert("gps_tow".into(), format!("{tow:.3}"));
        f.insert("lat_deg".into(), format!("{:.9}", self.lat_deg));
        f.insert("lon_deg".into(), format!("{:.9}", self.lon_deg));
        f.insert("lat_rad".into(), format!("{:.12}", self.lat_deg.to_radians()));
        f.insert("lon_rad".into(), format!("{:.12}", self.lon_deg.to_radians()));
        f.insert("h_m".into(), format!("{:.3}", self.h_m));
        for (k, v) in ["x", "y", "z"].iter().zip(self.ecef_m) {
            f.insert((*k).into(), format!("{v:.3}"));
        }
        for (k, v) in ["vx", "vy", "vz"].iter().zip(self.vel_ecef_mps) {
            f.insert((*k).into(), format!("{v:.4}"));
        }
        for (k, v) in ["ve", "vn", "vu"].iter().zip(self.vel_enu_mps) {
            f.insert((*k).into(), format!("{v:.4}"));
        }
        for (k, v) in [
            ("yaw", self.yaw_deg),
            ("pitch", self.pitch_deg),
            ("roll", self.roll_deg),
        ] {
            f.insert(format!("{k}_deg"), format!("{v:.4}"));
            f.insert(format!("{k}_rad"), format!("{:.8}", v.to_radians()));
        }
        f
    }
}

/// Motion samples from trajectory samples (equally spaced or not).
pub fn motion_from_traj(samples: &[TrajSample]) -> Vec<MotionSample> {
    let n = samples.len();
    (0..n)
        .map(|i| {
            let s = &samples[i];
            let vu = if n < 2 {
                0.0
            } else {
                let (a, b) = if i == 0 {
                    (0, 1)
                } else if i == n - 1 {
                    (n - 2, n - 1)
                } else {
                    (i - 1, i + 1)
                };
                let dt = samples[b].t_s - samples[a].t_s;
                if dt > 0.0 {
                    (samples[b].h_m - samples[a].h_m) / dt
                } else {
                    0.0
                }
            };
            let hd = s.heading_deg.to_radians();
            let enu = [s.speed_mps * hd.sin(), s.speed_mps * hd.cos(), vu];
            MotionSample {
                t_s: s.t_s,
                utc: s.utc,
                lat_deg: s.lat_deg,
                lon_deg: s.lon_deg,
                h_m: s.h_m,
                ecef_m: llh_to_ecef(s.lat_deg, s.lon_deg, s.h_m),
                vel_enu_mps: enu,
                vel_ecef_mps: enu_to_ecef_vec(s.lat_deg, s.lon_deg, enu),
                yaw_deg: s.heading_deg,
                pitch_deg: vu.atan2(s.speed_mps).to_degrees(),
                roll_deg: 0.0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecef_round_trip_and_known_point() {
        let p = llh_to_ecef(0.0, 0.0, 0.0);
        assert!((p[0] - WGS84_A).abs() < 1e-6 && p[1].abs() < 1e-6 && p[2].abs() < 1e-6);
        let p = llh_to_ecef(90.0, 0.0, 0.0);
        assert!((p[2] - 6_356_752.314_245).abs() < 1e-3);
        for &(la, lo, h) in &[(55.5, -3.25, 10.0), (-33.9, 151.2, 1200.0), (36.0, 128.0, 0.0)] {
            let (a, b, c) = ecef_to_llh(llh_to_ecef(la, lo, h));
            assert!((a - la).abs() < 1e-9 && (b - lo).abs() < 1e-9 && (c - h).abs() < 1e-4);
        }
    }

    #[test]
    fn velocity_north_at_equator_is_plus_z() {
        let v = enu_to_ecef_vec(0.0, 0.0, [0.0, 10.0, 0.0]);
        assert!((v[2] - 10.0).abs() < 1e-12 && v[0].abs() < 1e-12);
        let v = enu_to_ecef_vec(0.0, 90.0, [5.0, 0.0, 0.0]);
        assert!((v[0] + 5.0).abs() < 1e-12);
    }
}
