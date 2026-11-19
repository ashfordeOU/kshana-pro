// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D2. "Circular T for the Moon": TL(k) − UTC with uncertainty, published monthly
//!
//! The CCTF asks who will compute an "International Lunar Time", and how (research note §2.2,
//! gap 2). This is a reference processing chain for that computation, in the shape of BIPM
//! Circular T:
//!
//! 1. **Input**: relayed time-transfer comparisons between lunar clocks and Earth
//!    laboratories, each the link-calibrated reading difference `TL(k) − UTC(k_E)` (s) at a UTC
//!    epoch, with its statistical uncertainty and its link-calibration uncertainty; and the
//!    Earth laboratories' `[UTC − UTC(k_E)]` from BIPM Circular T Section 1 (read with the open
//!    engine's parser, [`kshana::realdata::clk::parse_circular_t_section1`]) or a CSV.
//! 2. **Reduction**: for each comparison, the deviation of the realisation from the ideal scale
//!    computed from UTC with the D1 model,
//!    `[TL − TL(k)] = (TL − UTC)_model − (TL(k) − UTC(k_E)) + [UTC − UTC(k_E)]`.
//! 3. **Estimation**: at each Circular T date (0h UTC on the MJDs ending in 4 and 9 in the
//!    month), a weighted least-squares line through the comparisons within ± `window_days`,
//!    evaluated at the date. `uA` is the standard error of the intercept (scaled by the
//!    reduced χ² when that exceeds one); `uB` combines the link calibration, the Earth
//!    laboratory's Circular T uncertainty and the D1 model uncertainty of `(TL − UTC)` at the
//!    date; `u = √(uA² + uB²)`.
//! 4. **Publication**: a text table in the layout of Circular T Section 1 (so the open engine's
//!    Circular T parser reads it back), a Section 2 with the full `TL(k) − UTC` in seconds, a
//!    Section 3 stating the model, ephemerides and uncertainties as CGPM 2026 Resolution 6 asks,
//!    and the same content as JSON. The scale may be TL or TCL ([`LunarOption`]).
//!
//! This is a **prototype, not a BIPM publication**, and every table says so. Evidence label:
//! **MODELLED**, checked on synthetic comparisons with known truth; no lunar clock data exists.

use std::collections::BTreeMap;

use kshana::timescales::{TwoPartJd, MJD_OFFSET};
use serde::Serialize;

use super::time::{LunarOption, Model};

/// One relayed comparison of a lunar clock against an Earth laboratory's UTC(k).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Comparison {
    /// UTC epoch as a (fractional) MJD.
    pub mjd_utc: f64,
    /// The lunar realisation, e.g. `LTL1`.
    pub lunar_clock: String,
    /// The Earth laboratory whose UTC(k) was the reference, e.g. `PTB`.
    pub earth_lab: String,
    /// The link-calibrated reading difference `TL(k) − UTC(k_E)` (s).
    pub reading_s: f64,
    /// Statistical standard uncertainty of this comparison (ns).
    pub u_a_ns: f64,
    /// Link-calibration standard uncertainty (ns), systematic.
    pub u_b_ns: f64,
}

/// Parse the comparisons CSV: `mjd_utc,lunar_clock,earth_lab,reading_s,u_a_ns,u_b_ns`
/// (a header line and `#` comments allowed).
pub fn parse_comparisons(text: &str) -> Result<Vec<Comparison>, String> {
    let mut out = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("mjd_utc") {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() != 6 {
            return Err(format!(
                "line {}: expected 6 fields, found {}",
                i + 1,
                f.len()
            ));
        }
        let num = |s: &str| -> Result<f64, String> {
            s.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("line {}: bad number '{s}'", i + 1))
        };
        let c = Comparison {
            mjd_utc: num(f[0])?,
            lunar_clock: f[1].to_string(),
            earth_lab: f[2].to_string(),
            reading_s: num(f[3])?,
            u_a_ns: num(f[4])?,
            u_b_ns: num(f[5])?,
        };
        if c.u_a_ns <= 0.0 || c.u_b_ns < 0.0 {
            return Err(format!("line {}: uncertainties must be positive", i + 1));
        }
        if c.lunar_clock.is_empty() || c.lunar_clock.contains(char::is_whitespace) {
            return Err(format!("line {}: bad lunar clock code", i + 1));
        }
        out.push(c);
    }
    if out.is_empty() {
        return Err("no comparisons".into());
    }
    Ok(out)
}

