// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The hosted trial sandbox (roadmap F3b)
//!
//! `kshana-pro serve --trial --data-dir <dir>` opens the job service to anyone, with no
//! credential, at the **Evaluation** tier (open scenario kinds and the free seam check
//! only, whatever licence the server holds), under quotas:
//!
//! * each caller is a tenant of its own, `trial-` and 16 hexadecimal characters of a
//!   salted SHA-256 of its address, so a caller sees only its own jobs and no address is
//!   stored or shown;
//! * at most `jobs_per_day` new jobs per caller per UTC day, counted from the job store
//!   (so a restart does not reset them); a resubmitted identical request is the same job
//!   and is not counted again;
//! * every job runs under a time limit of at most `max_run_secs`;
//! * at most `requests_per_minute` requests per address per minute;
//! * at most `max_queued` trial jobs waiting at once, server-wide;
//! * synchronous runs (`POST /run`), study requests and every admin route are closed.
//!
//! Answers past a quota are 429 with `Retry-After`. The salt is `<data-dir>/trial-salt`,
//! made once at random.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use super::HttpResponse;

/// The trial's limits.
#[derive(Debug, Clone)]
pub struct TrialConfig {
    pub jobs_per_day: u32,
    pub max_run_secs: u64,
    pub requests_per_minute: u32,
    pub max_queued: usize,
}

impl Default for TrialConfig {
    fn default() -> Self {
        TrialConfig {
            jobs_per_day: 20,
            max_run_secs: 60,
            requests_per_minute: 120,
            max_queued: 100,
        }
    }
}

/// The trial's state.
pub struct Trial {
    pub config: TrialConfig,
    salt: String,
    window: Mutex<(i64, HashMap<IpAddr, u32>)>,
}

/// The tenant-name prefix of a trial caller.
pub const PREFIX: &str = "trial-";

impl Trial {
    /// Open the trial for `data_dir` (creating the salt on first use).
    pub fn open(data_dir: &Path, config: TrialConfig) -> std::io::Result<Trial> {
        std::fs::create_dir_all(data_dir)?;
        let p = data_dir.join("trial-salt");
        let salt = match std::fs::read_to_string(&p) {
            Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => {
                use rand::RngCore;
                let mut b = [0u8; 32];
                rand::rngs::OsRng.fill_bytes(&mut b);
                let s = hex::encode(b);
                super::jobs::write_atomic(&p, s.as_bytes())?;
                s
            }
        };
        Ok(Trial {
            config,
            salt,
            window: Mutex::new((i64::MIN, HashMap::new())),
        })
    }

    /// The tenant of the caller at `ip`.
    pub fn tenant_for(&self, ip: IpAddr) -> String {
        let mut h = Sha256::new();
        h.update(self.salt.as_bytes());
        h.update(ip.to_string().as_bytes());
        format!("{PREFIX}{}", &hex::encode(h.finalize())[..16])
    }

    /// Count one request from `ip` against the per-minute limit.
    pub fn admit(&self, ip: IpAddr, now: i64) -> Result<(), HttpResponse> {
        let minute = now.div_euclid(60);
        let mut w = self.window.lock().unwrap_or_else(|p| p.into_inner());
        if w.0 != minute {
            *w = (minute, HashMap::new());
        }
        let n = w.1.entry(ip).or_insert(0);
        *n += 1;
        if *n > self.config.requests_per_minute {
            return Err(too_many(
                &format!(
                    "trial limit: {} requests a minute from one address",
                    self.config.requests_per_minute
                ),
                60 - now.rem_euclid(60),
            ));
        }
        Ok(())
    }

    /// Whether `tenant` may submit one more new job at `now`.
    pub fn may_submit(
        &self,
        jobs: &super::worker::JobService,
        tenant: &str,
        now: i64,
    ) -> Result<(), HttpResponse> {
        let day_start = now.div_euclid(86_400) * 86_400;
        let all = jobs.store.list();
        let today = all
            .iter()
            .filter(|s| s.tenant == tenant && s.submitted_unix >= day_start)
            .count();
        if today >= self.config.jobs_per_day as usize {
            return Err(too_many(
                &format!(
                    "trial limit: {} jobs a day; the count starts again at 00:00 UTC",
                    self.config.jobs_per_day
                ),
                day_start + 86_400 - now,
            ));
        }
        let queued = all
            .iter()
            .filter(|s| s.tenant.starts_with(PREFIX) && !s.state.is_finished())
            .count();
        if queued >= self.config.max_queued {
            let mut r = HttpResponse::error(503, "the trial is busy; try again in a minute");
            r.headers.push(("Retry-After", "60".into()));
            return Err(r);
        }
        Ok(())
    }
}

fn too_many(msg: &str, retry_after: i64) -> HttpResponse {
    let mut r = HttpResponse::error(429, msg);
    r.headers
        .push(("Retry-After", retry_after.max(1).to_string()));
    r
}
