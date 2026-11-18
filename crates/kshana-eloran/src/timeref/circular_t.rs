// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! BIPM Circular T importer (section 1: `[UTC − UTC(k)]` at 5-day MJD intervals).
//!
//! The parser follows the published plain-text layout of section 1:
//!
//! ```text
//! 1 - Coordinated Universal Time UTC and its local realizations UTC(k). ...
//!     From 2017 January 1, 0h UTC, TAI-UTC = 37 s.
//!
//! Date 2026     0h UTC    OCT 26   OCT 31   NOV  5 ...   Uncertainty/ns  Notes
//!               MJD        61339    61344    61349 ...    uA    uB    u
//! Laboratory k                       [UTC-UTC(k)]/ns
//! AOS  (Borowiec)            1.1      0.9      0.7 ...   0.3   1.5   1.5
//! NPL  (Teddington)          0.4      0.2       -  ...   0.3   1.2   1.2
//! ```
//!
//! i.e. a header line whose first word is `MJD` followed by the column MJDs, then one line
//! per laboratory: the lab code, an optional location in parentheses, one value per MJD
//! column (ns, `-` when missing), then up to three uncertainties (uA, uB, u) and optional
//! notes. Section 1 ends at the line starting `2 -`. Lines that do not fit are reported in
//! [`CircularT::skipped`] rather than silently dropped. The column layout has changed over
//! the years; check the import of an older/newer circular against its text.
//!
//! Sign: Circular T gives `UTC − UTC(k)`; [`crate::timebase::TimeRefModel`] uses
//! `x = UTC(k) − UTC`, so values are negated on conversion.

use super::{mjd_from_utc, utc_from_mjd};
use crate::sync::UtcStamp;
use crate::timebase::{TimeRefModel, TimeStep};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One laboratory row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabRow {
    pub code: String,
    /// Location text in parentheses, if any.
    pub name: String,
    /// `UTC − UTC(k)` (ns) per MJD column; `None` where the circular shows `-`.
    pub utc_minus_utck_ns: Vec<Option<f64>>,
    pub u_a_ns: Option<f64>,
    pub u_b_ns: Option<f64>,
    pub u_ns: Option<f64>,
    pub notes: String,
}

/// A parsed Circular T section 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CircularT {
    /// Circular number, when the header shows `CIRCULAR T <n>`.
    pub number: Option<u32>,
    /// TAI − UTC (s) stated in section 1.
    pub tai_minus_utc_s: Option<f64>,
    /// Column MJDs.
    pub mjd: Vec<i64>,
    /// Rows by lab code.
    pub labs: BTreeMap<String, LabRow>,
    /// Non-empty lines after the MJD header that were not lab rows.
    pub skipped: Vec<String>,
}