/// The Earth laboratories' `[UTC − UTC(k)]` series (ns) with their combined uncertainty (ns).
#[derive(Clone, Debug, Default, Serialize)]
pub struct EarthSeries {
    /// lab → [(MJD, value ns, u ns)], sorted by MJD.
    pub labs: BTreeMap<String, Vec<(f64, f64, f64)>>,
    pub source: String,
}

impl EarthSeries {
    /// From one or more Circular T texts (Section 1), through the open engine's parser.
    pub fn from_circular_t(texts: &[String]) -> Result<Self, String> {
        let mut s = EarthSeries {
            source: "BIPM Circular T Section 1 (open engine parser)".into(),
            ..Default::default()
        };
        for t in texts {
            let sec = kshana::realdata::clk::parse_circular_t_section1(t)?;
            for row in &sec.rows {
                let e = s.labs.entry(row.code.clone()).or_default();
                for (m, v) in sec.mjds.iter().zip(&row.values) {
                    if let Some(v) = v {
                        e.push((*m as f64, *v, row.u.unwrap_or(f64::NAN)));
                    }
                }
            }
        }
        s.finish()
    }
    /// From a CSV `mjd,lab,utc_minus_utck_ns,u_ns`.
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let mut s = EarthSeries {
            source: "CSV mjd,lab,utc_minus_utck_ns,u_ns".into(),
            ..Default::default()
        };
        for (i, l) in text.lines().enumerate() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') || l.starts_with("mjd") {
                continue;
            }
            let f: Vec<&str> = l.split(',').map(str::trim).collect();
            let bad = || format!("line {}: expected mjd,lab,value_ns,u_ns", i + 1);
            if f.len() != 4 {
                return Err(bad());
            }
            let m: f64 = f[0].parse().map_err(|_| bad())?;
            let v: f64 = f[2].parse().map_err(|_| bad())?;
            let u: f64 = f[3].parse().map_err(|_| bad())?;
            s.labs.entry(f[1].to_string()).or_default().push((m, v, u));
        }
        s.finish()
    }
    fn finish(mut self) -> Result<Self, String> {
        if self.labs.is_empty() {
            return Err("no Earth laboratory values".into());
        }
        for v in self.labs.values_mut() {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            v.dedup_by(|a, b| a.0 == b.0);
        }
        Ok(self)
    }
    /// `[UTC − UTC(lab)]` (ns) and its uncertainty at `mjd`, linearly interpolated; refused
    /// outside the published dates (no extrapolation).
    pub fn at(&self, lab: &str, mjd: f64) -> Result<(f64, f64), String> {
        let v = self
            .labs
            .get(lab)
            .ok_or_else(|| format!("no Circular T values for laboratory {lab}"))?;
        let i = v.partition_point(|p| p.0 <= mjd);
        if i == 0 || (i == v.len() && v[i - 1].0 < mjd) {
            return Err(format!(
                "MJD {mjd:.3} is outside the published values of {lab} (no extrapolation)"
            ));
        }
        if v[i - 1].0 == mjd {
            return Ok((v[i - 1].1, v[i - 1].2));
        }
        let (a, b) = (v[i - 1], v[i]);
        let w = (mjd - a.0) / (b.0 - a.0);
        Ok((a.1 + w * (b.1 - a.1), a.2.max(b.2)))
    }
}

/// The processing configuration.
#[derive(Clone, Debug, Serialize)]
pub struct Config {
    pub option: LunarOption,
    pub issue: u32,
    pub year: i32,
    pub month: u32,
    /// Half-width of the fitting window around each date (days).
    pub window_days: f64,
}

/// One published value.
#[derive(Clone, Debug, Serialize)]
pub struct Value {
    pub mjd: i64,
    /// `[TL − TL(k)]` (ns), or `None` when fewer than two comparisons fall in the window.
    pub dev_ns: Option<f64>,
    /// `TL(k) − UTC` (s).
    pub realisation_minus_utc_s: Option<f64>,
    pub u_a_ns: Option<f64>,
    pub u_b_ns: Option<f64>,
    pub u_ns: Option<f64>,
    pub n_comparisons: usize,
}

