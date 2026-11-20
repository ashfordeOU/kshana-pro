// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Basis risk and regional accumulation (B2802)
//!
//! **Basis risk** ([`basis`]): a parametric trigger ([`super::trigger`]) is evaluated and its
//! modelled payouts are set against the loss records the user supplies (CSV
//! `insured,start_utc,end_utc,loss[,lat,lon]`). Fired events and losses that overlap in time
//! (within `match_tolerance_h`) form one cluster; per cluster the payout P and the loss L are
//! compared. Reported: payout without recorded loss, loss without payout (and whether a trigger
//! event below the duration threshold overlapped it), shortfall Σmax(L−P, 0), excess
//! Σmax(P−L, 0), the basis-risk ratio Σ|P−L| / ΣL with a seeded bootstrap 90 % interval over
//! clusters, the hit rate, and the payout-loss correlation. With positions, the location basis:
//! each loss's distance to the nearest trigger site and the share of loss beyond
//! `max_site_distance_km`.
//!
//! **Accumulation** ([`accumulation`]): a portfolio (CSV `insured,lat,lon,exposure[,dependency]`,
//! exposure in the user's units: sum insured or daily business-interruption value; dependency
//! 0–1, default 1) is set under the footprints of historical interference events
//! (`kshana-pnt-interference-event/1` with a location) and deterministic circular scenarios. The
//! modelled loss of an insured under a footprint is
//! exposure × dependency × min(max_loss_ratio, loss_ratio_per_hour × hours). Reported per
//! footprint, per grid cell (exposure and concentration, Herfindahl index), the largest event,
//! the period aggregate, its annualised value and an empirical exceedance table.
//!
//! Both are reproducible from their inputs: input hashes and a result fingerprint are recorded.
//! Evidence: **MODELLED**. The loss ratios are the user's assumptions, not fitted values.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::trigger::{self, TriggerResult};
use super::{
    distance_km, esc, fingerprint, html_table, num, page, pearson, quantile, read_input, sig9, utc,
    InputRef, SplitMix,
};

pub const BASIS_FORMAT: &str = "kshana-pro-basis-risk/1";
pub const BASIS_METHOD: &str = "kshana-pro/basis-risk/v1";
pub const BASIS_FILE: &str = "basis-risk.json";
pub const BASIS_LABEL: &str = "MODELLED: trigger payouts under the stated cover compared with the \
loss records supplied; a measure of the mismatch, not a claim or pricing decision.";

pub const ACC_FORMAT: &str = "kshana-pro-accumulation/1";
pub const ACC_METHOD: &str = "kshana-pro/accumulation/v1";
pub const ACC_FILE: &str = "accumulation.json";
pub const ACC_LABEL: &str = "MODELLED: portfolio exposure under historical event footprints and \
stated scenarios with the user's loss ratios; not a probable-maximum-loss certification or a \
capital figure.";

fn d_tol() -> f64 {
    6.0
}
fn d_boot() -> usize {
    1000
}
fn d_seed() -> u64 {
    1
}
fn d_dist() -> f64 {
    50.0
}

/// A basis-risk request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasisRequest {
    pub title: String,
    /// The trigger request (the `insurance trigger` format), relative to this file.
    pub trigger: String,
    pub losses: Vec<String>,
    #[serde(default = "d_tol")]
    pub match_tolerance_h: f64,
    #[serde(default = "d_boot")]
    pub bootstrap: usize,
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_dist")]
    pub max_site_distance_km: f64,
}

#[derive(Clone, Debug)]
struct Loss {
    insured: String,
    t0: f64,
    t1: f64,
    loss: f64,
    pos: Option<(f64, f64)>,
}

/// One cluster of overlapping payouts and losses.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cluster {
    pub start_utc: String,
    pub end_utc: String,
    pub events: Vec<String>,
    pub insureds: Vec<String>,
    pub payout: f64,
    pub loss: f64,
    /// `paid with loss`, `payout without recorded loss`, `loss without payout`.
    pub class: String,
    /// A trigger event below the duration threshold overlapped this loss.
    pub below_duration_event: bool,
}

/// The location basis of one loss.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocationRow {
    pub insured: String,
    pub nearest_site: String,
    pub distance_km: f64,
    pub loss: f64,
    pub paid: bool,
}

