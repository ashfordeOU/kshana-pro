// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Local and urban effects on the groundwave (MODELLED, **heuristic**): site ASF offsets,
//! re-radiation by large conductive structures (masts, bridges, steel buildings, power
//! lines) and the resulting H-field distortion of a crossed-loop bearing.
//!
//! `[propagation.ground] model = "urban"` wraps any other ground model (`base`):
//!
//! ```toml
//! [propagation.ground]
//! model = "urban"
//! site_asf_ns = 15.0                      # added everywhere (e.g. a surveyed site bias)
//! [propagation.ground.base]
//! model = "smooth_earth"
//! sigma_s_m = 0.01
//! [[propagation.ground.sites]]            # local offsets with a smooth taper
//! lat_deg = 51.50
//! lon_deg = -0.12
//! radius_m = 2000.0
//! asf_ns = 40.0
//! [[propagation.ground.structures]]
//! name = "mast"
//! lat_deg = 51.51
//! lon_deg = -0.10
//! height_m = 150.0
//! # radius_m = 450.0                      # 1/e influence radius (default 3·height, ≥ 200 m)
//! # coefficient = 0.04                    # re-radiation coefficient (default below)
//! [propagation.ground.loop_distortion]
//! quadrantal_deg = 2.0
//! ```
//!
//! # Site offsets
//!
//! `site_asf_ns` is added to every path. Each `sites` entry adds `asf_ns` (and optionally
//! `field_db`, `ecd_ns`) with weight 1 within `radius_m` of its centre, falling smoothly
//! (smoothstep) to 0 at twice that distance.
//!
//! # Re-radiation (phasor heuristic)
//!
//! A structure `S` at geodesic distance `r` from the receiver, distance `D_TS` from the
//! transmitter (direct path `D`), re-radiates a wave of relative complex amplitude
//!
//! ```text
//! a = Γ · exp(−r/R) · exp(−i k Δ),   Δ = D_TS + r − D,   k = n·ω/c
//! Γ = 0.2 · min(1, h/(λ/4))  (default; λ/4 = 750 m at 100 kHz)
//! ```
//!
//! The received field becomes `E·(1 + Σ a)`: the phase delay changes by
//! `−arg(1 + Σa)/ω`, the field by `|1 + Σa|`, and the group delay — from the same sum at
//! f₀ ± 10 kHz — by its slope, i.e. an ECD perturbation. This gives the oscillating,
//! distance-decaying pattern (period ≈ λ/(1 − cos α) along the direction α between the
//! transmitter and the structure) seen around large structures. The values of `Γ` and
//! `R` are engineering guesses, not fitted to measurements; with a 100 m mast and the
//! defaults the peak phase perturbation is ≈ 40 ns. Not a scattering computation.
//!
//! # Crossed-loop (H-field) distortion
//!
//! The re-radiated magnetic field arrives from the structure's bearing `β_s` instead of the
//! station's bearing `β`, so a crossed-loop pair sees
//! `A = cos(β − ψ) + Σ a cos(β_s − ψ)`, `B = sin(β − ψ) + Σ a sin(β_s − ψ)`
//! (heading ψ). [`UrbanModel::apparent_bearing_rad`] returns the bearing a receiver
//! estimates from the components of `A`, `B` in phase with the (equally perturbed)
//! E-field, plus an optional platform quadrantal error `q·sin 2(β − ψ)` (the classical
//! loop/hull error; `quadrantal_deg`). That value can be passed to
//! [`crate::antenna::OutputChannel::gain`] in place of the geometric bearing; the scene
//! currently uses the geometric bearing (the hook is not wired into synthesis here), so
//! the bearing distortion is an analysis API only.

use super::ground::GroundConfig;
use super::{BuildContext, PathKind, PathEffect, PropagationModel, C};
use super::complex::{Cx, ONE};
use super::ground::{F0_HZ, N_ATM};
use super::smooth_earth::GROUP_HALF_BAND_HZ;
use crate::chain::Station;
use crate::geo::{inverse, Geo};
use crate::time::Ps;
use crate::traj::RxState;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// A local ASF offset area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Site {
    #[serde(default)]
    pub name: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub radius_m: f64,
    #[serde(default)]
    pub asf_ns: f64,
    #[serde(default)]
    pub field_db: f64,
    #[serde(default)]
    pub ecd_ns: f64,
}

/// A re-radiating structure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Structure {
    #[serde(default)]
    pub name: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub height_m: f64,
    /// 1/e influence radius (m); default `max(3·height, 200 m)`.
    #[serde(default)]
    pub radius_m: Option<f64>,
    /// Re-radiation coefficient Γ; default `0.2·min(1, h/(λ/4))`.
    #[serde(default)]
    pub coefficient: Option<f64>,
}

