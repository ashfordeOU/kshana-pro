// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Fleet revalidation planner over open product registers (research 2026-11-20 gaps, B2505)
//!
//! The open engine publishes the **product register** (`kshana.register/1`: products scored
//! under a stated method, each with the date it is to be scored again) and reads the issuer's
//! signed **revocation list** (`kshana.register-crl/1`); `kshana register view` joins one
//! register and one list at a date. Registers are signed with the open `kshana register sign`
//! and lists by their issuer; nothing here signs either.
//!
//! This module plans across many registers and entries: per entry, the open status at the plan
//! date (`current`, `due` or `revoked`; revocation `unknown`, `checked` or `stale`, a stale
//! list's revocations still applying), the date re-scoring should start (the revalidation date
//! less the maker's lead time) and a flag (`OVERDUE`, `DUE SOON` within the warning days,
//! `SCHEDULED`, `REVOKED`); then a calendar by month and a table per maker, written as a signed
//! report with CSV and iCalendar.
//!
//! The two documents are read here by their documented format (schema id, a newline, then the
//! compact JSON in struct order with an empty `signature`, signed with Ed25519).
//! TODO: switch to the open types (`kshana::product_register`) once they are on the open
//! engine's main branch and in the pinned release (#159).
//!
//! **MODELLED**: a plan from the registers supplied. An entry says only that a product was
//! scored under a method; the plan says when it is to be scored again.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc};

/// Pro feature.
pub const FEATURE: &str = "pro-revalidation-plan";
/// Report format.
pub const FORMAT: &str = "kshana-pro-revalidation-plan/1";
/// Open schemas read here.
pub const REGISTER_SCHEMA: &str = "kshana.register/1";
pub const CRL_SCHEMA: &str = "kshana.register-crl/1";
/// Evidence label.
pub const EVIDENCE: &str = "MODELLED";

// ---------------------------------------------------------------------------------------
// The open documents (field order and serialisation as documented; see the TODO above).

/// One register entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub product: String,
    pub maker: String,
    pub version: String,
    pub method: String,
    pub method_version: String,
    pub scored: String,
    pub revalidate_by: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_set_version: Option<String>,
}

/// A signed register.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Register {
    pub schema: String,
    pub publisher: String,
    pub issued: String,
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub signer: String,
    #[serde(default)]
    pub signature: String,
}

/// One revoked entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revoked {
    pub id: String,
    pub revoked: String,
    pub reason: String,
}

/// A signed revocation list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crl {
    pub schema: String,
    pub issuer: String,
    pub issued: String,
    pub next_update: String,
    pub revoked: Vec<Revoked>,
    #[serde(default)]
    pub signer: String,
    #[serde(default)]
    pub signature: String,
}

/// The bytes a signature covers: the schema id, a newline, then the compact JSON of `doc` (in
/// struct order) whose `signature` the caller has emptied.
pub fn signing_bytes<T: Serialize>(schema: &str, doc: &T) -> Vec<u8> {
    let mut out = format!("{schema}\n").into_bytes();
    out.extend(serde_json::to_vec(doc).expect("serialises"));
    out
}

fn register_bytes(r: &Register) -> Vec<u8> {
    let mut d = r.clone();
    d.signature.clear();
    signing_bytes(REGISTER_SCHEMA, &d)
}

fn crl_bytes(c: &Crl) -> Vec<u8> {
    let mut d = c.clone();
    d.signature.clear();
    signing_bytes(CRL_SCHEMA, &d)
}

fn verify_sig(
    bytes: &[u8],
    signer: &str,
    signature: &str,
    pin: Option<&str>,
) -> Result<(), String> {
    if let Some(want) = pin {
        if !want.trim().eq_ignore_ascii_case(signer.trim()) {
            return Err(format!("signed by key {signer}, expected {}", want.trim()));
        }
    }
    let pk = hex::decode(signer.trim())
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v.as_slice()).ok());
    let sig = hex::decode(signature.trim())
        .ok()
        .and_then(|v| <[u8; 64]>::try_from(v.as_slice()).ok());
    let (Some(pk), Some(sig)) = (pk, sig) else {
        return Err("the signature does not verify".into());
    };
    let ok = VerifyingKey::from_bytes(&pk)
        .map(|k| k.verify_strict(bytes, &Signature::from_bytes(&sig)).is_ok())
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err("the signature does not verify".into())
    }
}

