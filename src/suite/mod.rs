// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # L3 — Kshana Pro suite: trade-study & assurance workflow  *(GATED — not built on spec)*
//!
//! The proprietary tooling that turns Kshana runs into a deliverable: automated
//! quantum-vs-classical architecture trades across the design space, wired into
//! ECSS systems-engineering artifacts (requirements traceability, budgets/margins,
//! SRD/SDR inputs) and audit-grade reproducible evidence packs for agency reviews
//! (PRR/ISRR / DO-178-style).
//!
//! **Attaches via:** [`kshana::api::ExternalPack`] (the suite is a meta-pack that
//! orchestrates open runs) layered on the open `sweep` engine — the open engine is
//! the calculator; L3 is the workflow a study contractor is paid to run.
//!
//! **Trigger (do NOT implement before this exists):** a **second paying customer**
//! (productise once recurring demand is proven). See BUILD-TRACKER PR15–PR18.
//!
//! **Why closed:** proprietary methodology + tooling — the recurring-revenue SKU.
//!
//! Open-core discipline: seam guard only — no trade-study engine, MBSE generator,
//! or evidence-pack tooling is implemented on spec.

use kshana::api::KshanaError;

/// Stable reason string for the gate, so callers/tests can assert on it.
pub const GATE_REASON: &str =
    "L3 trade-study & assurance suite is gated on a second paying customer; not built on spec. \
     See BUILD-TRACKER PR15–PR18.";

/// The seam guard. The suite is not implemented until demand is proven.
pub fn gated() -> KshanaError {
    KshanaError::Unsupported(GATE_REASON.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l3_is_gated_with_an_honest_reason() {
        match gated() {
            KshanaError::Unsupported(msg) => {
                assert!(msg.contains("second paying customer"));
                assert!(msg.contains("not built on spec"));
            }
            other => panic!("L3 gate must be Unsupported, got {other:?}"),
        }
    }
}
