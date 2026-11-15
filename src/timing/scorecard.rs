// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Timing scorecards for grandmasters and PTP networks under spoofing scenarios (roadmap
//! 0.42, T4).
//!
//! For each grandmaster of a timing fleet (T1 fleet file) and each timing scenario of the
//! threat library (E2 `[timing]` profiles: a time push, a time step, or a denial), the
//! scorecard asks what a GNSS-disciplined grandmaster and the PTP network behind it would
//! deliver:
//!
//! 1. **Detection.** The grandmaster compares its GNSS time with its own oscillator. The
//!    oscillator's holdover error from the handover is predicted by the T1 model (mean and
//!    1-sigma, [`super::model::HoldoverModel`]); a counterfeit offset is detected when it
//!    leaves the envelope `k_sigma * sigma(u) + |mean(u)| + floor_s`. Where the scenario
//!    reaches one receiver only and the site has a second, independent reference, a
//!    cross-check of the two detects when the offset exceeds `k_sigma * sqrt(r1^2 + r2^2) +
//!    floor_s` (`r`: each record's residual RMS). A denial is seen at once.
//! 2. **Delivered error.** Before detection the network follows the counterfeit time; after
//!    detection the grandmaster holds over on its oscillator from the error it had reached.
//!    Each downstream clock (the `[[network]]` links) receives the grandmaster's error plus its
//!    own present servo offset.
//! 3. **Verdicts per clock and limit** (the fleet's user-set limits): a limit breached before
//!    detection; a holdover warning (breach probability 5 %) before the threat ends.
//!
//! The verdicts are mapped to the resilience levels with the E3 scoring
//! ([`crate::scorecards::levels`]), as test evidence.
//!
//! **Honest limits.** The oscillator path is the T1 prediction (its mean, with the envelope),
//! not a recorded run under attack, and the T1 time to breach is not a validated claim (V5 and
//! V5b missed). Level 1 here assumes the clock re-locks to genuine signals once the threat is
//! removed and its alarm is acted on; a modelled run cannot show the re-lock.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::fleet::{self, ClockAssessment, FleetSpec};
use super::model::{norm_cdf, HoldoverModel};
use crate::scorecards::levels::{self, CaseLevels, LevelMapping, Verdict};
use crate::threat_library::{self, TimingProfile};

/// The scenario kind.
pub const KIND: &str = "pro-timing-scorecard";
/// The format of a timing scorecard.
pub const FORMAT: &str = "kshana-pro-timing-scorecard/1";
/// The method; the rules above are constants of this version.
pub const METHOD_ID: &str = "kshana-pro/timing-scorecard/v1";

/// The detector, stated before scoring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Detector {
    /// Envelope width in predicted sigmas.
    pub k_sigma: f64,
    /// The comparison's own measurement floor, s.
    pub floor_s: f64,
    /// Use a second reference of the same site as a cross-check where the scenario reaches
    /// one receiver only.
    pub cross_check: bool,
    /// Level 4: every delivered error within this fraction of the clock's tightest limit.
    pub full_fraction: f64,
}

impl Default for Detector {
    fn default() -> Self {
        Detector {
            k_sigma: 3.0,
            floor_s: 20e-9,
            cross_check: true,
            full_fraction: 0.1,
        }
    }
}

/// A downstream PTP link: `clock` follows `upstream`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub clock: String,
    pub upstream: String,
}

/// A timing scorecard request (`kind = "pro-timing-scorecard"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimingScorecardSpec {
    pub kind: String,
    pub title: String,
    /// The T1 fleet file, relative to this file.
    pub fleet: String,
    /// The grandmasters to score (fleet clock ids).
    pub grandmasters: Vec<String>,
    /// Second references per site for the cross-check: `[[pairs]] a = .., b = ..`.
    #[serde(default)]
    pub pairs: Vec<Pair>,
    /// Threat library scenario ids with a `[timing]` profile.
    pub scenarios: Vec<String>,
    #[serde(default)]
    pub network: Vec<Link>,
    #[serde(default)]
    pub detector: Detector,
    /// When the detector and bars were fixed (ISO-8601 UTC), as stated.
    #[serde(default)]
    pub fixed_utc: Option<String>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

