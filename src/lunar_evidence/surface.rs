// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! B2404: landing and surface navigation evidence. Independent checks a landing team, its
//! reviewer or its insurer can re-run; none of them navigates, estimates a state or fuses
//! sensors: they score results and configurations the team supplies.
//!
//! * **FDIR verification** (`fdir`): the team's fault-injection results (`case,run,detected,
//!   latency_s`) against a plan registered **before** the runs. The plan's SHA-256 is fixed in
//!   the request at registration; if the plan file no longer hashes to it the whole result says
//!   the plan changed after registration. Per fault case the timely-detection probability
//!   (detected within `max_latency_s`) has a Wilson 95 % interval judged against the plan's
//!   `min_detection`; per nominal case the false-alarm probability against `max_false_alarm`.
//!   **PASS** (the whole interval on the right side), **FAIL** (the whole interval on the wrong
//!   side) or **INSUFFICIENT RUNS**. Results for cases not in the plan are listed, not scored.
//!   The failure pattern it targets (altimeter or rangefinder loss or mis-rejection) is the one
//!   the research note (13, G-A) draws from public mission reports.
//! * **Configuration-state attestation** (`config`): the configuration as loaded for flight
//!   (`key,value`) against the reviewed configuration (a TOML `[expected]` table); every
//!   mismatch, missing key and unexpected key is listed (a disabled sensor-enable flag found
//!   late is the case in point).
//! * **Map-to-frame datum tie** (`datum`): control points (`id,map_x,map_y,map_z,ref_x,ref_y,
//!   ref_z,sigma_m`, lunar body-fixed metres) give the map's offset from the reference: a
//!   weighted translation, or a 7-parameter Helmert fit (Jacobian of the open
//!   `kshana::lunar_datum`) when the points span 100 km or more. The centroid offset is compared
//!   with the mean-Earth/principal-axis rotation at the site, taken from the open
//!   `kshana::lunar_frame` (DE421 ME→PA, NAIF `moon_080317.tf`): an offset that matches it says
//!   the map is in the other frame than declared (about 0.6 to 0.9 km at the surface). Verdict
//!   **TIED** (RMS within `max_rms_m` and no frame mismatch) or **NOT TIED**.
//! * **Tip-over probability** (`tipover`): Monte Carlo over the landing dispersion on a slope
//!   grid (`x_m,y_m,slope_deg`, regular, local east/north), with the DEM's slope uncertainty;
//!   a touchdown tips when the slope exceeds the static tip-over angle `atan(r/h)` less a
//!   dynamic margin. Also the probability of exceeding the design slope limit, and the
//!   sensitivity to half and double the dispersion. MODELLED: a geometric static model, not a
//!   touchdown dynamics simulation.
//!
//! Not built here (backlog B2404): the dust-plume degradation model, LTV/EVA safety evidence and
//! RF self-interference forensics.

use std::collections::{BTreeMap, BTreeSet};

use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use super::clocks::solve;
use super::{csv_rows, num, rnd, wilson95, Loaded};
use crate::lunar_pro::budget::std_normal;

// ---------------------------------------------------------------------------------------------
// FDIR
// ---------------------------------------------------------------------------------------------

/// `[fdir]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FdirReq {
    pub plan: String,
    /// SHA-256 of the plan file, recorded at registration.
    pub registered_sha256: String,
    pub results: String,
}

/// One planned case.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FdirCase {
    pub id: String,
    pub fault: String,
    pub expect_detect: bool,
    #[serde(default)]
    pub max_latency_s: Option<f64>,
    #[serde(default)]
    pub min_runs: usize,
}

