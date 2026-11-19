// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Test-as-code kit (research 2026-11-20 gaps 07, 08, 08b, 11, 12)
//!
//! Receiver tests written once, run anywhere, scored the same way every build:
//!
//! * [`spec`]: a **neutral scenario spec** (versioned TOML/JSON, `kshana-testkit-scenario/1`):
//!   constellations and signals, trajectory, environment, threat events at signal-parameter
//!   level (or a threat-library scenario by id, [`library`]), the expected behaviour and the
//!   pass/fail criteria, all stated before a run.
//! * [`compile`]: the scenario in each existing simulator adapter's native form (Skydel,
//!   Spirent, R&S SMBV, LabSat, gps-sdr-sim, through `kshana-eloran::sync`) and a validator
//!   that reports what each adapter cannot express.
//! * Next part (separate change): the CI runner with signed verdicts per build, the
//!   regression diff, incident-to-regression and the replay fidelity statement.
//! * [`packs`]: scenario packs (DP vessels after the IMCA IN 1730 themes, aviation post-spoof
//!   recovery, UAS, timing, resilience levels modelled on the DHS RPCF).
//!
//! Licence: [`FEATURE`] (`pro`) for compile, from-library and pack export. Validate and pack
//! listing are free.
//!
//! **Evidence label: MODELLED.** No receiver has been scored with the kit in this build; the
//! pack parameters are representative; the simulator profiles are unconfirmed templates
//! unless a lab confirmed them. A verdict is test evidence for the scenario run; it never
//! says a receiver is compliant, conformant or certified. Safety: the kit drives lab
//! simulators' own controls (conducted or shielded use) and writes no signal of its own.

pub mod compile;
pub mod library;
pub mod packs;
pub mod spec;

/// The licence feature of compile, from-library and pack export.
pub const FEATURE: &str = "pro-test-kit";

/// The evidence label of every kit output.
pub const LABEL: &str = "MODELLED: verdicts computed from the receiver output supplied, \
against criteria stated in the scenario before the run. Scenario parameters are \
representative; simulator profiles are unconfirmed templates unless the lab confirmed them. \
Test evidence for the scenarios run; not a statement that any receiver meets a standard, and \
not a certification.";
/// The one-line label written at the top of scenario files.
pub const LABEL_SHORT: &str =
    "MODELLED: representative parameters; modelled on the themes named, not a test method of them.";