/// One lunar clock's row.
#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub lunar_clock: String,
    pub earth_labs: Vec<String>,
    pub values: Vec<Value>,
    /// Row uncertainties as printed (the largest over the month).
    pub u_a_ns: Option<f64>,
    pub u_b_ns: Option<f64>,
    pub u_ns: Option<f64>,
}

/// The month's publication.
#[derive(Clone, Debug, Serialize)]
pub struct Circular {
    pub config: Config,
    pub scale: &'static str,
    pub mjds: Vec<i64>,
    pub rows: Vec<Row>,
    /// The D1 model as disclosed (CGPM 2026 Res. 6).
    pub model: Model,
    /// The D1 budget of `(scale − UTC)` at the middle date.
    pub model_budget_mid: super::Budget,
    pub earth_source: String,
    pub label: &'static str,
    pub disclaimer: &'static str,
}

/// The disclaimer every table carries.
pub const DISCLAIMER: &str =
    "PROTOTYPE computed by Kshana Pro; NOT a BIPM publication and not an official lunar time scale";

/// The Circular T dates of a month: MJDs ending in 4 or 9 (0h UTC).
pub fn circular_dates(year: i32, month: u32) -> Result<Vec<i64>, String> {
    if !(1..=12).contains(&month) {
        return Err(format!("bad month {month}"));
    }
    let first =
        (kshana::timescales::julian_date(year, month, 1, 0, 0, 0.0) - MJD_OFFSET).round() as i64;
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let next = (kshana::timescales::julian_date(ny, nm, 1, 0, 0, 0.0) - MJD_OFFSET).round() as i64;
    Ok((first..next)
        .filter(|m| m % 10 == 4 || m % 10 == 9)
        .collect())
}

/// `(scale − UTC)` (s) from the model at a UTC MJD.
pub fn model_offset(model: &Model, option: LunarOption, mjd_utc: f64) -> super::Budget {
    model.lunar_minus_utc(option, TwoPartJd::from_f64(mjd_utc + MJD_OFFSET))
}

