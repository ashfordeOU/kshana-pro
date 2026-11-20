// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Dust-plume visibility: a MODELLED parametric model (E24 B2404)
//!
//! A descending (or hovering) engine erodes regolith and throws it out in a thin sheet at a
//! low angle above the local horizontal. This module asks, for lines of sight the user
//! declares (a surface beacon to the lander, a camera to a landmark, a relay to a rover),
//! how much that sheet attenuates the optical and the RF path as the engine comes down.
//!
//! **The model and its assumptions** (all stated in every output, [`ASSUMPTIONS`]):
//!
//! 1. Erosion rate against engine height `H`: `Ṁ(H) = Ṁ₀·exp(−H/H_s)` below an onset height
//!    `H_on`, zero above it and after engine cut-off. `Ṁ₀`, `H_s` and `H_on` are the user's.
//! 2. Monodisperse spherical particles of radius `a`, bulk density `ρ`, ejected radially from
//!    the landing point at one speed `v` and one angle `θ` above the horizontal; ballistic
//!    curvature and gravity are neglected over the ranges considered (`r ≤ r_max`).
//! 3. The sheet at range `r` fills `0 ≤ z ≤ h(r) = h₀ + r·tan θ` above a flat surface, between
//!    `r_min` (the eroded zone) and `r_max`; mass conservation through the cylinder of
//!    radius `r` gives the number density `n(r,t) = Ṁ(t − r/v) / (m_p · v · 2πr·h(r))`, with
//!    the travel time `r/v` as a retarded time.
//! 4. Optical extinction in the large-particle limit, `Q_ext = 2` (valid when `2πa/λ ≫ 1`).
//! 5. RF extinction in the Rayleigh limit (valid when `x = 2πa/λ ≪ 1`; the output flags
//!    `x > 0.3`) with the regolith's complex relative permittivity `ε = ε′ + iε″`:
//!    `Q_abs = 4x·Im[(ε−1)/(ε+2)]`, `Q_sca = (8/3)x⁴·|(ε−1)/(ε+2)|²` (Bohren and Huffman,
//!    *Absorption and Scattering of Light by Small Particles*, 1983, §5.2).
//! 6. Extinction coefficient `σ = n·πa²·Q`; optical depth `τ = ∫σ ds` along each line of
//!    sight (sampled), attenuation `10·log₁₀(e)·τ` dB; single scattering, no multiple
//!    scattering or forward-scatter gain.
//!
//! **MODELLED.** A parametric screening model to bound when a line of sight is likely to be
//! lost and how much link margin to hold. It is not validated against Apollo, CLPS or test
//! stand plume measurements; its parameters have no defaults because the right values are
//! the mission's to state (the synthetic example uses SYNTHETIC values).

use serde::{Deserialize, Serialize};

use super::{positive, r, EVIDENCE};

/// Pro feature.
pub const FEATURE: &str = "pro-lunar-plume-visibility";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-lunar-plume-visibility/1";
/// Method identifier.
pub const METHOD_ID: &str = "kshana-pro/plume-sheet-extinction/v1";
/// Speed of light (m/s).
const C: f64 = 299_792_458.0;
/// dB per neper of optical depth.
const DB_PER_TAU: f64 = 4.342_944_819_032_518;

/// The assumptions, written into every output.
pub const ASSUMPTIONS: &[&str] = &[
    "Erosion rate Mdot(H) = Mdot0 * exp(-H / H_s) below the onset height, zero above it and after cut-off (user parameters).",
    "Monodisperse spheres of radius a and density rho, ejected radially at one speed v and one angle theta; gravity and ballistic curvature neglected within r_max.",
    "Sheet fills 0 <= z <= h0 + r tan(theta) between r_min and r_max over a flat surface; number density from mass conservation through the cylinder of radius r, at the retarded time t - r/v.",
    "Optical extinction efficiency Q_ext = 2 (large-particle limit).",
    "RF extinction in the Rayleigh limit with the stated complex permittivity (Bohren and Huffman 1983, section 5.2); flagged when the size parameter exceeds 0.3.",
    "Single scattering; no forward-scatter gain, no multipath from the surface, no electrostatic levitation or lofted fines.",
];

/// The descent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descent {
    /// Engine height above the surface at the start (m).
    pub start_height_m: f64,
    /// Descent rate (m/s).
    pub rate_mps: f64,
    /// Hover at touchdown height before cut-off (s).
    #[serde(default)]
    pub hover_s: f64,
    /// Height of the engine exit at touchdown (m).
    #[serde(default)]
    pub touchdown_height_m: f64,
    /// Time step (s).
    pub step_s: f64,
}

