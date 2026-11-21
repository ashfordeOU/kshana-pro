// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Station fleet management and calibration register (research 2026-11-20 gaps, B2604)
//!
//! For an operator of monitoring or timing stations:
//!
//! * **configuration history** (`config.jsonl`): an append-only, hash-chained record per change
//!   (receiver, antenna, cable, firmware, position, clock) with the same chain rule as the
//!   calibration register: `hash = SHA-256(prev ‖ "\n" ‖ body)`; editing, inserting, deleting or
//!   reordering a line breaks the chain;
//! * **calibration register**: the existing hash-chained register
//!   ([`kshana_eloran::calreg`]) per station or asset serial, with its least-squares **ageing
//!   curve** (drift per year with its standard error), the predicted value now, and the date
//!   the prediction leaves the stated tolerance;
//! * **site log**: a text log in the section layout of an IGS site log (site identification,
//!   location, receiver and antenna history) generated from the configuration history; field
//!   names follow that layout and are to confirm against the current IGS template;
//! * **monthly clock-quality statement**: from a station's clock offset series against its
//!   reference, per calendar month: mean and RMS offset, peak-to-peak, MTIE and TDEV at stated
//!   windows, and overlapping ADEV, written into a directory and signed with Ed25519 so it
//!   verifies offline (`dossier verify`). A statement of measured figures, not a certificate;
//! * **host dashboard**: one static HTML page of every station: latest configuration, chain
//!   status, calibration due dates and ageing, latest statement.
//!
//! **MODELLED** where it predicts (ageing); the rest is arithmetic on the operator's records.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::forensics::inputs::parse_utc;
use kshana_eloran::calreg::{self, Register};

/// Pro feature.
pub const FEATURE: &str = "pro-station-fleet";
/// Formats.
pub const STATEMENT_FORMAT: &str = "kshana-pro-clock-quality-statement/1";
pub const AGEING_FORMAT: &str = "kshana-pro-calibration-ageing/1";

/// One configuration change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigBody {
    pub seq: u64,
    pub station: String,
    pub utc: String,
    /// `receiver`, `antenna`, `cable`, `firmware`, `position`, `clock`, `site`, `other`.
    pub item: String,
    pub value: String,
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub notes: String,
    pub by: String,
}

/// A stored configuration line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub body: ConfigBody,
    pub prev: String,
    pub hash: String,
}

const ITEMS: &[&str] = &[
    "receiver", "antenna", "cable", "firmware", "position", "clock", "site", "other",
];

fn lines(path: &Path) -> Result<Vec<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// The stored text of the `body` object of a line (hashed as stored).
fn body_text(line: &str) -> Option<&str> {
    let s = line.strip_prefix("{\"body\":")?;
    let end = s.rfind(",\"prev\":")?;
    Some(&s[..end])
}

/// Append a configuration change; returns the stored entry.
pub fn config_append(path: &Path, mut body: ConfigBody) -> Result<ConfigEntry, String> {
    if !ITEMS.contains(&body.item.as_str()) {
        return Err(format!(
            "item '{}' is not one of {}",
            body.item,
            ITEMS.join(", ")
        ));
    }
    parse_utc(&body.utc).ok_or_else(|| format!("bad utc '{}'", body.utc))?;
    if body.station.trim().is_empty() || body.by.trim().is_empty() {
        return Err("station and by must be stated".into());
    }
    let v = config_verify(path)?;
    if !v.problems.is_empty() {
        return Err(format!(
            "the history does not verify: {}",
            v.problems.join("; ")
        ));
    }
    body.seq = v.entries as u64;
    let bt = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    let hash = calreg::entry_hash(&v.head, &bt);
    let line = format!(
        "{{\"body\":{bt},\"prev\":\"{}\",\"hash\":\"{hash}\"}}\n",
        v.head
    );
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(ConfigEntry {
        body,
        prev: v.head,
        hash,
    })
}

/// Chain check of a configuration history.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChainCheck {
    pub entries: usize,
    pub head: String,
    pub problems: Vec<String>,
}

/// Verify the configuration history chain.
pub fn config_verify(path: &Path) -> Result<ChainCheck, String> {
    let mut prev = calreg::GENESIS.to_string();
    let mut problems = Vec::new();
    let ls = lines(path)?;
    for (i, l) in ls.iter().enumerate() {
        let e: ConfigEntry = match serde_json::from_str(l) {
            Ok(e) => e,
            Err(e) => {
                problems.push(format!("line {}: {e}", i + 1));
                continue;
            }
        };
        let Some(bt) = body_text(l) else {
            problems.push(format!("line {}: layout changed", i + 1));
            continue;
        };
        if e.prev != prev {
            problems.push(format!("line {}: does not chain to the line before", i + 1));
        }
        if calreg::entry_hash(&e.prev, bt) != e.hash {
            problems.push(format!("line {}: changed after it was written", i + 1));
        }
        if e.body.seq != i as u64 {
            problems.push(format!(
                "line {}: sequence {} out of order",
                i + 1,
                e.body.seq
            ));
        }
        prev = e.hash;
    }
    Ok(ChainCheck {
        entries: ls.len(),
        head: prev,
        problems,
    })
}