impl Structure {
    pub fn radius(&self) -> f64 {
        self.radius_m.unwrap_or((3.0 * self.height_m).max(200.0))
    }
    pub fn gamma(&self) -> f64 {
        let quarter = C / F0_HZ / 4.0;
        self.coefficient
            .unwrap_or(0.2 * (self.height_m / quarter).min(1.0))
    }
    pub fn position(&self) -> Geo {
        Geo::new(self.lat_deg, self.lon_deg)
    }
}

/// Crossed-loop distortion options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct LoopDistortion {
    /// Platform quadrantal error amplitude (deg).
    #[serde(default)]
    pub quadrantal_deg: f64,
    /// Include the structures' re-radiated H-field.
    #[serde(default = "d_true")]
    pub structures: bool,
}

fn d_true() -> bool {
    true
}

/// `model = "urban"`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanGroundConfig {
    pub base: Box<GroundConfig>,
    #[serde(default)]
    pub site_asf_ns: f64,
    #[serde(default)]
    pub sites: Vec<Site>,
    #[serde(default)]
    pub structures: Vec<Structure>,
    #[serde(default)]
    pub loop_distortion: Option<LoopDistortion>,
}

/// The wrapper model.
pub struct UrbanModel {
    pub base: Box<dyn PropagationModel>,
    pub site_asf_ns: f64,
    pub sites: Vec<Site>,
    pub structures: Vec<Structure>,
    pub loop_distortion: Option<LoopDistortion>,
}

/// Perturbation of one path by the local effects.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Perturbation {
    /// Phase-delay change (s).
    pub phase_s: f64,
    /// Group-delay change (s).
    pub group_s: f64,
    /// Field factor (linear).
    pub field: f64,
}

fn smoothstep_down(r: f64, r0: f64) -> f64 {
    if r <= r0 {
        1.0
    } else if r >= 2.0 * r0 {
        0.0
    } else {
        let u = (2.0 * r0 - r) / r0;
        u * u * (3.0 - 2.0 * u)
    }
}

impl UrbanModel {
    pub fn new(
        base: Box<dyn PropagationModel>,
        site_asf_ns: f64,
        sites: Vec<Site>,
        structures: Vec<Structure>,
        loop_distortion: Option<LoopDistortion>,
    ) -> Result<UrbanModel, String> {
        for s in &sites {
            if !(s.radius_m > 0.0) {
                return Err(format!("urban site {:?}: radius_m must be positive", s.name));
            }
        }
        for s in &structures {
            if !(s.height_m > 0.0) || !(s.radius() > 0.0) || !(0.0..=1.0).contains(&s.gamma()) {
                return Err(format!(
                    "urban structure {:?}: height_m > 0, radius_m > 0, coefficient in 0..1",
                    s.name
                ));
            }
        }
        if !site_asf_ns.is_finite() {
            return Err("urban: site_asf_ns must be finite".into());
        }
        Ok(UrbanModel {
            base,
            site_asf_ns,
            sites,
            structures,
            loop_distortion,
        })
    }

    /// Re-radiated relative amplitudes `(a, bearing from rx to the structure)` at `f_hz`.
    fn scattered(&self, tx: Geo, rx: Geo, f_hz: f64) -> Vec<(Cx, f64)> {
        let k = N_ATM * 2.0 * PI * f_hz / C;
        let d = inverse(tx, rx).distance_m;
        self.structures
            .iter()
            .filter_map(|s| {
                let rs = inverse(rx, s.position());
                if rs.distance_m > 12.0 * s.radius() {
                    return None;
                }
                let dts = inverse(tx, s.position()).distance_m;
                let excess = (dts + rs.distance_m - d).max(0.0);
                let amp = s.gamma() * (-rs.distance_m / s.radius()).exp();
                Some((Cx::polar(amp, -k * excess), rs.az1_rad))
            })
            .collect()
    }

    /// Delay and field perturbation of a groundwave from `tx` at `rx`.
    pub fn perturbation(&self, tx: Geo, rx: Geo) -> Perturbation {
        let sum = |f: f64| -> Cx {
            self.scattered(tx, rx, f)
                .into_iter()
                .fold(ONE, |acc, (a, _)| acc + a)
        };
        let (s0, sm, sp) = (
            sum(F0_HZ),
            sum(F0_HZ - GROUP_HALF_BAND_HZ),
            sum(F0_HZ + GROUP_HALF_BAND_HZ),
        );
        let mut phase = -s0.arg() / (2.0 * PI * F0_HZ);
        let mut group = -(sp.arg() - sm.arg()) / (2.0 * PI * 2.0 * GROUP_HALF_BAND_HZ);
        let mut field_db = 20.0 * s0.abs().log10();
        phase += self.site_asf_ns * 1e-9;
        group += self.site_asf_ns * 1e-9;
        for s in &self.sites {
            let w = smoothstep_down(
                inverse(Geo::new(s.lat_deg, s.lon_deg), rx).distance_m,
                s.radius_m,
            );
            phase += w * s.asf_ns * 1e-9;
            group += w * (s.asf_ns + s.ecd_ns) * 1e-9;
            field_db += w * s.field_db;
        }
        Perturbation {
            phase_s: phase,
            group_s: group,
            field: 10f64.powf(field_db / 20.0),
        }
    }

