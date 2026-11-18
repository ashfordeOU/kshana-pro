// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Receiver trajectories: static, or waypoints with linear interpolation. Heading comes
//! from the waypoint when given, otherwise from the course between waypoints.

use crate::geo::{inverse, Geo};
use serde::{Deserialize, Serialize};

/// Receiver state at an instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RxState {
    /// Scene time (s).
    pub t_s: f64,
    pub pos: Geo,
    /// Heading (rad, clockwise from true north).
    pub heading_rad: f64,
    /// Speed over ground (m/s).
    pub speed_mps: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub t_s: f64,
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub h_m: f64,
    #[serde(default)]
    pub heading_deg: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trajectory {
    Static {
        lat_deg: f64,
        lon_deg: f64,
        #[serde(default)]
        h_m: f64,
        #[serde(default)]
        heading_deg: f64,
    },
    Waypoints { points: Vec<Waypoint> },
}

impl Trajectory {
    pub fn validate(&self) -> Result<(), String> {
        if let Trajectory::Waypoints { points } = self {
            if points.len() < 2 {
                return Err("a waypoint trajectory needs at least two points".into());
            }
            if points.windows(2).any(|w| w[1].t_s <= w[0].t_s) {
                return Err("waypoint times must increase".into());
            }
        }
        Ok(())
    }

    pub fn state(&self, t_s: f64) -> RxState {
        match self {
            Trajectory::Static { lat_deg, lon_deg, h_m, heading_deg } => RxState {
                t_s,
                pos: Geo { lat_deg: *lat_deg, lon_deg: *lon_deg, h_m: *h_m },
                heading_rad: heading_deg.to_radians(),
                speed_mps: 0.0,
            },
            Trajectory::Waypoints { points } => {
                let i = match points.iter().position(|p| p.t_s > t_s) {
                    Some(0) => 0,
                    Some(i) => i - 1,
                    None => points.len() - 2,
                };
                let (a, b) = (&points[i], &points[i + 1]);
                let f = ((t_s - a.t_s) / (b.t_s - a.t_s)).clamp(0.0, 1.0);
                let pos = Geo {
                    lat_deg: a.lat_deg + f * (b.lat_deg - a.lat_deg),
                    lon_deg: a.lon_deg + f * (b.lon_deg - a.lon_deg),
                    h_m: a.h_m + f * (b.h_m - a.h_m),
                };
                let inv = inverse(Geo::new(a.lat_deg, a.lon_deg), Geo::new(b.lat_deg, b.lon_deg));
                let heading_rad = match (a.heading_deg, b.heading_deg) {
                    (Some(x), Some(y)) => (x + f * (y - x)).to_radians(),
                    (Some(x), None) => x.to_radians(),
                    _ => inv.az1_rad,
                };
                RxState { t_s, pos, heading_rad, speed_mps: inv.distance_m / (b.t_s - a.t_s) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waypoints_interpolate_and_head_along_course() {
        let t = Trajectory::Waypoints {
            points: vec![
                Waypoint { t_s: 0.0, lat_deg: 50.0, lon_deg: 0.0, h_m: 0.0, heading_deg: None },
                Waypoint { t_s: 100.0, lat_deg: 50.01, lon_deg: 0.0, h_m: 0.0, heading_deg: None },
            ],
        };
        t.validate().unwrap();
        let s = t.state(50.0);
        assert!((s.pos.lat_deg - 50.005).abs() < 1e-12);
        assert!(s.heading_rad.abs() < 1e-6); // due north
        assert!((s.speed_mps - 11.13).abs() < 0.05);
    }
}
