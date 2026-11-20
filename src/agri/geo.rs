// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! A local east-north-up frame on the WGS 84 ellipsoid, on the open engine's geodetic
//! conversions (`kshana::frames`, `kshana::ppp_real::enu_axes`). Field-scale work (a few
//! kilometres) is done in this plane.

use kshana::frames::{ecef_to_geodetic, geodetic_to_ecef, Geodetic};
use kshana::ppp_real::enu_axes;

type V3 = [f64; 3];

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A local tangent-plane frame at an origin.
#[derive(Clone, Copy, Debug)]
pub struct LocalFrame {
    origin: V3,
    e: V3,
    n: V3,
    u: V3,
    h0: f64,
}

impl LocalFrame {
    /// The frame at `lat_deg`, `lon_deg`, height `h_m` above the ellipsoid.
    pub fn new(lat_deg: f64, lon_deg: f64, h_m: f64) -> Self {
        let g = Geodetic {
            lat_rad: lat_deg.to_radians(),
            lon_rad: lon_deg.to_radians(),
            alt_m: h_m,
        };
        let (e, n, u) = enu_axes(g.lat_rad, g.lon_rad);
        Self {
            origin: geodetic_to_ecef(g),
            e,
            n,
            u,
            h0: h_m,
        }
    }

    /// East, north, up (m) of a point. A missing height is taken as the origin's.
    pub fn enu(&self, lat_deg: f64, lon_deg: f64, h_m: Option<f64>) -> V3 {
        let r = geodetic_to_ecef(Geodetic {
            lat_rad: lat_deg.to_radians(),
            lon_rad: lon_deg.to_radians(),
            alt_m: h_m.unwrap_or(self.h0),
        });
        let d = [
            r[0] - self.origin[0],
            r[1] - self.origin[1],
            r[2] - self.origin[2],
        ];
        [dot(d, self.e), dot(d, self.n), dot(d, self.u)]
    }

    /// East, north (m) of a point at the origin's height.
    pub fn en(&self, lat_deg: f64, lon_deg: f64) -> [f64; 2] {
        let v = self.enu(lat_deg, lon_deg, None);
        [v[0], v[1]]
    }

    /// Latitude, longitude (deg) and height (m) of a local point.
    pub fn geodetic(&self, e: f64, n: f64, u: f64) -> (f64, f64, f64) {
        let r = [
            self.origin[0] + e * self.e[0] + n * self.n[0] + u * self.u[0],
            self.origin[1] + e * self.e[1] + n * self.n[1] + u * self.u[1],
            self.origin[2] + e * self.e[2] + n * self.n[2] + u * self.u[2],
        ];
        let g = ecef_to_geodetic(r);
        (g.lat_rad.to_degrees(), g.lon_rad.to_degrees(), g.alt_m)
    }

    /// Latitude, longitude (deg) of a point on the tangent plane (`u` = 0 at the origin's
    /// height, so a short line keeps its horizontal geometry).
    pub fn latlon(&self, e: f64, n: f64) -> (f64, f64) {
        let (la, lo, _) = self.geodetic(e, n, 0.0);
        (la, lo)
    }
}

/// Azimuth (deg clockwise from north, 0..360) of the vector (de, dn).
pub fn azimuth_deg(de: f64, dn: f64) -> f64 {
    de.atan2(dn).to_degrees().rem_euclid(360.0)
}

/// Unit vector (east, north) of an azimuth.
pub fn unit(az_deg: f64) -> [f64; 2] {
    let a = az_deg.to_radians();
    [a.sin(), a.cos()]
}

/// Ray-casting point-in-polygon on a plane (polygon closed or open).
pub fn inside(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut c = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0];
            if p[0] < x {
                c = !c;
            }
        }
        j = i;
    }
    c
}

/// Shortest distance (m) from a point to a polygon's boundary.
pub fn distance_to_boundary(p: [f64; 2], poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    let mut best = f64::INFINITY;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let ab = [b[0] - a[0], b[1] - a[1]];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1];
        let t = if l2 > 0.0 {
            (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = [a[0] + t * ab[0], a[1] + t * ab[1]];
        best = best.min(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt());
    }
    best
}

/// Shoelace area (m², absolute) of a plane polygon.
pub fn area(poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    (s / 2.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_distances() {
        let f = LocalFrame::new(49.9, -97.1, 230.0);
        let (la, lo) = f.latlon(500.0, -300.0);
        let v = f.en(la, lo);
        assert!(
            (v[0] - 500.0).abs() < 1e-3 && (v[1] + 300.0).abs() < 1e-3,
            "{v:?}"
        );
        // 0.001 deg of latitude is about 111 m.
        let w = f.en(49.901, -97.1);
        assert!((w[1] - 111.2).abs() < 0.3, "{w:?}");
        assert!((azimuth_deg(1.0, 0.0) - 90.0).abs() < 1e-12);
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!(inside([5.0, 5.0], &sq) && !inside([15.0, 5.0], &sq));
        assert!((area(&sq) - 100.0).abs() < 1e-12);
        assert!((distance_to_boundary([15.0, 5.0], &sq) - 5.0).abs() < 1e-12);
    }
}
