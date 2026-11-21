// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Jammer enforcement evidence pack (research 2026-11-20 gaps, B0605)
//!
//! When an authority takes action over a GNSS jammer, its case rests on what the jammer did:
//! which receivers lost or degraded service, when, for how long, and that the records of it
//! were kept intact. This module assembles that part of a case file from the victims' side:
//!
//! * a **timeline** of every victim effect (start, end, kind, who reported it), with the
//!   device-off time the authority records, if any, and the custody events of a sealed capture;
//! * the **effect on victims**: per victim, durations and effect kinds; totals, the most
//!   victims affected at once, and whether effects ended when the device was switched off;
//! * the **affected area**, and only that: the grid cells where victim receivers were affected
//!   (and, where supplied, cells where receivers were present and not affected). It is built
//!   from victim positions only; no emitter position, bearing or direction-finding result is
//!   taken, computed or output, and an input column that looks like one is refused;
//! * the **custody chain** of a sealed I/Q capture ([`crate::custody`]), verified and exported;
//! * a **method statement for experts**: inputs and their hashes, rules, labels, limits;
//! * a **checklist** derived from published enforcement cases ([`CHECKLIST`], each item cited),
//!   marking what the pack supplies and what stays with the authority.
//!
//! Evidence labels: victim effects are REPORTED or MEASURED as their sources state; everything
//! computed here is DERIVED. The pack is an engineering record for the authority's case, not a
//! legal opinion; whether an offence was committed is for the authority and the court.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc, round, sha256_hex};
use crate::transport::Table;

/// Pro feature.
pub const FEATURE: &str = "pro-jammer-enforcement-pack";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-jammer-enforcement-pack/1";
/// Method.
pub const METHOD_ID: &str = "kshana-pro/jammer-enforcement/v1";
/// Evidence label.
pub const EVIDENCE: &str = "Victim effects REPORTED or MEASURED as their sources state; \
timeline, totals, concurrency and affected cells DERIVED by stated rules. MODELLED where the \
synthetic fixture is used.";
/// The scope statement every pack carries.
pub const NO_LOCATION: &str = "The affected area is the set of grid cells where victim \
receivers were affected, from the victims' own positions. This pack takes, computes and \
outputs no emitter position, bearing or direction-finding result; locating a device is the \
authority's matter and outside this pack.";

/// One checklist item derived from published enforcement cases.
pub struct CheckItem {
    pub id: &'static str,
    pub item: &'static str,
    /// What the pack supplies for it, or `None` when it stays with the authority.
    pub supplied_by: Option<&'static str>,
    pub sources: &'static [&'static str],
}

/// Published cases the checklist is derived from (read 2026-11-21).
pub const SOURCES: &[(&str, &str)] = &[
    ("FCC-13-106", "FCC, In the Matter of Gary P. Bojczak, Notice of Apparent Liability for Forfeiture, FCC 13-106 (adopted 1 Aug 2013, released 2 Aug 2013): vehicle GPS jammer, interference to the Newark Liberty GBAS; proposed forfeiture USD 31,875 (paras 3, 4, 8, 10, 12-14; n.7)"),
    ("DA-16-184", "FCC Enforcement Bureau, Gary P. Bojczak, Order and Consent Decree, DA 16-184 (2016): the investigation resolved on sworn financial information"),
    ("FCC-14-92", "FCC, In the Matter of C.T.S. Technology Co., Limited, Notice of Apparent Liability for Forfeiture, FCC 14-92 (released 19 Jun 2014): jammer marketing; purchased devices tested by the FCC Office of Engineering and Technology (para 8)"),
    ("Nantes-2017", "Tribunal correctionnel de Nantes: EUR 2,000 fine for a GPS jammer left running in a car at Nantes Atlantique airport, flights delayed; reported by The Connexion and summarised by the Resilient Navigation and Timing Foundation, 15 Sep 2017"),
];

