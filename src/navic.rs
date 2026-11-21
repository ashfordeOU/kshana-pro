// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # NavIC handset/chipset test kit and integrity evidence (research 2026-11-20 gaps, B1605)
//!
//! * [`scaffold`]: an L1 SPS test-vector set skeleton (acquisition sensitivity, tracking,
//!   navigation-data decoding, time to first fix, health handling) whose every value is a
//!   placeholder with a `source` to fill from the NavIC L1 SPS interface specification the user
//!   holds; Kshana ships no signal parameter or limit;
//! * [`health`]: health-aware receiver checks from a receiver log: no satellite used while the
//!   user's health record marks it unhealthy, and no NavIC-only position fix at an epoch with
//!   fewer than four healthy NavIC satellites tracked (the 3-versus-4 satellite case);
//! * [`consistency`]: per-epoch NavIC, GPS and Galileo solutions compared pairwise against a
//!   gate `k·√(σa² + σb²)`; inconsistent episodes per pair, to see a fault in one system;
//! * [`timing`]: a receiver's clock offset against NavIC time and against GPS time over a
//!   period: the difference series (inter-system offset as seen), its mean, RMS and MTIE, with
//!   the traceability chain as the user states it (to confirm against the operator's
//!   publications).
//!
//! MODELLED: checks on logs as supplied; nothing here generates a signal or locates an emitter.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc};
use crate::transport::{dist_m, Table};

/// Pro feature.
pub const FEATURE: &str = "pro-navic-evidence";
/// Formats.
pub const SCAFFOLD_FORMAT: &str = "kshana-pro-navic-l1-vectors/1";
pub const HEALTH_FORMAT: &str = "kshana-pro-navic-health-check/1";
pub const CONSISTENCY_FORMAT: &str = "kshana-pro-navic-consistency/1";
pub const TIMING_FORMAT: &str = "kshana-pro-navic-timing/1";

/// A value to transcribe with its source; `null` is a placeholder.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub value: Option<f64>,
    pub unit: String,
    pub source: Option<String>,
}

/// One test case of the scaffold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub id: String,
    pub what: String,
    pub fields: BTreeMap<String, Field>,
}

/// The scaffold.
pub fn scaffold() -> serde_json::Value {
    let f = |u: &str| Field {
        value: None,
        unit: u.into(),
        source: None,
    };
    let cases = vec![
        Case {
            id: "acquisition-sensitivity".into(),
            what: "acquire the L1 SPS signal at a stated power with or without assistance".into(),
            fields: BTreeMap::from([
                ("signal_power_dbm".into(), f("dBm")),
                ("time_limit_s".into(), f("s")),
                ("required_fraction".into(), f("")),
            ]),
        },
        Case {
            id: "tracking-sensitivity".into(),
            what: "keep tracking down to a stated power".into(),
            fields: BTreeMap::from([
                ("signal_power_dbm".into(), f("dBm")),
                ("duration_s".into(), f("s")),
            ]),
        },
        Case {
            id: "navigation-data".into(),
            what: "decode the L1 navigation data without error at a stated power".into(),
            fields: BTreeMap::from([
                ("signal_power_dbm".into(), f("dBm")),
                ("error_rate_limit".into(), f("")),
            ]),
        },
        Case {
            id: "ttff".into(),
            what: "time to first fix, cold and warm".into(),
            fields: BTreeMap::from([("cold_s".into(), f("s")), ("warm_s".into(), f("s"))]),
        },
        Case {
            id: "health-handling".into(),
            what: "drop a satellite flagged unhealthy within a stated time".into(),
            fields: BTreeMap::from([("drop_time_s".into(), f("s"))]),
        },
    ];
    serde_json::json!({
        "format": SCAFFOLD_FORMAT,
        "basis": "Scaffold only: every value is a placeholder to transcribe, with its source, from the NavIC L1 SPS interface specification the user holds. Kshana ships no signal parameter or limit.",
        "cases": cases,
    })
}

/// One health violation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Violation {
    pub utc: String,
    pub what: String,
}

