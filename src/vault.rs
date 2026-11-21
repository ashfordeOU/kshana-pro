// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Customer evidence vault, first slice (research 2026-11-20 gaps, B2504)
//!
//! A local, file-based vault for the signed evidence directories a customer accumulates over
//! years:
//!
//! * **ingest** copies a signed evidence directory in after checking its signature (the
//!   signature must verify; trusting the key is the customer's decision and is recorded), and
//!   records its id (the SHA-256 of its `signature.json`), format, family, tags and a retention
//!   date;
//! * **legal hold** marks a pack so it cannot be deleted until released; **delete** is refused
//!   under hold or before the retention date;
//! * every action is an entry of a **hash-chained ledger** (`ledger.json`), so a removed or
//!   changed entry shows;
//! * **trend**: packs per month per output family (a multi-year dashboard as JSON and HTML);
//! * **calendar**: the user's own obligations (name, recurrence, the output formats that serve
//!   it) with the next due date and the latest pack that serves each;
//! * **submission**: a submission skeleton that cites pack ids, formats and signature key ids,
//!   with headings the user fills; no regulator's form is reproduced;
//! * **export / import**: a full open export (plain directories and JSON, with an
//!   `export-manifest.json` of SHA-256 digests) that imports back to an identical vault.
//!
//! Not in this slice: a server, access control and multi-user audit (the vault is a directory
//! the customer controls), regulator-specific templates.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::forensics::inputs::parse_utc;

/// Enterprise feature of the vault.
pub const FEATURE: &str = "pro-evidence-vault";
/// Format of the vault index.
pub const FORMAT: &str = "kshana-pro-evidence-vault/1";
/// Format of an export manifest.
pub const EXPORT_FORMAT: &str = "kshana-pro-vault-export/1";
const INDEX: &str = "index.json";
const LEDGER: &str = "ledger.json";
const PACKS: &str = "packs";
const MANIFEST: &str = "export-manifest.json";

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// One pack in the vault.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub format: Option<String>,
    pub family: Option<String>,
    pub signer: String,
    pub key_id: String,
    pub signed_utc: String,
    pub ingested_utc: String,
    /// Whether the signing key was in the trust list given at ingest.
    pub trusted_at_ingest: Option<bool>,
    pub tags: Vec<String>,
    pub retain_until_utc: String,
    pub legal_hold: Option<String>,
    pub deleted_utc: Option<String>,
}

/// The vault index.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Index {
    pub format: String,
    pub entries: Vec<Entry>,
}

/// One ledger record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub seq: u64,
    pub utc: String,
    pub action: String,
    pub pack: Option<String>,
    pub detail: String,
    pub prev_hash: String,
    pub hash: String,
}

fn rd<T: for<'de> Deserialize<'de>>(p: &Path) -> Result<T, String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))
}

fn wr<T: Serialize>(p: &Path, v: &T) -> Result<(), String> {
    let t = serde_json::to_string_pretty(v).map_err(|e| e.to_string())? + "\n";
    std::fs::write(p, t).map_err(|e| format!("{}: {e}", p.display()))
}

/// Create an empty vault.
pub fn init(vault: &Path, utc: &str) -> Result<(), String> {
    if vault.join(INDEX).exists() {
        return Err(format!("{}: already a vault", vault.display()));
    }
    std::fs::create_dir_all(vault.join(PACKS)).map_err(|e| e.to_string())?;
    wr(
        &vault.join(INDEX),
        &Index {
            format: FORMAT.into(),
            entries: Vec::new(),
        },
    )?;
    wr(&vault.join(LEDGER), &Vec::<Record>::new())?;
    log(vault, utc, "init", None, "vault created")
}

fn record_hash(r: &Record) -> String {
    sha(
        &serde_json::to_vec(&(r.seq, &r.utc, &r.action, &r.pack, &r.detail, &r.prev_hash))
            .unwrap_or_default(),
    )
}