/// Whether `s` is a calendar date written YYYY-MM-DD.
pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    if !s
        .bytes()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let (y, m, d): (u32, u32, u32) = (
        s[0..4].parse().unwrap_or(0),
        s[5..7].parse().unwrap_or(0),
        s[8..10].parse().unwrap_or(0),
    );
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

fn need_date(what: &str, v: &str) -> Result<(), String> {
    if is_date(v) {
        Ok(())
    } else {
        Err(format!("{what}: '{v}' is not a YYYY-MM-DD date"))
    }
}

/// Check a register: schema, signature (against `pin` when given), dates, unique ids, and no
/// revalidation date before the scoring date.
pub fn check_register(text: &str, pin: Option<&str>) -> Result<Register, String> {
    let r: Register =
        serde_json::from_str(text).map_err(|e| format!("not a {REGISTER_SCHEMA}: {e}"))?;
    if r.schema != REGISTER_SCHEMA {
        return Err(format!("schema '{}', expected {REGISTER_SCHEMA}", r.schema));
    }
    verify_sig(&register_bytes(&r), &r.signer, &r.signature, pin)
        .map_err(|e| format!("register: {e}"))?;
    need_date("issued", &r.issued)?;
    let mut ids = BTreeSet::new();
    for e in &r.entries {
        if !ids.insert(e.id.as_str()) {
            return Err(format!("register: entry '{}' twice", e.id));
        }
        need_date(&format!("{}: scored", e.id), &e.scored)?;
        need_date(&format!("{}: revalidate_by", e.id), &e.revalidate_by)?;
        if e.revalidate_by < e.scored {
            return Err(format!("{}: revalidate_by before scored", e.id));
        }
    }
    Ok(r)
}

/// Check a revocation list against its issuer key `pin` (required).
pub fn check_crl(text: &str, pin: &str) -> Result<Crl, String> {
    let c: Crl = serde_json::from_str(text).map_err(|e| format!("not a {CRL_SCHEMA}: {e}"))?;
    if c.schema != CRL_SCHEMA {
        return Err(format!("schema '{}', expected {CRL_SCHEMA}", c.schema));
    }
    if pin.trim().is_empty() {
        return Err("revocation list: the issuer key is needed".into());
    }
    verify_sig(&crl_bytes(&c), &c.signer, &c.signature, Some(pin))
        .map_err(|e| format!("revocation list: {e}"))?;
    need_date("issued", &c.issued)?;
    need_date("next_update", &c.next_update)?;
    if c.next_update < c.issued {
        return Err("revocation list: next_update before issued".into());
    }
    for v in &c.revoked {
        need_date(&format!("{}: revoked", v.id), &v.revoked)?;
    }
    Ok(c)
}

// ---------------------------------------------------------------------------------------
// The plan.

/// One register in the request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRef {
    pub file: String,
    /// The publisher's key, hex (pins the register signature; any key when absent).
    #[serde(default)]
    pub key: Option<String>,
    /// The revocation list that applies to this register.
    #[serde(default)]
    pub crl: Option<String>,
    /// The list issuer's key, hex (required with `crl`).
    #[serde(default)]
    pub crl_key: Option<String>,
}

/// A maker's lead time.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lead {
    pub maker: String,
    pub days: u32,
}

fn d_warn() -> u32 {
    60
}
fn d_lead() -> u32 {
    90
}

/// The request (`request.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    #[serde(default = "d_warn")]
    pub warn_days: u32,
    /// Days before the revalidation date that re-scoring should start, unless a maker has its
    /// own.
    #[serde(default = "d_lead")]
    pub lead_days: u32,
    #[serde(default)]
    pub lead: Vec<Lead>,
    #[serde(rename = "register")]
    pub registers: Vec<RegisterRef>,
}

/// One register as read.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterInfo {
    pub file: String,
    pub publisher: String,
    pub signer: String,
    pub issued: String,
    pub entries: usize,
    pub crl_issuer: Option<String>,
    pub crl_next_update: Option<String>,
    /// `unknown` (no list), `checked` or `stale`.
    pub revocation: String,
    /// Revocations naming no entry of this register.
    pub crl_unmatched: Vec<String>,
}

/// One entry in the plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub publisher: String,
    pub id: String,
    pub product: String,
    pub maker: String,
    pub version: String,
    pub method: String,
    pub method_version: String,
    pub scored: String,
    pub revalidate_by: String,
    pub lead_days: u32,
    pub start_by: String,
    pub days_left: i64,
    /// Open status: `current`, `due` or `revoked`.
    pub status: String,
    /// `unknown`, `checked` or `stale`.
    pub revocation: String,
    /// `OVERDUE`, `DUE SOON`, `SCHEDULED` or `REVOKED`.
    pub flag: String,
    pub revoked: Option<Revoked>,
}

