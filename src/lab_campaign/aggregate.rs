//! Selection and ordering of the engine's per-cell numbers for the report (step L2).
//!
//! Nothing here computes a metric. Each value the report prints is either a field copied
//! from a cell or one of the declared aggregates in [`Op`] (a median, a sum, a count or a
//! ratio of sums) over cell fields. The aggregate carries its operation and the number of
//! inputs, so the report can say "median over 4 satellites" next to every derived figure.
//!
//! Scorecard metrics are whole-run metrics per (front-end chain, loop design). Event metrics
//! are aggregated per stated event, never across events: a jamming ramp and an outage gap
//! do not share a median.

use super::load::LoadedCampaign;
use super::model::{CampaignCell, EventResult};
use std::collections::BTreeMap;

/// How a printed value was obtained from the cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Median over the inputs (the mean of the middle two for an even count).
    Median,
    /// Sum over the inputs.
    Sum,
    /// Count of inputs meeting a condition.
    Count,
    /// Sum of numerators over sum of denominators (e.g. false-lock episodes per locked hour).
    RatioOfSums,
}

impl Op {
    pub fn describe(self) -> &'static str {
        match self {
            Op::Median => "median",
            Op::Sum => "sum",
            Op::Count => "count",
            Op::RatioOfSums => "ratio of sums",
        }
    }
}

/// A derived value: `None` when no input had the field (e.g. every satellite held lock).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stat {
    pub value: Option<f64>,
    pub op: Op,
    /// Inputs that contributed.
    pub n: usize,
}

/// Median of the finite values, or `None` if there are none.
pub fn median(values: impl IntoIterator<Item = f64>) -> Stat {
    let mut v: Vec<f64> = values.into_iter().filter(|x| x.is_finite()).collect();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let value = match n {
        0 => None,
        _ if n % 2 == 1 => Some(v[n / 2]),
        _ => Some(0.5 * (v[n / 2 - 1] + v[n / 2])),
    };
    Stat {
        value,
        op: Op::Median,
        n,
    }
}

/// Whole-run scorecard of one (chain, design) over every recording and satellite.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreRow {
    pub frontend: String,
    pub design: String,
    pub cells: usize,
    pub satellites: usize,
    pub availability: Stat,
    pub phase_lock_frac: Stat,
    pub pll_jitter_deg: Stat,
    pub dll_jitter_chips: Stat,
    /// Episodes over locked hours, summed across satellites.
    pub false_lock_per_locked_hour: Stat,
    pub false_lock_episodes: Stat,
    pub losses: Stat,
    /// The engine's whole-run verdicts against the client's bars: (passed, evaluated).
    pub verdicts: (usize, usize),
    /// Bars that failed, with how many rows failed each.
    pub failed_bars: BTreeMap<String, usize>,
}

/// One stated event, for one (chain, design), over the satellites it affected.
#[derive(Debug, Clone, PartialEq)]
pub struct EventRow {
    pub recording: String,
    pub event_id: String,
    /// `interference`, `spoofing`, `outage` or `other`.
    pub kind: String,
    /// The lab's stated type label.
    pub event_type: String,
    pub frontend: String,
    pub design: String,
    pub satellites: usize,
    /// Satellites that lost lock during the event.
    pub lost: Stat,
    /// Satellites that held lock throughout.
    pub held: usize,
    pub time_to_loss_s: Stat,
    pub js_at_loss_db: Stat,
    /// Satellites that re-acquired within the grace window.
    pub reacquired: Stat,
    pub reacq_time_s: Stat,
    pub availability: Stat,
    /// The engine's verdicts for this event against the client's bars: (passed, evaluated).
    pub verdicts: (usize, usize),
    /// Bars that failed, with how many satellites failed each.
    pub failed_bars: BTreeMap<String, usize>,
}

/// One J/S bin of an event's curve, for one (chain, design), across satellites.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveBin {
    pub js_db: f64,
    /// Locked epochs in the bin, summed over satellites.
    pub n: Stat,
    pub measured_degradation_db: Stat,
    pub modelled_degradation_db: Stat,
}

/// An event's C/N0-vs-J/S curve for one (chain, design).
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub recording: String,
    pub event_id: String,
    pub frontend: String,
    pub design: String,
    pub bins: Vec<CurveBin>,
    /// The engine's Q and formula for the MODELLED reference (identical across satellites).
    pub q: Option<f64>,
    pub formula: Option<String>,
}