/// The configuration history of a station.
pub fn config_history(path: &Path, station: &str) -> Result<Vec<ConfigBody>, String> {
    Ok(lines(path)?
        .iter()
        .filter_map(|l| serde_json::from_str::<ConfigEntry>(l).ok())
        .map(|e| e.body)
        .filter(|b| b.station == station)
        .collect())
}

/// A site log in the IGS section layout (field names to confirm against the current template).
pub fn site_log(
    path: &Path,
    station: &str,
    prepared_by: &str,
    utc: &str,
) -> Result<String, String> {
    let h = config_history(path, station)?;
    if h.is_empty() {
        return Err(format!("no configuration history for station '{station}'"));
    }
    let latest = |item: &str| h.iter().rev().find(|b| b.item == item);
    let mut o = format!(
        "     {station} Site Information Form (site log)\n     Generated by Kshana Pro from the hash-chained configuration history; section layout of\n     the IGS site log, field names to confirm against the current IGS template.\n\n0.   Form\n\n     Prepared by (full name)  : {prepared_by}\n     Date Prepared            : {}\n     Report Type              : UPDATE\n\n1.   Site Identification of the GNSS Monument\n\n     Site Name                : {station}\n     Four Character ID        : {}\n\n2.   Site Location Information\n\n     Approximate Position     : {}\n\n",
        &utc[..10.min(utc.len())],
        station.chars().take(4).collect::<String>().to_uppercase(),
        latest("position").map(|b| b.value.as_str()).unwrap_or("(not recorded)")
    );
    let section = |o: &mut String, n: &str, title: &str, item: &str, label: &str| {
        o.push_str(&format!("{n}.   {title}\n\n"));
        let v: Vec<&ConfigBody> = h.iter().filter(|b| b.item == item).collect();
        for (i, b) in v.iter().enumerate() {
            let removed = v
                .get(i + 1)
                .map(|x| x.utc.clone())
                .unwrap_or_else(|| "(CCYY-MM-DDThh:mmZ)".into());
            o.push_str(&format!(
                "{n}.{}  {label:<23}: {}\n       Serial Number          : {}\n       Date Installed         : {}\n       Date Removed           : {removed}\n       Additional Information : {}\n\n",
                i + 1,
                b.value,
                if b.serial.is_empty() { "(not recorded)" } else { &b.serial },
                b.utc,
                b.notes
            ));
        }
        if v.is_empty() {
            o.push_str("     (none recorded)\n\n");
        }
    };
    section(
        &mut o,
        "3",
        "GNSS Receiver Information",
        "receiver",
        "Receiver Type",
    );
    section(
        &mut o,
        "4",
        "GNSS Antenna Information",
        "antenna",
        "Antenna Type",
    );
    section(&mut o, "5", "Antenna Cable", "cable", "Cable");
    section(&mut o, "6", "Frequency Standard", "clock", "Standard Type");
    section(
        &mut o,
        "7",
        "Receiver Firmware",
        "firmware",
        "Firmware Version",
    );
    Ok(o)
}

/// Ageing of one asset quantity.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Ageing {
    pub format: &'static str,
    pub subject_id: String,
    pub quantity: String,
    pub entries: usize,
    pub latest_value: f64,
    pub latest_date: String,
    pub drift_per_year: f64,
    pub drift_u_per_year: Option<f64>,
    pub predicted_now: f64,
    pub tolerance: Option<[f64; 2]>,
    /// First date the straight-line prediction leaves the tolerance (within 50 years).
    pub leaves_tolerance: Option<String>,
    pub due_date: Option<String>,
    pub chain_ok: bool,
}

/// Ageing curve of a register subject and quantity.
pub fn ageing(
    reg_path: &Path,
    subject: &str,
    quantity: &str,
    tolerance: Option<[f64; 2]>,
    today: &str,
) -> Result<Ageing, String> {
    let reg = Register::open(reg_path);
    let vr = reg.verify()?;
    let h = reg.history(subject, Some(quantity))?;
    let last = h
        .last()
        .ok_or_else(|| format!("no entry for {subject} {quantity}"))?;
    let fit = reg.drift(subject, quantity)?;
    let now = fit.predict(today)?;
    let leaves = tolerance.and_then(|[lo, hi]| {
        let d0 = calreg::day_number(today).ok()?;
        (0..50 * 366).step_by(7).find_map(|k| {
            let d = calreg::date_of(d0 + k);
            let v = fit.predict(&d).ok()?;
            (v < lo || v > hi).then_some(d)
        })
    });
    Ok(Ageing {
        format: AGEING_FORMAT,
        subject_id: subject.into(),
        quantity: quantity.into(),
        entries: h.len(),
        latest_value: last.body.value,
        latest_date: last.body.date.clone(),
        drift_per_year: fit.slope_per_year,
        drift_u_per_year: fit.slope_u_per_year,
        predicted_now: now,
        tolerance,
        leaves_tolerance: leaves,
        due_date: reg.due_date(subject, Some(quantity), None)?,
        chain_ok: vr.ok(),
    })
}

