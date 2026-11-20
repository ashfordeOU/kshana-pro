// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Civil test-campaign evidence (`kshana-pro altpnt campaign`).
//!
//! A campaign is a set of **runs** of one system under test, each under a stated
//! **condition** (for example `nominal`, `gnss-denied`, `gnss-jammed-simulated`) and in a
//! stated [`Environment`]. For each run the tester supplies what the system **output** (time,
//! position, optionally its horizontal protection level, its alert flag and its time error
//! against the tester's reference) and the **truth** (time, position) from the tester's own
//! reference. The campaign scores the outputs against the truth; it never computes a
//! position:
//!
//! * availability: the share of truth epochs with an output within the match window;
//! * horizontal error (great-circle distance to the truth interpolated at the output time):
//!   50th and 95th percentile (nearest rank) and maximum; vertical error 95th percentile when
//!   both sides carry a height; the time error's 95th percentile of its magnitude;
//! * integrity: an epoch whose horizontal error exceeds the stated protection level is
//!   **misleading**; without the alert flag raised it is **hazardously misleading** (HMI). The
//!   HMI rate carries a Wilson 95 % interval.
//!
//! Runs of one condition pool their epochs, from the strongest environment only (a twin run
//! is never averaged into a flight result). The supplier's **claims** are checked against
//! the pooled scores; a rate claim is also checked at the bound of its interval.
//!
//! Before anything is read, every dataset passes the civil-releasability [`screen`]: an
//! unclassified, civil-releasable dataset of civil signals for a civil end use. A dataset
//! naming a military signal (PRS, M-code, P(Y), SAASM) or a weapons end use is refused and
//! nothing is produced.

use std::collections::BTreeMap;
use std::path::Path;

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};

use super::{evidence_label, rnd, wilson95, Environment, Inputs, EVIDENCE, SCOPE};

/// The result format.
pub const FORMAT: &str = "kshana-pro-altpnt-campaign/1";

fn default_window() -> f64 {
    0.5
}

/// One dataset's releasability declaration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dataset {
    pub id: String,
    /// Must be `unclassified`.
    pub classification: String,
    /// `public` or `civil-restricted`.
    pub release: String,
    /// The signals present in the data (for example "GPS L1 C/A", "Galileo E1 OS").
    #[serde(default)]
    pub signals: Vec<String>,
    #[serde(default)]
    pub owner: String,
}

/// One run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub id: String,
    pub dataset: String,
    pub condition: String,
    pub environment: Environment,
    /// CSV: `t_s,lat_deg,lon_deg` and optional `alt_m,hpl_m,alert,time_error_ns`.
    pub output: String,
    /// CSV: `t_s,lat_deg,lon_deg` and optional `alt_m`.
    pub truth: String,
}

/// A stated claim on a pooled metric of one condition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    pub condition: String,
    /// availability, h50_m, h95_m, hmax_m, v95_m, time95_ns, hmi_rate.
    pub metric: String,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub stated_in: String,
}

/// The request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub system: String,
    /// The campaign's end use (screened).
    pub end_use: String,
    /// Output-to-truth match window, s.
    #[serde(default = "default_window")]
    pub match_window_s: f64,
    #[serde(rename = "dataset")]
    pub datasets: Vec<Dataset>,
    #[serde(rename = "run")]
    pub runs: Vec<Run>,
    #[serde(rename = "claim", default)]
    pub claims: Vec<Claim>,
}

/// The screen's record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    pub end_use: String,
    pub datasets: Vec<String>,
    pub passed: bool,
    pub statement: &'static str,
}

const MIL_SIGNALS: [&str; 7] = [
    "prs", "m-code", "m code", "mcode", "p(y)", "p-code", "saasm",
];
const MIL_END_USES: [&str; 7] = [
    "weapon",
    "munition",
    "missile",
    "targeting",
    "fire control",
    "fire-control",
    "warhead",
];