/// Everything the report renders, in the campaign's own order.
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregates {
    /// Chain names in campaign order.
    pub frontends: Vec<String>,
    /// Design names in campaign order (colour follows the design, never its rank).
    pub designs: Vec<String>,
    pub scorecard: Vec<ScoreRow>,
    pub events: Vec<EventRow>,
    pub curves: Vec<Curve>,
    /// Per-recording bars that are looser than the campaign's, or that cannot be compared.
    pub relaxed: Vec<RelaxedBar>,
}

/// A recording's own bar (its test conditions' `bars` table) that is looser than the campaign
/// bar of the same name, or that this report cannot rank against it. The engine lets a recording
/// override a campaign bar; the report shows every such override beside the recording's rows.
#[derive(Debug, Clone, PartialEq)]
pub struct RelaxedBar {
    pub recording: String,
    pub bar: String,
    /// The campaign's value for the bar, as the engine wrote it.
    pub campaign: serde_json::Value,
    /// The recording's value.
    pub recording_value: serde_json::Value,
    /// `false` when the direction is unknown (name not `min_*`/`max_*`) or a value is not a
    /// number: shown as "cannot be compared", never silently accepted.
    pub comparable: bool,
}

impl RelaxedBar {
    /// One line for the HTML and the PDF.
    pub fn describe(&self) -> String {
        if self.comparable {
            format!(
                "recording {}: bar {} = {} is looser than the campaign's {}",
                self.recording, self.bar, self.recording_value, self.campaign
            )
        } else {
            format!(
                "recording {}: bar {} = {} differs from the campaign's {} and cannot be ranked as stricter or looser",
                self.recording, self.bar, self.recording_value, self.campaign
            )
        }
    }
}

/// The per-recording bars that relax a campaign bar. A bar the campaign does not state is an
/// extra bar, not a relaxation. `min_*` bars are looser when smaller, `max_*` when larger.
pub fn relaxed_bars(m: &super::model::CampaignManifest) -> Vec<RelaxedBar> {
    let Some(camp) = m.scoring.get("bars").and_then(|b| b.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for r in &m.recordings {
        let Some(own) = r.conditions.get("bars").and_then(|b| b.as_object()) else {
            continue;
        };
        for (name, v) in own {
            let Some(cv) = camp.get(name) else { continue };
            if cv == v {
                continue;
            }
            let looser = match (cv.as_f64(), v.as_f64()) {
                (Some(c), Some(x)) if name.starts_with("min_") => Some(x < c),
                (Some(c), Some(x)) if name.starts_with("max_") => Some(x > c),
                _ => None,
            };
            if looser != Some(false) {
                out.push(RelaxedBar {
                    recording: r.id.clone(),
                    bar: name.clone(),
                    campaign: cv.clone(),
                    recording_value: v.clone(),
                    comparable: looser.is_some(),
                });
            }
        }
    }
    out
}

fn cells_for<'a>(c: &'a LoadedCampaign, fe: &str, de: &str) -> Vec<&'a CampaignCell> {
    c.cells
        .iter()
        .filter(|x| x.frontend.name == fe && x.design.name == de)
        .collect()
}

fn opt(it: impl IntoIterator<Item = Option<f64>>) -> Stat {
    median(it.into_iter().flatten())
}

fn count(n: usize, of: usize) -> Stat {
    Stat {
        value: Some(n as f64),
        op: Op::Count,
        n: of,
    }
}

fn score_row(fe: &str, de: &str, cells: &[&CampaignCell]) -> ScoreRow {
    let sats: Vec<_> = cells.iter().flat_map(|c| &c.satellites).collect();
    let w =
        |f: fn(&super::model::WholeRun) -> Option<f64>| opt(sats.iter().map(|s| f(&s.whole_run)));
    let episodes: u64 = sats.iter().map(|s| s.whole_run.false_lock_episodes).sum();
    let locked_h: f64 = sats.iter().map(|s| s.whole_run.locked_s).sum::<f64>() / 3600.0;
    let losses: u64 = sats.iter().map(|s| s.whole_run.loss_count).sum();
    ScoreRow {
        frontend: fe.to_owned(),
        design: de.to_owned(),
        cells: cells.len(),
        satellites: sats.len(),
        availability: w(|r| r.availability),
        phase_lock_frac: w(|r| r.phase_lock_frac),
        pll_jitter_deg: opt(sats.iter().map(|s| s.whole_run.pll_jitter_deg)),
        dll_jitter_chips: opt(sats.iter().map(|s| s.whole_run.dll_jitter_chips)),
        false_lock_per_locked_hour: Stat {
            value: (locked_h > 0.0).then(|| episodes as f64 / locked_h),
            op: Op::RatioOfSums,
            n: sats.len(),
        },
        false_lock_episodes: Stat {
            value: Some(episodes as f64),
            op: Op::Sum,
            n: sats.len(),
        },
        losses: Stat {
            value: Some(losses as f64),
            op: Op::Sum,
            n: sats.len(),
        },
        verdicts: (0, 0),
        failed_bars: BTreeMap::new(),
    }
}

