// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Lunar time and lunar PNT evidence (Kshana Pro, W1119 family D)
//!
//! The Pro layer on top of the open engine's lunar modules ([`kshana::lunar_time`],
//! [`kshana::lunar_gauge`], [`kshana::clock_specs`], [`kshana::powerlaw`],
//! [`kshana::holdover`], [`kshana::timescales`], [`kshana::realdata::clk`],
//! [`kshana::rinex_obs`]). Nothing here copies those modules: they are called through the
//! dependency, and this layer adds the uncertainty budgets, the processing chains, the
//! codecs and the file formats that the research note (03-lunar-deep-space, gaps 1, 2, 4,
//! 5, 8 and 30) found nobody provides.
//!
//! | Item | Module | Gap | Evidence label |
//! |------|--------|-----|----------------|
//! | D1 lunar time with GUM uncertainty, TCL or TL, W0 sensitivity | [`time`] | 1, 8 | MODELLED; the secular rates are cross-checked against published values (LTE440, Ashby and Patla 2024, Turyshev 2025, OSTP 2024) |
//! | D2 "Circular T for the Moon" | [`circular`] | 2 | MODELLED; checked on synthetic data with known truth |
//! | D3 Earth–Moon TWTT holdover calculator | [`holdover`] | 4 | MODELLED; checked against closed-form ADEV to time-error relations and Monte Carlo |
//! | D4 TL–UTC broadcast offset codec | [`offset_codec`] | 5 | PROPOSAL pending LNIS AD5, not a standard; MODELLED |
//! | D5 LUNEX prototype | [`lunex`] | 30 | PROTOTYPE proposal for the IGS RINEX Committee, not a standard; MODELLED |
//! | D6 LuGRE C/N0 reproduction harness, pre-registered | [`lugre`] | 14 | MODELLED harness; no LuGRE data scored; input schema to be confirmed against the Zenodo record |
//! | D7 LNSP SLA verification (SISE/UEE, availability, continuity), signed monthly report | [`sla`] | 16, 25 | MODELLED; synthetic providers; SRD/SLA mapping illustrative until confirmed |
//!
//! **Nothing here is VALIDATED** against external measured lunar data: there is no lunar clock
//! data yet. Where a figure is compared with a published number, the comparison is a
//! cross-check of a model against another model, and is labelled so.
//!
//! CLI: `kshana-pro lunar ...` (feature `pro-timing-holdover`, Pro edition; decoding and
//! validating are free). Docs: `docs/LUNAR-PRO.md`.

pub mod budget;
pub mod circular;
pub mod holdover;
pub mod lugre;
pub mod lunex;
pub mod offset_codec;
pub mod sla;
pub mod time;

pub use budget::{Budget, Dist, Term, TermKind};

/// The evidence label every output of this module family carries.
pub const LABEL: &str = "MODELLED (Kshana Pro lunar layer): checked by internal tests and \
    cross-checked against published model values; not validated against measured lunar data";

/// The licence feature the `kshana-pro lunar` commands are gated on (an existing Pro feature:
/// lunar time keeping and holdover is timing assurance).
pub const FEATURE: &str = crate::timing::KIND;
