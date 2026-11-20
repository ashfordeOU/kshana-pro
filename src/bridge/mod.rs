// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Ship bridge consistency, dead reckoning and drills (research 2026-11-20 gaps, epic E08)
//!
//! Decision support for a ship's crew and its operator, from what the bridge already records:
//!
//! * [`consistency`]: a cross-sensor consistency aid over recorded IEC 61162-1 (NMEA 0183)
//!   sentences, plain or in IEC 61162-450 datagrams. GNSS position, speed and course, a GNSS
//!   compass, the gyro, the speed log, the echo-sounder, radar tracked targets of charted
//!   fixed marks and a second GNSS receiver are compared pair by pair; the result is a
//!   consistency score per epoch and alerts that name the sensor the data implicates, with
//!   the checks behind it (`kshana-pro bridge check`).
//! * [`dr`]: the dead-reckoning error budget during a GNSS outage (log bias and scale, gyro
//!   bias and drift, unknown current, initial position), with a deterministic Monte Carlo
//!   check of the closed form (`kshana-pro bridge dr`), and an ECDIS estimated-position (EP)
//!   fallback tester: stimulus vectors and a checker of what an ECDIS recorded
//!   (`kshana-pro bridge ep-vectors`, `ep-check`).
//! * [`drills`]: GNSS-denied drill packs for bridge simulators, built from scripted event
//!   sequences through the open engine's training NMEA generator, each with an instructor
//!   log, an assessment rubric and scoring, and a training record for the safety management
//!   system (`kshana-pro bridge drills`).
//!
//! **Advisory only.** Nothing here is navigation equipment, gives a position to steer by, or
//! tells the crew what to do with the ship. Outputs are labelled MODELLED: the rules are
//! exercised on synthetic logs with known truth and have not been validated against a
//! labelled set of real bridge recordings. IMO and IEC documents are named only as the
//! criteria a result is measured against.

pub mod consistency;
pub mod dr;
pub mod drills;
pub mod ep;
pub mod nmea;

/// The licence feature of `kshana-pro bridge check`.
pub const FEATURE_CONSISTENCY: &str = "pro-bridge-consistency";
/// The licence feature of `kshana-pro bridge ep-check`.
pub const FEATURE_EP_TEST: &str = "pro-ecdis-ep-test";
/// The licence feature of `kshana-pro bridge drills build|score`.
pub const FEATURE_DRILLS: &str = "pro-bridge-drills";

/// The evidence label every bridge output carries.
pub const LABEL: &str = "MODELLED: rules exercised on synthetic bridge logs with known truth; not \
validated against a labelled set of real bridge recordings.";

/// What every bridge output says about itself.
pub const ADVISORY: &str = "Decision support for the crew and the operator. Not navigation \
equipment, not type-approved, and not a position or an instruction to navigate by; the officer \
of the watch keeps every decision.";

/// Round to nine significant digits, so figures that feed a hash or a signature are stable
/// across platforms.
pub fn sig9(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    format!("{x:.8e}").parse().unwrap_or(x)
}

/// Mean Earth radius used for the local geometry, metres (as the open engine's grid).
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Great-circle distance, metres.
pub fn distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    kshana::interference_map::grid::haversine_m(lat1, lon1, lat2, lon2)
}

/// Initial great-circle bearing from point 1 to point 2, degrees true in [0, 360).
pub fn bearing_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    y.atan2(x).to_degrees().rem_euclid(360.0)
}

/// Move a point by `dn` metres north and `de` metres east (local flat Earth; the steps here
/// are seconds of ship motion).
pub fn move_ne(lat: f64, lon: f64, dn: f64, de: f64) -> (f64, f64) {
    let dlat = (dn / EARTH_RADIUS_M).to_degrees();
    let c = lat.to_radians().cos().max(1e-9);
    let dlon = (de / (EARTH_RADIUS_M * c)).to_degrees();
    (lat + dlat, lon + dlon)
}

/// The smallest signed difference `a - b` between two angles, degrees in (-180, 180].
pub fn ang_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    if d > 180.0 {
        d - 360.0
    } else {
        d
    }
}

/// One knot in metres per second.
pub const KN_MPS: f64 = 1852.0 / 3600.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_significant_digits() {
        assert_eq!(sig9(1.234_567_891_23), 1.234_567_89);
        assert_eq!(sig9(-0.000_123_456_789_9), -0.000_123_456_790);
        assert_eq!(sig9(0.0), 0.0);
    }

    #[test]
    fn geometry_round_trips() {
        let (la, lo) = move_ne(51.0, 1.4, 1000.0, 0.0);
        assert!((distance_m(51.0, 1.4, la, lo) - 1000.0).abs() < 0.5);
        assert!(bearing_deg(51.0, 1.4, la, lo).abs() < 1e-6);
        let (la, lo) = move_ne(51.0, 1.4, 0.0, 1000.0);
        assert!((bearing_deg(51.0, 1.4, la, lo) - 90.0).abs() < 0.01);
        assert_eq!(ang_diff(359.0, 1.0), -2.0);
        assert_eq!(ang_diff(1.0, 359.0), 2.0);
    }
}
