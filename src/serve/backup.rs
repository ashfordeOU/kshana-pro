// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Backup of a data directory (roadmap F2a)
//!
//! `kshana-pro backup create --data-dir <dir> --out <backup-dir>` copies a data
//! directory while the service runs, and `kshana-pro backup verify <backup-dir>` checks
//! a copy offline. Restoring is copying the backup back as the data directory (and
//! removing `backup-manifest.json`).
//!
//! Consistency while the service runs: the delivery ledger is copied **first**, then
//! the jobs, then the audit logs. A job's outputs are written before its ledger record,
//! so every delivery in the copied ledger finds its outputs in the copied jobs. A job
//! that finished after the ledger was copied is in the copy without its ledger record,
//! which the ledger check does not require; a job copied while running restores as
//! running and is run again by the restored service. The lease (`ha/`) is not copied.
//!
//! `backup-manifest.json` records every copied file with its SHA-256; verification
//! recomputes each and runs the ledger check (chain and every delivered job's files) on
//! the copy.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::delivery::DeliveryLedger;
use super::jobs::JobStore;

/// The manifest's name inside a backup.
pub const MANIFEST: &str = "backup-manifest.json";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BackupFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub format: String,
    pub created_utc: String,
    pub source: String,
    pub files: Vec<BackupFile>,
}

/// What a verification found.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Report {
    pub files: usize,
    pub problems: Vec<String>,
    pub ledger_verified: bool,
    pub delivered_jobs: usize,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.problems.is_empty() && self.ledger_verified
    }
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn walk(d: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !d.exists() {
        return Ok(());
    }
    let mut ents: Vec<_> = std::fs::read_dir(d)?.collect::<Result<_, _>>()?;
    ents.sort_by_key(|e| e.file_name());
    for e in ents {
        let ft = e.file_type()?;
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name.contains(".tmp-") {
            continue; // an atomic write in progress
        }
        if ft.is_dir() {
            walk(&p, out)?;
        } else if ft.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

fn copy_one(
    src_root: &Path,
    dst_root: &Path,
    p: &Path,
    files: &mut Vec<BackupFile>,
) -> std::io::Result<()> {
    let r = rel(src_root, p);
    let bytes = match std::fs::read(p) {
        Ok(b) => b,
        // Removed between listing and reading (a temporary file renamed away).
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let dst = dst_root.join(&r);
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&dst, &bytes)?;
    files.push(BackupFile {
        path: r,
        sha256: hex::encode(Sha256::digest(&bytes)),
        bytes: bytes.len() as u64,
    });
    Ok(())
}

/// Copy `data_dir` to `out` (which must not exist yet) and write the manifest.
pub fn create(data_dir: &Path, out: &Path, created_utc: &str) -> std::io::Result<Manifest> {
    if out.exists() {
        return Err(std::io::Error::other(format!(
            "{} exists; a backup goes to a new directory",
            out.display()
        )));
    }
    if !data_dir.join("jobs").is_dir() {
        return Err(std::io::Error::other(format!(
            "{} is not a service data directory (no jobs/)",
            data_dir.display()
        )));
    }
    std::fs::create_dir_all(out)?;
    let mut files = Vec::new();
    // 1. The ledger first (see the module notes on consistency).
    let ledger = data_dir.join("ledger.json");
    if ledger.exists() {
        copy_one(data_dir, out, &ledger, &mut files)?;
    }
    // 2. Jobs, 3. audit logs, then anything else except the lease.
    let mut rest = Vec::new();
    walk(&data_dir.join("jobs"), &mut rest)?;
    walk(&data_dir.join("audit"), &mut rest)?;
    let mut others = Vec::new();
    walk(data_dir, &mut others)?;
    for p in others {
        let r = rel(data_dir, &p);
        if r == "ledger.json"
            || r.starts_with("jobs/")
            || r.starts_with("audit/")
            || r.starts_with("ha/")
        {
            continue;
        }
        rest.push(p);
    }
    for p in rest {
        copy_one(data_dir, out, &p, &mut files)?;
    }
    let m = Manifest {
        format: "kshana-pro-backup/1".into(),
        created_utc: created_utc.into(),
        source: data_dir.display().to_string(),
        files,
    };
    std::fs::write(
        out.join(MANIFEST),
        serde_json::to_string_pretty(&m).map_err(std::io::Error::other)? + "\n",
    )?;
    Ok(m)
}

/// Verify a backup: every file's hash, nothing missing, and the ledger check on the copy.
pub fn verify(dir: &Path) -> std::io::Result<Report> {
    let m: Manifest = serde_json::from_str(&std::fs::read_to_string(dir.join(MANIFEST))?)
        .map_err(std::io::Error::other)?;
    let mut r = Report {
        files: m.files.len(),
        ..Default::default()
    };
    for f in &m.files {
        match std::fs::read(dir.join(&f.path)) {
            Ok(b) if hex::encode(Sha256::digest(&b)) == f.sha256 => {}
            Ok(_) => r.problems.push(format!("{}: changed", f.path)),
            Err(_) => r.problems.push(format!("{}: missing", f.path)),
        }
    }
    let store = JobStore::open(dir)?;
    let report = DeliveryLedger::new(dir).verify(&store);
    r.ledger_verified = report.verified;
    r.delivered_jobs = report.delivered_jobs;
    if !report.verified {
        r.problems.push(format!(
            "ledger: {}",
            report.reason.unwrap_or_else(|| "not verified".into())
        ));
    }
    Ok(r)
}
