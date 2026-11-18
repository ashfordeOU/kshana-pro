// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Millington's method for mixed (piecewise-homogeneous) groundwave paths, applied to the
//! field strength (Millington 1949) and to the phase and group delay (the
//! "Millington–Pressey" use, below).
//!
//! A path from the transmitter T to the receiver R crosses `N` sections of homogeneous
//! ground, section `i` ending at distance `D_i` from T (`D_0 = 0`, `D_N = D`). With
//! `C_i(d)` any additive groundwave quantity over a homogeneous path of section `i`'s
//! ground (here: `ln|W|`, the phase delay `−arg W/ω` and the group delay), with
//! `C_i(0) = 0`:
//!
//! ```text
//! forward  C_T→R = C_1(D_1) − C_2(D_1) + C_2(D_2) − … + C_N(D_N)
//!               = Σ_i [C_i(D_i) − C_i(D_{i−1})]
//! reverse  C_R→T = the same with the sections taken from R (distances D − D_i)
//! Millington C = ½ (C_T→R + C_R→T)
//! ```
//!
//! Averaging the two directions makes the result reciprocal by construction (Millington,
//! "Ground-wave propagation over an inhomogeneous smooth earth", Proc. IEE 96(III), 1949;
//! ITU-R P.368 §"mixed paths"). Applied to amplitude it reproduces the "recovery effect"
//! after a land-to-sea boundary. **Phase** (the "Millington–Pressey" use): Pressey,
//! Ashwell and Fowler (Proc. IEE, 1950s, measurements of the 100 kHz ground-wave phase
//! across coastlines) found that Millington's construction applied to the phase lag
//! reproduces the measured phase recovery; it is the standard LORAN ASF practice (the
//! Millington method of the Loran-C User Handbook and of Johler/Samaddar ASF work).
//! The same linear construction is applied here to the group delay (for the ECD).
//!
//! MODELLED (the method is an approximation of the exact mixed-path integral equation;
//! its accuracy is typically better than 1 dB and a few tens of ns at LF).

use super::smooth_earth::CurvePoint;

/// One homogeneous section of a path: ground-type index and the distance (m) from the
/// transmitter at which the section ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    pub ground: usize,
    pub end_m: f64,
}

fn add(a: CurvePoint, b: CurvePoint, sgn: f64) -> CurvePoint {
    CurvePoint {
        ln_amp: a.ln_amp + sgn * b.ln_amp,
        phase_delay_s: a.phase_delay_s + sgn * b.phase_delay_s,
        group_delay_s: a.group_delay_s + sgn * b.group_delay_s,
    }
}

/// One-direction Millington sum for sections given as (ground, start, end) distances
/// measured from the launching end.
fn one_way<F: Fn(usize, f64) -> CurvePoint>(secs: &[(usize, f64, f64)], curve: &F) -> CurvePoint {
    let mut acc = CurvePoint::default();
    for &(g, a, b) in secs {
        acc = add(acc, curve(g, b), 1.0);
        if a > 0.0 {
            acc = add(acc, curve(g, a), -1.0);
        }
    }
    acc
}

/// Millington's method over `sections` (in order from the transmitter, contiguous, the last
/// ending at the path length). `curve(g, d)` gives ground `g`'s homogeneous-path values.
pub fn millington<F: Fn(usize, f64) -> CurvePoint>(sections: &[Section], curve: F) -> CurvePoint {
    if sections.is_empty() {
        return CurvePoint::default();
    }
    let total = sections.last().map(|s| s.end_m).unwrap_or(0.0);
    let mut fwd = Vec::with_capacity(sections.len());
    let mut start = 0.0;
    for s in sections {
        fwd.push((s.ground, start, s.end_m));
        start = s.end_m;
    }
    let rev: Vec<(usize, f64, f64)> = fwd.iter().rev().map(|&(g, a, b)| (g, total - b, total - a)).collect();
    let f = one_way(&fwd, &curve);
    let r = one_way(&rev, &curve);
    CurvePoint {
        ln_amp: 0.5 * (f.ln_amp + r.ln_amp),
        phase_delay_s: 0.5 * (f.phase_delay_s + r.phase_delay_s),
        group_delay_s: 0.5 * (f.group_delay_s + r.group_delay_s),
    }
}

/// Merge per-sample ground indices (samples equally spaced over `[0, total_m]`, both ends
/// included) into sections, with boundaries half-way between samples of different ground.
pub fn sections_from_samples(grounds: &[usize], total_m: f64) -> Vec<Section> {
    let n = grounds.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![Section { ground: grounds[0], end_m: total_m }];
    }
    let step = total_m / (n - 1) as f64;
    let mut out = Vec::new();
    for i in 1..n {
        if grounds[i] != grounds[i - 1] {
            out.push(Section { ground: grounds[i - 1], end_m: (i as f64 - 0.5) * step });
        }
    }
    out.push(Section { ground: grounds[n - 1], end_m: total_m });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(g: usize, d: f64) -> CurvePoint {
        // toy curves: different, nonlinear in d
        let k = [1.0, 3.0, 0.5][g];
        CurvePoint { ln_amp: -k * d * d * 1e-12, phase_delay_s: k * d.sqrt() * 1e-9, group_delay_s: k * d * 1e-15 }
    }

    #[test]
    fn homogeneous_path_is_exact() {
        let s = [Section { ground: 1, end_m: 4e5 }];
        let m = millington(&s, lin);
        let e = lin(1, 4e5);
        assert!((m.phase_delay_s - e.phase_delay_s).abs() < 1e-18);
        // split into two sections of the same ground: still exact
        let s2 = [Section { ground: 1, end_m: 1e5 }, Section { ground: 1, end_m: 4e5 }];
        let m2 = millington(&s2, lin);
        assert!((m2.phase_delay_s - e.phase_delay_s).abs() < 1e-18);
        assert!((m2.ln_amp - e.ln_amp).abs() < 1e-15);
    }

    #[test]
    fn reciprocity_two_sections() {
        let ab = [Section { ground: 0, end_m: 1.5e5 }, Section { ground: 1, end_m: 6e5 }];
        let ba = [Section { ground: 1, end_m: 4.5e5 }, Section { ground: 0, end_m: 6e5 }];
        let x = millington(&ab, lin);
        let y = millington(&ba, lin);
        assert!((x.phase_delay_s - y.phase_delay_s).abs() < 1e-18);
        assert!((x.ln_amp - y.ln_amp).abs() < 1e-15);
        assert!((x.group_delay_s - y.group_delay_s).abs() < 1e-18);
    }

    #[test]
    fn sections_merge() {
        let s = sections_from_samples(&[0, 0, 1, 1, 1, 0], 5.0);
        assert_eq!(s, vec![
            Section { ground: 0, end_m: 1.5 },
            Section { ground: 1, end_m: 4.5 },
            Section { ground: 0, end_m: 5.0 }
        ]);
    }
}
