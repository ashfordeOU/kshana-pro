// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Holdover evidence with a GUM uncertainty budget (W1120 TA3)
//!
//! Research 05 and 07b: Finland's Regulation 54 requires mobile networks to withstand at least
//! two weeks of GNSS disruption (confirmed by Traficom, 21 Apr 2026); SVN-23 put about 40
//! network elements into holdover. Given the oscillator's characterisation (a clock class,
//! datasheet Allan deviations or TDEV points), its residual frequency offset at the loss of
//! GNSS, ageing and temperature sensitivity, and the time error at the loss (or an ITU-T
//! equipment class for it), this computes the time error's growth in holdover
//!
//! `TE(t) = x0 + y0·t + D·t²/2`, with standard uncertainties
//! `u_x0`, `t·u_y0`, `t²/2·u_D`, `t·(c_T·ΔT)/√3` (temperature, rectangular) and the
//! oscillator noise `sqrt(q_wf·t + q_rw·t³/3 + R)` (the open engine's coast variance),
//!
//! combined by the GUM (JCGM 100:2008) law of propagation for uncorrelated inputs, expanded
//! with `k` (default 2), and compares the bound `|TE(t)| + U(t)` with the limit for the stated
//! period. The largest holdover for which the bound stays within the limit is reported.
//!
//! TDEV points are turned into Allan variances under a white-FM assumption
//! (`σ_y²(τ) = 6·TDEV²(τ)/τ²`, since `TVAR = τ²·MVAR/3` and `MVAR = AVAR/2` for white FM);
//! the report says so. ITU-T equipment-class figures used for the time error at the loss are
//! as transcribed here and to confirm with the Recommendations (G.8272, G.8272.1, G.8273.2).
//!
//! **Label: MODELLED.** A prediction from the stated characterisation, not a measurement of a
//! holdover; a signed report says which inputs were stated and which defaulted.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::model::{class_noise, fit_noise, norm_cdf, Noise, CLASS_NAMES};

pub const FEATURE: &str = "pro-timing-holdover-evidence";
pub const FORMAT: &str = "kshana-pro-holdover-evidence/1";
pub const LABEL: &str = "MODELLED: holdover time error predicted from the stated oscillator characterisation with a GUM uncertainty budget (JCGM 100:2008); not a measured holdover. Whether a requirement is met is for the operator, its auditor and its authority to judge.";

/// Equipment classes for the time error at the loss: name, bound (s), source.
pub const EQUIPMENT_CLASSES: [(&str, f64, &str); 6] = [
    (
        "prtc-a",
        100e-9,
        "ITU-T G.8272 PRTC-A max|TE| 100 ns (to confirm with the Recommendation)",
    ),
    (
        "prtc-b",
        40e-9,
        "ITU-T G.8272 PRTC-B max|TE| 40 ns (to confirm with the Recommendation)",
    ),
    (
        "eprtc",
        30e-9,
        "ITU-T G.8272.1 ePRTC 30 ns (to confirm with the Recommendation)",
    ),
    (
        "t-bc-class-a",
        50e-9,
        "ITU-T G.8273.2 T-BC class A cTE 50 ns (to confirm with the Recommendation)",
    ),
    (
        "t-bc-class-b",
        20e-9,
        "ITU-T G.8273.2 T-BC class B cTE 20 ns (to confirm with the Recommendation)",
    ),
    (
        "t-bc-class-c",
        10e-9,
        "ITU-T G.8273.2 T-BC class C cTE 10 ns (to confirm with the Recommendation)",
    ),
];