/// Basis-risk metrics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BasisMetrics {
    pub total_payout: f64,
    pub total_loss: f64,
    pub shortfall: f64,
    pub excess: f64,
    pub basis_risk_ratio: Option<f64>,
    pub basis_risk_ratio_90: Option<[f64; 2]>,
    pub hit_rate: Option<f64>,
    pub payout_without_loss_rate: Option<f64>,
    pub payout_loss_correlation: Option<f64>,
    pub loss_beyond_distance_share: Option<f64>,
}

/// A basis-risk result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BasisResult {
    pub format: String,
    pub method: String,
    pub label: String,
    pub statement: String,
    pub title: String,
    pub match_tolerance_h: f64,
    pub max_site_distance_km: f64,
    pub bootstrap: usize,
    pub seed: u64,
    pub inputs: Vec<InputRef>,
    pub trigger: TriggerResult,
    pub clusters: Vec<Cluster>,
    pub location: Vec<LocationRow>,
    pub metrics: BasisMetrics,
    pub notes: Vec<String>,
    pub fingerprint: String,
}

fn csv_rows(text: &str, name: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or_else(|| format!("{name}: empty"))?
        .split(',')
        .map(|c| c.trim().to_ascii_lowercase())
        .collect();
    let rows = lines
        .map(|l| l.split(',').map(|c| c.trim().to_string()).collect())
        .collect();
    Ok((head, rows))
}

fn col(head: &[String], name: &str, file: &str) -> Result<usize, String> {
    head.iter()
        .position(|h| h == name)
        .ok_or_else(|| format!("{file}: no '{name}' column"))
}

fn f(row: &[String], k: usize, what: &str) -> Result<f64, String> {
    row.get(k)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{what}: not a number"))
}

fn load_losses(
    names: &[String],
    base: &Option<PathBuf>,
    inputs: &mut Vec<InputRef>,
) -> Result<Vec<Loss>, String> {
    let mut out = vec![];
    for n in names {
        let (b, rf) = read_input(base, n, "losses")?;
        let (head, rows) = csv_rows(&String::from_utf8_lossy(&b), n)?;
        let (ci, cs, ce, cl) = (
            col(&head, "insured", n)?,
            col(&head, "start_utc", n)?,
            col(&head, "end_utc", n)?,
            col(&head, "loss", n)?,
        );
        let (cla, clo) = (
            head.iter().position(|h| h == "lat"),
            head.iter().position(|h| h == "lon"),
        );
        for (i, r) in rows.iter().enumerate() {
            let w = format!("{n} row {}", i + 2);
            let g = |k: usize| r.get(k).map(String::as_str).unwrap_or("");
            let (t0, t1) = (utc(&w, g(cs))?, utc(&w, g(ce))?);
            let loss = f(r, cl, &w)?;
            if t1 < t0 || loss < 0.0 {
                return Err(format!("{w}: end before start or negative loss"));
            }
            let pos = match (cla, clo) {
                (Some(a), Some(o)) if !g(a).is_empty() => Some((f(r, a, &w)?, f(r, o, &w)?)),
                _ => None,
            };
            out.push(Loss {
                insured: g(ci).to_string(),
                t0,
                t1,
                loss,
                pos,
            });
        }
        inputs.push(rf);
    }
    Ok(out)
}

fn ratio(c: &[(f64, f64)]) -> Option<f64> {
    let l: f64 = c.iter().map(|x| x.1).sum();
    (l > 0.0).then(|| c.iter().map(|x| (x.0 - x.1).abs()).sum::<f64>() / l)
}