    /// Bearing (rad, from north) of `st` that a crossed-loop receiver at `rx` estimates,
    /// with the structures' H-field and the quadrantal error (see the module docs).
    /// Equals the geometric bearing without `loop_distortion`.
    pub fn apparent_bearing_rad(&self, st: &Station, rx: &RxState) -> f64 {
        let beta = inverse(rx.pos, st.position).az1_rad;
        let Some(ld) = &self.loop_distortion else {
            return beta;
        };
        let psi = rx.heading_rad;
        let mut a = Cx::real((beta - psi).cos());
        let mut b = Cx::real((beta - psi).sin());
        let mut e = ONE;
        if ld.structures {
            for (s, bs) in self.scattered(st.position, rx.pos, F0_HZ) {
                a += s * (bs - psi).cos();
                b += s * (bs - psi).sin();
                e += s;
            }
        }
        let r = e.conj();
        let rel = (b * r).re.atan2((a * r).re);
        let quad = ld.quadrantal_deg.to_radians() * (2.0 * (beta - psi)).sin();
        (psi + rel + quad).rem_euclid(2.0 * PI)
    }

    /// Bearing error (rad, apparent − geometric, wrapped to ±π).
    pub fn bearing_error_rad(&self, st: &Station, rx: &RxState) -> f64 {
        let beta = inverse(rx.pos, st.position).az1_rad;
        (self.apparent_bearing_rad(st, rx) - beta + 3.0 * PI).rem_euclid(2.0 * PI) - PI
    }
}

impl PropagationModel for UrbanModel {
    fn name(&self) -> String {
        format!(
            "{} + local effects ({} site(s), {} structure(s); heuristic)",
            self.base.name(),
            self.sites.len(),
            self.structures.len()
        )
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        let mut p = self.base.paths(st, rx);
        let q = self.perturbation(st.position, rx.pos);
        for g in p.iter_mut().filter(|g| g.kind == PathKind::Ground) {
            g.delay += Ps::from_s(q.phase_s);
            g.asf_ns += q.phase_s * 1e9;
            g.field_uv_m *= q.field;
            g.ecd_s += q.group_s - q.phase_s;
        }
        p
    }
}