/// Run the chain.
pub fn compute(
    model: &Model,
    cfg: &Config,
    comparisons: &[Comparison],
    earth: &EarthSeries,
) -> Result<Circular, String> {
    if !(cfg.window_days > 0.0) {
        return Err("window_days must be positive".into());
    }
    let mjds = circular_dates(cfg.year, cfg.month)?;
    // Reduce every comparison once.
    struct Red {
        t: f64,
        dev_ns: f64,
        u_a: f64,
        u_b: f64,
        u_e: f64,
        lab: String,
    }
    let mut by_clock: BTreeMap<String, Vec<Red>> = BTreeMap::new();
    let lo = *mjds.first().unwrap_or(&0) as f64 - cfg.window_days;
    let hi = *mjds.last().unwrap_or(&0) as f64 + cfg.window_days;
    for c in comparisons {
        if c.mjd_utc < lo || c.mjd_utc > hi {
            continue;
        }
        let model_s = model_offset(model, cfg.option, c.mjd_utc).value();
        let (e_ns, u_e) = earth.at(&c.earth_lab, c.mjd_utc)?;
        let dev_ns = (model_s - c.reading_s) * 1e9 + e_ns;
        by_clock
            .entry(c.lunar_clock.clone())
            .or_default()
            .push(Red {
                t: c.mjd_utc,
                dev_ns,
                u_a: c.u_a_ns,
                u_b: c.u_b_ns,
                u_e,
                lab: c.earth_lab.clone(),
            });
    }
    if by_clock.is_empty() {
        return Err("no comparisons fall in the month's windows".into());
    }
    let mut rows = Vec::new();
    for (clock, reds) in &by_clock {
        let mut labs: Vec<String> = reds.iter().map(|r| r.lab.clone()).collect();
        labs.sort();
        labs.dedup();
        let mut values = Vec::new();
        for &m in &mjds {
            let w: Vec<&Red> = reds
                .iter()
                .filter(|r| (r.t - m as f64).abs() <= cfg.window_days)
                .collect();
            if w.len() < 2 {
                values.push(Value {
                    mjd: m,
                    dev_ns: None,
                    realisation_minus_utc_s: None,
                    u_a_ns: None,
                    u_b_ns: None,
                    u_ns: None,
                    n_comparisons: w.len(),
                });
                continue;
            }
            let pts: Vec<(f64, f64, f64)> = w
                .iter()
                .map(|r| (r.t - m as f64, r.dev_ns, r.u_a))
                .collect();
            let Some((a, ua)) = wls_intercept(&pts) else {
                values.push(Value {
                    mjd: m,
                    dev_ns: None,
                    realisation_minus_utc_s: None,
                    u_a_ns: None,
                    u_b_ns: None,
                    u_ns: None,
                    n_comparisons: w.len(),
                });
                continue;
            };
            let n = w.len() as f64;
            let u_link = (w.iter().map(|r| r.u_b * r.u_b).sum::<f64>() / n).sqrt();
            let u_earth = w.iter().map(|r| r.u_e).fold(0.0, f64::max);
            let mb = model_offset(model, cfg.option, m as f64);
            let u_model = mb.u_c() * 1e9;
            let ub = (u_link * u_link + u_earth * u_earth + u_model * u_model).sqrt();
            values.push(Value {
                mjd: m,
                dev_ns: Some(a),
                realisation_minus_utc_s: Some(mb.value() - a * 1e-9),
                u_a_ns: Some(ua),
                u_b_ns: Some(ub),
                u_ns: Some((ua * ua + ub * ub).sqrt()),
                n_comparisons: w.len(),
            });
        }
        let maxo = |f: fn(&Value) -> Option<f64>| {
            values
                .iter()
                .filter_map(f)
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
        };
        rows.push(Row {
            lunar_clock: clock.clone(),
            earth_labs: labs,
            u_a_ns: maxo(|v| v.u_a_ns),
            u_b_ns: maxo(|v| v.u_b_ns),
            u_ns: maxo(|v| v.u_ns),
            values,
        });
    }
    let mid = mjds[mjds.len() / 2] as f64;
    Ok(Circular {
        config: cfg.clone(),
        scale: cfg.option.scale().as_str(),
        mjds,
        rows,
        model: model.clone(),
        model_budget_mid: model_offset(model, cfg.option, mid),
        earth_source: earth.source.clone(),
        label: super::LABEL,
        disclaimer: DISCLAIMER,
    })
}

