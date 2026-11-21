// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The exposure ledger (`kshana-pro/exposure-ledger/v1`) and the post-spoofing reset check
//! (`kshana-pro/reset-check/v1`).
//!
//! **Ledger.** Per aircraft (tail) or vessel (IMO/MMSI), from signed or plain episode files:
//! exposure minutes (the union of the subject's member intervals, by kind), episodes, spoofing
//! events (episodes where the subject had a spoofing-like member), first and last exposure,
//! and, with a resets file, what happened since the last recorded receiver reset. A subject is
//! flagged `review` when it has at least `spoofing_events_flag` spoofing events, or at least
//! `exposure_minutes_flag` minutes of exposure, since its last reset (or ever, with none
//! recorded). A planning aid for maintenance, not an airworthiness or seaworthiness
//! determination.
//!
//! **Reset check.** After an episode, does the subject's receiver still disagree with an
//! independent reference? From `settle_min` after the episode end: the share of fixes more
//! than `max_disagreement_m` from the reference (a surveyed position, or an independent
//! track such as the inertial/FMS position or a second receiver), and a position that stays
//! frozen while the reference moves. Outcome: RESET RECOMMENDED, NO RESET INDICATED or NOT
//! ASSESSABLE. MODELLED: a recommendation only; the manufacturer's and operator's procedures
//! decide.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};

use super::episodes::{EpisodesDoc, MemberSource};
use super::text::{iso, round};
use super::verify::Epoch;
use super::*;

pub const LEDGER_FORMAT: &str = "kshana-pro-exposure-ledger/1";
pub const LEDGER_METHOD: &str = "kshana-pro/exposure-ledger/v1";
pub const LEDGER_FILE: &str = "ledger.json";
pub const LEDGER_EVIDENCE: &str = "MODELLED: counts of the episodes supplied; a planning aid for maintenance, \
not an airworthiness or seaworthiness determination. Exposure is as complete as the reports and ADS-B data behind the episodes.";

pub const RESET_FORMAT: &str = "kshana-pro-reset-check/1";
pub const RESET_METHOD: &str = "kshana-pro/reset-check/v1";
pub const RESET_FILE: &str = "reset-check.json";
pub const RESET_EVIDENCE: &str = "MODELLED: a recommendation from the disagreement between the receiver's \
outputs and the reference supplied; not validated against receiver manufacturers' reset criteria. The manufacturer's and operator's procedures decide.";

/// The ledger request (`ledger.toml`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerRequest {
    pub title: String,
    /// Episode directories (signed by `reports episodes`) or `episodes.json` files.
    pub episodes: Vec<String>,
    /// CSV `subject_id,reset_utc[,action]`: recorded receiver resets or power cycles.
    #[serde(default)]
    pub resets: Option<String>,
    #[serde(default)]
    pub period_start_utc: Option<String>,
    #[serde(default)]
    pub period_end_utc: Option<String>,
    #[serde(default = "one")]
    pub spoofing_events_flag: usize,
    #[serde(default = "two_hours")]
    pub exposure_minutes_flag: f64,
    /// Other ids of a subject (ICAO 24-bit address → registration, MMSI → IMO).
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

fn one() -> usize {
    1
}

fn two_hours() -> f64 {
    120.0
}

/// One subject's line in the ledger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub subject_id: String,
    pub platform: Platform,
    pub episodes: Vec<String>,
    pub exposure_minutes: f64,
    pub exposure_minutes_by_kind: BTreeMap<String, f64>,
    pub spoofing_events: usize,
    pub first_exposure_utc: String,
    pub last_exposure_utc: String,
    #[serde(default)]
    pub last_spoofing_utc: Option<String>,
    #[serde(default)]
    pub last_reset_utc: Option<String>,
    pub spoofing_events_since_reset: usize,
    pub exposure_minutes_since_reset: f64,
    /// `review` or `none`.
    pub flag: String,
    pub flag_reason: String,
}

