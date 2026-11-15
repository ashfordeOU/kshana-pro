// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Hosted Kshana Pro service for secret methods (roadmap 0.42 C3)
//!
//! Commercial plan C3: methods that are patented, or acceptable to be studied, run on the
//! customer's device; trade-secret methods run only here, results-only. `kshana-pro serve
//! --hosted <hosted.toml> --data-dir <dir> --keys-file <keys>` adds two routes to the job
//! service:
//!
//! * `GET /v1/verdict`: the results-only kinds this server offers, and the caller's
//!   allowance left this month;
//! * `POST /v1/verdict/<kind>` with `{"input": {...}}`: inputs in, a signed verdict out
//!   ([`crate::results_only`], X6): verdict, evidence class, input hash, query ID, the
//!   licence holder's watermark (C4) and the signature. Intermediates never leave.
//!
//! Each subscription is a tenant of the server's API keys or OIDC (an anonymous caller gets
//! 401 before reaching here) and has a **monthly allowance** (queries and compute seconds)
//! in `hosted.toml`; a tenant with no subscription line gets 403. The per-minute limit is
//! the hosted trial's limiter ([`super::trial::RateLimiter`], S6), keyed by tenant. The
//! server's licence must allow each kind ([`crate::license::check`]); so on a Kshana Pro
//! server only Pro kinds can be offered, and an Arksa-exclusive kind only where the
//! server holds Arksa's entitlement.
//!
//! `hosted.toml`:
//!
//! ```toml
//! requests_per_minute = 60          # per tenant
//! probe_window_secs = 3600
//! probe_alert_after = 10
//! probe_block_after = 25
//! [default]                         # allowance for every subscription unless overridden
//! queries_per_month = 1000
//! compute_secs_per_month = 3600
//! [[subscription]]
//! tenant = "acme"                   # the API key's tenant
//! licensee = "Acme Ltd"             # for the watermark; never shown in results
//! queries_per_month = 500           # optional override
//! active_until_unix = 1830297600    # optional; 0 or absent: no end
//! ```
//!
//! The signing key (`KSHANA_PRO_SIGNER`, a seed or AWS KMS) and the watermark key
//! (`KSHANA_PRO_WATERMARK_KEY`) come from the environment; neither is in the file.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::auth::{Principal, Refusal as AuthRefusal, Role};
use super::trial::RateLimiter;
use super::{HttpRequest, HttpResponse};
use crate::license::signer::SharedSigner;
use crate::results_only::{self, Allowance, Gateway, GatewayConfig, Query};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowanceToml {
    queries_per_month: Option<u32>,
    compute_secs_per_month: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubscriptionToml {
    tenant: String,
    licensee: String,
    #[serde(default)]
    queries_per_month: Option<u32>,
    #[serde(default)]
    compute_secs_per_month: Option<u64>,
    #[serde(default)]
    active_until_unix: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostedToml {
    #[serde(default = "default_rpm")]
    requests_per_minute: u32,
    #[serde(default)]
    probe_window_secs: Option<i64>,
    #[serde(default)]
    probe_alert_after: Option<u32>,
    #[serde(default)]
    probe_block_after: Option<u32>,
    #[serde(default)]
    default: AllowanceToml,
    #[serde(default)]
    subscription: Vec<SubscriptionToml>,
}

fn default_rpm() -> u32 {
    60
}

/// One subscription.
#[derive(Debug, Clone)]
pub struct Subscription {
    pub licensee: String,
    pub allowance: Allowance,
    pub active_until_unix: i64,
}

/// The parsed `hosted.toml`.
#[derive(Debug, Clone)]
pub struct HostedConfig {
    pub requests_per_minute: u32,
    pub gateway: GatewayConfig,
    pub subscriptions: BTreeMap<String, Subscription>,
}

impl HostedConfig {
    /// Parse `hosted.toml` text.
    pub fn parse(text: &str) -> Result<HostedConfig, String> {
        let t: HostedToml = toml::from_str(text).map_err(|e| format!("hosted.toml: {e}"))?;
        let mut g = GatewayConfig::default();
        if let Some(q) = t.default.queries_per_month {
            g.queries_per_period = q;
        }
        if let Some(c) = t.default.compute_secs_per_month {
            g.compute_ms_per_period = c.saturating_mul(1000);
        }
        if let Some(w) = t.probe_window_secs {
            g.probe_window_secs = w.max(1);
        }
        if let Some(a) = t.probe_alert_after {
            g.probe_alert_after = a;
        }
        if let Some(b) = t.probe_block_after {
            g.probe_block_after = b;
        }
        let mut subs = BTreeMap::new();
        for s in t.subscription {
            let tenant = s.tenant.trim().to_string();
            if tenant.is_empty() || s.licensee.trim().is_empty() {
                return Err("hosted.toml: every subscription needs a tenant and a licensee".into());
            }
            if subs.contains_key(&tenant) {
                return Err(format!("hosted.toml: tenant '{tenant}' listed twice"));
            }
            subs.insert(
                tenant,
                Subscription {
                    licensee: s.licensee.trim().to_string(),
                    allowance: Allowance {
                        queries_per_period: s.queries_per_month,
                        compute_ms_per_period: s
                            .compute_secs_per_month
                            .map(|c| c.saturating_mul(1000)),
                    },
                    active_until_unix: s.active_until_unix,
                },
            );
        }
        Ok(HostedConfig {
            requests_per_minute: t.requests_per_minute.max(1),
            gateway: g,
            subscriptions: subs,
        })
    }

    /// Read and parse a `hosted.toml` file.
    pub fn from_file(p: &Path) -> Result<HostedConfig, String> {
        let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        HostedConfig::parse(&t)
    }
}

/// The hosted service's state.
pub struct Hosted {
    pub config: HostedConfig,
    gateway: Gateway,
    limiter: RateLimiter,
}

impl Hosted {
    /// Open the service: the gateway keeps its usage and access log in `<data_dir>/hosted`.
    pub fn open(
        data_dir: &Path,
        config: HostedConfig,
        signer: SharedSigner,
        watermark_key: [u8; 32],
    ) -> Result<Hosted, String> {
        let gateway = Gateway::new(
            config.gateway.clone(),
            signer,
            watermark_key,
            Some(&data_dir.join("hosted")),
        )
        .map_err(|e| e.to_string())?
        .with_alert_sink(Box::new(|a| {
            eprintln!(
                "ALERT probing: tenant {} kind {} {} near-identical queries{}",
                a.tenant,
                a.kind,
                a.count,
                if a.blocked { " (blocked)" } else { "" }
            )
        }));
        Ok(Hosted {
            limiter: RateLimiter::new(config.requests_per_minute),
            config,
            gateway,
        })
    }

    /// The gateway (tests read its access log path).
    pub fn gateway(&self) -> &Gateway {
        &self.gateway
    }

    /// Route `GET /v1/verdict` and `POST /v1/verdict/<kind>`.
    pub fn dispatch(
        &self,
        svc: &super::api::Service,
        req: &HttpRequest,
        path: &str,
        principal: &Result<Principal, AuthRefusal>,
        now: i64,
    ) -> HttpResponse {
        let Ok(who) = principal else {
            let mut r = HttpResponse::error(
                401,
                "authorization required: send Authorization: Bearer <API key or OIDC token>",
            );
            r.headers
                .push(("WWW-Authenticate", "Bearer realm=\"kshana-pro\"".into()));
            return r;
        };
        if let Err(wait) = self.limiter.admit(&who.tenant, now) {
            return super::trial::too_many(
                &format!(
                    "limit: {} requests a minute per subscription",
                    self.limiter.per_minute
                ),
                wait,
            );
        }
        let Some(sub) = self.config.subscriptions.get(&who.tenant) else {
            return HttpResponse::error(403, "this tenant has no hosted subscription");
        };
        if sub.active_until_unix != 0 && sub.active_until_unix <= now {
            return HttpResponse::error(403, "this hosted subscription has ended");
        }
        match (req.method.as_str(), path) {
            ("GET", "/v1/verdict") => {
                let kinds: Vec<String> = results_only::registered_kinds()
                    .into_iter()
                    .filter(|k| crate::license::check(&svc.entitlement, k, now).is_ok())
                    .collect();
                HttpResponse::json(
                    200,
                    serde_json::json!({
                        "kinds": kinds,
                        "allowance": {
                            "queries_per_month": sub.allowance.queries_per_period
                                .unwrap_or(self.config.gateway.queries_per_period),
                            "compute_secs_per_month": sub.allowance.compute_ms_per_period
                                .unwrap_or(self.config.gateway.compute_ms_per_period) / 1000,
                        },
                    })
                    .to_string(),
                )
            }
            ("POST", p) if p.starts_with("/v1/verdict/") => {
                if who.role < Role::Submitter {
                    return HttpResponse::error(403, "this route needs the submitter role");
                }
                let kind = &p["/v1/verdict/".len()..];
                if crate::license::check(&svc.entitlement, kind, now).is_err()
                    || !results_only::registered_kinds().iter().any(|k| k == kind)
                {
                    // Same answer whether the kind is unknown or not licensed here.
                    return HttpResponse::error(404, "no such results-only kind");
                }
                let body: serde_json::Value = match serde_json::from_str(&req.body) {
                    Ok(v) => v,
                    Err(_) => {
                        return HttpResponse::error(400, "body must be JSON {\"input\": {...}}")
                    }
                };
                let Some(input) = body.get("input") else {
                    return HttpResponse::error(422, "input is required");
                };
                let q = Query {
                    tenant: &who.tenant,
                    licensee: &sub.licensee,
                    kind,
                    input,
                    allowance: Some(&sub.allowance),
                };
                match self.gateway.query(&q, now) {
                    Ok(signed) => {
                        let env = signed
                            .split_once('.')
                            .and_then(|(p, _)| hex::decode(p).ok())
                            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                            .unwrap_or_default();
                        HttpResponse::json(
                            200,
                            serde_json::json!({
                                "verdict": env["verdict"],
                                "evidence_class": env["evidence_class"],
                                "query_id": env["query_id"],
                                "input_sha256": env["input_sha256"],
                                "signed": signed,
                            })
                            .to_string(),
                        )
                    }
                    Err(f) => {
                        let mut r = HttpResponse::error(f.status(), &f.message());
                        if let Some(s) = f.retry_after() {
                            r.headers.push(("Retry-After", s.max(1).to_string()));
                        }
                        r
                    }
                }
            }
            _ => HttpResponse::error(404, "not found"),
        }
    }
}
