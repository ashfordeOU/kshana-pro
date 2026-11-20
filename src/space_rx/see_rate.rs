// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! B2003: a single-event-effect rate forecast for a receiver (MODELLED).
//!
//! Each effect (latch-up, SEFI, reset, upset) has a Weibull cross-section over LET from the
//! receiver's heavy-ion test, `σ(L) = σsat (1 − exp(−((L − L0)/W)^s))` for `L > L0`, and
//! optionally a Weibull over proton energy. Each environment gives an integral LET spectrum
//! (particles per cm² per day with LET above L, behind the stated shielding, for example
//! from a CREME96 or OMERE run) and optionally an integral proton spectrum. The rate is the
//! cross-section folded with the differential spectrum, bin by bin:
//! `R = Σ σ(L_mid) · (F(L_i) − F(L_{i+1}))`.
//!
//! **Simplifications, stated in every result:** the fold uses the omnidirectional integral
//! flux and the cross-section at normal incidence, with no chord-length (RPP/IRPP) treatment;
//! the cross-section is per device as tested. It is a planning figure for a review and for
//! test planning, not a predicted on-orbit rate.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{round, LABEL};

pub const FORMAT: &str = "kshana-space-rx-see-rate/1";
pub const SCHEMA: &str = "kshana-space-rx-see-rate-result/1";

/// A Weibull cross-section.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weibull {
    pub sigma_sat_cm2: f64,
    /// Onset (LET in MeV cm2/mg, or proton energy in MeV).
    pub onset: f64,
    pub width: f64,
    pub shape: f64,
}