/// The health check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Health {
    pub format: &'static str,
    pub epochs: usize,
    pub unhealthy_used: Vec<Violation>,
    pub navic_only_under_four: Vec<Violation>,
    /// Epochs with fewer than four healthy NavIC satellites tracked.
    pub epochs_under_four: usize,
    pub verdict: String,
}

/// Health-aware checks. `sats`: `timestamp,system,prn,used` (system `I` for NavIC); `health`:
/// `timestamp,system,prn,healthy` (the latest record at or before an epoch applies); `fixes`:
/// `timestamp,fix_systems` (e.g. `I`, `G+E+I`).
pub fn health(sats: &str, health: &str, fixes: &str) -> Result<Health, String> {
    let s = Table::parse(sats).map_err(|e| format!("satellites: {e}"))?;
    let h = Table::parse(health).map_err(|e| format!("health: {e}"))?;
    let f = Table::parse(fixes).map_err(|e| format!("fixes: {e}"))?;
    let t = |tb: &Table, i: usize, c: usize| -> Result<f64, String> {
        let x = tb.text(i, Some(c));
        parse_utc(x).ok_or_else(|| format!("row {}: bad timestamp '{x}'", i + 2))
    };
    // Health history per satellite.
    let (ht, hs, hp, hh) = (
        h.need("timestamp")?,
        h.need("system")?,
        h.need("prn")?,
        h.need("healthy")?,
    );
    let mut hist: BTreeMap<(String, String), Vec<(f64, bool)>> = BTreeMap::new();
    for i in 0..h.rows.len() {
        hist.entry((h.text(i, Some(hs)).into(), h.text(i, Some(hp)).into()))
            .or_default()
            .push((
                t(&h, i, ht)?,
                matches!(h.text(i, Some(hh)), "1" | "true" | "yes"),
            ));
    }
    for v in hist.values_mut() {
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let healthy_at = |sys: &str, prn: &str, at: f64| -> Option<bool> {
        hist.get(&(sys.to_string(), prn.to_string()))?
            .iter()
            .rev()
            .find(|x| x.0 <= at)
            .map(|x| x.1)
    };
    let (st, ss, sp, su) = (
        s.need("timestamp")?,
        s.need("system")?,
        s.need("prn")?,
        s.need("used")?,
    );
    let mut by_epoch: BTreeMap<i64, Vec<(String, String, bool)>> = BTreeMap::new();
    let mut out = Health {
        format: HEALTH_FORMAT,
        epochs: 0,
        unhealthy_used: vec![],
        navic_only_under_four: vec![],
        epochs_under_four: 0,
        verdict: String::new(),
    };
    for i in 0..s.rows.len() {
        let tt = t(&s, i, st)?;
        let (sys, prn) = (
            s.text(i, Some(ss)).to_string(),
            s.text(i, Some(sp)).to_string(),
        );
        let used = matches!(s.text(i, Some(su)), "1" | "true" | "yes");
        if used && healthy_at(&sys, &prn, tt) == Some(false) {
            out.unhealthy_used.push(Violation {
                utc: iso(tt),
                what: format!("{sys}{prn} used while marked unhealthy"),
            });
        }
        by_epoch
            .entry((tt * 10.0).round() as i64)
            .or_default()
            .push((sys, prn, used));
    }
    out.epochs = by_epoch.len();
    let healthy_navic = |k: i64| -> usize {
        by_epoch
            .get(&k)
            .map(|v| {
                v.iter()
                    .filter(|(s, p, _)| {
                        s == "I" && healthy_at(s, p, k as f64 / 10.0) != Some(false)
                    })
                    .count()
            })
            .unwrap_or(0)
    };
    out.epochs_under_four = by_epoch.keys().filter(|k| healthy_navic(**k) < 4).count();
    let (ft, fs) = (f.need("timestamp")?, f.need("fix_systems")?);
    for i in 0..f.rows.len() {
        let tt = t(&f, i, ft)?;
        let sys = f.text(i, Some(fs)).replace(' ', "");
        if sys == "I" {
            let n = healthy_navic((tt * 10.0).round() as i64);
            if n < 4 {
                out.navic_only_under_four.push(Violation {
                    utc: iso(tt),
                    what: format!("NavIC-only fix with {n} healthy NavIC satellite(s) tracked"),
                });
            }
        }
    }
    out.verdict = format!(
        "{} epoch(s), {} with fewer than four healthy NavIC satellites; {} use(s) of an unhealthy satellite; {} NavIC-only fix(es) with fewer than four healthy satellites.",
        out.epochs,
        out.epochs_under_four,
        out.unhealthy_used.len(),
        out.navic_only_under_four.len()
    );
    Ok(out)
}

/// One inconsistent episode between two solutions.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Episode {
    pub pair: String,
    pub start_utc: String,
    pub end_utc: String,
    pub epochs: usize,
    pub max_separation_m: f64,
}

