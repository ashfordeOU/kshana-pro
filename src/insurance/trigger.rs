// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Parametric PNT-outage trigger, evaluated on the user's own station data (B2801, local part)
//!
//! A parametric cover pays on an index, not on an adjusted loss. For GNSS the natural index is a
//! measured signal quantity at named monitoring sites. This module evaluates a trigger
//! definition on the user's own C/N0 series and reports what it would have done:
//!
//! * **Series**: CSV `utc,site,cn0_dbhz` (one site-level C/N0 value per epoch, for example the
//!   mean of the tracked satellites) or `kshana.obs/1` observation files (the epoch value is the
//!   mean C/N0 of the satellites tracked; the site is the header's station id).
//! * **Baseline** per site: the `baseline_dbhz` the request states, else the median of the site's
//!   samples in the period (unaffected while degraded time stays under half the period).
//! * **Degraded sample**: C/N0 below baseline minus `cn0_drop_db`. Missing data is never
//!   degraded: a gap longer than `max_gap_s` ends an episode (the conservative reading; it is
//!   listed as data coverage, because it is itself a source of basis risk).
//! * **Site episodes**: runs of degraded samples, merged across gaps up to `merge_gap_min`.
//! * **Trigger events**: the times when at least `min_sites` sites are in an episode at once,
//!   merged the same way; an event lasting `min_duration_min` or more **fires**.
//! * **Outage index**: per event, the sum over its sites of (baseline − C/N0) × time, in dB·h;
//!   per period, degraded site-hours over observed site-hours.
//! * **Modelled payout**: the highest tier of the cover schedule the event's duration reaches,
//!   capped per event and in aggregate. A calculation under the stated terms, never a claim
//!   decision.
//! * **Backtest**: each archived event the request lists (with its documented basis) is matched
//!   against the fired events; unmatched fired events are listed.
//!
//! A hosted oracle would add independently operated stations, a published and signed index, and
//! a neutral record of the trigger's state; this command works only on the data supplied.
//! Evidence: **MODELLED**.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{esc, fingerprint, html_table, iso, num, page, read_input, sig9, utc, InputRef};

pub const FORMAT: &str = "kshana-pro-parametric-trigger/1";
pub const METHOD_ID: &str = "kshana-pro/parametric-trigger/v1";
pub const RESULT_FILE: &str = "trigger.json";
pub const LABEL: &str = "MODELLED: a trigger evaluated on the station series supplied under the \
stated definition; payouts are calculations under the stated cover terms, not claim decisions.";

/// A trigger request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerRequest {
    pub title: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    /// CSV `utc,site,cn0_dbhz` files or `kshana.obs/1` files (`.jsonl`).
    pub series: Vec<String>,
    #[serde(default)]
    pub sites: Vec<SiteSpec>,
    pub trigger: TriggerDef,
    #[serde(default)]
    pub cover: Option<Cover>,
    #[serde(default)]
    pub backtest: Vec<ArchivedEvent>,
}

/// A monitoring site: optional position (for spatial basis risk) and a fixed baseline.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SiteSpec {
    pub id: String,
    #[serde(default)]
    pub lat_deg: Option<f64>,
    #[serde(default)]
    pub lon_deg: Option<f64>,
    #[serde(default)]
    pub baseline_dbhz: Option<f64>,
}

fn d_min_sites() -> usize {
    1
}
fn d_gap() -> f64 {
    120.0
}
fn d_merge() -> f64 {
    10.0
}

/// The trigger definition.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerDef {
    pub cn0_drop_db: f64,
    #[serde(default = "d_min_sites")]
    pub min_sites: usize,
    pub min_duration_min: f64,
    #[serde(default = "d_gap")]
    pub max_gap_s: f64,
    #[serde(default = "d_merge")]
    pub merge_gap_min: f64,
}

/// A payout tier: events lasting at least `min_hours` pay `amount`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PayTier {
    pub min_hours: f64,
    pub amount: f64,
}

/// The cover schedule. `synthetic` must be stated: true for an illustrative schedule.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cover {
    pub name: String,
    pub currency: String,
    pub synthetic: bool,
    pub tiers: Vec<PayTier>,
    #[serde(default)]
    pub per_event_limit: Option<f64>,
    #[serde(default)]
    pub aggregate_limit: Option<f64>,
}