fn log(
    vault: &Path,
    utc: &str,
    action: &str,
    pack: Option<&str>,
    detail: &str,
) -> Result<(), String> {
    let mut l: Vec<Record> = rd(&vault.join(LEDGER))?;
    let mut r = Record {
        seq: l.len() as u64 + 1,
        utc: utc.into(),
        action: action.into(),
        pack: pack.map(str::to_string),
        detail: detail.into(),
        prev_hash: l
            .last()
            .map(|x| x.hash.clone())
            .unwrap_or_else(|| "0".repeat(64)),
        hash: String::new(),
    };
    r.hash = record_hash(&r);
    l.push(r);
    wr(&vault.join(LEDGER), &l)
}

/// Problems with the ledger chain (empty when intact).
pub fn check_ledger(vault: &Path) -> Result<Vec<String>, String> {
    let l: Vec<Record> = rd(&vault.join(LEDGER))?;
    let mut prev = "0".repeat(64);
    let mut out = Vec::new();
    for (i, r) in l.iter().enumerate() {
        if r.seq != i as u64 + 1 || r.prev_hash != prev || record_hash(r) != r.hash {
            out.push(format!(
                "ledger record {} is changed, missing or out of order",
                i + 1
            ));
        }
        prev = r.hash.clone();
    }
    Ok(out)
}

fn index(vault: &Path) -> Result<Index, String> {
    let i: Index = rd(&vault.join(INDEX))?;
    if i.format != FORMAT {
        return Err(format!("{}: not a vault index", vault.display()));
    }
    Ok(i)
}

fn copy_dir(a: &Path, b: &Path) -> Result<(), String> {
    std::fs::create_dir_all(b).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(a).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let ft = e.file_type().map_err(|e| e.to_string())?;
        if ft.is_symlink() {
            return Err(format!(
                "{}: symbolic links are not taken",
                e.path().display()
            ));
        }
        let to = b.join(e.file_name());
        if ft.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// A UTC time as the vault stores it (`YYYY-MM-DDTHH:MM:SSZ`), from any form `parse_utc`
/// accepts, a bare date (`YYYY-MM-DD`, midnight) included. The calendar arithmetic below works on
/// that text, so every time is brought to it first.
fn canon(utc: &str, what: &str) -> Result<String, String> {
    let s = utc.trim();
    let full = if s.len() == 10 && s.as_bytes().get(4) == Some(&b'-') {
        format!("{s}T00:00:00Z")
    } else {
        s.to_string()
    };
    let t = parse_utc(&full).ok_or_else(|| format!("{what}: '{utc}' is not a UTC time"))?;
    Ok(crate::forensics::inputs::iso(t.floor()))
}

fn add_years(utc: &str, years: u32) -> String {
    // Same calendar date `years` later (29 February becomes 28 February).
    let (y, rest) = utc.split_at(4);
    let y: i32 = y.parse().unwrap_or(1970) + years as i32;
    let r = if rest.starts_with("-02-29") && !(y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)) {
        rest.replacen("-02-29", "-02-28", 1)
    } else {
        rest.to_string()
    };
    format!("{y:04}{r}")
}

/// The Kshana format of an evidence directory: the `format` of the first top-level JSON file
/// (by name) that carries one starting with `kshana`.
pub fn detect_format(dir: &Path) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.ends_with(".json")
                && n != crate::signed_dossier::SIGNATURE_FILE
                && n != crate::envelope::FILE
        })
        .collect();
    names.sort();
    names.into_iter().find_map(|n| {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(&n)).ok()?).ok()?;
        v.get("format")?
            .as_str()
            .filter(|f| f.starts_with("kshana"))
            .map(str::to_string)
    })
}

/// The trend family of a format: its first name token after the `kshana-pro-` or `kshana-`
/// prefix (`kshana-pro-grid-pmu/1` is `grid`).
pub fn family_of(format: &str) -> String {
    let n = format.split('/').next().unwrap_or(format);
    let n = n
        .strip_prefix("kshana-pro-")
        .or_else(|| n.strip_prefix("kshana-"))
        .unwrap_or(n);
    n.split('-').next().unwrap_or(n).to_string()
}