/// Compute basis risk. `path` is the request file (relative names resolve against its folder).
pub fn basis(path: &Path) -> Result<(BasisResult, String), String> {
    let (r, src, base): (BasisRequest, _, _) = super::read_request(path)?;
    let tpath = super::resolve(&base, &r.trigger);
    let (tr, tsrc, tbase): (trigger::TriggerRequest, _, _) = super::read_request(&tpath)?;
    if tr.cover.is_none() {
        return Err(format!(
            "{}: basis risk needs a [cover] schedule",
            r.trigger
        ));
    }
    let t = trigger::evaluate(&tr, &tbase)?;
    let mut inputs = vec![InputRef {
        role: "trigger".into(),
        file: r.trigger.clone(),
        sha256: crate::forensics::inputs::sha256_hex(tsrc.as_bytes()),
        bytes: tsrc.len(),
    }];
    let losses = load_losses(&r.losses, &base, &mut inputs)?;
    let tol = r.match_tolerance_h * 3600.0;
    // Intervals: fired events (payout) and losses; cluster by overlap within the tolerance.
    let mut iv: Vec<(f64, f64, Option<usize>, Option<usize>)> = vec![];
    for (i, e) in t.events.iter().enumerate().filter(|(_, e)| e.fired) {
        iv.push((e.t0, e.t1, Some(i), None));
    }
    for (j, l) in losses.iter().enumerate() {
        iv.push((l.t0, l.t1, None, Some(j)));
    }
    iv.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut groups: Vec<(f64, f64, Vec<usize>, Vec<usize>)> = vec![];
    for (a, b, e, l) in iv {
        match groups.last_mut() {
            Some(g) if a - g.1 <= tol => {
                g.1 = g.1.max(b);
                g.2.extend(e);
                g.3.extend(l);
            }
            _ => groups.push((a, b, e.into_iter().collect(), l.into_iter().collect())),
        }
    }
    let mut clusters = vec![];
    let mut pairs = vec![];
    for (a, b, es, ls) in groups {
        let p: f64 = es.iter().map(|&i| t.events[i].payout).sum();
        let l: f64 = ls.iter().map(|&j| losses[j].loss).sum();
        let class = match (p > 0.0 || !es.is_empty(), l > 0.0) {
            (true, true) => "paid with loss",
            (true, false) => "payout without recorded loss",
            _ => "loss without payout",
        };
        let below = es.is_empty()
            && t.events
                .iter()
                .any(|e| !e.fired && e.t0 <= b + tol && e.t1 >= a - tol);
        let mut ins: Vec<String> = ls.iter().map(|&j| losses[j].insured.clone()).collect();
        ins.sort();
        ins.dedup();
        pairs.push((p, l));
        clusters.push(Cluster {
            start_utc: super::iso(a),
            end_utc: super::iso(b),
            events: es.iter().map(|&i| t.events[i].id.clone()).collect(),
            insureds: ins,
            payout: sig9(p),
            loss: sig9(l),
            class: class.into(),
            below_duration_event: below,
        });
    }
    // Location basis.
    let sites: Vec<(&str, f64, f64)> = tr
        .sites
        .iter()
        .filter_map(|s| Some((s.id.as_str(), s.lat_deg?, s.lon_deg?)))
        .collect();
    let mut location = vec![];
    for l in &losses {
        let (Some((la, lo)), false) = (l.pos, sites.is_empty()) else {
            continue;
        };
        let (s, d) = sites
            .iter()
            .map(|s| (s.0, distance_km(la, lo, s.1, s.2)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or(("", f64::NAN));
        let paid = clusters.iter().any(|c| {
            c.insureds.contains(&l.insured)
                && c.payout > 0.0
                && super::utc("", &c.start_utc).is_ok_and(|a| a <= l.t1 + tol)
                && super::utc("", &c.end_utc).is_ok_and(|b| b >= l.t0 - tol)
        });
        location.push(LocationRow {
            insured: l.insured.clone(),
            nearest_site: s.into(),
            distance_km: sig9(d),
            loss: sig9(l.loss),
            paid,
        });
    }
    let total_loss: f64 = pairs.iter().map(|x| x.1).sum();
    let with_loss = pairs.iter().filter(|x| x.1 > 0.0).count();
    let with_pay = clusters.iter().filter(|c| !c.events.is_empty()).count();
    // Seeded bootstrap over clusters.
    let mut rng = SplitMix(r.seed);
    let mut boots = vec![];
    if pairs.len() >= 2 {
        for _ in 0..r.bootstrap {
            let s: Vec<(f64, f64)> = (0..pairs.len())
                .map(|_| pairs[rng.below(pairs.len())])
                .collect();
            if let Some(x) = ratio(&s) {
                boots.push(x);
            }
        }
    }
    let loc_loss: f64 = location.iter().map(|x| x.loss).sum();
    let metrics = BasisMetrics {
        total_payout: sig9(pairs.iter().map(|x| x.0).sum()),
        total_loss: sig9(total_loss),
        shortfall: sig9(pairs.iter().map(|x| (x.1 - x.0).max(0.0)).sum()),
        excess: sig9(pairs.iter().map(|x| (x.0 - x.1).max(0.0)).sum()),
        basis_risk_ratio: ratio(&pairs).map(sig9),
        basis_risk_ratio_90: match (quantile(&boots, 0.05), quantile(&boots, 0.95)) {
            (Some(a), Some(b)) => Some([sig9(a), sig9(b)]),
            _ => None,
        },
        hit_rate: (with_loss > 0).then(|| {
            sig9(
                clusters
                    .iter()
                    .filter(|c| c.class == "paid with loss")
                    .count() as f64
                    / with_loss as f64,
            )
        }),
        payout_without_loss_rate: (with_pay > 0).then(|| {
            sig9(
                clusters
                    .iter()
                    .filter(|c| c.class == "payout without recorded loss")
                    .count() as f64
                    / with_pay as f64,
            )
        }),
        payout_loss_correlation: pearson(
            &pairs.iter().map(|x| x.0).collect::<Vec<_>>(),
            &pairs.iter().map(|x| x.1).collect::<Vec<_>>(),
        )
        .map(sig9),
        loss_beyond_distance_share: (loc_loss > 0.0).then(|| {
            sig9(
                location
                    .iter()
                    .filter(|x| x.distance_km > r.max_site_distance_km)
                    .map(|x| x.loss)
                    .sum::<f64>()
                    / loc_loss,
            )
        }),
    };
    let mut notes = vec![
        "Payouts are compared with the sum of the losses supplied in each cluster; the cover is \
         read as protecting that aggregate."
            .to_string(),
    ];
    if clusters.iter().any(|c| c.below_duration_event) {
        notes.push(
            "Some losses overlap a trigger event that stayed below the duration threshold \
             (duration basis)."
                .into(),
        );
    }
    let mut res = BasisResult {
        format: BASIS_FORMAT.into(),
        method: BASIS_METHOD.into(),
        label: BASIS_LABEL.into(),
        statement: super::STATEMENT.into(),
        title: r.title.clone(),
        match_tolerance_h: r.match_tolerance_h,
        max_site_distance_km: r.max_site_distance_km,
        bootstrap: r.bootstrap,
        seed: r.seed,
        inputs,
        trigger: t,
        clusters,
        location,
        metrics,
        notes,
        fingerprint: String::new(),
    };
    res.fingerprint = fingerprint(&res);
    Ok((res, src))
}

fn opt(x: Option<f64>) -> String {
    x.map(num).unwrap_or_else(|| "n/a".into())
}

/// The basis-risk HTML page.
pub fn basis_html(r: &BasisResult) -> String {
    let m = &r.metrics;
    let mut b = html_table(
        &["Measure", "Value"],
        &[
            vec!["Total modelled payout".into(), num(m.total_payout)],
            vec!["Total recorded loss".into(), num(m.total_loss)],
            vec!["Shortfall (loss not met)".into(), num(m.shortfall)],
            vec!["Excess (payout over loss)".into(), num(m.excess)],
            vec!["Basis-risk ratio".into(), opt(m.basis_risk_ratio)],
            vec![
                "Basis-risk ratio, bootstrap 90 %".into(),
                m.basis_risk_ratio_90
                    .map(|x| format!("{} to {}", num(x[0]), num(x[1])))
                    .unwrap_or_else(|| "n/a".into()),
            ],
            vec!["Hit rate".into(), opt(m.hit_rate)],
            vec![
                "Payout without recorded loss".into(),
                opt(m.payout_without_loss_rate),
            ],
            vec![
                "Payout-loss correlation".into(),
                opt(m.payout_loss_correlation),
            ],
            vec![
                format!("Loss beyond {} km of a site", r.max_site_distance_km),
                opt(m.loss_beyond_distance_share),
            ],
        ],
    );
    b.push_str("<h2>Clusters</h2>");
    b.push_str(&html_table(
        &[
            "Start", "End", "Events", "Insureds", "Payout", "Loss", "Class",
        ],
        &r.clusters
            .iter()
            .map(|c| {
                vec![
                    c.start_utc.clone(),
                    c.end_utc.clone(),
                    c.events.join(" "),
                    c.insureds.join(" "),
                    num(c.payout),
                    num(c.loss),
                    format!(
                        "{}{}",
                        c.class,
                        if c.below_duration_event {
                            " (below-duration trigger event)"
                        } else {
                            ""
                        }
                    ),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    for n in r.notes.iter().chain(&r.trigger.notes) {
        b.push_str(&format!("<p class=\"l\">{}</p>", esc(n)));
    }
    b.push_str(&format!(
        "<p class=\"l\">Fingerprint {}</p>",
        esc(&r.fingerprint)
    ));
    page(&r.title, &r.label, &b)
}

// ----- accumulation -------------------------------------------------------------------------

fn d_cell() -> f64 {
    1.0
}

/// A deterministic circular scenario.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub radius_km: f64,
    pub duration_h: f64,
}

/// An accumulation request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccumulationRequest {
    pub title: String,
    pub currency: String,
    pub portfolio: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub scenario: Vec<Scenario>,
    pub loss_ratio_per_hour: f64,
    pub max_loss_ratio: f64,
    #[serde(default)]
    pub footprint_buffer_km: f64,
    #[serde(default = "d_cell")]
    pub cell_deg: f64,
}

#[derive(Clone, Debug)]
struct Insured {
    lat: f64,
    lon: f64,
    exposure: f64,
    dependency: f64,
}

/// One footprint's accumulation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FootprintRow {
    pub id: String,
    /// `historical` or `scenario`.
    pub source: String,
    pub kind: String,
    pub start_utc: Option<String>,
    pub radius_km: f64,
    pub hours: f64,
    pub insureds: usize,
    pub exposure_inside: f64,
    pub modelled_loss: f64,
}

/// One grid cell.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CellRow {
    /// South-west corner `[lat, lon]`.
    pub cell: [f64; 2],
    pub insureds: usize,
    pub exposure: f64,
    pub share: f64,
}

/// One exceedance row (historical events).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExceedanceRow {
    pub rank: usize,
    pub loss: f64,
    pub empirical_return_period_y: f64,
}

/// Portfolio totals.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccTotals {
    pub insureds: usize,
    pub exposure: f64,
    pub historical_events: usize,
    pub events_without_location: usize,
    pub largest_event_loss: f64,
    pub period_aggregate_loss: f64,
    pub annualised_aggregate_loss: f64,
    pub top_cell_share: f64,
    pub herfindahl: f64,
    pub largest_scenario_loss: f64,
}

/// An accumulation result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccumulationResult {
    pub format: String,
    pub method: String,
    pub label: String,
    pub statement: String,
    pub title: String,
    pub currency: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub loss_ratio_per_hour: f64,
    pub max_loss_ratio: f64,
    pub footprint_buffer_km: f64,
    pub cell_deg: f64,
    pub inputs: Vec<InputRef>,
    pub footprints: Vec<FootprintRow>,
    pub cells: Vec<CellRow>,
    pub exceedance: Vec<ExceedanceRow>,
    pub totals: AccTotals,
    pub fingerprint: String,
}