/// The erosion and particle parameters (no defaults: the mission states them).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ejecta {
    /// Erosion rate at zero height (kg/s).
    pub mdot0_kg_s: f64,
    /// Scale height of the erosion rate (m).
    pub scale_height_m: f64,
    /// Height above which no erosion occurs (m).
    pub onset_height_m: f64,
    /// Particle radius (m).
    pub particle_radius_m: f64,
    /// Particle bulk density (kg/m³).
    pub particle_density_kg_m3: f64,
    /// Ejection speed (m/s).
    pub speed_mps: f64,
    /// Ejection angle above the horizontal (deg).
    pub angle_deg: f64,
    /// Sheet thickness at the inner edge (m).
    pub base_thickness_m: f64,
    /// Inner edge of the sheet (m).
    pub r_min_m: f64,
    /// Outer edge of the sheet (m).
    pub r_max_m: f64,
}

/// The RF channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rf {
    pub frequency_hz: f64,
    /// Real part of the relative permittivity of the particles.
    pub permittivity_real: f64,
    /// Imaginary (loss) part, positive for a lossy medium.
    pub permittivity_imag: f64,
}

/// A declared line of sight: `[east, north, up]` metres from the landing point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sight {
    pub name: String,
    pub from: [f64; 3],
    pub to: [f64; 3],
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub descent: Descent,
    pub ejecta: Ejecta,
    pub rf: Rf,
    pub sights: Vec<Sight>,
    /// A line of sight is "obscured" above this optical attenuation (dB).
    #[serde(default = "d_thr")]
    pub optical_threshold_db: f64,
    /// RF attenuation above which the RF link is flagged (dB).
    #[serde(default = "d_rf_thr")]
    pub rf_threshold_db: f64,
    /// Samples along each line of sight.
    #[serde(default = "d_samples")]
    pub samples: usize,
}

fn d_thr() -> f64 {
    3.0
}
fn d_rf_thr() -> f64 {
    1.0
}
fn d_samples() -> usize {
    400
}

/// One time step of one line of sight.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Step {
    pub t_s: f64,
    pub height_m: f64,
    pub mdot_kg_s: f64,
    pub optical_db: f64,
    pub rf_db: f64,
}

/// Summary of one line of sight.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SightResult {
    pub name: String,
    pub max_optical_db: f64,
    pub max_rf_db: f64,
    /// Engine height when the optical threshold is first crossed.
    pub obscured_from_height_m: Option<f64>,
    pub obscured_from_t_s: Option<f64>,
    pub obscured_s: f64,
    pub rf_flagged_s: f64,
    pub steps: Vec<Step>,
}

/// The result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub method: &'static str,
    pub title: String,
    pub evidence: &'static str,
    pub assumptions: Vec<&'static str>,
    pub request: Request,
    /// Particle mass (kg).
    pub particle_mass_kg: f64,
    /// RF size parameter `2πa/λ`.
    pub rf_size_parameter: f64,
    /// RF extinction efficiency used.
    pub rf_q_ext: f64,
    pub warnings: Vec<String>,
    pub sights: Vec<SightResult>,
    pub verdict: String,
}

fn validate(q: &Request) -> Result<(), String> {
    let d = &q.descent;
    let e = &q.ejecta;
    positive("descent.start_height_m", d.start_height_m)?;
    positive("descent.rate_mps", d.rate_mps)?;
    positive("descent.step_s", d.step_s)?;
    if d.hover_s.is_nan()
        || d.hover_s < 0.0
        || d.touchdown_height_m.is_nan()
        || d.touchdown_height_m < 0.0
    {
        return Err(
            "descent.hover_s and descent.touchdown_height_m must be zero or positive".into(),
        );
    }
    if d.touchdown_height_m >= d.start_height_m {
        return Err("descent.touchdown_height_m must be below start_height_m".into());
    }
    for (n, v) in [
        ("ejecta.mdot0_kg_s", e.mdot0_kg_s),
        ("ejecta.scale_height_m", e.scale_height_m),
        ("ejecta.onset_height_m", e.onset_height_m),
        ("ejecta.particle_radius_m", e.particle_radius_m),
        ("ejecta.particle_density_kg_m3", e.particle_density_kg_m3),
        ("ejecta.speed_mps", e.speed_mps),
        ("ejecta.angle_deg", e.angle_deg),
        ("ejecta.base_thickness_m", e.base_thickness_m),
        ("ejecta.r_min_m", e.r_min_m),
        ("ejecta.r_max_m", e.r_max_m),
        ("rf.frequency_hz", q.rf.frequency_hz),
        ("rf.permittivity_real", q.rf.permittivity_real),
    ] {
        positive(n, v)?;
    }
    if e.angle_deg >= 45.0 {
        return Err("ejecta.angle_deg must be below 45 (a low-angle sheet)".into());
    }
    if e.r_max_m <= e.r_min_m {
        return Err("ejecta.r_max_m must exceed r_min_m".into());
    }
    if q.rf.permittivity_imag.is_nan() || q.rf.permittivity_imag < 0.0 {
        return Err("rf.permittivity_imag must be zero or positive".into());
    }
    if q.sights.is_empty() {
        return Err("at least one [[sights]] entry is needed".into());
    }
    if q.samples < 10 {
        return Err("samples must be at least 10".into());
    }
    let steps = (d.start_height_m - d.touchdown_height_m) / d.rate_mps / d.step_s;
    if steps > 200_000.0 {
        return Err("descent has more than 200000 steps: raise step_s".into());
    }
    Ok(())
}

