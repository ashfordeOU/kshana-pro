// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! B2002: space receiver qualification scorecards, the selection database and the
//! qualification process template.
//!
//! * The **campaign** (`kshana-space-rx-campaign/1`) records what was done to one receiver
//!   (vendor, model, firmware): total ionising dose steps with the navigation outputs at each
//!   step, single-event runs (particle, LET, fluence, events by type), vibration runs per axis,
//!   thermal and thermal-vacuum runs, tracking cases (orbit dynamics on a simulator) and
//!   interference runs (the J/S at which tracking held).
//! * The **mission profile** (`kshana-space-rx-mission/1`) states the limits a mission applies
//!   (dose with a radiation design margin, latch-up LET threshold, SEFI cross-section, position
//!   error, TTFF, vibration level, temperature range, tracking cases, J/S), each with a source.
//! * The **scorecard** checks the campaign against the profile: each check met, not met or not
//!   assessed (no data or no limit), with the figures. It is evidence for a part selection or a
//!   review; it is never a statement that the part is fit for flight.
//! * The **selection database** (`kshana-space-rx-db/1`) keeps the test-backed figures of
//!   each campaign and the scorecards run, so receivers can be compared on what was measured
//!   rather than on datasheet claims.
//! * The **template** is a qualification process outline in the shape of published GNSS
//!   payload campaigns (phases from component screening to acceptance), with each evidence
//!   slot filled from a campaign or marked OPEN. Standards are cited by number only.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{round, sha256_hex, Check, LABEL};

pub const CAMPAIGN_FORMAT: &str = "kshana-space-rx-campaign/1";
pub const MISSION_FORMAT: &str = "kshana-space-rx-mission/1";
pub const SCORECARD_SCHEMA: &str = "kshana-space-rx-scorecard/1";
pub const DB_FORMAT: &str = "kshana-space-rx-db/1";

/// The receiver a campaign tested.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverId {
    pub vendor: String,
    pub model: String,
    #[serde(default)]
    pub firmware: String,
    /// `cots`, `newspace` or `hirel`.
    pub class: String,
    #[serde(default)]
    pub constellations: Vec<String>,
    #[serde(default)]
    pub signals: Vec<String>,
    #[serde(default)]
    pub mass_g: Option<f64>,
    #[serde(default)]
    pub power_w: Option<f64>,
}

/// One total-ionising-dose step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TidStep {
    pub dose_krad: f64,
    pub functional: bool,
    #[serde(default)]
    pub position_error_m: Option<f64>,
    #[serde(default)]
    pub cn0_mean_dbhz: Option<f64>,
    #[serde(default)]
    pub ttff_s: Option<f64>,
    #[serde(default)]
    pub current_ma: Option<f64>,
}

/// One single-event run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeeRun {
    /// `heavy-ion` or `proton`.
    pub particle: String,
    #[serde(default)]
    pub ion: String,
    /// LET for heavy ions (MeV cm2/mg); 0 for protons.
    #[serde(default)]
    pub let_mev_cm2_mg: f64,
    #[serde(default)]
    pub energy_mev: Option<f64>,
    pub fluence_cm2: f64,
    #[serde(default)]
    pub latchup: u32,
    #[serde(default)]
    pub sefi: u32,
    #[serde(default)]
    pub reset: u32,
    #[serde(default)]
    pub seu: u32,
}

/// One vibration run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VibRun {
    pub axis: String,
    pub grms: f64,
    #[serde(default)]
    pub duration_s: f64,
    /// Whether tracking was lost during the run (receiver powered and tracking).
    pub tracking_lost: bool,
    #[serde(default)]
    pub cn0_drop_db: Option<f64>,
    pub functional_after: bool,
}

/// One thermal or thermal-vacuum run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThermalRun {
    pub temp_c: f64,
    #[serde(default)]
    pub cycles: u32,
    #[serde(default)]
    pub vacuum: bool,
    pub functional: bool,
    #[serde(default)]
    pub ttff_s: Option<f64>,
    #[serde(default)]
    pub position_error_m: Option<f64>,
}

/// One tracking case on a simulator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackRun {
    pub case: String,
    pub functional: bool,
    #[serde(default)]
    pub ttff_s: Option<f64>,
    #[serde(default)]
    pub position_error_m: Option<f64>,
}

/// One interference run: the J/S applied and whether tracking held.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsRun {
    pub js_db: f64,
    pub tracking_held: bool,
    #[serde(default)]
    pub waveform: String,
}

