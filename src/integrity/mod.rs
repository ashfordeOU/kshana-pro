// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # AIS and position integrity scoring (roadmap 0.42 E5)
//!
//! For a stream of AIS position reports and, optionally, independent RF fixes (anonymous
//! positions of radiating vessels, for example from a space RF survey or a coastal network),
//! this module decides for every AIS identity whether its data are consistent, and summarises
//! the decisions **per fleet and per area** as integrity scores.
//!
//! * **Stage 1** is the open engine's committed M4 v3 decision (`kshana::m4_logic::decide`,
//!   `M4Params::v3()`): AIS gaps, one identity in two places, RF fixes with no matching AIS
//!   ("dark" fixes), and its own offset test.
//! * **Stage 2** is the pre-registered V3b offset test ([`offset_v4`]): an identity whose AIS
//!   positions run parallel to a persistent RF track at a constant displacement. V3 and V3b were
//!   both NOT MET (false-alarm rates of 0.132 and V3b's in its RESULTS), so stage 2 **never
//!   changes a class**: its hits are listed as offset candidates for a human to review, marked as
//!   coming from a method that missed its bars.
//! * **Scores**: per group (all vessels, each fleet, each area), the number of identities in each
//!   class, the integrity score (the share decided consistent, with an exact 95 % interval) and,
//!   per area, the RF fixes with no matching AIS.
//!
//! Every output describes **data consistency** ("AIS position inconsistent with the RF fixes",
//! "no matching AIS"), never intent: a gap or an offset has ordinary causes (equipment faults,
//! reception loss, GNSS interference, transponders switched off for legitimate reasons).
//!
//! **Frame of the RF cross-check.** Stage 1's offset and dark-track tests predict RF accesses
//! from the open engine's M2 survey layer over a 6 h window (times in seconds from the window
//! start); RF fixes from another sensor need that sensor's access model, which is not built
//! (`LIMITS`). Stage 2 uses only the fixes supplied. Without RF fixes only the AIS-only checks
//! (gaps, identity in two places) run.
//!
//! The quality bars are V3 (`docs/evidence/v3-m4-offset-successor/`), scored on synthetic
//! scenes; [`VALIDATION_STATUS`] states the outcome.

pub mod offset_v4;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::m4_dark_vessel::{Label, Scene};
use kshana::m4_logic::{clopper_pearson, decide, M4Params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The scenario kind (and the licence feature of `kshana-pro integrity score`).
pub const KIND: &str = "pro-ais-integrity";
/// The output format tag.
pub const FORMAT: &str = "kshana-pro-ais-integrity/1";
/// The method: open M4 v3 then the V3 offset test.
pub const METHOD_ID: &str = "kshana-pro/ais-integrity/v1";
/// The open engine the method is pinned to.
pub const OPEN_ENGINE_PIN: &str = "0.41.0";
/// The file a scoring writes into its report directory.
pub const RESULT_FILE: &str = "integrity-score.json";

/// The honesty label every output carries.
pub const LABEL: &str = "REPORTED AIS positions; RF fixes as supplied (MEASURED or MODELLED, as \
the supplier states); MODELLED classes: each class describes the consistency of the data \
supplied under rules fixed before the run, never intent. An engineering record, not a legal \
opinion, not a finding of fact and not a certification.";

/// The validation status every output states (roadmap V3).
pub const VALIDATION_STATUS: &str = "V3 (the M4 position-offset bar with the stage-2 method, \
pre-registered 15 November 2026): see docs/evidence/v3-m4-offset-successor/RESULTS.md. Scored on \
synthetic scenes only (MODELLED); no real AIS or RF data has been scored.";

/// The stage-2 method used for offset candidates: V3b's committed pair (the lower false-alarm
/// rate of the two registered methods). Its bars were not met; it never changes a class.
pub const STAGE2: offset_v4::Params = offset_v4::V3B_CHOSEN;

/// What every offset candidate says about its method.
pub const STAGE2_NOTE: &str = "Offset candidate from the stage-2 parallel-track test. Its pre-registered bars (V3, V3b) were NOT MET, with a high false-alarm rate on synthetic scenes: a candidate is a lead for review, not a class.";

/// What the method does not do.
pub const LIMITS: &[&str] = &[
    "Stage 1's RF access prediction is the open engine's M2 survey layer over a 6 h window; RF fixes from another sensor need that sensor's access model, which is not built.",
    "RF fixes carry no identity; association is by position and time only.",
    "Classes describe data consistency, never intent; a gap or an offset has ordinary causes.",
    "The bars (V3) were scored on synthetic scenes; no real AIS or RF data has been scored.",
    "No class B AIS, satellite AIS reception effects or slot collisions are modelled.",
];

/// A fleet: a named set of AIS identities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fleet {
    pub name: String,
    pub ids: Vec<u64>,
}

