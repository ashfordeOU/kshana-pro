// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Receiver-side signal guards for eLoran (W1119 programme, patent candidates E3/E4):
//!
//! * [`meacon`]: meacon / relay detection from the consistency of a station's measured
//!   propagation signature (ECD, field strength, skywave structure, bearing, phase-code
//!   alignment, timing and cross-station TOA geometry) with the signature the propagation
//!   model predicts for the claimed transmitter-to-receiver path (E4);
//! * [`asf_learn`]: spoof-safe online ASF learning from GNSS-derived truth, gated by the
//!   timing protection level and the cross-system attribution alarm, with a common-mode
//!   guard, a physical rate cap, verified checkpoints and rollback (E3).
//!
//! CONFIDENTIAL: these mechanisms are patent candidates (research 04 §2). Kshana Pro only;
//! never move them into the open engine or describe them publicly before counsel decides.
//!
//! Evidence label: MODELLED (checked by the internal tests of each module; no external data).

pub mod asf_learn;
pub mod meacon;

/// Standard normal quantile Φ⁻¹(p) (Acklam's rational approximation, |ε| < 1.2e-9).
pub fn normal_quantile(p: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) {
        return if p <= 0.0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    const A: [f64; 6] = [
        -3.969683028665376e1,
        2.209460984245205e2,
        -2.759285104469687e2,
        1.38357751867269e2,
        -3.066479806614716e1,
        2.506628277459239e0,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e1,
        1.615858368580409e2,
        -1.556989798598866e2,
        6.680131188771972e1,
        -1.328068155288572e1,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-3,
        -3.223964580411365e-1,
        -2.400758277161838e0,
        -2.549732539343734e0,
        4.374664141464968e0,
        2.938163982698783e0,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-3,
        3.224671290700398e-1,
        2.445134137142996e0,
        3.754408661907416e0,
    ];
    let pl = 0.02425;
    if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        -normal_quantile(1.0 - p)
    }
}

/// χ² quantile with `k` degrees of freedom at probability `p`: exact for k = 1, 2;
/// Wilson–Hilferty above (slightly conservative in the far tail for small k).
pub fn chi2_quantile(p: f64, k: usize) -> f64 {
    match k {
        0 => 0.0,
        1 => {
            let z = normal_quantile(0.5 + p / 2.0);
            z * z
        }
        2 => -2.0 * (1.0 - p).ln(),
        _ => {
            let k = k as f64;
            let z = normal_quantile(p);
            let a = 2.0 / (9.0 * k);
            k * (1.0 - a + z * a.sqrt()).powi(3)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_match_tabulated_values() {
        assert!((normal_quantile(0.975) - 1.959964).abs() < 1e-5);
        assert!((normal_quantile(0.001) + 3.090232).abs() < 1e-5);
        // χ² table values (k, p, quantile, tolerance)
        for (k, p, q, tol) in [
            (1, 0.999, 10.828, 0.01),
            (2, 0.999, 13.816, 0.01),
            (3, 0.99, 11.345, 0.1),
            (4, 0.999, 18.467, 0.3),
            (6, 0.95, 12.592, 0.1),
        ] {
            let v = chi2_quantile(p, k);
            assert!((v - q).abs() < tol, "χ²({k}, {p}) = {v}, table {q}");
            if p >= 0.99 {
                assert!(v >= q - 0.02, "not conservative in the tail: {k} {p} {v}");
            }
        }
    }
}