/// A campaign (`kshana-space-rx-campaign/1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Campaign {
    pub format: String,
    pub campaign_id: String,
    /// `synthetic`, `public` (from a cited publication) or `own` (the user's lab).
    pub data_class: String,
    pub source: String,
    #[serde(default)]
    pub lab: String,
    #[serde(default)]
    pub tid_source: String,
    #[serde(default)]
    pub dose_rate_rad_s: Option<f64>,
    pub receiver: ReceiverId,
    #[serde(default)]
    pub tid: Vec<TidStep>,
    #[serde(default)]
    pub see: Vec<SeeRun>,
    #[serde(default)]
    pub vibration: Vec<VibRun>,
    #[serde(default)]
    pub thermal: Vec<ThermalRun>,
    #[serde(default)]
    pub tracking: Vec<TrackRun>,
    #[serde(default)]
    pub jamming: Vec<JsRun>,
}

fn d_rdm() -> f64 {
    2.0
}

/// A mission profile (`kshana-space-rx-mission/1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mission {
    pub format: String,
    pub name: String,
    #[serde(default)]
    pub tid_krad: Option<f64>,
    /// Radiation design margin applied to the dose.
    #[serde(default = "d_rdm")]
    pub rdm: f64,
    /// No latch-up below this LET (MeV cm2/mg).
    #[serde(default)]
    pub latchup_let_min: Option<f64>,
    #[serde(default)]
    pub sefi_cross_section_max_cm2: Option<f64>,
    #[serde(default)]
    pub max_position_error_m: Option<f64>,
    #[serde(default)]
    pub max_ttff_s: Option<f64>,
    #[serde(default)]
    pub max_current_increase_pct: Option<f64>,
    #[serde(default)]
    pub vib_grms: Option<f64>,
    #[serde(default)]
    pub max_cn0_drop_db: Option<f64>,
    #[serde(default)]
    pub temp_min_c: Option<f64>,
    #[serde(default)]
    pub temp_max_c: Option<f64>,
    #[serde(default)]
    pub thermal_cycles: Option<u32>,
    #[serde(default)]
    pub vacuum_required: bool,
    #[serde(default)]
    pub tracking_cases: Vec<String>,
    #[serde(default)]
    pub min_tested_js_db: Option<f64>,
    /// Where each limit comes from (field name to source).
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
}

/// The claim words outputs never use (built so this file does not contain them).
pub fn claim_words() -> [String; 4] {
    [
        format!("{}{}", "compl", "iant"),
        format!("{}{}", "con", "form"),
        format!("{}{}", "certi", "fied"),
        format!("{}{}", "quali", "fied"),
    ]
}

fn no_claims(what: &str, text: &str) -> Result<(), String> {
    let l = text.to_lowercase();
    for w in claim_words() {
        if l.contains(&w) {
            return Err(format!(
                "{what}: uses the word '{w}'; state evidence, not a claim"
            ));
        }
    }
    Ok(())
}

impl Campaign {
    pub fn parse(src: &str) -> Result<Self, String> {
        let c: Campaign = toml::from_str(src).map_err(|e| format!("campaign: {e}"))?;
        if c.format != CAMPAIGN_FORMAT {
            return Err(format!("campaign: format must be \"{CAMPAIGN_FORMAT}\""));
        }
        if !["synthetic", "public", "own"].contains(&c.data_class.as_str()) {
            return Err("campaign: data_class must be synthetic, public or own".into());
        }
        if !["cots", "newspace", "hirel"].contains(&c.receiver.class.as_str()) {
            return Err("campaign: receiver.class must be cots, newspace or hirel".into());
        }
        if c.source.trim().is_empty() {
            return Err("campaign: source is required (a citation, or SYNTHETIC)".into());
        }
        if c.tid.windows(2).any(|w| w[1].dose_krad < w[0].dose_krad) {
            return Err("campaign: tid steps must be in increasing dose".into());
        }
        for r in &c.see {
            if r.fluence_cm2 <= 0.0 {
                return Err("campaign: see.fluence_cm2 must be positive".into());
            }
            if r.particle != "heavy-ion" && r.particle != "proton" {
                return Err("campaign: see.particle must be heavy-ion or proton".into());
            }
        }
        no_claims("campaign", src)?;
        Ok(c)
    }
    pub fn sha256(&self) -> String {
        sha256_hex(toml::to_string(self).unwrap_or_default().as_bytes())
    }
}

impl Mission {
    pub fn parse(src: &str) -> Result<Self, String> {
        let m: Mission = toml::from_str(src).map_err(|e| format!("mission: {e}"))?;
        if m.format != MISSION_FORMAT {
            return Err(format!("mission: format must be \"{MISSION_FORMAT}\""));
        }
        if m.rdm < 1.0 {
            return Err("mission: rdm must be at least 1".into());
        }
        no_claims("mission", src)?;
        Ok(m)
    }
    pub fn sha256(&self) -> String {
        sha256_hex(toml::to_string(self).unwrap_or_default().as_bytes())
    }
    fn src(&self, k: &str) -> String {
        self.sources
            .get(k)
            .cloned()
            .unwrap_or_else(|| "mission profile".into())
    }
}