/// Two independent references of one site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    pub a: String,
    pub b: String,
}

impl TimingScorecardSpec {
    pub fn parse(src: &str) -> Result<Self, KshanaError> {
        let s: Self = toml::from_str(src)
            .map_err(|e| KshanaError::InvalidInput(format!("invalid {KIND}: {e}")))?;
        if s.kind != KIND {
            return Err(KshanaError::InvalidInput(format!("kind must be '{KIND}'")));
        }
        let d = &s.detector;
        let ok = d.k_sigma.is_finite()
            && d.k_sigma > 0.0
            && d.floor_s.is_finite()
            && d.floor_s >= 0.0
            && d.full_fraction > 0.0
            && d.full_fraction <= 1.0;
        if !ok || s.grandmasters.is_empty() || s.scenarios.is_empty() {
            return Err(KshanaError::InvalidInput(
                "a timing scorecard needs grandmasters, scenarios and a valid detector".into(),
            ));
        }
        Ok(s)
    }

    pub fn from_file(path: &Path) -> Result<(Self, String), KshanaError> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", path.display())))?;
        let mut s = Self::parse(&src)?;
        s.base_dir = path.parent().map(Path::to_path_buf);
        Ok((s, src))
    }

    fn fleet_path(&self) -> PathBuf {
        let p = PathBuf::from(&self.fleet);
        match (&self.base_dir, p.is_absolute()) {
            (Some(b), false) => b.join(p),
            _ => p,
        }
    }

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description: "Timing scorecard (T4): grandmasters and the PTP network behind them \
under threat library timing scenarios (time push, time step, denial): detection against the T1 \
holdover envelope, delivered time error per clock and limit, holdover after detection; mapped to \
the resilience levels as test evidence.",
            required_fields: &["title", "fleet", "grandmasters", "scenarios"],
            optional_fields: &["pairs", "network", "detector", "fixed_utc"],
        }
    }
}

/// One clock's verdicts in one scenario.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClockVerdict {
    pub clock: String,
    pub role: Option<String>,
    /// `grandmaster` or `downstream (via <path>)`.
    pub position: String,
    pub tightest_limit: Option<String>,
    pub tightest_limit_s: Option<f64>,
    /// The clock's own present servo offset added to what it receives, s.
    pub own_offset_s: f64,
    /// Largest |time error| delivered before detection (or over the scenario, undetected), s.
    pub max_error_before_detection_s: f64,
    /// Per limit: breached before detection.
    pub breached_before_detection: BTreeMap<String, bool>,
    /// Per limit: holdover warning (5 % breach probability) before the threat ends, and when
    /// (s after detection).
    pub holdover_warning_s: BTreeMap<String, Option<f64>>,
    /// 95 % bound of the delivered error at the end of the threat, s.
    pub error_95_at_end_s: f64,
    pub levels: CaseLevels,
}

/// One grandmaster in one scenario.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScenarioResult {
    pub grandmaster: String,
    pub scenario: String,
    pub scenario_title: String,
    pub profile: TimingProfile,
    /// `own-envelope`, `cross-check`, `loss-of-signal` or `none`.
    pub detected_by: String,
    /// Detection time after onset, s (`None`: not detected within the threat).
    pub detection_s: Option<f64>,
    /// |counterfeit offset| at detection, s.
    pub offset_at_detection_s: f64,
    /// The envelope at detection (or at the end), s.
    pub envelope_s: f64,
    pub cross_check_threshold_s: Option<f64>,
    pub clocks: Vec<ClockVerdict>,
    pub levels: CaseLevels,
}

