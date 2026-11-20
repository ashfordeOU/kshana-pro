// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # gNB timing-alarm adapter (backlog B3004)
//!
//! Reads a gNB or network-management alarm export in a neutral CSV
//! (`timestamp,gnb_id,alarm,state[,severity]`, `state` = `raise`/`clear`) and maps the vendor's
//! alarm names to timing classes through a table **the user supplies** (`[map]`; Kshana bakes in
//! no vendor's names). From the mapped alarms it builds per-gNB episodes and puts them against
//! the TDD timing model of [`super::nr::tdd`] (holdover bound against the ±1.5 µs phase budget
//! and the guard-period interference onset):
//!
//! * per GNSS-loss or holdover episode: its duration, the modelled time at which the phase
//!   budget is passed, whether the episode outlasted it and by how much, and whether the gNB
//!   raised its own phase alarm before or after the modelled crossing;
//! * per declared neighbour pair: concurrent holdover longer than the modelled interference
//!   onset (two gNBs drifting apart use up the guard period);
//! * alarm names not in the map are listed (an interface-change signal), never guessed.
//!
//! **MODELLED**: the crossing times come from the stated holdover model, not from measured time
//! error; the adapter does not read or control any gNB.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::nr::{self, TddReport, TddRequest};
use super::Table;
use crate::forensics::inputs::{iso, parse_utc};

/// Format tag.
pub const FORMAT: &str = "kshana-pro-gnb-timing-alarms/1";

/// Timing class of an alarm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// GNSS reference lost (the gNB enters holdover).
    GnssLoss,
    /// The gNB reports it is in holdover.
    Holdover,
    /// The gNB reports its holdover limit exceeded.
    HoldoverExceeded,
    /// PTP reference lost.
    PtpLoss,
    /// The gNB's own phase or time-error alarm.
    PhaseAlarm,
    /// Not a timing alarm.
    Ignore,
}

/// The request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// The alarm CSV.
    pub alarms: String,
    /// Vendor alarm name to class.
    pub map: BTreeMap<String, Class>,
    /// The TDD timing model shared by the gNBs.
    pub tdd: TddRequest,
    /// Neighbour pairs `[a, b]` for the interference check.
    #[serde(default)]
    pub neighbours: Vec<[String; 2]>,
}

/// One episode.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Episode {
    pub gnb: String,
    pub class: Class,
    pub start_utc: String,
    pub end_utc: String,
    /// Still raised at the end of the file.
    pub open: bool,
    pub duration_s: f64,
    #[serde(skip)]
    pub t0: f64,
    #[serde(skip)]
    pub t1: f64,
}

/// A holdover episode against the model.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HoldoverCheck {
    pub gnb: String,
    pub start_utc: String,
    pub duration_s: f64,
    /// Modelled time of passing the phase budget (`None` beyond the model horizon).
    pub modelled_budget_utc: Option<String>,
    pub beyond_budget_s: f64,
    /// First own phase alarm in the episode, and its time minus the modelled crossing (s).
    pub own_phase_alarm_utc: Option<String>,
    pub own_alarm_minus_model_s: Option<f64>,
}

/// A neighbour pair in concurrent holdover.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PairRisk {
    pub a: String,
    pub b: String,
    pub overlap_start_utc: String,
    pub overlap_s: f64,
    /// Modelled onset of downlink-to-uplink interference.
    pub interference_onset_utc: Option<String>,
    pub at_risk: bool,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub model: TddReport,
    pub rows: usize,
    pub unmapped: BTreeMap<String, usize>,
    pub episodes: Vec<Episode>,
    pub holdover: Vec<HoldoverCheck>,
    pub pairs: Vec<PairRisk>,
    pub verdict: String,
}

/// Episodes, unmapped alarm names with their counts, and the number of rows.
pub type Parsed = (Vec<Episode>, BTreeMap<String, usize>, usize);