/// The test-backed figures of a campaign (what the database keeps).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Specs {
    /// Highest dose with every step up to it functional, krad(Si).
    pub tid_functional_krad: Option<f64>,
    /// Lowest LET with a latch-up observed (MeV cm2/mg).
    pub latchup_let_observed: Option<f64>,
    /// Highest LET tested.
    pub let_max_tested: Option<f64>,
    /// Largest SEFI cross-section measured (cm2), with upper bound from zero counts left out.
    pub sefi_cross_section_max_cm2: Option<f64>,
    /// Highest vibration level with the receiver functional after.
    pub vib_grms_functional: Option<f64>,
    /// Temperature range with the receiver functional.
    pub temp_functional_c: Option<[f64; 2]>,
    /// Highest J/S with tracking held.
    pub js_held_db: Option<f64>,
    pub tracking_cases_functional: Vec<String>,
}

/// The figures a campaign supports.
pub fn specs(c: &Campaign) -> Specs {
    let mut s = Specs::default();
    let mut tid = None;
    for t in &c.tid {
        if !t.functional {
            break;
        }
        tid = Some(t.dose_krad);
    }
    s.tid_functional_krad = tid;
    let hi: Vec<&SeeRun> = c.see.iter().filter(|r| r.particle == "heavy-ion").collect();
    s.latchup_let_observed = hi
        .iter()
        .filter(|r| r.latchup > 0)
        .map(|r| r.let_mev_cm2_mg)
        .reduce(f64::min);
    s.let_max_tested = hi.iter().map(|r| r.let_mev_cm2_mg).reduce(f64::max);
    s.sefi_cross_section_max_cm2 = c
        .see
        .iter()
        .filter(|r| r.sefi > 0)
        .map(|r| r.sefi as f64 / r.fluence_cm2)
        .reduce(f64::max);
    s.vib_grms_functional = c
        .vibration
        .iter()
        .filter(|v| v.functional_after)
        .map(|v| v.grms)
        .reduce(f64::max);
    let ok: Vec<f64> = c
        .thermal
        .iter()
        .filter(|t| t.functional)
        .map(|t| t.temp_c)
        .collect();
    if !ok.is_empty() {
        s.temp_functional_c = Some([
            ok.iter().copied().fold(f64::MAX, f64::min),
            ok.iter().copied().fold(f64::MIN, f64::max),
        ]);
    }
    s.js_held_db = c
        .jamming
        .iter()
        .filter(|j| j.tracking_held)
        .map(|j| j.js_db)
        .reduce(f64::max);
    s.tracking_cases_functional = c
        .tracking
        .iter()
        .filter(|t| t.functional)
        .map(|t| t.case.clone())
        .collect();
    s
}

/// A scorecard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub schema: String,
    pub label: String,
    pub evidence: String,
    pub receiver: ReceiverId,
    pub campaign_id: String,
    pub campaign_sha256: String,
    pub data_class: String,
    pub campaign_source: String,
    pub mission: String,
    pub mission_sha256: String,
    pub specs: Specs,
    pub checks: Vec<Check>,
    pub met: usize,
    pub not_met: usize,
    pub not_assessed: usize,
}

fn fmt_opt(x: Option<f64>) -> String {
    x.map_or("none".into(), |v| format!("{}", round(v, 3)))
}

