// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Kshana Station fleet (roadmap L3, Pro part; F10e station count)
//!
//! One Kshana Station is open. Managing **more than one** from one place is the Pro
//! feature `pro-station-fleet` (the 0.41 edition matrix). The Pro service keeps the fleet:
//! each Station registers (`POST /fleet/stations`) and reports its health
//! (`POST /fleet/stations/<id>/heartbeat`); `GET /fleet/stations` lists them with their
//! last report and whether they are live or stale.
//!
//! The licence sets how many Stations may be registered at once: the token's
//! `stations` claim when the licence carries `pro-station-fleet` (directly or through
//! its tier), else one. A registration past the limit is refused with 403 naming the
//! limit; retiring a Station (`POST /fleet/stations/<id>/retire`) frees its place.
//!
//! Stored under `<data-dir>/fleet/<tenant>/<id>.json`; each tenant has its own fleet
//! and the limit counts every tenant's active Stations together (one licence).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A Station is stale when its last heartbeat is older than this.
pub const STALE_AFTER_SECS: i64 = 120;

/// One registered Station.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Station {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
    pub tenant: String,
    pub registered_unix: i64,
    #[serde(default)]
    pub last_seen_unix: Option<i64>,
    #[serde(default)]
    pub health: serde_json::Value,
    #[serde(default)]
    pub retired: bool,
}

fn dir(data_dir: &Path, tenant: &str) -> PathBuf {
    let t = if tenant == super::auth::ALL_TENANTS {
        "_all"
    } else {
        tenant
    };
    data_dir.join("fleet").join(t)
}

fn path(data_dir: &Path, tenant: &str, id: &str) -> PathBuf {
    dir(data_dir, tenant).join(format!("{id}.json"))
}

fn write(data_dir: &Path, s: &Station) -> std::io::Result<()> {
    let p = path(data_dir, &s.tenant, &s.id);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let body = serde_json::to_string_pretty(s).map_err(std::io::Error::other)?;
    super::jobs::write_atomic(&p, body.as_bytes())
}

/// One Station of `tenant`.
pub fn get(data_dir: &Path, tenant: &str, id: &str) -> Option<Station> {
    std::fs::read_to_string(path(data_dir, tenant, id))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

/// Every Station of every tenant.
pub fn all(data_dir: &Path) -> Vec<Station> {
    let mut out = Vec::new();
    if let Ok(tenants) = std::fs::read_dir(data_dir.join("fleet")) {
        for t in tenants.filter_map(Result::ok) {
            if let Ok(files) = std::fs::read_dir(t.path()) {
                for f in files.filter_map(Result::ok) {
                    if let Some(s) = std::fs::read_to_string(f.path())
                        .ok()
                        .and_then(|x| serde_json::from_str::<Station>(&x).ok())
                    {
                        out.push(s);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| (a.tenant.as_str(), a.id.as_str()).cmp(&(b.tenant.as_str(), b.id.as_str())));
    out
}

/// Why a registration was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum Refused {
    /// The licence allows this many active Stations.
    Limit(u32),
    Io(String),
}

/// Register (or re-register, which keeps the place) a Station, within `limit`.
pub fn register(
    data_dir: &Path,
    tenant: &str,
    id: &str,
    name: &str,
    url: Option<String>,
    limit: u32,
    now: i64,
) -> Result<(Station, bool), Refused> {
    if let Some(mut s) = get(data_dir, tenant, id) {
        let created = s.retired;
        if s.retired {
            let active = all(data_dir).iter().filter(|x| !x.retired).count() as u32;
            if active >= limit {
                return Err(Refused::Limit(limit));
            }
        }
        s.retired = false;
        s.name = name.to_string();
        s.url = url;
        write(data_dir, &s).map_err(|e| Refused::Io(e.to_string()))?;
        return Ok((s, created));
    }
    let active = all(data_dir).iter().filter(|x| !x.retired).count() as u32;
    if active >= limit {
        return Err(Refused::Limit(limit));
    }
    let s = Station {
        id: id.to_string(),
        name: name.to_string(),
        url,
        tenant: tenant.to_string(),
        registered_unix: now,
        last_seen_unix: None,
        health: serde_json::Value::Null,
        retired: false,
    };
    write(data_dir, &s).map_err(|e| Refused::Io(e.to_string()))?;
    Ok((s, true))
}

/// Record a heartbeat. `None` when the Station is not registered (or retired).
pub fn heartbeat(
    data_dir: &Path,
    tenant: &str,
    id: &str,
    health: serde_json::Value,
    now: i64,
) -> std::io::Result<Option<Station>> {
    let Some(mut s) = get(data_dir, tenant, id).filter(|s| !s.retired) else {
        return Ok(None);
    };
    s.last_seen_unix = Some(now);
    s.health = health;
    write(data_dir, &s)?;
    Ok(Some(s))
}

/// Retire a Station: it no longer counts against the limit.
pub fn retire(data_dir: &Path, tenant: &str, id: &str) -> std::io::Result<bool> {
    let Some(mut s) = get(data_dir, tenant, id) else {
        return Ok(false);
    };
    s.retired = true;
    write(data_dir, &s)?;
    Ok(true)
}

/// The state shown for a Station.
pub fn state(s: &Station, now: i64) -> &'static str {
    match (s.retired, s.last_seen_unix) {
        (true, _) => "retired",
        (false, None) => "registered",
        (false, Some(t)) if now - t <= STALE_AFTER_SECS => "live",
        _ => "stale",
    }
}

/// The number of Stations a licence allows: its `stations` claim when it carries the
/// fleet feature (at least one), else one.
pub fn limit_for(ent: &crate::license::Entitlement, stations_claim: u32, now: i64) -> u32 {
    if ent.allows("pro-station-fleet", now) {
        stations_claim.max(1)
    } else {
        1
    }
}
