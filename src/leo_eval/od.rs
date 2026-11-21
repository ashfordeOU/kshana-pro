// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! LEO orbit-determination and time-synchronisation integrity, DCB and PPP dependency risk
//! (B2107, evaluation only).
//!
//! * [`weights`]: the expected squared projections of a satellite position error onto the line
//!   of sight, averaged over the users that see the satellite above an elevation mask (users
//!   uniform over the visible Earth cap). For a satellite at altitude `h` on the z axis and a user
//!   at Earth-central angle `θ`, the unit line of sight `u` has radial component `u_z`; with
//!   radial, along-track and cross-track errors `R, A, C` and clock error `T` (m),
//!   `E[(u·Δ − T)²] = E[u_z²]R² − 2E[u_z]RT + T² + E[1 − u_z²](A² + C²)/2`, the azimuthal
//!   average splitting the transverse part equally between A and C. Computed numerically for the
//!   stated altitude, so no published weight is assumed.
//! * [`sisre`]: per satellite, the global-average signal-in-space range error of broadcast minus
//!   precise orbit and clock, its RMS, 95th percentile and maximum, the orbit-only part, and
//!   with a broadcast sigma, the Gaussian overbound check of `SISRE/σ`.
//! * [`dcb`]: day-to-day stability of differential code bias estimates and jumps larger than
//!   `k·√(σ₁² + σ₂²)`.
//! * [`deps`]: a PPP dependency register (correction service, orbit and clock products, ground
//!   segment, reference frame, network): a dependency with a single source, no fallback and loss
//!   of convergence on outage is critical.
//!
//! Evaluation of supplied products only; no onboard orbit-integrity method is built here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::transport::integrity::{overbound, Overbound};
use crate::transport::{quantile, Table};

/// Mean Earth radius (m).
const R_E: f64 = 6_371_000.0;

/// Line-of-sight weights for an altitude.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Weights {
    pub altitude_km: f64,
    pub elevation_mask_deg: f64,
    pub e_uz: f64,
    pub e_uz2: f64,
    pub e_transverse: f64,
}

/// Numeric weights for `altitude_km` and `mask_deg`.
pub fn weights(altitude_km: f64, mask_deg: f64) -> Result<Weights, String> {
    if !(altitude_km > 0.0 && (0.0..90.0).contains(&mask_deg)) {
        return Err("altitude must be positive and the mask in [0, 90) degrees".into());
    }
    let r = R_E + altitude_km * 1000.0;
    let m = mask_deg.to_radians();
    // Largest Earth-central angle with the satellite above the mask.
    let theta_max = (R_E / r * m.cos()).acos() - m;
    let n = 4000;
    let (mut w, mut s1, mut s2) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let th = (i as f64 + 0.5) / n as f64 * theta_max;
        let (ux, uz) = (-R_E * th.sin(), r - R_E * th.cos());
        let l = ux.hypot(uz);
        let c = uz / l; // radial component of the unit line of sight
        let a = th.sin(); // area weight
        w += a;
        s1 += a * c;
        s2 += a * c * c;
    }
    let (e1, e2) = (s1 / w, s2 / w);
    Ok(Weights {
        altitude_km,
        elevation_mask_deg: mask_deg,
        e_uz: e1,
        e_uz2: e2,
        e_transverse: (1.0 - e2) / 2.0,
    })
}

/// SISRE of one satellite.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SatSisre {
    pub sat: String,
    pub epochs: usize,
    pub rms_m: f64,
    pub p95_m: f64,
    pub max_m: f64,
    pub orbit_only_rms_m: f64,
    pub overbound: Option<Overbound>,
}

/// SISRE over `t,sat,dR_m,dA_m,dC_m,dclk_m[,sigma_m]`.
pub fn sisre(csv: &str, w: &Weights) -> Result<Vec<SatSisre>, String> {
    let t = Table::parse(csv)?;
    let (cs, cr, ca, cc, ck) = (
        t.need("sat")?,
        t.need("dR_m")?,
        t.need("dA_m")?,
        t.need("dC_m")?,
        t.need("dclk_m")?,
    );
    let cg = t.col("sigma_m");
    let mut by: BTreeMap<String, Vec<(f64, f64, f64)>> = BTreeMap::new();
    for i in 0..t.rows.len() {
        let n = |c| {
            t.num(i, Some(c))
                .and_then(|v| v.ok_or_else(|| format!("row {}: empty value", i + 2)))
        };
        let (r, a, c, k) = (n(cr)?, n(ca)?, n(cc)?, n(ck)?);
        let s2 = w.e_uz2 * r * r - 2.0 * w.e_uz * r * k + k * k + w.e_transverse * (a * a + c * c);
        let orbit = w.e_uz2 * r * r + w.e_transverse * (a * a + c * c);
        by.entry(t.text(i, Some(cs)).to_string())
            .or_default()
            .push((
                s2.max(0.0).sqrt(),
                orbit.max(0.0).sqrt(),
                t.num(i, cg)?.unwrap_or(f64::NAN),
            ));
    }
    Ok(by
        .into_iter()
        .map(|(sat, v)| {
            let s: Vec<f64> = v.iter().map(|x| x.0).collect();
            let o: Vec<f64> = v.iter().map(|x| x.1).collect();
            let rms = |x: &[f64]| (x.iter().map(|y| y * y).sum::<f64>() / x.len() as f64).sqrt();
            let z: Vec<f64> = v.iter().filter(|x| x.2 > 0.0).map(|x| x.0 / x.2).collect();
            SatSisre {
                epochs: v.len(),
                rms_m: rms(&s),
                p95_m: quantile(&s, 0.95).unwrap_or(f64::NAN),
                max_m: s.iter().cloned().fold(0.0, f64::max),
                orbit_only_rms_m: rms(&o),
                overbound: (z.len() >= 10).then(|| overbound(&z)),
                sat,
            }
        })
        .collect())
}