/// Score a campaign against a mission profile.
pub fn score(c: &Campaign, m: &Mission) -> Scorecard {
    let sp = specs(c);
    let mut ch = Vec::new();
    // TID: functional to the required dose times the design margin.
    let need = m.tid_krad.map(|d| d * m.rdm);
    ch.push(match (need, c.tid.is_empty()) {
        (None, _) => Check::new(
            "tid.functional",
            None,
            "no dose limit in the mission profile",
        ),
        (Some(_), true) => Check::new("tid.functional", None, "no dose steps in the campaign"),
        (Some(n), false) => {
            let got = sp.tid_functional_krad.unwrap_or(0.0);
            let failed_below = c.tid.iter().any(|t| !t.functional && t.dose_krad <= n);
            let verdict = if got >= n {
                Some(true)
            } else if failed_below {
                Some(false)
            } else {
                None
            };
            Check::new(
                "tid.functional",
                verdict,
                format!(
                    "functional to {} krad(Si); needed {} x RDM {} = {} krad(Si) ({}){}",
                    round(got, 2),
                    fmt_opt(m.tid_krad),
                    m.rdm,
                    round(n, 2),
                    m.src("tid_krad"),
                    if verdict.is_none() {
                        "; campaign stopped below the needed dose"
                    } else {
                        ""
                    }
                ),
            )
        }
    });
    // Navigation outputs over the dose steps up to the needed dose.
    let in_range: Vec<&TidStep> = c
        .tid
        .iter()
        .filter(|t| need.is_none_or(|n| t.dose_krad <= n))
        .collect();
    let worst = |f: &dyn Fn(&TidStep) -> Option<f64>| -> Option<f64> {
        in_range.iter().filter_map(|t| f(t)).reduce(f64::max)
    };
    for (id, lim, key, got, unit) in [
        (
            "tid.position_error",
            m.max_position_error_m,
            "max_position_error_m",
            worst(&|t| t.position_error_m),
            "m",
        ),
        (
            "tid.ttff",
            m.max_ttff_s,
            "max_ttff_s",
            worst(&|t| t.ttff_s),
            "s",
        ),
    ] {
        ch.push(match (lim, got) {
            (Some(l), Some(g)) => Check::new(
                id,
                Some(g <= l),
                format!(
                    "worst over the dose steps {} {unit}; limit {l} {unit} ({})",
                    round(g, 3),
                    m.src(key)
                ),
            ),
            (None, _) => Check::new(id, None, "no limit in the mission profile"),
            (_, None) => Check::new(id, None, "not recorded at the dose steps"),
        });
    }
    let i0 = c.tid.first().and_then(|t| t.current_ma);
    let i1 = in_range.last().and_then(|t| t.current_ma);
    ch.push(match (m.max_current_increase_pct, i0, i1) {
        (Some(l), Some(a), Some(b)) if a > 0.0 => {
            let pct = (b - a) / a * 100.0;
            Check::new(
                "tid.current",
                Some(pct <= l),
                format!(
                    "supply current {a} -> {b} mA ({} %); limit {l} % ({})",
                    round(pct, 1),
                    m.src("max_current_increase_pct")
                ),
            )
        }
        (None, _, _) => Check::new("tid.current", None, "no limit in the mission profile"),
        _ => Check::new("tid.current", None, "supply current not recorded"),
    });
    // SEE.
    ch.push(match (m.latchup_let_min, sp.let_max_tested) {
        (None, _) => Check::new(
            "see.latchup",
            None,
            "no latch-up threshold in the mission profile",
        ),
        (Some(_), None) => Check::new("see.latchup", None, "no heavy-ion runs in the campaign"),
        (Some(th), Some(max)) => match sp.latchup_let_observed {
            Some(l) if l < th => Check::new(
                "see.latchup",
                Some(false),
                format!(
                    "latch-up observed at LET {l} MeV cm2/mg, below the threshold {th} ({})",
                    m.src("latchup_let_min")
                ),
            ),
            _ if max >= th => Check::new(
                "see.latchup",
                Some(true),
                format!(
                    "no latch-up below LET {th} MeV cm2/mg; tested to {max} ({})",
                    m.src("latchup_let_min")
                ),
            ),
            _ => Check::new(
                "see.latchup",
                None,
                format!("tested only to LET {max} MeV cm2/mg; threshold {th}"),
            ),
        },
    });
    ch.push(match (m.sefi_cross_section_max_cm2, c.see.is_empty()) {
        (None, _) => Check::new("see.sefi", None, "no SEFI limit in the mission profile"),
        (Some(_), true) => Check::new("see.sefi", None, "no single-event runs in the campaign"),
        (Some(l), false) => {
            let got = sp.sefi_cross_section_max_cm2.unwrap_or(0.0);
            Check::new(
                "see.sefi",
                Some(got <= l),
                format!(
                    "largest SEFI cross-section {got:.3e} cm2 (0 = no SEFI seen); limit {l:.3e} cm2 ({})",
                    m.src("sefi_cross_section_max_cm2")
                ),
            )
        }
    });
    // Vibration.
    ch.push(match m.vib_grms {
        None => Check::new("vibration", None, "no vibration level in the mission profile"),
        Some(g) => {
            let runs: Vec<&VibRun> = c.vibration.iter().filter(|v| v.grms >= g).collect();
            if runs.is_empty() {
                Check::new(
                    "vibration",
                    None,
                    format!("no run at or above {g} gRMS"),
                )
            } else {
                let drop_ok = |v: &VibRun| match (m.max_cn0_drop_db, v.cn0_drop_db) {
                    (Some(l), Some(d)) => d <= l,
                    _ => true,
                };
                let bad: Vec<String> = runs
                    .iter()
                    .filter(|v| !v.functional_after || v.tracking_lost || !drop_ok(v))
                    .map(|v| v.axis.clone())
                    .collect();
                Check::new(
                    "vibration",
                    Some(bad.is_empty()),
                    format!(
                        "{} run(s) at or above {g} gRMS ({}); {}",
                        runs.len(),
                        m.src("vib_grms"),
                        if bad.is_empty() {
                            "tracking held and functional after on every axis".to_string()
                        } else {
                            format!("lost tracking, C/N0 drop over limit or not functional after on axis {}", bad.join(", "))
                        }
                    ),
                )
            }
        }
    });
    // Thermal.
    ch.push(match (m.temp_min_c, m.temp_max_c) {
        (None, None) => Check::new(
            "thermal.range",
            None,
            "no temperature range in the mission profile",
        ),
        (lo, hi) => {
            let cover_lo = lo.map(|l| c.thermal.iter().any(|t| t.temp_c <= l));
            let cover_hi = hi.map(|h| c.thermal.iter().any(|t| t.temp_c >= h));
            let fail = c.thermal.iter().any(|t| {
                !t.functional
                    && lo.is_none_or(|l| t.temp_c >= l)
                    && hi.is_none_or(|h| t.temp_c <= h)
            });
            let fail_ext = c.thermal.iter().any(|t| {
                !t.functional
                    && (lo.is_some_and(|l| t.temp_c <= l) || hi.is_some_and(|h| t.temp_c >= h))
            });
            let covered = cover_lo.unwrap_or(true) && cover_hi.unwrap_or(true);
            let v = if fail || fail_ext {
                Some(false)
            } else if covered {
                Some(true)
            } else {
                None
            };
            Check::new(
                "thermal.range",
                v,
                format!(
                    "functional range tested {:?} degC; needed {} to {} degC ({})",
                    sp.temp_functional_c,
                    fmt_opt(lo),
                    fmt_opt(hi),
                    m.src("temp_min_c")
                ),
            )
        }
    });
    let cycles: u32 = c.thermal.iter().map(|t| t.cycles).sum();
    ch.push(match m.thermal_cycles {
        None => Check::new(
            "thermal.cycles",
            None,
            "no cycle count in the mission profile",
        ),
        Some(n) => Check::new(
            "thermal.cycles",
            if cycles >= n {
                Some(true)
            } else if c.thermal.is_empty() {
                None
            } else {
                Some(false)
            },
            format!(
                "{cycles} cycles run; needed {n} ({})",
                m.src("thermal_cycles")
            ),
        ),
    });
    if m.vacuum_required {
        let v = c.thermal.iter().any(|t| t.vacuum && t.functional);
        ch.push(Check::new(
            "thermal.vacuum",
            if v {
                Some(true)
            } else if c.thermal.iter().any(|t| t.vacuum) {
                Some(false)
            } else {
                None
            },
            if v {
                "functional in thermal vacuum".to_string()
            } else {
                "no functional thermal-vacuum run".to_string()
            },
        ));
    }
    // Tracking cases.
    for case in &m.tracking_cases {
        let run = c.tracking.iter().find(|t| &t.case == case);
        let id = format!("tracking.{case}");
        ch.push(match run {
            None => Check::new(&id, None, "case not run"),
            Some(r) => {
                let err_ok = match (m.max_position_error_m, r.position_error_m) {
                    (Some(l), Some(e)) => e <= l,
                    _ => true,
                };
                let ttff_ok = match (m.max_ttff_s, r.ttff_s) {
                    (Some(l), Some(t)) => t <= l,
                    _ => true,
                };
                Check::new(
                    &id,
                    Some(r.functional && err_ok && ttff_ok),
                    format!(
                        "functional {}; position error {} m; TTFF {} s",
                        r.functional,
                        fmt_opt(r.position_error_m),
                        fmt_opt(r.ttff_s)
                    ),
                )
            }
        });
    }
    // Interference.
    ch.push(match m.min_tested_js_db {
        None => Check::new(
            "interference.js",
            None,
            "no J/S level in the mission profile",
        ),
        Some(l) => match sp.js_held_db {
            Some(g) => Check::new(
                "interference.js",
                Some(g >= l),
                format!(
                    "tracking held to J/S {g} dB; needed {l} dB ({})",
                    m.src("min_tested_js_db")
                ),
            ),
            None if c.jamming.is_empty() => {
                Check::new("interference.js", None, "no interference runs")
            }
            None => Check::new(
                "interference.js",
                Some(false),
                "tracking did not hold in any interference run",
            ),
        },
    });
    let count = |v: &str| ch.iter().filter(|c| c.verdict == v).count();
    Scorecard {
        schema: SCORECARD_SCHEMA.into(),
        label: LABEL.into(),
        evidence: if c.data_class == "synthetic" {
            "MODELLED (SYNTHETIC campaign)".into()
        } else {
            format!("MODELLED scoring of {} test data", c.data_class)
        },
        receiver: c.receiver.clone(),
        campaign_id: c.campaign_id.clone(),
        campaign_sha256: c.sha256(),
        data_class: c.data_class.clone(),
        campaign_source: c.source.clone(),
        mission: m.name.clone(),
        mission_sha256: m.sha256(),
        specs: sp,
        met: count("met"),
        not_met: count("not_met"),
        not_assessed: count("not_assessed"),
        checks: ch,
    }
}

