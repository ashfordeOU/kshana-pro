// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # L1 — Quantum-payload digital twin  *(GATED — not built on spec)*
//!
//! Calibrate Kshana under NDA to a customer/partner's **actual** quantum clock or
//! cold-atom interferometer (their lab and, later, flight data) to simulate *that*
//! payload's real mission-level PNT performance.
//!
//! **Attaches via:** [`kshana::models::ErrorModel`] — a calibrated device twin is
//! an `ErrorModel` whose coefficients are fitted to the partner's characterisation
//! data, dropped into the existing clock/inertial run paths unchanged.
//!
//! **Trigger (do NOT implement before this exists):** an NDA-covered hardware
//! partner **and** funding — first instance is contract-funded Foreground IP under
//! an ESA award (AO/1-13503). See `BUILD-TRACKER` PR8–PR11 and
//! `docs/OVERLAY-ARCHITECTURE.md`.
//!
//! **Why closed:** device characterisation data is customer-confidential and
//! likely export-controlled.
//!
//! Open-core discipline: this module is a *seam guard*, not a capability. It exists
//! so the architecture is navigable and so any premature call fails honestly.

use kshana::api::KshanaError;

/// Stable reason string for the gate, so callers/tests can assert on it.
pub const GATE_REASON: &str =
    "L1 quantum-payload digital twin is contract/partner-gated (NDA hardware partner + funding); \
     not built on spec. See BUILD-TRACKER PR8–PR11.";

/// The seam guard. Any attempt to use L1 before the trigger exists fails with a
/// structured, honest error rather than silently returning a fabricated twin.
pub fn gated() -> KshanaError {
    KshanaError::Unsupported(GATE_REASON.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l1_is_gated_with_an_honest_reason() {
        match gated() {
            KshanaError::Unsupported(msg) => {
                assert!(msg.contains("contract/partner-gated"));
                assert!(msg.contains("not built on spec"));
            }
            other => panic!("L1 gate must be Unsupported, got {other:?}"),
        }
    }
}