impl Weibull {
    pub fn sigma(&self, x: f64) -> f64 {
        if x <= self.onset {
            return 0.0;
        }
        self.sigma_sat_cm2 * (1.0 - (-((x - self.onset) / self.width).powf(self.shape)).exp())
    }
    fn check(&self, what: &str) -> Result<(), String> {
        if !(self.sigma_sat_cm2 > 0.0 && self.width > 0.0 && self.shape > 0.0 && self.onset >= 0.0)
        {
            return Err(format!(
                "{what}: Weibull needs sigma_sat_cm2, width, shape > 0 and onset >= 0"
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub name: String,
    pub heavy_ion: Weibull,
    #[serde(default)]
    pub proton: Option<Weibull>,
    #[serde(default)]
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub name: String,
    pub source: String,
    /// `[[LET, integral flux per cm2 per day above LET], ...]`, LET increasing.
    pub let_spectrum: Vec<[f64; 2]>,
    /// `[[E MeV, integral flux per cm2 per day above E], ...]`, energy increasing.
    #[serde(default)]
    pub proton_spectrum: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub format: String,
    pub name: String,
    #[serde(rename = "effect")]
    pub effects: Vec<Effect>,
    #[serde(rename = "environment")]
    pub environments: Vec<Environment>,
}

fn check_spectrum(what: &str, s: &[[f64; 2]]) -> Result<(), String> {
    if s.len() < 2 {
        return Err(format!("{what}: needs at least two points"));
    }
    if s.windows(2)
        .any(|w| w[1][0] <= w[0][0] || w[1][1] > w[0][1])
    {
        return Err(format!(
            "{what}: the abscissa must increase and the integral flux must not increase"
        ));
    }
    if s.iter().any(|p| p[1] < 0.0 || !p[0].is_finite()) {
        return Err(format!("{what}: negative or non-finite values"));
    }
    Ok(())
}

impl Model {
    pub fn parse(src: &str) -> Result<Self, String> {
        let m: Model = toml::from_str(src).map_err(|e| format!("rate model: {e}"))?;
        if m.format != FORMAT {
            return Err(format!("rate model: format must be \"{FORMAT}\""));
        }
        if m.effects.is_empty() || m.environments.is_empty() {
            return Err("rate model: needs at least one effect and one environment".into());
        }
        for e in &m.effects {
            e.heavy_ion.check(&e.name)?;
            if let Some(p) = &e.proton {
                p.check(&e.name)?;
            }
        }
        for v in &m.environments {
            check_spectrum(&format!("{} let_spectrum", v.name), &v.let_spectrum)?;
            if !v.proton_spectrum.is_empty() {
                check_spectrum(&format!("{} proton_spectrum", v.name), &v.proton_spectrum)?;
            }
        }
        Ok(m)
    }
}

/// Fold a cross-section with an integral spectrum: events per day. Bins are split in
/// log-spaced sub-bins (log-linear interpolation of the integral flux) so a coarse table
/// does not hide the Weibull onset.
pub fn fold(w: &Weibull, spectrum: &[[f64; 2]]) -> f64 {
    let mut r = 0.0;
    for b in spectrum.windows(2) {
        let (x0, f0, x1, f1) = (b[0][0], b[0][1], b[1][0], b[1][1]);
        let n = 20;
        for k in 0..n {
            let (a, c) = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
            let xa = x0 + (x1 - x0) * a;
            let xc = x0 + (x1 - x0) * c;
            let interp = |t: f64| {
                if f0 > 0.0 && f1 > 0.0 {
                    (f0.ln() + (f1.ln() - f0.ln()) * t).exp()
                } else {
                    f0 + (f1 - f0) * t
                }
            };
            r += w.sigma((xa + xc) / 2.0) * (interp(a) - interp(c));
        }
    }
    r
}

/// Run the forecast.
pub fn run(m: &Model) -> Value {
    let mut rows = Vec::new();
    for v in &m.environments {
        for e in &m.effects {
            let hi = fold(&e.heavy_ion, &v.let_spectrum);
            let pr = match (&e.proton, v.proton_spectrum.is_empty()) {
                (Some(p), false) => Some(fold(p, &v.proton_spectrum)),
                _ => None,
            };
            let tot = hi + pr.unwrap_or(0.0);
            rows.push(json!({
                "environment": v.name,
                "effect": e.name,
                "heavy_ion_per_day": hi,
                "proton_per_day": pr,
                "per_day": tot,
                "per_year": tot * 365.25,
                "mean_days_between": if tot > 0.0 { json!(round(1.0 / tot, 1)) } else { Value::Null },
                "proton_term": if e.proton.is_none() { "no proton cross-section" } else if v.proton_spectrum.is_empty() { "no proton spectrum" } else { "included" },
            }));
        }
    }
    json!({
        "schema": SCHEMA,
        "label": LABEL,
        "evidence": "MODELLED",
        "model": m.name,
        "simplifications": [
            "omnidirectional integral flux folded with the normal-incidence cross-section; no chord-length (RPP/IRPP) treatment",
            "cross-sections per device as tested; spectra as supplied, behind the shielding stated by their source",
            "a planning figure for review and test planning, not a predicted on-orbit rate",
        ],
        "environments": m.environments.iter().map(|v| json!({"name": v.name, "source": v.source})).collect::<Vec<_>>(),
        "effects": m.effects.iter().map(|e| json!({"name": e.name, "source": e.source, "heavy_ion": e.heavy_ion, "proton": e.proton})).collect::<Vec<_>>(),
        "rates": rows,
    })
}

/// A Markdown table of a forecast.
pub fn markdown(doc: &Value) -> String {
    let mut m = format!(
        "# SEE rate forecast: {}\n\n{}\n\n| Environment | Effect | per day | per year | mean days between |\n|---|---|---|---|---|\n",
        doc["model"].as_str().unwrap_or(""),
        LABEL
    );
    for r in doc["rates"].as_array().into_iter().flatten() {
        m.push_str(&format!(
            "| {} | {} | {:.3e} | {:.3} | {} |\n",
            r["environment"].as_str().unwrap_or(""),
            r["effect"].as_str().unwrap_or(""),
            r["per_day"].as_f64().unwrap_or(0.0),
            r["per_year"].as_f64().unwrap_or(0.0),
            r["mean_days_between"]
        ));
    }
    m
}

/// A SYNTHETIC sample model (illustrative numbers; not from any receiver or environment run).
pub const SAMPLE: &str = r#"format = "kshana-space-rx-see-rate/1"
name = "SYNTHETIC sample: example COTS receiver, LEO 550 km"

[[effect]]
name = "latchup"
source = "SYNTHETIC Weibull"
heavy_ion = { sigma_sat_cm2 = 2e-5, onset = 40.0, width = 30.0, shape = 2.0 }

[[effect]]
name = "sefi"
source = "SYNTHETIC Weibull"
heavy_ion = { sigma_sat_cm2 = 1e-4, onset = 3.0, width = 25.0, shape = 1.5 }
proton = { sigma_sat_cm2 = 1e-10, onset = 20.0, width = 60.0, shape = 1.2 }

[[effect]]
name = "upset"
source = "SYNTHETIC Weibull"
heavy_ion = { sigma_sat_cm2 = 5e-4, onset = 1.0, width = 20.0, shape = 1.2 }
proton = { sigma_sat_cm2 = 2e-9, onset = 10.0, width = 50.0, shape = 1.0 }

[[environment]]
name = "solar minimum"
source = "SYNTHETIC spectra for the sample"
let_spectrum = [[0.1, 5.0], [1.0, 2e-2], [10.0, 1e-3], [30.0, 5e-5], [60.0, 2e-6], [100.0, 1e-8]]
proton_spectrum = [[10.0, 8e5], [30.0, 3e5], [50.0, 1.5e5], [100.0, 4e4], [200.0, 6e3]]

[[environment]]
name = "solar maximum"
source = "SYNTHETIC spectra for the sample"
let_spectrum = [[0.1, 3.0], [1.0, 1.2e-2], [10.0, 6e-4], [30.0, 3e-5], [60.0, 1.2e-6], [100.0, 6e-9]]
proton_spectrum = [[10.0, 6e5], [30.0, 2.2e5], [50.0, 1.1e5], [100.0, 3e4], [200.0, 4.5e3]]
"#;
