// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **Pack C** — a versioned, subscribable library of resilience scenarios.
//!
//! A customer subscribes to a scenario pack and runs their own config against it
//! in CI, getting a reproducible scored report plus a regression gate. This module
//! owns the versioned [`manifest`] type, the figure-of-merit [`metrics`]
//! extractor, the regression [`gate`], and the CI [`run`] orchestrator. The
//! branded report renderer and the CLI are added by later tasks.
//!
//! **Campaign watch** extends the subscription: [`watch`] (the `[[watch]]` tables),
//! [`semantics`] (which leaves are figures), [`campaign`] (named campaign figures),
//! [`stat_gate`] (noise against change for Monte Carlo figures), [`history`] (a
//! hash-chained run history), [`trend`] (the trend report) and [`diff`] (what moved
//! between two reports). A pack without watches runs exactly as before.

pub mod campaign;
pub mod diff;
pub mod gate;
pub mod history;
pub mod manifest;
pub mod metrics;
pub mod run;
pub mod semantics;
pub mod stat_gate;
pub mod trend;
pub mod watch;

pub use gate::{compare, direction_for, Direction, GateVerdict, Regression, Thresholds};
pub use manifest::{parse_pack, PackEntry, ScenarioPack};
pub use metrics::MetricSet;
pub use run::{assemble_report, run_ci, run_pack, CiContext, CiReport};
pub use run::{run_ci_with, WatchOptions, WatchOutcome, WatchSection};
pub use watch::Watch;
