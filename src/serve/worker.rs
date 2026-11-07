// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Workers
//!
//! A bounded pool of worker threads (`--workers N`, default 1) takes job identifiers
//! from an in-memory queue with three priority lanes (high, normal, low; first in,
//! first out within a lane). The queue is only a cache of the store:
//! the job's state on disk is the truth. On start, every job found in state `running`
//! or `queued` is queued again in submission order, so a stop or a crash loses no work;
//! a finished job (succeeded or failed) is never run again.
//!
//! Executing a job: mark it `running` (attempts + 1, server-clock start time), run its
//! operation with the process entitlement, write the result and the artefacts, append
//! the delivery records to the ledger, and only then mark it `succeeded`. Any error, or
//! a panic inside the operation, marks it `failed` with the error text; a failed job is
//! never written to the ledger.
//!
//! Cancellation and time limits (roadmap F2a): a job runs on a thread of its own while
//! its worker watches for a cancel request (`POST /jobs/<id>/cancel`) and for the job's
//! `timeout_secs`; either ends the job (`cancelled`, or `failed` with "timed out") and
//! the abandoned run's output is never stored.
//!
//! Stopping: [`WorkerPool::stop`] lets each worker finish the job it is running, then
//! joins it. Jobs still queued stay `queued` on disk for the next start.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::delivery::{delivery_manifest, DeliveryLedger};
use super::jobs::{now_stamp, sha256_hex, JobState, JobStore, Priority};
use super::ops::{self, OpOutput, RunContext};
use crate::license::Entitlement;

/// Upper bound on `--workers`.
pub const MAX_WORKERS: usize = 64;

/// Upper bound on a job's time limit (one week).
pub const MAX_TIMEOUT_SECS: u64 = 7 * 86_400;

#[derive(Default)]
struct QueueState {
    /// One lane per priority, high first.
    lanes: [VecDeque<String>; 3],
    members: HashSet<String>,
    shutdown: bool,
}

/// The in-memory queue of job identifiers: three first-in, first-out lanes (high,
/// normal, low); a worker always takes from the highest non-empty lane.
#[derive(Default)]
pub struct JobQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
}

impl JobQueue {
    /// Queue `id` at normal priority unless it is already queued.
    pub fn push(&self, id: &str) -> bool {
        self.push_with(id, Priority::Normal)
    }

    /// Queue `id` at `priority` unless it is already queued. Returns whether it was added.
    pub fn push_with(&self, id: &str, priority: Priority) -> bool {
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if st.members.contains(id) {
            return false;
        }
        st.members.insert(id.to_string());
        st.lanes[priority.lane()].push_back(id.to_string());
        self.ready.notify_one();
        true
    }

    /// Block until a job is available (Some) or the queue is shut down (None).
    fn pop(&self) -> Option<String> {
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if st.shutdown {
                return None;
            }
            if let Some(id) = st.lanes.iter_mut().find_map(|l| l.pop_front()) {
                st.members.remove(&id);
                return Some(id);
            }
            st = self.ready.wait(st).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// Remove `id` if it is waiting. Returns whether it was.
    pub fn remove(&self, id: &str) -> bool {
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if !st.members.remove(id) {
            return false;
        }
        for l in st.lanes.iter_mut() {
            l.retain(|x| x != id);
        }
        true
    }

    fn shutdown(&self) {
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        st.shutdown = true;
        self.ready.notify_all();
    }

    /// Number of identifiers waiting.
    pub fn len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .members
            .len()
    }

    /// True when nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Why a cancellation was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum CancelError {
    NotFound,
    Finished(JobState),
    Io(String),
}

/// Everything the job routes and the workers share.
pub struct JobService {
    pub data_dir: PathBuf,
    pub store: JobStore,
    pub queue: JobQueue,
    pub ledger: DeliveryLedger,
    pub entitlement: Entitlement,
    /// Cancel flags of the jobs running in this process.
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Whether this process may run jobs. Always true without high availability; with
    /// it, true only while this node holds the lease ([`super::ha`]).
    pub active: Arc<AtomicBool>,
}