fn is_lab_code(t: &str) -> bool {
    !t.is_empty()
        && t.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && t.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn value(t: &str) -> Option<Option<f64>> {
    if t == "-" || t == "--" {
        return Some(None);
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite()).map(Some)
}

impl CircularT {
    /// Parse the text of a circular (whole circular or just section 1).
    pub fn parse(text: &str) -> Result<CircularT, String> {
        let lines: Vec<&str> = text.lines().collect();
        let mut number = None;
        for l in &lines {
            let u = l.to_uppercase();
            if let Some(p) = u.find("CIRCULAR T") {
                number = u[p + 10..]
                    .split_whitespace()
                    .next()
                    .and_then(|t| t.parse::<u32>().ok());
                if number.is_some() {
                    break;
                }
            }
        }
        let start = lines
            .iter()
            .position(|l| l.trim_start().starts_with("1 -"))
            .unwrap_or(0);
        let end = lines[start + 1..]
            .iter()
            .position(|l| l.trim_start().starts_with("2 -"))
            .map(|p| p + start + 1)
            .unwrap_or(lines.len());
        let sec = &lines[start..end];
        let mut tai_minus_utc_s = None;
        for l in sec {
            if let Some(p) = l.find("TAI-UTC") {
                let rest = l[p + 7..].trim_start().trim_start_matches('=').trim_start();
                tai_minus_utc_s = rest
                    .split_whitespace()
                    .next()
                    .and_then(|t| t.parse::<f64>().ok());
                if tai_minus_utc_s.is_some() {
                    break;
                }
            }
        }
        let hdr = sec
            .iter()
            .position(|l| l.split_whitespace().next() == Some("MJD"))
            .ok_or("Circular T: no MJD header line in section 1")?;
        let mjd: Vec<i64> = sec[hdr]
            .split_whitespace()
            .skip(1)
            .map_while(|t| t.parse::<i64>().ok())
            .collect();
        if mjd.len() < 2 {
            return Err("Circular T: fewer than two MJD columns".into());
        }
        if mjd.windows(2).any(|w| w[1] <= w[0]) {
            return Err("Circular T: MJD columns must increase".into());
        }
        let n = mjd.len();
        let mut labs = BTreeMap::new();
        let mut skipped = Vec::new();
        for l in &sec[hdr + 1..] {
            let t = l.trim();
            if t.is_empty() {
                continue;
            }
            let Some(code) = t.split_whitespace().next().filter(|c| is_lab_code(c)) else {
                skipped.push(t.to_string());
                continue;
            };
            let mut rest = t[code.len()..].trim_start();
            let mut name = String::new();
            if rest.starts_with('(') {
                if let Some(b) = rest.find(')') {
                    name = rest[1..b].trim().to_string();
                    rest = rest[b + 1..].trim_start();
                }
            }
            let toks: Vec<&str> = rest.split_whitespace().collect();
            if toks.len() < n {
                skipped.push(t.to_string());
                continue;
            }
            let vals: Option<Vec<Option<f64>>> = toks[..n].iter().map(|x| value(x)).collect();
            let Some(vals) = vals else {
                skipped.push(t.to_string());
                continue;
            };
            let mut unc = Vec::new();
            let mut k = n;
            while k < toks.len() && unc.len() < 3 {
                match toks[k].parse::<f64>() {
                    Ok(v) => unc.push(v),
                    Err(_) => break,
                }
                k += 1;
            }
            labs.insert(
                code.to_string(),
                LabRow {
                    code: code.to_string(),
                    name,
                    utc_minus_utck_ns: vals,
                    u_a_ns: unc.first().copied(),
                    u_b_ns: unc.get(1).copied(),
                    u_ns: unc.get(2).copied(),
                    notes: toks[k..].join(" "),
                },
            );
        }
        if labs.is_empty() {
            return Err("Circular T: no laboratory rows found".into());
        }
        Ok(CircularT {
            number,
            tai_minus_utc_s,
            mjd,
            labs,
            skipped,
        })
    }

    /// Available `(MJD, x = UTC(k) − UTC in ns)` points of a lab.
    pub fn points(&self, lab: &str) -> Result<Vec<(f64, f64)>, String> {
        let r = self
            .labs
            .get(lab)
            .ok_or_else(|| format!("Circular T: no laboratory {lab:?}"))?;
        Ok(self
            .mjd
            .iter()
            .zip(&r.utc_minus_utck_ns)
            .filter_map(|(m, v)| v.map(|v| (*m as f64, -v)))
            .collect())
    }

    /// Build a [`TimeRefModel`] for `lab` over the scene `[start, start + duration_s]`.
    pub fn time_ref_model(
        &self,
        lab: &str,
        start: UtcStamp,
        duration_s: f64,
        mode: ModelMode,
    ) -> Result<ImportedTimeRef, String> {
        let pts = self.points(lab)?;
        if pts.len() < 2 {
            return Err(format!(
                "Circular T: laboratory {lab} has fewer than two values"
            ));
        }
        if !(duration_s >= 0.0) {
            return Err("duration must be >= 0".into());
        }
        let m0 = mjd_from_utc(start);
        let m1 = m0 + duration_s / 86_400.0;
        let (lo, hi) = (pts[0].0, pts[pts.len() - 1].0);
        let mut notes = Vec::new();
        let extrapolated = m0 < lo || m1 > hi;
        if extrapolated {
            notes.push(format!(
                "scene MJD {m0:.3}–{m1:.3} lies (partly) outside the data {lo}–{hi}: extrapolated"
            ));
        }
        let id = format!("UTC({lab})");
        let mut model = TimeRefModel::ideal(&id);
        let rms;
        match mode {
            ModelMode::LinearFit => {
                let (a, b) = fit(&pts);
                model.offset_ns = a + b * m0;
                model.drift_ns_per_day = b;
                rms = (pts
                    .iter()
                    .map(|(m, x)| (x - a - b * m).powi(2))
                    .sum::<f64>()
                    / pts.len() as f64)
                    .sqrt();
            }
            ModelMode::PiecewiseLinear { step_interval_s } => {
                if !(step_interval_s > 0.0) {
                    return Err("step_interval_s must be > 0".into());
                }
                let pw = |m: f64| interp(&pts, m);
                model.offset_ns = pw(m0);
                model.drift_ns_per_day = slope_at(&pts, m0);
                // knots inside the scene change the slope: follow them with phase steps on
                // a grid (max error |Δdrift|·step_interval between grid points)
                if pts.iter().any(|(m, _)| *m > m0 && *m < m1) {
                    let mut steps_ns = 0.0;
                    let mut k = 1u64;
                    loop {
                        let t = k as f64 * step_interval_s;
                        if t > duration_s + 1e-9 {
                            break;
                        }
                        let lin = model.offset_ns + model.drift_ns_per_day * t / 86_400.0;
                        let d = pw(m0 + t / 86_400.0) - lin - steps_ns;
                        if d.abs() > 1e-6 {
                            model.steps.push(TimeStep {
                                at_s: t,
                                step_ns: d,
                            });
                            steps_ns += d;
                        }
                        k += 1;
                    }
                    notes.push(format!(
                        "piecewise-linear: slope changes inside the scene followed by {} phase steps every {step_interval_s} s",
                        model.steps.len()
                    ));
                }
                rms = 0.0;
            }
        }
        Ok(ImportedTimeRef {
            model,
            lab: lab.into(),
            points_used: pts.len(),
            scene_start_mjd: m0,
            scene_start_utc: utc_from_mjd(m0).iso(),
            extrapolated,
            fit_rms_ns: rms,
            notes,
        })
    }

    /// Models for several labs.
    pub fn time_ref_models(
        &self,
        labs: &[&str],
        start: UtcStamp,
        duration_s: f64,
        mode: ModelMode,
    ) -> Result<Vec<ImportedTimeRef>, String> {
        labs.iter()
            .map(|l| self.time_ref_model(l, start, duration_s, mode))
            .collect()
    }
}

/// How a lab's 5-day values become a model.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ModelMode {
    /// Least-squares offset + drift over all values of the circular.
    LinearFit,
    /// Linear interpolation between the published values; slope changes inside the scene
    /// are followed by phase steps every `step_interval_s`.
    PiecewiseLinear { step_interval_s: f64 },
}