/// An area: a named latitude and longitude box, degrees.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub name: String,
    pub lat_min: f64,
    pub lat_max: f64,
    pub lon_min: f64,
    pub lon_max: f64,
}

impl Area {
    fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.lat_min && lat <= self.lat_max && lon >= self.lon_min && lon <= self.lon_max
    }
}

/// A scoring request (`kind = "pro-ais-integrity"`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrityScenario {
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    /// AIS reports: CSV `t_s,id,lat,lon` (header optional) or an open M4 AIS document.
    pub ais: String,
    /// RF fixes: CSV `t_s,lat,lon` (header optional) or an open M4 RF document.
    #[serde(default)]
    pub rf: Option<String>,
    #[serde(default)]
    pub fleet: Vec<Fleet>,
    #[serde(default)]
    pub area: Vec<Area>,
    /// Directory the paths are relative to (set by [`IntegrityScenario::from_file`]).
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl IntegrityScenario {
    pub const KIND: &'static str = KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description: "AIS and position integrity scoring at fleet and area scale (E5): per identity, gaps, one identity in two places and AIS positions inconsistent with independent RF fixes (open M4 v3 plus the pre-registered V3 offset test); integrity scores per fleet and per area; RF fixes with no matching AIS per area. Describes data consistency, never intent. MODELLED.",
            required_fields: &["ais"],
            optional_fields: &["title", "rf", "fleet", "area"],
        }
    }

    /// Read a request; paths in it are relative to its directory.
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut s: Self =
            toml::from_str(&src).map_err(|e| format!("invalid {KIND} request: {e}"))?;
        s.base_dir = path.parent().map(Path::to_path_buf);
        Ok((s, src))
    }

    fn path(&self, p: &str) -> PathBuf {
        match &self.base_dir {
            Some(d) if Path::new(p).is_relative() => d.join(p),
            _ => PathBuf::from(p),
        }
    }
}

/// One input file as read.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InputRecord {
    pub role: &'static str,
    pub path: String,
    pub sha256: String,
    pub records: usize,
}

/// One identity's decision.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VesselRow {
    pub id: u64,
    /// `NORMAL`, `GAP`, `POSITION-OFFSET` or `IDENTITY-DUPLICATE`.
    pub class: &'static str,
    /// `stage-1` (open M4 v3) or `ais-only` (no RF fixes supplied).
    pub decided_by: &'static str,
    pub reason: String,
    pub reports: usize,
    /// A stage-2 offset candidate for an identity stage 1 decided NORMAL ([`STAGE2_NOTE`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_candidate: Option<offset_v4::OffsetFinding>,
    pub fleets: Vec<String>,
    pub areas: Vec<String>,
}

/// The scores of one group of identities.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GroupScore {
    /// `all`, `fleet` or `area`.
    pub group: &'static str,
    pub name: String,
    pub vessels: usize,
    pub counts: BTreeMap<&'static str, usize>,
    /// Stage-2 offset candidates among the group's NORMAL identities (not a class).
    pub offset_candidates: usize,
    /// Share of the group's identities decided NORMAL.
    pub integrity_score: Option<f64>,
    pub integrity_score_ci95: [f64; 2],
    /// Identities of a fleet with no AIS report in the data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_seen: Option<usize>,
    /// RF fixes inside an area with no matching AIS (stage 1's dark fixes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rf_fixes_no_matching_ais: Option<usize>,
}

/// A scoring result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IntegrityScore {
    pub format: &'static str,
    pub method_id: &'static str,
    pub stage2_method: &'static str,
    pub stage2_params: offset_v4::Params,
    pub stage2_note: &'static str,
    pub open_engine: String,
    pub label: &'static str,
    pub validation_status: &'static str,
    pub title: String,
    pub inputs: Vec<InputRecord>,
    pub rf_cross_check: bool,
    pub totals: Totals,
    pub groups: Vec<GroupScore>,
    pub vessels: Vec<VesselRow>,
    pub limits: Vec<&'static str>,
}