/// A timing scorecard.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TimingScorecard {
    pub format: &'static str,
    pub kind: &'static str,
    pub method: &'static str,
    pub label: String,
    pub title: String,
    pub fixed_utc: Option<String>,
    pub detector: Detector,
    pub spec_sha256: String,
    pub fleet_title: String,
    pub threat_library: String,
    pub results: Vec<ScenarioResult>,
    pub levels: LevelMapping,
    pub limits: Vec<String>,
}

/// The label of every timing scorecard.
pub fn label() -> String {
    format!(
        "MODELLED - grandmaster and network time error under threat library timing profiles, from each clock's T1 holdover model; detection against the model's envelope; {}. Test evidence for the scenarios run; not a certification.",
        super::v5_status()
    )
}

fn grid_step(duration_s: f64) -> f64 {
    (duration_s / 20_000.0).max(1.0)
}

/// Breach probability of `|e0 + TE(u)| >= limit`, TE ~ N(mean(u), sigma(u)).
fn p_breach(m: &HoldoverModel, e0: f64, u: f64, limit: f64) -> f64 {
    let mu = e0 + m.mean(u);
    let s = m.sigma(u);
    if s <= 0.0 {
        return if mu.abs() >= limit { 1.0 } else { 0.0 };
    }
    norm_cdf((-limit - mu) / s) + (1.0 - norm_cdf((limit - mu) / s))
}

fn tightest(spec: &FleetSpec, c: &ClockAssessment) -> Option<(String, f64)> {
    let fc = spec.clocks.iter().find(|x| x.id == c.id)?;
    spec.limits
        .iter()
        .filter(|l| fc.limits.as_ref().is_none_or(|n| n.contains(&l.name)))
        .min_by(|a, b| a.value_s.total_cmp(&b.value_s))
        .map(|l| (l.name.clone(), l.value_s))
}

fn limits_of<'a>(spec: &'a FleetSpec, id: &str) -> Vec<&'a fleet::Limit> {
    let fc = spec.clocks.iter().find(|x| x.id == id);
    spec.limits
        .iter()
        .filter(|l| {
            fc.and_then(|f| f.limits.as_ref())
                .is_none_or(|n| n.contains(&l.name))
        })
        .collect()
}

/// The downstream clocks of `gm`, with their path.
fn downstream(links: &[Link], gm: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut frontier = vec![(gm.to_string(), gm.to_string())];
    let mut seen = BTreeSet::from([gm.to_string()]);
    while let Some((up, path)) = frontier.pop() {
        for l in links.iter().filter(|l| l.upstream == up) {
            if seen.insert(l.clock.clone()) {
                let p = format!("{path} > {}", l.clock);
                out.push((l.clock.clone(), p.clone()));
                frontier.push((l.clock.clone(), p));
            }
        }
    }
    out.sort();
    out
}

