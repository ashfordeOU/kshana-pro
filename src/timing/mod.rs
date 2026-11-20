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
//! * **T4** ([`scorecard`]): timing scorecards for grandmasters and the PTP network behind them
//!   under threat library timing scenarios, mapped to the resilience levels with the E3 scoring.
//!   Kind `pro-timing-scorecard`.
//! * **TA1** ([`timebomb`]): future-epoch replay vectors (NMEA, UBX, simulator sheets), the
//!   analyser of a device's output during a replay, and the fleet check against epoch hazards
//!   and a vendor-advisory table. Feature `pro-timing-timebomb`.
//! * **TA2** ([`selfcheck`]): a node's time voted against its independent sources (the time is
//!   wrong although GNSS looks healthy), feature `pro-timing-selfcheck`; one site diverging
//!   from its neighbours or operators, feature `pro-timing-correlation` (Enterprise).
//! * **TA3** ([`holdover_evidence`]): holdover time-error growth from the oscillator's
//!   characterisation with a GUM budget, against a limit for a stated period (Finland's
//!   two-week rule), signed. Feature `pro-timing-holdover-evidence`.
//! * **TA4** ([`compliance`]): the RTS 25 pack's cable-delay and holdover items and the ITU-T
//!   G.8271.1 network-limit pack, under the audit feature.
//! * **E31** ([`grid`], [`sfn`]): power-grid timing evidence (PMU phase-angle error, gradual
//!   spoof detection, SCADA time stamps, PRC-002 evidence with holdover resilience, event
//!   forensics), feature `pro-grid-timing`; broadcast SFN timing (pair guard margins and
//!   UTC-level scenarios such as SVN-23), feature `pro-sfn-timing`.
//! * **E19 timing networks**: the appliance security posture (advisories, vendor SBOM
//!   components, management reach, firmware-image checks, CycloneDX SBOMs; [`posture`],
//!   feature `pro-timing-posture`), the time-distribution chain verifier (White Rabbit, PTP,
//!   fibre and common-view hops to UTC(k), GUM budget, calibration currency, onboarding test,
//!   signed traceability statement; [`chain`], `pro-timing-chain`), the multi-source
//!   cross-check with non-GNSS sources grouped by traceability root ([`crosscheck`],
//!   `pro-timing-crosscheck`), and ground-station timing and pass-quality assurance
//!   ([`station`], `pro-timing-station`).
//!
//! The prediction method is pre-registered and scored as evidence row V5, and its successor
//! (used by the fleet assessment) as V5b (`docs/evidence/v5-holdover-time-to-breach/`);
//! [`V5_PASSED`] and [`V5B_PASSED`] record the published outcomes and every result carries
//! them in its label. Docs: `docs/TIMING-ASSURANCE.md`.

pub mod attack;
pub mod audit;
pub mod chain;
pub mod compliance;
pub mod console;
pub mod crosscheck;
pub mod evidence;
pub mod fleet;
pub mod grid;
pub mod holdover_evidence;
pub mod metrics;
pub mod model;
pub mod posture;
pub mod recovery;
pub mod scorecard;
pub mod selfcheck;
pub mod sfn;
pub mod station;
pub mod timebomb;

/// The scenario kind of a fleet assessment.
pub const KIND: &str = "pro-timing-holdover";
/// The command feature of the fleet page, metrics and evidence history.
pub const FLEET_FEATURE: &str = "pro-timing-fleet";
/// The command feature of the audit export.
pub const AUDIT_FEATURE: &str = "pro-timing-audit";

/// Whether the V5 pre-registered bars passed on both data sets (RESULTS.md). T1 is called
/// claimed only when this is true; `tests/timing_v5.rs` keeps it equal to the scored result.
pub const V5_PASSED: bool = false;

/// Whether the successor predictor's pre-registered bars (V5b, on data V5 did not use) passed
/// on both data sets (`V5b_RESULTS.md`); `tests/timing_v5.rs` keeps it equal to the scored result.
pub const V5B_PASSED: bool = false;

/// Whether the V5c pre-registered bars passed on D-R3 and D-S3 (`V5c_RESULTS.md`): they did
/// (scored once, 16 Nov 2026). The fleet assessment still runs the V5b predictor
/// (`HoldoverModel::fit_kalman`), so [`v5_status`] and the labels are unchanged until the
/// product is switched to `HoldoverModel::fit_harmonic`. `tests/timing_v5.rs` keeps this equal
/// to the scored result.
pub const V5C_PASSED: bool = true;

/// The V5 status in words, for labels and reports.
pub fn v5_status() -> &'static str {
    match (V5_PASSED, V5B_PASSED) {
        (true, _) => "prediction bars pre-registered as V5 and passed on recorded clocks",
        (false, true) => "V5 missed; the successor predictor's pre-registered bars (V5b) passed on recorded clocks",
        (false, false) => "prediction bars pre-registered as V5 and as its successor V5b both NOT passed: the time to breach is not a validated claim",
    }
}

/// The honesty label of every timing result.
pub fn label() -> String {
    format!(
        "MODELLED - holdover predictions from each clock's own record (or a stated prior for a servo record) under a three-term clock noise model; {}.",
        v5_status()
    )
}

/// `x` rounded to nine significant digits (values that feed a hash or a signature).
pub fn sig9(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    format!("{x:.8e}").parse().unwrap_or(x)
}
