// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # LTV / EVA navigation safety evidence (E24 B2404)
//!
//! Evidence for a traverse's navigation safety case from its own log: what the navigation
//! system reported (position and 1-sigma), and where available the surveyed truth. Three
//! questions, each with the thresholds the user states:
//!
//! 1. **Was the reported sigma honest?** With truth logged: Stanford regions of the error
//!    against the protection level `PL = k·σ` and the alert limit, an exact bound on the
//!    misleading-information rate with an effective sample size, and the two-sided Gaussian
//!    overbound check of the per-axis errors `Δe/σ`, `Δn/σ` (`σ` is the reported 1-sigma
//!    per horizontal axis; shared with the transport tools,
//!    [`crate::transport::integrity`]).
//! 2. **Were hazards kept clear with margin?** For every declared keep-out circle (a crater
//!    rim, a slope, a lander exclusion zone), the margin `distance − radius − PL` per epoch;
//!    epochs with a negative margin are those at which the navigation could not show the
//!    vehicle clear. With truth: incursions, and whether the PL bound covered them.
//! 3. **Could the crew walk back?** Per epoch, the time to return to the declared base along a
//!    straight line at the declared walking speed (inflated by the PL) against the
//!    consumables left above the reserve.
//!
//! **MODELLED.** Statistics and geometry on the records as supplied; flat local frame
//! (east, north) in metres; straight-line walk-back. Not a safety approval and not a
//! navigation solution: nothing is fused or corrected here.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{positive, r, read_rel, EVIDENCE};
use crate::transport::integrity::{overbound, stanford, Epoch, Overbound, Stanford};
use crate::transport::Table;

/// Pro feature.
pub const FEATURE: &str = "pro-lunar-traverse-safety";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-lunar-traverse-safety/1";
/// Method identifier.
pub const METHOD_ID: &str = "kshana-pro/traverse-safety/v1";

/// A keep-out circle in the local frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hazard {
    pub name: String,
    pub east_m: f64,
    pub north_m: f64,
    pub radius_m: f64,
}

/// Walk-back constraint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Walkback {
    /// Base (habitat, lander, rover) in the local frame.
    pub base: [f64; 2],
    /// Walking speed the crew can hold (m/s).
    pub speed_mps: f64,
    /// Log time (s) at which consumables reach zero.
    pub consumables_end_s: f64,
    /// Reserve to keep at arrival (s).
    pub reserve_s: f64,
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// CSV `t_s,east_m,north_m,sigma_m[,true_east_m,true_north_m]`; `sigma_m` is the reported
    /// 1-sigma per horizontal axis.
    pub log: String,
    /// Protection level multiplier `k` (`PL = k·σ`).
    pub pl_k: f64,
    /// Alert limit (m).
    pub alert_limit_m: f64,
    /// Decorrelation time of the errors (s), for the effective sample size.
    pub decorrelation_s: f64,
    #[serde(default)]
    pub hazards: Vec<Hazard>,
    #[serde(default)]
    pub walkback: Option<Walkback>,
}

/// One logged epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub t: f64,
    pub e: f64,
    pub n: f64,
    pub sigma: f64,
    pub truth: Option<(f64, f64)>,
}

/// Hazard summary.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HazardResult {
    pub name: String,
    /// Smallest `distance − radius − PL` (m) and when.
    pub min_margin_m: f64,
    pub min_margin_t_s: f64,
    /// Epochs with a negative margin.
    pub not_shown_clear: usize,
    /// With truth: epochs truly inside the hazard.
    pub incursions: Option<usize>,
    /// With truth: incursions at which the estimate with its PL did not overlap the hazard.
    pub incursions_not_covered: Option<usize>,
}

/// Walk-back summary.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WalkbackResult {
    pub min_margin_s: f64,
    pub min_margin_t_s: f64,
    /// First epoch at which the walk-back margin is negative.
    pub first_breach_t_s: Option<f64>,
    pub breach_epochs: usize,
    pub max_distance_m: f64,
}

/// The result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub method: &'static str,
    pub title: String,
    pub evidence: &'static str,
    pub request: Request,
    pub log_sha256: String,
    pub epochs: usize,
    pub duration_s: f64,
    pub truth_epochs: usize,
    pub stanford: Option<Stanford>,
    pub overbound: Option<Overbound>,
    pub hazards: Vec<HazardResult>,
    pub walkback: Option<WalkbackResult>,
    pub verdict: String,
    pub limits: Vec<&'static str>,
}