/// Counts over the whole input.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Totals {
    pub identities: usize,
    pub ais_reports: usize,
    pub rf_fixes: usize,
    pub rf_fixes_no_matching_ais: usize,
}

fn sha_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn data_lines(text: &str) -> impl Iterator<Item = (usize, Vec<&str>)> {
    text.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
        .map(|(i, l)| (i, l.split(',').map(str::trim).collect::<Vec<_>>()))
}

fn bad_line(what: &str, i: usize) -> String {
    format!("{what}: line {i} is not valid")
}

/// Parse AIS reports from CSV `t_s,id,lat,lon` or an open M4 AIS document.
pub fn parse_ais(text: &str) -> Result<Vec<(u32, u64, f64, f64)>, String> {
    if text.trim_start().starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("AIS: {e}"))?;
        let rows = v["reports"].as_array().ok_or("AIS: no reports array")?;
        return rows
            .iter()
            .map(|r| {
                Some((
                    u32::try_from(r[0].as_u64()?).ok()?,
                    r[1].as_u64()?,
                    r[2].as_f64()?,
                    r[3].as_f64()?,
                ))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "AIS: a report is not [t, id, lat, lon]".to_string());
    }
    let mut out = Vec::new();
    for (i, f) in data_lines(text) {
        if f.len() != 4 {
            return Err(bad_line("AIS", i));
        }
        let parsed = (
            f[0].parse::<u32>(),
            f[1].parse::<u64>(),
            f[2].parse::<f64>(),
            f[3].parse::<f64>(),
        );
        match parsed {
            (Ok(t), Ok(id), Ok(a), Ok(b)) if a.is_finite() && b.is_finite() => {
                out.push((t, id, a, b))
            }
            _ if out.is_empty() && f[0].parse::<f64>().is_err() => continue, // header
            _ => return Err(bad_line("AIS", i)),
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(out)
}

/// Parse RF fixes from CSV `t_s,lat,lon` or an open M4 RF document.
pub fn parse_rf(text: &str) -> Result<Vec<(u32, f64, f64)>, String> {
    if text.trim_start().starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("RF: {e}"))?;
        let rows = v["fixes"].as_array().ok_or("RF: no fixes array")?;
        return rows
            .iter()
            .map(|r| {
                Some((
                    u32::try_from(r[0].as_u64()?).ok()?,
                    r[1].as_f64()?,
                    r[2].as_f64()?,
                ))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "RF: a fix is not [t, lat, lon]".to_string());
    }
    let mut out = Vec::new();
    for (i, f) in data_lines(text) {
        if f.len() != 3 {
            return Err(bad_line("RF", i));
        }
        match (
            f[0].parse::<u32>(),
            f[1].parse::<f64>(),
            f[2].parse::<f64>(),
        ) {
            (Ok(t), Ok(a), Ok(b)) if a.is_finite() && b.is_finite() => out.push((t, a, b)),
            _ if out.is_empty() && f[0].parse::<f64>().is_err() => continue,
            _ => return Err(bad_line("RF", i)),
        }
    }
    out.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.total_cmp(&b.2))
    });
    Ok(out)
}

/// The decision of a stream: stage 1's classes, stage 2's offset candidates (with [`STAGE2`];
/// they change no class) and stage 1's dark-fix flags.
pub fn decide_stream(
    ais: &[(u32, u64, f64, f64)],
    rf: &[(u32, f64, f64)],
) -> (
    BTreeMap<u64, Label>,
    Vec<offset_v4::OffsetFinding>,
    Vec<bool>,
) {
    let scene = Scene {
        seed: 0,
        truth: serde_json::Value::Null,
        ais: ais.to_vec(),
        rf: rf.to_vec(),
        rf_owner: Vec::new(),
        redraws: 0,
    };
    let d1 = decide(&scene, &M4Params::v3());
    let (_, found) = offset_v4::stage2(ais, rf, &d1.class, STAGE2);
    (d1.class, found, d1.dark_fix)
}