/// Rayleigh extinction efficiency `(Q_ext, x)` for radius `a`, frequency `f` and permittivity.
pub fn rayleigh_q(a_m: f64, f_hz: f64, eps_re: f64, eps_im: f64) -> (f64, f64) {
    let x = 2.0 * std::f64::consts::PI * a_m * f_hz / C;
    // K = (ε − 1)/(ε + 2) with ε = ε′ + iε″.
    let (nr, ni) = (eps_re - 1.0, eps_im);
    let (dr, di) = (eps_re + 2.0, eps_im);
    let den = dr * dr + di * di;
    let (kr, ki) = ((nr * dr + ni * di) / den, (ni * dr - nr * di) / den);
    let q_abs = 4.0 * x * ki;
    let q_sca = 8.0 / 3.0 * x.powi(4) * (kr * kr + ki * ki);
    (q_abs + q_sca, x)
}

/// Engine height at time `t` and whether the engine is firing.
fn height_at(d: &Descent, t: f64) -> (f64, bool) {
    let t_down = (d.start_height_m - d.touchdown_height_m) / d.rate_mps;
    if t <= t_down {
        (d.start_height_m - d.rate_mps * t, true)
    } else {
        (d.touchdown_height_m, t <= t_down + d.hover_s)
    }
}

fn mdot(e: &Ejecta, d: &Descent, t: f64) -> f64 {
    if t < 0.0 {
        return 0.0;
    }
    let (h, firing) = height_at(d, t);
    if !firing || h > e.onset_height_m {
        0.0
    } else {
        e.mdot0_kg_s * (-h / e.scale_height_m).exp()
    }
}

/// Optical depth along a line of sight at time `t` for efficiency `q`.
fn tau(q: &Request, s: &Sight, t: f64, eff: f64, m_p: f64) -> f64 {
    let e = &q.ejecta;
    let tan = e.angle_deg.to_radians().tan();
    let (a, b) = (s.from, s.to);
    let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
    let ds = len / q.samples as f64;
    let cross = std::f64::consts::PI * e.particle_radius_m.powi(2) * eff;
    let mut acc = 0.0;
    for i in 0..q.samples {
        let f = (i as f64 + 0.5) / q.samples as f64;
        let p = [
            a[0] + f * (b[0] - a[0]),
            a[1] + f * (b[1] - a[1]),
            a[2] + f * (b[2] - a[2]),
        ];
        let rr = p[0].hypot(p[1]);
        if rr < e.r_min_m || rr > e.r_max_m {
            continue;
        }
        let h = e.base_thickness_m + rr * tan;
        if p[2] < 0.0 || p[2] > h {
            continue;
        }
        let md = mdot(e, &q.descent, t - rr / e.speed_mps);
        if md <= 0.0 {
            continue;
        }
        let n = md / (m_p * e.speed_mps * 2.0 * std::f64::consts::PI * rr * h);
        acc += n * cross * ds;
    }
    acc
}