/// The registered plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FdirPlan {
    pub id: String,
    #[serde(default)]
    pub registered_utc: String,
    pub min_detection: f64,
    pub max_false_alarm: f64,
    pub case: Vec<FdirCase>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseResult {
    pub id: String,
    pub fault: String,
    pub runs: usize,
    /// Timely detections (fault cases) or false alarms (nominal cases).
    pub events: usize,
    pub late: usize,
    pub latency_max_s: Option<f64>,
    pub probability: f64,
    pub wilson95: Option<(f64, f64)>,
    pub verdict: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FdirReport {
    pub schema: String,
    pub plan: String,
    pub plan_sha256: String,
    pub preregistered: bool,
    pub cases: Vec<CaseResult>,
    pub unplanned_cases: Vec<String>,
    pub verdict: String,
    pub evidence: String,
}

/// FDIR verification against a pre-registered plan.
pub fn fdir(l: &mut Loaded) -> Result<FdirReport, String> {
    let r = l
        .req
        .fdir
        .clone()
        .ok_or("the request has no [fdir] section")?;
    let plan_text = l.read(&r.plan, "fdir-plan")?;
    let sha = crate::forensics::inputs::sha256_hex(plan_text.as_bytes());
    let pre = sha.eq_ignore_ascii_case(r.registered_sha256.trim());
    let plan: FdirPlan = toml::from_str(&plan_text).map_err(|e| format!("{}: {e}", r.plan))?;
    let text = l.read(&r.results, "fdir-results")?;
    let mut by: BTreeMap<String, Vec<(bool, Option<f64>)>> = BTreeMap::new();
    for f in csv_rows(&text, &["case", "run", "detected", "latency_s"])? {
        let det = matches!(f[2].as_str(), "1" | "true" | "yes");
        let lat = if f[3].is_empty() {
            None
        } else {
            Some(num(&f[3], "latency_s")?)
        };
        by.entry(f[0].clone()).or_default().push((det, lat));
    }
    let mut cases = Vec::new();
    for c in &plan.case {
        let v = by.get(&c.id).cloned().unwrap_or_default();
        let n = v.len();
        let lmax = v
            .iter()
            .filter(|x| x.0)
            .filter_map(|x| x.1)
            .reduce(f64::max);
        let late = v
            .iter()
            .filter(|x| x.0 && matches!((x.1, c.max_latency_s), (Some(a), Some(m)) if a > m))
            .count();
        let events = if c.expect_detect {
            v.iter().filter(|x| x.0).count() - late
        } else {
            v.iter().filter(|x| x.0).count()
        };
        let w = wilson95(events, n);
        let verdict = match w {
            _ if n == 0 => "NO RESULTS".to_string(),
            _ if n < c.min_runs => "INSUFFICIENT RUNS".to_string(),
            Some((lo, hi)) if c.expect_detect => {
                if lo >= plan.min_detection {
                    "PASS".into()
                } else if hi < plan.min_detection {
                    "FAIL".into()
                } else {
                    "INSUFFICIENT RUNS".into()
                }
            }
            Some((lo, hi)) => {
                if hi <= plan.max_false_alarm {
                    "PASS".into()
                } else if lo > plan.max_false_alarm {
                    "FAIL".into()
                } else {
                    "INSUFFICIENT RUNS".into()
                }
            }
            None => "NO RESULTS".into(),
        };
        cases.push(CaseResult {
            id: c.id.clone(),
            fault: c.fault.clone(),
            runs: n,
            events,
            late,
            latency_max_s: lmax,
            probability: if n > 0 {
                rnd(events as f64 / n as f64, 5)
            } else {
                0.0
            },
            wilson95: w,
            verdict,
        });
    }
    let planned: BTreeSet<&str> = plan.case.iter().map(|c| c.id.as_str()).collect();
    let unplanned: Vec<String> = by
        .keys()
        .filter(|k| !planned.contains(k.as_str()))
        .cloned()
        .collect();
    let verdict = if !pre {
        "PLAN CHANGED AFTER REGISTRATION: results are not pre-registered evidence".to_string()
    } else if cases.iter().any(|c| c.verdict == "FAIL") {
        "FAIL".into()
    } else if cases.iter().all(|c| c.verdict == "PASS") {
        "PASS".into()
    } else {
        "INCOMPLETE".into()
    };
    Ok(FdirReport {
        schema: "kshana-pro/landing-fdir-verification/v1".into(),
        plan: plan.id,
        plan_sha256: sha,
        preregistered: pre,
        cases,
        unplanned_cases: unplanned,
        verdict,
        evidence: super::EVIDENCE.into(),
    })
}

// ---------------------------------------------------------------------------------------------
// Configuration-state attestation
// ---------------------------------------------------------------------------------------------

/// `[config]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigReq {
    pub expected: String,
    pub loaded: String,
    #[serde(default)]
    pub vehicle: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConfigReport {
    pub schema: String,
    pub vehicle: String,
    pub keys: usize,
    pub mismatched: Vec<[String; 3]>,
    pub missing: Vec<String>,
    pub unexpected: Vec<String>,
    pub verdict: String,
    pub evidence: String,
}

/// Configuration-state attestation.
pub fn config(l: &mut Loaded) -> Result<ConfigReport, String> {
    let r = l
        .req
        .config
        .clone()
        .ok_or("the request has no [config] section")?;
    #[derive(Deserialize)]
    struct Exp {
        expected: BTreeMap<String, toml::Value>,
    }
    let et = l.read(&r.expected, "reviewed-configuration")?;
    let exp: Exp = toml::from_str(&et).map_err(|e| format!("{}: {e}", r.expected))?;
    let show = |v: &toml::Value| match v {
        toml::Value::String(s) => s.clone(),
        o => o.to_string(),
    };
    let lt = l.read(&r.loaded, "loaded-configuration")?;
    let mut got: BTreeMap<String, String> = BTreeMap::new();
    for f in csv_rows(&lt, &["key", "value"])? {
        if got.insert(f[0].clone(), f[1].clone()).is_some() {
            return Err(format!(
                "key {} appears twice in the loaded configuration",
                f[0]
            ));
        }
    }
    let mut mismatched = Vec::new();
    let mut missing = Vec::new();
    for (k, v) in &exp.expected {
        match got.get(k) {
            None => missing.push(k.clone()),
            Some(g) if *g != show(v) => mismatched.push([k.clone(), show(v), g.clone()]),
            _ => {}
        }
    }
    let unexpected: Vec<String> = got
        .keys()
        .filter(|k| !exp.expected.contains_key(*k))
        .cloned()
        .collect();
    let ok = mismatched.is_empty() && missing.is_empty() && unexpected.is_empty();
    Ok(ConfigReport {
        schema: "kshana-pro/configuration-attestation/v1".into(),
        vehicle: r.vehicle,
        keys: exp.expected.len(),
        mismatched,
        missing,
        unexpected,
        verdict: if ok {
            "MATCHES the reviewed configuration"
        } else {
            "DIFFERS from the reviewed configuration"
        }
        .into(),
        evidence: super::EVIDENCE.into(),
    })
}

// ---------------------------------------------------------------------------------------------
// Datum tie
// ---------------------------------------------------------------------------------------------

fn d_rms() -> f64 {
    5.0
}
fn d_jd() -> f64 {
    2_451_545.0
}

/// `[datum]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatumReq {
    pub points: String,
    /// Declared frame of the map and of the reference (`PA` or `ME`).
    pub map_frame: String,
    pub ref_frame: String,
    #[serde(default)]
    pub map: String,
    #[serde(default = "d_rms")]
    pub max_rms_m: f64,
    #[serde(default = "d_jd")]
    pub epoch_jd_tdb: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DatumReport {
    pub schema: String,
    pub map: String,
    pub declared: String,
    pub points: usize,
    pub model: String,
    pub translation_m: [f64; 3],
    pub translation_sigma_m: [f64; 3],
    pub scale_ppm: Option<f64>,
    pub rotation_arcsec: Option<[f64; 3]>,
    pub centroid_offset_m: [f64; 3],
    pub rms_residual_m: f64,
    pub chi2_per_dof: f64,
    pub outliers: Vec<String>,
    /// The ME→PA displacement at the centroid (map ME, reference PA), m.
    pub me_pa_offset_m: [f64; 3],
    pub frame_finding: Option<String>,
    pub verdict: String,
    pub evidence: String,
}

type M3 = [[f64; 3]; 3];
fn mul(a: &M3, v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}
fn norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// The ME→PA rotation (`r_PA = R r_ME`) from the open engine's frame chain.
pub fn pa_from_me(jd_tdb: f64) -> M3 {
    let pa = kshana::lunar_frame::icrf_to_moon_pa(jd_tdb);
    let me = kshana::lunar_frame::icrf_to_iau_moon(jd_tdb);
    // PA · MEᵀ
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| pa[i][k] * me[j][k]).sum();
        }
    }
    r
}