fn load_portfolio(
    name: &str,
    base: &Option<PathBuf>,
    inputs: &mut Vec<InputRef>,
) -> Result<Vec<Insured>, String> {
    let (b, rf) = read_input(base, name, "portfolio")?;
    let (head, rows) = csv_rows(&String::from_utf8_lossy(&b), name)?;
    col(&head, "insured", name)?;
    let (ca, co, ce) = (
        col(&head, "lat", name)?,
        col(&head, "lon", name)?,
        col(&head, "exposure", name)?,
    );
    let cd = head.iter().position(|h| h == "dependency");
    let mut out = vec![];
    for (i, r) in rows.iter().enumerate() {
        let w = format!("{name} row {}", i + 2);
        let dep = match cd.and_then(|k| r.get(k)).filter(|v| !v.is_empty()) {
            Some(_) => f(r, cd.unwrap_or(0), &w)?,
            None => 1.0,
        };
        let x = Insured {
            lat: f(r, ca, &w)?,
            lon: f(r, co, &w)?,
            exposure: f(r, ce, &w)?,
            dependency: dep,
        };
        if !(0.0..=1.0).contains(&x.dependency) || x.exposure < 0.0 || x.lat.abs() > 90.0 {
            return Err(format!(
                "{w}: dependency must be 0-1, exposure not negative, latitude valid"
            ));
        }
        out.push(x);
    }
    inputs.push(rf);
    Ok(out)
}

