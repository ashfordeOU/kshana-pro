// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Digital-twin recalibration from telemetry: evidence pack on the hosted `calibrate` verdict
//!
//! A model that should track flight data drifts away from it: a clock ages, a spacecraft's drag
//! changes. This pack shows the recalibration and its drift over time. It holds **no fitting
//! code**: the parameter fit is the hosted verdict service's `calibrate` method
//! ([`crate::remote_verdict`], owner decision of 19 November 2026; docs/REMOTE-VERDICTS.md).
//! What this module does around it:
//!
//! 1. splits the flown telemetry (`t_s,value,sigma`) into windows;
//! 2. for each window, writes the `calibrate` request: the stated parameters (priors and bounds)
//!    and the model, either as an arithmetic **expression** or as a **design of runs** of the
//!    local model over parameter sets the request states (the service's two model kinds);
//! 3. sends it, and **verifies** each signed verdict against the trusted service key;
//! 4. computes, **locally**, the residuals of the model before (the initial parameters) and
//!    after (the returned posterior) for each window, and the **drift** of each parameter over
//!    the windows: a weighted straight-line fit of the posterior values against window mid-time
//!    with its standard error.
//!
//! Two worked models on open-engine physics:
//!
//! * `clock`: time error `x0 + TIE(y, d, t)` (ns), with the deterministic time-interval error
//!   `y·t + ½·d·t²` of [`kshana::holdover::deterministic_tie`]; sent as an expression;
//! * `orbit-decay`: altitude of a near-circular orbit decaying by drag in the US Standard
//!   Atmosphere 1976 ([`kshana::reentry::us76_density`]) with ballistic coefficient `b`
//!   (m²/kg); sent as a design of runs over stated `b` values.
//!
//! The pack carries every request, every signed verdict, the local residuals and drift, and is
//! signed. MODELLED: calibrated parameters are what best explains the telemetry under the model,
//! not a validation.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::remote_verdict::{self, SignedVerdict, VerdictTransport};
use crate::transport::Table;

/// Pro feature (the pack; the fit itself is licensed by the service).
pub const FEATURE: &str = "pro-twin-recalibration";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-twin-recalibration/1";
/// Label.
pub const EVIDENCE: &str = "MODELLED: parameters fitted by the hosted calibrate verdict (signed, \
verified here) under the stated model; residuals and drift computed locally from the returned \
parameters. Calibrated parameters explain the telemetry supplied under the model supplied; they \
are not a validation.";

/// A model parameter, as the service takes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub name: String,
    pub initial: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_sigma: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper: Option<f64>,
}

/// The model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Model {
    /// Clock time error (ns): parameters `x0` (ns), `y` (ns/s), `d` (ns/s²).
    Clock,
    /// Altitude (km) of a decaying near-circular orbit from `altitude0_km` at t = 0; parameter
    /// `b` (Cd·A/m, m²/kg); the design runs over `b_design`.
    OrbitDecay {
        altitude0_km: f64,
        b_design: Vec<f64>,
    },
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub data_source: String,
    pub observable: String,
    pub telemetry: String,
    pub window_s: f64,
    pub model: Model,
    #[serde(rename = "parameter")]
    pub parameters: Vec<Param>,
    /// Drift is called significant above this many standard errors.
    #[serde(default = "d_k")]
    pub drift_k: f64,
}

fn d_k() -> f64 {
    3.0
}

#[derive(Clone, Debug, PartialEq)]
struct Obs {
    t: f64,
    value: f64,
    sigma: f64,
}

/// One window.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Window {
    pub n: usize,
    pub from_s: f64,
    pub to_s: f64,
    pub observations: usize,
    pub posterior: Vec<(String, f64, Option<f64>)>,
    pub converged: Option<bool>,
    pub rms_before: f64,
    pub rms_after: f64,
    pub chi2_before: f64,
    pub chi2_after: f64,
    pub verdict_sha256: String,
}

/// Drift of one parameter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Drift {
    pub parameter: String,
    /// Units of the parameter per day.
    pub slope_per_day: Option<f64>,
    pub slope_std_err_per_day: Option<f64>,
    pub significant: Option<bool>,
}

/// The pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pack {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub data_source: String,
    pub model: Model,
    pub windows: Vec<Window>,
    pub drift: Vec<Drift>,
    pub verdict: String,
}

