// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Alternative-PNT evaluation and civil test-campaign evidence (research backlog E40 B4004)
//!
//! Four evidence products for an alternative-PNT (A-PNT) system that someone else built, run
//! by a tester, a buyer or the supplier on data they already hold:
//!
//! * [`failure_modes`]: the **architecture failure-mode pack**. The system's sources and the
//!   resources they depend on (antenna, power, reference clock, ...) and its declared failure
//!   modes with their detection, mitigation and fallback; the pack finds common-cause
//!   resources, fallbacks that share the failing resource, single points of failure, sources
//!   with no declared failure mode, and labels the test evidence of every mode by where it
//!   was obtained (`kshana-pro altpnt failure-modes`, feature [`FEATURE_EVIDENCE`]).
//! * [`labels`]: **digital-twin versus flight-test labels**. Each performance claim is
//!   labelled by the strongest environment that produced a result for it, and a twin
//!   prediction is compared with the field or flight measurement of the same metric, so a
//!   figure that was only ever an architectural target is shown as one
//!   (`altpnt labels`, [`FEATURE_EVIDENCE`]).
//! * [`campaign`]: **civil test-campaign evidence**. The outputs the system under test
//!   produced (position, protection level, alert, time) are scored against the tester's
//!   truth per run and per condition, the supplier's stated claims are checked against the
//!   scores, and every dataset passes a civil-releasability screen first
//!   (`altpnt campaign`, [`FEATURE_CAMPAIGN`]).
//! * [`time_iface`]: **resilient-time civil interface tests**. The time a resilient-time
//!   service delivers at its civil interface, measured against an independent reference, is
//!   tested against the limits it declares per state, the honesty of its declared quality,
//!   holdover growth, recovery and an MTIE mask (`altpnt time-iface`, [`FEATURE_EVIDENCE`]).
//!
//! ## Scope (export control)
//!
//! This is evaluation and test evidence only ([`SCOPE`]). Nothing here computes a position or
//! time solution, fuses sensors, matches a map (terrain, gravity or magnetic), selects or
//! switches sources, locates an emitter, steers an antenna or radiates. The campaign reads
//! outputs the system under test already produced and compares them with truth the tester
//! supplies; the failure-mode pack reads declarations. Military signals (PRS, M-code, P(Y))
//! and weapons end uses are refused by the releasability screen ([`campaign::screen`]).
//!
//! Every result is labelled MODELLED: the rules are exercised on synthetic fixtures with known
//! answers, not validated against a real campaign. A result is test evidence for the data
//! supplied, never a certification, a qualification or an approval.

pub mod campaign;
pub mod failure_modes;
pub mod labels;
pub mod synth;
pub mod time_iface;

use std::path::Path;

pub use crate::aviation::{rnd, wilson95, write_result, InputRef, Inputs};

/// `failure-modes`, `labels`, `time-iface` (Pro).
pub const FEATURE_EVIDENCE: &str = "pro-altpnt-evidence";
/// `campaign` (Pro).
pub const FEATURE_CAMPAIGN: &str = "pro-altpnt-campaign";

/// The evidence class of every result.
pub const EVIDENCE: &str = "MODELLED";

/// What every result says about its scope.
pub const SCOPE: &str = "Evaluation and test evidence only. Scores and documents what the \
system under test declared and produced against truth supplied by the tester; computes no \
position or time solution, fuses no sensors, matches no maps, switches no sources, locates no \
emitter and controls no antenna. Civil-releasable data only. Not a certification, \
qualification or approval, and not a statement about conditions other than those tested.";

/// Where a result was obtained, weakest first.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Environment {
    /// A digital twin or architecture model.
    Twin,
    /// A signal or scenario simulator driving no hardware under test.
    Simulator,
    /// Hardware in the loop (the unit under test fed by a simulator).
    Hil,
    /// Over-the-air or conducted RF in a lab or chamber.
    LabRf,
    /// A field test on the ground (van, vessel, fixed site).
    Field,
    /// A flight test.
    Flight,
}

impl Environment {
    /// The result is a measurement of the unit under test (not a model).
    pub fn measured(self) -> bool {
        self >= Environment::Hil
    }
    /// The result was obtained in the operating environment (field or flight).
    pub fn operational(self) -> bool {
        self >= Environment::Field
    }
    pub fn name(self) -> &'static str {
        match self {
            Environment::Twin => "twin",
            Environment::Simulator => "simulator",
            Environment::Hil => "hil",
            Environment::LabRf => "lab-rf",
            Environment::Field => "field",
            Environment::Flight => "flight",
        }
    }
}

/// The evidence label of a claim or a failure mode, from the environments of its results.
///
/// * `MEASURED-OPERATIONAL`: measured in the field or in flight;
/// * `MEASURED-LAB`: measured on the unit under test in a lab (HIL, RF), not in operation;
/// * `MODELLED`: only twin or simulator results, an architectural target rather than an
///   empirical result;
/// * `CLAIMED`: no result at all.
///
/// [`labels`] adds `VALIDATED` for a claim whose twin prediction agrees with the operational
/// measurement within the stated tolerance.
pub fn evidence_label(envs: &[Environment]) -> &'static str {
    match envs.iter().max() {
        None => "CLAIMED",
        Some(e) if e.operational() => "MEASURED-OPERATIONAL",
        Some(e) if e.measured() => "MEASURED-LAB",
        Some(_) => "MODELLED",
    }
}

/// The plain-text summary of a result.
pub fn summary(kind: &str, headline: &str) -> String {
    format!("[altpnt {kind}] {headline}\n{SCOPE}\nEvidence class: {EVIDENCE}.\n")
}

/// Read a TOML request and record it as the first input.
pub fn read_request<T: serde::de::DeserializeOwned>(path: &Path) -> Result<(T, Inputs), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: T = toml::from_str(&src).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut inputs = Inputs::default();
    inputs.add_text("request.toml", "request", &src);
    Ok((req, inputs))
}

/// Words a result never uses about the system under test.
pub const FORBIDDEN_WORDS: [&str; 4] = ["compliant", "conformant", "certified", "approved"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_the_strongest_environment() {
        use Environment::*;
        assert_eq!(evidence_label(&[]), "CLAIMED");
        assert_eq!(evidence_label(&[Twin, Simulator]), "MODELLED");
        assert_eq!(evidence_label(&[Twin, Hil]), "MEASURED-LAB");
        assert_eq!(evidence_label(&[LabRf, Flight]), "MEASURED-OPERATIONAL");
        assert!(Field.operational() && !LabRf.operational() && Hil.measured());
        for w in FORBIDDEN_WORDS {
            assert!(!SCOPE.to_lowercase().contains(w), "{w}");
        }
    }
}