/// A scorecard as Markdown.
pub fn scorecard_md(s: &Scorecard) -> String {
    let mut m = format!(
        "# Receiver scorecard: {} {} {}\n\nMission profile: {}. Campaign {} ({}; {}).\n\n{}\n\n**{} met, {} not met, {} not assessed.**\n\n| Check | Verdict | Detail |\n|---|---|---|\n",
        s.receiver.vendor,
        s.receiver.model,
        s.receiver.firmware,
        s.mission,
        s.campaign_id,
        s.data_class,
        s.campaign_source,
        s.evidence,
        s.met,
        s.not_met,
        s.not_assessed
    );
    for c in &s.checks {
        m.push_str(&format!(
            "| {} | {} | {} |\n",
            c.id,
            c.verdict,
            c.detail.replace('|', "/")
        ));
    }
    m.push_str(&format!("\n{}\n", s.label));
    m
}

/// The selection database.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Db {
    pub format: String,
    pub entries: Vec<DbEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DbEntry {
    pub receiver: ReceiverId,
    pub campaign_id: String,
    pub campaign_sha256: String,
    pub data_class: String,
    pub source: String,
    pub specs: Specs,
    pub scorecards: Vec<DbScore>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DbScore {
    pub mission: String,
    pub mission_sha256: String,
    pub met: usize,
    pub not_met: usize,
    pub not_assessed: usize,
}

impl Db {
    pub fn new() -> Self {
        Self {
            format: DB_FORMAT.into(),
            entries: Vec::new(),
        }
    }
    pub fn parse(src: &str) -> Result<Self, String> {
        let d: Db = serde_json::from_str(src).map_err(|e| format!("database: {e}"))?;
        if d.format != DB_FORMAT {
            return Err(format!("database: format must be \"{DB_FORMAT}\""));
        }
        Ok(d)
    }
    /// Add a scorecard: one entry per campaign hash; a scorecard for the same mission
    /// profile hash replaces the earlier one.
    pub fn add(&mut self, s: &Scorecard) {
        let score = DbScore {
            mission: s.mission.clone(),
            mission_sha256: s.mission_sha256.clone(),
            met: s.met,
            not_met: s.not_met,
            not_assessed: s.not_assessed,
        };
        if let Some(e) = self
            .entries
            .iter_mut()
            .find(|e| e.campaign_sha256 == s.campaign_sha256)
        {
            e.scorecards
                .retain(|x| x.mission_sha256 != s.mission_sha256);
            e.scorecards.push(score);
        } else {
            self.entries.push(DbEntry {
                receiver: s.receiver.clone(),
                campaign_id: s.campaign_id.clone(),
                campaign_sha256: s.campaign_sha256.clone(),
                data_class: s.data_class.clone(),
                source: s.campaign_source.clone(),
                specs: s.specs.clone(),
                scorecards: vec![score],
            });
        }
        self.entries.sort_by(|a, b| {
            (&a.receiver.vendor, &a.receiver.model, &a.campaign_id).cmp(&(
                &b.receiver.vendor,
                &b.receiver.model,
                &b.campaign_id,
            ))
        });
    }
    /// Entries whose test-backed figures reach the given dose and J/S (a selection query).
    pub fn select(&self, min_tid_krad: Option<f64>, min_js_db: Option<f64>) -> Vec<&DbEntry> {
        self.entries
            .iter()
            .filter(|e| {
                min_tid_krad.is_none_or(|d| e.specs.tid_functional_krad.unwrap_or(0.0) >= d)
            })
            .filter(|e| min_js_db.is_none_or(|j| e.specs.js_held_db.is_some_and(|x| x >= j)))
            .collect()
    }
}

/// The published JSON Schema of the database.
pub fn db_schema() -> Value {
    let num = json!({"type": ["number", "null"]});
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "kshana-space-rx-db/1",
        "title": "Space GNSS receiver selection database (test-backed figures)",
        "type": "object",
        "required": ["format", "entries"],
        "properties": {
            "format": {"const": DB_FORMAT},
            "entries": {"type": "array", "items": {
                "type": "object",
                "required": ["receiver", "campaign_id", "campaign_sha256", "data_class", "source", "specs", "scorecards"],
                "properties": {
                    "receiver": {"type": "object", "required": ["vendor", "model", "class"], "properties": {
                        "vendor": {"type": "string"}, "model": {"type": "string"}, "firmware": {"type": "string"},
                        "class": {"enum": ["cots", "newspace", "hirel"]},
                        "constellations": {"type": "array", "items": {"type": "string"}},
                        "signals": {"type": "array", "items": {"type": "string"}},
                        "mass_g": num, "power_w": num
                    }},
                    "campaign_id": {"type": "string"},
                    "campaign_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "data_class": {"enum": ["synthetic", "public", "own"]},
                    "source": {"type": "string"},
                    "specs": {"type": "object", "properties": {
                        "tid_functional_krad": num, "latchup_let_observed": num, "let_max_tested": num,
                        "sefi_cross_section_max_cm2": num, "vib_grms_functional": num,
                        "temp_functional_c": {"type": ["array", "null"], "items": {"type": "number"}, "minItems": 2, "maxItems": 2},
                        "js_held_db": num,
                        "tracking_cases_functional": {"type": "array", "items": {"type": "string"}}
                    }},
                    "scorecards": {"type": "array", "items": {"type": "object",
                        "required": ["mission", "mission_sha256", "met", "not_met", "not_assessed"],
                        "properties": {"mission": {"type": "string"}, "mission_sha256": {"type": "string"},
                            "met": {"type": "integer"}, "not_met": {"type": "integer"}, "not_assessed": {"type": "integer"}}}}
                }
            }}
        }
    })
}