/// Compute the accumulation. `path` is the request file.
pub fn accumulation(path: &Path) -> Result<(AccumulationResult, String), String> {
    let (r, src, base): (AccumulationRequest, _, _) = super::read_request(path)?;
    if r.loss_ratio_per_hour < 0.0 || !(0.0..=1.0).contains(&r.max_loss_ratio) || r.cell_deg <= 0.0
    {
        return Err(
            "loss_ratio_per_hour not negative, max_loss_ratio 0-1, cell_deg positive".into(),
        );
    }
    let (p0, p1) = (
        utc("period_start_utc", &r.period_start_utc)?,
        utc("period_end_utc", &r.period_end_utc)?,
    );
    if p1 <= p0 {
        return Err("the period is empty".into());
    }
    let mut inputs = vec![];
    let port = load_portfolio(&r.portfolio, &base, &mut inputs)?;
    let events = crate::briefings::load_events(&base, &r.events, &mut inputs)?;
    let ratio = |h: f64| (r.loss_ratio_per_hour * h).min(r.max_loss_ratio);
    let under = |lat: f64, lon: f64, rad: f64, h: f64| -> (usize, f64, f64) {
        port.iter()
            .filter(|x| distance_km(lat, lon, x.lat, x.lon) <= rad)
            .fold((0, 0.0, 0.0), |(n, e, l), x| {
                (
                    n + 1,
                    e + x.exposure,
                    l + x.exposure * x.dependency * ratio(h),
                )
            })
    };
    let mut footprints = vec![];
    let mut no_loc = 0;
    for e in &events {
        let Some(p) = &e.location else {
            no_loc += 1;
            continue;
        };
        let (a, b) = (utc(&e.id, &e.start_utc)?, utc(&e.id, &e.end_utc)?);
        if b < p0 || a > p1 {
            continue;
        }
        let rad = p.radius_m / 1000.0 + r.footprint_buffer_km;
        let h = (b - a) / 3600.0;
        let (n, ex, l) = under(p.lat_deg, p.lon_deg, rad, h);
        footprints.push(FootprintRow {
            id: e.id.clone(),
            source: "historical".into(),
            kind: format!("{:?}", e.kind).to_ascii_lowercase(),
            start_utc: Some(e.start_utc.clone()),
            radius_km: sig9(rad),
            hours: sig9(h),
            insureds: n,
            exposure_inside: sig9(ex),
            modelled_loss: sig9(l),
        });
    }
    let hist: Vec<f64> = footprints.iter().map(|x| x.modelled_loss).collect();
    for s in &r.scenario {
        let (n, ex, l) = under(s.lat_deg, s.lon_deg, s.radius_km, s.duration_h);
        footprints.push(FootprintRow {
            id: s.name.clone(),
            source: "scenario".into(),
            kind: "scenario".into(),
            start_utc: None,
            radius_km: s.radius_km,
            hours: s.duration_h,
            insureds: n,
            exposure_inside: sig9(ex),
            modelled_loss: sig9(l),
        });
    }
    let total: f64 = port.iter().map(|x| x.exposure).sum();
    let mut cells: BTreeMap<(i64, i64), (usize, f64)> = BTreeMap::new();
    for x in &port {
        let k = (
            (x.lat / r.cell_deg).floor() as i64,
            (x.lon / r.cell_deg).floor() as i64,
        );
        let c = cells.entry(k).or_default();
        c.0 += 1;
        c.1 += x.exposure;
    }
    let mut cells: Vec<CellRow> = cells
        .into_iter()
        .map(|((a, o), (n, e))| CellRow {
            cell: [a as f64 * r.cell_deg, o as f64 * r.cell_deg],
            insureds: n,
            exposure: sig9(e),
            share: if total > 0.0 { sig9(e / total) } else { 0.0 },
        })
        .collect();
    cells.sort_by(|a, b| b.exposure.total_cmp(&a.exposure));
    let years = (p1 - p0) / (365.25 * 86_400.0);
    let mut sorted = hist.clone();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let exceedance = sorted
        .iter()
        .enumerate()
        .map(|(i, l)| ExceedanceRow {
            rank: i + 1,
            loss: *l,
            empirical_return_period_y: sig9(years / (i + 1) as f64),
        })
        .collect();
    let agg: f64 = hist.iter().sum();
    let totals = AccTotals {
        insureds: port.len(),
        exposure: sig9(total),
        historical_events: hist.len(),
        events_without_location: no_loc,
        largest_event_loss: hist.iter().copied().fold(0.0, f64::max),
        period_aggregate_loss: sig9(agg),
        annualised_aggregate_loss: sig9(agg / years),
        top_cell_share: cells.first().map_or(0.0, |c| c.share),
        herfindahl: sig9(cells.iter().map(|c| c.share * c.share).sum()),
        largest_scenario_loss: footprints
            .iter()
            .filter(|x| x.source == "scenario")
            .map(|x| x.modelled_loss)
            .fold(0.0, f64::max),
    };
    let mut res = AccumulationResult {
        format: ACC_FORMAT.into(),
        method: ACC_METHOD.into(),
        label: ACC_LABEL.into(),
        statement: super::STATEMENT.into(),
        title: r.title.clone(),
        currency: r.currency.clone(),
        period_start_utc: r.period_start_utc.clone(),
        period_end_utc: r.period_end_utc.clone(),
        loss_ratio_per_hour: r.loss_ratio_per_hour,
        max_loss_ratio: r.max_loss_ratio,
        footprint_buffer_km: r.footprint_buffer_km,
        cell_deg: r.cell_deg,
        inputs,
        footprints,
        cells,
        exceedance,
        totals,
        fingerprint: String::new(),
    };
    res.fingerprint = fingerprint(&res);
    Ok((res, src))
}