fn read_obs(text: &str) -> Result<Vec<Obs>, String> {
    let t = Table::parse(text)?;
    let (ct, cv, cs) = (t.need("t_s")?, t.need("value")?, t.need("sigma")?);
    let mut v = vec![];
    for i in 0..t.rows.len() {
        let n = |c| -> Result<f64, String> {
            t.num(i, Some(c))?
                .ok_or_else(|| format!("telemetry row {}: empty", i + 2))
        };
        let o = Obs {
            t: n(ct)?,
            value: n(cv)?,
            sigma: n(cs)?,
        };
        if o.sigma.is_nan() || o.sigma <= 0.0 {
            return Err(format!("telemetry row {}: sigma must be positive", i + 2));
        }
        v.push(o);
    }
    v.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok(v)
}

const GM: f64 = 3.986_004_418e14;
const RE: f64 = 6_378_137.0;

/// Altitude (km) at each time (sorted) of a near-circular orbit decaying by drag (US76).
pub fn orbit_altitudes_km(h0_km: f64, b: f64, times: &[f64]) -> Vec<f64> {
    let mut a = RE + h0_km * 1e3;
    let mut t = 0.0;
    let mut out = Vec::with_capacity(times.len());
    for &tt in times {
        while t < tt && a - RE > 120e3 {
            let dt = (tt - t).min(600.0);
            let dadt = -(GM * a).sqrt() * kshana::reentry::us76_density(a - RE) * b;
            a += dadt * dt;
            t += dt;
        }
        out.push((a - RE) / 1e3);
    }
    out
}

fn get(p: &[(String, f64)], n: &str) -> Result<f64, String> {
    p.iter()
        .find(|x| x.0 == n)
        .map(|x| x.1)
        .ok_or_else(|| format!("parameter '{n}' is required by the model"))
}

/// The local model's predictions at `times` for parameter values `p`.
pub fn predict(m: &Model, p: &[(String, f64)], times: &[f64]) -> Result<Vec<f64>, String> {
    match m {
        Model::Clock => {
            let (x0, y, d) = (get(p, "x0")?, get(p, "y")?, get(p, "d")?);
            Ok(times
                .iter()
                .map(|&t| x0 + kshana::holdover::deterministic_tie(y * 1e-9, d * 1e-9, t) * 1e9)
                .collect())
        }
        Model::OrbitDecay { altitude0_km, .. } => {
            Ok(orbit_altitudes_km(*altitude0_km, get(p, "b")?, times))
        }
    }
}

/// The `calibrate` request body for one window.
pub fn request_body(q: &Request, obs: &[(f64, f64, f64)]) -> Result<Value, String> {
    let model = match &q.model {
        Model::Clock => json!({
            "kind": "expression",
            "observables": { q.observable.clone(): "x0 + y*t + 0.5*d*t*t" },
        }),
        Model::OrbitDecay { b_design, .. } => {
            if b_design.len() < 3 {
                return Err("orbit-decay: give at least three b_design values".into());
            }
            let times: Vec<f64> = obs.iter().map(|o| o.0).collect();
            let mut runs = vec![];
            for &b in b_design {
                let pred = predict(&q.model, &[("b".into(), b)], &times)?;
                runs.push(json!({
                    "params": {"b": b},
                    "predictions": { q.observable.clone(): times.iter().zip(&pred).map(|(t, v)| [*t, *v]).collect::<Vec<_>>() },
                }));
            }
            json!({"kind": "design", "runs": runs})
        }
    };
    Ok(json!({
        "parameters": q.parameters,
        "model": model,
        "observations": obs.iter().map(|(t, v, s)| json!({"observable": q.observable, "t_s": t, "value": v, "sigma": s})).collect::<Vec<_>>(),
    }))
}

fn rms_chi2(m: &Model, p: &[(String, f64)], obs: &[Obs]) -> Result<(f64, f64), String> {
    let times: Vec<f64> = obs.iter().map(|o| o.t).collect();
    let pr = predict(m, p, &times)?;
    let n = obs.len().max(1) as f64;
    let ss: f64 = obs
        .iter()
        .zip(&pr)
        .map(|(o, v)| (o.value - v).powi(2))
        .sum();
    let c2: f64 = obs
        .iter()
        .zip(&pr)
        .map(|(o, v)| ((o.value - v) / o.sigma).powi(2))
        .sum();
    Ok(((ss / n).sqrt(), c2 / n))
}

/// What the run produced: the pack and, per window, the request sent and the verdict received.
pub struct Run {
    pub pack: Pack,
    pub exchanges: Vec<(Value, SignedVerdict)>,
}