fn names_term(text: &str, term: &str) -> bool {
    let t = text.to_lowercase();
    // A whole-word match, so "prs" does not match inside "uprsite" and similar.
    let mut from = 0;
    while let Some(i) = t[from..].find(term) {
        let s = from + i;
        let e = s + term.len();
        let before = t[..s]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after = t[e..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        // "weapon" also matches "weapons", "munition" "munitions".
        if before && (after || term.len() > 5) {
            return true;
        }
        from = e;
    }
    false
}

/// The civil-releasability screen. `Err` names every reason a dataset or the end use is
/// refused.
pub fn screen(req: &Request) -> Result<Screen, String> {
    let mut why = Vec::new();
    for t in MIL_END_USES {
        if names_term(&req.end_use, t) {
            why.push(format!(
                "end use names '{t}': weapons end uses are out of scope"
            ));
        }
    }
    for d in &req.datasets {
        if d.classification.trim().to_lowercase() != "unclassified" {
            why.push(format!(
                "dataset {}: classification '{}' (only unclassified data)",
                d.id, d.classification
            ));
        }
        if !matches!(d.release.as_str(), "public" | "civil-restricted") {
            why.push(format!(
                "dataset {}: release '{}' (public or civil-restricted)",
                d.id, d.release
            ));
        }
        for s in &d.signals {
            for t in MIL_SIGNALS {
                if names_term(s, t) {
                    why.push(format!(
                        "dataset {}: signal '{s}' is a military or restricted signal (civil signals only)",
                        d.id
                    ));
                }
            }
        }
    }
    if !why.is_empty() {
        return Err(format!(
            "civil-releasability screen refused the campaign: {}",
            why.join("; ")
        ));
    }
    Ok(Screen {
        end_use: req.end_use.clone(),
        datasets: req
            .datasets
            .iter()
            .map(|d| format!("{} ({}, {})", d.id, d.classification, d.release))
            .collect(),
        passed: true,
        statement: "Unclassified civil-releasable datasets of civil signals for a civil end use, \
                    as declared by the tester.",
    })
}

/// One output epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutEpoch {
    pub t: f64,
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,
    pub hpl: Option<f64>,
    pub alert: bool,
    pub time_error_ns: Option<f64>,
}

/// One truth epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TruthEpoch {
    pub t: f64,
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,
}

fn rows(text: &str, need: &[&str]) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or("empty CSV")?
        .split(',')
        .map(|h| h.trim().to_lowercase())
        .collect();
    for n in need {
        if !head.iter().any(|h| h == n) {
            return Err(format!("CSV header needs '{n}' (has {})", head.join(",")));
        }
    }
    Ok((
        head,
        lines
            .map(|l| l.split(',').map(|c| c.trim().to_string()).collect())
            .collect(),
    ))
}

