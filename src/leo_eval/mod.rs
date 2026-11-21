// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # LEO-PNT evaluation (research 2026-11-20 gaps, epic E21: B2101, B2107, B2108)
//!
//! * [`sims`] (B2101): neutral multi-simulator procedures for LEO-PNT (the published procedures
//!   are in `docs/leo-procedures/`, frozen at 1.0): a method hash registered before any run; a
//!   scorecard per simulator and step in input order (never ranked); simulator equivalence within
//!   stated tolerances with the simulator self-test verdict vocabulary; Doppler bounds.
//! * [`od`] (B2107, evaluation only): signal-in-space range error of LEO broadcast orbits and
//!   clocks against precise products with geometric weights computed for the stated altitude,
//!   an overbound check of the broadcast sigma, DCB stability and jumps, a PPP dependency
//!   register, and ([`traim`]) a T-RAIM fault-injection study on the open engine's snapshot
//!   RAIM (redundancy sweep, multi-fault cases, recovery time). Onboard orbit-integrity methods
//!   are not built here.
//! * [`claims`] (B2108, Pro part): an investor due-diligence report template on a cited claim
//!   dataset and measured results; every verdict cites its source and carries its label. The open
//!   claim datasets are built in the open engine.
//!
//! * [`crosswalk`] (B2101): every published procedure step mapped to the public requirement
//!   documents read for it (L-band and C-band user equipment), each row cited or "to confirm".
//!
//! MODELLED unless an output names the validation data it used.

pub mod claims;
pub mod crosswalk;
pub mod od;
pub mod sims;
pub mod traim;

/// Pro feature.
pub const FEATURE: &str = "pro-leo-evaluation";
