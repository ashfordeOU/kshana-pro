// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Timing assurance (roadmap 0.42, family T)
//!
//! * **T1** ([`model`], [`fleet`]): holdover and time to breach for every clock of a
//!   mixed-vendor fleet, each read from its own I2 source through the open engine's clock-log
//!   readers, with a per-clock model and its uncertainty, against limits the operator sets.
//!   Kind `pro-timing-holdover`.
//! * **T2** ([`evidence`], [`console`], [`metrics`]): the per-clock evidence history (hash
//!   chained), the fleet page served by the Station console, and the Prometheus exposition
//!   behind the Grafana dashboard `deploy/grafana/kshana-pro-timing-fleet.json`. Feature
//!   `pro-timing-fleet`.
//! * **T3** ([`audit`]): a signed audit report mapped to MiFID II RTS 25 traceability or to the
//!   NIST IR 8323 PNT profile. Feature `pro-timing-audit`. Verifying a report is free.
//!
//! The prediction method is pre-registered and scored as evidence row V5
//! (`docs/evidence/v5-holdover-time-to-breach/`); [`V5_PASSED`] records the published
//! outcome and every result carries it in its label. Docs: `docs/TIMING-ASSURANCE.md`.

pub mod audit;
pub mod console;
pub mod evidence;
pub mod fleet;
pub mod metrics;
pub mod model;

/// The scenario kind of a fleet assessment.
pub const KIND: &str = "pro-timing-holdover";
/// The command feature of the fleet page, metrics and evidence history.
pub const FLEET_FEATURE: &str = "pro-timing-fleet";
/// The command feature of the audit export.
pub const AUDIT_FEATURE: &str = "pro-timing-audit";

/// Whether the V5 pre-registered bars passed on both data sets (RESULTS.md). T1 is called
/// claimed only when this is true; `tests/timing_v5.rs` keeps it equal to the scored result.
pub const V5_PASSED: bool = false;

/// The V5 status in words, for labels and reports.
pub fn v5_status() -> &'static str {
    if V5_PASSED {
        "prediction bars pre-registered as V5 and passed on recorded clocks"
    } else {
        "prediction bars pre-registered as V5 and NOT passed: the time to breach is not a validated claim"
    }
}

/// The honesty label of every timing result.
pub fn label() -> String {
    format!(
        "MODELLED - holdover predictions from each clock's own record (or a stated prior for a servo record) under a three-term clock noise model; {}.",
        v5_status()
    )
}
