// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The audit log (roadmap A9)
//!
//! One JSON line per request (except `GET /health`) in `<data-dir>/audit/YYYY-MM-DD.jsonl`
//! (UTC days): time, principal id, tenant, role, source, method, route template, job id when
//! there is one, status and duration. Never a token, a request body, a query string or a
//! client address. Each line carries `prev`, the SHA-256 of the previous line's bytes (the
//! first line of a day chains to the last line of the day before; the very first line to
//! 64 zeros), so deleting or editing a line breaks the chain at the next one, and
//! [`verify`] names it. Whole day files older than the retention are deleted by
//! [`AuditLog::prune`]; the deletion is itself a line of the log.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// One audit line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Server time, Unix seconds.
    pub t: i64,
    /// Principal id (`-` for a refused request).
    pub who: String,
    pub tenant: String,
    pub role: String,
    /// `open`, `key`, `oidc`, or `-`.
    pub source: String,
    pub method: String,
    /// The route template (`/jobs/{id}`), `-` when no route matched, or an internal event.
    pub route: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    pub status: u16,
    pub ms: u64,
    /// SHA-256 of the previous line.
    #[serde(default)]
    pub prev: String,
}

struct State {
    day: i64,
    last: String,
}

/// The append-only audit log of one data directory.
pub struct AuditLog {
    dir: PathBuf,
    state: Mutex<State>,
}

fn day_name(day: i64) -> String {
    let (y, m, d) = civil(day);
    format!("{y:04}-{m:02}-{d:02}.jsonl")
}

/// Civil date of a day number since 1970-01-01 (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The day files of `dir`, oldest first.
fn day_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                        n.len() == 16 && n.ends_with(".jsonl") && n.as_bytes()[4] == b'-'
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn digest(line: &str) -> String {
    hex::encode(Sha256::digest(line.as_bytes()))
}

impl AuditLog {
    /// Open (creating when absent) `<data_dir>/audit`, continuing the chain from the last line.
    pub fn open(data_dir: &Path) -> io::Result<AuditLog> {
        let dir = data_dir.join("audit");
        fs::create_dir_all(&dir)?;
        let last = day_files(&dir)
            .last()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|t| t.lines().last().map(digest))
            .unwrap_or_else(|| GENESIS.to_string());
        Ok(AuditLog {
            dir,
            state: Mutex::new(State {
                day: i64::MIN,
                last,
            }),
        })
    }

    /// The directory of the day files.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Append one entry (its `prev` is set here). An I/O error is returned, never hidden.
    pub fn record(&self, mut e: Entry) -> io::Result<()> {
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        e.prev = st.last.clone();
        let line = serde_json::to_string(&e).map_err(io::Error::other)?;
        let day = e.t.div_euclid(86_400);
        st.day = day;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(day_name(day)))?;
        f.write_all(format!("{line}\n").as_bytes())?;
        f.sync_data()?;
        st.last = digest(&line);
        Ok(())
    }

    /// Delete whole day files older than `days` days before `now`, and record each deletion.
    /// The chain then starts at the oldest kept line, whose `prev` names the deleted
    /// file's last line; [`verify`] accepts that one break only when the deletion is logged.
    pub fn prune(&self, days: u32, now: i64) -> io::Result<usize> {
        let cutoff = now.div_euclid(86_400) - i64::from(days);
        let mut n = 0;
        for p in day_files(&self.dir) {
            let Some(name) = p.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
                continue;
            };
            if name.as_str() >= day_name(cutoff).as_str() {
                break;
            }
            fs::remove_file(&p)?;
            n += 1;
            self.record(Entry {
                t: now,
                who: "kshana-pro".into(),
                tenant: "-".into(),
                role: "-".into(),
                source: "-".into(),
                method: "-".into(),
                route: format!("retention: deleted audit/{name}"),
                job: None,
                status: 0,
                ms: 0,
                prev: String::new(),
            })?;
        }
        Ok(n)
    }
}

