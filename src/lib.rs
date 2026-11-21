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

pub mod adsb_phantom;
pub mod agri;
pub mod ais_evidence;
pub mod alerts;
pub mod altpnt;
pub mod area;
pub mod attack_iq;
pub mod aviation;
pub mod avionics;
pub mod batch;
pub mod benefits;
pub mod brand_pdf;
pub mod bridge;
pub mod briefings;
pub mod cli;
pub mod connectors;
pub mod criterion;
pub mod custody;
pub mod dependency;
pub mod desktop;
#[cfg(feature = "unfiled-methods")]
pub mod eloran_attrib;
pub mod eloran_claims;
pub mod eloran_fusion;
pub mod eloran_gnss;
pub mod eloran_gnss_sdr;
pub mod eloran_service;
pub mod eloran_timexfer;
pub mod eloran_trust;
pub mod enforcement;
pub mod envelope;
pub mod exposure;
pub mod extension;
pub mod fdm;
pub mod fingerprint;
pub mod forensics;
pub mod frameworks;
pub mod hybrid_sim;
pub mod ievents;
pub mod insurance;
pub mod integrity;
pub mod interface_drift;
pub mod ireports;
pub mod ks_inside;
pub mod lab_campaign;
pub mod leo_eval;
pub mod leo_timing;
pub mod license;
pub mod longterm;
pub mod lunar_evidence;
pub mod lunar_pro;
pub mod lunar_surface;
pub mod method_uncertainty;
pub mod method_validation;
pub mod multi_source;
pub mod navaids;
pub mod navic;
pub mod obs_provenance;
pub mod osnma_conformance;
pub mod osnma_scenarios;
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
pub mod product_bench;
pub mod propack;
pub mod registry;
pub mod regpacks;
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
pub mod space_rx;
pub mod study_dossier;
pub mod study_ledger;
pub mod testkit;
pub mod threat_library;
pub mod units;
pub mod vault;
// Shared helpers; public so a server-side extension crate can reuse them (not a stable API).
pub mod simlab;
#[doc(hidden)]
pub mod util;
pub mod utm;
pub mod watermark;

// Value layers — gated stubs (seam guards only; no proprietary capability is built
// on spec, per the open-core discipline in docs/OVERLAY-ARCHITECTURE.md).
pub mod resilience;
pub mod space_ops;
pub mod station_fleet;
pub mod suite;
pub mod survey_qc;
pub mod tacho_osnma;
pub mod timing;
pub mod transport;
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