/// The checklist. Statuses are filled per pack ([`Check`]).
pub const CHECKLIST: &[CheckItem] = &[
    CheckItem {
        id: "C1",
        item: "Complaint or report from the affected system's operator, with times",
        supplied_by: Some("victims.csv rows with source and times; timeline"),
        sources: &["FCC-13-106 para 3"],
    },
    CheckItem {
        id: "C2",
        item: "Effect on the victim system documented (harmful interference, service interrupted, operations delayed)",
        supplied_by: Some("effect on victims: per-victim effect kinds and durations, totals, services"),
        sources: &["FCC-13-106 paras 8, 10", "Nantes-2017"],
    },
    CheckItem {
        id: "C3",
        item: "Repeated or continuing interference shown over time",
        supplied_by: Some("timeline; episodes per victim; most victims affected at once"),
        sources: &["FCC-13-106 para 8"],
    },
    CheckItem {
        id: "C4",
        item: "Band of the emissions recorded (GNSS band occupied)",
        supplied_by: Some("sealed I/Q capture metadata and its custody chain"),
        sources: &["FCC-13-106 para 4, n.7"],
    },
    CheckItem {
        id: "C5",
        item: "Interference ceased when the device was switched off or surrendered",
        supplied_by: Some("cessation check of victim effects against the recorded device-off time"),
        sources: &["FCC-13-106 para 4"],
    },
    CheckItem {
        id: "C6",
        item: "Records kept intact from collection to the case file",
        supplied_by: Some("custody chain verification and export; input SHA-256; signed pack"),
        sources: &["FCC-13-106 para 4"],
    },
    CheckItem {
        id: "C7",
        item: "Duration the device was left operating (aggravating or mitigating)",
        supplied_by: Some("first and last victim effect; total affected time"),
        sources: &["FCC-13-106 para 12", "Nantes-2017"],
    },
    CheckItem {
        id: "C8",
        item: "Device identified and linked to the person; statements and admissions",
        supplied_by: None,
        sources: &["FCC-13-106 para 4"],
    },
    CheckItem {
        id: "C9",
        item: "Device tested by a laboratory (frequencies blocked, power)",
        supplied_by: None,
        sources: &["FCC-14-92 para 8"],
    },
    CheckItem {
        id: "C10",
        item: "Legal basis and sanction (statutes, penalty, mitigation such as voluntary surrender, ability to pay)",
        supplied_by: None,
        sources: &["FCC-13-106 paras 1, 9, 13-14", "DA-16-184"],
    },
    CheckItem {
        id: "C11",
        item: "Locating the device",
        supplied_by: None,
        sources: &["FCC-13-106 para 4"],
    },
];

/// Thresholds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// Affected-area grid (deg).
    pub cell_deg: f64,
    /// Effects of one victim closer than this (s) are one episode.
    pub merge_s: f64,
    /// Effects must end within this (s) after the device-off time to count as ceasing with it.
    pub cessation_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            cell_deg: 0.05,
            merge_s: 60.0,
            cessation_s: 120.0,
        }
    }
}

/// The request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub case_reference: String,
    pub data_source: String,
    /// Victim effects CSV: `victim_id,kind,start_utc,end_utc,lat,lon,effect,source` with
    /// optional `cn0_drop_db`; `effect = none` records a receiver present and not affected.
    pub victims: String,
    /// The device-off or surrender time the authority records.
    #[serde(default)]
    pub device_off_utc: Option<String>,
    /// A custody directory of a sealed capture ([`crate::custody`]).
    #[serde(default)]
    pub custody_dir: Option<String>,
    /// Public keys (hex) trusted for the custody chain.
    #[serde(default)]
    pub custody_trust: Vec<String>,
    #[serde(default)]
    pub params: Params,
}

/// Effect kinds.
pub const EFFECTS: &[&str] = &[
    "loss_of_fix",
    "degraded",
    "cn0_drop",
    "integrity_alert",
    "service_outage",
    "none",
];

#[derive(Clone, Debug)]
struct Row {
    victim: String,
    kind: String,
    t0: f64,
    t1: f64,
    lat: f64,
    lon: f64,
    effect: String,
    source: String,
    cn0: Option<f64>,
}

/// Per-victim effect.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VictimEffect {
    pub victim_id: String,
    pub kind: String,
    pub episodes: Vec<(String, String)>,
    pub affected_s: f64,
    pub effects: BTreeMap<String, usize>,
    pub max_cn0_drop_db: Option<f64>,
    pub reported_by: Vec<String>,
    /// Last effect ended within the cessation window after the device-off time.
    pub ceased_with_device_off: Option<bool>,
}