/// Score one grandmaster under one profile.
#[allow(clippy::too_many_arguments)]
fn score_one(
    spec: &TimingScorecardSpec,
    fspec: &FleetSpec,
    a: &fleet::FleetAssessment,
    gm: &ClockAssessment,
    partner: Option<&ClockAssessment>,
    scen_id: &str,
    scen_title: &str,
    p: &TimingProfile,
) -> ScenarioResult {
    let d = &spec.detector;
    let m = &gm.holdover;
    let step = grid_step(p.duration_s);
    let n = (p.duration_s / step).ceil() as usize;
    let envelope = |u: f64| d.k_sigma * m.sigma(u) + m.mean(u).abs() + d.floor_s;
    let cross = (d.cross_check && p.scope == "one-receiver")
        .then_some(partner)
        .flatten()
        .map(|q| {
            d.k_sigma * (m.residual_rms_s.powi(2) + q.holdover.residual_rms_s.powi(2)).sqrt()
                + d.floor_s
        });
    let (mut det, mut by) = (None, "none");
    if p.kind == "denial" {
        det = Some(0.0);
        by = "loss-of-signal";
    } else {
        for k in 0..=n {
            let u = (k as f64 * step).min(p.duration_s);
            let off = p.offset(u).abs();
            if off > envelope(u) {
                det = Some(u);
                by = "own-envelope";
                break;
            }
            if cross.is_some_and(|x| off > x) {
                det = Some(u);
                by = "cross-check";
                break;
            }
        }
    }
    let u_det = det.unwrap_or(p.duration_s);
    let max_before = (0..=n)
        .map(|k| (k as f64 * step).min(p.duration_s))
        .filter(|u| det.is_none() || *u < u_det)
        .map(|u| p.offset(u).abs())
        .fold(0.0f64, f64::max);
    let e0 = if det.is_some() { p.offset(u_det) } else { 0.0 };
    let remaining = (p.duration_s - u_det).max(0.0);
    let mut members = vec![(gm.id.clone(), "grandmaster".to_string())];
    for (c, path) in downstream(&spec.network, &gm.id) {
        members.push((c, format!("downstream (via {path})")));
    }
    let mut clocks = Vec::new();
    for (cid, position) in members {
        let Some(ca) = a.clock(&cid) else { continue };
        let own = if ca.record == "disciplined" {
            ca.holdover.phase_s.abs()
        } else {
            0.0
        };
        let delivered = max_before + own;
        let mut breached = BTreeMap::new();
        let mut warn = BTreeMap::new();
        for l in limits_of(fspec, &cid) {
            breached.insert(l.name.clone(), delivered >= l.value_s);
            let w = if det.is_some() && remaining > 0.0 {
                let mut t = None;
                let hs = grid_step(remaining);
                let hn = (remaining / hs).ceil() as usize;
                for k in 1..=hn {
                    let u = (k as f64 * hs).min(remaining);
                    if p_breach(m, e0.abs() + own, u, l.value_s) >= 0.05 {
                        t = Some(u);
                        break;
                    }
                }
                t
            } else {
                None
            };
            warn.insert(l.name.clone(), w);
        }
        let end_95 = if det.is_some() {
            (e0.abs() + own + m.mean(remaining)).abs() + 1.96 * m.sigma(remaining)
        } else {
            p.offset(p.duration_s).abs() + own
        };
        let tl = tightest(fspec, ca);
        let no_mislead = Verdict::from_bool(Some(!breached.values().any(|b| *b)));
        let recover = Verdict::from_bool(Some(det.is_some()));
        let bounded = Verdict::from_bool(Some(det.is_some() && warn.values().all(Option::is_none)));
        let full = match &tl {
            Some((_, v)) => Verdict::from_bool(Some(
                det.is_some() && delivered.max(end_95) <= d.full_fraction * v,
            )),
            None => Verdict::NotAssessed,
        };
        clocks.push(ClockVerdict {
            clock: cid.clone(),
            role: ca.role.clone(),
            position,
            tightest_limit: tl.as_ref().map(|t| t.0.clone()),
            tightest_limit_s: tl.as_ref().map(|t| t.1),
            own_offset_s: own,
            max_error_before_detection_s: delivered,
            breached_before_detection: breached,
            holdover_warning_s: warn,
            error_95_at_end_s: end_95,
            levels: CaseLevels::from_criteria(recover, no_mislead, bounded, full),
        });
    }
    // The scenario's levels: every clock of the network must meet a level.
    let all = |lvl: u8| {
        clocks
            .iter()
            .fold(Verdict::Met, |acc, c| acc.and(c.levels.get(lvl)))
    };
    let levels = CaseLevels {
        l1: all(1),
        l2: all(2),
        l3: all(3),
        l4: all(4),
    };
    ScenarioResult {
        grandmaster: gm.id.clone(),
        scenario: scen_id.into(),
        scenario_title: scen_title.into(),
        profile: p.clone(),
        detected_by: by.into(),
        detection_s: det,
        offset_at_detection_s: e0.abs(),
        envelope_s: envelope(u_det),
        cross_check_threshold_s: cross,
        clocks,
        levels,
    }
}