/// Run the windows against the service through `t`, verifying every verdict with `trust`.
pub fn run(
    q: &Request,
    telemetry: &str,
    t: &dyn VerdictTransport,
    token: &str,
    trust: &str,
) -> Result<Run, String> {
    if q.data_source.trim().is_empty() {
        return Err("data_source must be stated".into());
    }
    if q.window_s.is_nan() || q.window_s <= 0.0 {
        return Err("window_s must be positive".into());
    }
    let obs = read_obs(telemetry)?;
    let (Some(first), Some(last)) = (obs.first(), obs.last()) else {
        return Err("no telemetry".into());
    };
    let initial: Vec<(String, f64)> = q
        .parameters
        .iter()
        .map(|p| (p.name.clone(), p.initial))
        .collect();
    predict(&q.model, &initial, &[first.t])?;
    let mut windows = vec![];
    let mut exchanges = vec![];
    let mut w0 = first.t;
    while w0 <= last.t {
        let w1 = w0 + q.window_s;
        let wo: Vec<Obs> = obs
            .iter()
            .filter(|o| o.t >= w0 && o.t < w1)
            .cloned()
            .collect();
        if wo.len() > q.parameters.len() {
            let body = request_body(
                q,
                &wo.iter()
                    .map(|o| (o.t, o.value, o.sigma))
                    .collect::<Vec<_>>(),
            )?;
            let v = remote_verdict::request(t, "calibrate", token, &body.to_string())?;
            let problems = remote_verdict::verify(&v, trust);
            if !problems.is_empty() {
                return Err(format!(
                    "window {}: the verdict does not verify: {}",
                    windows.len() + 1,
                    problems.join("; ")
                ));
            }
            let post: Vec<(String, f64, Option<f64>)> = v.verdict["posterior"]
                .as_array()
                .ok_or("verdict has no posterior")?
                .iter()
                .filter_map(|p| {
                    Some((
                        p["name"].as_str()?.to_string(),
                        p["value"].as_f64()?,
                        p["std_err"].as_f64(),
                    ))
                })
                .collect();
            let after: Vec<(String, f64)> = initial
                .iter()
                .map(|(n, x)| {
                    (
                        n.clone(),
                        post.iter().find(|p| &p.0 == n).map(|p| p.1).unwrap_or(*x),
                    )
                })
                .collect();
            let (rb, cb) = rms_chi2(&q.model, &initial, &wo)?;
            let (ra, ca) = rms_chi2(&q.model, &after, &wo)?;
            windows.push(Window {
                n: windows.len() + 1,
                from_s: w0,
                to_s: w1,
                observations: wo.len(),
                posterior: post,
                converged: v.verdict["converged"].as_bool(),
                rms_before: rb,
                rms_after: ra,
                chi2_before: cb,
                chi2_after: ca,
                verdict_sha256: v.verdict_sha256.clone(),
            });
            exchanges.push((body, v));
        }
        w0 = w1;
    }
    if windows.is_empty() {
        return Err("no window holds more observations than parameters".into());
    }
    // Drift: weighted line fit of each parameter's value against window mid-time.
    let mut drift = vec![];
    for p in &q.parameters {
        let pts: Vec<(f64, f64, f64)> = windows
            .iter()
            .filter_map(|w| {
                let x = w.posterior.iter().find(|x| x.0 == p.name)?;
                Some((
                    (w.from_s + w.to_s) / 2.0 / 86_400.0,
                    x.1,
                    x.2.unwrap_or(1.0).max(1e-300),
                ))
            })
            .collect();
        let d = if pts.len() >= 2 {
            let (mut s, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for (x, y, e) in &pts {
                let w = 1.0 / (e * e);
                s += w;
                sx += w * x;
                sy += w * y;
                sxx += w * x * x;
                sxy += w * x * y;
            }
            let del = s * sxx - sx * sx;
            if del > 0.0 {
                let slope = (s * sxy - sx * sy) / del;
                let se = (s / del).sqrt();
                Drift {
                    parameter: p.name.clone(),
                    slope_per_day: Some(slope),
                    slope_std_err_per_day: Some(se),
                    significant: Some(slope.abs() > q.drift_k * se),
                }
            } else {
                Drift {
                    parameter: p.name.clone(),
                    slope_per_day: None,
                    slope_std_err_per_day: None,
                    significant: None,
                }
            }
        } else {
            Drift {
                parameter: p.name.clone(),
                slope_per_day: None,
                slope_std_err_per_day: None,
                significant: None,
            }
        };
        drift.push(d);
    }
    let verdict = format!(
        "{} window(s); RMS before {:.4}, after {:.4} (mean); drift significant for: {} (MODELLED)",
        windows.len(),
        windows.iter().map(|w| w.rms_before).sum::<f64>() / windows.len() as f64,
        windows.iter().map(|w| w.rms_after).sum::<f64>() / windows.len() as f64,
        {
            let s: Vec<&str> = drift
                .iter()
                .filter(|d| d.significant == Some(true))
                .map(|d| d.parameter.as_str())
                .collect();
            if s.is_empty() {
                "none".to_string()
            } else {
                s.join(", ")
            }
        }
    );
    Ok(Run {
        pack: Pack {
            format: FORMAT,
            evidence: EVIDENCE,
            title: q.title.clone(),
            data_source: q.data_source.clone(),
            model: q.model.clone(),
            windows,
            drift,
            verdict,
        },
        exchanges,
    })
}

/// Write the pack directory (unsigned): `pack.json`, `pack.html`, `exchanges/<n>-request.json`
/// and `exchanges/<n>-verdict.json`, and the inputs.
pub fn write(
    out: &std::path::Path,
    r: &Run,
    request_src: &str,
    telemetry: &str,
) -> Result<(), String> {
    crate::transport::write_pack(out, &r.pack, &html(&r.pack), "request.toml", request_src)?;
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::write(out.join("inputs").join("telemetry.csv"), telemetry).map_err(io)?;
    let ex = out.join("exchanges");
    std::fs::create_dir_all(&ex).map_err(io)?;
    for (i, (b, v)) in r.exchanges.iter().enumerate() {
        std::fs::write(
            ex.join(format!("{:02}-request.json", i + 1)),
            serde_json::to_string_pretty(b).unwrap_or_default(),
        )
        .map_err(io)?;
        std::fs::write(
            ex.join(format!("{:02}-verdict.json", i + 1)),
            serde_json::to_string_pretty(v).unwrap_or_default(),
        )
        .map_err(io)?;
    }
    Ok(())
}

/// HTML page.
pub fn html(p: &Pack) -> String {
    let o = |x: Option<f64>| x.map(|v| format!("{v:.4e}")).unwrap_or_else(|| "-".into());
    crate::transport::page(
        &p.title,
        p.evidence,
        &p.verdict,
        &[format!("Data source: {}.", p.data_source)],
        &[
            (
                "Windows".into(),
                vec![
                    "#",
                    "from (s)",
                    "to (s)",
                    "obs",
                    "posterior",
                    "RMS before",
                    "RMS after",
                    "verdict SHA-256",
                ],
                p.windows
                    .iter()
                    .map(|w| {
                        vec![
                            w.n.to_string(),
                            format!("{:.0}", w.from_s),
                            format!("{:.0}", w.to_s),
                            w.observations.to_string(),
                            w.posterior
                                .iter()
                                .map(|(n, v, e)| format!("{n} = {v:.6e} ± {}", o(*e)))
                                .collect::<Vec<_>>()
                                .join("; "),
                            format!("{:.4}", w.rms_before),
                            format!("{:.4}", w.rms_after),
                            w.verdict_sha256[..16.min(w.verdict_sha256.len())].to_string(),
                        ]
                    })
                    .collect(),
            ),
            (
                "Drift".into(),
                vec!["parameter", "slope per day", "std err", "significant"],
                p.drift
                    .iter()
                    .map(|d| {
                        vec![
                            d.parameter.clone(),
                            o(d.slope_per_day),
                            o(d.slope_std_err_per_day),
                            d.significant.map(|b| b.to_string()).unwrap_or_default(),
                        ]
                    })
                    .collect(),
            ),
        ],
    )
}

/// SYNTHETIC clock telemetry over four days (one point per 10 min, 2 ns noise): offset 50 ns,
/// frequency 0.002 ns/s (2 ppt), drift growing day by day. Returns (telemetry CSV, request TOML).
pub fn synth_clock() -> (String, String) {
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut nz = || {
        let mut u = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f64 / (1u64 << 53) as f64
        };
        (u() + u() + u() + u() - 2.0) * 1.732
    };
    let mut csv = String::from("t_s,value,sigma\n");
    for k in 0..576 {
        let t = k as f64 * 600.0;
        let day = (t / 86_400.0).floor();
        let y = 0.002 + 0.0005 * day;
        csv += &format!("{t:.0},{:.3},2.0\n", 50.0 + y * t + 2.0 * nz());
    }
    let req = r#"title = "SYNTHETIC clock twin recalibration"
data_source = "SYNTHETIC (kshana-pro twin-recal synth)"
observable = "time_error_ns"
telemetry = "telemetry.csv"
window_s = 86400.0

[model]
kind = "clock"

[[parameter]]
name = "x0"
initial = 0.0

[[parameter]]
name = "y"
initial = 0.0

[[parameter]]
name = "d"
initial = 0.0
prior = 0.0
prior_sigma = 1e-9
"#;
    (csv, req.into())
}