/// The consistency result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Consistency {
    pub format: &'static str,
    pub gate_k: f64,
    /// Pair to `(compared, inconsistent)`.
    pub pairs: BTreeMap<String, (usize, usize)>,
    pub episodes: Vec<Episode>,
    pub verdict: String,
}

/// Pairwise consistency of per-system solutions `timestamp,solution,lat,lon,sigma_h_m`
/// (`solution` e.g. `I`, `G`, `E`).
pub fn consistency(csv: &str, gate_k: f64, join_s: f64) -> Result<Consistency, String> {
    if !(gate_k > 0.0 && join_s > 0.0) {
        return Err("gate_k and join_s must be positive".into());
    }
    let tb = Table::parse(csv)?;
    let (ct, cs, cla, clo, csg) = (
        tb.need("timestamp")?,
        tb.need("solution")?,
        tb.need("lat")?,
        tb.need("lon")?,
        tb.need("sigma_h_m")?,
    );
    let mut ep: BTreeMap<i64, BTreeMap<String, (f64, f64, f64)>> = BTreeMap::new();
    for i in 0..tb.rows.len() {
        let x = tb.text(i, Some(ct));
        let tt = parse_utc(x).ok_or_else(|| format!("row {}: bad timestamp", i + 2))?;
        let n = |c| {
            tb.num(i, Some(c))?
                .ok_or_else(|| format!("row {}: empty value", i + 2))
        };
        ep.entry((tt * 10.0).round() as i64).or_default().insert(
            tb.text(i, Some(cs)).to_string(),
            (n(cla)?, n(clo)?, n(csg)?),
        );
    }
    let mut pairs: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut open: BTreeMap<String, (f64, f64, usize, f64)> = BTreeMap::new();
    let mut eps = Vec::new();
    let close = |pair: &str, o: (f64, f64, usize, f64), eps: &mut Vec<Episode>| {
        eps.push(Episode {
            pair: pair.into(),
            start_utc: iso(o.0),
            end_utc: iso(o.1),
            epochs: o.2,
            max_separation_m: crate::forensics::inputs::round(o.3, 1),
        });
    };
    for (k, sols) in &ep {
        let t = *k as f64 / 10.0;
        let names: Vec<&String> = sols.keys().collect();
        for i in 0..names.len() {
            for j in i + 1..names.len() {
                let (a, b) = (&sols[names[i]], &sols[names[j]]);
                let pair = format!("{}-{}", names[i], names[j]);
                let d = dist_m(a.0, a.1, b.0, b.1);
                let g = gate_k * (a.2 * a.2 + b.2 * b.2).sqrt();
                let e = pairs.entry(pair.clone()).or_insert((0, 0));
                e.0 += 1;
                if d > g {
                    e.1 += 1;
                    let o = open.entry(pair.clone()).or_insert((t, t, 0, 0.0));
                    if t - o.1 > join_s {
                        let old = *o;
                        close(&pair, old, &mut eps);
                        *o = (t, t, 0, 0.0);
                    }
                    o.1 = t;
                    o.2 += 1;
                    o.3 = o.3.max(d);
                }
            }
        }
    }
    for (p, o) in open {
        close(&p, o, &mut eps);
    }
    eps.sort_by(|a, b| a.start_utc.cmp(&b.start_utc));
    let verdict = format!(
        "{} pair(s) compared; {} inconsistent episode(s). An episode on every pair with one system points at that system.",
        pairs.len(),
        eps.len()
    );
    Ok(Consistency {
        format: CONSISTENCY_FORMAT,
        gate_k,
        pairs,
        episodes: eps,
        verdict,
    })
}