/// Parameter count, translation, its sigma, scale (ppm), rotation (arcsec), residuals.
type Fit = (
    usize,
    [f64; 3],
    [f64; 3],
    Option<f64>,
    Option<[f64; 3]>,
    Vec<[f64; 3]>,
);

/// Fit a datum tie (library form): `pts` are `(id, map, ref, sigma)`.
#[allow(clippy::needless_range_loop)]
pub fn datum_fit(
    pts: &[(String, [f64; 3], [f64; 3], f64)],
    q: &DatumReq,
) -> Result<DatumReport, String> {
    if pts.len() < 3 {
        return Err("a datum tie needs three or more control points".into());
    }
    let w: Vec<f64> = pts.iter().map(|p| 1.0 / (p.3 * p.3)).collect();
    let sw: f64 = w.iter().sum();
    let c = [0, 1, 2].map(|k| pts.iter().map(|p| p.1[k]).sum::<f64>() / pts.len() as f64);
    let spread = pts
        .iter()
        .map(|p| norm([p.1[0] - c[0], p.1[1] - c[1], p.1[2] - c[2]]))
        .fold(0.0, f64::max);
    let off = [0, 1, 2].map(|k| {
        pts.iter()
            .zip(&w)
            .map(|(p, wi)| wi * (p.2[k] - p.1[k]))
            .sum::<f64>()
            / sw
    });
    let helmert = spread >= 100_000.0 && pts.len() >= 4;
    let (np, t, ts, scale, rot, resid): Fit = if helmert {
        // Unknowns about the centroid: [t(3), s, θ(3)], columns scaled to metres.
        let mut nm = vec![vec![0.0; 7]; 7];
        let mut b = [0.0; 7];
        let sc = [1.0, 1.0, 1.0, spread, spread, spread, spread];
        for (p, wi) in pts.iter().zip(&w) {
            let rel = [p.1[0] - c[0], p.1[1] - c[1], p.1[2] - c[2]];
            let j = kshana::lunar_datum::datum7_point_jacobian_body(rel);
            for row in 0..3 {
                let d = p.2[row] - p.1[row];
                for a in 0..7 {
                    b[a] += wi * j[row][a] * d / sc[a];
                    for bb in 0..7 {
                        nm[a][bb] += wi * j[row][a] * j[row][bb] / (sc[a] * sc[bb]);
                    }
                }
            }
        }
        let x = solve(&mut nm.clone(), &b).ok_or("Helmert normal equations are singular")?;
        let x: Vec<f64> = x.iter().zip(&sc).map(|(v, s)| v / s).collect();
        let var = |k: usize| {
            let mut e = [0.0; 7];
            e[k] = 1.0;
            solve(&mut nm.clone(), &e)
                .map(|v| v[k].max(0.0).sqrt() / sc[k])
                .unwrap_or(f64::NAN)
        };
        let res = pts
            .iter()
            .map(|p| {
                let rel = [p.1[0] - c[0], p.1[1] - c[1], p.1[2] - c[2]];
                let j = kshana::lunar_datum::datum7_point_jacobian_body(rel);
                [0, 1, 2].map(|r| p.2[r] - p.1[r] - (0..7).map(|a| j[r][a] * x[a]).sum::<f64>())
            })
            .collect();
        let as_ = 206_264.806;
        (
            7,
            [x[0], x[1], x[2]],
            [var(0), var(1), var(2)],
            Some(rnd(x[3] * 1e6, 4)),
            Some([rnd(x[4] * as_, 4), rnd(x[5] * as_, 4), rnd(x[6] * as_, 4)]),
            res,
        )
    } else {
        let s = (1.0 / sw).sqrt();
        let res = pts
            .iter()
            .map(|p| [0, 1, 2].map(|k| p.2[k] - p.1[k] - off[k]))
            .collect();
        (3, off, [s, s, s], None, None, res)
    };
    let n = pts.len();
    let chi2: f64 = resid
        .iter()
        .zip(&w)
        .map(|(r, wi)| wi * (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]))
        .sum();
    let dof = (3 * n).saturating_sub(np).max(1) as f64;
    let rms = (resid
        .iter()
        .map(|r| r[0] * r[0] + r[1] * r[1] + r[2] * r[2])
        .sum::<f64>()
        / n as f64)
        .sqrt();
    let outliers: Vec<String> = pts
        .iter()
        .zip(&resid)
        .filter(|(p, r)| norm(**r) / p.3 > 3.0 * 3f64.sqrt())
        .map(|(p, _)| p.0.clone())
        .collect();
    // ME→PA displacement at the centroid: map in ME, reference in PA.
    let rpm = pa_from_me(q.epoch_jd_tdb);
    let rmp = [0, 1, 2].map(|i| [rpm[0][i], rpm[1][i], rpm[2][i]]);
    let back = mul(&rmp, c);
    let d_me = [c[0] - back[0], c[1] - back[1], c[2] - back[2]];
    let tol = (3.0 * norm(ts)).max(0.1 * norm(d_me));
    let near = |s: f64| {
        norm([
            off[0] - s * d_me[0],
            off[1] - s * d_me[1],
            off[2] - s * d_me[2],
        ]) <= tol
    };
    let declared = format!("map {} / reference {}", q.map_frame, q.ref_frame);
    let frame_finding = if q.map_frame.eq_ignore_ascii_case(&q.ref_frame) {
        if near(1.0) {
            Some(format!("offset matches the ME→PA rotation at this site ({:.0} m): the map looks like mean-Earth (ME) while both are declared {}", norm(d_me), q.ref_frame))
        } else if near(-1.0) {
            Some(format!("offset matches the PA→ME rotation at this site ({:.0} m): the map looks like principal-axis (PA) while both are declared {}", norm(d_me), q.ref_frame))
        } else {
            None
        }
    } else if !near(if q.map_frame.eq_ignore_ascii_case("ME") {
        1.0
    } else {
        -1.0
    }) {
        Some("frames are declared different but the offset does not match the ME/PA rotation: transform the map before use and re-tie".into())
    } else {
        Some("frames declared different and the offset matches the ME/PA rotation: transform the map before use".into())
    };
    let tied = rms <= q.max_rms_m
        && frame_finding.is_none()
        && norm(off) <= q.max_rms_m.max(3.0 * norm(ts));
    Ok(DatumReport {
        schema: "kshana-pro/map-datum-tie/v1".into(),
        map: q.map.clone(),
        declared,
        points: n,
        model: if helmert {
            "helmert-7"
        } else {
            "translation-3 (points span under 100 km)"
        }
        .into(),
        translation_m: t.map(|v| rnd(v, 4)),
        translation_sigma_m: ts.map(|v| rnd(v, 4)),
        scale_ppm: scale,
        rotation_arcsec: rot,
        centroid_offset_m: off.map(|v| rnd(v, 4)),
        rms_residual_m: rnd(rms, 4),
        chi2_per_dof: rnd(chi2 / dof, 4),
        outliers,
        me_pa_offset_m: d_me.map(|v| rnd(v, 3)),
        frame_finding,
        verdict: if tied { "TIED" } else { "NOT TIED" }.into(),
        evidence: super::EVIDENCE.into(),
    })
}

