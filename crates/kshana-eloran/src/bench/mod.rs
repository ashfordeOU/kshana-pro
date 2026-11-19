// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Bench readiness: make a lab ready for hardware before the hardware arrives.
//!
//! * [`catalog`]: published specifications of sources, simulators, references, counters,
//!   scopes, receivers and antennas, each value cited to the vendor's documentation.
//! * [`check`]: does an inventory of devices in roles meet the simulator's needs? Pass,
//!   Warn, Fail or Unknown per check, with the source of each value.
//! * [`spectwin`]: a device emulated from its datasheet, and its predicted performance as
//!   an eLoran source.
//! * [`open`]: open any supported device (or its twin) from one address string.
//! * [`probe`]: find and identify connected instruments (read-only).
//! * [`kit`]: wiring plan, first-plug procedure, instrument profiles and predictions.
//! * [`run`]: the acceptance suite on the bench, with hardware or rehearsed with twins.
//!
//! Evidence: catalogue values are **datasheet values**, twin results are **MODELLED**;
//! nothing in this module was measured on hardware.

pub mod catalog;
pub mod check;
pub mod kit;
pub mod open;
pub mod probe;
pub mod run;
pub mod spectwin;