/// An imported model with its provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportedTimeRef {
    pub model: TimeRefModel,
    pub lab: String,
    pub points_used: usize,
    pub scene_start_mjd: f64,
    pub scene_start_utc: String,
    pub extrapolated: bool,
    /// RMS residual of the linear fit (ns; 0 for piecewise).
    pub fit_rms_ns: f64,
    pub notes: Vec<String>,
}

/// Least squares `x = a + b·m` (b per day).
fn fit(p: &[(f64, f64)]) -> (f64, f64) {
    let n = p.len() as f64;
    let mm = p.iter().map(|q| q.0).sum::<f64>() / n;
    let xm = p.iter().map(|q| q.1).sum::<f64>() / n;
    let (mut smm, mut smx) = (0.0, 0.0);
    for (m, x) in p {
        smm += (m - mm) * (m - mm);
        smx += (m - mm) * (x - xm);
    }
    let b = smx / smm;
    (xm - b * mm, b)
}

/// Index of the segment used at `m` (clamped to the end segments).
fn seg(p: &[(f64, f64)], m: f64) -> usize {
    let mut i = 0;
    while i + 2 < p.len() && m >= p[i + 1].0 {
        i += 1;
    }
    i
}

fn slope_at(p: &[(f64, f64)], m: f64) -> f64 {
    let i = seg(p, m);
    (p[i + 1].1 - p[i].1) / (p[i + 1].0 - p[i].0)
}

