// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
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

pub mod alerts;
pub mod area;
pub mod attack_iq;
pub mod batch;
pub mod benefits;
pub mod brand_pdf;
pub mod cli;
pub mod connectors;
pub mod criterion;
pub mod desktop;
#[cfg(feature = "unfiled-methods")]
pub mod eloran_attrib;
pub mod eloran_fusion;
pub mod eloran_gnss;
pub mod eloran_gnss_sdr;
pub mod eloran_service;
pub mod eloran_timexfer;
pub mod eloran_trust;
pub mod exposure;
pub mod extension;
pub mod fdm;
pub mod forensics;
pub mod frameworks;
pub mod ievents;
pub mod integrity;
pub mod ireports;
pub mod lab_campaign;
pub mod license;
pub mod lunar_pro;
pub mod multi_source;
pub mod osnma_conformance;
pub mod pack_c;
pub mod packs;
pub mod payments;
pub mod pdf_text;
pub mod pics;
pub mod planner_front;
pub mod pnt_study;
pub mod pntdp;
pub mod premium;
pub mod pro_study;
pub mod propack;
pub mod registry;
pub mod remote_verdict;
pub mod report;
pub mod report_brand;
pub mod report_lab_campaign;
pub mod report_lab_campaign_pdf;
pub mod report_pack_a;
pub mod report_pack_c;
pub mod report_pdf;
pub mod results_only;
pub mod scorecards;
pub mod serve;
pub mod signed_dossier;
pub mod study_dossier;
pub mod study_ledger;
pub mod threat_library;
pub mod units;
// Shared helpers; public so a server-side extension crate can reuse them (not a stable API).
#[doc(hidden)]
pub mod util;
pub mod watermark;

// Value layers — gated stubs (seam guards only; no proprietary capability is built
// on spec, per the open-core discipline in docs/OVERLAY-ARCHITECTURE.md).
pub mod resilience;
pub mod suite;
pub mod timing;
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