/// One month's clock-quality statement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Statement {
    pub format: &'static str,
    pub station: String,
    pub month: String,
    pub reference: String,
    pub samples: usize,
    pub tau0_s: f64,
    pub coverage: f64,
    pub mean_offset_s: f64,
    pub rms_offset_s: f64,
    pub peak_to_peak_s: f64,
    /// `(window s, MTIE s)`.
    pub mtie: Vec<(f64, f64)>,
    /// `(tau s, TDEV s)`.
    pub tdev: Vec<(f64, f64)>,
    /// `(tau s, overlapping ADEV)`.
    pub adev: Vec<(f64, f64)>,
    pub input_sha256: String,
    pub statement: &'static str,
}

/// The statement of `month` (`YYYY-MM`) from a CSV `timestamp,offset_s` at a uniform `tau0`.
pub fn statement(
    station: &str,
    reference: &str,
    month: &str,
    csv: &str,
    windows_s: &[f64],
) -> Result<Statement, String> {
    let t = crate::transport::Table::parse(csv)?;
    let (ct, co) = (t.need("timestamp")?, t.need("offset_s")?);
    let mut s: Vec<(f64, f64)> = Vec::new();
    for i in 0..t.rows.len() {
        let ts = t.text(i, Some(ct));
        if !ts.starts_with(month) {
            continue;
        }
        let tt = parse_utc(ts).ok_or_else(|| format!("row {}: bad timestamp", i + 2))?;
        if let Some(x) = t.num(i, Some(co))? {
            s.push((tt, x));
        }
    }
    if s.len() < 16 {
        return Err(format!("{month}: fewer than 16 samples"));
    }
    s.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut dts: Vec<f64> = s.windows(2).map(|w| w[1].0 - w[0].0).collect();
    dts.sort_by(f64::total_cmp);
    let tau0 = dts[dts.len() / 2];
    if tau0 <= 0.0 {
        return Err("duplicate time stamps".into());
    }
    let x: Vec<f64> = s.iter().map(|p| p.1).collect();
    let n = x.len() as f64;
    let mean = x.iter().sum::<f64>() / n;
    let rms = (x.iter().map(|v| v * v).sum::<f64>() / n).sqrt();
    let (mn, mx) = x
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(*v), b.max(*v))
        });
    let mut mtie = Vec::new();
    let mut tdev = Vec::new();
    let mut adev = Vec::new();
    for &w in windows_s {
        let m = (w / tau0).round() as usize;
        if m == 0 || m >= x.len() {
            continue;
        }
        let mut best: f64 = 0.0;
        for i in 0..x.len() - m {
            let win = &x[i..=i + m];
            let (a, b) = win
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                    (a.min(*v), b.max(*v))
                });
            best = best.max(b - a);
        }
        mtie.push((w, best));
        if 3 * m < x.len() {
            tdev.push((w, kshana::allan::time_deviation(&x, tau0, m)));
        }
        if 2 * m < x.len() {
            adev.push((w, kshana::allan::overlapping_adev(&x, tau0, m)));
        }
    }
    Ok(Statement {
        format: STATEMENT_FORMAT,
        station: station.into(),
        month: month.into(),
        reference: reference.into(),
        samples: x.len(),
        tau0_s: tau0,
        coverage: crate::forensics::inputs::round((x.len() as f64 * tau0 / month_seconds(month)?).min(1.0), 4),
        mean_offset_s: mean,
        rms_offset_s: rms,
        peak_to_peak_s: mx - mn,
        mtie,
        tdev,
        adev,
        input_sha256: hex::encode(Sha256::digest(csv.as_bytes())),
        statement: "Measured figures of the station clock against the stated reference for the month; a statement of measurements, not a certificate or an approval.",
    })
}

fn month_seconds(month: &str) -> Result<f64, String> {
    let a = parse_utc(&format!("{month}-01T00:00:00Z")).ok_or("bad month")?;
    let y: i32 = month[..4].parse().map_err(|_| "bad month")?;
    let m: u32 = month[5..7].parse().map_err(|_| "bad month")?;
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let b = parse_utc(&format!("{ny:04}-{nm:02}-01T00:00:00Z")).ok_or("bad month")?;
    Ok(b - a)
}

