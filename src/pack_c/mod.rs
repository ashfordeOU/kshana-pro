// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! **Pack C** — a versioned, subscribable library of resilience scenarios.
//!
//! A customer subscribes to a scenario pack and runs their own config against it
//! in CI, getting a reproducible scored report plus a regression gate. This module
//! currently owns only the addressable unit — the versioned [`manifest`] type.
//! Metric extraction, the regression gate, the run orchestrator, the branded
//! report, and the CLI are added by later tasks.

pub mod manifest;
pub mod metrics;

pub use manifest::{parse_pack, PackEntry, ScenarioPack};
pub use metrics::MetricSet;