/// The accumulation HTML page.
pub fn accumulation_html(r: &AccumulationResult) -> String {
    let t = &r.totals;
    let mut b = html_table(
        &["Measure", &format!("Value ({})", r.currency)],
        &[
            vec!["Insureds".into(), t.insureds.to_string()],
            vec!["Total exposure".into(), num(t.exposure)],
            vec!["Largest historical event".into(), num(t.largest_event_loss)],
            vec!["Period aggregate".into(), num(t.period_aggregate_loss)],
            vec![
                "Annualised aggregate".into(),
                num(t.annualised_aggregate_loss),
            ],
            vec!["Largest scenario".into(), num(t.largest_scenario_loss)],
            vec!["Top cell share".into(), num(t.top_cell_share)],
            vec!["Herfindahl index (cells)".into(), num(t.herfindahl)],
        ],
    );
    b.push_str("<h2>Footprints</h2>");
    b.push_str(&html_table(
        &[
            "Id",
            "Source",
            "Kind",
            "Radius km",
            "Hours",
            "Insureds",
            "Exposure inside",
            "Modelled loss",
        ],
        &r.footprints
            .iter()
            .map(|x| {
                vec![
                    x.id.clone(),
                    x.source.clone(),
                    x.kind.clone(),
                    num(x.radius_km),
                    num(x.hours),
                    x.insureds.to_string(),
                    num(x.exposure_inside),
                    num(x.modelled_loss),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    b.push_str("<h2>Cells</h2>");
    b.push_str(&html_table(
        &["Cell (SW corner)", "Insureds", "Exposure", "Share"],
        &r.cells
            .iter()
            .map(|c| {
                vec![
                    format!("{}, {}", c.cell[0], c.cell[1]),
                    c.insureds.to_string(),
                    num(c.exposure),
                    num(c.share),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    if t.events_without_location > 0 {
        b.push_str(&format!(
            "<p class=\"l\">{} event(s) without a location were left out.</p>",
            t.events_without_location
        ));
    }
    b.push_str(&format!(
        "<p class=\"l\">Fingerprint {}</p>",
        esc(&r.fingerprint)
    ));
    page(&r.title, &r.label, &b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_by_hand() {
        assert_eq!(ratio(&[(10.0, 10.0)]), Some(0.0));
        assert_eq!(ratio(&[(0.0, 10.0), (5.0, 0.0)]), Some(1.5));
        assert_eq!(ratio(&[(5.0, 0.0)]), None);
    }
}