const LIMITS: &[&str] = &[
    "Flat local frame in metres; hazards are circles as declared.",
    "Walk-back is straight-line at a constant speed; terrain, slope and fatigue are not modelled.",
    "The integrity statistics speak to the epochs logged; they are evidence for the stated sigma, not a proof beyond the data.",
];

/// Parse the log.
pub fn parse_log(text: &str) -> Result<Vec<Row>, String> {
    let t = Table::parse(text)?;
    let (ct, ce, cn, cs) = (
        t.need("t_s")?,
        t.need("east_m")?,
        t.need("north_m")?,
        t.need("sigma_m")?,
    );
    let (cte, ctn) = (t.col("true_east_m"), t.col("true_north_m"));
    let mut out = Vec::with_capacity(t.rows.len());
    for i in 0..t.rows.len() {
        let need = |c| {
            t.num(i, Some(c))?
                .ok_or_else(|| format!("row {}: empty cell in '{}'", i + 2, t.cols[c]))
        };
        let sigma = need(cs)?;
        if sigma.is_nan() || sigma <= 0.0 {
            return Err(format!("row {}: sigma_m must be positive", i + 2));
        }
        let truth = match (t.num(i, cte)?, t.num(i, ctn)?) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        };
        out.push(Row {
            t: need(ct)?,
            e: need(ce)?,
            n: need(cn)?,
            sigma,
            truth,
        });
    }
    if out.len() < 2 {
        return Err("the log needs at least two epochs".into());
    }
    if out.windows(2).any(|w| w[1].t <= w[0].t) {
        return Err("t_s must increase".into());
    }
    Ok(out)
}

fn validate(q: &Request) -> Result<(), String> {
    positive("pl_k", q.pl_k)?;
    positive("alert_limit_m", q.alert_limit_m)?;
    positive("decorrelation_s", q.decorrelation_s)?;
    for h in &q.hazards {
        positive(&format!("hazard '{}' radius_m", h.name), h.radius_m)?;
    }
    if let Some(w) = &q.walkback {
        positive("walkback.speed_mps", w.speed_mps)?;
        if w.reserve_s.is_nan() || w.reserve_s < 0.0 {
            return Err("walkback.reserve_s must be zero or positive".into());
        }
    }
    Ok(())
}

/// Run on a request whose log is relative to `base`.
pub fn run(q: &Request, base: &Path) -> Result<Report, String> {
    validate(q)?;
    let text = read_rel(base, &q.log)?;
    let rows = parse_log(&text).map_err(|e| format!("{}: {e}", q.log))?;
    Ok(analyse(
        q,
        &rows,
        crate::forensics::inputs::sha256_hex(text.as_bytes()),
    ))
}