/// Score a request. `src` is its file text (hashed into the scorecard).
pub fn score(spec: &TimingScorecardSpec, src: &str) -> Result<TimingScorecard, KshanaError> {
    let fpath = spec.fleet_path();
    let ftext = std::fs::read_to_string(&fpath)
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", fpath.display())))?;
    let mut fspec = FleetSpec::parse(&ftext)?;
    if let Some(b) = fpath.parent() {
        fspec.resolve_paths(b);
    }
    let a = fleet::assess(&fspec)?;
    let ids: BTreeSet<&str> = a.clocks.iter().map(|c| c.id.as_str()).collect();
    for id in spec
        .grandmasters
        .iter()
        .chain(spec.network.iter().flat_map(|l| [&l.clock, &l.upstream]))
        .chain(spec.pairs.iter().flat_map(|p| [&p.a, &p.b]))
    {
        if !ids.contains(id.as_str()) {
            return Err(KshanaError::InvalidInput(format!(
                "clock '{id}' is not in the fleet"
            )));
        }
    }
    let mut results = Vec::new();
    for sid in &spec.scenarios {
        let s = threat_library::get(sid)?;
        let p = s.timing.as_ref().ok_or_else(|| {
            KshanaError::InvalidInput(format!("threat scenario '{sid}' has no [timing] profile"))
        })?;
        for g in &spec.grandmasters {
            let gm = a.clock(g).expect("checked above");
            let partner = spec
                .pairs
                .iter()
                .find_map(|pr| {
                    if pr.a == *g {
                        Some(&pr.b)
                    } else if pr.b == *g {
                        Some(&pr.a)
                    } else {
                        None
                    }
                })
                .and_then(|id| a.clock(id));
            results.push(score_one(spec, &fspec, &a, gm, partner, &s.id, &s.title, p));
        }
    }
    let levels = levels::map(&results.iter().map(|r| r.levels.clone()).collect::<Vec<_>>());
    Ok(TimingScorecard {
        format: FORMAT,
        kind: KIND,
        method: METHOD_ID,
        label: label(),
        title: spec.title.clone(),
        fixed_utc: spec.fixed_utc.clone(),
        detector: spec.detector.clone(),
        spec_sha256: hex::encode(Sha256::digest(src.as_bytes())),
        fleet_title: a.title.clone(),
        threat_library: threat_library::EDITION.into(),
        results,
        levels,
        limits: vec![
            "The oscillator path is each clock's T1 holdover prediction (mean and envelope), not a recorded run under attack; the T1 time to breach is not a validated claim (V5 and V5b missed).".into(),
            "Level 1 assumes the clock re-locks to genuine signals once the threat is removed and its alarm is acted on; a modelled run cannot show the re-lock.".into(),
            "Downstream clocks receive the grandmaster's error plus their own present servo offset; path asymmetry and packet delay variation are not modelled.".into(),
            "Timing profiles are representative of published descriptions (threat library), not read off any recording.".into(),
        ],
    })
}

