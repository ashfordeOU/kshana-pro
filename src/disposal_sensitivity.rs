// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Disposal-plan sensitivity study: lifetime against solar activity and ballistic coefficient
//!
//! The space-operations disposal check ([`crate::space_ops::disposal`]) estimates one lifetime in
//! the static US Standard Atmosphere 1976, which has no solar-activity dependence, and says so.
//! A disposal plan stands or falls on how that lifetime moves with solar activity and with the
//! uncertainty of the ballistic coefficient. This study runs the same circular-orbit decay
//! `da/dt = −√(GM a) ρ B` (`B = Cd A / m`) to 120 km over a grid:
//!
//! * **solar-activity bands**: constant F10.7, 81-day F10.7 and Kp per band (defaults: low,
//!   moderate, high), with density from the open engine's Jacchia 1971 thermosphere at the
//!   diurnal-mean exospheric temperature ([`kshana::space_weather::space_weather_density`]);
//! * a **solar-cycle case**: F10.7 varying sinusoidally between the stated minimum and maximum
//!   over an 11-year cycle, starting at minimum and, separately, at maximum;
//! * **ballistic coefficient**: nominal, low and high by the stated relative uncertainty;
//! * the **US76 baseline** of the existing disposal check, for reference.
//!
//! Output: the lifetime of every case, the range, how many cases fall within the plan's own
//! deadline, and the elasticity of the lifetime to the ballistic coefficient and to F10.7. The
//! wording is "within / beyond the plan's deadline under this model"; nothing here says a plan
//! meets a rule. **MODELLED**.

use kshana::space_weather::{space_weather_density, SpaceWeather};
use serde::{Deserialize, Serialize};

use crate::forensics::inputs::round;

/// Pro feature.
pub const FEATURE: &str = "pro-disposal-sensitivity";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-disposal-sensitivity/1";
/// Label.
pub const EVIDENCE: &str = "MODELLED: circular-orbit drag decay to 120 km with Jacchia 1971 \
density at the diurnal-mean exospheric temperature (open engine), constant or sinusoidal solar \
activity, and a stated ballistic-coefficient spread. A sensitivity study for a disposal plan, not \
a lifetime prediction for a real object and not a regulatory determination.";

const GM: f64 = 3.986_004_418e14;
const RE: f64 = 6_378_137.0;
const YEAR_S: f64 = 365.25 * 86_400.0;
const CYCLE_Y: f64 = 11.0;

/// A solar-activity band.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Band {
    pub name: String,
    pub f107: f64,
    #[serde(default)]
    pub f107a: Option<f64>,
    pub kp: f64,
}

/// The sinusoidal solar cycle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cycle {
    pub f107_min: f64,
    pub f107_max: f64,
    pub kp: f64,
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub altitude_km: f64,
    pub mass_kg: f64,
    pub area_m2: f64,
    #[serde(default = "d_cd")]
    pub cd: f64,
    /// Relative 1-sigma-style spread of `Cd A / m` used for the low and high cases (0.2 = ±20 %).
    pub bc_uncertainty: f64,
    pub plan_deadline_years: f64,
    pub plan_source: String,
    #[serde(default = "d_bands", rename = "band")]
    pub bands: Vec<Band>,
    #[serde(default = "d_cycle")]
    pub cycle: Option<Cycle>,
    /// Stop at this many years.
    #[serde(default = "d_limit")]
    pub limit_years: f64,
}

fn d_cd() -> f64 {
    2.2
}
fn d_limit() -> f64 {
    200.0
}
fn d_bands() -> Vec<Band> {
    vec![
        Band {
            name: "low".into(),
            f107: 70.0,
            f107a: None,
            kp: 1.0,
        },
        Band {
            name: "moderate".into(),
            f107: 130.0,
            f107a: None,
            kp: 2.0,
        },
        Band {
            name: "high".into(),
            f107: 200.0,
            f107a: None,
            kp: 3.0,
        },
    ]
}
fn d_cycle() -> Option<Cycle> {
    Some(Cycle {
        f107_min: 70.0,
        f107_max: 200.0,
        kp: 2.0,
    })
}

/// One case.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Case {
    pub activity: String,
    pub bc: String,
    pub bc_m2_kg: f64,
    pub lifetime_years: Option<f64>,
    pub within_plan: bool,
}

