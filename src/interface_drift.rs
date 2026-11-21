// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Interface-change regression for correction decoders (research 2026-11-20 gaps, B1604)
//!
//! Correction services change their interfaces: a new constellation in a mask, a new message
//! subtype, a new transmitting satellite (QZSS IS-QZSS-L6 streams such as MADOCA-PPP and CLAS,
//! and the move to seven QZSS satellites). A decoder that silently drops what it does not know
//! degrades without telling anyone. This module is decoder-neutral:
//!
//! * an **interface profile** (`kshana-pro-interface-profile/1`) states, for one stream and one
//!   interface version, the message types and subtypes, the GNSS identifiers allowed in masks,
//!   the transmitting satellite PRNs and any fixed header fields, each with its source. Kshana
//!   ships only a **template** whose values are placeholders ([`template`]): the values come from
//!   the published interface specification the user holds;
//! * **drift check** ([`check`]): decoded header records (`timestamp,stream,message_type,
//!   subtype,gnss_ids,prn[,field=value;...]`, one per decoded message, from any decoder) against
//!   the profile; the first appearance of anything unknown raises an alarm with its time, the
//!   element and how often it recurred, so a decoder meeting an unknown version is caught;
//! * **regression vectors** ([`vectors`]): per profile version, header records that must be
//!   accepted (every known element) and mutated records that must be flagged (an unknown GNSS
//!   identifier, subtype, message type, PRN or field value); [`score`] scores a decoder's
//!   accept/flag output on them.
//!
//! MODELLED: a regression harness around decoders; it decodes no signal itself.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::parse_utc;
use crate::transport::Table;

/// Pro feature.
pub const FEATURE: &str = "pro-decoder-regression";
/// Formats.
pub const PROFILE_FORMAT: &str = "kshana-pro-interface-profile/1";
pub const VECTORS_FORMAT: &str = "kshana-pro-interface-vectors/1";
pub const CHECK_FORMAT: &str = "kshana-pro-interface-drift/1";

/// An interface profile for one stream version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub format: String,
    pub stream: String,
    pub version: String,
    /// Where the values come from (specification, revision, section).
    pub source: String,
    pub message_types: Vec<u32>,
    #[serde(default)]
    pub subtypes: Vec<u32>,
    #[serde(default)]
    pub gnss_ids: Vec<String>,
    #[serde(default)]
    pub prns: Vec<u32>,
    /// Header fields with their allowed values.
    #[serde(default)]
    pub fields: BTreeMap<String, Vec<String>>,
}

impl Profile {
    /// Check the profile.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != PROFILE_FORMAT {
            return Err(format!("format '{}' is not {PROFILE_FORMAT}", self.format));
        }
        if self.source.trim().is_empty() || self.source.contains("PLACEHOLDER") {
            return Err(format!(
                "profile {} {}: the source must name the specification the values come from",
                self.stream, self.version
            ));
        }
        if self.message_types.is_empty() {
            return Err("message_types must list at least one type".into());
        }
        Ok(())
    }
}

/// A template profile: every value a placeholder to fill from the specification.
pub fn template(stream: &str) -> String {
    format!(
        "# Interface profile template. Fill every value from the published interface specification\n\
         # (e.g. IS-QZSS-L6 for MADOCA-PPP or CLAS), with its revision and section in `source`.\n\
         # Kshana ships no values: the lists below are empty placeholders.\n\
         format = \"{PROFILE_FORMAT}\"\nstream = \"{stream}\"\nversion = \"PLACEHOLDER\"\n\
         source = \"PLACEHOLDER: specification, revision, section\"\n\
         message_types = []   # PLACEHOLDER\nsubtypes = []        # PLACEHOLDER\n\
         gnss_ids = []        # PLACEHOLDER (e.g. the GNSS identifiers the mask may carry)\n\
         prns = []            # PLACEHOLDER (transmitting satellites)\n\n[fields]             # PLACEHOLDER: header field = [allowed values]\n"
    )
}

