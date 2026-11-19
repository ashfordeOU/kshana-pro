// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Calibration wizards and hardware acceptance (stream B, A4/C3 and A5).
//!
//! * [`calibrate`]: calibration procedures as plain functions over traits (so a GUI or the
//!   CLI drives them): simulator channel delay/gain/linearity with re-verification, and
//!   receiver timing calibration with a GUM budget and a certificate-style JSON report.
//! * [`suite`]: the hardware acceptance / performance test suite — tests as data, a runner
//!   against provided devices and instruments, and the requirements verification matrix
//!   (RVM) generator.
//!
//! Evidence label: **MODELLED** — the procedures are verified against in-process mock
//! devices and instruments and against the simulator's own scenes. Every result that
//! concerns a hardware connector needs the real bench to mean anything (see each module).

pub mod calibrate;
pub mod suite;
pub mod transmitter;