impl JobService {
    /// Open the store and the ledger under `data_dir`.
    pub fn open(data_dir: PathBuf, entitlement: Entitlement) -> std::io::Result<JobService> {
        std::fs::create_dir_all(&data_dir)?;
        Ok(JobService {
            store: JobStore::open(&data_dir)?,
            ledger: DeliveryLedger::new(&data_dir),
            queue: JobQueue::default(),
            entitlement,
            data_dir,
            running: Mutex::new(HashMap::new()),
            active: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Queue every job left `queued` or `running` (by an earlier process, or by another
    /// node of a highly available pair), at its priority. Returns how many were queued.
    pub fn recover(&self) -> usize {
        let ids = self.store.unfinished();
        let running = self.running.lock().unwrap_or_else(|p| p.into_inner());
        let mut n = 0;
        for id in ids {
            if running.contains_key(&id) {
                continue;
            }
            let pr = self
                .store
                .status(&id)
                .ok()
                .flatten()
                .map(|s| s.priority)
                .unwrap_or_default();
            if self.queue.push_with(&id, pr) {
                n += 1;
            }
        }
        n
    }

    /// Cancel a job: a queued job is marked cancelled at once; a running one is told to
    /// stop and is marked cancelled by its worker. A finished job is not changed.
    pub fn cancel(&self, id: &str, by: &str) -> Result<super::jobs::JobStatus, CancelError> {
        let mut status = match self.store.status(id) {
            Ok(Some(s)) => s,
            Ok(None) => return Err(CancelError::NotFound),
            Err(e) => return Err(CancelError::Io(e.to_string())),
        };
        if status.state.is_finished() {
            return Err(CancelError::Finished(status.state));
        }
        if let Some(flag) = self
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
        {
            flag.store(true, Ordering::SeqCst);
            return Ok(status);
        }
        self.queue.remove(id);
        let (now, utc) = now_stamp();
        status.state = JobState::Cancelled;
        status.finished_unix = Some(now);
        status.finished_utc = Some(utc);
        status.error = Some(format!("cancelled by {by}"));
        self.store
            .write_status(&status)
            .map_err(|e| CancelError::Io(e.to_string()))?;
        Ok(status)
    }

    /// Execute one job to completion (or skip it when it is already finished).
    pub fn execute(&self, id: &str) {
        let mut status = match self.store.status(id) {
            Ok(Some(s)) => s,
            Ok(None) => return,
            Err(e) => {
                eprintln!("[job {}] cannot read status: {e}", short(id));
                return;
            }
        };
        if status.state.is_finished() {
            return;
        }
        let flag = Arc::new(AtomicBool::new(false));
        self.running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.to_string(), flag.clone());
        let (now, utc) = now_stamp();
        status.state = JobState::Running;
        status.attempts += 1;
        status.started_unix = Some(now);
        status.started_utc = Some(utc);
        if let Err(e) = self.store.write_status(&status) {
            eprintln!("[job {}] cannot mark running: {e}", short(id));
            self.running
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(id);
            return;
        }
        eprintln!("[job {}] {} running", short(id), status.op);

        let outcome = self.run_op(id, status.timeout_secs, &flag);
        self.running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
        let (now, utc) = now_stamp();
        status.finished_unix = Some(now);
        status.finished_utc = Some(utc.clone());
        match outcome {
            Ok((result_sha, artefacts, label)) => {
                status.state = JobState::Succeeded;
                status.result_sha256 = Some(result_sha);
                status.artefacts = artefacts;
                status.label = label;
                status.error = None;
            }
            Err(Stop::Cancelled) => {
                status.state = JobState::Cancelled;
                status.error = Some("cancelled while running".into());
                status.result_sha256 = None;
                status.artefacts = Vec::new();
            }
            Err(Stop::Failed(e)) => {
                status.state = JobState::Failed;
                status.error = Some(e);
                status.result_sha256 = None;
                status.artefacts = Vec::new();
            }
        }
        if let Err(e) = self.store.write_status(&status) {
            eprintln!("[job {}] cannot write final status: {e}", short(id));
            return;
        }
        eprintln!(
            "[job {}] {} {}",
            short(id),
            status.op,
            status.state.as_str()
        );
    }

    /// Run the job's operation on its own thread, waiting for it, for a cancel request
    /// or for the time limit, whichever comes first; then store the outputs and append
    /// the ledger records. A cancelled or timed-out run is abandoned: its thread finishes
    /// on its own and whatever it returns is dropped, so nothing of it is stored or
    /// delivered.
    #[allow(clippy::type_complexity)]
    fn run_op(
        &self,
        id: &str,
        timeout_secs: Option<u64>,
        cancel: &AtomicBool,
    ) -> Result<(String, Vec<super::jobs::ArtefactEntry>, Option<String>), Stop> {
        let (op_name, request) = self
            .store
            .request(id)
            .map_err(|e| Stop::Failed(e.to_string()))?;
        let op = ops::find(&op_name)
            .ok_or_else(|| Stop::Failed(format!("unknown operation '{op_name}'")))?;
        let work_dir = self.store.job_dir(id).join("inputs");
        let (now, _) = now_stamp();
        let ent = self.entitlement.clone();
        let (tx, rx) = mpsc::channel::<Result<OpOutput, String>>();
        let name = op_name.clone();
        std::thread::Builder::new()
            .name(format!("kshana-job-{}", short(id)))
            .spawn(move || {
                let ctx = RunContext {
                    entitlement: &ent,
                    now_unix: now,
                    work_dir: &work_dir,
                };
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    (op.run)(&request, &ctx)
                }))
                .unwrap_or_else(|_| {
                    Err(format!("internal error: the '{name}' operation panicked"))
                });
                let _ = tx.send(out);
            })
            .map_err(|e| Stop::Failed(format!("cannot start the run: {e}")))?;
        let deadline = timeout_secs.map(|t| Instant::now() + Duration::from_secs(t));
        let output = loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(Stop::Cancelled);
            }
            if let Some(d) = deadline {
                if Instant::now() >= d {
                    return Err(Stop::Failed(format!(
                        "timed out after {} s (the job's time limit)",
                        timeout_secs.unwrap_or(0)
                    )));
                }
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(r) => break r.map_err(Stop::Failed)?,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(Stop::Failed("the run ended without a result".into()))
                }
            }
        };
        let (result_sha, artefacts) = self
            .store
            .write_outputs(id, &output.result, &output.artefacts)
            .map_err(|e| Stop::Failed(format!("cannot store the outputs: {e}")))?;
        let request_hash = sha256_hex(
            &self
                .store
                .request_bytes(id)
                .map_err(|e| Stop::Failed(e.to_string()))?,
        );
        let run_hash = sha256_hex(delivery_manifest(&result_sha, &artefacts).as_bytes());
        let (_, delivered_utc) = now_stamp();
        self.ledger
            .append_delivery(id, &request_hash, &run_hash, &delivered_utc)
            .map_err(|e| Stop::Failed(format!("cannot record the delivery: {e}")))?;
        let label = serde_json::from_str::<serde_json::Value>(&output.result)
            .ok()
            .and_then(|v| v.get("label").and_then(|l| l.as_str()).map(str::to_string));
        Ok((result_sha, artefacts, label))
    }
}

