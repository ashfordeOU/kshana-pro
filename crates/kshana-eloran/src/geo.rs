// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! WGS-84 geodesy for LF propagation paths: Vincenty's inverse (distance and azimuths) and
//! direct (point at a distance along an azimuth) formulae on the ellipsoid, accurate to
//! well under a millimetre for the path lengths eLoran uses (tens to ~2,000 km).

use serde::{Deserialize, Serialize};

pub const WGS84_A: f64 = 6_378_137.0;
pub const WGS84_F: f64 = 1.0 / 298.257_223_563;
pub const WGS84_B: f64 = WGS84_A * (1.0 - WGS84_F);

/// A geodetic position (degrees, metres above the ellipsoid).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Geo {
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub h_m: f64,
}

impl Geo {
    pub fn new(lat_deg: f64, lon_deg: f64) -> Geo {
        Geo { lat_deg, lon_deg, h_m: 0.0 }
    }
}

/// Result of the inverse problem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inverse {
    /// Geodesic distance (m).
    pub distance_m: f64,
    /// Forward azimuth at the first point (rad, clockwise from north).
    pub az1_rad: f64,
    /// Forward azimuth at the second point (rad).
    pub az2_rad: f64,
}

/// Vincenty's inverse formula. Falls back to a spherical solution for near-antipodal
/// points where the iteration does not converge (not a case eLoran paths reach).
pub fn inverse(p1: Geo, p2: Geo) -> Inverse {
    let (a, b, f) = (WGS84_A, WGS84_B, WGS84_F);
    let l = (p2.lon_deg - p1.lon_deg).to_radians();
    let u1 = ((1.0 - f) * p1.lat_deg.to_radians().tan()).atan();
    let u2 = ((1.0 - f) * p2.lat_deg.to_radians().tan()).atan();
    let (su1, cu1) = u1.sin_cos();
    let (su2, cu2) = u2.sin_cos();
    let mut lambda = l;
    let mut iter = 0;
    let (mut s_sig, mut c_sig, mut sig, mut c2a, mut c2sm);
    loop {
        let (sl, cl) = lambda.sin_cos();
        s_sig = ((cu2 * sl).powi(2) + (cu1 * su2 - su1 * cu2 * cl).powi(2)).sqrt();
        if s_sig == 0.0 {
            return Inverse { distance_m: 0.0, az1_rad: 0.0, az2_rad: 0.0 };
        }
        c_sig = su1 * su2 + cu1 * cu2 * cl;
        sig = s_sig.atan2(c_sig);
        let sa = cu1 * cu2 * sl / s_sig;
        c2a = 1.0 - sa * sa;
        c2sm = if c2a != 0.0 { c_sig - 2.0 * su1 * su2 / c2a } else { 0.0 };
        let c = f / 16.0 * c2a * (4.0 + f * (4.0 - 3.0 * c2a));
        let prev = lambda;
        lambda = l
            + (1.0 - c)
                * f
                * sa
                * (sig + c * s_sig * (c2sm + c * c_sig * (-1.0 + 2.0 * c2sm * c2sm)));
        iter += 1;
        if (lambda - prev).abs() < 1e-12 || iter > 200 {
            break;
        }
    }
    if iter > 200 {
        // spherical fallback
        let r = (a + b) / 2.0;
        let d = sig * r;
        return Inverse { distance_m: d, az1_rad: 0.0, az2_rad: 0.0 };
    }
    let u2s = c2a * (a * a - b * b) / (b * b);
    let ka = 1.0 + u2s / 16384.0 * (4096.0 + u2s * (-768.0 + u2s * (320.0 - 175.0 * u2s)));
    let kb = u2s / 1024.0 * (256.0 + u2s * (-128.0 + u2s * (74.0 - 47.0 * u2s)));
    let dsig = kb
        * s_sig
        * (c2sm
            + kb / 4.0
                * (c_sig * (-1.0 + 2.0 * c2sm * c2sm)
                    - kb / 6.0 * c2sm * (-3.0 + 4.0 * s_sig * s_sig) * (-3.0 + 4.0 * c2sm * c2sm)));
    let s = b * ka * (sig - dsig);
    let (sl, cl) = lambda.sin_cos();
    let az1 = (cu2 * sl).atan2(cu1 * su2 - su1 * cu2 * cl);
    let az2 = (cu1 * sl).atan2(-su1 * cu2 + cu1 * su2 * cl);
    Inverse {
        distance_m: s,
        az1_rad: az1.rem_euclid(std::f64::consts::TAU),
        az2_rad: az2.rem_euclid(std::f64::consts::TAU),
    }
}

