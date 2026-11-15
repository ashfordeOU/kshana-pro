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
//!
//! **Trial tokens (roadmap S6).** When the server has a licence signer
//! (`ServeConfig::trial_signer`), `POST /trial/token` hands out a **14-day** Pro trial token
//! (`trial: true`; products `cli` and `desktop`; one seat; optionally bound to the machine
//! hash in the body). Builds refuse a token marked `trial` that lasts longer than
//! [`crate::license::TRIAL_TOKEN_MAX_DAYS`], so a longer one cannot be minted even by
//! mistake. Limits: one token per caller per `token_cooldown_days` (default 30) and at most
//! `tokens_per_day` server-wide; past them, 429. No name, e-mail or address is asked for or
//! kept: the token's licensee is the caller's trial tenant, and the server keeps only that
//! salted hash with the time of issue.
//!
//! **No customer data kept (S6).** Trial jobs (their request, result and artefacts) are
//! deleted `retention_secs` (default one hour) after they finish, and any trial job still
//! unfinished a day after submission; the daily job count lives in `trial-usage.json` (salted
//! tenant hashes and counts only), so purging a job does not refund the quota.

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
    /// Days a trial token lasts (at most [`crate::license::TRIAL_TOKEN_MAX_DAYS`]).
    pub token_days: i64,
    /// Trial tokens per UTC day, server-wide.
    pub tokens_per_day: u32,
    /// Days before the same caller may have another trial token.
    pub token_cooldown_days: i64,
    /// Seconds a finished trial job is kept before it is deleted.
    pub retention_secs: i64,
}

impl Default for TrialConfig {
    fn default() -> Self {
        TrialConfig {
            jobs_per_day: 20,
            max_run_secs: 60,
            requests_per_minute: 120,
            max_queued: 100,
            token_days: crate::license::TRIAL_TOKEN_MAX_DAYS,
            tokens_per_day: 200,
            token_cooldown_days: 30,
            retention_secs: 3600,
        }
    }
}

/// A fixed-window per-minute request limiter, keyed by any string (an address for the
/// trial, a tenant for the hosted service, [`super::hosted`]).
pub struct RateLimiter {
    pub per_minute: u32,
    window: Mutex<(i64, HashMap<String, u32>)>,
}

impl RateLimiter {
    pub fn new(per_minute: u32) -> RateLimiter {
        RateLimiter {
            per_minute,
            window: Mutex::new((i64::MIN, HashMap::new())),
        }
    }