/// An affected-area cell.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cell {
    /// South-west corner `lat,lon`.
    pub cell: String,
    pub victims_affected: usize,
    pub receivers_unaffected: usize,
}

/// One timeline row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TimelineEntry {
    pub utc: String,
    pub source: String,
    pub label: &'static str,
    pub text: String,
    #[serde(skip)]
    t: f64,
}

/// A checklist status.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Check {
    pub id: &'static str,
    pub item: &'static str,
    /// `supplied`, `partial`, `missing` or `for-the-authority`.
    pub status: &'static str,
    pub detail: String,
    pub sources: Vec<&'static str>,
}

/// The pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pack {
    pub format: &'static str,
    pub method: &'static str,
    pub evidence: &'static str,
    pub scope: &'static str,
    pub title: String,
    pub case_reference: String,
    pub data_source: String,
    pub params: Params,
    pub inputs: BTreeMap<String, String>,
    pub first_effect_utc: Option<String>,
    pub last_effect_utc: Option<String>,
    pub victims_affected: usize,
    pub victim_affected_s: f64,
    pub max_concurrent_victims: usize,
    pub max_concurrent_utc: Option<String>,
    pub victims: Vec<VictimEffect>,
    pub affected_area: Vec<Cell>,
    pub device_off_utc: Option<String>,
    /// Victims whose effects ended within the cessation window of the device-off time.
    pub ceased_with_device_off: Option<String>,
    pub custody: Option<crate::custody::Chain>,
    pub timeline: Vec<TimelineEntry>,
    pub method_statement: Vec<String>,
    pub checklist: Vec<Check>,
    pub sources: Vec<(String, String)>,
    pub limits: Vec<String>,
    pub verdict: String,
}

fn read_rows(text: &str) -> Result<Vec<Row>, String> {
    let t = Table::parse(text)?;
    for c in &t.cols {
        let l = c.to_ascii_lowercase();
        if [
            "emitter",
            "jammer",
            "source_lat",
            "source_lon",
            "bearing",
            "aoa",
            "tdoa",
            "df_",
        ]
        .iter()
        .any(|k| l.contains(k))
        {
            return Err(format!(
                "column '{c}': this pack takes no emitter position, bearing or direction-finding input"
            ));
        }
    }
    let need = |n: &str| t.need(n);
    let (cv, ck, cs, ce, cla, clo, cf, cso) = (
        need("victim_id")?,
        need("kind")?,
        need("start_utc")?,
        need("end_utc")?,
        need("lat")?,
        need("lon")?,
        need("effect")?,
        need("source")?,
    );
    let cc = t.col("cn0_drop_db");
    let mut out = vec![];
    for i in 0..t.rows.len() {
        let tm =
            |c| parse_utc(t.text(i, Some(c))).ok_or_else(|| format!("row {}: bad time", i + 2));
        let (t0, t1) = (tm(cs)?, tm(ce)?);
        if t1 < t0 {
            return Err(format!("row {}: end before start", i + 2));
        }
        let effect = t.text(i, Some(cf)).to_string();
        if !EFFECTS.contains(&effect.as_str()) {
            return Err(format!(
                "row {}: effect '{effect}' is not one of {}",
                i + 2,
                EFFECTS.join(", ")
            ));
        }
        let (Some(lat), Some(lon)) = (t.num(i, Some(cla))?, t.num(i, Some(clo))?) else {
            return Err(format!("row {}: victim position missing", i + 2));
        };
        out.push(Row {
            victim: t.text(i, Some(cv)).into(),
            kind: t.text(i, Some(ck)).into(),
            t0,
            t1,
            lat,
            lon,
            effect,
            source: t.text(i, Some(cso)).into(),
            cn0: t.num(i, cc)?,
        });
    }
    Ok(out)
}

fn cell(lat: f64, lon: f64, step: f64) -> String {
    let f = |x: f64| (x / step).floor() * step;
    format!("{:.3},{:.3}", f(lat), f(lon))
}