fn col(head: &[String], row: &[String], name: &str) -> Option<f64> {
    let i = head.iter().position(|h| h == name)?;
    row.get(i)
        .and_then(|c| c.parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

/// Parse an output CSV: (epochs, rows skipped).
pub fn parse_output(text: &str) -> Result<(Vec<OutEpoch>, usize), String> {
    let (h, rs) = rows(text, &["t_s", "lat_deg", "lon_deg"])?;
    let mut out = Vec::new();
    let mut bad = 0;
    for r in rs {
        match (
            col(&h, &r, "t_s"),
            col(&h, &r, "lat_deg"),
            col(&h, &r, "lon_deg"),
        ) {
            (Some(t), Some(lat), Some(lon)) if lat.abs() <= 90.0 && lon.abs() <= 180.0 => {
                out.push(OutEpoch {
                    t,
                    lat,
                    lon,
                    alt: col(&h, &r, "alt_m"),
                    hpl: col(&h, &r, "hpl_m").filter(|v| *v >= 0.0),
                    alert: col(&h, &r, "alert").is_some_and(|v| v != 0.0),
                    time_error_ns: col(&h, &r, "time_error_ns"),
                })
            }
            _ => bad += 1,
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok((out, bad))
}

/// Parse a truth CSV: (epochs, rows skipped).
pub fn parse_truth(text: &str) -> Result<(Vec<TruthEpoch>, usize), String> {
    let (h, rs) = rows(text, &["t_s", "lat_deg", "lon_deg"])?;
    let mut out = Vec::new();
    let mut bad = 0;
    for r in rs {
        match (
            col(&h, &r, "t_s"),
            col(&h, &r, "lat_deg"),
            col(&h, &r, "lon_deg"),
        ) {
            (Some(t), Some(lat), Some(lon)) if lat.abs() <= 90.0 && lon.abs() <= 180.0 => {
                out.push(TruthEpoch {
                    t,
                    lat,
                    lon,
                    alt: col(&h, &r, "alt_m"),
                })
            }
            _ => bad += 1,
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    if out.len() < 2 {
        return Err("truth needs at least two epochs".into());
    }
    Ok((out, bad))
}

/// The truth linearly interpolated at `t` (None outside the truth's span).
pub fn truth_at(truth: &[TruthEpoch], t: f64) -> Option<TruthEpoch> {
    let i = truth.partition_point(|x| x.t < t);
    if i < truth.len() && truth[i].t == t {
        return Some(truth[i]);
    }
    if i == 0 || i >= truth.len() {
        return None;
    }
    let (a, b) = (truth[i - 1], truth[i]);
    let f = (t - a.t) / (b.t - a.t);
    Some(TruthEpoch {
        t,
        lat: a.lat + f * (b.lat - a.lat),
        lon: a.lon + f * (b.lon - a.lon),
        alt: match (a.alt, b.alt) {
            (Some(x), Some(y)) => Some(x + f * (y - x)),
            _ => None,
        },
    })
}

/// Nearest-rank percentile of `v` (sorted ascending), `p` in (0, 100].
pub fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let k = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    Some(sorted[k.min(sorted.len()) - 1])
}

/// The per-epoch residuals of one run.
#[derive(Clone, Debug, Default)]
pub struct Residuals {
    pub truth_epochs: usize,
    pub covered_truth_epochs: usize,
    pub h: Vec<f64>,
    pub v: Vec<f64>,
    pub t_ns: Vec<f64>,
    pub with_pl: usize,
    pub misleading: usize,
    pub hmi: usize,
    pub alerts: usize,
}

/// Compare outputs with the truth.
pub fn residuals(out: &[OutEpoch], truth: &[TruthEpoch], window_s: f64) -> Residuals {
    let mut r = Residuals {
        truth_epochs: truth.len(),
        ..Residuals::default()
    };
    // Availability: truth epochs with an output within the window.
    for te in truth {
        let i = out.partition_point(|o| o.t < te.t - window_s);
        if out.get(i).is_some_and(|o| o.t <= te.t + window_s) {
            r.covered_truth_epochs += 1;
        }
    }
    for o in out {
        let Some(tr) = truth_at(truth, o.t) else {
            continue;
        };
        let h = haversine_m(o.lat, o.lon, tr.lat, tr.lon);
        r.h.push(h);
        if let (Some(a), Some(b)) = (o.alt, tr.alt) {
            r.v.push((a - b).abs());
        }
        if let Some(te) = o.time_error_ns {
            r.t_ns.push(te.abs());
        }
        if o.alert {
            r.alerts += 1;
        }
        if let Some(pl) = o.hpl {
            r.with_pl += 1;
            if h > pl {
                r.misleading += 1;
                if !o.alert {
                    r.hmi += 1;
                }
            }
        }
    }
    r
}

/// Scores of a run or a pooled condition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scores {
    pub scored_epochs: usize,
    pub availability: Option<f64>,
    pub availability_ci95: Option<(f64, f64)>,
    pub h50_m: Option<f64>,
    pub h95_m: Option<f64>,
    pub hmax_m: Option<f64>,
    pub v95_m: Option<f64>,
    pub time95_ns: Option<f64>,
    pub epochs_with_protection_level: usize,
    pub misleading_epochs: usize,
    pub hmi_epochs: usize,
    pub hmi_rate: Option<f64>,
    pub hmi_rate_ci95: Option<(f64, f64)>,
    pub alert_epochs: usize,
}

/// Scores from residuals.
pub fn scores(r: &Residuals) -> Scores {
    let s = |v: &[f64]| {
        let mut v = v.to_vec();
        v.sort_by(f64::total_cmp);
        v
    };
    let (h, v, t) = (s(&r.h), s(&r.v), s(&r.t_ns));
    let r2 = |x: Option<f64>| x.map(|x| rnd(x, 3));
    Scores {
        scored_epochs: r.h.len(),
        availability: (r.truth_epochs > 0)
            .then(|| rnd(r.covered_truth_epochs as f64 / r.truth_epochs as f64, 6)),
        availability_ci95: wilson95(r.covered_truth_epochs, r.truth_epochs),
        h50_m: r2(percentile(&h, 50.0)),
        h95_m: r2(percentile(&h, 95.0)),
        hmax_m: r2(h.last().copied()),
        v95_m: r2(percentile(&v, 95.0)),
        time95_ns: r2(percentile(&t, 95.0)),
        epochs_with_protection_level: r.with_pl,
        misleading_epochs: r.misleading,
        hmi_epochs: r.hmi,
        hmi_rate: (r.with_pl > 0).then(|| rnd(r.hmi as f64 / r.with_pl as f64, 6)),
        hmi_rate_ci95: wilson95(r.hmi, r.with_pl),
        alert_epochs: r.alerts,
    }
}

/// One run's result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunResult {
    pub id: String,
    pub dataset: String,
    pub condition: String,
    pub environment: Environment,
    pub label: &'static str,
    pub skipped_rows: usize,
    pub scores: Scores,
}

/// One condition's pooled result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConditionResult {
    pub condition: String,
    /// The strongest environment among its runs; only its runs are pooled.
    pub environment: Environment,
    pub label: &'static str,
    pub runs_pooled: Vec<String>,
    pub runs_not_pooled: Vec<String>,
    pub scores: Scores,
}

/// One claim checked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimCheck {
    pub id: String,
    pub condition: String,
    pub metric: String,
    pub limit: String,
    pub value: Option<f64>,
    pub label: &'static str,
    /// MET, NOT-MET or NOT-TESTED.
    pub verdict: &'static str,
    /// For a rate: the bound of the 95 % interval also meets the limit.
    pub met_at_interval_bound: Option<bool>,
}

/// The campaign result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Campaign {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub system: String,
    pub screen: Screen,
    pub match_window_s: f64,
    pub runs: Vec<RunResult>,
    pub conditions: Vec<ConditionResult>,
    pub claims: Vec<ClaimCheck>,
    pub scope: &'static str,
}