/// One decoded header record.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Record {
    pub t: f64,
    pub stream: String,
    pub message_type: u32,
    pub subtype: Option<u32>,
    pub gnss_ids: Vec<String>,
    pub prn: Option<u32>,
    pub fields: BTreeMap<String, String>,
}

/// Parse decoded header records.
pub fn parse_records(text: &str) -> Result<Vec<Record>, String> {
    let t = Table::parse(text)?;
    let (ct, cs, cm) = (
        t.need("timestamp")?,
        t.need("stream")?,
        t.need("message_type")?,
    );
    let (csub, cg, cp, cf) = (
        t.col("subtype"),
        t.col("gnss_ids"),
        t.col("prn"),
        t.col("fields"),
    );
    // An empty subtype or PRN cell is "none"; anything else must be a number, or a decoder
    // that writes `G33` would make the unknown-PRN alarm silently never fire.
    let opt = |i: usize, c: Option<usize>, what: &str| -> Result<Option<u32>, String> {
        let v = t.text(i, c);
        if v.is_empty() {
            return Ok(None);
        }
        v.parse()
            .map(Some)
            .map_err(|_| format!("row {}: {what} '{v}' is not a number", i + 2))
    };
    let mut out = Vec::new();
    for i in 0..t.rows.len() {
        let ts = t.text(i, Some(ct));
        let tt = parse_utc(ts).ok_or_else(|| format!("row {}: bad timestamp '{ts}'", i + 2))?;
        let mt = t.text(i, Some(cm));
        out.push(Record {
            t: tt,
            stream: t.text(i, Some(cs)).to_string(),
            message_type: mt
                .parse()
                .map_err(|_| format!("row {}: message_type '{mt}'", i + 2))?,
            subtype: opt(i, csub, "subtype")?,
            gnss_ids: t
                .text(i, cg)
                .split(['|', ' '])
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            prn: opt(i, cp, "prn")?,
            fields: t
                .text(i, cf)
                .split(';')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                .collect(),
        });
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok(out)
}

/// What is unknown in a record against a profile.
pub fn unknowns(p: &Profile, r: &Record) -> Vec<String> {
    let mut u = Vec::new();
    if !p.message_types.contains(&r.message_type) {
        u.push(format!("message type {}", r.message_type));
    }
    if let Some(s) = r.subtype {
        if !p.subtypes.is_empty() && !p.subtypes.contains(&s) {
            u.push(format!("subtype {s}"));
        }
    }
    for g in &r.gnss_ids {
        if !p.gnss_ids.contains(g) {
            u.push(format!("GNSS id {g}"));
        }
    }
    if let Some(n) = r.prn {
        if !p.prns.is_empty() && !p.prns.contains(&n) {
            u.push(format!("transmitting PRN {n}"));
        }
    }
    for (k, v) in &r.fields {
        if let Some(allowed) = p.fields.get(k) {
            if !allowed.contains(v) {
                u.push(format!("field {k}={v}"));
            }
        }
    }
    u
}

/// One drift alarm.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Alarm {
    pub element: String,
    pub first_utc: String,
    pub occurrences: usize,
    pub last_utc: String,
}

/// The drift check of one stream.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Drift {
    pub format: &'static str,
    pub stream: String,
    pub profile_version: String,
    pub profile_source: String,
    pub records: usize,
    pub other_streams: usize,
    pub alarms: Vec<Alarm>,
    pub verdict: String,
}