/// Build the pack. `base` resolves relative paths.
pub fn build(q: &Request, base: &Path) -> Result<Pack, String> {
    let p = &q.params;
    for (n, v) in [
        ("data_source", &q.data_source),
        ("case_reference", &q.case_reference),
    ] {
        if v.trim().is_empty() {
            return Err(format!("{n} must be stated"));
        }
    }
    if !(p.cell_deg > 0.0 && p.merge_s >= 0.0 && p.cessation_s >= 0.0) {
        return Err("params must be positive".into());
    }
    let mut inputs = BTreeMap::new();
    let text = crate::timing::grid::read_text(base, &q.victims)?;
    inputs.insert(q.victims.clone(), sha256_hex(text.as_bytes()));
    let rows = read_rows(&text).map_err(|e| format!("{}: {e}", q.victims))?;
    let off = match &q.device_off_utc {
        Some(s) => {
            Some(parse_utc(s).ok_or_else(|| format!("device_off_utc '{s}' is not a UTC time"))?)
        }
        None => None,
    };
    let aff: Vec<&Row> = rows.iter().filter(|r| r.effect != "none").collect();

    // Per victim.
    let mut by: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for r in &aff {
        by.entry(r.victim.as_str()).or_default().push(r);
    }
    let mut victims = vec![];
    let mut total = 0.0;
    for (id, mut v) in by {
        v.sort_by(|a, b| a.t0.total_cmp(&b.t0));
        let mut eps: Vec<(f64, f64)> = vec![];
        for r in &v {
            match eps.last_mut() {
                Some(e) if r.t0 - e.1 <= p.merge_s => e.1 = e.1.max(r.t1),
                _ => eps.push((r.t0, r.t1)),
            }
        }
        let secs: f64 = eps.iter().map(|e| e.1 - e.0).sum();
        total += secs;
        let mut effects = BTreeMap::new();
        for r in &v {
            *effects.entry(r.effect.clone()).or_insert(0) += 1;
        }
        let last = eps.last().map(|e| e.1).unwrap_or(f64::NAN);
        victims.push(VictimEffect {
            victim_id: id.into(),
            kind: v[0].kind.clone(),
            episodes: eps.iter().map(|e| (iso(e.0), iso(e.1))).collect(),
            affected_s: round(secs, 1),
            effects,
            max_cn0_drop_db: v.iter().filter_map(|r| r.cn0).reduce(f64::max),
            reported_by: v
                .iter()
                .map(|r| r.source.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            ceased_with_device_off: off.map(|o| last <= o + p.cessation_s),
        });
    }

    // Concurrency (sweep).
    let mut ev: Vec<(f64, i32, &str)> = vec![];
    for v in &victims {
        for e in &v.episodes {
            ev.push((parse_utc(&e.0).unwrap_or(0.0), 1, &v.victim_id));
            ev.push((parse_utc(&e.1).unwrap_or(0.0), -1, &v.victim_id));
        }
    }
    ev.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let (mut cur, mut best, mut best_t) = (0i32, 0i32, None);
    for e in &ev {
        cur += e.1;
        if cur > best {
            best = cur;
            best_t = Some(e.0);
        }
    }

    // Affected area: cells from victim positions only.
    let mut cells: BTreeMap<String, (BTreeSet<&str>, BTreeSet<&str>)> = BTreeMap::new();
    for r in &rows {
        let c = cells.entry(cell(r.lat, r.lon, p.cell_deg)).or_default();
        if r.effect == "none" {
            c.1.insert(&r.victim);
        } else {
            c.0.insert(&r.victim);
        }
    }
    let affected_area: Vec<Cell> = cells
        .into_iter()
        .map(|(k, (a, u))| Cell {
            cell: k,
            victims_affected: a.len(),
            receivers_unaffected: u.difference(&a).count(),
        })
        .collect();

    // Custody.
    let custody = match &q.custody_dir {
        Some(d) => {
            let trust = q
                .custody_trust
                .iter()
                .map(|k| crate::license::parse_key_hex(k).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let dir = if Path::new(d).is_absolute() {
                d.into()
            } else {
                base.join(d)
            };
            let c = crate::custody::export(&dir, &trust)?;
            inputs.insert(
                format!("{d}/{}", crate::custody::DATA),
                c.form.capture_id.clone(),
            );
            Some(c)
        }
        None => None,
    };

    // Timeline.
    let mut timeline = vec![];
    for r in &aff {
        timeline.push(TimelineEntry {
            utc: iso(r.t0),
            source: r.source.clone(),
            label: "REPORTED",
            text: format!("{} ({}): {} starts", r.victim, r.kind, r.effect),
            t: r.t0,
        });
        timeline.push(TimelineEntry {
            utc: iso(r.t1),
            source: r.source.clone(),
            label: "REPORTED",
            text: format!("{} ({}): {} ends", r.victim, r.kind, r.effect),
            t: r.t1,
        });
    }
    if let Some(o) = off {
        timeline.push(TimelineEntry {
            utc: iso(o),
            source: "authority".into(),
            label: "REPORTED",
            text: "Device switched off or surrendered (as the authority records)".into(),
            t: o,
        });
    }
    if let Some(c) = &custody {
        if let Some(t) = parse_utc(&c.form.collected_utc) {
            timeline.push(TimelineEntry {
                utc: iso(t),
                source: c.form.collected_by.clone(),
                label: "MEASURED",
                text: format!(
                    "Sealed I/Q capture {} collected",
                    &c.form.capture_id[..12.min(c.form.capture_id.len())]
                ),
                t,
            });
        }
        for h in &c.handovers {
            if let Some(t) = parse_utc(&h.utc) {
                timeline.push(TimelineEntry {
                    utc: iso(t),
                    source: h.from.clone(),
                    label: "REPORTED",
                    text: format!("Custody handover {} to {} ({})", h.from, h.to, h.purpose),
                    t,
                });
            }
        }
    }
    timeline.sort_by(|a, b| a.t.total_cmp(&b.t));

    let first = aff.iter().map(|r| r.t0).reduce(f64::min);
    let last = aff.iter().map(|r| r.t1).reduce(f64::max);
    let ceased = off.map(|_| {
        let n = victims
            .iter()
            .filter(|v| v.ceased_with_device_off == Some(true))
            .count();
        format!(
            "{n} of {} affected victims' effects ended within {:.0} s after the recorded device-off time",
            victims.len(),
            p.cessation_s
        )
    });
    let all_ceased = victims
        .iter()
        .all(|v| v.ceased_with_device_off == Some(true));
    let custody_ok = custody
        .as_ref()
        .map(|c| c.verification.passed() && c.verification.trusted != Some(false));

    // Checklist.
    let reported_with_source = aff.iter().all(|r| !r.source.trim().is_empty());
    let checklist = CHECKLIST
        .iter()
        .map(|c| {
            let (status, detail) = match c.id {
                "C1" if !aff.is_empty() && reported_with_source => (
                    "supplied",
                    format!(
                        "{} victim effect record(s), each with its source",
                        aff.len()
                    ),
                ),
                "C1" => ("missing", "no victim effect with a named source".into()),
                "C2" if !victims.is_empty() => (
                    "supplied",
                    format!(
                        "{} victim(s), {:.0} victim-seconds affected",
                        victims.len(),
                        total
                    ),
                ),
                "C2" => ("missing", "no affected victim".into()),
                "C3" => {
                    let rep = victims.iter().any(|v| v.episodes.len() > 1) || victims.len() > 1;
                    if rep {
                        ("supplied", format!("up to {best} victims affected at once"))
                    } else {
                        (
                            "partial",
                            "a single effect episode on a single victim".into(),
                        )
                    }
                }
                "C4" => match &custody {
                    Some(_) => (
                        "supplied",
                        "sealed capture; band and sample rate in its SigMF metadata".into(),
                    ),
                    None => ("missing", "no sealed capture (custody_dir)".into()),
                },
                "C5" => match (off, all_ceased) {
                    (Some(_), true) => ("supplied", ceased.clone().unwrap_or_default()),
                    (Some(_), false) => ("partial", ceased.clone().unwrap_or_default()),
                    (None, _) => ("missing", "no device_off_utc recorded".into()),
                },
                "C6" => match custody_ok {
                    Some(true) => (
                        "supplied",
                        "custody chain verifies; inputs hashed; pack signed".into(),
                    ),
                    Some(false) => (
                        "partial",
                        "custody chain does not verify or is not trusted; see custody.verification"
                            .into(),
                    ),
                    None => (
                        "partial",
                        "inputs hashed and pack signed; no capture custody chain".into(),
                    ),
                },
                "C7" => match (first, last) {
                    (Some(a), Some(b)) => (
                        "supplied",
                        format!("effects from {} to {} ({:.0} s)", iso(a), iso(b), b - a),
                    ),
                    _ => ("missing", "no effect".into()),
                },
                _ => (
                    "for-the-authority",
                    c.supplied_by.unwrap_or("outside this pack").to_string(),
                ),
            };
            Check {
                id: c.id,
                item: c.item,
                status,
                detail: if c.supplied_by.is_none() {
                    "Outside this pack: the authority's investigation".into()
                } else {
                    detail
                },
                sources: c.sources.to_vec(),
            }
        })
        .collect::<Vec<_>>();

    let method_statement = vec![
        format!("Method {METHOD_ID}. Inputs and their SHA-256 are listed under inputs; the pack directory is signed as a whole."),
        "Victim effects are taken as the victim's operator or receiver log states them (REPORTED or MEASURED); this method does not re-detect them.".into(),
        format!("A victim's effects closer than {:.0} s are merged into one episode; affected time is the sum of episode durations.", p.merge_s),
        "Concurrency is the largest number of victims with an open episode at the same instant.".into(),
        format!("Affected area: {:.3}-degree cells holding a victim position while affected; cells of receivers present and not affected are listed beside them.", p.cell_deg),
        format!("Cessation: a victim's effects ceased with the device if its last episode ended no later than {:.0} s after the recorded device-off time.", p.cessation_s),
        "The custody chain of a sealed capture is verified (every byte, signature and handover link) and exported unchanged.".into(),
        NO_LOCATION.into(),
        "Reproduce: run the same request on the same inputs with the same method version; the outputs are deterministic.".into(),
    ];
    let verdict = format!(
        "{} victim(s) affected for {:.0} victim-seconds; up to {} at once; checklist: {} supplied, {} partial, {} missing, {} for the authority",
        victims.len(),
        total,
        best,
        checklist.iter().filter(|c| c.status == "supplied").count(),
        checklist.iter().filter(|c| c.status == "partial").count(),
        checklist.iter().filter(|c| c.status == "missing").count(),
        checklist.iter().filter(|c| c.status == "for-the-authority").count(),
    );
    Ok(Pack {
        format: FORMAT,
        method: METHOD_ID,
        evidence: EVIDENCE,
        scope: NO_LOCATION,
        title: q.title.clone(),
        case_reference: q.case_reference.clone(),
        data_source: q.data_source.clone(),
        params: p.clone(),
        inputs,
        first_effect_utc: first.map(iso),
        last_effect_utc: last.map(iso),
        victims_affected: victims.len(),
        victim_affected_s: round(total, 1),
        max_concurrent_victims: best.max(0) as usize,
        max_concurrent_utc: best_t.map(iso),
        victims,
        affected_area,
        device_off_utc: off.map(iso),
        ceased_with_device_off: ceased,
        custody,
        timeline,
        method_statement,
        checklist,
        sources: SOURCES
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
        limits: vec![
            "Victim effects are as reported; their accuracy is their sources'.".into(),
            "Cells show where victims were affected, not where a device was.".into(),
            "Cessation at the device-off time is consistent with, not proof of, the device being the cause.".into(),
            "Not a legal opinion; admissibility and the offence are for the authority and the court.".into(),
        ],
        verdict,
    })
}

/// HTML page.
pub fn html(k: &Pack) -> String {
    let mut intro = vec![
        format!("Case {}. Data source: {}.", k.case_reference, k.data_source),
        k.scope.to_string(),
    ];
    if let Some(c) = &k.ceased_with_device_off {
        intro.push(c.clone());
    }
    crate::transport::page(
        &k.title,
        k.evidence,
        &k.verdict,
        &intro,
        &[
            (
                "Effect on victims".into(),
                vec![
                    "victim",
                    "kind",
                    "episodes",
                    "affected (s)",
                    "effects",
                    "reported by",
                ],
                k.victims
                    .iter()
                    .map(|v| {
                        vec![
                            v.victim_id.clone(),
                            v.kind.clone(),
                            v.episodes
                                .iter()
                                .map(|e| format!("{} to {}", e.0, e.1))
                                .collect::<Vec<_>>()
                                .join("; "),
                            format!("{:.0}", v.affected_s),
                            v.effects
                                .iter()
                                .map(|(a, n)| format!("{a} ({n})"))
                                .collect::<Vec<_>>()
                                .join(", "),
                            v.reported_by.join(", "),
                        ]
                    })
                    .collect(),
            ),
            (
                "Affected area (victim receivers only)".into(),
                vec![
                    "cell (SW corner)",
                    "victims affected",
                    "receivers not affected",
                ],
                k.affected_area
                    .iter()
                    .map(|c| {
                        vec![
                            c.cell.clone(),
                            c.victims_affected.to_string(),
                            c.receivers_unaffected.to_string(),
                        ]
                    })
                    .collect(),
            ),
            (
                "Timeline".into(),
                vec!["UTC", "source", "label", "event"],
                k.timeline
                    .iter()
                    .map(|e| {
                        vec![
                            e.utc.clone(),
                            e.source.clone(),
                            e.label.to_string(),
                            e.text.clone(),
                        ]
                    })
                    .collect(),
            ),
            (
                "Checklist from published cases".into(),
                vec!["id", "item", "status", "detail", "cited"],
                k.checklist
                    .iter()
                    .map(|c| {
                        vec![
                            c.id.to_string(),
                            c.item.to_string(),
                            c.status.to_string(),
                            c.detail.clone(),
                            c.sources.join("; "),
                        ]
                    })
                    .collect(),
            ),
            (
                "Sources".into(),
                vec!["key", "case"],
                k.sources
                    .iter()
                    .map(|(a, b)| vec![a.clone(), b.clone()])
                    .collect(),
            ),
            (
                "Method statement for experts".into(),
                vec!["statement"],
                k.method_statement.iter().map(|m| vec![m.clone()]).collect(),
            ),
            (
                "Limits".into(),
                vec!["limit"],
                k.limits.iter().map(|m| vec![m.clone()]).collect(),
            ),
        ],
    )
}

/// A SYNTHETIC case: a vehicle jammer near an airfield affecting a ground augmentation
/// station, two aircraft on approach and a fleet tracker; one receiver unaffected; the device
/// switched off at 10:40:00Z. Returns (victims.csv, request.toml without custody).
pub fn synth() -> (String, String) {
    let csv = "victim_id,kind,start_utc,end_utc,lat,lon,effect,source,cn0_drop_db\n\
GBAS-1,ground-augmentation-station,2026-06-01T10:02:00Z,2026-06-01T10:09:30Z,51.012,0.512,integrity_alert,station operator log,14\n\
GBAS-1,ground-augmentation-station,2026-06-01T10:21:00Z,2026-06-01T10:39:40Z,51.012,0.512,service_outage,station operator log,22\n\
AC-ALPHA,aircraft,2026-06-01T10:24:10Z,2026-06-01T10:26:40Z,51.030,0.470,loss_of_fix,crew report via ATC,\n\
AC-BRAVO,aircraft,2026-06-01T10:33:00Z,2026-06-01T10:35:20Z,51.028,0.480,degraded,FDM extract,9\n\
TRK-07,fleet-tracker,2026-06-01T10:05:00Z,2026-06-01T10:39:50Z,51.010,0.520,cn0_drop,tracker telemetry,18\n\
REF-STN,reference-station,2026-06-01T10:00:00Z,2026-06-01T10:45:00Z,51.120,0.610,none,reference station log,\n";
    let req = "title = \"SYNTHETIC jammer enforcement pack\"\ncase_reference = \"SYNTH-0001\"\n\
data_source = \"SYNTHETIC (kshana-pro enforcement synth)\"\nvictims = \"victims.csv\"\n\
device_off_utc = \"2026-06-01T10:40:00Z\"\n";
    (csv.into(), req.into())
}