/// One maker.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MakerRow {
    pub maker: String,
    pub entries: usize,
    pub overdue: usize,
    pub due_soon: usize,
    pub scheduled: usize,
    pub revoked: usize,
    pub next_revalidate_by: Option<String>,
    pub next_start_by: Option<String>,
}

/// The plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub format: String,
    pub title: String,
    pub evidence: String,
    pub date: String,
    pub warn_days: u32,
    pub lead_days: u32,
    pub lead: Vec<Lead>,
    pub verdict: String,
    pub registers: Vec<RegisterInfo>,
    pub items: Vec<Item>,
    /// Per month of the revalidation date (YYYY-MM), entries per flag.
    pub by_month: BTreeMap<String, BTreeMap<String, usize>>,
    pub makers: Vec<MakerRow>,
    pub caveats: Vec<String>,
}

fn day(d: &str) -> f64 {
    parse_utc(&format!("{d}T00:00:00Z")).expect("checked date") / 86_400.0
}

fn date_minus(d: &str, days: u32) -> String {
    iso((day(d) - f64::from(days)) * 86_400.0)[..10].to_string()
}

/// One register with its optional list, read and checked.
pub type Loaded = (String, Register, Option<Crl>);

/// The plan at `date` (YYYY-MM-DD) from registers already checked.
pub fn plan(q: &Request, loaded: &[Loaded], date: &str) -> Result<Plan, String> {
    need_date("plan date", date)?;
    if loaded.is_empty() {
        return Err("no [[register]]".into());
    }
    let lead_of = |maker: &str| {
        q.lead
            .iter()
            .find(|l| l.maker == maker)
            .map(|l| l.days)
            .unwrap_or(q.lead_days)
    };
    let mut registers = Vec::new();
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    for (file, reg, crl) in loaded {
        let revocation = match crl {
            None => "unknown",
            Some(c) if date > c.next_update.as_str() => "stale",
            Some(_) => "checked",
        };
        let by_id: BTreeMap<&str, &Revoked> = crl
            .iter()
            .flat_map(|c| c.revoked.iter().map(|v| (v.id.as_str(), v)))
            .collect();
        let ids: BTreeSet<&str> = reg.entries.iter().map(|e| e.id.as_str()).collect();
        registers.push(RegisterInfo {
            file: file.clone(),
            publisher: reg.publisher.clone(),
            signer: reg.signer.clone(),
            issued: reg.issued.clone(),
            entries: reg.entries.len(),
            crl_issuer: crl.as_ref().map(|c| c.issuer.clone()),
            crl_next_update: crl.as_ref().map(|c| c.next_update.clone()),
            revocation: revocation.into(),
            crl_unmatched: crl
                .iter()
                .flat_map(|c| c.revoked.iter())
                .filter(|v| !ids.contains(v.id.as_str()))
                .map(|v| v.id.clone())
                .collect(),
        });
        for e in &reg.entries {
            if !seen.insert((reg.signer.clone(), e.id.clone())) {
                return Err(format!(
                    "entry '{}' of {} appears in two registers",
                    e.id, reg.publisher
                ));
            }
            let rv = by_id
                .get(e.id.as_str())
                .filter(|v| v.revoked.as_str() <= date)
                .map(|v| (*v).clone());
            let status = if rv.is_some() {
                "revoked"
            } else if date > e.revalidate_by.as_str() {
                "due"
            } else {
                "current"
            };
            let days_left = (day(&e.revalidate_by) - day(date)).round() as i64;
            let flag = if rv.is_some() {
                "REVOKED"
            } else if days_left < 0 {
                "OVERDUE"
            } else if days_left <= i64::from(q.warn_days) {
                "DUE SOON"
            } else {
                "SCHEDULED"
            };
            let lead = lead_of(&e.maker);
            items.push(Item {
                publisher: reg.publisher.clone(),
                id: e.id.clone(),
                product: e.product.clone(),
                maker: e.maker.clone(),
                version: e.version.clone(),
                method: e.method.clone(),
                method_version: e.method_version.clone(),
                scored: e.scored.clone(),
                revalidate_by: e.revalidate_by.clone(),
                lead_days: lead,
                start_by: date_minus(&e.revalidate_by, lead),
                days_left,
                status: status.into(),
                revocation: revocation.into(),
                flag: flag.into(),
                revoked: rv,
            });
        }
    }
    items.sort_by(|a, b| {
        a.revalidate_by
            .cmp(&b.revalidate_by)
            .then(a.maker.cmp(&b.maker))
            .then(a.id.cmp(&b.id))
    });
    let mut by_month: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut makers: BTreeMap<String, MakerRow> = BTreeMap::new();
    for i in &items {
        *by_month
            .entry(i.revalidate_by[..7].to_string())
            .or_default()
            .entry(i.flag.clone())
            .or_insert(0) += 1;
        let m = makers.entry(i.maker.clone()).or_insert_with(|| MakerRow {
            maker: i.maker.clone(),
            ..Default::default()
        });
        m.entries += 1;
        match i.flag.as_str() {
            "OVERDUE" => m.overdue += 1,
            "DUE SOON" => m.due_soon += 1,
            "SCHEDULED" => m.scheduled += 1,
            _ => m.revoked += 1,
        }
        if i.flag != "REVOKED" && m.next_revalidate_by.is_none() {
            m.next_revalidate_by = Some(i.revalidate_by.clone());
            m.next_start_by = Some(i.start_by.clone());
        }
    }
    let n = |f: &str| items.iter().filter(|i| i.flag == f).count();
    let stale = registers.iter().filter(|r| r.revocation == "stale").count();
    let unknown = registers
        .iter()
        .filter(|r| r.revocation == "unknown")
        .count();
    let verdict = format!(
        "{} entries in {} registers on {date}: {} overdue, {} due within {} days, {} scheduled, {} revoked; revocation unchecked for {unknown} registers, list stale for {stale}",
        items.len(),
        registers.len(),
        n("OVERDUE"),
        n("DUE SOON"),
        q.warn_days,
        n("SCHEDULED"),
        n("REVOKED"),
    );
    Ok(Plan {
        format: FORMAT.into(),
        title: q.title.clone(),
        evidence: EVIDENCE.into(),
        date: date.into(),
        warn_days: q.warn_days,
        lead_days: q.lead_days,
        lead: q.lead.clone(),
        verdict,
        registers,
        items,
        by_month,
        makers: makers.into_values().collect(),
        caveats: vec![
            "Status follows the open register view: revoked when on the list from a date at or before the plan date (a stale list's revocations still apply), due when the revalidation date has passed, current otherwise.".into(),
            "Without a revocation list, or with a list past its next update, an entry not shown as revoked may still have been revoked.".into(),
            "An entry says only that a product was scored under a method; the plan says when it is to be scored again.".into(),
        ],
    })
}