fn interp(p: &[(f64, f64)], m: f64) -> f64 {
    let i = seg(p, m);
    p[i].1 + slope_at(p, m) * (m - p[i].0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timebase::TimeRef;

    /// Synthetic values in the published section-1 layout (NOT real BIPM data).
    const SAMPLE: &str = r#"
                                   CIRCULAR T 999
                                                     SYNTHETIC TEST SAMPLE - NOT BIPM DATA
1 - Coordinated Universal Time UTC and its local realizations UTC(k). Computed values
    of [UTC-UTC(k)] and uncertainties valid for the period of this Circular.
    From 2017 January 1, 0h UTC, TAI-UTC = 37 s.

Date 2026     0h UTC     OCT 26    OCT 31    NOV  5    NOV 10    NOV 15    NOV 20   Uncertainty/ns  Notes
              MJD         61339     61344     61349     61354     61359     61364    uA    uB    u
Laboratory k                                    [UTC-UTC(k)]/ns
AAA  (Alpha City)          1.0       2.0       3.0       4.0       5.0       6.0     0.3   1.5   1.5
BBB  (Beta Town)          -2.0      -2.0       -       -1.0      -0.5       0.0     0.3   2.0   2.0
CCC                         0.0       0.0       0.0      10.0      20.0      30.0     0.4   2.5   2.6   (1)
DDD  (Delta, Upper Shire)   -         -         -         -        4.0        -      0.5   5.0   5.0

2 - International Atomic Time TAI ...
EEE  (Not section 1)        9.0       9.0       9.0       9.0       9.0       9.0     0.1   0.1   0.1
"#;

    #[test]
    fn parses_section_1() {
        let c = CircularT::parse(SAMPLE).unwrap();
        assert_eq!(c.number, Some(999));
        assert_eq!(c.tai_minus_utc_s, Some(37.0));
        assert_eq!(c.mjd, vec![61339, 61344, 61349, 61354, 61359, 61364]);
        assert_eq!(c.labs.len(), 4);
        assert!(!c.labs.contains_key("EEE"));
        let a = &c.labs["AAA"];
        assert_eq!(a.name, "Alpha City");
        assert_eq!(a.utc_minus_utck_ns[5], Some(6.0));
        assert_eq!((a.u_a_ns, a.u_b_ns, a.u_ns), (Some(0.3), Some(1.5), Some(1.5)));
        assert_eq!(c.labs["BBB"].utc_minus_utck_ns[2], None);
        assert_eq!(c.labs["CCC"].name, "");
        assert_eq!(c.labs["CCC"].notes, "(1)");
        assert_eq!(c.labs["DDD"].name, "Delta, Upper Shire");
        assert!(c.skipped.iter().any(|s| s.starts_with("Laboratory")));
        // sign flip: x = UTC(k) − UTC
        assert_eq!(c.points("BBB").unwrap()[0], (61339.0, 2.0));
        assert_eq!(c.points("BBB").unwrap().len(), 5);
        assert!(CircularT::parse("nothing here").is_err());
    }

    #[test]
    fn linear_fit_model() {
        let c = CircularT::parse(SAMPLE).unwrap();
        // AAA: UTC−UTC(AAA) = 1 + 0.2·(MJD − 61339) ⇒ x = −1 − 0.2·(MJD − 61339)
        let start = UtcStamp::parse("2026-11-18T12:00:00Z").unwrap(); // MJD 61362.5
        let m = c
            .time_ref_model("AAA", start, 3600.0, ModelMode::LinearFit)
            .unwrap();
        assert_eq!(m.model.id, "UTC(AAA)");
        assert!((m.model.offset_ns - (-1.0 - 0.2 * 23.5)).abs() < 1e-9);
        assert!((m.model.drift_ns_per_day + 0.2).abs() < 1e-12);
        assert!(m.fit_rms_ns < 1e-9 && !m.extrapolated);
        let late = UtcStamp::parse("2026-12-01T00:00:00Z").unwrap();
        assert!(c
            .time_ref_model("AAA", late, 10.0, ModelMode::LinearFit)
            .unwrap()
            .extrapolated);
        assert!(c
            .time_ref_model("DDD", start, 10.0, ModelMode::LinearFit)
            .is_err());
        assert!(c
            .time_ref_model("ZZZ", start, 10.0, ModelMode::LinearFit)
            .is_err());
    }

    #[test]
    fn piecewise_model_follows_knots() {
        let c = CircularT::parse(SAMPLE).unwrap();
        // CCC: x = 0 to MJD 61349, then −2 ns/day. Scene from MJD 61348 for 2 days.
        let start = super::super::utc_from_mjd(61348.0);
        let dur = 2.0 * 86_400.0;
        let m = c
            .time_ref_model(
                "CCC",
                start,
                dur,
                ModelMode::PiecewiseLinear {
                    step_interval_s: 600.0,
                },
            )
            .unwrap();
        assert_eq!(m.model.offset_ns, 0.0);
        assert_eq!(m.model.drift_ns_per_day, 0.0);
        assert!(!m.model.steps.is_empty());
        let r = TimeRef::new(m.model.clone(), dur);
        for t in [0.0, 43_200.0, 86_400.0, 129_600.0, 172_800.0] {
            let want = interp(&c.points("CCC").unwrap(), 61348.0 + t / 86_400.0);
            let got = r.offset_s(t) * 1e9;
            assert!((got - want).abs() < 1e-6, "t = {t}: {got} vs {want}");
        }
        // between grid points the error is bounded by |Δdrift|·interval
        let t = 86_400.0 + 300.0;
        let want = interp(&c.points("CCC").unwrap(), 61348.0 + t / 86_400.0);
        assert!((r.offset_s(t) * 1e9 - want).abs() <= 2.0 * 600.0 / 86_400.0 + 1e-9);
        // a scene inside one segment gets no steps
        let m2 = c
            .time_ref_model(
                "CCC",
                super::super::utc_from_mjd(61355.0),
                3600.0,
                ModelMode::PiecewiseLinear {
                    step_interval_s: 60.0,
                },
            )
            .unwrap();
        assert!(m2.model.steps.is_empty());
        assert!((m2.model.offset_ns + 12.0).abs() < 1e-9);
        assert!((m2.model.drift_ns_per_day + 2.0).abs() < 1e-12);
        let all = c
            .time_ref_models(&["AAA", "BBB"], start, 60.0, ModelMode::LinearFit)
            .unwrap();
        assert_eq!(all.len(), 2);
    }
}