/// The ledger document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub title: String,
    #[serde(default)]
    pub period_start_utc: Option<String>,
    #[serde(default)]
    pub period_end_utc: Option<String>,
    pub rules: Vec<String>,
    pub inputs: Vec<InputFile>,
    /// Episode sources refused (signature failed or signer not trusted).
    pub refused: Vec<String>,
    pub entries: Vec<LedgerEntry>,
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Union length of intervals, seconds.
fn union_s(mut v: Vec<(f64, f64)>) -> f64 {
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut total = 0.0;
    let mut cur: Option<(f64, f64)> = None;
    for (a, b) in v {
        cur = match cur {
            Some((s, e)) if a <= e => Some((s, e.max(b))),
            Some((s, e)) => {
                total += e - s;
                Some((a, b))
            }
            None => Some((a, b)),
        };
    }
    if let Some((s, e)) = cur {
        total += e - s;
    }
    total
}

/// Episode documents read, their input records, the sources refused, and the copies.
pub type EpisodeSources = (Vec<EpisodesDoc>, Vec<InputFile>, Vec<String>, Copies);

/// Read the episode sources: a directory is signature-checked (and trust-pinned with
/// `trust`); a file is read as is.
pub fn read_episodes(
    paths: &[String],
    base: &Path,
    trust: &[[u8; 32]],
) -> Result<EpisodeSources, String> {
    let (mut docs, mut inputs, mut refused, mut copies) = (vec![], vec![], vec![], vec![]);
    for p in paths {
        let path = base.join(p);
        let file = if path.is_dir() {
            let v = crate::signed_dossier::verify(&path, trust).map_err(|e| format!("{p}: {e}"))?;
            if !v.passed() {
                refused.push(format!(
                    "{p}: signature or files do not verify ({})",
                    v.problems.join("; ")
                ));
                continue;
            }
            if v.trusted == Some(false) {
                refused.push(format!("{p}: signer key {} is not trusted", v.key_id));
                continue;
            }
            path.join(super::episodes::RESULT_FILE)
        } else {
            path.clone()
        };
        let b = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let d: EpisodesDoc =
            serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", file.display()))?;
        if d.format != super::episodes::FORMAT {
            return Err(format!("{}: not an episodes file", file.display()));
        }
        let name = format!("{}-{}", docs.len() + 1, safe_name(&file.to_string_lossy()));
        inputs.push(InputFile {
            role: "episodes".into(),
            file: name.clone(),
            sha256: sha256_hex(&b),
        });
        copies.push((name, b));
        docs.push(d);
    }
    Ok((docs, inputs, refused, copies))
}

