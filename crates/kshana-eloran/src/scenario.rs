// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Scenario files (TOML): everything a run needs, deterministic from file + seed.
//!
//! ```toml
//! [scene]
//! start_utc = "2026-11-18T00:00:00Z"
//! duration_s = 2.0
//! fs_hz = 2000000
//! seed = 1
//! output_scale_uv_m = 100000.0
//!
//! [receiver]               # a Trajectory
//! type = "static"
//! lat_deg = 36.0
//! lon_deg = 128.0
//!
//! [[outputs]]              # one per RF channel
//! type = "e_field"
//!
//! [[time_refs]]
//! id = "UTC(KRIS)"
//! offset_ns = 5.0
//!
//! [propagation.ground]
//! model = "simple"
//!
//! [[interference]]
//! type = "wgn"
//! rms_uv_m_20khz = 10.0
//!
//! [[chains]]
//! name = "9930"
//! gri_us = 99300
//! [[chains.stations]]
//! id = "M"
//! role = "master"
//! position = { lat_deg = 36.0, lon_deg = 129.3 }
//! time_ref = "UTC(KRIS)"
//! ```

use crate::antenna::OutputChannel;
use crate::chain::Chain;
use crate::interf::{self, InterfConfig};
use crate::ldc::{self, LdcConfig};
use crate::prop::{self, PropConfig};
use crate::scene::{Scene, SceneParams};
use crate::time::AbsTime;
use crate::timebase::{GnssTime, TimeRefModel, Timebase};
use crate::traj::Trajectory;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneSection {
    pub start_utc: String,
    #[serde(flatten)]
    pub params: SceneParams,
}

/// A chain plus its data-channel configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChainSpec {
    #[serde(flatten)]
    pub chain: Chain,
    #[serde(default)]
    pub ldc: Option<LdcConfig>,
}

/// Output file settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct OutputFile {
    /// "ri16", "rf32" or "none".
    #[serde(default = "d_fmt")]
    pub format: String,
}

fn d_fmt() -> String {
    "ri16".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub scene: SceneSection,
    pub receiver: Trajectory,
    pub outputs: Vec<OutputChannel>,
    #[serde(default)]
    pub time_refs: Vec<TimeRefModel>,
    /// Steering residual of GNSS times (ns), e.g. `{ Gpst = 3.0 }`.
    #[serde(default)]
    pub gnss_time_residual_ns: BTreeMap<GnssTime, f64>,
    #[serde(default)]
    pub propagation: PropConfig,
    #[serde(default)]
    pub interference: Vec<InterfConfig>,
    pub chains: Vec<ChainSpec>,
    #[serde(default)]
    pub file: OutputFile,
}

impl Scenario {
    pub fn from_toml(src: &str) -> Result<Scenario, String> {
        toml::from_str(src).map_err(|e| format!("scenario: {e}"))
    }
    /// SHA-256 of the scenario's canonical JSON form (stamped on every output).
    pub fn hash(&self) -> String {
        let j = serde_json::to_vec(self).expect("serialisable");
        hex::encode(Sha256::digest(&j))
    }
    /// Build the runnable scene.
    pub fn build(&self) -> Result<Scene, String> {
        let start = AbsTime::from_utc_iso(&self.scene.start_utc)?;
        let p = &self.scene.params;
        let mut timebase = Timebase::new(self.time_refs.clone(), p.duration_s);
        for (k, v) in &self.gnss_time_residual_ns {
            timebase.gnss_residual_s.insert(*k, v * 1e-9);
        }
        for c in &self.chains {
            for s in &c.chain.stations {
                if !s.time_ref.is_empty() && !timebase.has(&s.time_ref) {
                    return Err(format!(
                        "chain {} station {}: time reference {:?} is not defined in [[time_refs]]",
                        c.chain.name, s.id, s.time_ref
                    ));
                }
            }
        }
        let interferers = self
            .interference
            .iter()
            .map(|c| interf::build(c, p.fs_hz, p.seed))
            .collect::<Result<Vec<_>, _>>()?;
        let modulators = self
            .chains
            .iter()
            .map(|c| match &c.ldc {
                Some(l) => ldc::build(l),
                None => Ok(None),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scene = Scene {
            params: p.clone(),
            start,
            chains: self.chains.iter().map(|c| c.chain.clone()).collect(),
            modulators,
            timebase,
            prop: prop::build(&self.propagation)?,
            trajectory: self.receiver.clone(),
            outputs: self.outputs.clone(),
            interferers,
            noise_rms_20k: interf::noise_rms_20k(&self.interference),
        };
        scene.validate()?;
        Ok(scene)
    }
}
