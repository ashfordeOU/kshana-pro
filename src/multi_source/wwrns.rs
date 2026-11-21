// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Position-domain multi-source trust for WWRNS users: GNSS, R-Mode and eLoran
//!
//! Research 2026-11-20 gaps B3701 (07b R06): IMO A.1046(27) as amended by MSC.599(111) opens
//! the World-Wide Radionavigation System to terrestrial systems, so a ship can carry GNSS, MF
//! R-Mode and eLoran fixes at once. This module applies the F3 selection rule
//! ([`super`], pre-registered for time sources) to **position fixes**, epoch by epoch:
//!
//! * **Exclusion.** A source whose status is not `ok`, or that gives no fix, is out.
//! * **Votes.** Every other group votes through its best source (smallest stated sigma). Two
//!   fixes agree when their horizontal distance is at most `K·√(σ_a² + σ_b²)`, with `σ` the
//!   per-axis one-sigma and `K = √(−2 ln p_fa)` (the Rayleigh quantile: the distance between
//!   two consistent circular-Gaussian fixes exceeds it with probability `p_fa`).
//! * **Qualification.** At least one agreeing vote and no more disagreeing than agreeing.
//! * **Choice.** The qualified source with the smallest sigma (configured order breaks ties).
//!   The output is the **chosen source's own fix**: sources are never blended, so this is a
//!   trust decision with its evidence, not a navigation filter.
//! * **GNSS suspect.** GNSS reported `ok` but disqualified while two other groups agree with
//!   each other: the pattern a spoofed or faulty GNSS leaves against independent terrestrial
//!   systems.
//! * **Accuracy check.** The chosen source's 95 % horizontal radius (`2.448·σ` for a circular
//!   error) against the requirement the user states (for example the figure of the WWRNS
//!   resolution for the sea area; not built in).
//!
//! Evidence: **MODELLED**; the decision is only as good as each source's stated sigma.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "kshana-pro-wwrns-trust/1";

