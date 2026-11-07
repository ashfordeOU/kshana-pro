// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # High availability: two or more nodes on one shared data directory (roadmap F2a)
//!
//! Every node serves the API (reads, and submissions stored to the shared job store);
//! exactly one node, the holder of the **lease**, runs jobs. The lease is
//! `<data-dir>/ha/lease.json` (node name and expiry). The holder renews it every third
//! of the lease time; a standby that finds it expired takes it over, re-queues every job
//! left queued or running (the failed node's running job runs again, `attempts` 2), and
//! from then on runs the queue. A lease change is made under `<data-dir>/ha/lock`,
//! created exclusively, so two standbys cannot both take the lease; a lock left by a
//! node that died while holding it is removed once it is older than the lease time.
//!
//! The holder also rescans the store each renewal, so a job submitted through a standby
//! is run by the holder within a third of the lease time.
//!
//! Limits: the shared directory must give exclusive create and atomic rename (a local
//! disk, NFS version 3 or later, SMB). A holder that is paused for longer than the lease
//! (for example cut off from the shared storage) may finish the job it was running after
//! a standby has taken over and started the same job again; the lease time is chosen
//! well above any expected pause (default 30 s). Each node keeps its own audit log
//! (`audit/<node>/`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::worker::JobService;

/// Lease settings for one node.
#[derive(Debug, Clone)]
pub struct HaConfig {
    /// This node's name (letters, digits, `-`, `_`, `.`).
    pub node: String,
    /// Lease time in milliseconds (default 30 000).
    pub lease_ms: u64,
}

impl HaConfig {
    pub fn new(node: &str, lease_ms: u64) -> Result<HaConfig, String> {
        if node.is_empty()
            || node.len() > 64
            || !node
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        {
            return Err("a node name is 1 to 64 letters, digits, '-', '_' or '.'".into());
        }
        if !(200..=600_000).contains(&lease_ms) {
            return Err("the lease time is 0.2 to 600 seconds".into());
        }
        Ok(HaConfig {
            node: node.to_string(),
            lease_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Lease {
    pub node: String,
    pub expires_ms: u64,
    /// Increases each time the lease changes hands.
    pub term: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn ha_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("ha")
}

/// The lease as stored, if any.
pub fn read_lease(data_dir: &Path) -> Option<Lease> {
    std::fs::read_to_string(ha_dir(data_dir).join("lease.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

/// One acquire-or-renew attempt. Returns whether this node holds the lease afterwards.
pub fn try_hold(data_dir: &Path, cfg: &HaConfig, now: u64) -> std::io::Result<bool> {
    let dir = ha_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let lock = dir.join("lock");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
    {
        Ok(mut f) => {
            use std::io::Write;
            let _ = writeln!(f, "{} {now}", cfg.node);
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Another node is changing the lease, or died doing it.
            let age = std::fs::metadata(&lock)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            if age > cfg.lease_ms {
                let _ = std::fs::remove_file(&lock);
            }
            return Ok(
                read_lease(data_dir).is_some_and(|l| l.node == cfg.node && l.expires_ms > now)
            );
        }
        Err(e) => return Err(e),
    }
    let result = (|| {
        let cur = read_lease(data_dir);
        let held = match &cur {
            None => true,
            Some(l) if l.node == cfg.node => true,
            Some(l) => l.expires_ms <= now,
        };
        if held {
            let term = match &cur {
                Some(l) if l.node == cfg.node => l.term,
                Some(l) => l.term + 1,
                None => 1,
            };
            let lease = Lease {
                node: cfg.node.clone(),
                expires_ms: now + cfg.lease_ms,
                term,
            };
            let body = serde_json::to_string(&lease).map_err(std::io::Error::other)?;
            super::jobs::write_atomic(&dir.join("lease.json"), body.as_bytes())?;
        }
        Ok(held)
    })();
    let _ = std::fs::remove_file(&lock);
    result
}

/// Give the lease up (clean stop), when this node holds it.
pub fn release(data_dir: &Path, cfg: &HaConfig) {
    if read_lease(data_dir).is_some_and(|l| l.node == cfg.node) {
        let _ = std::fs::remove_file(ha_dir(data_dir).join("lease.json"));
    }
}

/// The lease keeper of one node.
pub struct HaNode {
    stop: Arc<AtomicBool>,
    release_on_stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl HaNode {
    /// Start keeping the lease for `jobs` (which must start inactive).
    pub fn start(cfg: HaConfig, jobs: Arc<JobService>) -> HaNode {
        let stop = Arc::new(AtomicBool::new(false));
        let release_on_stop = Arc::new(AtomicBool::new(true));
        let (st, rel) = (stop.clone(), release_on_stop.clone());
        let thread = std::thread::Builder::new()
            .name("kshana-ha".into())
            .spawn(move || {
                let period = Duration::from_millis((cfg.lease_ms / 3).max(50));
                while !st.load(Ordering::SeqCst) {
                    let held = try_hold(&jobs.data_dir, &cfg, now_ms()).unwrap_or(false);
                    let was = jobs.active.swap(held, Ordering::SeqCst);
                    if held {
                        let n = jobs.recover();
                        if !was {
                            eprintln!(
                                "[ha] {} holds the lease; {n} unfinished job(s) queued",
                                cfg.node
                            );
                        }
                    } else if was {
                        eprintln!("[ha] {} lost the lease; standing by", cfg.node);
                    }
                    let until = std::time::Instant::now() + period;
                    while std::time::Instant::now() < until && !st.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                jobs.active.store(false, Ordering::SeqCst);
                if rel.load(Ordering::SeqCst) {
                    release(&jobs.data_dir, &cfg);
                }
            })
            .expect("spawn the lease keeper");
        HaNode {
            stop,
            release_on_stop,
            thread: Some(thread),
        }
    }

    /// Stop; give the lease up when `release` (a clean stop), or keep it to expire (as a
    /// node that died would).
    pub fn stop(mut self, release: bool) {
        self.release_on_stop.store(release, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for HaNode {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_holder_at_a_time_and_takeover_after_expiry() {
        let d = std::env::temp_dir().join(format!("kshana-pro-ha-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let a = HaConfig::new("node-a", 1_000).unwrap();
        let b = HaConfig::new("node-b", 1_000).unwrap();
        assert!(try_hold(&d, &a, 10_000).unwrap());
        assert!(!try_hold(&d, &b, 10_500).unwrap());
        assert!(try_hold(&d, &a, 10_600).unwrap()); // renew: expires 11 600
        assert!(!try_hold(&d, &b, 11_599).unwrap());
        assert!(try_hold(&d, &b, 11_600).unwrap()); // expired: taken over
        let l = read_lease(&d).unwrap();
        assert_eq!((l.node.as_str(), l.term), ("node-b", 2));
        assert!(!try_hold(&d, &a, 11_700).unwrap());
        release(&d, &a); // not the holder: nothing happens
        assert!(read_lease(&d).is_some());
        release(&d, &b);
        assert!(read_lease(&d).is_none());
        assert!(HaConfig::new("bad name", 1_000).is_err());
        assert!(HaConfig::new("n", 10).is_err());
    }
}