/// Build the wrapper around the configured base model.
pub fn build(cfg: &UrbanGroundConfig, ctx: &BuildContext) -> Result<UrbanModel, String> {
    if matches!(*cfg.base, GroundConfig::Urban(_)) {
        return Err("urban: the base model must not itself be urban".into());
    }
    let base = super::ground::build(&cfg.base, ctx)?;
    UrbanModel::new(
        base,
        cfg.site_asf_ns,
        cfg.sites.clone(),
        cfg.structures.clone(),
        cfg.loop_distortion.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station() -> Station {
        Station {
            id: "M".into(),
            name: "M".into(),
            role: crate::signal::Role::Master,
            position: Geo::new(50.0, 0.0),
            power_kw: 100.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: crate::chain::StationStatus::On,
            faults: vec![],
        }
    }
    fn rx(lat: f64, lon: f64) -> RxState {
        RxState {
            t_s: 0.0,
            pos: Geo::new(lat, lon),
            heading_rad: 0.3,
            speed_mps: 0.0,
        }
    }
    fn ctx() -> BuildContext {
        BuildContext::new(crate::time::AbsTime::from_utc_iso("2026-01-01T00:00:00Z").unwrap(), 1.0, 0)
    }
    fn model(extra: &str) -> UrbanModel {
        let cfg: GroundConfig = toml::from_str(&format!(
            "model = \"urban\"\n{extra}\n[base]\nmodel = \"smooth_earth\"\nsigma_s_m = 0.01\n"
        ))
        .unwrap();
        match cfg {
            GroundConfig::Urban(u) => build(&u, &ctx()).unwrap(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn site_offsets_and_taper() {
        let m = model(
            "site_asf_ns = 10.0\n[[sites]]\nlat_deg = 51.0\nlon_deg = 0.0\nradius_m = 1000.0\nasf_ns = 30.0\nfield_db = -1.0",
        );
        let base = m.base.paths(&station(), &rx(51.0, 0.0))[0];
        let p = m.paths(&station(), &rx(51.0, 0.0))[0];
        assert!((p.asf_ns - base.asf_ns - 40.0).abs() < 1e-6);
        assert!(((p.delay - base.delay).as_ns() - 40.0).abs() < 0.01);
        assert!((20.0 * (p.field_uv_m / base.field_uv_m).log10() + 1.0).abs() < 1e-9);
        assert!((p.ecd_s - base.ecd_s).abs() < 1e-15);
        // halfway through the taper (1.5 km) the site adds half
        let q = crate::geo::direct(Geo::new(51.0, 0.0), 1.0, 1500.0);
        let b2 = m.base.paths(&station(), &rx(q.lat_deg, q.lon_deg))[0];
        let p2 = m.paths(&station(), &rx(q.lat_deg, q.lon_deg))[0];
        assert!((p2.asf_ns - b2.asf_ns - 25.0).abs() < 1e-3, "{}", p2.asf_ns - b2.asf_ns);
        // far away only the global offset
        let p3 = m.paths(&station(), &rx(51.2, 0.0))[0];
        let b3 = m.base.paths(&station(), &rx(51.2, 0.0))[0];
        assert!((p3.asf_ns - b3.asf_ns - 10.0).abs() < 1e-6);
    }

    #[test]
    fn structure_perturbation_decays_with_distance() {
        let m = model(
            "[[structures]]\nname = \"mast\"\nlat_deg = 51.0\nlon_deg = 0.2\nheight_m = 150.0\n[loop_distortion]\nquadrantal_deg = 0.0",
        );
        let s = Geo::new(51.0, 0.2);
        let st = station();
        // envelope of |Δdelay| in rings around the structure falls off
        let ring_max = |r: f64| -> f64 {
            (0..36)
                .map(|k| {
                    let p = crate::geo::direct(s, k as f64 * 10f64.to_radians(), r);
                    (m.perturbation(st.position, p).phase_s * 1e9).abs()
                })
                .fold(0.0, f64::max)
        };
        let (a, b, c) = (ring_max(100.0), ring_max(1000.0), ring_max(3000.0));
        println!("ring max |Δτ|: {a:.2} {b:.2} {c:.3} ns");
        assert!(a > b && b > c, "{a} {b} {c}");
        assert!(a > 5.0 && a < 200.0, "{a}");
        assert!(c < 0.1 * a);
        // beyond 12 radii nothing at all
        let far = crate::geo::direct(s, 0.0, 12.0 * 450.0 + 100.0);
        assert_eq!(m.perturbation(st.position, far).phase_s, 0.0);
        // bearing distortion near the structure, none far away; field changes too
        let near = crate::geo::direct(s, 1.0, 200.0);
        let e_near = m.bearing_error_rad(&st, &rx(near.lat_deg, near.lon_deg)).to_degrees();
        let e_far = m.bearing_error_rad(&st, &rx(far.lat_deg, far.lon_deg)).to_degrees();
        println!("bearing error near {e_near:.3}°, far {e_far:.3e}°");
        assert!(e_near.abs() > 0.05 && e_near.abs() < 30.0);
        assert!(e_far.abs() < 1e-9);
        let pn = m.perturbation(st.position, near);
        assert!((pn.field - 1.0).abs() > 1e-4 && pn.group_s != pn.phase_s);
    }

    #[test]
    fn quadrantal_error_and_validation() {
        let m = model("[loop_distortion]\nquadrantal_deg = 2.0\nstructures = false");
        let st = station();
        let r = rx(51.0, 1.0);
        let beta = inverse(r.pos, st.position).az1_rad;
        let want = 2.0 * (2.0 * (beta - r.heading_rad)).sin();
        assert!((m.bearing_error_rad(&st, &r).to_degrees() - want).abs() < 1e-9);
        // no loop_distortion: the geometric bearing
        let n = model("");
        assert!((n.apparent_bearing_rad(&st, &r) - beta).abs() < 1e-12);
        // invalid structures and a nested urban base are refused
        let bad: GroundConfig = toml::from_str(
            "model = \"urban\"\n[[structures]]\nlat_deg = 0.0\nlon_deg = 0.0\nheight_m = -1.0\n[base]\nmodel = \"simple\"",
        )
        .unwrap();
        assert!(super::super::ground::build(&bad, &ctx()).is_err());
        let nested: GroundConfig = toml::from_str(
            "model = \"urban\"\n[base]\nmodel = \"urban\"\n[base.base]\nmodel = \"simple\"",
        )
        .unwrap();
        assert!(super::super::ground::build(&nested, &ctx()).is_err());
    }
}