/// Presets: name, period (h), limit (s), basis.
pub const PRESETS: [(&str, f64, f64, &str); 2] = [
    (
        "finland-reg54",
        336.0,
        1.5e-6,
        "Traficom Regulation 54: mobile networks withstand at least two weeks of GNSS disruption (research 07b [T12]); the ±1.5 µs limit is the ITU-T G.8271 end-application budget for TDD radio, the operator's choice of limit, not a figure of the Regulation",
    ),
    (
        "g8271-1-day",
        24.0,
        1.5e-6,
        "illustrative: one day of holdover against the ±1.5 µs ITU-T G.8271 end-application budget",
    ),
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OscillatorSpec {
    /// A clock class of the open engine (`tcxo`, `ocxo`, `rafs`, `csac`, `uso`, `dsac`).
    #[serde(default)]
    pub class: Option<String>,
    /// Datasheet Allan deviations `[[tau_s, adev], ...]` (wins over the class).
    #[serde(default)]
    pub adev: Option<Vec<[f64; 2]>>,
    /// TDEV points `[[tau_s, tdev_s], ...]` (white-FM conversion; wins over the class).
    #[serde(default)]
    pub tdev: Option<Vec<[f64; 2]>>,
    /// Residual fractional frequency offset at the loss, and its standard uncertainty.
    #[serde(default)]
    pub initial_freq: f64,
    #[serde(default)]
    pub u_initial_freq: Option<f64>,
    /// Linear ageing (fractional frequency per day), and its standard uncertainty.
    #[serde(default)]
    pub ageing_per_day: f64,
    #[serde(default)]
    pub u_ageing_per_day: Option<f64>,
    /// Temperature coefficient (fractional frequency per kelvin) and the ± half-range of the
    /// site's temperature (K), taken as a rectangular distribution.
    #[serde(default)]
    pub temp_coeff_per_k: f64,
    #[serde(default)]
    pub temp_range_k: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldoverSpec {
    pub site: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub period_h: Option<f64>,
    #[serde(default)]
    pub limit_s: Option<f64>,
    #[serde(default)]
    pub coverage_k: Option<f64>,
    /// An equipment class giving the time error at the loss as a ± bound (rectangular).
    #[serde(default)]
    pub equipment_class: Option<String>,
    /// Mean time error at the loss and its standard uncertainty, s.
    #[serde(default)]
    pub initial_te_s: f64,
    #[serde(default)]
    pub u_initial_te_s: f64,
    pub oscillator: OscillatorSpec,
}

impl HoldoverSpec {
    pub fn parse(src: &str) -> Result<HoldoverSpec, String> {
        toml::from_str(src).map_err(|e| format!("holdover evidence request: {e}"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Component {
    pub name: String,
    /// Standard uncertainty at the period, s.
    pub u_s: f64,
    /// Share of the combined variance.
    pub share: f64,
    pub kind: &'static str,
    pub basis: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CurvePoint {
    pub t_s: f64,
    pub mean_s: f64,
    pub u_c_s: f64,
    pub bound_s: f64,
    pub within: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HoldoverReport {
    pub format: &'static str,
    pub label: &'static str,
    pub site: String,
    pub title: Option<String>,
    pub utc: String,
    pub period_s: f64,
    pub limit_s: f64,
    pub limit_basis: String,
    pub coverage_k: f64,
    pub noise: Noise,
    pub noise_basis: String,
    pub mean_at_period_s: f64,
    pub u_c_at_period_s: f64,
    pub expanded_at_period_s: f64,
    pub bound_at_period_s: f64,
    pub probability_beyond_limit_at_period: f64,
    /// Largest holdover with the bound within the limit (`None`: beyond 10 x the period).
    pub max_supported_holdover_s: Option<f64>,
    pub supported: bool,
    pub verdict: String,
    pub budget: Vec<Component>,
    pub curve: Vec<CurvePoint>,
    pub defaults: Vec<String>,
}

struct Model {
    x0: f64,
    y0: f64,
    d: f64,
    u_x0: f64,
    u_class: f64,
    u_y0: f64,
    u_d: f64,
    temp: f64,
    noise: Noise,
}

impl Model {
    fn mean(&self, t: f64) -> f64 {
        self.x0 + self.y0 * t + self.d * t * t / 2.0
    }
    fn comps(&self, t: f64) -> [f64; 6] {
        [
            self.u_x0,
            self.u_class,
            t * self.u_y0,
            t * t / 2.0 * self.u_d,
            t * self.temp / 3f64.sqrt(),
            (kshana::holdover::coast_phase_variance(self.noise.q_wf, self.noise.q_rw, 0.0, t)
                + self.noise.r_s2)
                .max(0.0)
                .sqrt(),
        ]
    }
    fn uc(&self, t: f64) -> f64 {
        self.comps(t).iter().map(|c| c * c).sum::<f64>().sqrt()
    }
}

/// Compute the holdover evidence.
pub fn assess(spec: &HoldoverSpec, utc: &str) -> Result<HoldoverReport, String> {
    let preset = match &spec.preset {
        Some(p) => Some(PRESETS.iter().find(|x| x.0 == p).ok_or_else(|| {
            format!(
                "unknown preset '{p}' (expected {})",
                PRESETS.map(|x| x.0).join(", ")
            )
        })?),
        None => None,
    };
    let period_h = spec
        .period_h
        .or(preset.map(|p| p.1))
        .ok_or("period_h (or a preset) is needed")?;
    let limit = spec
        .limit_s
        .or(preset.map(|p| p.2))
        .ok_or("limit_s (or a preset) is needed")?;
    if period_h.is_nan() || period_h <= 0.0 || limit.is_nan() || limit <= 0.0 {
        return Err("period_h and limit_s must be > 0".into());
    }
    let limit_basis = match (spec.limit_s, preset) {
        (None, Some(p)) => p.3.to_string(),
        (Some(_), Some(p)) => format!("the operator's limit; preset context: {}", p.3),
        _ => "the operator's limit".to_string(),
    };
    let k = spec.coverage_k.unwrap_or(2.0);
    if k.is_nan() || k <= 0.0 {
        return Err("coverage_k must be > 0".into());
    }
    let o = &spec.oscillator;
    let mut defaults = Vec::new();
    let (noise, noise_basis) = if let Some(a) = &o.adev {
        let pts: Vec<(f64, f64)> = a.iter().map(|p| (p[0], p[1] * p[1])).collect();
        (
            fit_noise(&pts),
            format!("fitted to {} stated Allan deviation points", a.len()),
        )
    } else if let Some(t) = &o.tdev {
        let pts: Vec<(f64, f64)> = t
            .iter()
            .map(|p| (p[0], 6.0 * p[1] * p[1] / (p[0] * p[0])))
            .collect();
        (
            fit_noise(&pts),
            format!("fitted to {} stated TDEV points converted to Allan variance under a white-FM assumption (6·TDEV²/τ²)", t.len()),
        )
    } else if let Some(c) = &o.class {
        (
            class_noise(c).ok_or_else(|| {
                format!(
                    "unknown oscillator class '{c}' (expected {})",
                    CLASS_NAMES.join(", ")
                )
            })?,
            format!("the open engine's representative noise of class '{c}'"),
        )
    } else {
        return Err("the oscillator needs a class, adev points or tdev points".into());
    };
    let (u_class, class_basis) = match &spec.equipment_class {
        Some(c) => {
            let e = EQUIPMENT_CLASSES.iter().find(|x| x.0 == c).ok_or_else(|| {
                format!(
                    "unknown equipment class '{c}' (expected {})",
                    EQUIPMENT_CLASSES.map(|x| x.0).join(", ")
                )
            })?;
            (e.1 / 3f64.sqrt(), e.2.to_string())
        }
        None => (0.0, String::new()),
    };
    let u_y0 = o.u_initial_freq.unwrap_or_else(|| {
        defaults.push("u_initial_freq not stated: taken as 0; the disciplining loop's residual frequency error at the loss is often the largest term".into());
        0.0
    });
    let u_d = o.u_ageing_per_day.unwrap_or_else(|| {
        let v = o.ageing_per_day.abs() * 0.5;
        defaults.push(format!(
            "u_ageing_per_day not stated: taken as half the stated ageing ({v:e}/day)"
        ));
        v
    });
    if spec.u_initial_te_s == 0.0 && spec.equipment_class.is_none() {
        defaults.push("no time error at the loss stated (u_initial_te_s and equipment_class absent): taken as exact".into());
    }
    let m = Model {
        x0: spec.initial_te_s,
        y0: o.initial_freq,
        d: o.ageing_per_day / 86_400.0,
        u_x0: spec.u_initial_te_s,
        u_class,
        u_y0,
        u_d: u_d / 86_400.0,
        temp: o.temp_coeff_per_k * o.temp_range_k,
        noise,
    };
    let period = period_h * 3600.0;
    let bound = |t: f64| m.mean(t).abs() + k * m.uc(t);
    // Largest supported holdover: scan a log grid to 10 x the period, then bisect.
    let max_t = 10.0 * period;
    let mut prev = 0.0;
    let mut max_supported = None;
    if bound(0.0) > limit {
        max_supported = Some(0.0);
    } else {
        let n = 400;
        for i in 0..=n {
            let t = 1.0 * (max_t / 1.0f64).powf(i as f64 / n as f64);
            if bound(t) > limit {
                let (mut lo, mut hi) = (prev, t);
                for _ in 0..60 {
                    let mid = (lo + hi) / 2.0;
                    if bound(mid) > limit {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                max_supported = Some(lo);
                break;
            }
            prev = t;
        }
    }
    let mean_p = m.mean(period);
    let uc_p = m.uc(period);
    let bound_p = bound(period);
    let supported = bound_p <= limit;
    let p_beyond = if uc_p > 0.0 {
        norm_cdf((-limit - mean_p) / uc_p) + 1.0 - norm_cdf((limit - mean_p) / uc_p)
    } else if mean_p.abs() > limit {
        1.0
    } else {
        0.0
    };
    let c = m.comps(period);
    let var: f64 = c.iter().map(|x| x * x).sum::<f64>().max(f64::MIN_POSITIVE);
    let names = [
        (
            "time error at the loss (stated)",
            "B",
            format!("stated u = {}", h(spec.u_initial_te_s)),
        ),
        (
            "time error at the loss (equipment class)",
            "B",
            if class_basis.is_empty() {
                "no class stated".into()
            } else {
                format!("{class_basis}; rectangular, a/√3")
            },
        ),
        (
            "residual frequency offset at the loss",
            "B",
            format!("t · u(y0), u(y0) = {u_y0:e}"),
        ),
        ("ageing", "B", format!("t²/2 · u(D), u(D) = {u_d:e}/day")),
        (
            "temperature",
            "B",
            format!(
                "t · c_T·ΔT/√3 with c_T = {:e}/K, ΔT = ±{} K (rectangular)",
                o.temp_coeff_per_k, o.temp_range_k
            ),
        ),
        (
            "oscillator noise",
            "A/B",
            format!("sqrt(q_wf·t + q_rw·t³/3 + R), {noise_basis}"),
        ),
    ];
    let budget = names
        .iter()
        .zip(c)
        .map(|((n, kind, b), u)| Component {
            name: n.to_string(),
            u_s: u,
            share: u * u / var,
            kind,
            basis: b.clone(),
        })
        .collect();
    let mut ts: Vec<f64> = [
        600.0,
        3600.0,
        4.0 * 3600.0,
        86_400.0,
        3.0 * 86_400.0,
        7.0 * 86_400.0,
        14.0 * 86_400.0,
        30.0 * 86_400.0,
    ]
    .into_iter()
    .filter(|t| *t < period)
    .collect();
    ts.push(period);
    let curve = ts
        .iter()
        .map(|&t| CurvePoint {
            t_s: t,
            mean_s: m.mean(t),
            u_c_s: m.uc(t),
            bound_s: bound(t),
            within: bound(t) <= limit,
        })
        .collect();
    let dur = |s: f64| super::fleet::human_duration(Some(s));
    let verdict = if supported {
        format!(
            "supported by the model: |TE| + U stays within {} for the {} period (bound {} at the end; largest supported holdover {})",
            h(limit),
            dur(period),
            h(bound_p),
            max_supported.map(dur).unwrap_or_else(|| format!("beyond {}", dur(max_t)))
        )
    } else {
        format!(
            "not supported by the model: |TE| + U reaches {} after {}, before the end of the {} period (bound {} at the end)",
            h(limit),
            max_supported.map(dur).unwrap_or_else(|| "0 s".into()),
            dur(period),
            h(bound_p)
        )
    };
    Ok(HoldoverReport {
        format: FORMAT,
        label: LABEL,
        site: spec.site.clone(),
        title: spec.title.clone(),
        utc: utc.into(),
        period_s: period,
        limit_s: limit,
        limit_basis,
        coverage_k: k,
        noise,
        noise_basis,
        mean_at_period_s: mean_p,
        u_c_at_period_s: uc_p,
        expanded_at_period_s: k * uc_p,
        bound_at_period_s: bound_p,
        probability_beyond_limit_at_period: p_beyond,
        max_supported_holdover_s: max_supported,
        supported,
        verdict,
        budget,
        curve,
        defaults,
    })
}

fn h(x: f64) -> String {
    super::fleet::human_time(x)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn html(r: &HoldoverReport) -> String {
    let mut o = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Holdover evidence: {}</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#111;background:#fff}}\
table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccc;padding:.3rem;text-align:left}}small{{color:#555}}</style></head><body>\
<h1>Holdover evidence: {}</h1><p>{}</p><p><strong>{}</strong></p><p>Period {} · limit {} ({}) · k = {} · {}</p>",
        esc(&r.site),
        esc(&r.site),
        esc(r.label),
        esc(&r.verdict),
        super::fleet::human_duration(Some(r.period_s)),
        h(r.limit_s),
        esc(&r.limit_basis),
        r.coverage_k,
        esc(&r.utc)
    );
    o.push_str("<h2>Uncertainty budget at the end of the period</h2><table><tr><th>Component</th><th>Type</th><th>u</th><th>Share</th><th>Basis</th></tr>");
    for c in &r.budget {
        o.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.1} %</td><td><small>{}</small></td></tr>",
            esc(&c.name),
            c.kind,
            h(c.u_s),
            100.0 * c.share,
            esc(&c.basis)
        ));
    }
    o.push_str(&format!(
        "</table><p>Mean TE {} · u<sub>c</sub> {} · U = k·u<sub>c</sub> {} · bound {} · P(|TE| &gt; limit) {:.3e}</p>",
        h(r.mean_at_period_s),
        h(r.u_c_at_period_s),
        h(r.expanded_at_period_s),
        h(r.bound_at_period_s),
        r.probability_beyond_limit_at_period
    ));
    o.push_str("<h2>Growth</h2><table><tr><th>Holdover</th><th>Mean TE</th><th>u<sub>c</sub></th><th>|TE| + U</th><th>Within</th></tr>");
    for p in &r.curve {
        o.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            super::fleet::human_duration(Some(p.t_s)),
            h(p.mean_s),
            h(p.u_c_s),
            h(p.bound_s),
            if p.within { "yes" } else { "no" }
        ));
    }
    o.push_str("</table>");
    if !r.defaults.is_empty() {
        o.push_str("<h2>Defaults applied</h2><ul>");
        for d in &r.defaults {
            o.push_str(&format!("<li>{}</li>", esc(d)));
        }
        o.push_str("</ul>");
    }
    o.push_str("</body></html>\n");
    o
}

/// Write `report.json`, `report.html`, `budget.csv`, `curve.csv` and `inputs/holdover.toml`.
pub fn write_dir(out: &Path, r: &HoldoverReport, src: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out.join("inputs")).map_err(io)?;
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(r).expect("serialises"),
    )
    .map_err(io)?;
    std::fs::write(out.join("report.html"), html(r)).map_err(io)?;
    let mut b = String::from("component,type,u_s,share,basis\n");
    for c in &r.budget {
        b.push_str(&format!(
            "\"{}\",{},{:e},{:.6},\"{}\"\n",
            c.name,
            c.kind,
            c.u_s,
            c.share,
            c.basis.replace('"', "'")
        ));
    }
    std::fs::write(out.join("budget.csv"), b).map_err(io)?;
    let mut c = String::from("t_s,mean_s,u_c_s,bound_s,within\n");
    for p in &r.curve {
        c.push_str(&format!(
            "{},{:e},{:e},{:e},{}\n",
            p.t_s, p.mean_s, p.u_c_s, p.bound_s, p.within
        ));
    }
    std::fs::write(out.join("curve.csv"), c).map_err(io)?;
    std::fs::write(out.join("inputs").join("holdover.toml"), src).map_err(io)?;
    Ok(())
}