/// Build the ledger.
pub fn build_ledger(
    req: &LedgerRequest,
    docs: &[EpisodesDoc],
    resets_csv: Option<&str>,
    inputs: Vec<InputFile>,
    refused: Vec<String>,
) -> Result<Ledger, String> {
    let p0 = req
        .period_start_utc
        .as_deref()
        .map(|s| text::parse_utc(s).ok_or(format!("period_start_utc '{s}'")))
        .transpose()?;
    let p1 = req
        .period_end_utc
        .as_deref()
        .map(|s| text::parse_utc(s).ok_or(format!("period_end_utc '{s}'")))
        .transpose()?;
    let alias: BTreeMap<String, String> = req
        .aliases
        .iter()
        .map(|(k, v)| (norm(k), v.clone()))
        .collect();
    let canon = |id: &str| {
        alias
            .get(&norm(id))
            .cloned()
            .unwrap_or_else(|| id.to_string())
    };
    let mut resets: BTreeMap<String, f64> = BTreeMap::new();
    if let Some(csv) = resets_csv {
        for (i, line) in text::csv_records(csv).iter().enumerate() {
            let f = text::csv_fields(line);
            if i == 0
                && f.first()
                    .is_some_and(|h| text::norm_key(h).contains("subject"))
            {
                continue;
            }
            let (Some(id), Some(t)) = (f.first(), f.get(1).and_then(|t| text::parse_datetime(t)))
            else {
                return Err(format!("resets line {}: needs subject_id,reset_utc", i + 1));
            };
            let k = norm(&canon(id));
            let e = resets.entry(k).or_insert(t);
            *e = e.max(t);
        }
    }
    struct Acc {
        platform: Platform,
        id: String,
        episodes: BTreeSet<String>,
        by_kind: BTreeMap<Kind, Vec<(f64, f64)>>,
        spoof_eps: BTreeMap<String, f64>,
    }
    let mut acc: BTreeMap<String, Acc> = BTreeMap::new();
    for d in docs {
        for e in &d.episodes {
            for m in &e.members {
                let Some(sid) = &m.subject_id else { continue };
                let (Some(mut a), Some(mut b)) =
                    (text::parse_utc(&m.start_utc), text::parse_utc(&m.end_utc))
                else {
                    continue;
                };
                if let Some(p0) = p0 {
                    a = a.max(p0);
                }
                if let Some(p1) = p1 {
                    b = b.min(p1);
                }
                if b < a {
                    continue;
                }
                let id = canon(sid);
                let x = acc.entry(norm(&id)).or_insert_with(|| Acc {
                    platform: m.platform,
                    id: id.clone(),
                    episodes: BTreeSet::new(),
                    by_kind: BTreeMap::new(),
                    spoof_eps: BTreeMap::new(),
                });
                x.episodes.insert(e.id.clone());
                // An instant (ADS-B jump) counts as one minute of exposure.
                let b = if b - a < 1.0 && m.source == MemberSource::AdsbJump {
                    a + 60.0
                } else {
                    b
                };
                x.by_kind.entry(m.kind).or_default().push((a, b));
                if m.kind == Kind::Spoofing {
                    let t = x.spoof_eps.entry(e.id.clone()).or_insert(a);
                    *t = t.min(a);
                }
            }
        }
    }
    let mut entries = vec![];
    for (k, x) in acc {
        let all: Vec<(f64, f64)> = x.by_kind.values().flatten().copied().collect();
        let first = all.iter().map(|i| i.0).fold(f64::INFINITY, f64::min);
        let last = all.iter().map(|i| i.1).fold(f64::NEG_INFINITY, f64::max);
        let reset = resets.get(&k).copied();
        let since: Vec<(f64, f64)> = all
            .iter()
            .filter_map(|&(a, b)| match reset {
                Some(r) if b <= r => None,
                Some(r) => Some((a.max(r), b)),
                None => Some((a, b)),
            })
            .collect();
        let spoof_since = x
            .spoof_eps
            .values()
            .filter(|t| reset.is_none_or(|r| **t > r))
            .count();
        let exp_since = round(union_s(since) / 60.0, 1);
        let (flag, reason) = if spoof_since >= req.spoofing_events_flag.max(1) {
            ("review", format!(
                "{spoof_since} spoofing event(s) since {}: check the receiver (`reports reset-check` with post-episode logs) or follow the manufacturer's procedure",
                reset.map(iso).unwrap_or_else(|| "the start of the record (no reset recorded)".into())
            ))
        } else if exp_since >= req.exposure_minutes_flag {
            (
                "review",
                format!(
                    "{exp_since} min of exposure since the last reset (threshold {} min)",
                    req.exposure_minutes_flag
                ),
            )
        } else {
            ("none", "below the review thresholds".to_string())
        };
        entries.push(LedgerEntry {
            subject_id: x.id,
            platform: x.platform,
            episodes: x.episodes.into_iter().collect(),
            exposure_minutes: round(union_s(all) / 60.0, 1),
            exposure_minutes_by_kind: x
                .by_kind
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), round(union_s(v.clone()) / 60.0, 1)))
                .collect(),
            spoofing_events: x.spoof_eps.len(),
            first_exposure_utc: iso(first),
            last_exposure_utc: iso(last),
            last_spoofing_utc: x.spoof_eps.values().copied().reduce(f64::max).map(iso),
            last_reset_utc: reset.map(iso),
            spoofing_events_since_reset: spoof_since,
            exposure_minutes_since_reset: exp_since,
            flag: flag.into(),
            flag_reason: reason,
        });
    }
    entries.sort_by(|a, b| {
        (b.flag == "review")
            .cmp(&(a.flag == "review"))
            .then(b.spoofing_events.cmp(&a.spoofing_events))
            .then(b.exposure_minutes.total_cmp(&a.exposure_minutes))
            .then(a.subject_id.cmp(&b.subject_id))
    });
    Ok(Ledger {
        format: LEDGER_FORMAT.into(),
        method: LEDGER_METHOD.into(),
        evidence: LEDGER_EVIDENCE.into(),
        title: req.title.clone(),
        period_start_utc: p0.map(iso),
        period_end_utc: p1.map(iso),
        rules: vec![
            "Exposure minutes: the union of the subject's member intervals in the episodes (an ADS-B jump counts one minute).".into(),
            "Spoofing events: episodes in which the subject had a spoofing-like member (a spoofing report or an ADS-B jump).".into(),
            format!(
                "Flag review: at least {} spoofing event(s), or at least {} min of exposure, since the last recorded reset (or ever, with none recorded).",
                req.spoofing_events_flag.max(1),
                req.exposure_minutes_flag
            ),
        ],
        inputs,
        refused,
        entries,
    })
}

