// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Measurement log: every instrument exchange as one JSON line, kept as evidence.
//!
//! Each line is a [`LogEntry`]:
//!
//! ```json
//! {"seq":0,"utc":"2026-11-18T10:00:00.123Z","instrument":"tic1","kind":"query",
//!  "command":":READ?","response":"+1.234E-08","error":null}
//! ```
//!
//! `kind` is `write`, `query`, `read` (unsolicited line) or `note`. The timestamp is the
//! host's system clock in UTC (not a traceable time; it orders and dates the evidence).
//! Tests inject a fixed clock with [`MeasurementLog::with_clock`].

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// One recorded exchange.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub seq: u64,
    pub utc: String,
    pub instrument: String,
    pub kind: String,
    pub command: String,
    pub response: Option<String>,
    pub error: Option<String>,
}

/// A log shared between instrument sessions.
pub type SharedLog = Arc<Mutex<MeasurementLog>>;

/// JSON-lines measurement log (optionally mirrored to a file, appended and flushed per line).
pub struct MeasurementLog {
    path: Option<PathBuf>,
    file: Option<File>,
    entries: Vec<LogEntry>,
    clock: Box<dyn Fn() -> String + Send>,
    seq: u64,
}

/// Current UTC time as ISO 8601 with milliseconds, from the system clock.
pub fn utc_now_iso() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let (y, m, dd) = crate::time::civil_from_days(secs.div_euclid(86_400));
    let sod = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{dd:02}T{:02}:{:02}:{:02}.{:03}Z",
        sod / 3600,
        (sod / 60) % 60,
        sod % 60,
        d.subsec_millis()
    )
}

/// Current UTC date `YYYY-MM-DD` from the system clock.
pub fn utc_today() -> String {
    utc_now_iso()[..10].to_string()
}

impl MeasurementLog {
    /// In-memory log.
    pub fn memory() -> MeasurementLog {
        MeasurementLog {
            path: None,
            file: None,
            entries: Vec::new(),
            clock: Box::new(utc_now_iso),
            seq: 0,
        }
    }

    /// Log appended to `path` (created if missing; existing lines are kept).
    pub fn to_file(path: &Path) -> Result<MeasurementLog, String> {
        let f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(MeasurementLog {
            path: Some(path.to_path_buf()),
            file: Some(f),
            ..MeasurementLog::memory()
        })
    }

    /// Replace the timestamp source (deterministic tests).
    pub fn with_clock(mut self, clock: impl Fn() -> String + Send + 'static) -> MeasurementLog {
        self.clock = Box::new(clock);
        self
    }

    pub fn shared(self) -> SharedLog {
        Arc::new(Mutex::new(self))
    }

    /// Record one exchange.
    pub fn record(
        &mut self,
        instrument: &str,
        kind: &str,
        command: &str,
        response: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), String> {
        let e = LogEntry {
            seq: self.seq,
            utc: (self.clock)(),
            instrument: instrument.to_string(),
            kind: kind.to_string(),
            command: command.to_string(),
            response: response.map(str::to_string),
            error: error.map(str::to_string),
        };
        self.seq += 1;
        if let Some(f) = &mut self.file {
            let line = serde_json::to_string(&e).expect("serialisable");
            writeln!(f, "{line}")
                .and_then(|_| f.flush())
                .map_err(|err| format!("measurement log: {err}"))?;
        }
        self.entries.push(e);
        Ok(())
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// All entries as JSON lines.
    pub fn to_jsonl(&self) -> String {
        let mut s = String::new();
        for e in &self.entries {
            s.push_str(&serde_json::to_string(e).expect("serialisable"));
            s.push('\n');
        }
        s
    }

    /// Parse a JSON-lines log.
    pub fn parse_jsonl(text: &str) -> Result<Vec<LogEntry>, String> {
        text.lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("line {}: {e}", i + 1)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_to_file_and_parses_back() {
        let d = std::env::temp_dir().join(format!("kshana-mlog-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("log.jsonl");
        let _ = std::fs::remove_file(&p);
        let mut l = MeasurementLog::to_file(&p)
            .unwrap()
            .with_clock(|| "2026-11-18T00:00:00.000Z".into());
        l.record("tic", "query", ":READ?", Some("1e-8"), None)
            .unwrap();
        l.record("tic", "write", "*RST", None, None).unwrap();
        let back = MeasurementLog::parse_jsonl(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(back, l.entries());
        assert_eq!(back[1].seq, 1);
        assert_eq!(l.to_jsonl().lines().count(), 2);
        let now = utc_now_iso();
        assert_eq!(now.len(), 24);
        assert!(now.ends_with('Z'));
        std::fs::remove_dir_all(&d).ok();
    }
}