fn resolve(base: &Path, p: &str) -> std::path::PathBuf {
    if Path::new(p).is_absolute() {
        p.into()
    } else {
        base.join(p)
    }
}

/// The input files (published name, bytes) of a plan, for the pack.
pub type Inputs = Vec<(String, Vec<u8>)>;

/// Read and check every register and list of the request, then plan at `date`.
pub fn run(q: &Request, base: &Path, date: &str) -> Result<(Plan, Inputs), String> {
    let mut loaded = Vec::new();
    let mut inputs = Vec::new();
    for (i, r) in q.registers.iter().enumerate() {
        let p = resolve(base, &r.file);
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let reg =
            check_register(&text, r.key.as_deref()).map_err(|e| format!("{}: {e}", p.display()))?;
        inputs.push((
            format!("registers/{:02}-register.json", i + 1),
            text.into_bytes(),
        ));
        let crl = match &r.crl {
            None => None,
            Some(c) => {
                let key = r
                    .crl_key
                    .as_deref()
                    .ok_or_else(|| format!("{}: crl needs crl_key", r.file))?;
                let cp = resolve(base, c);
                let t =
                    std::fs::read_to_string(&cp).map_err(|e| format!("{}: {e}", cp.display()))?;
                let crl = check_crl(&t, key).map_err(|e| format!("{}: {e}", cp.display()))?;
                inputs.push((format!("registers/{:02}-crl.json", i + 1), t.into_bytes()));
                Some(crl)
            }
        };
        loaded.push((r.file.clone(), reg, crl));
    }
    Ok((plan(q, &loaded, date)?, inputs))
}