/// Check records against a profile.
pub fn check(p: &Profile, records: &[Record]) -> Result<Drift, String> {
    p.validate()?;
    let mut alarms: BTreeMap<String, Alarm> = BTreeMap::new();
    let mut n = 0;
    for r in records.iter().filter(|r| r.stream == p.stream) {
        n += 1;
        for e in unknowns(p, r) {
            let iso = crate::forensics::inputs::iso(r.t);
            let a = alarms.entry(e.clone()).or_insert(Alarm {
                element: e,
                first_utc: iso.clone(),
                occurrences: 0,
                last_utc: iso.clone(),
            });
            a.occurrences += 1;
            a.last_utc = iso;
        }
    }
    let mut v: Vec<Alarm> = alarms.into_values().collect();
    v.sort_by(|a, b| {
        a.first_utc
            .cmp(&b.first_utc)
            .then(a.element.cmp(&b.element))
    });
    let verdict = if v.is_empty() {
        format!(
            "{n} record(s) of {} within interface version {}",
            p.stream, p.version
        )
    } else {
        format!(
            "INTERFACE DRIFT: {} element(s) unknown to version {} of {}, first {} at {}; the decoder meets a newer interface than its profile",
            v.len(),
            p.version,
            p.stream,
            v[0].element,
            v[0].first_utc
        )
    };
    Ok(Drift {
        format: CHECK_FORMAT,
        stream: p.stream.clone(),
        profile_version: p.version.clone(),
        profile_source: p.source.clone(),
        records: n,
        other_streams: records.len() - n,
        alarms: v,
        verdict,
    })
}

/// One regression vector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    pub id: String,
    pub message_type: u32,
    pub subtype: Option<u32>,
    pub gnss_ids: Vec<String>,
    pub prn: Option<u32>,
    pub fields: BTreeMap<String, String>,
    /// `accept` or `flag`.
    pub expect: String,
    pub why: String,
}

/// A vector set for one profile version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VectorSet {
    pub format: String,
    pub stream: String,
    pub version: String,
    pub source: String,
    pub vectors: Vec<Vector>,
}

/// Regression vectors of a profile: one accepted record per known combination element, and one
/// flagged mutation per kind of unknown element.
pub fn vectors(p: &Profile) -> Result<VectorSet, String> {
    p.validate()?;
    let base_fields: BTreeMap<String, String> = p
        .fields
        .iter()
        .filter_map(|(k, v)| v.first().map(|x| (k.clone(), x.clone())))
        .collect();
    let mt0 = p.message_types[0];
    let st0 = p.subtypes.first().copied();
    let prn0 = p.prns.first().copied();
    let mut v = Vec::new();
    let mut k = 0;
    let mut push = |v: &mut Vec<Vector>,
                    mt,
                    st,
                    g: Vec<String>,
                    prn,
                    f: BTreeMap<String, String>,
                    expect: &str,
                    why: String| {
        k += 1;
        v.push(Vector {
            id: format!("{}-{}-{k:03}", p.stream, p.version),
            message_type: mt,
            subtype: st,
            gnss_ids: g,
            prn,
            fields: f,
            expect: expect.into(),
            why,
        });
    };
    push(
        &mut v,
        mt0,
        st0,
        p.gnss_ids.clone(),
        prn0,
        base_fields.clone(),
        "accept",
        "every GNSS id of the profile together".into(),
    );
    for &mt in &p.message_types {
        push(
            &mut v,
            mt,
            st0,
            vec![],
            prn0,
            base_fields.clone(),
            "accept",
            format!("message type {mt}"),
        );
    }
    for &st in &p.subtypes {
        push(
            &mut v,
            mt0,
            Some(st),
            vec![],
            prn0,
            base_fields.clone(),
            "accept",
            format!("subtype {st}"),
        );
    }
    for &n in &p.prns {
        push(
            &mut v,
            mt0,
            st0,
            vec![],
            Some(n),
            base_fields.clone(),
            "accept",
            format!("transmitting PRN {n}"),
        );
    }
    // Mutations.
    let unknown_mt = (0..)
        .find(|x| !p.message_types.contains(x))
        .expect("finite list");
    push(
        &mut v,
        unknown_mt,
        st0,
        vec![],
        prn0,
        base_fields.clone(),
        "flag",
        format!("unknown message type {unknown_mt}"),
    );
    if !p.subtypes.is_empty() {
        let s = (0..)
            .find(|x| !p.subtypes.contains(x))
            .expect("finite list");
        push(
            &mut v,
            mt0,
            Some(s),
            vec![],
            prn0,
            base_fields.clone(),
            "flag",
            format!("unknown subtype {s}"),
        );
    }
    let g = ["G", "R", "E", "C", "J", "I", "S", "L"]
        .iter()
        .find(|g| !p.gnss_ids.iter().any(|x| x == *g))
        .map(|g| g.to_string())
        .unwrap_or_else(|| "X".into());
    let mut gg = p.gnss_ids.clone();
    gg.push(g.clone());
    push(
        &mut v,
        mt0,
        st0,
        gg,
        prn0,
        base_fields.clone(),
        "flag",
        format!("unknown GNSS id {g} added to a mask"),
    );
    if !p.prns.is_empty() {
        let n = p.prns.iter().max().copied().unwrap_or(0) + 1;
        push(
            &mut v,
            mt0,
            st0,
            vec![],
            Some(n),
            base_fields.clone(),
            "flag",
            format!("unknown transmitting PRN {n}"),
        );
    }
    for k2 in p.fields.keys() {
        let mut f = base_fields.clone();
        f.insert(k2.clone(), "UNKNOWN".into());
        push(
            &mut v,
            mt0,
            st0,
            vec![],
            prn0,
            f,
            "flag",
            format!("unknown value of field {k2}"),
        );
    }
    // Self-check: every vector's expectation matches the profile.
    for x in &v {
        let r = Record {
            t: 0.0,
            stream: p.stream.clone(),
            message_type: x.message_type,
            subtype: x.subtype,
            gnss_ids: x.gnss_ids.clone(),
            prn: x.prn,
            fields: x.fields.clone(),
        };
        let flagged = !unknowns(p, &r).is_empty();
        if flagged != (x.expect == "flag") {
            return Err(format!("vector {} does not match its expectation", x.id));
        }
    }
    Ok(VectorSet {
        format: VECTORS_FORMAT.into(),
        stream: p.stream.clone(),
        version: p.version.clone(),
        source: p.source.clone(),
        vectors: v,
    })
}

