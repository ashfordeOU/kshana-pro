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

pub mod asf_fde;
pub mod asf_learn;
pub mod meacon;

/// The quantiles moved to [`crate::stats`] (used outside the gated guards too).
pub use crate::stats::{chi2_quantile, normal_quantile};

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