/// The timing evidence request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimingRequest {
    pub title: String,
    /// CSV `timestamp,offset_navic_s,offset_gps_s` (receiver clock minus each system time).
    pub offsets: String,
    /// The traceability chain as the user states it (each link to confirm).
    pub traceability: Vec<String>,
    #[serde(default = "d_windows")]
    pub mtie_windows_s: Vec<f64>,
}

fn d_windows() -> Vec<f64> {
    vec![10.0, 100.0, 1000.0]
}

/// Timing evidence.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Timing {
    pub format: &'static str,
    pub title: String,
    pub samples: usize,
    pub start_utc: String,
    pub end_utc: String,
    pub navic_minus_gps_mean_s: f64,
    pub navic_minus_gps_rms_s: f64,
    pub navic_minus_gps_p2p_s: f64,
    pub mtie_navic_s: Vec<(f64, f64)>,
    pub traceability: Vec<String>,
    pub statement: &'static str,
}

/// Timing evidence from the offsets CSV text.
pub fn timing(q: &TimingRequest, csv: &str) -> Result<Timing, String> {
    if q.traceability.is_empty() {
        return Err("state the traceability chain (each link, to confirm)".into());
    }
    let tb = Table::parse(csv)?;
    let (ct, cn, cg) = (
        tb.need("timestamp")?,
        tb.need("offset_navic_s")?,
        tb.need("offset_gps_s")?,
    );
    let mut s: Vec<(f64, f64, f64)> = Vec::new();
    for i in 0..tb.rows.len() {
        let x = tb.text(i, Some(ct));
        let tt = parse_utc(x).ok_or_else(|| format!("row {}: bad timestamp", i + 2))?;
        if let (Some(a), Some(b)) = (tb.num(i, Some(cn))?, tb.num(i, Some(cg))?) {
            s.push((tt, a, b));
        }
    }
    if s.len() < 10 {
        return Err("fewer than 10 samples".into());
    }
    s.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Columns are receiver minus each system time, so NavIC − GPS = (rx − GPS) − (rx − NavIC).
    let d: Vec<f64> = s.iter().map(|x| x.2 - x.1).collect();
    let n = d.len() as f64;
    let mean = d.iter().sum::<f64>() / n;
    let rms = (d.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
    let (lo, hi) = d
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
            (a.min(*x), b.max(*x))
        });
    let xs: Vec<f64> = s.iter().map(|x| x.1).collect();
    let tau0 = (s[s.len() - 1].0 - s[0].0) / (s.len() - 1) as f64;
    let mtie = q
        .mtie_windows_s
        .iter()
        .filter_map(|&w| {
            let m = (w / tau0).round() as usize;
            (m >= 1 && m < xs.len()).then(|| {
                let v = (0..xs.len() - m)
                    .map(|i| {
                        let win = &xs[i..=i + m];
                        win.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                            - win.iter().cloned().fold(f64::INFINITY, f64::min)
                    })
                    .fold(0.0, f64::max);
                (w, v)
            })
        })
        .collect();
    Ok(Timing {
        format: TIMING_FORMAT,
        title: q.title.clone(),
        samples: s.len(),
        start_utc: iso(s[0].0),
        end_utc: iso(s[s.len() - 1].0),
        navic_minus_gps_mean_s: mean,
        navic_minus_gps_rms_s: rms,
        navic_minus_gps_p2p_s: hi - lo,
        mtie_navic_s: mtie,
        traceability: q.traceability.clone(),
        statement: "Measured offsets of the receiver clock against NavIC and GPS time as the receiver reports them; the traceability chain is as the user states it and each link is to confirm against the system operators' publications.",
    })
}