/// Why a run stopped without a result.
enum Stop {
    Cancelled,
    Failed(String),
}

/// The first 12 characters of a job identifier, for log lines.
fn short(id: &str) -> &str {
    &id[..id.len().min(12)]
}

/// The running worker threads.
pub struct WorkerPool {
    service: Arc<JobService>,
    handles: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    /// Start `n` workers (clamped to 1..=[`MAX_WORKERS`]).
    pub fn spawn(n: usize, service: Arc<JobService>) -> WorkerPool {
        let n = n.clamp(1, MAX_WORKERS);
        let handles = (0..n)
            .map(|i| {
                let svc = service.clone();
                std::thread::Builder::new()
                    .name(format!("kshana-job-worker-{i}"))
                    .spawn(move || {
                        while let Some(id) = svc.queue.pop() {
                            if !svc.active.load(Ordering::SeqCst) {
                                // This node lost the high-availability lease: leave the
                                // job queued on disk for the node that holds it.
                                continue;
                            }
                            svc.execute(&id);
                        }
                    })
                    .expect("spawn a worker thread")
            })
            .collect();
        WorkerPool { service, handles }
    }

    /// Let every worker finish its current job, then join them all.
    pub fn stop(self) {
        self.service.queue.shutdown();
        for h in self.handles {
            let _ = h.join();
        }
    }
}
