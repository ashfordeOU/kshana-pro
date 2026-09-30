// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Workers
//!
//! A bounded pool of worker threads (`--workers N`, default 1) takes job identifiers
//! from an in-memory first-in, first-out queue. The queue is only a cache of the store:
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
//! Stopping: [`WorkerPool::stop`] lets each worker finish the job it is running, then
//! joins it. Jobs still queued stay `queued` on disk for the next start.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use super::delivery::{delivery_manifest, DeliveryLedger};
use super::jobs::{now_stamp, sha256_hex, JobState, JobStore};
use super::ops::{self, RunContext};
use crate::license::Entitlement;

/// Upper bound on `--workers`.
pub const MAX_WORKERS: usize = 64;

#[derive(Default)]
struct QueueState {
    items: VecDeque<String>,
    members: HashSet<String>,
    shutdown: bool,
}

/// The in-memory queue of job identifiers.
#[derive(Default)]
pub struct JobQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
}

impl JobQueue {
    /// Queue `id` unless it is already queued. Returns whether it was added.
    pub fn push(&self, id: &str) -> bool {
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if st.members.contains(id) {
            return false;
        }
        st.members.insert(id.to_string());
        st.items.push_back(id.to_string());
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
            if let Some(id) = st.items.pop_front() {
                st.members.remove(&id);
                return Some(id);
            }
            st = self.ready.wait(st).unwrap_or_else(|p| p.into_inner());
        }
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
            .items
            .len()
    }

    /// True when nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Everything the job routes and the workers share.
pub struct JobService {
    pub data_dir: PathBuf,
    pub store: JobStore,
    pub queue: JobQueue,
    pub ledger: DeliveryLedger,
    pub entitlement: Entitlement,
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
        })
    }

    /// Queue every job left `queued` or `running` by an earlier process. Returns how
    /// many were queued.
    pub fn recover(&self) -> usize {
        let ids = self.store.unfinished();
        let mut n = 0;
        for id in ids {
            if self.queue.push(&id) {
                n += 1;
            }
        }
        n
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
        let (now, utc) = now_stamp();
        status.state = JobState::Running;
        status.attempts += 1;
        status.started_unix = Some(now);
        status.started_utc = Some(utc);
        if let Err(e) = self.store.write_status(&status) {
            eprintln!("[job {}] cannot mark running: {e}", short(id));
            return;
        }
        eprintln!("[job {}] {} running", short(id), status.op);

        let outcome = self.run_op(id);
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
            Err(e) => {
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

    /// Run the job's operation, store its outputs and append the ledger records.
    /// Returns the result hash, the artefact entries and the result's own label, or the
    /// error text.
    #[allow(clippy::type_complexity)]
    fn run_op(
        &self,
        id: &str,
    ) -> Result<(String, Vec<super::jobs::ArtefactEntry>, Option<String>), String> {
        let (op_name, request) = self.store.request(id).map_err(|e| e.to_string())?;
        let op = ops::find(&op_name).ok_or_else(|| format!("unknown operation '{op_name}'"))?;
        let work_dir = self.store.job_dir(id).join("inputs");
        let (now, _) = now_stamp();
        let ctx = RunContext {
            entitlement: &self.entitlement,
            now_unix: now,
            work_dir: &work_dir,
        };
        let output =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (op.run)(&request, &ctx)))
                .map_err(|_| format!("internal error: the '{op_name}' operation panicked"))??;
        let (result_sha, artefacts) = self
            .store
            .write_outputs(id, &output.result, &output.artefacts)
            .map_err(|e| format!("cannot store the outputs: {e}"))?;
        let request_hash = sha256_hex(&self.store.request_bytes(id).map_err(|e| e.to_string())?);
        let run_hash = sha256_hex(delivery_manifest(&result_sha, &artefacts).as_bytes());
        let (_, delivered_utc) = now_stamp();
        self.ledger
            .append_delivery(id, &request_hash, &run_hash, &delivered_utc)
            .map_err(|e| format!("cannot record the delivery: {e}"))?;
        let label = serde_json::from_str::<serde_json::Value>(&output.result)
            .ok()
            .and_then(|v| v.get("label").and_then(|l| l.as_str()).map(str::to_string));
        Ok((result_sha, artefacts, label))
    }
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
