// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Pro scenario packs — out-of-tree [`kshana::api::ExternalPack`] implementations.
//!
//! These extend the open engine through its public, semver-stable contract; the
//! open core is never forked. The only pack built on spec is [`seam_check`], the
//! P0 proof that the overlay architecture compiles and runs against the open
//! engine. Real value packs (L1/L2/L3) live behind their gated modules until a
//! trigger exists.

pub mod batch_pack;
pub mod beacon_dop_pack;
pub mod beacon_placement_pack;
pub mod detector_intake;
pub mod lunar_signal_security_pack;
pub mod lunar_time_budget_study_pack;
pub mod mbse_pack;
pub mod optimism_gap_audit;
pub mod quantum_systematics_pack;
pub mod realtime_frame_eop_study_pack;
pub mod seam_check;
pub mod study_pdf;
pub mod trade_study_pack;
pub mod twin_pack;