/// Run a ledger request read from `path`.
pub fn run_ledger(path: &Path, trust: &[[u8; 32]]) -> Result<(Ledger, Copies), String> {
    let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: LedgerRequest = toml::from_str(&t).map_err(|e| format!("{}: {e}", path.display()))?;
    if req.episodes.is_empty() {
        return Err("ledger request lists no episodes".into());
    }
    let base = path.parent().unwrap_or(Path::new("."));
    let (docs, mut inputs, refused, mut copies) = read_episodes(&req.episodes, base, trust)?;
    let resets = match &req.resets {
        Some(r) => {
            let p = base.join(r);
            let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            inputs.push(InputFile {
                role: "resets".into(),
                file: safe_name(r),
                sha256: sha256_hex(&b),
            });
            copies.push((r.clone(), b.clone()));
            Some(String::from_utf8_lossy(&b).to_string())
        }
        None => None,
    };
    inputs.push(InputFile {
        role: "request".into(),
        file: safe_name(&path.to_string_lossy()),
        sha256: sha256_hex(t.as_bytes()),
    });
    copies.push((path.to_string_lossy().to_string(), t.into_bytes()));
    let l = build_ledger(&req, &docs, resets.as_deref(), inputs, refused)?;
    Ok((l, copies))
}

/// Write `ledger.json`, `ledger.csv` and the inputs. The caller signs it.
pub fn write_ledger(dir: &Path, l: &Ledger, inputs: &[(String, Vec<u8>)]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join(LEDGER_FILE), json_text(l)).map_err(|e| e.to_string())?;
    let mut csv = String::from(
        "subject_id,platform,episodes,exposure_minutes,jamming_min,spoofing_min,unknown_min,spoofing_events,last_spoofing_utc,last_reset_utc,spoofing_events_since_reset,exposure_minutes_since_reset,flag\n",
    );
    for e in &l.entries {
        let k = |n: &str| e.exposure_minutes_by_kind.get(n).copied().unwrap_or(0.0);
        csv += &format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            e.subject_id,
            e.platform.as_str(),
            e.episodes.len(),
            e.exposure_minutes,
            k("jamming"),
            k("spoofing"),
            k("unknown"),
            e.spoofing_events,
            e.last_spoofing_utc.clone().unwrap_or_default(),
            e.last_reset_utc.clone().unwrap_or_default(),
            e.spoofing_events_since_reset,
            e.exposure_minutes_since_reset,
            e.flag
        );
    }
    std::fs::write(dir.join("ledger.csv"), csv).map_err(|e| e.to_string())?;
    write_inputs(dir, inputs)
}