/// Analyse parsed rows.
pub fn analyse(q: &Request, rows: &[Row], sha: String) -> Report {
    let pl = |x: &Row| q.pl_k * x.sigma;
    let truth: Vec<(&Row, f64)> = rows
        .iter()
        .filter_map(|x| x.truth.map(|(te, tn)| (x, (x.e - te).hypot(x.n - tn))))
        .collect();
    let dt = (rows[rows.len() - 1].t - rows[0].t) / (rows.len() - 1) as f64;
    let (st, ob) = if truth.len() >= 2 {
        let ep: Vec<Epoch> = truth
            .iter()
            .map(|(x, e)| Epoch {
                err_m: *e,
                pl_m: pl(x),
                al_m: q.alert_limit_m,
            })
            .collect();
        // Per-axis normalised errors: the reported sigma is per horizontal axis.
        let z: Vec<f64> = truth
            .iter()
            .flat_map(|(x, _)| {
                let (te, tn) = x.truth.expect("truth rows");
                [(x.e - te) / x.sigma, (x.n - tn) / x.sigma]
            })
            .collect();
        (
            Some(stanford(&ep, dt, q.decorrelation_s)),
            Some(overbound(&z)),
        )
    } else {
        (None, None)
    };
    let hazards: Vec<HazardResult> = q
        .hazards
        .iter()
        .map(|h| {
            let mut min = (f64::INFINITY, rows[0].t);
            let mut not_clear = 0;
            for x in rows {
                let m = (x.e - h.east_m).hypot(x.n - h.north_m) - h.radius_m - pl(x);
                if m < min.0 {
                    min = (m, x.t);
                }
                if m < 0.0 {
                    not_clear += 1;
                }
            }
            let (inc, uncovered) = if truth.is_empty() {
                (None, None)
            } else {
                let inside: Vec<&&Row> = truth
                    .iter()
                    .map(|(x, _)| x)
                    .filter(|x| {
                        let (te, tn) = x.truth.expect("truth rows");
                        (te - h.east_m).hypot(tn - h.north_m) < h.radius_m
                    })
                    .collect();
                let unc = inside
                    .iter()
                    .filter(|x| (x.e - h.east_m).hypot(x.n - h.north_m) - h.radius_m - pl(x) >= 0.0)
                    .count();
                (Some(inside.len()), Some(unc))
            };
            HazardResult {
                name: h.name.clone(),
                min_margin_m: r(min.0, 3),
                min_margin_t_s: min.1,
                not_shown_clear: not_clear,
                incursions: inc,
                incursions_not_covered: uncovered,
            }
        })
        .collect();
    let walkback = q.walkback.as_ref().map(|w| {
        let mut min = (f64::INFINITY, rows[0].t);
        let mut first = None;
        let mut n = 0;
        let mut maxd: f64 = 0.0;
        for x in rows {
            let d = (x.e - w.base[0]).hypot(x.n - w.base[1]);
            maxd = maxd.max(d);
            let need = (d + pl(x)) / w.speed_mps;
            let avail = w.consumables_end_s - w.reserve_s - x.t;
            let m = avail - need;
            if m < min.0 {
                min = (m, x.t);
            }
            if m < 0.0 {
                n += 1;
                first.get_or_insert(x.t);
            }
        }
        WalkbackResult {
            min_margin_s: r(min.0, 1),
            min_margin_t_s: min.1,
            first_breach_t_s: first,
            breach_epochs: n,
            max_distance_m: r(maxd, 1),
        }
    });
    let mut parts = Vec::new();
    if let Some(s) = &st {
        parts.push(format!(
            "{} of {} truth epochs MI, {} HMI",
            s.mi, s.epochs, s.hmi
        ));
    }
    if let Some(o) = &ob {
        parts.push(if o.bounded {
            "stated sigma overbounds the errors".to_string()
        } else {
            format!(
                "stated sigma does not overbound the errors (inflation {:.2} needed)",
                o.inflation_needed
            )
        });
    }
    let unclear: usize = hazards.iter().map(|h| h.not_shown_clear).sum();
    parts.push(format!(
        "{unclear} epoch(s) not shown clear of a declared hazard"
    ));
    if let Some(w) = &walkback {
        parts.push(match w.first_breach_t_s {
            Some(t) => format!("walk-back margin negative from t = {t:.0} s"),
            None => format!("walk-back margin held (minimum {:.0} s)", w.min_margin_s),
        });
    }
    let verdict = format!(
        "{}. Evidence for the traverse safety case, not a safety approval.",
        parts.join("; ")
    );
    Report {
        format: FORMAT,
        method: METHOD_ID,
        title: q.title.clone(),
        evidence: EVIDENCE,
        request: q.clone(),
        log_sha256: sha,
        epochs: rows.len(),
        duration_s: rows[rows.len() - 1].t - rows[0].t,
        truth_epochs: truth.len(),
        stanford: st,
        overbound: ob,
        hazards,
        walkback,
        verdict,
        limits: LIMITS.to_vec(),
    }
}

/// HTML page.
pub fn html(rep: &Report) -> String {
    let hrows = rep
        .hazards
        .iter()
        .map(|h| {
            vec![
                h.name.clone(),
                format!("{:.1}", h.min_margin_m),
                format!("{:.0}", h.min_margin_t_s),
                h.not_shown_clear.to_string(),
                h.incursions.map(|x| x.to_string()).unwrap_or_default(),
                h.incursions_not_covered
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
            ]
        })
        .collect();
    let mut wrows = Vec::new();
    if let Some(w) = &rep.walkback {
        wrows.push(vec![
            format!("{:.0}", w.min_margin_s),
            format!("{:.0}", w.min_margin_t_s),
            w.first_breach_t_s
                .map(|t| format!("{t:.0}"))
                .unwrap_or_else(|| "none".into()),
            format!("{:.0}", w.max_distance_m),
        ]);
    }
    crate::transport::page(
        &rep.title,
        rep.evidence,
        &rep.verdict,
        &[],
        &[
            (
                "Hazards".into(),
                vec![
                    "hazard",
                    "min margin (m)",
                    "at t (s)",
                    "epochs not shown clear",
                    "incursions",
                    "incursions not covered by PL",
                ],
                hrows,
            ),
            (
                "Walk-back".into(),
                vec![
                    "min margin (s)",
                    "at t (s)",
                    "first breach (s)",
                    "max distance (m)",
                ],
                wrows,
            ),
        ],
    )
}
