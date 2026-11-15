// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The job store on disk
//!
//! One directory per job under `<data-dir>/jobs/<job_id>/`:
//!
//! | File | Content |
//! |---|---|
//! | `request.json` | `{"op": ..., "request": ...}`, written once at submission |
//! | `status.json` | the [`JobStatus`]: state, server-clock times, attempts, error text |
//! | `result.json` | the operation's result document, written before the job is marked succeeded |
//! | `artefacts/` | the named artefacts the operation produced |
//! | `inputs/` | scratch inputs an operation needs as files (the `ci` scenario texts) |
//!
//! **Identity.** The job identifier is the SHA-256 (64 hexadecimal characters) of the
//! operation name and the canonical JavaScript Object Notation (JSON) of the request
//! (object keys sorted at every depth, no insignificant white space). Submitting the
//! same request again returns the same job and never runs it twice.
//!
//! **Durability.** Every file is written to a temporary name in the same directory,
//! flushed to disk and then renamed over the target, so a reader (or a restarted
//! server) sees either the old file or the new one, never a torn one. `status.json` is
//! the commit point: a job is succeeded only once its result and artefacts are on disk.
//!
//! **Ordering.** Each job carries a sequence number `seq`, one more than the largest
//! already in the store, so "newest first" does not depend on clock resolution.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Domain-separation prefix hashed before the operation name, so a job identifier can
/// never collide with any other SHA-256 the overlay computes over the same bytes.
const JOB_ID_DOMAIN: &str = "kshana-pro-job-v1";

/// The life-cycle state of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    /// Stored and waiting for a worker.
    Queued,
    /// A worker is executing it (a restart re-queues it).
    Running,
    /// Finished; `result.json` and the artefacts are on disk.
    Succeeded,
    /// Finished with an error (or its time limit); the error text is in the status.
    Failed,
    /// Cancelled by a caller before it finished (`POST /jobs/<id>/cancel`).
    Cancelled,
}

impl JobState {
    /// The lower-case name used in the documents.
    pub fn as_str(self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Running => "running",
            JobState::Succeeded => "succeeded",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }
    /// True once the job can no longer change.
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            JobState::Succeeded | JobState::Failed | JobState::Cancelled
        )
    }
}

/// One stored artefact: its file name, size and SHA-256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtefactEntry {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

/// `status.json`: everything the service knows about one job. Every time is read from
/// the server's clock (Unix seconds and the same instant as a UTC stamp).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobStatus {
    pub job_id: String,
    pub op: String,
    /// The tenant that submitted it (roadmap A9). Jobs stored before tenants existed read
    /// as the default tenant.
    #[serde(default = "default_tenant")]
    pub tenant: String,
    /// The scenario kind, when the operation runs one scenario (`run`, `dossier`).
    #[serde(default)]
    pub kind: Option<String>,
    pub state: JobState,
    /// Submission order within this data directory (larger is newer).
    pub seq: u64,
    pub submitted_unix: i64,
    pub submitted_utc: String,
    #[serde(default)]
    pub started_unix: Option<i64>,
    #[serde(default)]
    pub started_utc: Option<String>,
    #[serde(default)]
    pub finished_unix: Option<i64>,
    #[serde(default)]
    pub finished_utc: Option<String>,
    /// How many times a worker has started executing this job. It is 1 for a job that
    /// ran once; it is larger only when a stop or a crash interrupted a run and the
    /// restarted server ran it again.
    #[serde(default)]
    pub attempts: u32,
    /// The error text of a failed job (the engine's own message).
    #[serde(default)]
    pub error: Option<String>,
    /// SHA-256 of `result.json`, once succeeded.
    #[serde(default)]
    pub result_sha256: Option<String>,
    /// The stored artefacts, once succeeded.
    #[serde(default)]
    pub artefacts: Vec<ArtefactEntry>,
    /// The result's own top-level `label` (the engine's VALIDATED / MODELLED / PARTNER
    /// statement), copied verbatim when the result carries one.
    #[serde(default)]
    pub label: Option<String>,
    /// Queue priority (roadmap F2a): `high`, `normal` or `low`. Jobs stored before
    /// priorities existed read as `normal`.
    #[serde(default)]
    pub priority: Priority,
    /// Time limit of one run, seconds; past it the job fails with "timed out".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// Queue priority. Within a priority, first in, first out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    #[default]
    Normal,
    Low,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Normal => "normal",
            Priority::Low => "low",
        }
    }
    pub fn parse(s: &str) -> Option<Priority> {
        match s {
            "high" => Some(Priority::High),
            "normal" => Some(Priority::Normal),
            "low" => Some(Priority::Low),
            _ => None,
        }
    }
    /// Index into the queue's lanes (0 is served first).
    pub fn lane(self) -> usize {
        match self {
            Priority::High => 0,
            Priority::Normal => 1,
            Priority::Low => 2,
        }
    }
}