/// Ingest a signed evidence directory.
pub fn ingest(
    vault: &Path,
    pack: &Path,
    tags: &[String],
    retain_years: u32,
    trust: &[[u8; crate::license::KEY_LEN]],
    utc: &str,
) -> Result<Entry, String> {
    let utc = &canon(utc, "--utc")?;
    let mut idx = index(vault)?;
    let v = crate::signed_dossier::verify(pack, trust).map_err(|e| e.to_string())?;
    let sig_ok = v.signature_ok && v.problems.iter().all(|p| p.contains("not a trusted key"));
    if !sig_ok {
        return Err(format!(
            "{}: does not verify: {}",
            pack.display(),
            v.problems.join("; ")
        ));
    }
    let id = sha(
        &std::fs::read(pack.join(crate::signed_dossier::SIGNATURE_FILE))
            .map_err(|e| e.to_string())?,
    );
    if idx
        .entries
        .iter()
        .any(|e| e.id == id && e.deleted_utc.is_none())
    {
        return Err(format!("pack {id} is already in the vault"));
    }
    let format = detect_format(pack);
    let e = Entry {
        id: id.clone(),
        family: format.as_deref().map(family_of),
        format,
        signer: v.signer.clone(),
        key_id: v.key_id.clone(),
        signed_utc: v.utc.clone(),
        ingested_utc: utc.into(),
        trusted_at_ingest: v.trusted,
        tags: tags.to_vec(),
        retain_until_utc: add_years(utc, retain_years),
        legal_hold: None,
        deleted_utc: None,
    };
    copy_dir(pack, &vault.join(PACKS).join(&id))?;
    idx.entries.push(e.clone());
    wr(&vault.join(INDEX), &idx)?;
    log(
        vault,
        utc,
        "ingest",
        Some(&id),
        &format!("from {}", pack.display()),
    )?;
    Ok(e)
}

fn find<'a>(idx: &'a mut Index, id: &str) -> Result<&'a mut Entry, String> {
    let m: Vec<usize> = idx
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.id.starts_with(id) && e.deleted_utc.is_none())
        .map(|(i, _)| i)
        .collect();
    match m.as_slice() {
        [i] => Ok(&mut idx.entries[*i]),
        [] => Err(format!("no pack {id} in the vault")),
        _ => Err(format!("{id} names more than one pack")),
    }
}

/// Put a pack under legal hold.
pub fn hold(vault: &Path, id: &str, reason: &str, utc: &str) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err("a legal hold needs a reason".into());
    }
    let mut idx = index(vault)?;
    let e = find(&mut idx, id)?;
    e.legal_hold = Some(reason.into());
    let full = e.id.clone();
    wr(&vault.join(INDEX), &idx)?;
    log(vault, utc, "hold", Some(&full), reason)
}

/// Release a legal hold.
pub fn release(vault: &Path, id: &str, reason: &str, utc: &str) -> Result<(), String> {
    let mut idx = index(vault)?;
    let e = find(&mut idx, id)?;
    if e.legal_hold.is_none() {
        return Err(format!("pack {} is not under hold", e.id));
    }
    e.legal_hold = None;
    let full = e.id.clone();
    wr(&vault.join(INDEX), &idx)?;
    log(vault, utc, "release", Some(&full), reason)
}

/// Delete a pack: refused under legal hold or before its retention date.
pub fn delete(vault: &Path, id: &str, utc: &str) -> Result<(), String> {
    let mut idx = index(vault)?;
    let e = find(&mut idx, id)?;
    if let Some(r) = &e.legal_hold {
        return Err(format!(
            "pack {} is under legal hold ({r}); release it first",
            e.id
        ));
    }
    let now = parse_utc(utc).ok_or("bad --utc")?;
    let until = parse_utc(&e.retain_until_utc).ok_or_else(|| {
        format!(
            "pack {}: retention date '{}' is not a UTC time",
            e.id, e.retain_until_utc
        )
    })?;
    if now < until {
        return Err(format!(
            "pack {} is retained until {}",
            e.id, e.retain_until_utc
        ));
    }
    e.deleted_utc = Some(utc.into());
    let full = e.id.clone();
    std::fs::remove_dir_all(vault.join(PACKS).join(&full)).map_err(|e| e.to_string())?;
    wr(&vault.join(INDEX), &idx)?;
    log(vault, utc, "delete", Some(&full), "retention ended")
}

