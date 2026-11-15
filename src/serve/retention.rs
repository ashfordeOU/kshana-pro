// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Retention policies (roadmap 0.42 F8, Enterprise)
//!
//! `<data-dir>/retention.toml` says how long finished jobs and audit days are kept:
//!
//! ```toml
//! [default]
//! jobs_days = 365        # finished jobs (succeeded, failed, cancelled), from their finish time
//! audit_days = 730       # whole audit day files
//! [tenant.site-a]
//! jobs_days = 90         # a tenant's own period, shorter or longer
//! ```
//!
//! A key left out keeps everything of that kind. Two things are never deleted by a policy:
//! a job recorded as **delivered** in the delivery ledger (deleting it would break
//! `GET /ledger/verify`, which re-hashes every delivered job), and an unfinished job.
//! Expired or revoked share links are removed. `GET /retention` shows the policy and what
//! applying it now would delete (the caller's tenant; every tenant for an operator);
//! `POST /retention/apply` (an operator) deletes it, and every deletion of audit days is
//! itself an audit line ([`super::audit::AuditLog::prune`]). Enterprise feature
//! `pro-retention`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::worker::JobService;

/// The Enterprise feature.
pub const FEATURE: &str = "pro-retention";
/// The policy file, in the data directory.
pub const FILE: &str = "retention.toml";

/// One period set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Periods {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_days: Option<u32>,
}

/// `retention.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(default)]
    pub default: Periods,
    #[serde(default)]
    pub tenant: BTreeMap<String, Periods>,
}

impl Policy {
    /// Parse and check a policy (periods of 1 to 36 500 days; tenant names are names).
    pub fn parse(text: &str) -> Result<Policy, String> {
        let p: Policy = toml::from_str(text).map_err(|e| format!("{FILE}: {e}"))?;
        let ok = |d: Option<u32>| d.is_none_or(|d| (1..=36_500).contains(&d));
        if !ok(p.default.jobs_days) || !ok(p.default.audit_days) {
            return Err(format!("{FILE}: periods are 1 to 36500 days"));
        }
        for (t, per) in &p.tenant {
            if !super::auth::is_name(t) {
                return Err(format!("{FILE}: '{t}' is not a tenant name"));
            }
            if per.audit_days.is_some() {
                return Err(format!(
                    "{FILE}: audit_days is set once, under [default] (one audit log for the server)"
                ));
            }
            if !ok(per.jobs_days) {
                return Err(format!("{FILE}: periods are 1 to 36500 days"));
            }
        }
        Ok(p)
    }

    /// The policy of `data_dir`, or `None` without a file.
    pub fn load(data_dir: &Path) -> Result<Option<Policy>, String> {
        match std::fs::read_to_string(data_dir.join(FILE)) {
            Ok(t) => Policy::parse(&t).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{FILE}: {e}")),
        }
    }

    /// The job period of `tenant`.
    pub fn jobs_days(&self, tenant: &str) -> Option<u32> {
        self.tenant
            .get(tenant)
            .and_then(|p| p.jobs_days)
            .or(self.default.jobs_days)
    }
}

/// What applying a policy deletes (or would delete).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Report {
    pub applied: bool,
    /// Job ids deleted (or due), with their tenant.
    pub jobs: Vec<(String, String)>,
    /// Jobs past their period kept because the delivery ledger records them.
    pub kept_delivered: usize,
    pub audit_days: usize,
    pub shares: usize,
}

fn delivered(j: &JobService) -> std::collections::BTreeSet<String> {
    j.ledger
        .read_json()
        .ok()
        .and_then(|s| crate::study_ledger::StudyLedger::from_json(&s).ok())
        .map(|l| {
            l.records()
                .iter()
                .filter(|r| r.delivered_utc.is_some())
                .map(|r| r.study_id.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Apply `policy` (or, with `apply` false, report what it would do) for the tenants
/// `sees` accepts. Audit days are pruned only when applying for every tenant (`all`).
pub fn run(
    j: &JobService,
    audit: Option<&super::audit::AuditLog>,
    policy: &Policy,
    sees: &dyn Fn(&str) -> bool,
    all: bool,
    apply: bool,
    now: i64,
) -> std::io::Result<Report> {
    let mut r = Report {
        applied: apply,
        ..Default::default()
    };
    let kept = delivered(j);
    for s in j.store.list() {
        if !sees(&s.tenant) || !s.state.is_finished() {
            continue;
        }
        let (Some(days), Some(fin)) = (policy.jobs_days(&s.tenant), s.finished_unix) else {
            continue;
        };
        if now - fin < i64::from(days) * 86_400 {
            continue;
        }
        if kept.contains(&s.job_id) {
            r.kept_delivered += 1;
            continue;
        }
        if apply {
            j.store.remove(&s.job_id)?;
        }
        r.jobs.push((s.tenant.clone(), s.job_id.clone()));
    }
    let shares = j.data_dir.join("shares");
    for e in std::fs::read_dir(&shares).into_iter().flatten().flatten() {
        let Ok(b) = std::fs::read(e.path()) else {
            continue;
        };
        let Ok(sh) = serde_json::from_slice::<super::library::Share>(&b) else {
            continue;
        };
        if !sees(&sh.tenant) || !(sh.revoked || now >= sh.expires_unix) {
            continue;
        }
        if apply {
            std::fs::remove_file(e.path())?;
        }
        r.shares += 1;
    }
    if let (Some(days), Some(log), true) = (policy.default.audit_days, audit, all) {
        if apply {
            r.audit_days = log.prune(days, now)?;
        } else {
            let cutoff = now.div_euclid(86_400) - i64::from(days);
            r.audit_days = super::audit::day_files_before(log.dir(), cutoff);
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_parse_and_bad_ones_are_refused() {
        let p = Policy::parse(
            "[default]\njobs_days = 365\naudit_days = 730\n[tenant.site-a]\njobs_days = 90\n",
        )
        .unwrap();
        assert_eq!(p.jobs_days("site-a"), Some(90));
        assert_eq!(p.jobs_days("site-b"), Some(365));
        for bad in [
            "[default]\njobs_days = 0\n",
            "[default]\nkeep = 3\n",
            "[tenant.\"a b\"]\njobs_days = 3\n",
            "[tenant.x]\naudit_days = 3\n",
        ] {
            assert!(Policy::parse(bad).is_err(), "{bad}");
        }
    }
}
