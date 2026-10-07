//! # Lab campaign results (input side of the Pro lab campaign report)
//!
//! Reads the output directory of the engine's `kshana iq campaign` (0.34.0 "Lab replay";
//! implemented on public branch `feat/034-campaign` @ `da95b9c`): `campaign.json`,
//! `cells/<key>.json`, `scorecard.json`, `DIGEST`.
//! It checks the directory before anything is rendered. The report re-renders these
//! numbers and never recomputes a metric. The engine is the source of every figure.
//!
//! Plan: `docs/plans/2026-10-07-lab-campaign-report-plan.md`.
//!
//! **Data hygiene (export-control item N6).** This module reads client results at run
//! time. No client recording, result or score may be committed to this repository; the
//! committed fixtures are synthetic (`tests/fixtures/lab_campaign_synthetic/`), and
//! `tests/lab_campaign_no_client_data.rs` enforces it.

pub mod aggregate;
pub mod hash;
pub mod load;
pub mod model;

pub use load::{load, LoadError, LoadedCampaign};
pub use model::{CampaignCell, CampaignManifest, DataClass};
