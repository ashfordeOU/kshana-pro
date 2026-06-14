// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # L2 — Export-sensitive resilience pack  *(GATED — not built on spec)*
//!
//! The closed counterpart to the open jam/spoof models: real threat libraries,
//! adversary waveforms, anti-jam/anti-spoof performance, and restricted resilience
//! scenarios, behind export-control and need-to-know access.
//!
//! **Attaches via:** [`kshana::models::ErrorModel`] (real-parameter degradation
//! models) and [`kshana::api::ExternalPack`] (access-controlled scenario packs) —
//! the same seams the open `jamming`/`spoof` packs use, with closed parameters.
//!
//! **Trigger (do NOT implement before this exists):** a completed **export-control
//! (dual-use) assessment** (BUILD-TRACKER PR5 / `legal/EXPORT-CONTROL-ASSESSMENT.md`)
//! **and** a cleared customer. See BUILD-TRACKER PR12–PR14.
//!
//! **Why closed:** dual-use / export-controlled by law (EU Reg 2021/821; German
//! AWV/BAFA). Real threat data legally cannot be open.
//!
//! Open-core discipline: seam guard only — no real threat data, no waveforms, no
//! performance models live here until the gate clears.

use kshana::api::KshanaError;

/// Stable reason string for the gate, so callers/tests can assert on it.
pub const GATE_REASON: &str =
    "L2 export-sensitive resilience is gated on the dual-use export-control assessment (PR5) \
     plus a cleared customer; not built on spec. See BUILD-TRACKER PR12–PR14.";

/// The seam guard. Refuses to run until the export-control gate and a cleared
/// customer exist — fails closed rather than exposing fabricated threat models.
pub fn gated() -> KshanaError {
    KshanaError::Unsupported(GATE_REASON.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_is_gated_with_an_honest_reason() {
        match gated() {
            KshanaError::Unsupported(msg) => {
                assert!(msg.contains("export-control"));
                assert!(msg.contains("not built on spec"));
            }
            other => panic!("L2 gate must be Unsupported, got {other:?}"),
        }
    }
}
