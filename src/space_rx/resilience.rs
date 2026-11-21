// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Resilience evaluation of a GNSS space receiver under degradation scenarios
//!
//! Scores a space receiver's **logged behaviour** against a reference trajectory and clock over
//! the degradation windows a test campaign applied: signal outages, ionospheric scintillation,
//! broadcast-ephemeris faults and loss of signal above the constellation (weak, sparse signals
//! at high altitude). Per scenario, against the mission's stated limits:
//!
//! * **availability** inside the window (epochs with a fix) and in the clean baseline;
//! * **accuracy** inside the window (95th percentile position error against the reference);
//! * **integrity**: epochs with a fix whose position error exceeds the mission's position limit
//!   while the receiver raised no integrity flag (**misleading, unflagged**), and for a fault
//!   scenario the time from fault onset to the receiver's first flag;
//! * **time to recover** after the window: first epoch from which a fix within the position
//!   limit holds for the stated settle time;
//! * **holdover**: during epochs without a fix, the time error of the receiver's clock output
//!   against the reference clock, and how long it stayed within the mission's time limit.
//!
//! The log is the space-receiver trace (`kshana-space-rx-trace/1` columns, [`super::see`]) with
//! an optional `integrity_flag` column; the reference is `t_s,x_m,y_m,z_m,clock_bias_m`.
//! **Evaluation only**: it measures behaviour already logged; it implements no interference
//! mitigation or authentication technique. MODELLED where the logs are synthetic.

use serde::{Deserialize, Serialize};

use crate::transport::Table;

/// Pro feature.
pub const FEATURE: &str = "pro-space-rx-resilience";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-space-rx-resilience/1";
/// Label.
pub const EVIDENCE: &str = "Measured from the receiver log and reference supplied, against the \
mission's stated limits (MODELLED where the logs are synthetic). Evaluation only: no mitigation \
or authentication technique is implemented. Evidence for a review; not a statement that a part \
is fit for flight.";

const C: f64 = 299_792_458.0;

/// Scenario kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Outage,
    Scintillation,
    EphemerisFault,
    AboveConstellation,
}

/// A degradation window applied in the campaign.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub kind: Kind,
    pub from_s: f64,
    pub to_s: f64,
}

/// Mission limits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub position_m: f64,
    pub time_ns: f64,
    pub min_availability: f64,
    pub max_recovery_s: f64,
    /// A fault must be flagged within this (s).
    pub max_flag_s: f64,
    /// Recovery holds for this long (s).
    #[serde(default = "d_settle")]
    pub settle_s: f64,
    /// Where the limits come from.
    pub source: String,
}

fn d_settle() -> f64 {
    10.0
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub receiver: String,
    pub data_source: String,
    pub log: String,
    pub reference: String,
    pub limits: Limits,
    #[serde(rename = "scenario")]
    pub scenarios: Vec<Scenario>,
}

#[derive(Clone, Debug)]
struct Row {
    t: f64,
    r: [f64; 3],
    clk_m: f64,
    fix: bool,
    flag: bool,
}

/// One scenario's score.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Score {
    pub id: String,
    pub kind: Kind,
    pub epochs: usize,
    pub availability: f64,
    pub p95_position_m: Option<f64>,
    pub misleading_unflagged: usize,
    pub time_to_flag_s: Option<f64>,
    pub time_to_recover_s: Option<f64>,
    pub holdover_max_time_error_ns: Option<f64>,
    /// Seconds of no-fix epochs whose time error stayed within the limit, from the first.
    pub holdover_within_limit_s: Option<f64>,
    /// Check → met.
    pub checks: Vec<(String, bool)>,
    pub verdict: &'static str,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub receiver: String,
    pub data_source: String,
    pub limits: Limits,
    pub baseline_availability: f64,
    pub baseline_p95_position_m: Option<f64>,
    pub scenarios: Vec<Score>,
    pub verdict: String,
}

fn p95(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[((v.len() as f64 * 0.95).ceil() as usize).clamp(1, v.len()) - 1])
}