/// DCB stability of one satellite.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DcbSat {
    pub sat: String,
    pub days: usize,
    pub mean_ns: f64,
    pub day_to_day_std_ns: f64,
    /// `(date, step ns)` of jumps.
    pub jumps: Vec<(String, f64)>,
}

/// DCB over `date,sat,dcb_ns,sigma_ns` with jump factor `k`.
pub fn dcb(csv: &str, k: f64) -> Result<Vec<DcbSat>, String> {
    let t = Table::parse(csv)?;
    let (cd, cs, cv, cg) = (
        t.need("date")?,
        t.need("sat")?,
        t.need("dcb_ns")?,
        t.need("sigma_ns")?,
    );
    let mut by: BTreeMap<String, Vec<(String, f64, f64)>> = BTreeMap::new();
    for i in 0..t.rows.len() {
        let n = |c| {
            t.num(i, Some(c))
                .and_then(|v| v.ok_or_else(|| format!("row {}: empty value", i + 2)))
        };
        by.entry(t.text(i, Some(cs)).to_string())
            .or_default()
            .push((t.text(i, Some(cd)).to_string(), n(cv)?, n(cg)?));
    }
    Ok(by
        .into_iter()
        .map(|(sat, mut v)| {
            v.sort_by(|a, b| a.0.cmp(&b.0));
            let n = v.len() as f64;
            let mean = v.iter().map(|x| x.1).sum::<f64>() / n;
            let d: Vec<f64> = v.windows(2).map(|w| w[1].1 - w[0].1).collect();
            let std = if d.is_empty() {
                0.0
            } else {
                (d.iter().map(|x| x * x).sum::<f64>() / d.len() as f64 / 2.0).sqrt()
            };
            let jumps = v
                .windows(2)
                .filter(|w| (w[1].1 - w[0].1).abs() > k * (w[0].2.powi(2) + w[1].2.powi(2)).sqrt())
                .map(|w| (w[1].0.clone(), w[1].1 - w[0].1))
                .collect();
            DcbSat {
                sat,
                days: v.len(),
                mean_ns: mean,
                day_to_day_std_ns: std,
                jumps,
            }
        })
        .collect())
}

/// One dependency of a PPP service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub name: String,
    /// `correction-service`, `orbit-product`, `clock-product`, `ground-segment`,
    /// `reference-frame`, `network`, `other`.
    pub kind: String,
    pub single_source: bool,
    #[serde(default)]
    pub fallback: String,
    /// `convergence-lost`, `degraded` or `none`.
    pub outage_impact: String,
    #[serde(default)]
    pub evidence: String,
}

/// A dependency with its rating.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DepRisk {
    pub name: String,
    pub kind: String,
    /// `critical`, `high`, `moderate` or `low`.
    pub rating: &'static str,
    pub why: String,
}

/// Rate a dependency register: critical when single-source, no fallback and convergence is
/// lost; high when single-source with impact; moderate with a fallback and impact; else low.
pub fn deps(list: &[Dependency]) -> Result<Vec<DepRisk>, String> {
    list.iter()
        .map(|d| {
            if !["convergence-lost", "degraded", "none"].contains(&d.outage_impact.as_str()) {
                return Err(format!(
                    "dependency '{}': outage_impact '{}'",
                    d.name, d.outage_impact
                ));
            }
            let fb = !d.fallback.trim().is_empty();
            let rating = match (d.single_source, fb, d.outage_impact.as_str()) {
                (_, _, "none") => "low",
                (true, false, "convergence-lost") => "critical",
                (true, false, _) => "high",
                _ => "moderate",
            };
            Ok(DepRisk {
                name: d.name.clone(),
                kind: d.kind.clone(),
                rating,
                why: format!(
                    "{}; {}; on outage: {}",
                    if d.single_source {
                        "single source"
                    } else {
                        "several sources"
                    },
                    if fb {
                        format!("fallback: {}", d.fallback)
                    } else {
                        "no fallback".into()
                    },
                    d.outage_impact
                ),
            })
        })
        .collect()
}