/// The study.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Study {
    pub format: &'static str,
    pub evidence: &'static str,
    pub request: Request,
    pub cases: Vec<Case>,
    pub us76_nominal_years: Option<f64>,
    pub min_years: Option<f64>,
    pub max_years: Option<f64>,
    pub within_plan: usize,
    pub total: usize,
    /// d ln(lifetime) / d ln(B) at moderate activity (from the low and high cases).
    pub elasticity_bc: Option<f64>,
    /// d ln(lifetime) / d ln(F10.7) between the low and high bands at nominal B.
    pub elasticity_f107: Option<f64>,
    pub limits: Vec<String>,
    pub verdict: String,
}

/// Lifetime (years) to 120 km under `rho(alt_m, t_s)`; `None` beyond `limit_y`.
fn lifetime<F: Fn(f64, f64) -> f64>(alt_km: f64, b: f64, limit_y: f64, rho: F) -> Option<f64> {
    let mut a = RE + alt_km * 1000.0;
    let mut t = 0.0;
    let limit = limit_y * YEAR_S;
    while a - RE > 120_000.0 && t < limit {
        let dadt = -(GM * a).sqrt() * rho(a - RE, t) * b;
        let dt = if dadt < 0.0 {
            (500.0 / -dadt).min(5.0 * 86_400.0)
        } else {
            5.0 * 86_400.0
        };
        a += dadt * dt;
        t += dt;
    }
    (a - RE <= 120_000.0).then_some(t / YEAR_S)
}

/// Run the study.
pub fn study(q: &Request) -> Result<Study, String> {
    if !(q.altitude_km > 120.0
        && q.mass_kg > 0.0
        && q.area_m2 > 0.0
        && q.cd > 0.0
        && q.plan_deadline_years > 0.0
        && q.limit_years > 0.0
        && (0.0..1.0).contains(&q.bc_uncertainty))
    {
        return Err("altitude above 120 km; positive mass, area, cd, deadline and limit; bc_uncertainty in [0, 1)".into());
    }
    if q.plan_source.trim().is_empty() {
        return Err("plan_source must say where the deadline comes from".into());
    }
    if q.bands.is_empty() && q.cycle.is_none() {
        return Err("give at least one band or the cycle".into());
    }
    let b0 = q.cd * q.area_m2 / q.mass_kg;
    let bcs = [
        ("low", b0 * (1.0 - q.bc_uncertainty)),
        ("nominal", b0),
        ("high", b0 * (1.0 + q.bc_uncertainty)),
    ];
    let mut cases = vec![];
    let mut push = |activity: String, bc: &str, b: f64, y: Option<f64>| {
        cases.push(Case {
            activity,
            bc: bc.into(),
            bc_m2_kg: round(b, 6),
            lifetime_years: y.map(|v| round(v, 2)),
            within_plan: y.is_some_and(|v| v <= q.plan_deadline_years),
        })
    };
    for band in &q.bands {
        let sw = SpaceWeather {
            f107: band.f107,
            f107a: band.f107a.unwrap_or(band.f107),
            kp: band.kp,
        };
        for (n, b) in bcs {
            let y = lifetime(q.altitude_km, b, q.limit_years, |h, _| {
                space_weather_density(h, &sw)
            });
            push(format!("band {}", band.name), n, b, y);
        }
    }
    if let Some(c) = &q.cycle {
        for (start, phase) in [
            ("cycle from minimum", std::f64::consts::PI),
            ("cycle from maximum", 0.0),
        ] {
            for (n, b) in bcs {
                let y = lifetime(q.altitude_km, b, q.limit_years, |h, t| {
                    let x = (2.0 * std::f64::consts::PI * t / (CYCLE_Y * YEAR_S) + phase).cos();
                    let f = c.f107_min + (c.f107_max - c.f107_min) * (1.0 + x) / 2.0;
                    space_weather_density(
                        h,
                        &SpaceWeather {
                            f107: f,
                            f107a: f,
                            kp: c.kp,
                        },
                    )
                });
                push(start.into(), n, b, y);
            }
        }
    }
    let us76 = lifetime(q.altitude_km, b0, q.limit_years, |h, _| {
        kshana::reentry::us76_density(h)
    });
    let ys: Vec<f64> = cases.iter().filter_map(|c| c.lifetime_years).collect();
    let beyond = cases.iter().any(|c| c.lifetime_years.is_none());
    let min_years = ys.iter().copied().reduce(f64::min);
    let max_years = if beyond {
        None
    } else {
        ys.iter().copied().reduce(f64::max)
    };
    let get = |act: &str, bc: &str| {
        cases
            .iter()
            .find(|c| c.activity == act && c.bc == bc)
            .and_then(|c| c.lifetime_years)
    };
    let mid = q
        .bands
        .get(q.bands.len() / 2)
        .map(|b| format!("band {}", b.name));
    let elasticity_bc = mid.as_ref().and_then(|m| {
        let (l, h) = (get(m, "low")?, get(m, "high")?);
        let r = (1.0 + q.bc_uncertainty) / (1.0 - q.bc_uncertainty);
        (q.bc_uncertainty > 0.0).then(|| round((h / l).ln() / r.ln(), 3))
    });
    let elasticity_f107 = match (q.bands.first(), q.bands.last()) {
        (Some(lo), Some(hi)) if hi.f107 > lo.f107 => {
            let (a, b) = (
                get(&format!("band {}", lo.name), "nominal"),
                get(&format!("band {}", hi.name), "nominal"),
            );
            match (a, b) {
                (Some(a), Some(b)) => Some(round((b / a).ln() / (hi.f107 / lo.f107).ln(), 3)),
                _ => None,
            }
        }
        _ => None,
    };
    let within = cases.iter().filter(|c| c.within_plan).count();
    let total = cases.len();
    let verdict = format!(
        "Lifetime from {} to {} across {total} cases; {within} of {total} within the plan's {} years ({}) under this model (MODELLED)",
        min_years.map(|v| format!("{v:.1} years")).unwrap_or("-".into()),
        max_years
            .map(|v| format!("{v:.1} years"))
            .unwrap_or(format!("beyond {} years", q.limit_years)),
        q.plan_deadline_years,
        q.plan_source
    );
    Ok(Study {
        format: FORMAT,
        evidence: EVIDENCE,
        request: q.clone(),
        cases,
        us76_nominal_years: us76.map(|v| round(v, 2)),
        min_years: min_years.map(|v| round(v, 2)),
        max_years: max_years.map(|v| round(v, 2)),
        within_plan: within,
        total,
        elasticity_bc,
        elasticity_f107,
        limits: vec![
            "Circular orbit and orbit-averaged density at the diurnal-mean exospheric temperature; no eccentricity, attitude-dependent area or lift.".into(),
            "Solar activity is constant per band or a sinusoid between stated bounds; real cycles differ in amplitude and length, and storms are not modelled.".into(),
            "The ballistic-coefficient cases are the stated spread, not a probability distribution.".into(),
            "A sensitivity study: it says how the lifetime moves, not what it will be for a real object, and makes no regulatory determination.".into(),
        ],
        verdict,
    })
}