// ---------------------------------------------------------------------------------------------
// Reset check.

/// The reset-check request (`reset.toml`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetRequest {
    pub subject: String,
    /// When the episode ended (from the episode record).
    pub episode_end_utc: String,
    /// The subject's receiver after the episode.
    pub receiver: ResetReceiver,
    /// A fixed site's surveyed position `[lat, lon]`.
    #[serde(default)]
    pub reference_position: Option<[f64; 2]>,
    /// An independent track CSV `timestamp,lat,lon` (inertial/FMS position, a second receiver).
    #[serde(default)]
    pub reference_track: Option<String>,
    /// What the reference is (written into the record).
    #[serde(default)]
    pub reference_label: Option<String>,
    #[serde(default)]
    pub params: ResetParams,
}

/// The receiver log of a reset check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetReceiver {
    pub path: String,
    /// An open-engine log format or `csv`.
    pub format: String,
}

/// The reset check's thresholds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResetParams {
    pub settle_min: f64,
    pub max_disagreement_m: f64,
    pub min_share: f64,
    pub min_samples: usize,
    pub frozen_m: f64,
    pub frozen_min_s: f64,
    pub reference_moved_m: f64,
    pub reference_max_gap_s: f64,
}

impl Default for ResetParams {
    fn default() -> Self {
        ResetParams {
            settle_min: 5.0,
            max_disagreement_m: 500.0,
            min_share: 0.25,
            min_samples: 20,
            frozen_m: 5.0,
            frozen_min_s: 60.0,
            reference_moved_m: 1000.0,
            reference_max_gap_s: 30.0,
        }
    }
}

/// A finding of the reset check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResetFinding {
    pub name: String,
    pub detail: String,
    pub value: f64,
}

/// The reset-check record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResetCheck {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub subject: String,
    pub episode_end_utc: String,
    pub assessed_from_utc: String,
    #[serde(default)]
    pub assessed_to_utc: Option<String>,
    pub reference: String,
    /// `RESET RECOMMENDED`, `NO RESET INDICATED` or `NOT ASSESSABLE`.
    pub outcome: String,
    pub samples: usize,
    pub samples_with_reference: usize,
    pub disagreement_share: Option<f64>,
    pub median_disagreement_m: Option<f64>,
    pub max_disagreement_m: Option<f64>,
    /// Whether the last fifth of the samples still disagree.
    pub persisting_at_end: Option<bool>,
    pub findings: Vec<ResetFinding>,
    pub reasons: Vec<String>,
    pub params: ResetParams,
    pub inputs: Vec<InputFile>,
    pub statement: String,
    /// Rows of the receiver log and reference track that could not be read.
    #[serde(default)]
    pub skipped_rows: Vec<crate::timing::selfcheck::SkippedRows>,
}

fn interp(track: &[Epoch], t: f64, max_gap: f64) -> Option<(f64, f64)> {
    let i = track.partition_point(|e| e.t < t);
    let b = track.get(i)?;
    if (b.t - t).abs() < 1e-6 {
        return b.fix;
    }
    let a = track.get(i.checked_sub(1)?)?;
    if b.t - a.t > max_gap {
        return None;
    }
    let (fa, fb) = (a.fix?, b.fix?);
    let w = (t - a.t) / (b.t - a.t);
    Some((fa.0 + w * (fb.0 - fa.0), fa.1 + w * (fb.1 - fa.1)))
}