    /// Count one request for `key` at `now`; `Err(seconds to wait)` past the limit.
    pub fn admit(&self, key: &str, now: i64) -> Result<(), i64> {
        let minute = now.div_euclid(60);
        let mut w = self.window.lock().unwrap_or_else(|p| p.into_inner());
        if w.0 != minute {
            *w = (minute, HashMap::new());
        }
        let n = w.1.entry(key.to_string()).or_insert(0);
        *n += 1;
        if *n > self.per_minute {
            return Err(60 - now.rem_euclid(60));
        }
        Ok(())
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Usage {
    /// UTC day index.
    day: i64,
    /// New jobs today per trial tenant.
    jobs: std::collections::BTreeMap<String, u32>,
    /// Tokens issued today, server-wide.
    tokens_today: u32,
    /// Last token time per trial tenant (pruned after the cooldown).
    last_token: std::collections::BTreeMap<String, i64>,
}

/// The trial's state.
pub struct Trial {
    pub config: TrialConfig,
    salt: String,
    limiter: RateLimiter,
    usage_path: std::path::PathBuf,
    usage: Mutex<Usage>,
    last_purge: Mutex<i64>,
    /// Signs trial tokens (S6); `None`: `POST /trial/token` is closed.
    pub signer: Option<crate::license::signer::SharedSigner>,
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
        let usage_path = data_dir.join("trial-usage.json");
        let usage = std::fs::read_to_string(&usage_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Ok(Trial {
            limiter: RateLimiter::new(config.requests_per_minute),
            config,
            salt,
            usage_path,
            usage: Mutex::new(usage),
            last_purge: Mutex::new(i64::MIN),
            signer: None,
        })
    }

    fn with_usage<T>(&self, now: i64, f: impl FnOnce(&mut Usage) -> T) -> std::io::Result<T> {
        let mut u = self.usage.lock().unwrap_or_else(|p| p.into_inner());
        let day = now.div_euclid(86_400);
        if u.day != day {
            u.day = day;
            u.jobs.clear();
            u.tokens_today = 0;
        }
        let cooldown = self.config.token_cooldown_days * 86_400;
        u.last_token.retain(|_, t| *t > now - cooldown);
        let out = f(&mut u);
        let text = serde_json::to_vec(&*u).map_err(std::io::Error::other)?;
        super::jobs::write_atomic(&self.usage_path, &text)?;
        Ok(out)
    }

    /// Count one new job for `tenant` (after it was created).
    pub fn record_job(&self, tenant: &str, now: i64) -> std::io::Result<()> {
        self.with_usage(now, |u| *u.jobs.entry(tenant.to_string()).or_insert(0) += 1)
    }

    /// Delete trial jobs finished more than `retention_secs` ago, and trial jobs still
    /// unfinished a day after submission (S6: no customer data kept). Runs at most once a
    /// minute; returns how many were deleted.
    pub fn purge(&self, jobs: &super::worker::JobService, now: i64) -> usize {
        {
            let mut last = self.last_purge.lock().unwrap_or_else(|p| p.into_inner());
            if *last != i64::MIN && now - *last < 60 {
                return 0;
            }
            *last = now;
        }
        self.purge_now(jobs, now)
    }

    /// [`Trial::purge`] without the once-a-minute limit.
    pub fn purge_now(&self, jobs: &super::worker::JobService, now: i64) -> usize {
        let mut n = 0;
        for s in jobs.store.list() {
            if !s.tenant.starts_with(PREFIX) {
                continue;
            }
            let expired = match s.finished_unix {
                Some(f) => s.state.is_finished() && now - f >= self.config.retention_secs,
                None => now - s.submitted_unix >= 86_400,
            };
            if expired && jobs.store.remove(&s.job_id).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// `POST /trial/token`: a 14-day Pro trial token for the caller at `ip` (S6).
    pub fn issue_token(&self, ip: IpAddr, body: &str, now: i64) -> HttpResponse {
        let Some(signer) = &self.signer else {
            return HttpResponse::error(404, "this trial does not hand out trial tokens");
        };
        let req: serde_json::Value = if body.trim().is_empty() {
            serde_json::json!({})
        } else {
            match serde_json::from_str(body) {
                Ok(v) => v,
                Err(_) => return HttpResponse::error(400, "body must be JSON"),
            }
        };
        let machine = match req.get("machine") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(m))
                if m.len() == 64 && m.bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                Some(m.to_ascii_lowercase())
            }
            Some(_) => {
                return HttpResponse::error(
                    422,
                    "machine must be the 64-hex machine hash (`kshana-pro license machine-id`)",
                )
            }
        };
        if let Some(extra) = req
            .as_object()
            .and_then(|o| o.keys().find(|k| k.as_str() != "machine"))
        {
            return HttpResponse::error(
                422,
                &format!("unknown field '{extra}': the trial asks for nothing but an optional machine hash"),
            );
        }
        let tenant = self.tenant_for(ip);
        let cfg = &self.config;
        let check = self.with_usage(now, |u| {
            if let Some(t) = u.last_token.get(&tenant) {
                return Err(too_many(
                    &format!(
                        "trial limit: one trial token per caller every {} days",
                        cfg.token_cooldown_days
                    ),
                    t + cfg.token_cooldown_days * 86_400 - now,
                ));
            }
            if u.tokens_today >= cfg.tokens_per_day {
                return Err(too_many(
                    "trial limit: today's trial tokens are all given out; try again tomorrow",
                    (now.div_euclid(86_400) + 1) * 86_400 - now,
                ));
            }
            Ok(())
        });
        match check {
            Ok(Ok(())) => {}
            Ok(Err(r)) => return r,
            Err(e) => return HttpResponse::error(503, &format!("trial store: {e}")),
        }
        let days = cfg
            .token_days
            .clamp(1, crate::license::TRIAL_TOKEN_MAX_DAYS);
        let claims = crate::license::LicenseClaims {
            licensee: format!("Kshana Pro trial {tenant}"),
            tier: crate::license::Tier::Pro.as_str().into(),
            expires_unix: now + days * 86_400,
            products: vec!["cli".into(), "desktop".into()],
            seats: 1,
            machine,
            issued_unix: now,
            trial: true,
            ..Default::default()
        };
        let token = match crate::license::issue_v2(signer.as_ref(), &claims) {
            Ok(t) => t,
            Err(e) => return HttpResponse::error(503, &format!("could not sign: {e}")),
        };
        if let Err(e) = self.with_usage(now, |u| {
            u.tokens_today += 1;
            u.last_token.insert(tenant.clone(), now);
        }) {
            return HttpResponse::error(503, &format!("trial store: {e}"));
        }
        HttpResponse::json(
            201,
            serde_json::json!({
                "token": token,
                "tier": "pro",
                "trial": true,
                "expires_unix": claims.expires_unix,
                "days": days,
                "products": claims.products,
                "activate": "kshana-pro license activate <token>",
            })
            .to_string(),
        )
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
        // Keyed by the salted tenant, so no address is held even in memory longer than
        // the request.
        self.limiter
            .admit(&self.tenant_for(ip), now)
            .map_err(|wait| {
                too_many(
                    &format!(
                        "trial limit: {} requests a minute from one address",
                        self.config.requests_per_minute
                    ),
                    wait,
                )
            })
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
        let today = self
            .with_usage(now, |u| u.jobs.get(tenant).copied().unwrap_or(0))
            .unwrap_or(u32::MAX);
        if today >= self.config.jobs_per_day {
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

pub(crate) fn too_many(msg: &str, retry_after: i64) -> HttpResponse {
    let mut r = HttpResponse::error(429, msg);
    r.headers
        .push(("Retry-After", retry_after.max(1).to_string()));
    r
}