/// An archived event to backtest against, with the basis it is documented on.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchivedEvent {
    pub id: String,
    pub start_utc: String,
    pub end_utc: String,
    pub basis: String,
}

/// One site's summary.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SiteRow {
    pub id: String,
    pub samples: usize,
    pub baseline_dbhz: f64,
    /// `stated` or `median`.
    pub baseline_source: String,
    pub observed_h: f64,
    pub degraded_h: f64,
    pub episodes: usize,
}

/// One site episode.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode {
    pub site: String,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_min: f64,
    pub max_drop_db: f64,
    pub index_db_h: f64,
    #[serde(skip)]
    pub t0: f64,
    #[serde(skip)]
    pub t1: f64,
}

/// One trigger event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriggerEvent {
    pub id: String,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_h: f64,
    pub sites: Vec<String>,
    pub index_db_h: f64,
    pub fired: bool,
    /// The modelled payout under the cover (0 when not fired or no cover).
    pub payout: f64,
    #[serde(skip)]
    pub t0: f64,
    #[serde(skip)]
    pub t1: f64,
}

/// One backtest row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktestRow {
    pub id: String,
    pub start_utc: String,
    pub end_utc: String,
    pub basis: String,
    pub fired: bool,
    pub matched_events: Vec<String>,
}

/// Totals.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Totals {
    pub events: usize,
    pub fired: usize,
    pub total_payout: f64,
    pub period_outage_index: f64,
    pub archived_events: usize,
    pub archived_hit: usize,
    pub fired_unmatched: usize,
}

/// A trigger evaluation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriggerResult {
    pub format: String,
    pub method: String,
    pub label: String,
    pub statement: String,
    pub title: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub trigger: TriggerDef,
    pub cover: Option<Cover>,
    pub inputs: Vec<InputRef>,
    pub sites: Vec<SiteRow>,
    pub episodes: Vec<Episode>,
    pub events: Vec<TriggerEvent>,
    pub backtest: Vec<BacktestRow>,
    pub totals: Totals,
    pub notes: Vec<String>,
    pub fingerprint: String,
}

type Series = BTreeMap<String, Vec<(f64, f64)>>;

fn load_csv(text: &str, name: &str, out: &mut Series) -> Result<(), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or_else(|| format!("{name}: empty"))?
        .split(',')
        .map(|c| c.trim().to_ascii_lowercase())
        .collect();
    let col = |c: &str| {
        head.iter()
            .position(|h| h == c)
            .ok_or_else(|| format!("{name}: no '{c}' column"))
    };
    let (cu, cs, cc) = (col("utc")?, col("site")?, col("cn0_dbhz")?);
    for (i, l) in lines.enumerate() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let g = |k: usize| f.get(k).copied().unwrap_or("");
        let t = utc(&format!("{name} row {}", i + 2), g(cu))?;
        let v: f64 = g(cc)
            .parse()
            .map_err(|_| format!("{name} row {}: cn0_dbhz is not a number", i + 2))?;
        if g(cs).is_empty() {
            return Err(format!("{name} row {}: empty site", i + 2));
        }
        out.entry(g(cs).to_string()).or_default().push((t, v));
    }
    Ok(())
}