/// The report section (for the space-ops page).
pub fn section(s: &Study) -> crate::timing::grid::Section<'static> {
    let mut rows: Vec<Vec<String>> = s
        .cases
        .iter()
        .map(|c| {
            vec![
                c.activity.clone(),
                c.bc.clone(),
                format!("{:.5}", c.bc_m2_kg),
                c.lifetime_years
                    .map(|y| format!("{y:.1}"))
                    .unwrap_or_else(|| format!("> {}", s.request.limit_years)),
                if c.within_plan { "within" } else { "beyond" }.into(),
            ]
        })
        .collect();
    rows.push(vec![
        "US76 static (reference)".into(),
        "nominal".into(),
        String::new(),
        s.us76_nominal_years
            .map(|y| format!("{y:.1}"))
            .unwrap_or_else(|| format!("> {}", s.request.limit_years)),
        String::new(),
    ]);
    (
        "Disposal-plan sensitivity: lifetime by solar activity and ballistic coefficient".into(),
        vec![
            "activity",
            "B",
            "Cd·A/m (m²/kg)",
            "lifetime (years)",
            "plan deadline",
        ],
        rows,
    )
}

/// A SYNTHETIC request: 100 kg, 1 m², 550 km, ±20 % on B, a 5-year plan deadline.
pub fn synth() -> String {
    "title = \"SYNTHETIC disposal-plan sensitivity\"\naltitude_km = 550.0\nmass_kg = 100.0\narea_m2 = 1.0\ncd = 2.2\nbc_uncertainty = 0.2\nplan_deadline_years = 5.0\nplan_source = \"SYNTHETIC operator disposal plan\"\n".into()
}