/// Map-to-frame datum tie command.
pub fn datum(l: &mut Loaded) -> Result<DatumReport, String> {
    let q = l
        .req
        .datum
        .clone()
        .ok_or("the request has no [datum] section")?;
    for f in [&q.map_frame, &q.ref_frame] {
        if !(f.eq_ignore_ascii_case("PA") || f.eq_ignore_ascii_case("ME")) {
            return Err(format!("frame '{f}' must be PA or ME"));
        }
    }
    let text = l.read(&q.points, "control-points")?;
    let cols = [
        "id", "map_x", "map_y", "map_z", "ref_x", "ref_y", "ref_z", "sigma_m",
    ];
    let mut pts = Vec::new();
    for f in csv_rows(&text, &cols)? {
        let v: Vec<f64> = (1..8)
            .map(|i| num(&f[i], cols[i]))
            .collect::<Result<_, _>>()?;
        if v[6] <= 0.0 {
            return Err(format!("point {}: sigma_m must be positive", f[0]));
        }
        pts.push((f[0].clone(), [v[0], v[1], v[2]], [v[3], v[4], v[5]], v[6]));
    }
    datum_fit(&pts, &q)
}

// ---------------------------------------------------------------------------------------------
// Tip-over
// ---------------------------------------------------------------------------------------------