/// What [`verify`] found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub verified: bool,
    pub lines: usize,
    pub files: usize,
    /// `file:line` of the first line whose `prev` does not match, or that does not parse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_bad: Option<String>,
}

/// Check the chain over every day file of `dir`. The first kept line may start from any
/// `prev` (older files may have been deleted by retention); every later line must chain.
pub fn verify(dir: &Path) -> Report {
    let files = day_files(dir);
    let mut last: Option<String> = None;
    let mut lines = 0;
    for p in &files {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let Ok(text) = fs::read_to_string(p) else {
            return Report {
                verified: false,
                lines,
                files: files.len(),
                first_bad: Some(name),
            };
        };
        for (i, l) in text.lines().enumerate() {
            let bad = || Report {
                verified: false,
                lines,
                files: files.len(),
                first_bad: Some(format!("{name}:{}", i + 1)),
            };
            let Ok(e) = serde_json::from_str::<Entry>(l) else {
                return bad();
            };
            if last.as_ref().is_some_and(|d| *d != e.prev) {
                return bad();
            }
            last = Some(digest(l));
            lines += 1;
        }
    }
    Report {
        verified: true,
        lines,
        files: files.len(),
        first_bad: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "kshana-pro-audit-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn entry(t: i64, route: &str) -> Entry {
        Entry {
            t,
            who: "k1".into(),
            tenant: "a".into(),
            role: "reader".into(),
            source: "key".into(),
            method: "GET".into(),
            route: route.into(),
            job: None,
            status: 200,
            ms: 1,
            prev: String::new(),
        }
    }

    #[test]
    fn day_names_are_utc_dates() {
        assert_eq!(day_name(0), "1970-01-01.jsonl");
        assert_eq!(day_name(20_398), "2025-11-06.jsonl");
    }

    #[test]
    fn the_chain_spans_days_and_restarts_and_catches_edits_and_deletions() {
        let d = tmp();
        let day = 86_400;
        {
            let log = AuditLog::open(&d).unwrap();
            log.record(entry(10 * day + 5, "/jobs")).unwrap();
            log.record(entry(10 * day + 6, "/jobs/{id}")).unwrap();
        }
        // A restart continues the chain into the next day.
        let log = AuditLog::open(&d).unwrap();
        log.record(entry(11 * day + 1, "/ledger")).unwrap();
        log.record(entry(11 * day + 2, "/kinds")).unwrap();
        let r = verify(log.dir());
        assert_eq!((r.verified, r.lines, r.files), (true, 4, 2));

        // Edit one line: the next line no longer chains.
        let f = log.dir().join(day_name(10));
        let text = fs::read_to_string(&f).unwrap();
        fs::write(&f, text.replacen("\"/jobs\"", "\"/kinds\"", 1)).unwrap();
        assert_eq!(
            verify(log.dir()).first_bad.as_deref(),
            Some("1970-01-11.jsonl:2")
        );
        fs::write(&f, &text).unwrap();
        assert!(verify(log.dir()).verified);

        // Delete a middle line: caught at the line after it.
        let f2 = log.dir().join(day_name(11));
        let t2 = fs::read_to_string(&f2).unwrap();
        let kept: Vec<&str> = t2.lines().skip(1).collect();
        fs::write(&f2, kept.join("\n") + "\n").unwrap();
        assert_eq!(
            verify(log.dir()).first_bad.as_deref(),
            Some("1970-01-12.jsonl:1")
        );
        fs::write(&f2, &t2).unwrap();

        // Retention deletes the old day and logs it; the rest still verifies.
        assert_eq!(log.prune(1, 12 * day).unwrap(), 1);
        let r = verify(log.dir());
        assert!(r.verified, "{r:?}");
        assert_eq!(r.files, 2);
        let today = fs::read_to_string(log.dir().join(day_name(12))).unwrap();
        assert!(today.contains("retention: deleted audit/1970-01-11.jsonl"));
        let _ = fs::remove_dir_all(&d);
    }
}