fn reason(l: Label, rf: bool, f: Option<&offset_v4::OffsetFinding>) -> String {
    match (l, f) {
        (Label::Normal, Some(f)) => format!(
            "No inconsistency found by the checks that decide classes; offset candidate for review: a persistent RF track runs parallel at {:.1} km from t = {} s ({} RF epochs over {} s).",
            f.offset_m / 1000.0,
            f.onset_s,
            f.support,
            f.span_s
        ),
        (Label::Normal, _) if rf => {
            "No inconsistency found: AIS reports regular and consistent with the RF fixes supplied."
                .into()
        }
        (Label::Normal, _) => {
            "No inconsistency found in the AIS reports (no RF fixes supplied, so positions were not cross-checked)."
                .into()
        }
        (Label::Gap, _) => "AIS silent for more than 600 s between consecutive reports.".into(),
        (Label::IdentityDuplicate, _) => {
            "Consecutive reports of this identity imply more than 70 kn: two transmitters on one identity, or corrupted positions."
                .into()
        }
        (Label::PositionOffset, _) => {
            "AIS positions inconsistent with the RF fixes from an onset on (several RF epochs agree on one displacement)."
                .into()
        }
        (Label::Dark, _) => "No matching AIS.".into(),
    }
}

fn group(
    kind: &'static str,
    name: &str,
    rows: &[&VesselRow],
    not_seen: Option<usize>,
    dark: Option<usize>,
) -> GroupScore {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for l in [
        Label::Normal,
        Label::Gap,
        Label::PositionOffset,
        Label::IdentityDuplicate,
    ] {
        counts.insert(l.name(), 0);
    }
    for r in rows {
        *counts.entry(r.class).or_insert(0) += 1;
    }
    let n = rows.len();
    let k = counts.get("NORMAL").copied().unwrap_or(0);
    let (lo, hi) = clopper_pearson(k, n);
    GroupScore {
        group: kind,
        name: name.to_string(),
        vessels: n,
        counts,
        offset_candidates: rows.iter().filter(|r| r.offset_candidate.is_some()).count(),
        integrity_score: (n > 0).then(|| k as f64 / n as f64),
        integrity_score_ci95: [lo, hi],
        not_seen,
        rf_fixes_no_matching_ais: dark,
    }
}

/// Score parsed streams.
pub fn score_streams(
    title: &str,
    ais: &[(u32, u64, f64, f64)],
    rf: Option<&[(u32, f64, f64)]>,
    fleets: &[Fleet],
    areas: &[Area],
    inputs: Vec<InputRecord>,
) -> IntegrityScore {
    let rf_v: &[(u32, f64, f64)] = rf.unwrap_or(&[]);
    let (class, found, dark) = decide_stream(ais, rf_v);
    let found_by: BTreeMap<u64, &offset_v4::OffsetFinding> =
        found.iter().map(|f| (f.id, f)).collect();
    let mut reports: BTreeMap<u64, usize> = BTreeMap::new();
    let mut in_area: BTreeMap<u64, BTreeSet<usize>> = BTreeMap::new();
    for &(_, id, a, b) in ais {
        *reports.entry(id).or_insert(0) += 1;
        for (k, ar) in areas.iter().enumerate() {
            if ar.contains(a, b) {
                in_area.entry(id).or_default().insert(k);
            }
        }
    }
    let rf_on = rf.is_some();
    let vessels: Vec<VesselRow> = class
        .iter()
        .map(|(&id, &l)| {
            let f = found_by.get(&id).copied();
            let decided_by = if rf_on { "stage-1" } else { "ais-only" };
            VesselRow {
                id,
                class: l.name(),
                decided_by,
                reason: reason(l, rf_on, f),
                reports: reports.get(&id).copied().unwrap_or(0),
                offset_candidate: f.cloned(),
                fleets: fleets
                    .iter()
                    .filter(|fl| fl.ids.contains(&id))
                    .map(|fl| fl.name.clone())
                    .collect(),
                areas: in_area
                    .get(&id)
                    .map(|s| s.iter().map(|&k| areas[k].name.clone()).collect())
                    .unwrap_or_default(),
            }
        })
        .collect();
    let mut groups = vec![group(
        "all",
        "all identities",
        &vessels.iter().collect::<Vec<_>>(),
        None,
        rf_on.then(|| dark.iter().filter(|&&d| d).count()),
    )];
    for fl in fleets {
        let rows: Vec<&VesselRow> = vessels
            .iter()
            .filter(|v| v.fleets.contains(&fl.name))
            .collect();
        let seen: BTreeSet<u64> = rows.iter().map(|r| r.id).collect();
        let not_seen = fl
            .ids
            .iter()
            .collect::<BTreeSet<_>>()
            .iter()
            .filter(|i| !seen.contains(i))
            .count();
        groups.push(group("fleet", &fl.name, &rows, Some(not_seen), None));
    }
    for ar in areas {
        let rows: Vec<&VesselRow> = vessels
            .iter()
            .filter(|v| v.areas.contains(&ar.name))
            .collect();
        let d = rf_on.then(|| {
            rf_v.iter()
                .zip(&dark)
                .filter(|(f, &dk)| dk && ar.contains(f.1, f.2))
                .count()
        });
        groups.push(group("area", &ar.name, &rows, None, d));
    }
    IntegrityScore {
        format: FORMAT,
        method_id: METHOD_ID,
        stage2_method: offset_v4::METHOD_ID,
        stage2_params: STAGE2,
        stage2_note: STAGE2_NOTE,
        open_engine: format!("kshana {OPEN_ENGINE_PIN}"),
        label: LABEL,
        validation_status: VALIDATION_STATUS,
        title: title.to_string(),
        inputs,
        rf_cross_check: rf_on,
        totals: Totals {
            identities: vessels.len(),
            ais_reports: ais.len(),
            rf_fixes: rf_v.len(),
            rf_fixes_no_matching_ais: dark.iter().filter(|&&d| d).count(),
        },
        groups,
        vessels,
        limits: LIMITS.to_vec(),
    }
}