fn event_row(rec: &str, fe: &str, de: &str, evs: &[&EventResult]) -> EventRow {
    let n = evs.len();
    let lost = evs.iter().filter(|e| e.lost).count();
    let reacq = evs.iter().filter(|e| e.reacquired).count();
    EventRow {
        recording: rec.to_owned(),
        event_id: evs[0].event_id.clone(),
        kind: evs[0].kind.clone(),
        event_type: evs[0].event_type.clone(),
        frontend: fe.to_owned(),
        design: de.to_owned(),
        satellites: n,
        lost: count(lost, n),
        held: n - lost,
        time_to_loss_s: opt(evs.iter().map(|e| e.time_to_loss_s)),
        js_at_loss_db: opt(evs.iter().map(|e| e.js_at_loss_db)),
        reacquired: count(reacq, lost),
        reacq_time_s: opt(evs.iter().map(|e| e.reacq_time_s)),
        availability: opt(evs.iter().map(|e| e.availability)),
        verdicts: (0, 0),
        failed_bars: BTreeMap::new(),
    }
}

fn curve(rec: &str, fe: &str, de: &str, evs: &[&EventResult]) -> Option<Curve> {
    let mut by_bin: BTreeMap<u64, Vec<&super::model::Cn0Bin>> = BTreeMap::new();
    for e in evs {
        for b in &e.cn0_curve {
            // Bins are keyed by their exact J/S value as the engine wrote it.
            by_bin.entry(b.js_db.to_bits()).or_default().push(b);
        }
    }
    if by_bin.is_empty() {
        return None;
    }
    let mut bins: Vec<CurveBin> = by_bin
        .into_values()
        .map(|bs| CurveBin {
            js_db: bs[0].js_db,
            n: Stat {
                value: Some(bs.iter().map(|b| b.n).sum::<u64>() as f64),
                op: Op::Sum,
                n: bs.len(),
            },
            measured_degradation_db: opt(bs.iter().map(|b| b.measured_degradation_db)),
            modelled_degradation_db: opt(bs.iter().map(|b| b.modelled_degradation_db)),
        })
        .collect();
    bins.sort_by(|a, b| a.js_db.total_cmp(&b.js_db));
    let m = evs.iter().find_map(|e| e.modelled.as_ref());
    Some(Curve {
        recording: rec.to_owned(),
        event_id: evs[0].event_id.clone(),
        frontend: fe.to_owned(),
        design: de.to_owned(),
        bins,
        q: m.map(|m| m.q),
        formula: m.map(|m| m.formula.clone()),
    })
}

/// The engine's own verdicts (from `scorecard.json`) for the rows of `cells` in `scope`
/// (`run` or `event`, and then for `event_id`): (passed, evaluated) and failed-bar counts.
/// A row with an empty verdict had no bar to meet and is not counted.
fn verdicts(
    c: &LoadedCampaign,
    cells: &[&str],
    scope: &str,
    event_id: Option<&str>,
) -> ((usize, usize), BTreeMap<String, usize>) {
    let mut passed = 0;
    let mut evaluated = 0;
    let mut failed: BTreeMap<String, usize> = BTreeMap::new();
    for r in c.scorecard.rows.iter().filter(|r| {
        r.scope == scope
            && cells.contains(&r.cell.as_str())
            && event_id.is_none_or(|id| r.event_id == id)
    }) {
        match r.verdict.as_str() {
            "PASS" => {
                passed += 1;
                evaluated += 1;
            }
            "FAIL" => {
                evaluated += 1;
                for b in &r.failed_bars {
                    *failed.entry(b.clone()).or_default() += 1;
                }
            }
            _ => {}
        }
    }
    ((passed, evaluated), failed)
}

