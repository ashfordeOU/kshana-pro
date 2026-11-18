// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Skywave (stream S6 replaces the `fixed` model with geometry- and ITU-R P.684-based
//! skywave; this file holds the configuration and a fixed-delay/ratio model).

use super::{PathEffect, PathKind, PropagationModel};
use crate::chain::Station;
use crate::geo::inverse;
use crate::time::Ps;
use crate::traj::RxState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum SkyConfig {
    /// One skywave arriving `delay_us` after the groundwave with amplitude ratio
    /// `ratio_db` (sky/ground) relative to a simple groundwave estimate.
    Fixed {
        delay_us: f64,
        ratio_db: f64,
        #[serde(default)]
        phase_deg: f64,
    },
}

pub struct Fixed {
    pub delay_us: f64,
    pub ratio_db: f64,
    pub phase_deg: f64,
}

impl PropagationModel for Fixed {
    fn name(&self) -> String {
        "skywave-fixed".into()
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        let g = super::ground::Simple { atten_db_per_1000km: 3.0 }.paths(st, rx);
        let g = g[0];
        let _ = inverse;
        vec![PathEffect {
            kind: PathKind::Sky { hop: 1 },
            delay: g.delay + Ps::from_us(self.delay_us),
            field_uv_m: g.field_uv_m * 10f64.powf(self.ratio_db / 20.0),
            ecd_s: 0.0,
            phase_rad: self.phase_deg.to_radians(),
            geom_ns: g.geom_ns,
            pf_ns: 0.0,
            sf_ns: 0.0,
            asf_ns: 0.0,
        }]
    }
}

pub fn build(cfg: &SkyConfig) -> Result<Box<dyn PropagationModel>, String> {
    match cfg {
        SkyConfig::Fixed { delay_us, ratio_db, phase_deg } => Ok(Box::new(Fixed {
            delay_us: *delay_us,
            ratio_db: *ratio_db,
            phase_deg: *phase_deg,
        })),
    }
}