/// Score a request: read its files, decide, summarise.
pub fn score(s: &IntegrityScenario) -> Result<IntegrityScore, String> {
    if s.kind != KIND {
        return Err(format!("kind must be {KIND}"));
    }
    for a in &s.area {
        if !(a.lat_min <= a.lat_max && a.lon_min <= a.lon_max) {
            return Err(format!("area {}: the box is empty", a.name));
        }
    }
    let ap = s.path(&s.ais);
    let at = std::fs::read(&ap).map_err(|e| format!("{}: {e}", ap.display()))?;
    let ais = parse_ais(&String::from_utf8_lossy(&at))?;
    if ais.is_empty() {
        return Err("AIS: no reports".into());
    }
    let mut inputs = vec![InputRecord {
        role: "ais",
        path: s.ais.clone(),
        sha256: sha_hex(&at),
        records: ais.len(),
    }];
    let rf = match &s.rf {
        Some(p) => {
            let rp = s.path(p);
            let rt = std::fs::read(&rp).map_err(|e| format!("{}: {e}", rp.display()))?;
            let rf = parse_rf(&String::from_utf8_lossy(&rt))?;
            inputs.push(InputRecord {
                role: "rf",
                path: p.clone(),
                sha256: sha_hex(&rt),
                records: rf.len(),
            });
            Some(rf)
        }
        None => None,
    };
    Ok(score_streams(
        s.title.as_deref().unwrap_or("AIS and position integrity"),
        &ais,
        rf.as_deref(),
        &s.fleet,
        &s.area,
        inputs,
    ))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The report page of a scoring.
pub fn render_html(r: &IntegrityScore) -> String {
    let mut h = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;margin:2em;max-width:70em}}table{{border-collapse:collapse}}\
         td,th{{border:1px solid #bbb;padding:.25em .5em;text-align:left}}.l{{color:#555}}</style></head><body>\
         <h1>{}</h1><p class=\"l\">{}</p><p class=\"l\">{}</p>\
         <p>Method {} with stage 2 {} (m = {}, pairwise gate {}); open engine {}. RF cross-check: {}.</p>\
         <h2>Scores</h2><table><tr><th>Group</th><th>Name</th><th>Identities</th><th>Consistent</th><th>Gap</th>\
         <th>Position offset</th><th>Identity in two places</th><th>Integrity score (95 % interval)</th><th>RF fixes with no matching AIS</th><th>Offset candidates (not a class)</th></tr>",
        esc(&r.title),
        esc(&r.title),
        esc(r.label),
        esc(r.validation_status),
        r.method_id,
        r.stage2_method,
        r.stage2_params.m,
        r.stage2_params.g_pair,
        esc(&r.open_engine),
        if r.rf_cross_check { "yes" } else { "no (AIS-only checks)" },
    );
    for g in &r.groups {
        let c = |k: &str| g.counts.get(k).copied().unwrap_or(0);
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            g.group,
            esc(&g.name),
            g.vessels,
            c("NORMAL"),
            c("GAP"),
            c("POSITION-OFFSET"),
            c("IDENTITY-DUPLICATE"),
            g.integrity_score
                .map(|s| format!(
                    "{:.4} ({:.4} to {:.4})",
                    s, g.integrity_score_ci95[0], g.integrity_score_ci95[1]
                ))
                .unwrap_or_else(|| "n/a".into()),
            g.rf_fixes_no_matching_ais
                .map(|d| d.to_string())
                .unwrap_or_else(|| "n/a".into()),
            g.offset_candidates,
        ));
    }
    h.push_str(&format!("</table><h2>Identities not decided consistent, and offset candidates</h2><p class=\"l\">{}</p><table><tr><th>Identity</th><th>Class</th><th>Decided by</th><th>Reason</th></tr>", esc(r.stage2_note)));
    for v in r
        .vessels
        .iter()
        .filter(|v| v.class != "NORMAL" || v.offset_candidate.is_some())
    {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            v.id,
            v.class,
            v.decided_by,
            esc(&v.reason)
        ));
    }
    h.push_str("</table><h2>Inputs</h2><ul>");
    for i in &r.inputs {
        h.push_str(&format!(
            "<li>{}: {} ({} records), SHA-256 {}</li>",
            i.role,
            esc(&i.path),
            i.records,
            i.sha256
        ));
    }
    h.push_str("</ul><h2>Limits</h2><ul>");
    for l in &r.limits {
        h.push_str(&format!("<li>{}</li>", esc(l)));
    }
    h.push_str("</ul></body></html>\n");
    h
}