/// A loaded campaign: the request, each run's CSV texts, the inputs.
pub struct Loaded {
    pub req: Request,
    pub texts: Vec<(String, String)>,
    pub inputs: Inputs,
}

/// Load a request: screens first, then reads every run's files.
pub fn load(path: &Path) -> Result<Loaded, String> {
    let (req, mut inputs): (Request, Inputs) = super::read_request(path)?;
    screen(&req)?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut texts = Vec::new();
    // A file named by several runs (a shared truth) is read and recorded once.
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut get = |path: &str, role: &str, inputs: &mut Inputs| -> Result<String, String> {
        if let Some(t) = seen.get(path) {
            return Ok(t.clone());
        }
        let t = inputs.read(base, path, role)?;
        seen.insert(path.to_string(), t.clone());
        Ok(t)
    };
    for r in &req.runs {
        let o = get(&r.output, "system-output", &mut inputs)?;
        let t = get(&r.truth, "truth", &mut inputs)?;
        texts.push((o, t));
    }
    Ok(Loaded { req, texts, inputs })
}

/// A metric's value and, for a rate, its 95 % interval.
type MetricValue = (Option<f64>, Option<(f64, f64)>);

fn metric(s: &Scores, m: &str) -> Result<MetricValue, String> {
    Ok(match m {
        "availability" => (s.availability, s.availability_ci95),
        "h50_m" => (s.h50_m, None),
        "h95_m" => (s.h95_m, None),
        "hmax_m" => (s.hmax_m, None),
        "v95_m" => (s.v95_m, None),
        "time95_ns" => (s.time95_ns, None),
        "hmi_rate" => (s.hmi_rate, s.hmi_rate_ci95),
        o => return Err(format!("unknown claim metric '{o}'")),
    })
}