/// The reset check over loaded epochs.
pub fn reset_check(
    req: &ResetRequest,
    rx: &[Epoch],
    reference: Option<&[Epoch]>,
    inputs: Vec<InputFile>,
) -> Result<ResetCheck, String> {
    let p = &req.params;
    let end = text::parse_utc(&req.episode_end_utc)
        .ok_or(format!("episode_end_utc '{}'", req.episode_end_utc))?;
    let from = end + p.settle_min * 60.0;
    let after: Vec<&Epoch> = rx.iter().filter(|e| e.t >= from).collect();
    let ref_label = req.reference_label.clone().unwrap_or_else(|| {
        if req.reference_position.is_some() {
            "surveyed position".into()
        } else {
            "independent track".into()
        }
    });
    let mut out = ResetCheck {
        format: RESET_FORMAT.into(),
        method: RESET_METHOD.into(),
        evidence: RESET_EVIDENCE.into(),
        subject: req.subject.clone(),
        episode_end_utc: iso(end),
        assessed_from_utc: iso(from),
        assessed_to_utc: after.last().map(|e| iso(e.t)),
        reference: ref_label,
        outcome: "NOT ASSESSABLE".into(),
        samples: after.len(),
        samples_with_reference: 0,
        disagreement_share: None,
        median_disagreement_m: None,
        max_disagreement_m: None,
        persisting_at_end: None,
        findings: vec![],
        reasons: vec![],
        skipped_rows: vec![],
        params: p.clone(),
        inputs,
        statement: "A recommendation only (MODELLED). The receiver manufacturer's and the operator's procedures decide whether and how a receiver is reset.".into(),
    };
    let refpos = |t: f64| -> Option<(f64, f64)> {
        match (req.reference_position, reference) {
            (Some(p0), _) => Some((p0[0], p0[1])),
            (None, Some(tr)) => interp(tr, t, p.reference_max_gap_s),
            _ => None,
        }
    };
    let pairs: Vec<(f64, f64)> = after
        .iter()
        .filter_map(|e| {
            let f = e.fix?;
            let r = refpos(e.t)?;
            Some((e.t, haversine_m(f.0, f.1, r.0, r.1)))
        })
        .collect();
    out.samples_with_reference = pairs.len();
    let no_fix = after.iter().filter(|e| e.fix.is_none()).count();
    if !after.is_empty() && no_fix * 2 >= after.len() {
        out.reasons.push(format!(
            "{no_fix} of {} epochs after the settle time have no fix: interference may still be present; not assessable as a stuck receiver",
            after.len()
        ));
    }
    if pairs.len() < p.min_samples {
        out.reasons.push(format!(
            "{} sample(s) with a fix and a reference after {} (at least {} needed)",
            pairs.len(),
            iso(from),
            p.min_samples
        ));
        return Ok(out);
    }
    let d: Vec<f64> = pairs.iter().map(|x| x.1).collect();
    let bad = d.iter().filter(|v| **v > p.max_disagreement_m).count();
    let share = bad as f64 / d.len() as f64;
    out.disagreement_share = Some(round(share, 3));
    out.median_disagreement_m = super::verify::median(d.clone()).map(|v| round(v, 1));
    out.max_disagreement_m = Some(round(d.iter().copied().fold(0.0, f64::max), 1));
    let tail = &d[d.len() - (d.len() / 5).max(1)..];
    let persisting = tail.iter().filter(|v| **v > p.max_disagreement_m).count() * 2 > tail.len();
    out.persisting_at_end = Some(persisting);
    if share >= p.min_share {
        out.findings.push(ResetFinding {
            name: "position-disagreement".into(),
            detail: format!(
                "{bad} of {} fixes more than {} m from the {} (median {:.0} m){}",
                d.len(),
                p.max_disagreement_m,
                out.reference,
                out.median_disagreement_m.unwrap_or(0.0),
                if persisting {
                    ", still disagreeing at the end of the log"
                } else {
                    ""
                }
            ),
            value: round(share, 3),
        });
    }
    // Frozen position while the reference moves.
    let fixes: Vec<&Epoch> = after.iter().copied().filter(|e| e.fix.is_some()).collect();
    let mut i = 0;
    while i < fixes.len() {
        let f0 = fixes[i].fix.expect("fix");
        let mut j = i;
        while j + 1 < fixes.len() {
            let f = fixes[j + 1].fix.expect("fix");
            if haversine_m(f0.0, f0.1, f.0, f.1) > p.frozen_m {
                break;
            }
            j += 1;
        }
        let dur = fixes[j].t - fixes[i].t;
        if dur >= p.frozen_min_s {
            if let (Some(r0), Some(r1)) = (refpos(fixes[i].t), refpos(fixes[j].t)) {
                let moved = haversine_m(r0.0, r0.1, r1.0, r1.1);
                if moved >= p.reference_moved_m {
                    out.findings.push(ResetFinding {
                        name: "frozen-position".into(),
                        detail: format!(
                            "position unchanged for {dur:.0} s from {} while the reference moved {moved:.0} m",
                            iso(fixes[i].t)
                        ),
                        value: round(dur, 0),
                    });
                    break;
                }
            }
        }
        i = j + 1;
    }
    if out.findings.is_empty() {
        out.outcome = "NO RESET INDICATED".into();
        out.reasons.push(format!(
            "{} of {} fixes within {} m of the {} after the settle time",
            d.len() - bad,
            d.len(),
            p.max_disagreement_m,
            out.reference
        ));
    } else {
        out.outcome = "RESET RECOMMENDED".into();
        out.reasons.extend(
            out.findings
                .iter()
                .map(|f| f.detail.clone())
                .collect::<Vec<_>>(),
        );
    }
    Ok(out)
}