/// Write a scoring's report directory: `integrity-score.json`, `report.html` and `vessels.csv`.
pub fn write_dir(out: &Path, r: &IntegrityScore) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let w = |name: &str, b: &[u8]| {
        std::fs::write(out.join(name), b).map_err(|e| format!("{name}: {e}"))
    };
    w(
        RESULT_FILE,
        (serde_json::to_string_pretty(r).expect("plain data") + "\n").as_bytes(),
    )?;
    w("report.html", render_html(r).as_bytes())?;
    w("vessels.csv", vessels_csv(r).as_bytes())
}

/// One row per identity.
pub fn vessels_csv(r: &IntegrityScore) -> String {
    let mut s = String::from(
        "id,class,decided_by,reports,offset_candidate_km,offset_candidate_onset_s,fleets,areas\n",
    );
    for v in &r.vessels {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            v.id,
            v.class,
            v.decided_by,
            v.reports,
            v.offset_candidate
                .as_ref()
                .map(|o| format!("{:.3}", o.offset_m / 1000.0))
                .unwrap_or_default(),
            v.offset_candidate
                .as_ref()
                .map(|o| o.onset_s.to_string())
                .unwrap_or_default(),
            v.fleets.join(";"),
            v.areas.join(";"),
        ));
    }
    s
}

impl Scenario for IntegrityScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let r = score(self).map_err(KshanaError::InvalidInput)?;
        let all = &r.groups[0];
        let summary = format!(
            "Kshana Pro AIS integrity | {} identities, integrity score {} | gap {}, position offset {}, identity in two places {} | RF fixes with no matching AIS {} | MODELLED",
            all.vessels,
            all.integrity_score.map(|s| format!("{s:.4}")).unwrap_or_else(|| "n/a".into()),
            all.counts.get("GAP").copied().unwrap_or(0),
            all.counts.get("POSITION-OFFSET").copied().unwrap_or(0),
            all.counts.get("IDENTITY-DUPLICATE").copied().unwrap_or(0),
            r.totals.rf_fixes_no_matching_ais,
        );
        Ok(RunOutput {
            json: serde_json::to_string(&r).expect("plain data"),
            svg: String::new(),
            summary,
            csv: Some(vessels_csv(&r)),
        })
    }
}

impl ExternalPack for IntegrityScenario {
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