/// The qualification process template, populated from a campaign and a scorecard.
pub fn template_md(c: &Campaign, s: &Scorecard) -> String {
    let verdict = |id: &str| -> String {
        s.checks
            .iter()
            .filter(|x| x.id == id || x.id.starts_with(&format!("{id}.")))
            .map(|x| format!("{} {}", x.id, x.verdict))
            .collect::<Vec<_>>()
            .join("; ")
    };
    let open = "OPEN — no evidence in this campaign";
    let slot = |have: bool, text: String| if have { text } else { open.to_string() };
    let rows: Vec<(&str, &str, &str, String)> = vec![
        (
            "1 Requirements",
            "Mission profile and limits with sources",
            "mission profile",
            format!("{} (sha256 {})", s.mission, &s.mission_sha256[..16]),
        ),
        (
            "2 Part selection",
            "Receiver identity, class, heritage and datasheet figures",
            "selection database",
            format!(
                "{} {} {} ({})",
                c.receiver.vendor, c.receiver.model, c.receiver.firmware, c.receiver.class
            ),
        ),
        (
            "3 Radiation: TID",
            "Dose steps with navigation outputs, supply current, annealing",
            "ECSS-Q-ST-60-15C; ESCC 22900",
            slot(
                !c.tid.is_empty(),
                format!(
                    "{} steps, {} source, {}; {}",
                    c.tid.len(),
                    if c.tid_source.is_empty() {
                        "unstated"
                    } else {
                        &c.tid_source
                    },
                    c.dose_rate_rad_s
                        .map_or("dose rate unstated".into(), |r| format!("{r} rad/s")),
                    verdict("tid")
                ),
            ),
        ),
        (
            "4 Radiation: SEE",
            "Heavy-ion and proton runs: latch-up, SEFI, resets, upsets",
            "ECSS-Q-ST-60-15C; ESCC 25100",
            slot(
                !c.see.is_empty(),
                format!("{} runs; {}", c.see.len(), verdict("see")),
            ),
        ),
        (
            "5 Mechanical",
            "Random vibration per axis with the receiver tracking",
            "ECSS-E-ST-10-03C",
            slot(
                !c.vibration.is_empty(),
                format!("{} runs; {}", c.vibration.len(), verdict("vibration")),
            ),
        ),
        (
            "6 Thermal",
            "Thermal cycling and thermal vacuum with TTFF and accuracy",
            "ECSS-E-ST-10-03C",
            slot(
                !c.thermal.is_empty(),
                format!("{} runs; {}", c.thermal.len(), verdict("thermal")),
            ),
        ),
        (
            "7 Functional and performance",
            "RF constellation simulator: orbit dynamics, TTFF, accuracy",
            "mission profile",
            slot(
                !c.tracking.is_empty(),
                format!("{} cases; {}", c.tracking.len(), verdict("tracking")),
            ),
        ),
        (
            "8 Interference robustness",
            "J/S steps on a simulator; levels from the ground-interference envelope",
            "mission profile",
            slot(
                !c.jamming.is_empty(),
                format!("{} runs; {}", c.jamming.len(), verdict("interference")),
            ),
        ),
        (
            "9 EMC",
            "Conducted and radiated emissions and susceptibility",
            "ECSS-E-ST-20-07C",
            open.to_string(),
        ),
        (
            "10 Acceptance",
            "Flight-unit acceptance tests and review",
            "ECSS-E-ST-10-03C",
            open.to_string(),
        ),
    ];
    let mut m = format!(
        "# Qualification process template: {} {}\n\nStructure modelled on published GNSS payload qualification campaigns (for example the LuGRE payload paper, Politecnico di Torino, 2023) for an Earth-orbit mission. Standards are cited by number only; their requirements are not reproduced. Each evidence slot is filled from campaign {} ({}) or marked OPEN.\n\n{}\n\n| Phase | Activity | Reference | Evidence |\n|---|---|---|---|\n",
        c.receiver.vendor, c.receiver.model, c.campaign_id, c.data_class, LABEL
    );
    for (p, a, r, e) in &rows {
        m.push_str(&format!("| {p} | {a} | {r} | {} |\n", e.replace('|', "/")));
    }
    m.push_str(&format!(
        "\nScorecard: {} met, {} not met, {} not assessed. Open phases: {}.\n",
        s.met,
        s.not_met,
        s.not_assessed,
        rows.iter().filter(|r| r.3 == open).count()
    ));
    m
}