/// Score a campaign from the run texts `(output CSV, truth CSV)`, one per run.
pub fn score(req: &Request, texts: &[(String, String)]) -> Result<Campaign, String> {
    let scr = screen(req)?;
    if texts.len() != req.runs.len() {
        return Err("one (output, truth) pair per run".into());
    }
    if !(req.match_window_s.is_finite() && req.match_window_s > 0.0) {
        return Err("match_window_s must be > 0".into());
    }
    let mut runs = Vec::new();
    let mut res: Vec<Residuals> = Vec::new();
    for (r, (o, t)) in req.runs.iter().zip(texts) {
        if !req.datasets.iter().any(|d| d.id == r.dataset) {
            return Err(format!("run {}: unknown dataset '{}'", r.id, r.dataset));
        }
        let (out, b1) = parse_output(o).map_err(|e| format!("run {} output: {e}", r.id))?;
        let (tr, b2) = parse_truth(t).map_err(|e| format!("run {} truth: {e}", r.id))?;
        let rs = residuals(&out, &tr, req.match_window_s);
        runs.push(RunResult {
            id: r.id.clone(),
            dataset: r.dataset.clone(),
            condition: r.condition.clone(),
            environment: r.environment,
            label: evidence_label(&[r.environment]),
            skipped_rows: b1 + b2,
            scores: scores(&rs),
        });
        res.push(rs);
    }
    let mut conds: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, r) in req.runs.iter().enumerate() {
        conds.entry(&r.condition).or_default().push(i);
    }
    let mut conditions = Vec::new();
    for (c, idx) in conds {
        let env = idx
            .iter()
            .map(|&i| req.runs[i].environment)
            .max()
            .expect("non-empty");
        let mut pooled = Residuals::default();
        let (mut yes, mut no) = (Vec::new(), Vec::new());
        for &i in &idx {
            if req.runs[i].environment != env {
                no.push(req.runs[i].id.clone());
                continue;
            }
            yes.push(req.runs[i].id.clone());
            let r = &res[i];
            pooled.truth_epochs += r.truth_epochs;
            pooled.covered_truth_epochs += r.covered_truth_epochs;
            pooled.h.extend(&r.h);
            pooled.v.extend(&r.v);
            pooled.t_ns.extend(&r.t_ns);
            pooled.with_pl += r.with_pl;
            pooled.misleading += r.misleading;
            pooled.hmi += r.hmi;
            pooled.alerts += r.alerts;
        }
        conditions.push(ConditionResult {
            condition: c.to_string(),
            environment: env,
            label: evidence_label(&[env]),
            runs_pooled: yes,
            runs_not_pooled: no,
            scores: scores(&pooled),
        });
    }
    let mut claims = Vec::new();
    for k in &req.claims {
        if k.max.is_some() == k.min.is_some() {
            return Err(format!("claim {}: give exactly one of max or min", k.id));
        }
        let cond = conditions.iter().find(|c| c.condition == k.condition);
        let (value, ci) = match cond {
            Some(c) => metric(&c.scores, &k.metric)?,
            None => {
                metric(&scores(&Residuals::default()), &k.metric)?;
                (None, None)
            }
        };
        let ok = |v: f64| match (k.max, k.min) {
            (Some(m), _) => v <= m,
            (_, Some(m)) => v >= m,
            _ => unreachable!("checked"),
        };
        let verdict = match value {
            None => "NOT-TESTED",
            Some(v) if ok(v) => "MET",
            Some(_) => "NOT-MET",
        };
        let bound = ci.map(|(lo, hi)| ok(if k.max.is_some() { hi } else { lo }));
        claims.push(ClaimCheck {
            id: k.id.clone(),
            condition: k.condition.clone(),
            metric: k.metric.clone(),
            limit: match (k.max, k.min) {
                (Some(m), _) => format!("<= {m}"),
                (_, Some(m)) => format!(">= {m}"),
                _ => unreachable!("checked"),
            },
            value,
            label: cond.map(|c| c.label).unwrap_or("CLAIMED"),
            verdict,
            met_at_interval_bound: bound,
        });
    }
    Ok(Campaign {
        format: FORMAT,
        evidence: EVIDENCE,
        title: req.title.clone(),
        system: req.system.clone(),
        screen: scr,
        match_window_s: req.match_window_s,
        runs,
        conditions,
        claims,
        scope: SCOPE,
    })
}

/// The campaign as Markdown.
pub fn markdown(c: &Campaign) -> String {
    let o = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
    let mut s = format!(
        "# Civil test campaign: {}\n\nSystem under test: {}. Evidence class: {}.\n\n\
         Releasability: {} End use: {}.\n\n## Conditions (pooled, strongest environment only)\n\n\
         | Condition | Environment | Label | Runs | Availability | H50 m | H95 m | Hmax m | HMI epochs / with PL |\n\
         |---|---|---|---|---|---|---|---|---|\n",
        c.title, c.system, c.evidence, c.screen.statement, c.screen.end_use
    );
    for k in &c.conditions {
        s += &format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} / {} |\n",
            k.condition,
            k.environment.name(),
            k.label,
            k.runs_pooled.join(", "),
            o(k.scores.availability),
            o(k.scores.h50_m),
            o(k.scores.h95_m),
            o(k.scores.hmax_m),
            k.scores.hmi_epochs,
            k.scores.epochs_with_protection_level
        );
    }
    s += "\n## Claims\n\n| Claim | Condition | Metric | Limit | Value | Label | Verdict | At interval bound |\n|---|---|---|---|---|---|---|---|\n";
    for k in &c.claims {
        s += &format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            k.id,
            k.condition,
            k.metric,
            k.limit,
            o(k.value),
            k.label,
            k.verdict,
            match k.met_at_interval_bound {
                Some(true) => "met",
                Some(false) => "not met",
                None => "-",
            }
        );
    }
    s += "\n";
    s += c.scope;
    s += "\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_matches_whole_words() {
        assert!(names_term("Galileo PRS", "prs"));
        assert!(!names_term("Uprsite survey", "prs"));
        assert!(names_term("GPS M-code", "m-code"));
        assert!(names_term("weapons trials", "weapon"));
        assert!(!names_term("civil aviation", "weapon"));
    }

    #[test]
    fn percentiles_are_nearest_rank() {
        let v: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(percentile(&v, 95.0), Some(19.0));
        assert_eq!(percentile(&v, 50.0), Some(10.0));
        assert_eq!(percentile(&v, 100.0), Some(20.0));
        assert_eq!(percentile(&[], 50.0), None);
    }
}
