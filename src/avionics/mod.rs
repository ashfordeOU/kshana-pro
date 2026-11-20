// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Avionics test library (research 2026-11-20 gaps, epic E12)
//!
//! Executable test procedures for airborne GNSS equipment, written once and scored the same
//! way every run, with evidence an applicant can file:
//!
//! * [`sbas_msg`]: SBAS L1 message framing (250-bit frame, rotating preamble, CRC-24Q) and
//!   the message types the failure tests need (0, 1, 2 to 5, 6, 7, 26, 63), encode and decode.
//! * [`sbas_seq`]: an **SBAS message-sequence generator** for failure tests: a nominal
//!   broadcast schedule plus scripted faults (UDREI raised to "not monitored" or "do not use",
//!   GIVEI degraded, fast corrections withheld, message type 0, signal silence, mask change),
//!   as a time-tagged frame stream with the expected-behaviour timeline.
//! * [`procedure`]: the **procedure format** (`kshana-avionics-procedure/1`): what the test
//!   stimulates, which limits it needs (named parameters the applicant supplies from their own
//!   copy of the standard), the checks, and the scoring of the equipment's output log into a
//!   verdict per check: met, not met or not assessed.
//! * [`araim`]: ARAIM behaviour checks on the open engine's reference airborne algorithm
//!   (`kshana::araim_reference`): the published WG-C worked examples, an equipment-output
//!   comparison within a stated tolerance, constellation-declared-unusable ("institutional")
//!   cases, availability against the applicant's alert limits, and the effect of adding or
//!   removing a constellation (for example LEO ranging sources).
//! * [`recovery`]: the **per-avionics-type recovery-time scorecard**: recovery after a
//!   spoofing or jamming episode, from the test kit's signed verdicts, per equipment type,
//!   fleet-weighted.
//! * [`kit`]: the **evidence pack** (requirement-to-test traceability over the verdicts) and
//!   the **tool-qualification support kit**: material an applicant uses in their own DO-330
//!   tool qualification (operational requirements draft, verification cases run now with
//!   results, configuration index). It is input to the applicant's process, not a
//!   qualification of the tool.
//! * [`library`]: the built-in procedures (`avionics/procedures/*.toml`), compiled in.
//!
//! **Wording.** Procedures are *modelled on* RTCA/EUROCAE documents, cited by document,
//! revision and section only; their text and tables are not reproduced. Where a numeric limit
//! is needed and is not public, the procedure names a parameter and the applicant supplies the
//! value; a check whose limit was not supplied is **not assessed**. A verdict is test evidence
//! against the stated criteria for the run made; it is never a statement that the equipment
//! meets a standard.
//!
//! **Evidence label: MODELLED.** No avionics unit has been tested with this library in this
//! build; the procedures and examples are representative.

pub mod araim;
pub mod kit;
pub mod library;
pub mod procedure;
pub mod recovery;
pub mod sbas_msg;
pub mod sbas_seq;

/// The licence feature of procedure runs, the SBAS sequence generator and the ARAIM checks.
pub const FEATURE: &str = "pro-avionics-tests";
/// The licence feature of the per-avionics-type recovery-time scorecard.
pub const FEATURE_RECOVERY: &str = "pro-avionics-recovery";
/// The licence feature of the evidence pack and the tool-qualification support kit.
pub const FEATURE_KIT: &str = "pro-avionics-kit";

/// The evidence label of every library output.
pub const LABEL: &str = "MODELLED: verdicts computed from the equipment output supplied, \
against criteria and limits stated before the run (limits the applicant supplied are marked \
so). Procedures are modelled on the documents named, cited by section only; they are not the \
test methods of those documents. Test evidence for the runs made; not a statement that any \
equipment meets a standard.";

/// Round `x` to nine significant digits (values that feed hashes).
pub fn sig9(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let mag = x.abs().log10().floor() as i32;
    let scale = 10f64.powi(8 - mag);
    (x * scale).round() / scale
}

/// Lower-case hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