/// A SYNTHETIC sample campaign (to try the tools; not data from any receiver).
pub const SAMPLE_CAMPAIGN: &str = r#"format = "kshana-space-rx-campaign/1"
campaign_id = "sample-2026-01"
data_class = "synthetic"
source = "SYNTHETIC sample; not data from any receiver"
lab = "example lab"
tid_source = "Co-60"
dose_rate_rad_s = 0.01

[receiver]
vendor = "ExampleVendor"
model = "EX-1"
firmware = "1.0"
class = "cots"
constellations = ["GPS", "Galileo"]
signals = ["L1 C/A", "E1"]
mass_g = 20
power_w = 0.7

[[tid]]
dose_krad = 0
functional = true
position_error_m = 2.1
cn0_mean_dbhz = 44
ttff_s = 35
current_ma = 120
[[tid]]
dose_krad = 10
functional = true
position_error_m = 2.3
cn0_mean_dbhz = 44
ttff_s = 36
current_ma = 124
[[tid]]
dose_krad = 20
functional = true
position_error_m = 2.6
cn0_mean_dbhz = 43
ttff_s = 40
current_ma = 131
[[tid]]
dose_krad = 30
functional = true
position_error_m = 3.4
cn0_mean_dbhz = 42
ttff_s = 52
current_ma = 150
[[tid]]
dose_krad = 40
functional = false