fn read_log(text: &str) -> Result<Vec<Row>, String> {
    let t = Table::parse(text)?;
    let (ct, cx, cy, cz, cc, cf) = (
        t.need("t_s")?,
        t.need("x_m")?,
        t.need("y_m")?,
        t.need("z_m")?,
        t.need("clock_bias_m")?,
        t.need("fix")?,
    );
    let cflag = t.col("integrity_flag");
    let mut out = vec![];
    for i in 0..t.rows.len() {
        let n = |c: usize| -> Result<f64, String> {
            t.num(i, Some(c))?
                .ok_or_else(|| format!("row {}: empty", i + 2))
        };
        let b = |c: Option<usize>| matches!(t.text(i, c), "1" | "true" | "yes");
        out.push(Row {
            t: n(ct)?,
            r: [n(cx)?, n(cy)?, n(cz)?],
            clk_m: n(cc)?,
            fix: b(Some(cf)),
            flag: b(cflag),
        });
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok(out)
}

fn read_ref(text: &str) -> Result<Vec<(f64, [f64; 3], f64)>, String> {
    let t = Table::parse(text)?;
    let (ct, cx, cy, cz, cc) = (
        t.need("t_s")?,
        t.need("x_m")?,
        t.need("y_m")?,
        t.need("z_m")?,
        t.need("clock_bias_m")?,
    );
    let mut out = vec![];
    for i in 0..t.rows.len() {
        let n = |c: usize| -> Result<f64, String> {
            t.num(i, Some(c))?
                .ok_or_else(|| format!("row {}: empty", i + 2))
        };
        out.push((n(ct)?, [n(cx)?, n(cy)?, n(cz)?], n(cc)?));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(out)
}

fn ref_at(r: &[(f64, [f64; 3], f64)], t: f64) -> Option<([f64; 3], f64)> {
    let i = r.partition_point(|x| x.0 <= t);
    let (a, b) = (r.get(i.checked_sub(1)?)?, r.get(i).unwrap_or(r.get(i - 1)?));
    if b.0 - a.0 > 30.0 {
        return None;
    }
    let f = if b.0 > a.0 {
        (t - a.0) / (b.0 - a.0)
    } else {
        0.0
    };
    Some((
        [0, 1, 2].map(|k| a.1[k] + f * (b.1[k] - a.1[k])),
        a.2 + f * (b.2 - a.2),
    ))
}

/// Score a log.
pub fn score(q: &Request, log: &str, reference: &str) -> Result<Report, String> {
    let l = &q.limits;
    if q.data_source.trim().is_empty() || l.source.trim().is_empty() {
        return Err("data_source and limits.source must be stated".into());
    }
    let rows = read_log(log)?;
    let refr = read_ref(reference)?;
    let err = |r: &Row| -> Option<(f64, f64)> {
        let (p, c) = ref_at(&refr, r.t)?;
        let d =
            ((r.r[0] - p[0]).powi(2) + (r.r[1] - p[1]).powi(2) + (r.r[2] - p[2]).powi(2)).sqrt();
        Some((d, (r.clk_m - c) / C * 1e9))
    };
    let in_any = |t: f64| {
        q.scenarios
            .iter()
            .any(|s| t >= s.from_s && t < s.to_s + l.max_recovery_s)
    };
    let base: Vec<&Row> = rows.iter().filter(|r| !in_any(r.t)).collect();
    let baseline_availability =
        base.iter().filter(|r| r.fix).count() as f64 / base.len().max(1) as f64;
    let baseline_p95 = p95(base
        .iter()
        .filter(|r| r.fix)
        .filter_map(|r| err(r).map(|e| e.0))
        .collect());

    let mut scores = vec![];
    for s in &q.scenarios {
        if s.to_s.is_nan() || s.to_s <= s.from_s {
            return Err(format!("scenario {}: to_s after from_s", s.id));
        }
        let win: Vec<&Row> = rows
            .iter()
            .filter(|r| r.t >= s.from_s && r.t < s.to_s)
            .collect();
        let n = win.len();
        let availability = win.iter().filter(|r| r.fix).count() as f64 / n.max(1) as f64;
        let errs: Vec<(f64, f64)> = win
            .iter()
            .filter(|r| r.fix)
            .filter_map(|r| err(r))
            .collect();
        let p95_pos = p95(errs.iter().map(|e| e.0).collect());
        let misleading = win
            .iter()
            .filter(|r| r.fix && !r.flag && err(r).is_some_and(|e| e.0 > l.position_m))
            .count();
        let time_to_flag = rows
            .iter()
            .find(|r| r.t >= s.from_s && r.t < s.to_s + l.max_recovery_s && r.flag)
            .map(|r| r.t - s.from_s);
        // Recovery: first epoch after the window from which every epoch for settle_s has a fix
        // within the position limit.
        let after: Vec<&Row> = rows.iter().filter(|r| r.t >= s.to_s).collect();
        let good = |r: &Row| r.fix && !r.flag && err(r).is_some_and(|e| e.0 <= l.position_m);
        let mut recover = None;
        for (i, r) in after.iter().enumerate() {
            if !good(r) {
                continue;
            }
            let span: Vec<&&Row> = after[i..]
                .iter()
                .take_while(|x| x.t <= r.t + l.settle_s)
                .collect();
            if span.last().is_some_and(|x| x.t >= r.t + l.settle_s - 1e-6)
                && span.iter().all(|x| good(x))
            {
                recover = Some(r.t - s.to_s);
                break;
            }
        }
        // Holdover over the no-fix epochs of the window.
        let nofix: Vec<(f64, f64)> = win
            .iter()
            .filter(|r| !r.fix)
            .filter_map(|r| err(r).map(|e| (r.t, e.1)))
            .collect();
        let (hmax, hwithin) = if nofix.is_empty() {
            (None, None)
        } else {
            // Offset at the last fixed epoch before the window removes the steady clock offset.
            let off = rows
                .iter()
                .rev()
                .find(|r| r.t < s.from_s && r.fix)
                .and_then(&err)
                .map(|e| e.1)
                .unwrap_or(0.0);
            let t0 = nofix[0].0;
            let max = nofix.iter().map(|x| (x.1 - off).abs()).fold(0.0, f64::max);
            let within = nofix
                .iter()
                .take_while(|x| (x.1 - off).abs() <= l.time_ns)
                .last()
                .map(|x| x.0 - t0)
                .unwrap_or(0.0);
            (Some(max), Some(within))
        };
        let mut checks = vec![(
            "no misleading unflagged epochs".to_string(),
            misleading == 0,
        )];
        match s.kind {
            Kind::EphemerisFault | Kind::Scintillation => {
                let flagged_ok = misleading == 0 || time_to_flag.is_some_and(|t| t <= l.max_flag_s);
                checks.push((
                    format!(
                        "flagged within {} s or no error beyond the limit",
                        l.max_flag_s
                    ),
                    flagged_ok,
                ));
                checks.push((
                    format!("availability ≥ {}", l.min_availability),
                    availability >= l.min_availability,
                ));
            }
            Kind::Outage | Kind::AboveConstellation => {
                if let Some(h) = hmax {
                    checks.push((
                        format!("holdover time error ≤ {} ns", l.time_ns),
                        h <= l.time_ns,
                    ));
                }
                if s.kind == Kind::AboveConstellation {
                    checks.push((
                        format!("availability ≥ {}", l.min_availability),
                        availability >= l.min_availability,
                    ));
                }
            }
        }
        checks.push((
            format!("recovered within {} s", l.max_recovery_s),
            recover.is_some_and(|t| t <= l.max_recovery_s),
        ));
        let verdict = if checks.iter().all(|c| c.1) {
            "met"
        } else {
            "not met"
        };
        scores.push(Score {
            id: s.id.clone(),
            kind: s.kind,
            epochs: n,
            availability: (availability * 1e4).round() / 1e4,
            p95_position_m: p95_pos.map(|x| (x * 10.0).round() / 10.0),
            misleading_unflagged: misleading,
            time_to_flag_s: time_to_flag.map(|x| (x * 10.0).round() / 10.0),
            time_to_recover_s: recover.map(|x| (x * 10.0).round() / 10.0),
            holdover_max_time_error_ns: hmax.map(|x| (x * 10.0).round() / 10.0),
            holdover_within_limit_s: hwithin.map(|x| (x * 10.0).round() / 10.0),
            checks,
            verdict,
        });
    }
    let met = scores.iter().filter(|s| s.verdict == "met").count();
    let verdict = format!(
        "{}: {met} of {} scenario(s) met the mission limits ({}); baseline availability {:.3}",
        q.receiver,
        scores.len(),
        l.source,
        baseline_availability
    );
    Ok(Report {
        format: FORMAT,
        evidence: EVIDENCE,
        title: q.title.clone(),
        receiver: q.receiver.clone(),
        data_source: q.data_source.clone(),
        limits: l.clone(),
        baseline_availability: (baseline_availability * 1e4).round() / 1e4,
        baseline_p95_position_m: baseline_p95.map(|x| (x * 10.0).round() / 10.0),
        scenarios: scores,
        verdict,
    })
}

/// HTML page.
pub fn html(r: &Report) -> String {
    let o = |x: Option<f64>| x.map(|v| format!("{v}")).unwrap_or_else(|| "-".into());
    crate::transport::page(
        &r.title,
        r.evidence,
        &r.verdict,
        &[format!(
            "Receiver {}. Data source: {}. Limits: {}.",
            r.receiver, r.data_source, r.limits.source
        )],
        &[(
            "Scenarios".into(),
            vec![
                "scenario",
                "kind",
                "availability",
                "p95 position (m)",
                "misleading unflagged",
                "time to flag (s)",
                "time to recover (s)",
                "holdover max (ns)",
                "verdict",
            ],
            r.scenarios
                .iter()
                .map(|s| {
                    vec![
                        s.id.clone(),
                        format!("{:?}", s.kind),
                        format!("{}", s.availability),
                        o(s.p95_position_m),
                        s.misleading_unflagged.to_string(),
                        o(s.time_to_flag_s),
                        o(s.time_to_recover_s),
                        o(s.holdover_max_time_error_ns),
                        s.verdict.to_string(),
                    ]
                })
                .collect(),
        )],
    )
}

/// SYNTHETIC logs for a receiver over four scenarios: (log CSV, reference CSV, request TOML).
/// `good` flags the ephemeris fault in 15 s and recovers quickly; the other keeps an
/// unflagged biased fix and recovers slowly.
pub fn synth(good: bool) -> (String, String, String) {
    let truth = super::see::synth_trace(1800, 1.0, 550.0, 97.6, 11);
    let mut reference = String::from("t_s,x_m,y_m,z_m,clock_bias_m\n");
    let mut log =
        String::from("t_s,x_m,y_m,z_m,clock_bias_m,n_sv,cn0_mean_dbhz,fix,integrity_flag\n");
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut nz = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let t0 = 1000.0;
    // Windows (s after t0): outage 200-320, scintillation 500-620, ephemeris fault 800-950,
    // above-constellation 1200-1400.
    for e in &truth {
        let s = (e.t_s - t0).round();
        let t = t0 + s;
        let clk = 1500.0 + 0.3 * s;
        reference += &format!(
            "{t:.1},{:.3},{:.3},{:.3},{clk:.3}\n",
            e.r[0], e.r[1], e.r[2]
        );
        let (mut fix, mut flag, mut sigma, mut bias, mut nsv, mut cn0) =
            (true, false, 3.0, 0.0, 9, 44.0);
        let mut clock = clk + 0.5 * nz();
        if (200.0..320.0).contains(&s) {
            fix = false;
            nsv = 0;
            cn0 = 0.0;
            // Clock coasts: drift error grows (good receiver smaller).
            let dt = s - 200.0;
            clock = clk + if good { 0.05 } else { 1.0 } * dt * dt / 100.0;
        }
        if (320.0..335.0).contains(&s) && !good {
            fix = false;
        }
        if (500.0..620.0).contains(&s) {
            cn0 = 32.0;
            nsv = 5;
            sigma = 12.0;
            if (s as i64) % 7 == 0 {
                fix = false;
            }
        }
        if (800.0..950.0).contains(&s) {
            bias = 5.0 * (s - 800.0);
            if good && s >= 815.0 {
                flag = true;
            }
        }
        if !good && (950.0..1100.0).contains(&s) {
            bias = 300.0;
        }
        if (1200.0..1400.0).contains(&s) {
            nsv = 3;
            cn0 = 30.0;
            if (s as i64) % if good { 5 } else { 2 } == 0 {
                fix = false;
            }
            sigma = 20.0;
        }
        let r = [
            e.r[0] + bias + sigma * nz(),
            e.r[1] + sigma * nz(),
            e.r[2] + sigma * nz(),
        ];
        log += &format!(
            "{t:.1},{:.3},{:.3},{:.3},{clock:.3},{nsv},{cn0:.1},{},{}\n",
            r[0], r[1], r[2], fix as u8, flag as u8
        );
    }
    let req = format!(
        "title = \"SYNTHETIC space-receiver resilience evaluation\"\nreceiver = \"{}\"\ndata_source = \"SYNTHETIC (kshana-pro space-rx resilience synth)\"\nlog = \"log.csv\"\nreference = \"reference.csv\"\n\n[limits]\nposition_m = 100.0\ntime_ns = 300.0\nmin_availability = 0.7\nmax_recovery_s = 60.0\nmax_flag_s = 30.0\nsource = \"SYNTHETIC mission requirements\"\n\n[[scenario]]\nid = \"outage-120s\"\nkind = \"outage\"\nfrom_s = 1200.0\nto_s = 1320.0\n\n[[scenario]]\nid = \"scintillation\"\nkind = \"scintillation\"\nfrom_s = 1500.0\nto_s = 1620.0\n\n[[scenario]]\nid = \"ephemeris-ramp\"\nkind = \"ephemeris-fault\"\nfrom_s = 1800.0\nto_s = 1950.0\n\n[[scenario]]\nid = \"high-altitude-weak\"\nkind = \"above-constellation\"\nfrom_s = 2200.0\nto_s = 2400.0\n",
        if good { "SYNTH-RX-GOOD" } else { "SYNTH-RX-FAULTY" }
    );
    (log, reference, req)
}
