// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Propagation: the contract every propagation model implements, and the composite model a
//! scene uses (groundwave + optional skywave).
//!
//! A model returns, for one station and one receiver state, the list of [`PathEffect`]s
//! that reach the receiver: the groundwave first, then any skywave hops. Each carries the
//! total propagation delay (with its Primary, Secondary and Additional Secondary Factor
//! parts for the groundwave), the field strength at the receiver, the propagation-induced
//! ECD change and any extra carrier phase.

pub mod complex;
pub mod conductivity;
pub mod ground;
pub mod millington;
pub mod smooth_earth;
pub mod special;
pub mod sky;
pub mod sky_fading;

use crate::chain::Station;
use crate::time::Ps;
use crate::traj::RxState;
use serde::{Deserialize, Serialize};

/// Speed of light in vacuum (m/s).
pub const C: f64 = 299_792_458.0;

/// Kind of propagation path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathKind {
    Ground,
    Sky { hop: u8 },
}

/// One propagation path from a station to the receiver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathEffect {
    pub kind: PathKind,
    /// Total propagation delay, transmitter to receiver.
    pub delay: Ps,
    /// RMS field strength at the receiver at the pulse peak (µV/m).
    pub field_uv_m: f64,
    /// Propagation-induced change of ECD (s), added to the transmitted ECD.
    pub ecd_s: f64,
    /// Extra carrier phase of the path (rad) (e.g. ionospheric reflection phase).
    pub phase_rad: f64,
    /// Delay components (ns) for truth logging: geodesic distance / c, PF excess, SF, ASF.
    pub geom_ns: f64,
    pub pf_ns: f64,
    pub sf_ns: f64,
    pub asf_ns: f64,
}

/// A propagation model.
pub trait PropagationModel: Send + Sync {
    fn name(&self) -> String;
    /// Paths from `station` to a receiver in state `rx`.
    fn paths(&self, station: &Station, rx: &RxState) -> Vec<PathEffect>;
}

/// Propagation configuration of a scene.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct PropConfig {
    #[serde(default)]
    pub ground: ground::GroundConfig,
    #[serde(default)]
    pub sky: Option<sky::SkyConfig>,
}

/// Groundwave plus optional skywave.
pub struct Composite {
    pub ground: Box<dyn PropagationModel>,
    pub sky: Option<Box<dyn PropagationModel>>,
}

impl PropagationModel for Composite {
    fn name(&self) -> String {
        match &self.sky {
            Some(s) => format!("{} + {}", self.ground.name(), s.name()),
            None => self.ground.name(),
        }
    }
    fn paths(&self, station: &Station, rx: &RxState) -> Vec<PathEffect> {
        let mut p = self.ground.paths(station, rx);
        if let Some(s) = &self.sky {
            p.extend(s.paths(station, rx));
        }
        p
    }
}

/// Build the scene's propagation model.
pub fn build(cfg: &PropConfig) -> Result<Box<dyn PropagationModel>, String> {
    let ground = ground::build(&cfg.ground)?;
    let sky = match &cfg.sky {
        Some(s) => Some(sky::build(s)?),
        None => None,
    };
    Ok(Box::new(Composite { ground, sky }))
}
