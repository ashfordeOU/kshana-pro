// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Simulator self-test and replay-rig check (format [`FORMAT`]).
//!
//! A GNSS simulator (or an AWG/SDR rig replaying a station recording) is checked from the
//! outside: one or more **reference receivers**, cabled to its output, log C/N0 per
//! satellite, position fixes and a PPS offset against the simulator's PPS (TIC), and the
//! logs are compared with what the source was set to produce:
//!
//! * `mode = "simulator"`: the expected C/N0 is the scenario's power setting (one value, or
//!   one per PRN), the position the scenario's static position, the PPS offset the stated
//!   value;
//! * `mode = "replay-rig"`: the expected C/N0 per PRN is the mean the **station** logged
//!   while the replayed file was recorded (`expected.observations`), the position the
//!   station's surveyed position.
//!
//! Per receiver, four checks (each `pass`, `deviation` or `not-assessed`):
//!
//! 1. **power level**: the median over PRNs of `mean C/N0 − receiver bias − expected`
//!    (a common-mode offset; a simulator power-calibration error moves every satellite);
//! 2. **relative power**: the largest per-PRN deviation from that median, and PRNs expected
//!    but never tracked (a per-channel error moves one satellite);
//! 3. **position**: the 95th-percentile horizontal error of the fixes against the expected
//!    position (local flat-earth approximation, valid over metres);
//! 4. **time**: `|mean PPS offset − expected|`.
//!
//! **Attribution**: a deviation is put on the source (the simulator or the rig) only when
//! at least two reference receivers each show it and agree with each other within the
//! tolerance; when some receivers pass and others deviate, on those receivers; one receiver
//! alone cannot separate the two (`unattributed`). Verdict: `consistent`,
//! `simulator-deviation` / `rig-deviation`, `receiver-deviation` or `inconclusive`.
//!
//! MODELLED: tolerances are the user's; the C/N0 a receiver reports is itself only as good
//! as its own calibration (`cn0_bias_db`), which this check records but cannot verify. A
//! consistent result is evidence that the reference receivers saw what was expected, not a
//! calibration of the simulator.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{mean, median, numeric_csv, r, std_dev};
use crate::aviation::Inputs;

pub const FORMAT: &str = "kshana-simlab-selftest/1";
pub const METHOD: &str = "kshana-pro/simlab-selftest/v1";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    #[serde(default)]
    pub cn0_dbhz: Option<f64>,
    #[serde(default)]
    pub cn0_by_prn: BTreeMap<String, f64>,
    /// Replay-rig mode: the station's observations of the original recording.
    #[serde(default)]
    pub observations: Option<String>,
    /// `[lat_deg, lon_deg, h_m]`.
    #[serde(default)]
    pub position: Option<[f64; 3]>,
    #[serde(default)]
    pub pps_offset_ns: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tolerance {
    pub power_db: f64,
    pub spread_db: f64,
    pub position_m: f64,
    pub time_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverDef {
    pub name: String,
    #[serde(default)]
    pub observations: Option<String>,
    #[serde(default)]
    pub fixes: Option<String>,
    #[serde(default)]
    pub pps: Option<String>,
    #[serde(default)]
    pub cn0_bias_db: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub format: String,
    pub id: String,
    pub mode: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub expected: Expected,
    pub tolerance: Tolerance,
    #[serde(rename = "receiver")]
    pub receivers: Vec<ReceiverDef>,
}

/// One receiver's logs.
#[derive(Clone, Debug, Default)]
pub struct Logs {
    /// PRN → C/N0 samples.
    pub cn0: BTreeMap<String, Vec<f64>>,
    /// `(lat, lon, h)`.
    pub fixes: Vec<[f64; 3]>,
    pub pps_ns: Vec<f64>,
}

pub struct Loaded {
    pub req: Request,
    /// PRN → expected C/N0 (empty when only a common value is known).
    pub expected_cn0: BTreeMap<String, f64>,
    pub logs: Vec<Logs>,
    pub inputs: Inputs,
}

/// Parse `t_s,prn,cn0_dbhz` rows (header and malformed rows skipped).
pub fn parse_obs(text: &str) -> BTreeMap<String, Vec<f64>> {
    let mut m: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for l in text.lines() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() < 3 || f[0].parse::<f64>().is_err() {
            continue;
        }
        if let Ok(c) = f[2].parse::<f64>() {
            if c.is_finite() {
                m.entry(f[1].to_string()).or_default().push(c);
            }
        }
    }
    m
}