/// Run a reset-check request read from `path`.
pub fn run_reset(path: &Path) -> Result<(ResetCheck, Copies), String> {
    let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: ResetRequest = toml::from_str(&t).map_err(|e| format!("{}: {e}", path.display()))?;
    if req.reference_position.is_none() == req.reference_track.is_none() {
        return Err("give exactly one of reference_position and reference_track".into());
    }
    let base = path.parent().unwrap_or(Path::new("."));
    let rd = |p: &str| std::fs::read(base.join(p)).map_err(|e| format!("{p}: {e}"));
    let rb = rd(&req.receiver.path)?;
    let (rx, mut sk) = super::verify::load_receiver_counted(&req.receiver.format, &rb)
        .map_err(|e| format!("{}: {e}", req.receiver.path))?;
    sk.source = "receiver".into();
    let mut skipped: Vec<_> = sk.nonzero().into_iter().collect();
    let mut inputs = vec![InputFile {
        role: "receiver".into(),
        file: safe_name(&req.receiver.path),
        sha256: sha256_hex(&rb),
    }];
    let mut copies = vec![(req.receiver.path.clone(), rb)];
    let reference = match &req.reference_track {
        Some(r) => {
            let b = rd(r)?;
            let (tr, mut sk) =
                super::verify::load_csv_positions_counted(&String::from_utf8_lossy(&b))
                    .map_err(|e| format!("{r}: {e}"))?;
            sk.source = "reference track".into();
            skipped.extend(sk.nonzero());
            inputs.push(InputFile {
                role: "reference".into(),
                file: safe_name(r),
                sha256: sha256_hex(&b),
            });
            copies.push((r.clone(), b));
            Some(tr)
        }
        None => None,
    };
    inputs.push(InputFile {
        role: "request".into(),
        file: safe_name(&path.to_string_lossy()),
        sha256: sha256_hex(t.as_bytes()),
    });
    copies.push((path.to_string_lossy().to_string(), t.into_bytes()));
    let mut c = reset_check(&req, &rx, reference.as_deref(), inputs)?;
    for s in &skipped {
        c.reasons.push(format!(
            "{} row(s) of the {} skipped: {}",
            s.rows,
            s.source,
            s.first.join("; ")
        ));
    }
    c.skipped_rows = skipped;
    Ok((c, copies))
}

/// Write `reset-check.json` and the inputs. The caller signs it.
pub fn write_reset(dir: &Path, c: &ResetCheck, inputs: &[(String, Vec<u8>)]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join(RESET_FILE), json_text(c)).map_err(|e| e.to_string())?;
    write_inputs(dir, inputs)
}
