// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Groundwave propagation (stream S5 replaces the `simple` model with the full PF/SF/ASF
//! model; this file holds the configuration and a first-order model so scenes run now).

use super::{PathEffect, PathKind, PropagationModel, C};
use crate::chain::Station;
use crate::geo::inverse;
use crate::time::Ps;
use crate::traj::RxState;
use serde::{Deserialize, Serialize};

/// Atmospheric refractive index used for the Primary Factor (LORAN-C convention).
pub const N_ATM: f64 = 1.000_338;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum GroundConfig {
    /// First-order model: PF only (no SF/ASF), field `E = E1·√P / d` with a mild
    /// exponential attenuation. For tests and smoke runs; not a fidelity model.
    Simple {
        #[serde(default = "d_att")]
        atten_db_per_1000km: f64,
    },
}

fn d_att() -> f64 {
    3.0
}

impl Default for GroundConfig {
    fn default() -> Self {
        GroundConfig::Simple { atten_db_per_1000km: d_att() }
    }
}

/// Field of a short vertical monopole over perfect ground at 1 km per √kW (µV/m):
/// 300 mV/m·√(P/1 kW) at 1 km.
pub const E1_UV_M_PER_SQRT_KW_AT_1KM: f64 = 300_000.0;

pub struct Simple {
    pub atten_db_per_1000km: f64,
}

impl PropagationModel for Simple {
    fn name(&self) -> String {
        "groundwave-simple".into()
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        let d = inverse(st.position, rx.pos).distance_m.max(1.0);
        let geom = d / C;
        let pf_excess = (N_ATM - 1.0) * geom;
        let d_km = d / 1000.0;
        let field = E1_UV_M_PER_SQRT_KW_AT_1KM * st.power_kw.sqrt() / d_km.max(1e-3)
            * 10f64.powf(-self.atten_db_per_1000km * d_km / 1000.0 / 20.0);
        vec![PathEffect {
            kind: PathKind::Ground,
            delay: Ps::from_s(geom + pf_excess),
            field_uv_m: field,
            ecd_s: 0.0,
            phase_rad: 0.0,
            geom_ns: geom * 1e9,
            pf_ns: pf_excess * 1e9,
            sf_ns: 0.0,
            asf_ns: 0.0,
        }]
    }
}

pub fn build(cfg: &GroundConfig) -> Result<Box<dyn PropagationModel>, String> {
    match cfg {
        GroundConfig::Simple { atten_db_per_1000km } => {
            Ok(Box::new(Simple { atten_db_per_1000km: *atten_db_per_1000km }))
        }
    }
}
