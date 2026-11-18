// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Propagation: the contract every propagation model implements, and the composite model a
//! scene uses (groundwave + optional skywave).
//!
//! A model returns, for one station and one receiver state, the list of [`PathEffect`]s
//! that reach the receiver: the groundwave first, then any skywave hops. Each carries the
//! total propagation delay (with its Primary, Secondary and Additional Secondary Factor
//! parts for the groundwave), the field strength at the receiver, the propagation-induced
//! ECD change and any extra carrier phase.
//!
//! Models are built by [`build`] with the scene's [`BuildContext`] (absolute start,
//! duration, seed), so time-dependent models (skywave day/night, groundwave diurnal and
//! seasonal ASF) need no copy of `[scene] start_utc`. In the [`Composite`] the skywave
//! model sees the computed groundwave paths ([`PropagationModel::paths_with_ground`]), so a
//! stated sky/ground ratio or skywave delay refers to the actual groundwave.

pub mod complex;
pub mod conductivity;
pub mod ground;
pub mod millington;
pub mod smooth_earth;
pub mod special;
pub mod sky;
pub mod sky_fading;

use crate::chain::Station;
use crate::time::{AbsTime, Ps};
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
///
/// Absolute time: a model that needs it (solar zenith angle, diurnal or seasonal terms)
/// receives the scene start through [`BuildContext`] when it is built, and evaluates at
/// `start + rx.t_s`.
pub trait PropagationModel: Send + Sync {
    fn name(&self) -> String;
    /// Paths from `station` to a receiver in state `rx`.
    fn paths(&self, station: &Station, rx: &RxState) -> Vec<PathEffect>;
    /// Paths given the groundwave paths `ground` already computed by the scene's ground
    /// model for the same station and receiver state. Skywave models implement it so that
    /// quantities stated *relative to the groundwave* (a sky/ground ratio, a delay after
    /// the groundwave) refer to the actual groundwave of whichever ground model is
    /// configured; [`Composite`] calls it. The default ignores `ground`.
    fn paths_with_ground(&self, station: &Station, rx: &RxState, ground: &[PathEffect]) -> Vec<PathEffect> {
        let _ = ground;
        self.paths(station, rx)
    }
}

/// Scene context handed to the propagation builders: the absolute scene start (scene
/// time 0), the scene duration and the scene seed. Models derive absolute-time quantities
/// (solar zenith angle, UTC hour, day of year) from `start` plus [`RxState::t_s`], so the
/// scenario's `[scene] start_utc` need not be repeated in the propagation tables (an
/// explicit value there remains an override).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildContext {
    pub start: AbsTime,
    pub duration_s: f64,
    pub seed: u64,
}

impl BuildContext {
    /// Context of a scene starting at `start`.
    pub fn new(start: AbsTime, duration_s: f64, seed: u64) -> BuildContext {
        BuildContext { start, duration_s, seed }
    }
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
    /// The groundwave paths, then the skywave paths computed against them
    /// ([`PropagationModel::paths_with_ground`]).
    fn paths(&self, station: &Station, rx: &RxState) -> Vec<PathEffect> {
        let mut p = self.ground.paths(station, rx);
        if let Some(s) = &self.sky {
            let sky = s.paths_with_ground(station, rx, &p);
            p.extend(sky);
        }
        p
    }
}

/// Build the scene's propagation model in the scene context `ctx`.
pub fn build(cfg: &PropConfig, ctx: &BuildContext) -> Result<Box<dyn PropagationModel>, String> {
    let ground = ground::build(&cfg.ground, ctx)?;
    let sky = match &cfg.sky {
        Some(s) => Some(sky::build(s, ctx)?),
        None => None,
    };
    Ok(Box::new(Composite { ground, sky }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::StationStatus;
    use crate::geo::Geo;
    use crate::signal::Role;

    fn station() -> Station {
        Station {
            id: "M".into(),
            name: "M".into(),
            role: Role::Master,
            position: Geo::new(55.0, 0.0),
            power_kw: 400.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: StationStatus::On,
            faults: vec![],
        }
    }

    fn ctx() -> BuildContext {
        BuildContext::new(AbsTime::from_utc_iso("2026-12-21T22:00:00Z").unwrap(), 1.0, 0)
    }

    #[test]
    fn stated_sky_ratio_and_delay_hold_against_the_actual_groundwave() {
        let st = station();
        let rx = RxState { t_s: 0.0, pos: Geo::new(55.0, 10.0), heading_rad: 0.0, speed_mps: 0.0 };
        let ground: ground::GroundConfig = toml::from_str("model = \"smooth_earth\"\nsigma_s_m = 0.003").unwrap();
        for sky_toml in [
            "model = \"fixed\"\ndelay_us = 40.0\nratio_db = 6.0",
            "model = \"ionospheric\"\ndaylight = \"night\"\ndelay_us = 40.0\nratio_db = 6.0\nphase_height_offset_km = 0.0",
        ] {
            let cfg = PropConfig { ground: ground.clone(), sky: Some(toml::from_str(sky_toml).unwrap()) };
            let p = build(&cfg, &ctx()).unwrap().paths(&st, &rx);
            let (g, s) = (p[0], p[1]);
            assert_eq!(g.kind, PathKind::Ground);
            // the smooth-earth field over 3 mS/m land is well below the simple estimate…
            let simple = ground::Simple { atten_db_per_1000km: 3.0 }.paths(&st, &rx)[0];
            assert!(20.0 * (simple.field_uv_m / g.field_uv_m).log10() > 3.0);
            // …and the stated ratio and delay refer to the actual groundwave
            let ratio = 20.0 * (s.field_uv_m / g.field_uv_m).log10();
            assert!((ratio - 6.0).abs() < 1e-9, "{sky_toml}: ratio {ratio}");
            assert!(((s.delay - g.delay).as_us() + s.ecd_s * 1e6 - 40.0).abs() < 1e-5, "{sky_toml}");
        }
    }

    #[test]
    fn computed_skywave_field_is_absolute() {
        // P.684-computed field: independent of the configured ground model
        let st = station();
        let rx = RxState { t_s: 0.0, pos: Geo::new(55.0, 10.0), heading_rad: 0.0, speed_mps: 0.0 };
        let sky: sky::SkyConfig = toml::from_str("model = \"ionospheric\"\ndaylight = \"night\"").unwrap();
        let a = build(&PropConfig { ground: Default::default(), sky: Some(sky.clone()) }, &ctx()).unwrap().paths(&st, &rx);
        let se: ground::GroundConfig = toml::from_str("model = \"smooth_earth\"\nsigma_s_m = 0.003").unwrap();
        let b = build(&PropConfig { ground: se, sky: Some(sky) }, &ctx()).unwrap().paths(&st, &rx);
        assert_eq!(a[1].field_uv_m, b[1].field_uv_m);
        assert_eq!(a[1].delay, b[1].delay);
    }
}