pub fn load(path: &Path) -> Result<Loaded, String> {
    let (src, mut inputs) = super::read_request(path, "selftest")?;
    let req: Request = toml::from_str(&src).map_err(|e| format!("selftest: {e}"))?;
    if req.format != FORMAT {
        return Err(format!(
            "selftest: format '{}' (expected {FORMAT})",
            req.format
        ));
    }
    if req.mode != "simulator" && req.mode != "replay-rig" {
        return Err("selftest: mode must be simulator or replay-rig".into());
    }
    let t = &req.tolerance;
    if !(t.power_db > 0.0 && t.spread_db > 0.0 && t.position_m > 0.0 && t.time_ns > 0.0) {
        return Err("selftest: every tolerance must be > 0".into());
    }
    if req.receivers.is_empty() {
        return Err("selftest: no [[receiver]]".into());
    }
    let base = super::base_of(path);
    let mut expected_cn0 = req.expected.cn0_by_prn.clone();
    if req.mode == "replay-rig" {
        let o = req
            .expected
            .observations
            .as_ref()
            .ok_or("selftest: replay-rig mode needs expected.observations (the station's)")?;
        let st = parse_obs(&inputs.read(base, o, "station-observations")?);
        if st.is_empty() {
            return Err(format!("{o}: no station observations"));
        }
        expected_cn0 = st.into_iter().map(|(k, v)| (k, mean(&v))).collect();
    }
    let mut logs = Vec::new();
    for rx in &req.receivers {
        let mut l = Logs::default();
        if let Some(o) = &rx.observations {
            l.cn0 = parse_obs(&inputs.read(base, o, &format!("observations:{}", rx.name))?);
        }
        if let Some(f) = &rx.fixes {
            let (rows, _) = numeric_csv(&inputs.read(base, f, &format!("fixes:{}", rx.name))?, 4);
            l.fixes = rows.iter().map(|r| [r[1], r[2], r[3]]).collect();
        }
        if let Some(p) = &rx.pps {
            let (rows, _) = numeric_csv(&inputs.read(base, p, &format!("pps:{}", rx.name))?, 2);
            l.pps_ns = rows.iter().map(|r| r[1]).collect();
        }
        logs.push(l);
    }
    Ok(Loaded {
        req,
        expected_cn0,
        logs,
        inputs,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    /// `pass`, `deviation` or `not-assessed`.
    pub outcome: String,
    pub value: Option<f64>,
    pub tolerance: f64,
    pub unit: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReceiverResult {
    pub name: String,
    pub cn0_bias_db: f64,
    /// PRN → mean C/N0 − bias − expected, dB.
    pub per_prn_delta_db: BTreeMap<String, f64>,
    pub missing_prns: Vec<String>,
    pub checks: Vec<Check>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attribution {
    pub check: String,
    /// `none`, `source`, `receiver`, `unattributed` or `inconclusive`.
    pub to: String,
    pub receivers: Vec<String>,
    /// The source's estimated offset (mean over receivers) when attributed to the source.
    pub source_estimate: Option<f64>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelfTestReport {
    pub method: &'static str,
    pub label: &'static str,
    pub evidence: &'static str,
    pub id: String,
    pub mode: String,
    pub source: String,
    pub verdict: String,
    pub receivers: Vec<ReceiverResult>,
    pub attribution: Vec<Attribution>,
    pub inputs: Vec<crate::aviation::InputRef>,
}

const CHECKS: [&str; 4] = ["power-level", "relative-power", "position", "time"];

fn check(id: &str, value: Option<f64>, tol: f64, unit: &str, detail: String) -> Check {
    let outcome = match value {
        None => "not-assessed",
        Some(v) if v.abs() <= tol => "pass",
        Some(_) => "deviation",
    };
    Check {
        id: id.into(),
        outcome: outcome.into(),
        value: value.map(|v| r(v, 3)),
        tolerance: tol,
        unit: unit.into(),
        detail,
    }
}

/// The 95th percentile (nearest rank).
fn p95(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let k = ((0.95 * s.len() as f64).ceil() as usize).clamp(1, s.len());
    s[k - 1]
}

fn horizontal_errors(fixes: &[[f64; 3]], at: [f64; 3]) -> Vec<f64> {
    const R: f64 = 6_371_000.0;
    let c = at[0].to_radians().cos();
    fixes
        .iter()
        .map(|f| {
            let dn = (f[0] - at[0]).to_radians() * R;
            let de = (f[1] - at[1]).to_radians() * R * c;
            dn.hypot(de)
        })
        .collect()
}

fn assess_receiver(l: &Loaded, i: usize) -> ReceiverResult {
    let rx = &l.req.receivers[i];
    let logs = &l.logs[i];
    let t = &l.req.tolerance;
    let e = &l.req.expected;
    let mut deltas = BTreeMap::new();
    for (prn, v) in &logs.cn0 {
        let exp = l.expected_cn0.get(prn).copied().or(e.cn0_dbhz);
        if let Some(x) = exp {
            deltas.insert(prn.clone(), r(mean(v) - rx.cn0_bias_db - x, 3));
        }
    }
    let missing: Vec<String> = l
        .expected_cn0
        .keys()
        .filter(|p| !logs.cn0.contains_key(*p))
        .cloned()
        .collect();
    let dv: Vec<f64> = deltas.values().copied().collect();
    let common = (!dv.is_empty()).then(|| median(&dv));
    let mut checks = vec![check(
        "power-level",
        common,
        t.power_db,
        "dB",
        format!(
            "median over {} PRN(s) of mean C/N0 − bias − expected",
            dv.len()
        ),
    )];
    let spread = common.map(|c| dv.iter().map(|d| (d - c).abs()).fold(0.0, f64::max));
    let mut rel = check(
        "relative-power",
        spread,
        t.spread_db,
        "dB",
        "largest per-PRN deviation from the common-mode offset".into(),
    );
    if !missing.is_empty() && common.is_some() {
        rel.outcome = "deviation".into();
        rel.detail = format!(
            "{}; expected PRN(s) never tracked: {}",
            rel.detail,
            missing.join(", ")
        );
    }
    checks.push(rel);
    let pos = match (e.position, logs.fixes.is_empty()) {
        (Some(at), false) => Some(p95(&horizontal_errors(&logs.fixes, at))),
        _ => None,
    };
    checks.push(check(
        "position",
        pos,
        t.position_m,
        "m",
        format!(
            "95th-percentile horizontal error of {} fix(es)",
            logs.fixes.len()
        ),
    ));
    let time = match (e.pps_offset_ns, logs.pps_ns.is_empty()) {
        (Some(x), false) => Some(mean(&logs.pps_ns) - x),
        _ => None,
    };
    checks.push(check(
        "time",
        time,
        t.time_ns,
        "ns",
        format!(
            "mean PPS offset − expected over {} reading(s), std {:.2} ns",
            logs.pps_ns.len(),
            std_dev(&logs.pps_ns)
        ),
    ));
    ReceiverResult {
        name: rx.name.clone(),
        cn0_bias_db: rx.cn0_bias_db,
        per_prn_delta_db: deltas,
        missing_prns: missing,
        checks,
    }
}

fn attribute(rs: &[ReceiverResult], id: &str, tol: f64) -> Attribution {
    let got: Vec<(&str, &Check)> = rs
        .iter()
        .filter_map(|x| Some((x.name.as_str(), x.checks.iter().find(|c| c.id == id)?)))
        .filter(|(_, c)| c.outcome != "not-assessed")
        .collect();
    let dev: Vec<&(&str, &Check)> = got
        .iter()
        .filter(|(_, c)| c.outcome == "deviation")
        .collect();
    let names = |v: &[&(&str, &Check)]| v.iter().map(|x| x.0.to_string()).collect::<Vec<_>>();
    let a = |to: &str, recv: Vec<String>, est: Option<f64>, d: String| Attribution {
        check: id.into(),
        to: to.into(),
        receivers: recv,
        source_estimate: est.map(|x| r(x, 3)),
        detail: d,
    };
    if got.is_empty() {
        return a("none", vec![], None, "not assessed by any receiver".into());
    }
    if dev.is_empty() {
        return a(
            "none",
            vec![],
            None,
            "every receiver within tolerance".into(),
        );
    }
    if got.len() == 1 {
        return a(
            "unattributed",
            names(&dev),
            None,
            "one reference receiver: its deviation cannot be separated from the source's".into(),
        );
    }
    if dev.len() < got.len() {
        return a(
            "receiver",
            names(&dev),
            None,
            format!(
                "{} of {} receivers deviate while the others pass: the deviation is in those receivers or their cabling",
                dev.len(),
                got.len()
            ),
        );
    }
    let vals: Vec<f64> = dev.iter().filter_map(|(_, c)| c.value).collect();
    let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let same_sign = vals.iter().all(|v| *v > 0.0) || vals.iter().all(|v| *v < 0.0);
    if same_sign && hi - lo <= tol {
        a(
            "source",
            names(&dev),
            Some(mean(&vals)),
            format!(
                "all {} receivers deviate alike (spread {:.3} within the tolerance): the source is the common cause",
                dev.len(),
                hi - lo
            ),
        )
    } else {
        a(
            "inconclusive",
            names(&dev),
            None,
            format!(
                "every receiver deviates but they disagree (spread {:.3}): check the receivers before the source",
                hi - lo
            ),
        )
    }
}

pub fn assess(l: &Loaded) -> SelfTestReport {
    let rs: Vec<ReceiverResult> = (0..l.req.receivers.len())
        .map(|i| assess_receiver(l, i))
        .collect();
    let t = &l.req.tolerance;
    let tol = |id: &str| match id {
        "power-level" => t.power_db,
        "relative-power" => t.spread_db,
        "position" => t.position_m,
        _ => t.time_ns,
    };
    let attribution: Vec<Attribution> = CHECKS.iter().map(|c| attribute(&rs, c, tol(c))).collect();
    let has = |to: &str| attribution.iter().any(|a| a.to == to);
    let verdict = if has("source") {
        if l.req.mode == "replay-rig" {
            "rig-deviation"
        } else {
            "simulator-deviation"
        }
    } else if has("inconclusive") || has("unattributed") {
        "inconclusive"
    } else if has("receiver") {
        "receiver-deviation"
    } else {
        "consistent"
    };
    SelfTestReport {
        method: METHOD,
        label: super::LABEL,
        evidence: super::EVIDENCE,
        id: l.req.id.clone(),
        mode: l.req.mode.clone(),
        source: l.req.source.clone(),
        verdict: verdict.into(),
        receivers: rs,
        attribution,
        inputs: l.inputs.refs.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_and_flat_earth_error() {
        assert_eq!(p95(&(1..=20).map(|x| x as f64).collect::<Vec<_>>()), 19.0);
        let e = horizontal_errors(&[[52.0 + 1e-5, 5.0, 0.0]], [52.0, 5.0, 0.0]);
        assert!((e[0] - 1.112).abs() < 0.01);
        let o = parse_obs("t,prn,cn0\n0,G01,45\n1,G01,47\nbad\n");
        assert_eq!(o["G01"], vec![45.0, 47.0]);
    }
}