[[see]]
particle = "heavy-ion"
ion = "Kr"
let_mev_cm2_mg = 32
fluence_cm2 = 1e7
sefi = 3
reset = 2
seu = 40
[[see]]
particle = "heavy-ion"
ion = "Xe"
let_mev_cm2_mg = 62
fluence_cm2 = 1e7
latchup = 1
sefi = 7
reset = 4
seu = 95
[[see]]
particle = "proton"
energy_mev = 100
fluence_cm2 = 1e10
sefi = 1
seu = 12

[[vibration]]
axis = "X"
grms = 14.1
duration_s = 120
tracking_lost = false
cn0_drop_db = 1.5
functional_after = true
[[vibration]]
axis = "Y"
grms = 14.1
duration_s = 120
tracking_lost = false
cn0_drop_db = 2.0
functional_after = true
[[vibration]]
axis = "Z"
grms = 14.1
duration_s = 120
tracking_lost = true
cn0_drop_db = 6.0
functional_after = true

[[thermal]]
temp_c = -30
cycles = 8
functional = true
ttff_s = 48
[[thermal]]
temp_c = 70
cycles = 8
functional = true
ttff_s = 39
[[thermal]]
temp_c = 20
vacuum = true
functional = true

[[tracking]]
case = "leo-550km"
functional = true
ttff_s = 60
position_error_m = 3.0
[[tracking]]
case = "leo-cold-start"
functional = true
ttff_s = 180
position_error_m = 3.5

[[jamming]]
js_db = 20
tracking_held = true
waveform = "broadband"
[[jamming]]
js_db = 30
tracking_held = true
waveform = "broadband"
[[jamming]]
js_db = 40
tracking_held = false
waveform = "broadband"
"#;

/// A SYNTHETIC sample mission profile (illustrative limits, not from any mission).
pub const SAMPLE_MISSION: &str = r#"format = "kshana-space-rx-mission/1"
name = "example LEO 550 km, 3 years"
tid_krad = 10
rdm = 2.0
latchup_let_min = 60
sefi_cross_section_max_cm2 = 1e-5
max_position_error_m = 10
max_ttff_s = 200
max_current_increase_pct = 25
vib_grms = 14.1
max_cn0_drop_db = 3
temp_min_c = -20
temp_max_c = 60
thermal_cycles = 8
vacuum_required = true
tracking_cases = ["leo-550km", "leo-cold-start"]
min_tested_js_db = 25

[sources]
tid_krad = "illustrative value for the sample"
latchup_let_min = "illustrative value for the sample"
"#;