fn d_samples() -> usize {
    20_000
}
fn d_seed() -> u64 {
    1
}

/// `[tipover]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TipReq {
    pub slopes: String,
    #[serde(default)]
    pub target_m: [f64; 2],
    /// Landing dispersion, 1σ east and north (m).
    pub dispersion_m: [f64; 2],
    /// DEM slope uncertainty, 1σ (degrees).
    pub slope_sigma_deg: f64,
    pub footprint_radius_m: f64,
    pub cg_height_m: f64,
    #[serde(default)]
    pub dynamic_margin_deg: f64,
    #[serde(default)]
    pub design_slope_deg: Option<f64>,
    #[serde(default = "d_samples")]
    pub samples: usize,
    #[serde(default = "d_seed")]
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TipCase {
    pub dispersion_scale: f64,
    pub p_tip: f64,
    pub p_tip_wilson95: Option<(f64, f64)>,
    pub p_design_exceed: Option<f64>,
    pub off_map: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TipReport {
    pub schema: String,
    pub static_tip_angle_deg: f64,
    pub allowable_slope_deg: f64,
    pub samples: usize,
    pub cases: Vec<TipCase>,
    pub model: String,
    pub evidence: String,
}

/// A regular slope grid.
pub struct SlopeGrid {
    x0: f64,
    y0: f64,
    dx: f64,
    dy: f64,
    nx: usize,
    ny: usize,
    v: Vec<f64>,
}

impl SlopeGrid {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut pts = Vec::new();
        for f in csv_rows(text, &["x_m", "y_m", "slope_deg"])? {
            pts.push((
                num(&f[0], "x_m")?,
                num(&f[1], "y_m")?,
                num(&f[2], "slope_deg")?,
            ));
        }
        let axis = |sel: &dyn Fn(&(f64, f64, f64)) -> f64| -> Result<(f64, f64, usize), String> {
            let mut u: Vec<f64> = pts.iter().map(sel).collect();
            u.sort_by(|a, b| a.total_cmp(b));
            u.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            if u.len() < 2 {
                return Err("the slope grid needs two or more distinct x and y".into());
            }
            let d = u[1] - u[0];
            if u.windows(2)
                .any(|w| ((w[1] - w[0]) - d).abs() > 1e-6 * d.max(1.0))
            {
                return Err("the slope grid must be regular".into());
            }
            Ok((u[0], d, u.len()))
        };
        let (x0, dx, nx) = axis(&|p| p.0)?;
        let (y0, dy, ny) = axis(&|p| p.1)?;
        if pts.len() != nx * ny {
            return Err(format!(
                "the slope grid has {} cells, expected {nx} x {ny}",
                pts.len()
            ));
        }
        let mut v = vec![f64::NAN; nx * ny];
        for (x, y, s) in pts {
            let (i, j) = (
                ((x - x0) / dx).round() as usize,
                ((y - y0) / dy).round() as usize,
            );
            v[j * nx + i] = s;
        }
        Ok(SlopeGrid {
            x0,
            y0,
            dx,
            dy,
            nx,
            ny,
            v,
        })
    }

    /// Slope of the nearest cell, `None` off the grid.
    pub fn at(&self, x: f64, y: f64) -> Option<f64> {
        let i = ((x - self.x0) / self.dx).round();
        let j = ((y - self.y0) / self.dy).round();
        if i < 0.0 || j < 0.0 || i >= self.nx as f64 || j >= self.ny as f64 {
            return None;
        }
        Some(self.v[j as usize * self.nx + i as usize])
    }
}