/// The HTML report.
pub fn html(s: &TimingScorecard) -> String {
    let esc = |t: &str| {
        t.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let ht = super::fleet::human_time;
    let mut h = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Timing scorecard: {}</title><style>body{{font-family:system-ui,sans-serif;max-width:1000px;margin:2rem auto;padding:0 16px;color:#1d2330;background:#fff}}table{{border-collapse:collapse;width:100%;font-size:.85rem;margin:.6rem 0}}th,td{{border:1px solid #d5dae3;padding:4px 6px;text-align:left;vertical-align:top}}th{{background:#f2f4f8}}.label{{background:#fff7e0;border:1px solid #e8d9a8;padding:.6rem;font-size:.86rem}}h2{{font-size:1.1rem;margin-top:1.8rem}}</style></head><body><h1>Timing scorecard</h1><p><b>{}</b> · fleet {} · threat library {}</p><p class=\"label\">{}</p>",
        esc(&s.title),
        esc(&s.title),
        esc(&s.fleet_title),
        esc(&s.threat_library),
        esc(&s.label)
    );
    h.push_str(&format!(
        "<h2>Resilience levels (test evidence)</h2><p>{}</p><p><b>Highest level supported for every grandmaster and scenario: {}</b></p><table><tr><th>Level</th><th>Summary</th><th>Evidence</th><th>Met / not met / not assessed</th></tr>",
        esc(&s.levels.wording),
        s.levels.highest_supported
    ));
    for l in &s.levels.levels {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} / {} / {}</td></tr>",
            l.level,
            esc(&l.summary),
            l.evidence,
            l.cases_met,
            l.cases_not_met,
            l.cases_not_assessed
        ));
    }
    h.push_str("</table><h2>Grandmasters under each scenario</h2><table><tr><th>Grandmaster</th><th>Scenario</th><th>Detected</th><th>Offset at detection</th><th>L1 · L2 · L3 · L4</th></tr>");
    for r in &s.results {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}<br><small>{}</small></td><td>{}</td><td>{}</td><td>{} · {} · {} · {}</td></tr>",
            esc(&r.grandmaster),
            esc(&r.scenario),
            esc(&r.scenario_title),
            match r.detection_s {
                Some(t) => format!("{} after {}", r.detected_by, super::fleet::human_duration(Some(t))),
                None => "not detected".into(),
            },
            ht(r.offset_at_detection_s),
            r.levels.l1.as_str(),
            r.levels.l2.as_str(),
            r.levels.l3.as_str(),
            r.levels.l4.as_str()
        ));
    }
    h.push_str("</table><h2>Clocks</h2><table><tr><th>Grandmaster · scenario</th><th>Clock</th><th>Tightest limit</th><th>Max error before detection</th><th>Breached before detection</th><th>Holdover warning before the threat ends</th><th>95 % error at end</th></tr>");
    for r in &s.results {
        for c in &r.clocks {
            let br: Vec<String> = c
                .breached_before_detection
                .iter()
                .filter(|(_, b)| **b)
                .map(|(k, _)| esc(k))
                .collect();
            let hw: Vec<String> = c
                .holdover_warning_s
                .iter()
                .filter_map(|(k, v)| {
                    v.map(|t| format!("{} after {}", esc(k), super::fleet::human_duration(Some(t))))
                })
                .collect();
            h.push_str(&format!(
                "<tr><td>{} · {}</td><td>{}<br><small>{}</small></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&r.grandmaster),
                esc(&r.scenario),
                esc(&c.clock),
                esc(&c.position),
                c.tightest_limit
                    .as_deref()
                    .map(|n| format!("{} ({})", esc(n), ht(c.tightest_limit_s.unwrap_or(0.0))))
                    .unwrap_or_else(|| "none".into()),
                ht(c.max_error_before_detection_s),
                if br.is_empty() { "none".into() } else { br.join(", ") },
                if hw.is_empty() { "none".into() } else { hw.join("; ") },
                ht(c.error_95_at_end_s)
            ));
        }
    }
    let d = &s.detector;
    h.push_str(&format!(
        "</table><h2>Detector stated before scoring</h2><p>Envelope {} sigma of the T1 prediction plus |mean| plus a floor of {}; cross-check {}; level 4 within {} of the tightest limit. Fixed: {}. Request SHA-256 <code>{}</code>; method <code>{}</code>.</p><h2>What this scorecard does not show</h2><ul>",
        d.k_sigma,
        ht(d.floor_s),
        if d.cross_check { "on where a site pair is given and the scenario reaches one receiver" } else { "off" },
        d.full_fraction,
        esc(s.fixed_utc.as_deref().unwrap_or("not stated")),
        s.spec_sha256,
        s.method
    ));
    for l in &s.limits {
        h.push_str(&format!("<li>{}</li>", esc(l)));
    }
    h.push_str("</ul><p><small>Verify offline with the free edition: <code>kshana-pro dossier verify &lt;dir&gt; --trust &lt;signer key&gt;</code>.</small></p></body></html>\n");
    h
}

