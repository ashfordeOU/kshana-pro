// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # LEO-PNT evaluation (research 2026-11-20 gaps, epic E21: B2101, B2107, B2108)
//!
//! * [`sims`] (B2101): neutral multi-simulator procedures for LEO-PNT: a procedure with
//!   pre-registered metrics and bars, hashed before any run; a scorecard per simulator and step
//!   in input order (never ranked), with the spread across simulators and the simulators that
//!   differ from the others.
//! * [`od`] (B2107, evaluation only): signal-in-space range error of LEO broadcast orbits and
//!   clocks against precise products with geometric weights computed for the stated altitude,
//!   an overbound check of the broadcast sigma, DCB stability and jumps, and a PPP dependency
//!   register. Onboard orbit-integrity methods are not built here.
//! * [`claims`] (B2108): a constellation-claim dataset schema in which every claim cites a public
//!   source; Kshana ships templates for Celeste, CentiSpace and Geespace with placeholders only,
//!   and compares filled claims with measured results.
//!
//! MODELLED unless an output names the validation data it used.

pub mod claims;
pub mod od;
pub mod sims;

/// Pro feature.
pub const FEATURE: &str = "pro-leo-evaluation";