/// Vincenty's direct formula: the point `distance_m` from `p1` along azimuth `az1_rad`.
pub fn direct(p1: Geo, az1_rad: f64, distance_m: f64) -> Geo {
    let (a, b, f) = (WGS84_A, WGS84_B, WGS84_F);
    let (sa1, ca1) = az1_rad.sin_cos();
    let tu1 = (1.0 - f) * p1.lat_deg.to_radians().tan();
    let cu1 = 1.0 / (1.0 + tu1 * tu1).sqrt();
    let su1 = tu1 * cu1;
    let sig1 = tu1.atan2(ca1);
    let sa = cu1 * sa1;
    let c2a = 1.0 - sa * sa;
    let u2s = c2a * (a * a - b * b) / (b * b);
    let ka = 1.0 + u2s / 16384.0 * (4096.0 + u2s * (-768.0 + u2s * (320.0 - 175.0 * u2s)));
    let kb = u2s / 1024.0 * (256.0 + u2s * (-128.0 + u2s * (74.0 - 47.0 * u2s)));
    let mut sig = distance_m / (b * ka);
    let (mut s_sig, mut c_sig, mut c2sm);
    let mut iter = 0;
    loop {
        c2sm = (2.0 * sig1 + sig).cos();
        s_sig = sig.sin();
        c_sig = sig.cos();
        let dsig = kb
            * s_sig
            * (c2sm
                + kb / 4.0
                    * (c_sig * (-1.0 + 2.0 * c2sm * c2sm)
                        - kb / 6.0
                            * c2sm
                            * (-3.0 + 4.0 * s_sig * s_sig)
                            * (-3.0 + 4.0 * c2sm * c2sm)));
        let prev = sig;
        sig = distance_m / (b * ka) + dsig;
        iter += 1;
        if (sig - prev).abs() < 1e-12 || iter > 200 {
            break;
        }
    }
    let x = su1 * s_sig - cu1 * c_sig * ca1;
    let lat2 = (su1 * c_sig + cu1 * s_sig * ca1).atan2((1.0 - f) * (sa * sa + x * x).sqrt());
    let lambda = (s_sig * sa1).atan2(cu1 * c_sig - su1 * s_sig * ca1);
    let c = f / 16.0 * c2a * (4.0 + f * (4.0 - 3.0 * c2a));
    let l = lambda
        - (1.0 - c) * f * sa * (sig + c * s_sig * (c2sm + c * c_sig * (-1.0 + 2.0 * c2sm * c2sm)));
    let lon2 = p1.lon_deg.to_radians() + l;
    Geo {
        lat_deg: lat2.to_degrees(),
        lon_deg: ((lon2.to_degrees() + 540.0).rem_euclid(360.0)) - 180.0,
        h_m: 0.0,
    }
}

/// Points along the geodesic from `p1` to `p2` at `n + 1` equally spaced stations
/// (including both ends), with the distance of each from `p1`.
pub fn path_points(p1: Geo, p2: Geo, n: usize) -> Vec<(f64, Geo)> {
    let inv = inverse(p1, p2);
    let n = n.max(1);
    (0..=n)
        .map(|i| {
            let d = inv.distance_m * i as f64 / n as f64;
            (d, if i == 0 { p1 } else { direct(p1, inv.az1_rad, d) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vincenty_reference_flinders_buninyong() {
        // Vincenty's published test: Flinders Peak to Buninyong, 54 972.271 m.
        let p1 = Geo::new(-(37.0 + 57.0 / 60.0 + 3.72030 / 3600.0), 144.0 + 25.0 / 60.0 + 29.52440 / 3600.0);
        let p2 = Geo::new(-(37.0 + 39.0 / 60.0 + 10.15610 / 3600.0), 143.0 + 55.0 / 60.0 + 35.38390 / 3600.0);
        let inv = inverse(p1, p2);
        assert!((inv.distance_m - 54_972.271).abs() < 0.01, "{}", inv.distance_m);
        let az_deg = inv.az1_rad.to_degrees();
        let expect = 306.0 + 52.0 / 60.0 + 5.37 / 3600.0;
        assert!((az_deg - expect).abs() < 1e-4, "{az_deg}");
    }

    #[test]
    fn direct_inverts_inverse() {
        let p1 = Geo::new(54.9, -3.28);
        let p2 = Geo::new(49.1, -1.5);
        let inv = inverse(p1, p2);
        let q = direct(p1, inv.az1_rad, inv.distance_m);
        assert!((q.lat_deg - p2.lat_deg).abs() < 1e-8 && (q.lon_deg - p2.lon_deg).abs() < 1e-8);
    }
}