/// How a job is run, chosen at submission. Not part of the job's identity: the same
/// request submitted again with other options is the same job.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SubmitOptions {
    pub priority: Priority,
    pub timeout_secs: Option<u64>,
}

fn default_tenant() -> String {
    super::auth::DEFAULT_TENANT.to_string()
}

/// The engine's evidence tier at the start of a result label (`VALIDATED`, `MODELLED`
/// or `PARTNER`), when there is one.
pub fn label_tier(label: &str) -> Option<&'static str> {
    let first: String = label
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_uppercase())
        .collect();
    match first.as_str() {
        "VALIDATED" => Some("VALIDATED"),
        "MODELLED" => Some("MODELLED"),
        "PARTNER" => Some("PARTNER"),
        _ => None,
    }
}

/// Current server time as (Unix seconds, UTC stamp `YYYY-MM-DDTHH:MM:SSZ`).
pub fn now_stamp() -> (i64, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    (secs, utc_stamp(secs))
}

/// Format Unix seconds as a UTC stamp, `YYYY-MM-DDTHH:MM:SSZ` (standard library only).
pub fn utc_stamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    // Civil date from days since 1970-01-01 (proleptic Gregorian calendar).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if mo <= 2 { 1 } else { 0 };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Lower-case hexadecimal SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The canonical JSON of a value: object keys sorted at every depth, no white space.
/// Two requests that differ only in key order or spacing have the same canonical form.
pub fn canonical_json(v: &serde_json::Value) -> String {
    fn write(v: &serde_json::Value, out: &mut String) {
        match v {
            serde_json::Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                out.push('{');
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::Value::String((*k).clone()).to_string());
                    out.push(':');
                    write(&map[*k], out);
                }
                out.push('}');
            }
            serde_json::Value::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write(item, out);
                }
                out.push(']');
            }
            other => out.push_str(&other.to_string()),
        }
    }
    let mut out = String::new();
    write(v, &mut out);
    out
}

/// The job identifier: SHA-256 over a fixed prefix, the operation name and the
/// canonical request, as 64 lower-case hexadecimal characters.
pub fn job_id(op: &str, request: &serde_json::Value) -> String {
    tenant_job_id(super::auth::DEFAULT_TENANT, op, request)
}

/// The job identifier within a tenant. The default tenant's identifiers are the ones jobs
/// had before tenants existed; any other tenant's name is hashed in as well, so two
/// tenants submitting the same request get two jobs.
pub fn tenant_job_id(tenant: &str, op: &str, request: &serde_json::Value) -> String {
    let mut h = Sha256::new();
    h.update(JOB_ID_DOMAIN.as_bytes());
    h.update(b"\n");
    if tenant != super::auth::DEFAULT_TENANT {
        h.update(b"tenant:");
        h.update(tenant.as_bytes());
        h.update(b"\n");
    }
    h.update(op.as_bytes());
    h.update(b"\n");
    h.update(canonical_json(request).as_bytes());
    hex::encode(h.finalize())
}