/// Monte Carlo tip-over probability (library form).
pub fn tip_probability(g: &SlopeGrid, q: &TipReq) -> Result<TipReport, String> {
    if q.footprint_radius_m <= 0.0 || q.cg_height_m <= 0.0 || q.samples < 100 {
        return Err("footprint_radius_m and cg_height_m must be positive, samples ≥ 100".into());
    }
    let theta = (q.footprint_radius_m / q.cg_height_m).atan().to_degrees();
    let allow = theta - q.dynamic_margin_deg;
    let mut cases = Vec::new();
    for scale in [1.0, 0.5, 2.0] {
        let mut rng = rand::rngs::StdRng::seed_from_u64(q.seed);
        let (mut tip, mut design, mut off) = (0usize, 0usize, 0usize);
        for _ in 0..q.samples {
            let x = q.target_m[0] + scale * q.dispersion_m[0] * std_normal(&mut rng);
            let y = q.target_m[1] + scale * q.dispersion_m[1] * std_normal(&mut rng);
            let e = q.slope_sigma_deg * std_normal(&mut rng);
            match g.at(x, y) {
                None => off += 1,
                Some(s) => {
                    let s = (s + e).abs();
                    tip += (s > allow) as usize;
                    design += q.design_slope_deg.is_some_and(|d| s > d) as usize;
                }
            }
        }
        let n = q.samples - off;
        cases.push(TipCase {
            dispersion_scale: scale,
            p_tip: if n > 0 {
                rnd(tip as f64 / n as f64, 6)
            } else {
                f64::NAN
            },
            p_tip_wilson95: wilson95(tip, n),
            p_design_exceed: q.design_slope_deg.map(|_| {
                if n > 0 {
                    rnd(design as f64 / n as f64, 6)
                } else {
                    f64::NAN
                }
            }),
            off_map: off,
        });
    }
    Ok(TipReport {
        schema: "kshana-pro/landing-tipover/v1".into(),
        static_tip_angle_deg: rnd(theta, 3),
        allowable_slope_deg: rnd(allow, 3),
        samples: q.samples,
        cases,
        model: "static geometric tip-over: slope > atan(r/h) - dynamic margin; Gaussian landing \
                dispersion; Gaussian DEM slope error; nearest grid cell; off-map samples excluded \
                and counted. Not a touchdown dynamics simulation."
            .into(),
        evidence: super::EVIDENCE.into(),
    })
}

/// Tip-over command.
pub fn tipover(l: &mut Loaded) -> Result<TipReport, String> {
    let q = l
        .req
        .tipover
        .clone()
        .ok_or("the request has no [tipover] section")?;
    let text = l.read(&q.slopes, "slope-grid")?;
    tip_probability(&SlopeGrid::parse(&text)?, &q)
}