/// Build episodes from the alarm table.
pub fn episodes(t: &Table, map: &BTreeMap<String, Class>) -> Result<Parsed, String> {
    let (ct, cg, ca, cs) = (
        t.need("timestamp")?,
        t.need("gnb_id")?,
        t.need("alarm")?,
        t.need("state")?,
    );
    let mut ev: Vec<(f64, String, Class, bool)> = Vec::new();
    let mut unmapped: BTreeMap<String, usize> = BTreeMap::new();
    for i in 0..t.rows.len() {
        let ts = t.text(i, Some(ct));
        let tt = parse_utc(ts).ok_or_else(|| format!("row {}: bad timestamp '{ts}'", i + 2))?;
        let name = t.text(i, Some(ca));
        let Some(&c) = map.get(name) else {
            *unmapped.entry(name.to_string()).or_insert(0) += 1;
            continue;
        };
        if c == Class::Ignore {
            continue;
        }
        let on = match t.text(i, Some(cs)).to_ascii_lowercase().as_str() {
            "raise" | "raised" | "set" | "on" | "active" => true,
            "clear" | "cleared" | "off" | "inactive" => false,
            o => return Err(format!("row {}: state '{o}' is not raise or clear", i + 2)),
        };
        ev.push((tt, t.text(i, Some(cg)).to_string(), c, on));
    }
    ev.sort_by(|a, b| a.0.total_cmp(&b.0));
    let end = ev.last().map(|e| e.0).unwrap_or(0.0);
    let mut open: BTreeMap<(String, Class), f64> = BTreeMap::new();
    let mut out = Vec::new();
    let mk = |g: &str, c, t0: f64, t1: f64, open| Episode {
        gnb: g.to_string(),
        class: c,
        start_utc: iso(t0),
        end_utc: iso(t1),
        open,
        duration_s: t1 - t0,
        t0,
        t1,
    };
    for (tt, g, c, on) in ev {
        let k = (g.clone(), c);
        if on {
            open.entry(k).or_insert(tt);
        } else if let Some(t0) = open.remove(&k) {
            out.push(mk(&g, c, t0, tt, false));
        }
    }
    for ((g, c), t0) in open {
        out.push(mk(&g, c, t0, end, true));
    }
    out.sort_by(|a, b| a.t0.total_cmp(&b.t0).then(a.gnb.cmp(&b.gnb)));
    Ok((out, unmapped, t.rows.len()))
}

/// Run the adapter on a request whose alarm file is relative to `base`.
pub fn run(q: &Request, base: &std::path::Path, utc: &str) -> Result<Report, String> {
    let text = crate::timing::grid::read_text(base, &q.alarms)?;
    let t = Table::parse(&text).map_err(|e| format!("{}: {e}", q.alarms))?;
    let (eps, unmapped, rows) = episodes(&t, &q.map).map_err(|e| format!("{}: {e}", q.alarms))?;
    let model = nr::tdd(&q.tdd, utc)?;
    let is_hold = |e: &&Episode| matches!(e.class, Class::GnssLoss | Class::Holdover);
    // Merge GNSS-loss and holdover episodes per gNB into holdover intervals.
    let mut hold: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    for e in eps.iter().filter(is_hold) {
        let v = hold.entry(e.gnb.clone()).or_default();
        match v.last_mut() {
            Some(l) if e.t0 <= l.1 => l.1 = l.1.max(e.t1),
            _ => v.push((e.t0, e.t1)),
        }
    }
    let mut checks = Vec::new();
    for (g, v) in &hold {
        for &(a, b) in v {
            let cross = model.budget_exceeded_after_s.map(|s| a + s);
            let own = eps
                .iter()
                .filter(|e| &e.gnb == g && e.class == Class::PhaseAlarm && e.t0 >= a && e.t0 <= b)
                .map(|e| e.t0)
                .reduce(f64::min);
            checks.push(HoldoverCheck {
                gnb: g.clone(),
                start_utc: iso(a),
                duration_s: b - a,
                modelled_budget_utc: cross.map(iso),
                beyond_budget_s: cross.map(|c| (b - c).max(0.0)).unwrap_or(0.0),
                own_phase_alarm_utc: own.map(iso),
                own_alarm_minus_model_s: match (own, cross) {
                    (Some(o), Some(c)) => Some(o - c),
                    _ => None,
                },
            });
        }
    }
    let mut pairs = Vec::new();
    for [a, b] in &q.neighbours {
        let (Some(va), Some(vb)) = (hold.get(a), hold.get(b)) else {
            continue;
        };
        for x in va {
            for y in vb {
                let (s, e) = (x.0.max(y.0), x.1.min(y.1));
                if e <= s {
                    continue;
                }
                // Both drift from their own loss; the later loss sets the pair's clock.
                let onset = model.interference_after_s.map(|d| s + d);
                pairs.push(PairRisk {
                    a: a.clone(),
                    b: b.clone(),
                    overlap_start_utc: iso(s),
                    overlap_s: e - s,
                    interference_onset_utc: onset.map(iso),
                    at_risk: onset.is_some_and(|o| o < e),
                });
            }
        }
    }
    let beyond = checks.iter().filter(|c| c.beyond_budget_s > 0.0).count();
    let verdict = format!(
        "{} holdover episode(s) on {} gNB(s); {} outlasted the modelled phase budget; {} neighbour pair overlap(s) beyond the modelled interference onset; {} unmapped alarm name(s). MODELLED crossing times.",
        checks.len(),
        hold.len(),
        beyond,
        pairs.iter().filter(|p| p.at_risk).count(),
        unmapped.len()
    );
    Ok(Report {
        format: FORMAT,
        label: nr::LABEL,
        title: q.title.clone(),
        model,
        rows,
        unmapped,
        episodes: eps,
        holdover: checks,
        pairs,
        verdict,
    })
}