/// A dashboard row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    pub station: String,
    pub receiver: String,
    pub antenna: String,
    pub firmware: String,
    pub last_change_utc: String,
}

/// The static dashboard over a configuration history and optional ageing results.
pub fn dashboard(config: &Path, ageing: &[Ageing], utc: &str) -> Result<String, String> {
    let chk = config_verify(config)?;
    let mut stations: std::collections::BTreeMap<String, Vec<ConfigBody>> = Default::default();
    for l in lines(config)? {
        if let Ok(e) = serde_json::from_str::<ConfigEntry>(&l) {
            stations
                .entry(e.body.station.clone())
                .or_default()
                .push(e.body);
        }
    }
    let rows: Vec<Vec<String>> = stations
        .iter()
        .map(|(s, h)| {
            let latest = |item: &str| {
                h.iter()
                    .rev()
                    .find(|b| b.item == item)
                    .map(|b| b.value.clone())
                    .unwrap_or_else(|| "-".into())
            };
            vec![
                s.clone(),
                latest("receiver"),
                latest("antenna"),
                latest("firmware"),
                h.last().map(|b| b.utc.clone()).unwrap_or_default(),
            ]
        })
        .collect();
    let arows: Vec<Vec<String>> = ageing
        .iter()
        .map(|a| {
            vec![
                a.subject_id.clone(),
                a.quantity.clone(),
                format!("{:.4}", a.latest_value),
                format!("{:+.4}", a.drift_per_year),
                a.due_date.clone().unwrap_or_else(|| "-".into()),
                a.leaves_tolerance.clone().unwrap_or_else(|| "-".into()),
                if a.chain_ok { "ok" } else { "BROKEN" }.into(),
            ]
        })
        .collect();
    Ok(crate::transport::page(
        "Station fleet",
        &format!(
            "Generated {utc}; configuration history {} entries, chain {}",
            chk.entries,
            if chk.problems.is_empty() {
                "verifies"
            } else {
                "DOES NOT VERIFY"
            }
        ),
        &format!("{} station(s)", stations.len()),
        &[format!("Configuration chain head {}", chk.head)],
        &[
            (
                "Stations".into(),
                vec!["station", "receiver", "antenna", "firmware", "last change"],
                rows,
            ),
            (
                "Calibration ageing".into(),
                vec![
                    "asset",
                    "quantity",
                    "latest",
                    "drift / year",
                    "due",
                    "leaves tolerance",
                    "register chain",
                ],
                arows,
            ),
        ],
    ))
}

/// Write a statement into `out` (report.json + report.html + input) for signing.
pub fn write_statement(out: &Path, s: &Statement, csv: &str) -> Result<(), String> {
    let html = crate::transport::page(
        &format!("Clock-quality statement {} {}", s.station, s.month),
        s.statement,
        &format!(
            "mean {:.3e} s, RMS {:.3e} s, peak-to-peak {:.3e} s over {} samples",
            s.mean_offset_s, s.rms_offset_s, s.peak_to_peak_s, s.samples
        ),
        &[format!("Reference: {}", s.reference)],
        &[(
            "MTIE".into(),
            vec!["window (s)", "MTIE (s)"],
            s.mtie
                .iter()
                .map(|(w, m)| vec![format!("{w}"), format!("{m:.3e}")])
                .collect(),
        )],
    );
    crate::transport::write_pack(out, s, &html, "offsets.csv", csv)
}

/// An ageing result read back from its JSON (the dashboard's input).
#[derive(Clone, Debug, Deserialize)]
pub struct AgeingIn {
    pub subject_id: String,
    pub quantity: String,
    pub entries: usize,
    pub latest_value: f64,
    pub latest_date: String,
    pub drift_per_year: f64,
    pub drift_u_per_year: Option<f64>,
    pub predicted_now: f64,
    pub tolerance: Option<[f64; 2]>,
    pub leaves_tolerance: Option<String>,
    pub due_date: Option<String>,
    pub chain_ok: bool,
}

impl From<AgeingIn> for Ageing {
    fn from(a: AgeingIn) -> Ageing {
        Ageing {
            format: AGEING_FORMAT,
            subject_id: a.subject_id,
            quantity: a.quantity,
            entries: a.entries,
            latest_value: a.latest_value,
            latest_date: a.latest_date,
            drift_per_year: a.drift_per_year,
            drift_u_per_year: a.drift_u_per_year,
            predicted_now: a.predicted_now,
            tolerance: a.tolerance,
            leaves_tolerance: a.leaves_tolerance,
            due_date: a.due_date,
            chain_ok: a.chain_ok,
        }
    }
}