/// The plan as CSV.
pub fn csv(p: &Plan) -> String {
    let q = |s: &str| {
        if s.contains(',') || s.contains('"') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut s = String::from("publisher,id,product,maker,version,method,method_version,scored,revalidate_by,start_by,days_left,status,revocation,flag\n");
    for i in &p.items {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            q(&i.publisher),
            q(&i.id),
            q(&i.product),
            q(&i.maker),
            q(&i.version),
            q(&i.method),
            q(&i.method_version),
            i.scored,
            i.revalidate_by,
            i.start_by,
            i.days_left,
            i.status,
            i.revocation,
            i.flag
        ));
    }
    s
}

/// The plan as iCalendar: per entry not revoked, an all-day event on the revalidation date and
/// one on the start date.
pub fn ics(p: &Plan) -> String {
    let esc = |s: &str| {
        s.replace('\\', "\\\\")
            .replace(';', "\\;")
            .replace(',', "\\,")
    };
    let stamp = format!("{}T000000Z", p.date.replace('-', ""));
    let mut s = String::from(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Kshana Pro//revalidation plan//EN\r\n",
    );
    for i in p.items.iter().filter(|i| i.flag != "REVOKED") {
        for (kind, d, what) in [
            ("start", &i.start_by, "Start re-scoring"),
            ("due", &i.revalidate_by, "Revalidation due"),
        ] {
            s.push_str(&format!(
                "BEGIN:VEVENT\r\nUID:{}-{}-{kind}@kshana-pro\r\nDTSTAMP:{stamp}\r\nDTSTART;VALUE=DATE:{}\r\nSUMMARY:{}\r\nDESCRIPTION:{}\r\nEND:VEVENT\r\n",
                esc(&i.publisher.replace(' ', "_")),
                esc(&i.id),
                d.replace('-', ""),
                esc(&format!("{what}: {} {} ({})", i.maker, i.product, i.flag)),
                esc(&format!(
                    "Entry {} of {}: scored {} under {} v{}; revalidate by {}",
                    i.id, i.publisher, i.scored, i.method, i.method_version, i.revalidate_by
                )),
            ));
        }
    }
    s.push_str("END:VCALENDAR\r\n");
    s
}

/// The page.
pub fn html(p: &Plan) -> String {
    let regs = p
        .registers
        .iter()
        .map(|r| {
            vec![
                r.publisher.clone(),
                r.issued.clone(),
                r.entries.to_string(),
                r.crl_issuer.clone().unwrap_or_default(),
                r.revocation.clone(),
                r.crl_unmatched.join(", "),
            ]
        })
        .collect();
    let months = p
        .by_month
        .iter()
        .map(|(m, f)| {
            let g = |k: &str| f.get(k).copied().unwrap_or(0).to_string();
            vec![
                m.clone(),
                g("OVERDUE"),
                g("DUE SOON"),
                g("SCHEDULED"),
                g("REVOKED"),
            ]
        })
        .collect();
    let makers = p
        .makers
        .iter()
        .map(|m| {
            vec![
                m.maker.clone(),
                m.entries.to_string(),
                m.overdue.to_string(),
                m.due_soon.to_string(),
                m.scheduled.to_string(),
                m.revoked.to_string(),
                m.next_start_by.clone().unwrap_or_default(),
                m.next_revalidate_by.clone().unwrap_or_default(),
            ]
        })
        .collect();
    let items = p
        .items
        .iter()
        .map(|i| {
            vec![
                i.revalidate_by.clone(),
                i.start_by.clone(),
                i.maker.clone(),
                format!("{} {}", i.product, i.version),
                format!("{} v{}", i.method, i.method_version),
                i.flag.clone(),
                format!("{} ({})", i.status, i.revocation),
            ]
        })
        .collect();
    crate::transport::page(
        &p.title,
        &p.evidence,
        &p.verdict,
        &[p.caveats.join(" ")],
        &[
            (
                "Registers".into(),
                vec![
                    "publisher",
                    "issued",
                    "entries",
                    "revocation list",
                    "revocation",
                    "list ids not in register",
                ],
                regs,
            ),
            (
                "By month of revalidation".into(),
                vec!["month", "overdue", "due soon", "scheduled", "revoked"],
                months,
            ),
            (
                "By maker".into(),
                vec![
                    "maker",
                    "entries",
                    "overdue",
                    "due soon",
                    "scheduled",
                    "revoked",
                    "next start by",
                    "next revalidate by",
                ],
                makers,
            ),
            (
                "Entries".into(),
                vec![
                    "revalidate by",
                    "start by",
                    "maker",
                    "product",
                    "method",
                    "flag",
                    "status (revocation)",
                ],
                items,
            ),
        ],
    )
}