/// Run the model.
pub fn run(q: &Request) -> Result<Report, String> {
    validate(q)?;
    let e = &q.ejecta;
    let d = &q.descent;
    let m_p =
        4.0 / 3.0 * std::f64::consts::PI * e.particle_radius_m.powi(3) * e.particle_density_kg_m3;
    let (rf_q, x) = rayleigh_q(
        e.particle_radius_m,
        q.rf.frequency_hz,
        q.rf.permittivity_real,
        q.rf.permittivity_imag,
    );
    let mut warnings = Vec::new();
    if x > 0.3 {
        warnings.push(format!(
            "RF size parameter {x:.3} exceeds 0.3: the Rayleigh limit is not valid; RF attenuation is indicative only"
        ));
    }
    // Large-particle optical limit at 550 nm.
    let x_opt = 2.0 * std::f64::consts::PI * e.particle_radius_m / 550e-9;
    if x_opt < 10.0 {
        warnings.push(format!(
            "optical size parameter {x_opt:.1} at 550 nm is below 10: Q_ext = 2 is a rough value"
        ));
    }
    let t_down = (d.start_height_m - d.touchdown_height_m) / d.rate_mps;
    // Run past cut-off long enough for the last particles to cross r_max.
    let t_end = t_down + d.hover_s + e.r_max_m / e.speed_mps + d.step_s;
    let mut sights = Vec::new();
    for s in &q.sights {
        let mut steps = Vec::new();
        let mut t = 0.0;
        while t <= t_end {
            let (h, _) = height_at(d, t);
            let opt = DB_PER_TAU * tau(q, s, t, 2.0, m_p);
            let rf = DB_PER_TAU * tau(q, s, t, rf_q, m_p);
            steps.push(Step {
                t_s: r(t, 3),
                height_m: r(h, 3),
                mdot_kg_s: mdot(e, d, t),
                optical_db: opt,
                rf_db: rf,
            });
            t += d.step_s;
        }
        let first = steps.iter().find(|x| x.optical_db > q.optical_threshold_db);
        sights.push(SightResult {
            name: s.name.clone(),
            max_optical_db: steps.iter().map(|x| x.optical_db).fold(0.0, f64::max),
            max_rf_db: steps.iter().map(|x| x.rf_db).fold(0.0, f64::max),
            obscured_from_height_m: first.map(|x| x.height_m),
            obscured_from_t_s: first.map(|x| x.t_s),
            obscured_s: r(
                steps
                    .iter()
                    .filter(|x| x.optical_db > q.optical_threshold_db)
                    .count() as f64
                    * d.step_s,
                3,
            ),
            rf_flagged_s: r(
                steps.iter().filter(|x| x.rf_db > q.rf_threshold_db).count() as f64 * d.step_s,
                3,
            ),
            steps,
        });
    }
    let obscured: Vec<String> = sights
        .iter()
        .filter(|s| s.obscured_s > 0.0)
        .map(|s| {
            format!(
                "{} from {:.1} m engine height",
                s.name,
                s.obscured_from_height_m.unwrap_or(f64::NAN)
            )
        })
        .collect();
    let verdict = format!(
        "MODELLED: {} of {} line(s) of sight above {} dB optical attenuation{}; largest RF attenuation {:.3} dB. A parametric screening result under the stated assumptions, not a prediction validated against plume measurements.",
        obscured.len(),
        sights.len(),
        q.optical_threshold_db,
        if obscured.is_empty() {
            String::new()
        } else {
            format!(" ({})", obscured.join("; "))
        },
        sights.iter().map(|s| s.max_rf_db).fold(0.0, f64::max)
    );
    Ok(Report {
        format: FORMAT,
        method: METHOD_ID,
        title: q.title.clone(),
        evidence: EVIDENCE,
        assumptions: ASSUMPTIONS.to_vec(),
        request: q.clone(),
        particle_mass_kg: m_p,
        rf_size_parameter: x,
        rf_q_ext: rf_q,
        warnings,
        sights,
        verdict,
    })
}

/// HTML page.
pub fn html(rep: &Report) -> String {
    let rows = rep
        .sights
        .iter()
        .map(|s| {
            vec![
                s.name.clone(),
                format!("{:.2}", s.max_optical_db),
                format!("{:.4}", s.max_rf_db),
                s.obscured_from_height_m
                    .map(|h| format!("{h:.1}"))
                    .unwrap_or_else(|| "never".into()),
                format!("{:.1}", s.obscured_s),
                format!("{:.1}", s.rf_flagged_s),
            ]
        })
        .collect();
    let arows = rep
        .assumptions
        .iter()
        .map(|a| vec![(*a).to_string()])
        .collect();
    let mut intro: Vec<String> = rep.warnings.clone();
    intro.insert(
        0,
        "Parametric model; every assumption is listed below.".into(),
    );
    crate::transport::page(
        &rep.title,
        rep.evidence,
        &rep.verdict,
        &intro,
        &[
            (
                "Lines of sight".into(),
                vec![
                    "line of sight",
                    "max optical (dB)",
                    "max RF (dB)",
                    "obscured from height (m)",
                    "obscured (s)",
                    "RF flagged (s)",
                ],
                rows,
            ),
            ("Assumptions".into(), vec!["assumption"], arows),
        ],
    )
}
