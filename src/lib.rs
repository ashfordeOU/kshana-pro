// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # Kshana Pro
//!
//! The **proprietary overlay** on the open-source [`kshana`] PNT-resilience engine.
//!
//! Kshana Pro is a strict **superset** of open Kshana: it embeds the entire open
//! engine (every scenario kind, every surface) as a Cargo dependency and extends
//! it through the open core's public, semver-stable trait seams — the
//! [`kshana::api::ExternalPack`] contract for new scenario packs, and the
//! [`kshana::models::ErrorModel`] / `EphemerisProvider` / `ForceModel` interfaces
//! for device- and physics-level extensions. **The open core is never forked.**
//! See `docs/OVERLAY-ARCHITECTURE.md`.
//!
//! ## What is built vs gated
//!
//! Only the P0 *architecture seam* is built on spec (this crate's skeleton + the
//! seam-check proof pack and the superset dispatcher). The three value layers are
//! deliberately **gated** — their modules guard the seam but do not implement the
//! capability until the trigger (a partner, a contract, or a paying customer)
//! exists:
//!
//! | Layer | Module        | Attaches via                    | Trigger                     |
//! |-------|---------------|---------------------------------|-----------------------------|
//! | L1    | [`twin`]      | `kshana::models::ErrorModel`    | NDA partner + contract      |
//! | L2    | [`resilience`]| `kshana::models::ErrorModel`    | export clearance + customer |
//! | L3    | [`suite`]     | `kshana::api::ExternalPack` + sweep | second paying customer  |

// Re-export the open engine so downstream Pro code has a single import root and it
// is unambiguous that Pro *embeds* (does not reimplement) the open core.
pub use kshana;

pub mod batch;
pub mod benefits;
pub mod connectors;
pub mod license;
pub mod mbse;
pub mod mrd;
pub mod pack_c;
pub mod packs;
pub mod premium;
pub mod pro_study;
pub mod registry;
pub mod report;
pub mod report_brand;
pub mod report_pack_a;
pub mod report_pdf;
pub mod serve;
pub mod study_ledger;
mod util;

// Value layers — gated stubs (seam guards only; no proprietary capability is built
// on spec, per the open-core discipline in docs/OVERLAY-ARCHITECTURE.md).
pub mod resilience;
pub mod suite;
pub mod twin;

/// The version of this Pro overlay crate (distinct from the embedded open engine).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The number of scenario kinds the embedded open engine reports. Proves the
/// superset relationship is concrete and live: Pro inherits *all* of these by
/// construction, then adds its own packs on top (see [`registry::pro_kinds`]).
pub fn open_engine_kinds() -> usize {
    kshana::api::list_scenario_kinds().len()
}
