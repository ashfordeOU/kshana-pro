// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! Pro scenario packs — out-of-tree [`kshana::api::ExternalPack`] implementations.
//!
//! These extend the open engine through its public, semver-stable contract; the
//! open core is never forked. The only pack built on spec is [`seam_check`], the
//! P0 proof that the overlay architecture compiles and runs against the open
//! engine. Real value packs (L1/L2/L3) live behind their gated modules until a
//! trigger exists.

pub mod seam_check;