/// 95 % radius of a circular Gaussian error over its per-axis sigma: √(−2 ln 0.05).
pub const R95_OVER_SIGMA: f64 = 2.447_746_830_680_816;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WwrnsSource {
    pub id: String,
    /// Sources of one group share a signal path and never vote for each other.
    pub group: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WwrnsConfig {
    #[serde(rename = "source")]
    pub sources: Vec<WwrnsSource>,
    /// False-disagreement probability per pair test (default 1e-5).
    #[serde(default)]
    pub p_fa: Option<f64>,
    /// The group whose disqualification against two agreeing others is flagged.
    #[serde(default = "d_gnss")]
    pub gnss_group: String,
    /// Required 95 % horizontal accuracy (m), stated by the user with its source.
    #[serde(default)]
    pub required_accuracy_95_m: Option<f64>,
    #[serde(default)]
    pub requirement_source: String,
}

fn d_gnss() -> String {
    "gnss".into()
}

/// One source's fix at one epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fix {
    pub t_s: f64,
    pub source: String,
    pub ok: bool,
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    /// Stated per-axis one-sigma (m).
    pub sigma_m: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceVerdict {
    pub source: String,
    /// `out`, `qualified`, `disqualified`, `no_votes`.
    pub verdict: String,
    pub agree: usize,
    pub disagree: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WwrnsDecision {
    pub t_s: f64,
    pub choice: Option<String>,
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    /// `K·√(σ_c² + σ_ref²)` with the tightest agreeing vote (m).
    pub trust_radius_m: Option<f64>,
    pub accuracy_95_m: Option<f64>,
    pub meets_requirement: Option<bool>,
    pub gnss_suspect: bool,
    pub verdicts: Vec<SourceVerdict>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WwrnsReport {
    pub format: String,
    pub k: f64,
    pub epochs: usize,
    pub chosen: BTreeMap<String, usize>,
    pub no_choice: usize,
    pub switches: usize,
    pub gnss_suspect_epochs: usize,
    pub meets_requirement_epochs: Option<usize>,
    pub required_accuracy_95_m: Option<f64>,
    pub requirement_source: String,
    pub decisions: Vec<WwrnsDecision>,
    pub evidence: String,
}

fn dist_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    // Haversine on the mean sphere: ample for fix-to-fix distances of metres to kilometres.
    let r = 6_371_008.8;
    let (p1, p2) = (a.0.to_radians(), b.0.to_radians());
    let dp = p2 - p1;
    let dl = (b.1 - a.1).to_radians();
    let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

/// Parse fixes from CSV `t_s,source,status,lat_deg,lon_deg,sigma_m` (status `ok` or anything
/// else; empty cells missing).
pub fn parse_fixes(text: &str) -> Result<Vec<Fix>, String> {
    let mut out = vec![];
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("t_s") {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() < 6 {
            return Err(format!("fixes line {}: need 6 columns", i + 1));
        }
        let num = |s: &str| -> Result<Option<f64>, String> {
            if s.is_empty() {
                Ok(None)
            } else {
                s.parse()
                    .map(Some)
                    .map_err(|_| format!("fixes line {}: {s:?} is not a number", i + 1))
            }
        };
        out.push(Fix {
            t_s: num(f[0])?.ok_or(format!("fixes line {}: t_s missing", i + 1))?,
            source: f[1].to_string(),
            ok: f[2] == "ok",
            lat_deg: num(f[3])?,
            lon_deg: num(f[4])?,
            sigma_m: num(f[5])?,
        });
    }
    Ok(out)
}

/// Run the selection over all epochs (fixes grouped by `t_s`).
pub fn run(cfg: &WwrnsConfig, fixes: &[Fix]) -> Result<WwrnsReport, String> {
    if cfg.sources.len() < 2 {
        return Err("at least two sources are needed".into());
    }
    let p_fa = cfg.p_fa.unwrap_or(1e-5);
    if !(p_fa > 0.0 && p_fa < 0.5) {
        return Err("p_fa must be in (0, 0.5)".into());
    }
    let k = (-2.0 * p_fa.ln()).sqrt();
    let index: BTreeMap<&str, usize> = cfg
        .sources
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    if index.len() != cfg.sources.len() {
        return Err("source ids must be unique".into());
    }
    for f in fixes {
        if !index.contains_key(f.source.as_str()) {
            return Err(format!(
                "fix at {} s names unknown source {:?}",
                f.t_s, f.source
            ));
        }
    }
    let mut epochs: BTreeMap<i64, Vec<&Fix>> = BTreeMap::new();
    for f in fixes {
        epochs
            .entry((f.t_s * 1000.0).round() as i64)
            .or_default()
            .push(f);
    }
    let mut decisions = vec![];
    for (key, fs) in &epochs {
        let n = cfg.sources.len();
        let mut fix: Vec<Option<((f64, f64), f64)>> = vec![None; n];
        for f in fs {
            if let (true, Some(a), Some(o), Some(s)) = (f.ok, f.lat_deg, f.lon_deg, f.sigma_m) {
                if s > 0.0 {
                    fix[index[f.source.as_str()]] = Some(((a, o), s));
                }
            }
        }
        // Group representatives.
        let mut rep: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, s) in cfg.sources.iter().enumerate() {
            if let Some((_, si)) = fix[i] {
                let better = rep
                    .get(s.group.as_str())
                    .is_none_or(|&j| si < fix[j].unwrap().1);
                if better {
                    rep.insert(s.group.as_str(), i);
                }
            }
        }
        // Fixes from different sources are compared by great-circle distance: every source's
        // latitude and longitude must be on one datum (WGS-84); none is converted here.
        let agrees = |a: usize, b: usize| {
            let ((pa, sa), (pb, sb)) = (fix[a].unwrap(), fix[b].unwrap());
            dist_m(pa, pb) <= k * (sa * sa + sb * sb).sqrt()
        };
        let mut verdicts = vec![];
        let mut qualified = vec![];
        for (i, s) in cfg.sources.iter().enumerate() {
            if fix[i].is_none() {
                verdicts.push(SourceVerdict {
                    source: s.id.clone(),
                    verdict: "out".into(),
                    agree: 0,
                    disagree: 0,
                });
                continue;
            }
            let (mut ag, mut dis) = (0, 0);
            let mut tight: Option<f64> = None;
            for (g, &j) in &rep {
                if *g == s.group {
                    continue;
                }
                if agrees(i, j) {
                    ag += 1;
                    let sj = fix[j].unwrap().1;
                    tight = Some(tight.map_or(sj, |t: f64| t.min(sj)));
                } else {
                    dis += 1;
                }
            }
            let verdict = if ag + dis == 0 {
                "no_votes"
            } else if ag >= 1 && dis <= ag {
                qualified.push((i, tight.unwrap()));
                "qualified"
            } else {
                "disqualified"
            };
            verdicts.push(SourceVerdict {
                source: s.id.clone(),
                verdict: verdict.into(),
                agree: ag,
                disagree: dis,
            });
        }
        let choice = qualified
            .iter()
            .min_by(|a, b| {
                fix[a.0]
                    .unwrap()
                    .1
                    .total_cmp(&fix[b.0].unwrap().1)
                    .then(a.0.cmp(&b.0))
            })
            .copied();
        // GNSS suspect: a GNSS-group source is ok but disqualified, and two other groups agree.
        let others: Vec<usize> = rep
            .iter()
            .filter(|(g, _)| **g != cfg.gnss_group)
            .map(|(_, &j)| j)
            .collect();
        let others_agree = others
            .iter()
            .enumerate()
            .any(|(a, &i)| others[a + 1..].iter().any(|&j| agrees(i, j)));
        let gnss_suspect = others_agree
            && cfg
                .sources
                .iter()
                .zip(&verdicts)
                .any(|(s, v)| s.group == cfg.gnss_group && v.verdict == "disqualified");
        let (lat, lon, tr, acc) = match choice {
            Some((i, tight)) => {
                let ((a, o), s) = fix[i].unwrap();
                (
                    Some(a),
                    Some(o),
                    Some(k * (s * s + tight * tight).sqrt()),
                    Some(R95_OVER_SIGMA * s),
                )
            }
            None => (None, None, None, None),
        };
        decisions.push(WwrnsDecision {
            t_s: *key as f64 / 1000.0,
            choice: choice.map(|(i, _)| cfg.sources[i].id.clone()),
            lat_deg: lat,
            lon_deg: lon,
            trust_radius_m: tr,
            accuracy_95_m: acc,
            meets_requirement: cfg
                .required_accuracy_95_m
                .map(|r| acc.is_some_and(|a| a <= r)),
            gnss_suspect,
            verdicts,
        });
    }
    let mut chosen = BTreeMap::new();
    for d in &decisions {
        if let Some(c) = &d.choice {
            *chosen.entry(c.clone()).or_insert(0) += 1;
        }
    }
    let switches = decisions
        .windows(2)
        .filter(|w| w[0].choice.is_some() && w[1].choice.is_some() && w[0].choice != w[1].choice)
        .count();
    Ok(WwrnsReport {
        format: FORMAT.into(),
        k,
        epochs: decisions.len(),
        no_choice: decisions.iter().filter(|d| d.choice.is_none()).count(),
        switches,
        gnss_suspect_epochs: decisions.iter().filter(|d| d.gnss_suspect).count(),
        meets_requirement_epochs: cfg
            .required_accuracy_95_m
            .map(|_| decisions.iter().filter(|d| d.meets_requirement == Some(true)).count()),
        required_accuracy_95_m: cfg.required_accuracy_95_m,
        requirement_source: cfg.requirement_source.clone(),
        chosen,
        decisions,
        evidence: "MODELLED: the F3 vote rule in the position domain on stated sigmas; the chosen source's own fix, never a blend".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> WwrnsConfig {
        let s = |id: &str| WwrnsSource {
            id: id.into(),
            group: id.into(),
        };
        WwrnsConfig {
            sources: vec![s("gnss"), s("rmode"), s("eloran")],
            p_fa: None,
            gnss_group: "gnss".into(),
            required_accuracy_95_m: Some(100.0),
            requirement_source: "test".into(),
        }
    }

    fn fx(t: f64, s: &str, dn_m: f64, sigma: f64) -> Fix {
        Fix {
            t_s: t,
            source: s.into(),
            ok: true,
            lat_deg: Some(55.0 + dn_m / 111_195.0),
            lon_deg: Some(13.0),
            sigma_m: Some(sigma),
        }
    }

    #[test]
    fn agreeing_sources_choose_gnss_and_a_push_is_flagged() {
        let mut f = vec![];
        for t in 0..5 {
            let t = t as f64;
            f.push(fx(t, "gnss", 2.0, 3.0));
            f.push(fx(t, "rmode", -10.0, 8.0));
            f.push(fx(t, "eloran", 5.0, 10.0));
        }
        // Epochs 5..9: GNSS pushed 800 m north.
        for t in 5..10 {
            let t = t as f64;
            f.push(fx(t, "gnss", 800.0, 3.0));
            f.push(fx(t, "rmode", -10.0, 8.0));
            f.push(fx(t, "eloran", 5.0, 10.0));
        }
        let r = run(&cfg(), &f).unwrap();
        assert_eq!(r.decisions[0].choice.as_deref(), Some("gnss"));
        assert_eq!(r.decisions[7].choice.as_deref(), Some("rmode"));
        assert_eq!(r.gnss_suspect_epochs, 5);
        assert_eq!(r.switches, 1);
        assert_eq!(r.meets_requirement_epochs, Some(10));
    }

    #[test]
    fn a_lone_source_has_no_votes_and_is_not_chosen() {
        let f = vec![fx(0.0, "gnss", 0.0, 3.0)];
        let r = run(&cfg(), &f).unwrap();
        assert_eq!(r.no_choice, 1);
        assert_eq!(r.decisions[0].verdicts[0].verdict, "no_votes");
    }
}