/// A decoder's result on a vector set.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Score {
    pub vectors: usize,
    pub answered: usize,
    pub correct: usize,
    /// Vectors the decoder accepted although they carry an unknown element (silent drift).
    pub silently_accepted: Vec<String>,
    /// Known vectors the decoder flagged.
    pub wrongly_flagged: Vec<String>,
    pub missing: Vec<String>,
    pub verdict: String,
}

/// Score a decoder's output (`id,result` with `accept` or `flag`).
pub fn score(set: &VectorSet, output: &str) -> Result<Score, String> {
    let t = Table::parse(output)?;
    let (ci, cr) = (t.need("id")?, t.need("result")?);
    let got: BTreeMap<String, String> = (0..t.rows.len())
        .map(|i| {
            (
                t.text(i, Some(ci)).to_string(),
                t.text(i, Some(cr)).to_ascii_lowercase(),
            )
        })
        .collect();
    let mut s = Score {
        vectors: set.vectors.len(),
        answered: 0,
        correct: 0,
        silently_accepted: vec![],
        wrongly_flagged: vec![],
        missing: vec![],
        verdict: String::new(),
    };
    let ids: BTreeSet<&String> = set.vectors.iter().map(|v| &v.id).collect();
    for v in &set.vectors {
        match got.get(&v.id) {
            None => s.missing.push(v.id.clone()),
            Some(r) => {
                s.answered += 1;
                if r == &v.expect {
                    s.correct += 1;
                } else if v.expect == "flag" {
                    s.silently_accepted.push(v.id.clone());
                } else {
                    s.wrongly_flagged.push(v.id.clone());
                }
            }
        }
    }
    let extra = got.keys().filter(|k| !ids.contains(k)).count();
    s.verdict = format!(
        "{} of {} vectors correct; {} unknown element(s) silently accepted, {} known record(s) flagged, {} unanswered{}",
        s.correct,
        s.vectors,
        s.silently_accepted.len(),
        s.wrongly_flagged.len(),
        s.missing.len(),
        if extra > 0 { format!(", {extra} answer(s) for unknown ids") } else { String::new() }
    );
    Ok(s)
}