/// The live (not deleted) entries.
pub fn list(vault: &Path) -> Result<Vec<Entry>, String> {
    Ok(index(vault)?
        .entries
        .into_iter()
        .filter(|e| e.deleted_utc.is_none())
        .collect())
}

/// Packs per month (`YYYY-MM` of signing) per family.
pub fn trend(vault: &Path) -> Result<BTreeMap<String, BTreeMap<String, usize>>, String> {
    let mut m: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for e in list(vault)? {
        let month = e.signed_utc.get(..7).unwrap_or("unknown").to_string();
        *m.entry(month)
            .or_default()
            .entry(e.family.clone().unwrap_or_else(|| "other".into()))
            .or_insert(0) += 1;
    }
    Ok(m)
}

/// One obligation the user declares.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obligation {
    pub name: String,
    /// `monthly`, `quarterly` or `annual`.
    pub every: String,
    /// First due date (UTC date or time).
    pub first_due: String,
    /// Output formats (or format prefixes) that serve it.
    pub served_by: Vec<String>,
}

/// One calendar line.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Due {
    pub name: String,
    pub next_due: String,
    pub latest_pack: Option<String>,
    pub latest_signed_utc: Option<String>,
    /// Whether a serving pack was signed in the current period.
    pub served_this_period: bool,
}