/// Build the report's tables from a loaded campaign.
pub fn aggregate(c: &LoadedCampaign) -> Aggregates {
    let m = &c.manifest;
    let frontends: Vec<String> = (0..m.frontends.len())
        .filter_map(|i| m.frontend_name(i).map(str::to_owned))
        .collect();
    let designs: Vec<String> = (0..m.designs.len())
        .filter_map(|i| m.design_name(i).map(str::to_owned))
        .collect();
    let mut scorecard = Vec::new();
    let mut events = Vec::new();
    let mut curves = Vec::new();
    for fe in &frontends {
        for de in &designs {
            let cells = cells_for(c, fe, de);
            if cells.is_empty() {
                continue;
            }
            let mut row = score_row(fe, de, &cells);
            let keys: Vec<&str> = cells.iter().map(|x| x.key.as_str()).collect();
            (row.verdicts, row.failed_bars) = verdicts(c, &keys, "run", None);
            scorecard.push(row);
        }
    }
    for rec in &c.manifest.recordings {
        // Event ids in the order the recording's cells list them.
        let mut ids: Vec<String> = Vec::new();
        for cell in c.cells.iter().filter(|x| x.recording.id == rec.id) {
            for s in &cell.satellites {
                for e in &s.events {
                    if !ids.contains(&e.event_id) {
                        ids.push(e.event_id.clone());
                    }
                }
            }
        }
        for id in &ids {
            for fe in &frontends {
                for de in &designs {
                    let evs: Vec<&EventResult> = c
                        .cells
                        .iter()
                        .filter(|x| {
                            x.recording.id == rec.id
                                && &x.frontend.name == fe
                                && &x.design.name == de
                        })
                        .flat_map(|x| &x.satellites)
                        .flat_map(|s| &s.events)
                        .filter(|e| &e.event_id == id)
                        .collect();
                    if evs.is_empty() {
                        continue;
                    }
                    let mut row = event_row(&rec.id, fe, de, &evs);
                    let keys: Vec<&str> = c
                        .cells
                        .iter()
                        .filter(|x| {
                            x.recording.id == rec.id
                                && &x.frontend.name == fe
                                && &x.design.name == de
                        })
                        .map(|x| x.key.as_str())
                        .collect();
                    (row.verdicts, row.failed_bars) = verdicts(c, &keys, "event", Some(id));
                    events.push(row);
                    if let Some(cv) = curve(&rec.id, fe, de, &evs) {
                        curves.push(cv);
                    }
                }
            }
        }
    }
    Aggregates {
        frontends,
        designs,
        scorecard,
        events,
        curves,
        relaxed: relaxed_bars(&c.manifest),
    }
}

/// Competition ranking ("1, 1, 3") of `(name, value)` pairs. Missing values rank last and
/// share the last rank. `higher_is_better` picks the direction.
pub fn rank(values: &[(String, Option<f64>)], higher_is_better: bool) -> Vec<(String, usize)> {
    let mut v: Vec<&(String, Option<f64>)> = values.iter().collect();
    v.sort_by(|a, b| match (a.1, b.1) {
        (Some(x), Some(y)) => {
            let o = x.total_cmp(&y);
            if higher_is_better {
                o.reverse()
            } else {
                o
            }
        }
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    let mut out = Vec::with_capacity(v.len());
    let mut last: Option<Option<f64>> = None;
    let mut last_rank = 0;
    for (i, (name, val)) in v.into_iter().enumerate() {
        let r = if last == Some(*val) { last_rank } else { i + 1 };
        last = Some(*val);
        last_rank = r;
        out.push((name.clone(), r));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_odd_even_and_empty() {
        assert_eq!(median([3.0, 1.0, 2.0]).value, Some(2.0));
        assert_eq!(median([4.0, 1.0, 2.0, 3.0]).value, Some(2.5));
        assert_eq!(median([f64::NAN]).value, None);
        assert_eq!(median([]).n, 0);
    }

    #[test]
    fn ranking_shares_ties_and_puts_missing_last() {
        let v = vec![
            ("a".to_string(), Some(2.0)),
            ("b".to_string(), None),
            ("c".to_string(), Some(5.0)),
            ("d".to_string(), Some(2.0)),
        ];
        let r = rank(&v, false);
        assert_eq!(
            r,
            vec![
                ("a".into(), 1),
                ("d".into(), 1),
                ("c".into(), 3),
                ("b".into(), 4)
            ]
        );
        let r = rank(&v, true);
        assert_eq!(r[0], ("c".into(), 1));
        assert_eq!(r.last().unwrap(), &("b".into(), 4));
    }
}