/// True when `s` is a well-formed job identifier (64 lower-case hexadecimal characters).
pub fn is_job_id(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// True when `s` is a safe single file name: 1 to 128 characters from
/// `A-Z a-z 0-9 . _ -`, not starting with a dot (so never `.` or `..`) and never
/// containing a path separator.
pub fn is_safe_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.starts_with('.')
        && !s.contains("..")
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write `bytes` to `path` atomically: a temporary file in the same directory, flushed
/// to disk, then renamed over `path`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("atomic write needs a parent directory"))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::other("atomic write needs a file name"))?;
    let tmp = dir.join(format!(
        ".{name}.tmp-{}-{}",
        std::process::id(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// The on-disk job store.
pub struct JobStore {
    root: PathBuf,
    /// Serialises create-or-get so two identical submissions create one job.
    create_lock: Mutex<()>,
    next_seq: AtomicU64,
}

impl JobStore {
    /// Open (creating when absent) the store under `<data_dir>/jobs`.
    pub fn open(data_dir: &Path) -> io::Result<JobStore> {
        let root = data_dir.join("jobs");
        fs::create_dir_all(&root)?;
        let store = JobStore {
            root,
            create_lock: Mutex::new(()),
            next_seq: AtomicU64::new(1),
        };
        let max_seq = store.list_all().iter().map(|s| s.seq).max().unwrap_or(0);
        store.next_seq.store(max_seq + 1, Ordering::SeqCst);
        Ok(store)
    }

    /// The directory of one job. The caller has checked [`is_job_id`].
    pub fn job_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// Store a new job, or return the existing one with the same identifier. Returns
    /// the status and whether this call created it.
    pub fn submit(
        &self,
        op: &str,
        request: &serde_json::Value,
        kind: Option<String>,
    ) -> io::Result<(JobStatus, bool)> {
        self.submit_for(super::auth::DEFAULT_TENANT, op, request, kind)
    }

    /// [`submit`](Self::submit) for a tenant.
    pub fn submit_for(
        &self,
        tenant: &str,
        op: &str,
        request: &serde_json::Value,
        kind: Option<String>,
    ) -> io::Result<(JobStatus, bool)> {
        self.submit_with(tenant, op, request, kind, SubmitOptions::default())
    }

    /// [`submit_for`](Self::submit_for) with a priority and a time limit.
    pub fn submit_with(
        &self,
        tenant: &str,
        op: &str,
        request: &serde_json::Value,
        kind: Option<String>,
        opts: SubmitOptions,
    ) -> io::Result<(JobStatus, bool)> {
        let id = tenant_job_id(tenant, op, request);
        let _guard = self.create_lock.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = self.status(&id)? {
            return Ok((existing, false));
        }
        let dir = self.job_dir(&id);
        fs::create_dir_all(dir.join("artefacts"))?;
        let doc = serde_json::json!({ "op": op, "request": request });
        let body = serde_json::to_string_pretty(&doc).map_err(io::Error::other)?;
        write_atomic(&dir.join("request.json"), body.as_bytes())?;
        let (now, utc) = now_stamp();
        let status = JobStatus {
            job_id: id,
            op: op.to_string(),
            tenant: tenant.to_string(),
            kind,
            state: JobState::Queued,
            seq: self.next_seq.fetch_add(1, Ordering::SeqCst),
            submitted_unix: now,
            submitted_utc: utc,
            started_unix: None,
            started_utc: None,
            finished_unix: None,
            finished_utc: None,
            attempts: 0,
            error: None,
            result_sha256: None,
            artefacts: Vec::new(),
            label: None,
            priority: opts.priority,
            timeout_secs: opts.timeout_secs,
        };
        // status.json last: a directory without it is an incomplete submission and is
        // ignored by every reader.
        self.write_status(&status)?;
        Ok((status, true))
    }

    /// Read one job's status; `None` when there is no such job.
    pub fn status(&self, id: &str) -> io::Result<Option<JobStatus>> {
        let p = self.job_dir(id).join("status.json");
        match fs::read_to_string(&p) {
            Ok(s) => serde_json::from_str(&s)
                .map(Some)
                .map_err(|e| io::Error::other(format!("job {id}: unreadable status: {e}"))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Replace one job's status atomically.
    pub fn write_status(&self, status: &JobStatus) -> io::Result<()> {
        let body = serde_json::to_string_pretty(status).map_err(io::Error::other)?;
        write_atomic(
            &self.job_dir(&status.job_id).join("status.json"),
            body.as_bytes(),
        )
    }

    /// The raw bytes of `request.json`.
    pub fn request_bytes(&self, id: &str) -> io::Result<Vec<u8>> {
        fs::read(self.job_dir(id).join("request.json"))
    }

    /// The stored `(op, request)` of one job.
    pub fn request(&self, id: &str) -> io::Result<(String, serde_json::Value)> {
        let bytes = self.request_bytes(id)?;
        let doc: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::other(format!("job {id}: unreadable request: {e}")))?;
        let op = doc
            .get("op")
            .and_then(|v| v.as_str())
            .ok_or_else(|| io::Error::other(format!("job {id}: request has no op")))?
            .to_string();
        Ok((op, doc.get("request").cloned().unwrap_or_default()))
    }

    /// Write the result and the artefacts of a job (before it is marked succeeded).
    /// Returns the result's SHA-256 and the artefact entries.
    pub fn write_outputs(
        &self,
        id: &str,
        result: &str,
        artefacts: &[(String, String)],
    ) -> io::Result<(String, Vec<ArtefactEntry>)> {
        let dir = self.job_dir(id);
        let art_dir = dir.join("artefacts");
        fs::create_dir_all(&art_dir)?;
        let mut entries = Vec::new();
        for (name, body) in artefacts {
            if !is_safe_name(name) {
                return Err(io::Error::other(format!(
                    "operation produced an unsafe artefact name '{name}'"
                )));
            }
            write_atomic(&art_dir.join(name), body.as_bytes())?;
            entries.push(ArtefactEntry {
                name: name.clone(),
                bytes: body.len() as u64,
                sha256: sha256_hex(body.as_bytes()),
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        write_atomic(&dir.join("result.json"), result.as_bytes())?;
        Ok((sha256_hex(result.as_bytes()), entries))
    }

    /// The stored result document of a job.
    pub fn read_result(&self, id: &str) -> io::Result<String> {
        fs::read_to_string(self.job_dir(id).join("result.json"))
    }

    /// One stored artefact; `None` when the job has no artefact of that name. The
    /// caller has checked [`is_safe_name`].
    pub fn read_artefact(&self, id: &str, name: &str) -> io::Result<Option<String>> {
        match fs::read_to_string(self.job_dir(id).join("artefacts").join(name)) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Every readable job status, in no particular order. A directory whose status
    /// cannot be read is reported on standard error and skipped.
    fn list_all(&self) -> Vec<JobStatus> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return out;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(id) = name.to_str() else { continue };
            if !is_job_id(id) {
                continue;
            }
            match self.status(id) {
                Ok(Some(s)) => out.push(s),
                Ok(None) => {} // incomplete submission: no status yet
                Err(e) => eprintln!("[jobs] skipping {id}: {e}"),
            }
        }
        out
    }

    /// Delete a job: its request, status, result and artefacts (the hosted trial's
    /// retention, [`super::trial`]). The caller has checked [`is_job_id`].
    pub fn remove(&self, id: &str) -> io::Result<()> {
        if !is_job_id(id) {
            return Err(io::Error::other("not a job id"));
        }
        fs::remove_dir_all(self.job_dir(id))
    }

    /// Every job, newest first.
    pub fn list(&self) -> Vec<JobStatus> {
        let mut all = self.list_all();
        all.sort_by(|a, b| b.seq.cmp(&a.seq).then_with(|| a.job_id.cmp(&b.job_id)));
        all
    }

    /// The identifiers of every job not yet finished (queued or running), oldest first.
    pub fn unfinished(&self) -> Vec<String> {
        let mut all: Vec<JobStatus> = self
            .list_all()
            .into_iter()
            .filter(|s| !s.state.is_finished())
            .collect();
        all.sort_by_key(|s| s.seq);
        all.into_iter().map(|s| s.job_id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_sorts_keys_at_every_depth() {
        let a: serde_json::Value =
            serde_json::from_str(r#"{"b":1,"a":{"y":[1,{"q":2,"p":1}],"x":"s"}}"#).unwrap();
        let b: serde_json::Value =
            serde_json::from_str(r#"{ "a": {"x":"s", "y":[1,{"p":1,"q":2}]}, "b":1 }"#).unwrap();
        assert_eq!(canonical_json(&a), canonical_json(&b));
        assert_eq!(
            canonical_json(&a),
            r#"{"a":{"x":"s","y":[1,{"p":1,"q":2}]},"b":1}"#
        );
    }

    #[test]
    fn job_id_depends_on_op_and_request_only() {
        let r = serde_json::json!({"scenario": "kind = \"ephemeris\"\n"});
        let id = job_id("run", &r);
        assert!(is_job_id(&id));
        assert_eq!(id, job_id("run", &r.clone()));
        assert_ne!(id, job_id("dossier", &r));
    }

    #[test]
    fn identifier_and_name_checks() {
        assert!(!is_job_id("abc"));
        assert!(!is_job_id(&"G".repeat(64)));
        assert!(
            !is_job_id(&"A".repeat(64)),
            "upper case is not the canonical form"
        );
        assert!(is_safe_name("chart.svg"));
        for bad in ["", "..", ".hidden", "a/b", "a\\b", "..%2f", "x..y", "a b"] {
            assert!(!is_safe_name(bad), "{bad:?} must be refused");
        }
    }

    #[test]
    fn utc_stamp_matches_known_instants() {
        assert_eq!(utc_stamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_stamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_stamp(1_790_000_000), "2026-09-21T14:13:20Z");
    }
}