fn load_obs(text: &str, name: &str, out: &mut Series) -> Result<(), String> {
    let (records, _) = kshana::obs::check(text).map_err(|e| format!("{name}: {e}"))?;
    let mut site = String::new();
    let mut start: Option<f64> = None;
    for r in records {
        match r {
            kshana::obs::Record::Header(h) => {
                site = h.station.id.clone();
                start = h.start_label.as_deref().and_then(|s| utc("", s).ok());
            }
            kshana::obs::Record::Epoch(e) => {
                let ep = &e.epoch;
                let t = ep
                    .time_label
                    .as_deref()
                    .and_then(|s| utc("", s).ok())
                    .or(start.map(|s| s + ep.t_s));
                let Some(t) = t else {
                    return Err(format!("{name}: epochs carry no absolute time"));
                };
                if ep.cn0.is_empty() {
                    continue;
                }
                let m = ep.cn0.iter().map(|c| c.cn0_dbhz).sum::<f64>() / ep.cn0.len() as f64;
                out.entry(site.clone()).or_default().push((t, m));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Read the request's series files.
pub fn load_series(
    r: &TriggerRequest,
    base: &Option<PathBuf>,
    inputs: &mut Vec<InputRef>,
) -> Result<Series, String> {
    let mut s = Series::new();
    for n in &r.series {
        let (b, rf) = read_input(base, n, "series")?;
        let text = String::from_utf8(b).map_err(|_| format!("{n}: not UTF-8"))?;
        if n.ends_with(".jsonl") || n.ends_with(".json") {
            load_obs(&text, n, &mut s)?;
        } else {
            load_csv(&text, n, &mut s)?;
        }
        inputs.push(rf);
    }
    for v in s.values_mut() {
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    Ok(s)
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Merge `[a, b]` intervals whose gap is at most `gap` seconds (input sorted by start).
fn merge(iv: Vec<(f64, f64)>, gap: f64) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = vec![];
    for (a, b) in iv {
        match out.last_mut() {
            Some(l) if a - l.1 <= gap => l.1 = l.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// The payout of an event of `hours` under `c` before the aggregate limit.
pub fn tier_payout(c: &Cover, hours: f64) -> f64 {
    let p = c
        .tiers
        .iter()
        .filter(|t| hours + 1e-9 >= t.min_hours)
        .map(|t| t.amount)
        .fold(0.0, f64::max);
    c.per_event_limit.map_or(p, |l| p.min(l))
}

/// Evaluate the trigger. `base` resolves relative file names.
pub fn evaluate(r: &TriggerRequest, base: &Option<PathBuf>) -> Result<TriggerResult, String> {
    let (p0, p1) = (
        utc("period_start_utc", &r.period_start_utc)?,
        utc("period_end_utc", &r.period_end_utc)?,
    );
    if p1 <= p0 {
        return Err("the period is empty".into());
    }
    let d = &r.trigger;
    if d.cn0_drop_db <= 0.0 || d.min_duration_min < 0.0 || d.max_gap_s <= 0.0 || d.min_sites == 0 {
        return Err("trigger: cn0_drop_db and max_gap_s must be positive, min_sites at least 1, min_duration_min not negative".into());
    }
    if let Some(c) = &r.cover {
        if c.tiers.is_empty() || c.tiers.iter().any(|t| t.amount < 0.0 || t.min_hours < 0.0) {
            return Err("cover: at least one tier, amounts and hours not negative".into());
        }
    }
    let mut inputs = vec![];
    let series = load_series(r, base, &mut inputs)?;
    if series.is_empty() {
        return Err("the series hold no samples".into());
    }
    let stated: BTreeMap<&str, f64> = r
        .sites
        .iter()
        .filter_map(|s| s.baseline_dbhz.map(|b| (s.id.as_str(), b)))
        .collect();
    let merge_gap = d.merge_gap_min * 60.0;
    let mut sites = vec![];
    let mut episodes = vec![];
    let mut notes = vec![];
    for (id, all) in &series {
        let v: Vec<(f64, f64)> = all
            .iter()
            .copied()
            .filter(|(t, _)| *t >= p0 && *t <= p1)
            .collect();
        if v.is_empty() {
            notes.push(format!("site {id}: no samples in the period"));
            continue;
        }
        let (baseline, src) = match stated.get(id.as_str()) {
            Some(b) => (*b, "stated"),
            None => (
                median(&mut v.iter().map(|x| x.1).collect::<Vec<_>>()),
                "median",
            ),
        };
        let thr = baseline - d.cn0_drop_db;
        let (mut observed, mut degraded) = (0.0, 0.0);
        let mut runs: Vec<(f64, f64, f64, f64)> = vec![]; // start, end, max drop, dB·s
        let mut cur: Option<(f64, f64, f64, f64)> = None;
        for i in 0..v.len() {
            let (t, c) = v[i];
            let dt = v
                .get(i + 1)
                .map(|n| n.0 - t)
                .filter(|g| *g <= d.max_gap_s)
                .unwrap_or(0.0);
            observed += dt;
            let gap_before = i > 0 && t - v[i - 1].0 > d.max_gap_s;
            if gap_before {
                if let Some(c) = cur.take() {
                    runs.push(c);
                }
            }
            if c < thr {
                degraded += dt;
                let drop = baseline - c;
                cur = Some(match cur {
                    Some((a, _, m, s)) => (a, t + dt, m.max(drop), s + drop * dt),
                    None => (t, t + dt, drop, drop * dt),
                });
            } else if let Some(c) = cur.take() {
                runs.push(c);
            }
        }
        if let Some(c) = cur.take() {
            runs.push(c);
        }
        // Merge runs closer than the merge gap.
        let mut merged: Vec<(f64, f64, f64, f64)> = vec![];
        for x in runs {
            match merged.last_mut() {
                Some(l) if x.0 - l.1 <= merge_gap => {
                    l.1 = x.1;
                    l.2 = l.2.max(x.2);
                    l.3 += x.3;
                }
                _ => merged.push(x),
            }
        }
        sites.push(SiteRow {
            id: id.clone(),
            samples: v.len(),
            baseline_dbhz: sig9(baseline),
            baseline_source: src.into(),
            observed_h: sig9(observed / 3600.0),
            degraded_h: sig9(degraded / 3600.0),
            episodes: merged.len(),
        });
        for (a, b, m, s) in merged {
            episodes.push(Episode {
                site: id.clone(),
                start_utc: iso(a),
                end_utc: iso(b),
                duration_min: sig9((b - a) / 60.0),
                max_drop_db: sig9(m),
                index_db_h: sig9(s / 3600.0),
                t0: a,
                t1: b,
            });
        }
    }
    // k-of-n sweep over the site episodes.
    let mut edges: Vec<(f64, i32)> = episodes
        .iter()
        .flat_map(|e| [(e.t0, 1), (e.t1, -1)])
        .collect();
    edges.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let (mut active, mut open) = (0i32, None);
    let mut raw = vec![];
    for (t, s) in edges {
        active += s;
        if active as usize >= d.min_sites && open.is_none() {
            open = Some(t);
        } else if (active as usize) < d.min_sites {
            if let Some(a) = open.take() {
                raw.push((a, t));
            }
        }
    }
    let mut events = vec![];
    let mut paid = 0.0;
    for (k, (a, b)) in merge(raw, merge_gap).into_iter().enumerate() {
        let over: Vec<&Episode> = episodes.iter().filter(|e| e.t0 <= b && e.t1 >= a).collect();
        let mut s: Vec<String> = over.iter().map(|e| e.site.clone()).collect();
        s.sort();
        s.dedup();
        let hours = (b - a) / 3600.0;
        let fired = hours * 60.0 + 1e-9 >= d.min_duration_min;
        let mut payout = match (&r.cover, fired) {
            (Some(c), true) => tier_payout(c, hours),
            _ => 0.0,
        };
        if let Some(l) = r.cover.as_ref().and_then(|c| c.aggregate_limit) {
            payout = payout.min((l - paid).max(0.0));
        }
        paid += payout;
        events.push(TriggerEvent {
            id: format!("E{:03}", k + 1),
            start_utc: iso(a),
            end_utc: iso(b),
            duration_h: sig9(hours),
            sites: s,
            index_db_h: sig9(over.iter().map(|e| e.index_db_h).sum()),
            fired,
            payout: sig9(payout),
            t0: a,
            t1: b,
        });
    }
    let mut backtest = vec![];
    let mut matched_ids = std::collections::BTreeSet::new();
    for a in &r.backtest {
        let (s, e) = (
            utc(&format!("backtest {}", a.id), &a.start_utc)?,
            utc(&format!("backtest {}", a.id), &a.end_utc)?,
        );
        let m: Vec<String> = events
            .iter()
            .filter(|x| x.fired && x.t0 <= e + merge_gap && x.t1 >= s - merge_gap)
            .map(|x| x.id.clone())
            .collect();
        matched_ids.extend(m.iter().cloned());
        backtest.push(BacktestRow {
            id: a.id.clone(),
            start_utc: a.start_utc.clone(),
            end_utc: a.end_utc.clone(),
            basis: a.basis.clone(),
            fired: !m.is_empty(),
            matched_events: m,
        });
    }
    let (obs, deg): (f64, f64) = sites
        .iter()
        .fold((0.0, 0.0), |(o, g), s| (o + s.observed_h, g + s.degraded_h));
    if let Some(c) = &r.cover {
        if c.synthetic {
            notes.push(format!(
                "cover '{}' is a SYNTHETIC illustrative schedule, not a real policy",
                c.name
            ));
        }
    }
    notes.push(
        "Missing data never counts as degraded; gaps longer than max_gap_s end an episode.".into(),
    );
    let fired = events.iter().filter(|e| e.fired).count();
    let totals = Totals {
        events: events.len(),
        fired,
        total_payout: sig9(paid),
        period_outage_index: if obs > 0.0 { sig9(deg / obs) } else { 0.0 },
        archived_events: backtest.len(),
        archived_hit: backtest.iter().filter(|b| b.fired).count(),
        fired_unmatched: if backtest.is_empty() {
            0
        } else {
            events
                .iter()
                .filter(|e| e.fired && !matched_ids.contains(&e.id))
                .count()
        },
    };
    let mut res = TriggerResult {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        label: LABEL.into(),
        statement: super::STATEMENT.into(),
        title: r.title.clone(),
        period_start_utc: r.period_start_utc.clone(),
        period_end_utc: r.period_end_utc.clone(),
        trigger: r.trigger.clone(),
        cover: r.cover.clone(),
        inputs,
        sites,
        episodes,
        events,
        backtest,
        totals,
        notes,
        fingerprint: String::new(),
    };
    res.fingerprint = fingerprint(&res);
    Ok(res)
}

/// The trigger's HTML page.
pub fn render_html(r: &TriggerResult) -> String {
    let mut b = format!(
        "<p>Period {} to {}. Trigger: C/N0 at least {} dB below baseline at {} or more site(s) for {} min or more.</p>",
        esc(&r.period_start_utc),
        esc(&r.period_end_utc),
        r.trigger.cn0_drop_db,
        r.trigger.min_sites,
        r.trigger.min_duration_min
    );
    b.push_str("<h2>Sites</h2>");
    b.push_str(&html_table(
        &[
            "Site",
            "Samples",
            "Baseline dB-Hz",
            "Source",
            "Observed h",
            "Degraded h",
            "Episodes",
        ],
        &r.sites
            .iter()
            .map(|s| {
                vec![
                    s.id.clone(),
                    s.samples.to_string(),
                    num(s.baseline_dbhz),
                    s.baseline_source.clone(),
                    num(s.observed_h),
                    num(s.degraded_h),
                    s.episodes.to_string(),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    b.push_str("<h2>Trigger events</h2>");
    b.push_str(&html_table(
        &[
            "Event",
            "Start",
            "End",
            "Hours",
            "Sites",
            "Index dB-h",
            "Fired",
            "Modelled payout",
        ],
        &r.events
            .iter()
            .map(|e| {
                vec![
                    e.id.clone(),
                    e.start_utc.clone(),
                    e.end_utc.clone(),
                    num(e.duration_h),
                    e.sites.join(" "),
                    num(e.index_db_h),
                    if e.fired { "yes" } else { "no" }.into(),
                    num(e.payout),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    if !r.backtest.is_empty() {
        b.push_str("<h2>Backtest</h2>");
        b.push_str(&html_table(
            &["Archived event", "Start", "End", "Basis", "Fired", "Events"],
            &r.backtest
                .iter()
                .map(|x| {
                    vec![
                        x.id.clone(),
                        x.start_utc.clone(),
                        x.end_utc.clone(),
                        x.basis.clone(),
                        if x.fired { "yes" } else { "no" }.into(),
                        x.matched_events.join(" "),
                    ]
                })
                .collect::<Vec<_>>(),
        ));
    }
    for n in &r.notes {
        b.push_str(&format!("<p class=\"l\">{}</p>", esc(n)));
    }
    b.push_str(&format!(
        "<p class=\"l\">Fingerprint {}</p>",
        esc(&r.fingerprint)
    ));
    page(&r.title, &r.label, &b)
}

/// Write the result directory.
pub fn write_dir(out: &Path, r: &TriggerResult, request_src: &str) -> Result<(), String> {
    super::write_pack(out, RESULT_FILE, r, &render_html(r), request_src)
}
