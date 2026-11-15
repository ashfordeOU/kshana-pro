// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Per-clock evidence history (roadmap 0.42 T2): every assessment of a clock is appended to
//! `<dir>/<clock id>.jsonl` as one entry that carries the SHA-256 of the entry before it, so a
//! removed, changed or reordered entry breaks the chain ([`verify_chain`]). The files are plain
//! JSON lines: an audit report copies them and signs them with the rest of its directory.

use std::io::Write;
use std::path::{Path, PathBuf};

use kshana::api::KshanaError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::fleet::{ClockAssessment, FleetAssessment};

/// The format tag of an entry.
pub const ENTRY_FORMAT: &str = "kshana-pro-timing-evidence/1";

/// The chain value before the first entry.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// One recorded breach figure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntryBreach {
    pub limit: String,
    pub value_s: f64,
    pub t_warn_s: Option<f64>,
    pub t_50_s: Option<f64>,
}

/// One evidence entry (everything but `sha256` is hashed).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub format: String,
    pub seq: u64,
    pub utc: String,
    pub fleet: String,
    pub clock: String,
    pub source_format: String,
    pub record: String,
    pub noise_source: String,
    pub record_sha256: String,
    pub n_used: usize,
    pub tau0_s: f64,
    pub phase_s: f64,
    pub freq: f64,
    pub drift_per_s: f64,
    pub q_wf: f64,
    pub q_rw: f64,
    pub r_s2: f64,
    pub max_abs_phase_s: f64,
    pub status: String,
    pub breaches: Vec<EntryBreach>,
    pub prev_sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sha256: String,
}

impl Entry {
    fn digest(&self) -> String {
        let mut e = self.clone();
        e.sha256 = String::new();
        let bytes = serde_json::to_vec(&e).expect("an entry is plain data");
        hex::encode(Sha256::digest(&bytes))
    }

    fn from_assessment(fleet: &str, c: &ClockAssessment, utc: &str) -> Entry {
        Entry {
            format: ENTRY_FORMAT.into(),
            seq: 0,
            utc: utc.into(),
            fleet: fleet.into(),
            clock: c.id.clone(),
            source_format: c.source_format.clone(),
            record: c.record.clone(),
            noise_source: serde_json::to_value(c.noise_source)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default(),
            record_sha256: c.record_sha256.clone(),
            n_used: c.n_used,
            tau0_s: c.tau0_s,
            phase_s: c.holdover.phase_s,
            freq: c.holdover.freq,
            drift_per_s: c.holdover.drift_per_s,
            q_wf: c.holdover.noise.q_wf,
            q_rw: c.holdover.noise.q_rw,
            r_s2: c.holdover.noise.r_s2,
            max_abs_phase_s: c.max_abs_phase_s,
            status: c.status.clone(),
            breaches: c
                .breaches
                .iter()
                .map(|b| EntryBreach {
                    limit: b.name.clone(),
                    value_s: b.value_s,
                    t_warn_s: b.t_warn_s,
                    t_50_s: b.t_50_s,
                })
                .collect(),
            prev_sha256: GENESIS.into(),
            sha256: String::new(),
        }
    }
}

fn file(dir: &Path, clock: &str) -> PathBuf {
    dir.join(format!("{clock}.jsonl"))
}

/// Read one clock's history (empty when there is none).
pub fn read(dir: &Path, clock: &str) -> Result<Vec<Entry>, KshanaError> {
    let p = file(dir, clock);
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(KshanaError::InvalidInput(format!("{}: {e}", p.display()))),
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| {
            serde_json::from_str::<Entry>(l).map_err(|e| {
                KshanaError::InvalidInput(format!("{} line {}: {e}", p.display(), i + 1))
            })
        })
        .collect()
}

/// Check a history: sequence numbers count from 0, each entry's `prev_sha256` is the entry
/// before it, and each `sha256` is the entry's own digest. Returns the problems found.
pub fn verify_chain(entries: &[Entry]) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev = GENESIS.to_string();
    for (i, e) in entries.iter().enumerate() {
        if e.seq != i as u64 {
            out.push(format!(
                "entry {i}: sequence number {} (expected {i})",
                e.seq
            ));
        }
        if e.prev_sha256 != prev {
            out.push(format!("entry {i}: does not follow the entry before it"));
        }
        if e.digest() != e.sha256 {
            out.push(format!("entry {i}: its content does not match its hash"));
        }
        prev = e.sha256.clone();
    }
    out
}

/// Append one entry per clock of `a`, stamped `utc`. Refuses to append to a broken history.
pub fn append(dir: &Path, a: &FleetAssessment, utc: &str) -> Result<Vec<Entry>, KshanaError> {
    std::fs::create_dir_all(dir)
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", dir.display())))?;
    let mut written = Vec::new();
    for c in &a.clocks {
        let hist = read(dir, &c.id)?;
        let problems = verify_chain(&hist);
        if !problems.is_empty() {
            return Err(KshanaError::InvalidInput(format!(
                "evidence history of '{}' is broken ({}); not appending",
                c.id,
                problems.join("; ")
            )));
        }
        let mut e = Entry::from_assessment(&a.fleet, c, utc);
        e.seq = hist.len() as u64;
        e.prev_sha256 = hist
            .last()
            .map(|h| h.sha256.clone())
            .unwrap_or_else(|| GENESIS.into());
        e.sha256 = e.digest();
        let line = serde_json::to_string(&e).expect("an entry is plain data");
        let p = file(dir, &c.id);
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .map_err(|er| KshanaError::InvalidInput(format!("{}: {er}", p.display())))?;
        writeln!(f, "{line}")
            .map_err(|er| KshanaError::InvalidInput(format!("{}: {er}", p.display())))?;
        written.push(e);
    }
    Ok(written)
}