/// Write the report directory: `timing-scorecard.json`, `report.html`, `request.toml`,
/// `README.txt`. Signing is a separate step.
pub fn write_dir(out: &Path, s: &TimingScorecard, src: &str) -> Result<(), KshanaError> {
    let io = |e: std::io::Error| KshanaError::InvalidInput(format!("{}: {e}", out.display()));
    if out.exists() && std::fs::read_dir(out).map_err(io)?.next().is_some() {
        return Err(KshanaError::InvalidInput(format!(
            "{}: not empty",
            out.display()
        )));
    }
    std::fs::create_dir_all(out).map_err(io)?;
    let json =
        serde_json::to_string_pretty(s).map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
    std::fs::write(out.join("timing-scorecard.json"), json + "\n").map_err(io)?;
    std::fs::write(out.join("report.html"), html(s)).map_err(io)?;
    std::fs::write(out.join("request.toml"), src).map_err(io)?;
    std::fs::write(
        out.join("README.txt"),
        format!(
            "Timing scorecard ({}).\n\n{}\n\ntiming-scorecard.json  the scorecard\nreport.html            the readable report\nrequest.toml           the request as run\nsignature.json         the signer's Ed25519 signature over every file\n\nVerify offline, no licence needed: kshana-pro dossier verify <this directory> --trust <signer key>\n",
            s.method, s.label
        ),
    )
    .map_err(io)?;
    Ok(())
}

impl Scenario for TimingScorecardSpec {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let src = toml::to_string(self).unwrap_or_default();
        let s = score(self, &src)?;
        let detected = s.results.iter().filter(|r| r.detection_s.is_some()).count();
        Ok(RunOutput {
            summary: format!(
                "Kshana Pro timing scorecard | {} grandmaster-scenario pairs, {} detected | highest level supported: {} | MODELLED",
                s.results.len(),
                detected,
                s.levels.highest_supported
            ),
            svg: format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"480\" height=\"72\" font-family=\"system-ui,sans-serif\"><rect width=\"480\" height=\"72\" fill=\"#0f1216\"/><text x=\"16\" y=\"30\" fill=\"#8cc4f5\" font-size=\"15\" font-weight=\"bold\">Timing scorecard · {} pairs</text><text x=\"16\" y=\"54\" fill=\"#9aa3af\" font-size=\"12\">{} detected · level {} · MODELLED</text></svg>",
                s.results.len(),
                detected,
                s.levels.highest_supported
            ),
            csv: Some(csv(&s)),
            json: serde_json::to_string_pretty(&s)
                .map_err(|e| KshanaError::InvalidInput(e.to_string()))?,
        })
    }
}

/// One row per grandmaster, scenario and clock.
pub fn csv(s: &TimingScorecard) -> String {
    let mut out = String::from("grandmaster,scenario,clock,detected_by,detection_s,max_error_before_detection_s,breached_before_detection,error_95_at_end_s\n");
    for r in &s.results {
        for c in &r.clocks {
            out.push_str(&format!(
                "{},{},{},{},{},{:.6e},{},{:.6e}\n",
                r.grandmaster,
                r.scenario,
                c.clock,
                r.detected_by,
                r.detection_s.map(|t| t.to_string()).unwrap_or_default(),
                c.max_error_before_detection_s,
                c.breached_before_detection.values().any(|b| *b),
                c.error_95_at_end_s
            ));
        }
    }
    out
}

impl ExternalPack for TimingScorecardSpec {
    fn kind_name(&self) -> &'static str {
        KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(KIND));
    }
}