/// Weighted least-squares line `y = a + b t` with weights `1/u²`; returns `(a, u(a))`, `u(a)`
/// scaled by `√χ²_ν` when the reduced χ² exceeds one.
fn wls_intercept(pts: &[(f64, f64, f64)]) -> Option<(f64, f64)> {
    let (mut s, mut st, mut stt, mut sy, mut sty) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(t, y, u) in pts {
        let w = 1.0 / (u * u);
        s += w;
        st += w * t;
        stt += w * t * t;
        sy += w * y;
        sty += w * t * y;
    }
    let det = s * stt - st * st;
    if !(det > 0.0) || !det.is_finite() {
        return None;
    }
    let a = (stt * sy - st * sty) / det;
    let b = (s * sty - st * sy) / det;
    let var_a = stt / det;
    let mut scale = 1.0;
    if pts.len() > 2 {
        let chi2: f64 = pts
            .iter()
            .map(|&(t, y, u)| ((y - a - b * t) / u).powi(2))
            .sum();
        scale = (chi2 / (pts.len() as f64 - 2.0)).max(1.0);
    }
    Some((a, (var_a * scale).sqrt()))
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

fn opt(v: Option<f64>, prec: usize) -> String {
    v.map_or("-".to_string(), |x| format!("{x:.prec$}"))
}

impl Circular {
    /// The Circular-T-like text. Section 1 follows the layout the open engine's Circular T
    /// parser reads.
    pub fn to_text(&self) -> String {
        let s = self.scale;
        let mut o = String::new();
        o.push_str(&format!(
            "CIRCULAR T {} - LUNAR TIME PROTOTYPE ({s})    {} {}\n",
            self.config.issue,
            self.config.year,
            MONTHS[(self.config.month - 1) as usize]
        ));
        o.push_str(&format!("{DISCLAIMER}\n"));
        o.push_str(&format!("{}\n\n", self.label));
        o.push_str(&format!(
            "1 - Difference between {s}, computed from UTC with the model of Section 3, and its \
             realizations {s}(k).\n    Computed values of [{s}-{s}(k)] at 0h UTC (unit ns).\n\n"
        ));
        let mut date = format!("{:<28}", format!("Date {} 0h UTC", self.config.year));
        let mut mjd = format!("{:<28}", "MJD");
        for &m in &self.mjds {
            let c = kshana::timescales::civil_from_jd(m as f64 + MJD_OFFSET);
            date.push_str(&format!(
                " {:>3}{:>3}",
                MONTHS[(c.month - 1) as usize],
                c.day
            ));
            mjd.push_str(&format!(" {m:>6}"));
        }
        o.push_str(&format!("{date}\n{mjd}\n"));
        o.push_str(&format!(
            "Laboratory k                {}   uA    uB     u\n",
            " ".repeat(7 * self.mjds.len().saturating_sub(1))
        ));
        for r in &self.rows {
            let mut l = format!(
                "{:<28}",
                format!("{} (via {})", r.lunar_clock, r.earth_labs.join("+"))
            );
            for v in &r.values {
                l.push_str(&format!(" {:>6}", opt(v.dev_ns, 2)));
            }
            l.push_str(&format!(
                " {:>5} {:>5} {:>5}",
                opt(r.u_a_ns, 2),
                opt(r.u_b_ns, 2),
                opt(r.u_ns, 2)
            ));
            o.push_str(&l);
            o.push('\n');
        }
        o.push_str(&format!(
            "\n2 - [{s}(k) - UTC] at 0h UTC (unit s; the same values as Section 1 with the model offset \
             restored).\n\n"
        ));
        for r in &self.rows {
            for v in &r.values {
                if let (Some(x), Some(u)) = (v.realisation_minus_utc_s, v.u_ns) {
                    o.push_str(&format!(
                        "{:<10} MJD {}  {:.12} s  u = {:.2} ns  (n = {})\n",
                        r.lunar_clock, v.mjd, x, u, v.n_comparisons
                    ));
                }
            }
        }
        o.push_str(
            "\n3 - Model, ephemerides and uncertainties (CGPM 2026 Resolution 6 disclosure)\n\n",
        );
        o.push_str(&format!(
            "Scale: {s} (CGPM Res. 6 option); TCL rate source: {:?}; W0_L: {} = {:.6} m2/s2 ({}).\n",
            self.model.rate_source, self.model.w0.name, self.model.w0.w0_m2_s2, self.model.w0.source
        ));
        o.push_str(&format!("Earth laboratories: {}.\n", self.earth_source));
        o.push_str(&format!(
            "Fit: weighted straight line through the comparisons within +/- {} d of each date.\n",
            self.config.window_days
        ));
        o.push_str(&format!(
            "Budget of ({s} - UTC) at MJD {} (value {:.12} s, u_c {:.3} ns):\n",
            self.mjds[self.mjds.len() / 2],
            self.model_budget_mid.value(),
            self.model_budget_mid.u_c() * 1e9
        ));
        for t in &self.model_budget_mid.terms {
            o.push_str(&format!(
                "  {:<44} {:>22.12} s  u {:>10.3} ns  [{}]\n",
                t.name,
                t.value,
                t.u.abs() * 1e9,
                t.source
            ));
        }
        o
    }
}

/// A synthetic lunar clock for tests and the example: `[TL − TL(k)] = a + b·(MJD − mjd0)`
/// (ns, ns/day).
#[derive(Clone, Debug, Serialize)]
pub struct SyntheticClock {
    pub code: String,
    pub earth_lab: String,
    pub a_ns: f64,
    pub b_ns_per_day: f64,
}

impl SyntheticClock {
    /// The true deviation (ns) at `mjd`.
    pub fn truth(&self, mjd: f64, mjd0: f64) -> f64 {
        self.a_ns + self.b_ns_per_day * (mjd - mjd0)
    }
}

/// The synthetic data plan.
#[derive(Clone, Debug, Serialize)]
pub struct SyntheticPlan {
    pub year: i32,
    pub month: u32,
    pub clocks: Vec<SyntheticClock>,
    /// Comparison cadence (hours).
    pub step_h: f64,
    /// White link noise per comparison (ns).
    pub sigma_ns: f64,
    /// Stated link calibration uncertainty (ns).
    pub u_b_ns: f64,
    pub seed: u64,
}

impl SyntheticPlan {
    /// The example plan: two lunar clocks via PTB and USNO, November 2026.
    pub fn example() -> Self {
        SyntheticPlan {
            year: 2026,
            month: 11,
            clocks: vec![
                SyntheticClock {
                    code: "LTL1".into(),
                    earth_lab: "PTB".into(),
                    a_ns: 3.0,
                    b_ns_per_day: 0.4,
                },
                SyntheticClock {
                    code: "LTL2".into(),
                    earth_lab: "USNO".into(),
                    a_ns: -12.0,
                    b_ns_per_day: -0.1,
                },
            ],
            step_h: 6.0,
            sigma_ns: 0.5,
            u_b_ns: 1.0,
            seed: 1,
        }
    }
}

/// Synthetic comparisons with known truth (clearly SYNTHETIC: no lunar clock data exists):
/// every `step_h` hours over the month ± 6 days, white link noise, against Earth laboratories
/// whose `[UTC − UTC(k)]` is a stated line (5-day values, `u = 2 ns`). Returns the comparisons,
/// the Earth series and `mjd0` (the first Circular T date of the month).
pub fn synthetic(
    model: &Model,
    option: LunarOption,
    plan: &SyntheticPlan,
) -> Result<(Vec<Comparison>, EarthSeries, f64), String> {
    use rand::SeedableRng;
    let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(plan.seed);
    let dates = circular_dates(plan.year, plan.month)?;
    let mjd0 = dates[0] as f64;
    let (lo, hi) = (mjd0 - 6.0, *dates.last().unwrap_or(&0) as f64 + 6.0);
    let mut earth = EarthSeries {
        source: "SYNTHETIC Earth laboratory series (stated lines)".into(),
        ..Default::default()
    };
    let earth_line = |lab: &str, m: f64| -> f64 {
        let k = lab.bytes().map(|b| b as f64).sum::<f64>() % 7.0;
        (k - 3.0) * 1.5 + 0.05 * (m - mjd0)
    };
    for c in &plan.clocks {
        let e = earth.labs.entry(c.earth_lab.clone()).or_default();
        if e.is_empty() {
            let mut m = (lo / 5.0).floor() * 5.0 - 1.0;
            while m <= hi + 5.0 {
                e.push((m, earth_line(&c.earth_lab, m), 2.0));
                m += 5.0;
            }
        }
    }
    let mut out = Vec::new();
    for c in &plan.clocks {
        let mut k = 0usize;
        loop {
            let t = lo + k as f64 * plan.step_h / 24.0;
            if t > hi {
                break;
            }
            let model_s = model_offset(model, option, t).value();
            let e_ns = earth_line(&c.earth_lab, t);
            let noise = plan.sigma_ns * super::budget::std_normal(&mut rng);
            // dev = model − reading + e  ⇒  reading = model + e − dev (+ noise).
            let reading_s = model_s + (e_ns - c.truth(t, mjd0) + noise) * 1e-9;
            out.push(Comparison {
                mjd_utc: t,
                lunar_clock: c.code.clone(),
                earth_lab: c.earth_lab.clone(),
                reading_s,
                u_a_ns: plan.sigma_ns,
                u_b_ns: plan.u_b_ns,
            });
            k += 1;
        }
    }
    Ok((out, earth, mjd0))
}

/// Write comparisons as the CSV [`parse_comparisons`] reads.
pub fn comparisons_csv(c: &[Comparison]) -> String {
    let mut s = String::from("mjd_utc,lunar_clock,earth_lab,reading_s,u_a_ns,u_b_ns\n");
    for x in c {
        s.push_str(&format!(
            "{:.6},{},{},{:.13},{},{}\n",
            x.mjd_utc, x.lunar_clock, x.earth_lab, x.reading_s, x.u_a_ns, x.u_b_ns
        ));
    }
    s
}

/// Write an Earth series as the CSV [`EarthSeries::from_csv`] reads.
pub fn earth_csv(e: &EarthSeries) -> String {
    let mut s = String::from("mjd,lab,utc_minus_utck_ns,u_ns\n");
    for (lab, v) in &e.labs {
        for (m, x, u) in v {
            s.push_str(&format!("{m},{lab},{x:.4},{u}\n"));
        }
    }
    s
}