fn add_months(date: &str, n: i32) -> String {
    let y: i32 = date[..4].parse().unwrap_or(1970);
    let m: i32 = date[5..7].parse().unwrap_or(1);
    let d: u32 = date[8..10].parse().unwrap_or(1);
    let t = y * 12 + (m - 1) + n;
    let (ny, nm) = (t.div_euclid(12), t.rem_euclid(12) + 1);
    let dim = match nm {
        2 if ny % 4 == 0 && (ny % 100 != 0 || ny % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    format!("{ny:04}-{nm:02}-{:02}{}", d.min(dim), &date[10..])
}

/// The calendar of `obligations` at `utc`.
pub fn calendar(vault: &Path, obligations: &[Obligation], utc: &str) -> Result<Vec<Due>, String> {
    let entries = list(vault)?;
    let now = parse_utc(utc).ok_or("bad --utc")?;
    let mut out = Vec::new();
    for o in obligations {
        let step = match o.every.as_str() {
            "monthly" => 1,
            "quarterly" => 3,
            "annual" => 12,
            x => {
                return Err(format!(
                    "obligation '{}': every '{x}' (monthly, quarterly or annual)",
                    o.name
                ))
            }
        };
        let first = canon(&o.first_due, &format!("obligation '{}': first_due", o.name))?;
        // Each due date from the first one, so a month-end date stays at month end.
        let mut k = 0;
        while parse_utc(&add_months(&first, k * step)).is_some_and(|t| t < now) {
            k += 1;
        }
        let (prev, next) = (
            add_months(&first, (k - 1) * step),
            add_months(&first, k * step),
        );
        let serving: Vec<&Entry> = entries
            .iter()
            .filter(|e| {
                e.format
                    .as_deref()
                    .is_some_and(|f| o.served_by.iter().any(|s| f.starts_with(s.as_str())))
            })
            .collect();
        let latest = serving
            .iter()
            .max_by(|a, b| a.signed_utc.cmp(&b.signed_utc));
        let p0 = parse_utc(&prev).unwrap_or(f64::NEG_INFINITY);
        out.push(Due {
            name: o.name.clone(),
            next_due: next,
            latest_pack: latest.map(|e| e.id.clone()),
            latest_signed_utc: latest.map(|e| e.signed_utc.clone()),
            served_this_period: latest
                .and_then(|e| parse_utc(&e.signed_utc))
                .is_some_and(|t| t >= p0),
        });
    }
    Ok(out)
}

/// A submission skeleton citing packs (Markdown).
pub fn submission(vault: &Path, title: &str, ids: &[String]) -> Result<String, String> {
    let mut idx = index(vault)?;
    let mut rows = Vec::new();
    for id in ids {
        let e = find(&mut idx, id)?.clone();
        rows.push(format!(
            "| `{}` | {} | {} | {} | {} |",
            &e.id[..16],
            e.format.clone().unwrap_or_else(|| "-".into()),
            e.signed_utc,
            e.signer,
            e.key_id
        ));
    }
    Ok(format!(
        "# {title}\n\n_Submission skeleton. Fill each section; the evidence is cited by pack id (SHA-256 of its signature file) and can be verified offline with `kshana-pro dossier verify`._\n\n## 1. Submitting organisation\n\n[to fill]\n\n## 2. Purpose of this submission\n\n[to fill]\n\n## 3. Evidence cited\n\n| pack id | format | signed (UTC) | signer | key id |\n|---|---|---|---|---|\n{}\n\n## 4. What the evidence shows and what it does not\n\n[to fill: each pack's operating envelope (operating-envelope.json) states its valid inputs, ranges and exclusions]\n\n## 5. Contact\n\n[to fill]\n",
        rows.join("\n")
    ))
}

fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        for e in std::fs::read_dir(d).map_err(|e| e.to_string())? {
            let p = e.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                walk(&p, out)?;
            } else {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut v = Vec::new();
    walk(root, &mut v)?;
    v.sort();
    Ok(v)
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Export manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub exported_utc: String,
    pub files: BTreeMap<String, String>,
}

/// Full open export of the vault into `out`.
pub fn export(vault: &Path, out: &Path, utc: &str) -> Result<Manifest, String> {
    if out.exists()
        && std::fs::read_dir(out)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err(format!("{}: not empty", out.display()));
    }
    let problems = check_ledger(vault)?;
    if !problems.is_empty() {
        return Err(format!("ledger does not verify: {}", problems.join("; ")));
    }
    copy_dir(vault, out)?;
    let mut m = Manifest {
        format: EXPORT_FORMAT.into(),
        exported_utc: utc.into(),
        files: BTreeMap::new(),
    };
    for f in files(out)? {
        let b = std::fs::read(&f).map_err(|e| e.to_string())?;
        m.files.insert(rel(out, &f), sha(&b));
    }
    wr(&out.join(MANIFEST), &m)?;
    Ok(m)
}

/// Import an export into a new vault directory, checking every digest.
pub fn import(export: &Path, vault: &Path) -> Result<usize, String> {
    let m: Manifest = rd(&export.join(MANIFEST))?;
    if m.format != EXPORT_FORMAT {
        return Err("not a vault export".into());
    }
    let present: Vec<String> = files(export)?
        .iter()
        .map(|f| rel(export, f))
        .filter(|r| r != MANIFEST)
        .collect();
    for r in &present {
        if !m.files.contains_key(r) {
            return Err(format!("{r}: not in the export manifest"));
        }
    }
    for (r, d) in &m.files {
        let b = std::fs::read(export.join(r)).map_err(|e| format!("{r}: {e}"))?;
        if &sha(&b) != d {
            return Err(format!("{r}: changed since export"));
        }
    }
    if vault.exists()
        && std::fs::read_dir(vault)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err(format!("{}: not empty", vault.display()));
    }
    copy_dir(export, vault)?;
    std::fs::remove_file(vault.join(MANIFEST)).map_err(|e| e.to_string())?;
    let problems = check_ledger(vault)?;
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    Ok(m.files.len())
}

/// HTML page of a trend.
pub fn trend_html(t: &BTreeMap<String, BTreeMap<String, usize>>) -> String {
    let fams: std::collections::BTreeSet<&String> = t.values().flat_map(|m| m.keys()).collect();
    let mut cols = vec!["month"];
    cols.extend(fams.iter().map(|s| s.as_str()));
    let rows = t
        .iter()
        .map(|(m, c)| {
            let mut r = vec![m.clone()];
            r.extend(
                fams.iter()
                    .map(|f| c.get(*f).copied().unwrap_or(0).to_string()),
            );
            r
        })
        .collect();
    crate::transport::page(
        "Evidence vault trend",
        "Signed evidence packs held, by month of signing and output family",
        &format!("{} month(s)", t.len()),
        &[],
        &[("Packs per month".into(), cols, rows)],
    )
}
