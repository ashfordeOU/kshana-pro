// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The route table and the job routes
//!
//! [`ROUTES`] is the single list of every route the server answers. The router
//! ([`match_route`]) and the OpenAPI document ([`super::openapi`]) are both generated
//! from it, so the two cannot drift. Routes marked `needs_data_dir` are served only
//! when the server was started with a data directory; without one, the server answers
//! the routes it answered before the job service existed, with the same behaviour, plus
//! `GET /openapi.json`, which documents only those routes.
//!
//! Access token: when the server has one (`--api-token-file`), every request except
//! `GET /health` must carry `Authorization: Bearer <token>`. Only the token's SHA-256
//! is kept in memory; the comparison runs in constant time over the digests. The token
//! is never logged and never appears in a response.

use std::sync::Arc;

use super::audit::AuditLog;
use super::auth::{Authenticator, Principal, Refusal, Role};
use super::jobs::{is_job_id, is_safe_name, JobStatus, Priority, SubmitOptions};
use super::ops::{self, OpError};
use super::worker::JobService;
use super::{HttpRequest, HttpResponse};
use crate::license::Entitlement;

/// Identifies a route's handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteId {
    Health,
    Version,
    Kinds,
    Run,
    Study,
    Board,
    OpenApi,
    JobsSubmit,
    JobsList,
    JobStatus,
    JobResult,
    JobArtefact,
    JobCancel,
    JobEvents,
    Ledger,
    LedgerVerify,
    AuditVerify,
    Metrics,
}

/// One route: the method, the path template (OpenAPI form, `{param}` for a segment)
/// and what the OpenAPI document says about it.
#[derive(Debug)]
pub struct Route {
    pub method: &'static str,
    pub path: &'static str,
    pub id: RouteId,
    /// The least role that may call it; `None`: anyone, without a credential (`/health`).
    pub role: Option<Role>,
    pub summary: &'static str,
    /// Served only when the server has a data directory.
    pub needs_data_dir: bool,
    /// The last template parameter takes the rest of the path (so a name containing a
    /// slash reaches the handler and is refused there with a clear error).
    pub greedy_tail: bool,
    /// Request body: (media type, description).
    pub request: Option<(&'static str, &'static str)>,
    /// Query parameters: (name, description).
    pub query: &'static [(&'static str, &'static str)],
    /// Responses: (status, media type, description).
    pub responses: &'static [(u16, &'static str, &'static str)],
}

const JSON: &str = "application/json";
const TEXT: &str = "text/plain";
const HTML: &str = "text/html";

const AUTH_401: (u16, &str, &str) = (
    401,
    JSON,
    "Missing or wrong credential (an API key or an OIDC token; only when the server requires one)",
);
const AUTH_403: (u16, &str, &str) = (403, JSON, "The caller's role is below the route's");

/// Every route the server answers. The router and the OpenAPI document both read this.
pub static ROUTES: &[Route] = &[
    Route { method: "GET", path: "/health", id: RouteId::Health, role: None, summary: "Liveness probe", needs_data_dir: false, greedy_tail: false, request: None, query: &[], responses: &[(200, TEXT, "ok")] },
    Route { method: "GET", path: "/version", id: RouteId::Version, role: Some(Role::Reader), summary: "Overlay version, embedded engine kinds, serving tier", needs_data_dir: false, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Version document"), AUTH_401] },
    Route { method: "GET", path: "/kinds", id: RouteId::Kinds, role: Some(Role::Reader), summary: "Open and Pro scenario kinds, with the minimum tier of each Pro kind", needs_data_dir: false, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Kind catalogue"), AUTH_401] },
    Route { method: "POST", path: "/run", id: RouteId::Run, role: Some(Role::Submitter), summary: "Run one scenario synchronously (body: scenario TOML, at most 1 MiB)", needs_data_dir: false, greedy_tail: false, request: Some(("text/plain", "A scenario in TOML")), query: &[], responses: &[(200, JSON, "The run's result document"), (403, JSON, "The entitlement does not allow this kind"), (413, JSON, "Body above the cap"), (422, JSON, "Invalid scenario"), AUTH_401] },
    Route { method: "POST", path: "/study", id: RouteId::Study, role: Some(Role::Submitter), summary: "Validate and acknowledge a study request; with a data directory, also queue its dossier job", needs_data_dir: false, greedy_tail: false, request: Some((JSON, "A study request: pack, customer, prepared_for, inputs (inputs.scenario: the scenario TOML for the dossier job)")), query: &[], responses: &[(200, JSON, "Accepted (with job_id when a data directory is set)"), (400, JSON, "Invalid request"), (403, JSON, "Tier below Pro"), (422, JSON, "Empty body"), AUTH_401] },
    Route { method: "GET", path: "/", id: RouteId::Board, role: Some(Role::Reader), summary: "Job board: a self-contained page listing the jobs", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, HTML, "The job board page"), AUTH_401] },
    Route { method: "GET", path: "/openapi.json", id: RouteId::OpenApi, role: Some(Role::Reader), summary: "This OpenAPI 3.1 document, generated from the route table", needs_data_dir: false, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "OpenAPI document"), AUTH_401] },
    Route { method: "POST", path: "/jobs", id: RouteId::JobsSubmit, role: Some(Role::Submitter), summary: "Submit a job: {op, request}. The same request always gives the same job", needs_data_dir: true, greedy_tail: false, request: Some((JSON, "{\"op\": <operation name>, \"request\": <operation request>}")), query: &[], responses: &[(202, JSON, "Queued (a new job)"), (200, JSON, "The job already exists; nothing new is run"), (400, JSON, "Malformed JSON"), (403, JSON, "The entitlement does not allow it; nothing is queued"), (413, JSON, "Body above the cap"), (422, JSON, "Unknown operation or invalid request"), AUTH_401] },
    Route { method: "GET", path: "/jobs", id: RouteId::JobsList, role: Some(Role::Reader), summary: "List jobs, newest first", needs_data_dir: true, greedy_tail: false, request: None, query: &[("limit", "Maximum jobs returned (1 to 1000, default 50)"), ("state", "Only jobs in this state: queued, running, succeeded, failed or cancelled")], responses: &[(200, JSON, "Job list"), (400, JSON, "Bad query parameter"), AUTH_401] },
    Route { method: "GET", path: "/jobs/{id}", id: RouteId::JobStatus, role: Some(Role::Reader), summary: "One job's status, with server-clock times and the error text of a failed job", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Job status"), (400, JSON, "Not a 64-character hexadecimal identifier"), (404, JSON, "No such job"), AUTH_401] },
    Route { method: "GET", path: "/jobs/{id}/result", id: RouteId::JobResult, role: Some(Role::Reader), summary: "The result document of a succeeded job, byte for byte as stored", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Result document"), (400, JSON, "Bad identifier"), (404, JSON, "No such job"), (409, JSON, "Not finished, or failed"), AUTH_401] },
    Route { method: "GET", path: "/jobs/{id}/artefacts/{name}", id: RouteId::JobArtefact, role: Some(Role::Reader), summary: "One named artefact of a succeeded job", needs_data_dir: true, greedy_tail: true, request: None, query: &[], responses: &[(200, "application/octet-stream", "The artefact (media type from its extension)"), (400, JSON, "Bad identifier or artefact name"), (404, JSON, "No such job or artefact"), (409, JSON, "Not finished, or failed"), AUTH_401] },
    Route { method: "POST", path: "/jobs/{id}/cancel", id: RouteId::JobCancel, role: Some(Role::Submitter), summary: "Cancel a queued or running job (a running job stops at once; its output is never stored)", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(202, JSON, "Cancelled, or cancellation requested for a running job"), (400, JSON, "Bad identifier"), (404, JSON, "No such job"), (409, JSON, "Already finished"), AUTH_401] },
    Route { method: "GET", path: "/jobs/{id}/events", id: RouteId::JobEvents, role: Some(Role::Reader), summary: "Server-sent events: the job's status each time it changes, until it finishes (at most 1 hour)", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, "text/event-stream", "One `status` event per change, then the stream ends"), (400, JSON, "Bad identifier"), (404, JSON, "No such job"), AUTH_401] },
    Route { method: "GET", path: "/ledger", id: RouteId::Ledger, role: Some(Role::Admin), summary: "The hash-chained delivery ledger as stored", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Ledger document"), AUTH_401] },
    Route { method: "GET", path: "/ledger/verify", id: RouteId::LedgerVerify, role: Some(Role::Admin), summary: "Verify the ledger chain and every delivered job's stored files", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Verification report (verified true or false, first bad record named)"), AUTH_401] },
    Route { method: "GET", path: "/metrics", id: RouteId::Metrics, role: Some(Role::Reader), summary: "Service-level metrics in the Prometheus text format, for the caller's tenant (every tenant for an operator): jobs by state, queue wait and run time, requests by route and status, uptime", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, "text/plain; version=0.0.4", "Prometheus text exposition"), AUTH_401] },
    Route { method: "GET", path: "/audit/verify", id: RouteId::AuditVerify, role: Some(Role::Admin), summary: "Verify the hash chain of the audit log (admin)", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Verification report (verified true or false, first bad line named)"), AUTH_401, AUTH_403] },
];

/// The routes served with or without a data directory.
pub fn served_routes(jobs_enabled: bool) -> impl Iterator<Item = &'static Route> {
    ROUTES
        .iter()
        .filter(move |r| jobs_enabled || !r.needs_data_dir)
}

/// Match a method and a path (query string already removed) against the served
/// routes. Returns the route and its path parameters in template order.
pub fn match_route(
    method: &str,
    path: &str,
    jobs_enabled: bool,
) -> Option<(&'static Route, Vec<String>)> {
    let segs: Vec<&str> = path.split('/').collect();
    'routes: for route in served_routes(jobs_enabled) {
        if route.method != method {
            continue;
        }
        let tpl: Vec<&str> = route.path.split('/').collect();
        let mut params = Vec::new();
        for (i, t) in tpl.iter().enumerate() {
            let is_param = t.starts_with('{') && t.ends_with('}');
            let last = i + 1 == tpl.len();
            if is_param && last && route.greedy_tail {
                if segs.len() <= i {
                    continue 'routes;
                }
                let rest = segs[i..].join("/");
                if rest.is_empty() {
                    continue 'routes;
                }
                params.push(rest);
                return Some((route, params));
            }
            let Some(s) = segs.get(i) else {
                continue 'routes;
            };
            if is_param {
                if s.is_empty() {
                    continue 'routes;
                }
                params.push((*s).to_string());
            } else if t != s {
                continue 'routes;
            }
        }
        if segs.len() == tpl.len() {
            return Some((route, params));
        }
    }
    None
}

/// The server state the router needs.
pub struct Service {
    pub entitlement: Entitlement,
    pub jobs: Option<Arc<JobService>>,
    auth: Authenticator,
    audit: Option<AuditLog>,
    /// Requests served, by (tenant, route template, status), for `/metrics`.
    requests: std::sync::Mutex<std::collections::BTreeMap<(String, String, u16), u64>>,
    started: std::time::Instant,
}

impl Service {
    /// A service with the legacy single token (`--api-token-file`) only; it is hashed at
    /// once and the plain text is not kept.
    pub fn new(
        entitlement: Entitlement,
        jobs: Option<Arc<JobService>>,
        api_token: Option<&str>,
    ) -> Service {
        Service::with_auth(entitlement, jobs, Authenticator::new(api_token, None), None)
    }

    /// A service with an authenticator and, optionally, an audit log.
    pub fn with_auth(
        entitlement: Entitlement,
        jobs: Option<Arc<JobService>>,
        auth: Authenticator,
        audit: Option<AuditLog>,
    ) -> Service {
        Service {
            entitlement,
            jobs,
            auth,
            audit,
            requests: Default::default(),
            started: std::time::Instant::now(),
        }
    }

    /// True when requests must carry a credential.
    pub fn has_token(&self) -> bool {
        self.auth.required()
    }

    /// The audit log, when the server has a data directory.
    pub fn audit(&self) -> Option<&AuditLog> {
        self.audit.as_ref()
    }
}

/// Route one request. `authorization` is the raw `Authorization` header, if any. Every
/// request except `GET /health` is written to the audit log when the server has one.
pub fn dispatch(
    svc: &Service,
    req: &HttpRequest,
    authorization: Option<&str>,
    now: i64,
) -> HttpResponse {
    let started = std::time::Instant::now();
    let path = req
        .path
        .split_once('?')
        .map_or(req.path.as_str(), |(p, _)| p);
    let matched = match_route(&req.method, path, svc.jobs.is_some());
    let principal = svc.auth.authenticate(authorization, now);
    let resp = route_request(svc, req, matched.as_ref(), &principal, now);
    if !(req.method == "GET" && path == "/health") {
        let tenant = principal
            .as_ref()
            .map_or("-", |p| p.tenant.as_str())
            .to_string();
        let route = matched.as_ref().map_or("-", |(r, _)| r.path).to_string();
        *svc.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry((tenant, route, resp.status))
            .or_default() += 1;
    }
    if let (Some(log), false) = (&svc.audit, req.method == "GET" && path == "/health") {
        let (who, tenant, role, source) = match &principal {
            Ok(p) => (
                p.id.clone(),
                p.tenant.clone(),
                p.role.as_str(),
                p.source.as_str(),
            ),
            Err(_) => ("-".into(), "-".into(), "-", "-"),
        };
        let job = matched
            .as_ref()
            .and_then(|(_, params)| params.first().filter(|p| is_job_id(p)).cloned())
            .or_else(|| {
                serde_json::from_str::<serde_json::Value>(&resp.body)
                    .ok()
                    .and_then(|v| v["job_id"].as_str().map(str::to_string))
            });
        let entry = super::audit::Entry {
            t: now,
            who,
            tenant,
            role: role.into(),
            source: source.into(),
            method: req.method.clone(),
            route: matched.as_ref().map_or("-", |(r, _)| r.path).to_string(),
            job,
            status: resp.status,
            ms: started.elapsed().as_millis() as u64,
            prev: String::new(),
        };
        if let Err(e) = log.record(entry) {
            // A request the log cannot record is not served: the audit trail must be complete.
            return HttpResponse::error(503, &format!("the audit log cannot be written: {e}"));
        }
    }
    resp
}

fn route_request(
    svc: &Service,
    req: &HttpRequest,
    matched: Option<&(&'static Route, Vec<String>)>,
    principal: &Result<Principal, Refusal>,
    now: i64,
) -> HttpResponse {
    let (path, query) = match req.path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (req.path.as_str(), ""),
    };
    let is_health = req.method == "GET" && path == "/health";
    let who = match principal {
        Ok(p) => p.clone(),
        Err(_) if is_health => Principal::open(),
        Err(_) => {
            let mut r = HttpResponse::error(
                401,
                "authorization required: send Authorization: Bearer <API key or OIDC token>",
            );
            r.headers
                .push(("WWW-Authenticate", "Bearer realm=\"kshana-pro\"".into()));
            return r;
        }
    };
    if let Some((route, _)) = matched {
        if route.role.is_some_and(|need| who.role < need) {
            return HttpResponse::error(
                403,
                &format!(
                    "this route needs the {} role; the caller has {}",
                    route.role.map_or("", Role::as_str),
                    who.role.as_str()
                ),
            );
        }
    }
    let Some((route, params)) = matched.cloned() else {
        // Unchanged behaviour for anything else: the original router's 404.
        return super::handle(req, &svc.entitlement, now);
    };
    let jobs = svc.jobs.as_deref();
    match (route.id, jobs) {
        (RouteId::Health | RouteId::Version | RouteId::Kinds | RouteId::Run, _) => {
            super::handle(req, &svc.entitlement, now)
        }
        (RouteId::Study, None) => super::handle(req, &svc.entitlement, now),
        (RouteId::Study, Some(j)) => {
            let r = study_with_job(svc, j, &who, req, now);
            record_study(j, &who, req, &r, now);
            r
        }
        (RouteId::OpenApi, _) => HttpResponse::json(
            200,
            serde_json::to_string_pretty(&super::openapi::document(
                svc.jobs.is_some(),
                svc.has_token(),
            ))
            .unwrap_or_default(),
        ),
        (RouteId::Board, Some(j)) => board(j, &who),
        (RouteId::JobsSubmit, Some(j)) => submit(svc, j, &who, &req.body, now),
        (RouteId::JobsList, Some(j)) => list(j, &who, query),
        (RouteId::JobStatus, Some(j)) => with_job(j, &who, &params[0], |s| {
            HttpResponse::json(200, status_json(s).to_string())
        }),
        (RouteId::JobResult, Some(j)) => with_job(j, &who, &params[0], |s| result(j, s)),
        (RouteId::JobArtefact, Some(j)) => {
            if !is_safe_name(&params[1]) {
                return HttpResponse::error(
                    400,
                    "an artefact name is a plain file name: letters, digits, '.', '_' and '-', not starting with '.', no '..' and no '/'",
                );
            }
            with_job(j, &who, &params[0], |s| artefact(j, s, &params[1]))
        }
        (RouteId::JobCancel, Some(j)) => with_job(j, &who, &params[0], |s| {
            match j.cancel(&s.job_id, &who.id) {
                Ok(st) => HttpResponse::json(
                    202,
                    serde_json::json!({
                        "job_id": st.job_id,
                        "state": st.state.as_str(),
                        "cancel": if st.state == super::jobs::JobState::Cancelled { "done" } else { "requested" },
                    })
                    .to_string(),
                ),
                Err(super::worker::CancelError::Finished(state)) => HttpResponse::error(
                    409,
                    &format!("the job has already finished ({})", state.as_str()),
                ),
                Err(super::worker::CancelError::NotFound) => HttpResponse::error(404, "no such job"),
                Err(super::worker::CancelError::Io(e)) => HttpResponse::error(500, &e),
            }
        }),
        (RouteId::JobEvents, Some(j)) => with_job(j, &who, &params[0], |s| {
            // The body is the first event; the connection handler keeps the stream
            // open and writes one event per change (super::stream_job_events).
            let mut r = HttpResponse {
                status: 200,
                content_type: "text/event-stream",
                body: super::sse_event(&status_json(s)),
                headers: Vec::new(),
            };
            r.headers.push((super::STREAM_HEADER, s.job_id.clone()));
            r
        }),
        (RouteId::Ledger | RouteId::LedgerVerify | RouteId::AuditVerify, Some(_))
            if !operator(&who) =>
        {
            HttpResponse::error(
                403,
                "the ledger and the audit log span every tenant: they need an admin of tenant '*' (or the default tenant)",
            )
        }
        (RouteId::Metrics, Some(j)) => HttpResponse {
            status: 200,
            content_type: "text/plain; version=0.0.4",
            body: metrics(svc, j, &who),
            headers: Vec::new(),
        },
        (RouteId::AuditVerify, Some(_)) => match svc.audit() {
            Some(log) => HttpResponse::json(
                200,
                serde_json::to_string_pretty(&super::audit::verify(log.dir())).unwrap_or_default(),
            ),
            None => HttpResponse::error(404, "this server keeps no audit log"),
        },
        (RouteId::Ledger, Some(j)) => match j.ledger.read_json() {
            Ok(s) => HttpResponse::json(200, s),
            Err(e) => HttpResponse::error(500, &format!("cannot read the ledger: {e}")),
        },
        (RouteId::LedgerVerify, Some(j)) => HttpResponse::json(
            200,
            serde_json::to_string_pretty(&j.ledger.verify(&j.store)).unwrap_or_default(),
        ),
        // A data-directory route cannot match without one (served_routes filters it).
        (_, None) => super::handle(req, &svc.entitlement, now),
    }
}

/// One line of `<data-dir>/studies.jsonl`: the study funnel (roadmap F3c, the J metric
/// "study requests"). Written for every `POST /study` with a data directory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyRecord {
    pub t: i64,
    pub tenant: String,
    pub outcome: String,
    #[serde(default)]
    pub study_id: Option<String>,
    #[serde(default)]
    pub pack: Option<String>,
    #[serde(default)]
    pub job_id: Option<String>,
}

fn record_study(j: &JobService, who: &Principal, req: &HttpRequest, r: &HttpResponse, now: i64) {
    use std::io::Write;
    let body: serde_json::Value = serde_json::from_str(&r.body).unwrap_or_default();
    let outcome = match r.status {
        200 if body["job_id"].is_string() => "queued",
        200 => "acknowledged",
        400 | 422 => "invalid",
        403 => "forbidden",
        _ => "error",
    };
    let pack = serde_json::from_str::<serde_json::Value>(&req.body)
        .ok()
        .and_then(|v| v["pack"].as_str().map(str::to_string));
    let rec = StudyRecord {
        t: now,
        tenant: submit_tenant(who),
        outcome: outcome.into(),
        study_id: body["study_id"].as_str().map(str::to_string),
        pack,
        job_id: body["job_id"].as_str().map(str::to_string),
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(j.data_dir.join("studies.jsonl"))
    {
        let _ = writeln!(f, "{}", serde_json::to_string(&rec).unwrap_or_default());
    }
}

/// Every study record of the data directory.
pub fn study_records(j: &JobService) -> Vec<StudyRecord> {
    std::fs::read_to_string(j.data_dir.join("studies.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// A quantile of sorted values (nearest rank).
fn quantile(sorted: &[i64], q: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let i = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
    sorted[i]
}

/// Escape a Prometheus label value.
fn label(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// `GET /metrics` (roadmap F2b): service-level figures per tenant, from the job store
/// (the truth) and the request counters of this process.
fn metrics(svc: &Service, j: &JobService, who: &Principal) -> String {
    use std::collections::BTreeMap;
    use std::fmt::Write;
    let jobs: Vec<JobStatus> = j
        .store
        .list()
        .into_iter()
        .filter(|s| who.sees(&s.tenant))
        .collect();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# HELP kshana_jobs Jobs by tenant and state.\n# TYPE kshana_jobs gauge"
    );
    let mut by: BTreeMap<(&str, &str), u64> = BTreeMap::new();
    for s in &jobs {
        *by.entry((s.tenant.as_str(), s.state.as_str())).or_default() += 1;
    }
    for ((t, st), n) in &by {
        let _ = writeln!(
            out,
            "kshana_jobs{{tenant=\"{}\",state=\"{st}\"}} {n}",
            label(t)
        );
    }
    let _ = writeln!(out, "# HELP kshana_queue_length Jobs waiting in this process's queue.\n# TYPE kshana_queue_length gauge\nkshana_queue_length {}", if who.tenant == super::auth::ALL_TENANTS || who.tenant == super::auth::DEFAULT_TENANT { j.queue.len() } else { 0 });
    // Queue wait (submitted to started) and run time (started to finished), seconds,
    // over every finished job, per tenant.
    let mut wait: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    let mut run: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
    for s in jobs.iter().filter(|s| s.state.is_finished()) {
        if let (Some(st), Some(fi)) = (s.started_unix, s.finished_unix) {
            wait.entry(&s.tenant)
                .or_default()
                .push(st - s.submitted_unix);
            run.entry(&s.tenant).or_default().push(fi - st);
        }
    }
    for (name, help, map) in [
        (
            "kshana_job_queue_wait_seconds",
            "Seconds from submission to start, finished jobs.",
            &mut wait,
        ),
        (
            "kshana_job_run_seconds",
            "Seconds from start to finish, finished jobs.",
            &mut run,
        ),
    ] {
        let _ = writeln!(out, "# HELP {name} {help}\n# TYPE {name} summary");
        for (t, v) in map.iter_mut() {
            v.sort_unstable();
            for q in [0.5, 0.95, 0.99] {
                let _ = writeln!(
                    out,
                    "{name}{{tenant=\"{}\",quantile=\"{q}\"}} {}",
                    label(t),
                    quantile(v, q)
                );
            }
            let _ = writeln!(
                out,
                "{name}_sum{{tenant=\"{}\"}} {}",
                label(t),
                v.iter().sum::<i64>()
            );
            let _ = writeln!(out, "{name}_count{{tenant=\"{}\"}} {}", label(t), v.len());
        }
    }
    let _ = writeln!(out, "# HELP kshana_http_requests_total Requests served by this process, by tenant, route and status.\n# TYPE kshana_http_requests_total counter");
    for ((t, r, st), n) in svc
        .requests
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
    {
        if who.sees(t) {
            let _ = writeln!(
                out,
                "kshana_http_requests_total{{tenant=\"{}\",route=\"{}\",status=\"{st}\"}} {n}",
                label(t),
                label(r)
            );
        }
    }
    // The study funnel (F3c): requests by outcome, and the state of each queued study's
    // dossier job.
    let studies: Vec<StudyRecord> = study_records(j)
        .into_iter()
        .filter(|r| who.sees(&r.tenant))
        .collect();
    let _ = writeln!(out, "# HELP kshana_study_requests_total Study requests received, by tenant and outcome (queued, acknowledged, invalid, forbidden, error).\n# TYPE kshana_study_requests_total counter");
    let mut by: BTreeMap<(&str, &str), u64> = BTreeMap::new();
    for r in &studies {
        *by.entry((r.tenant.as_str(), r.outcome.as_str()))
            .or_default() += 1;
    }
    for ((t, o), n) in &by {
        let _ = writeln!(
            out,
            "kshana_study_requests_total{{tenant=\"{}\",outcome=\"{o}\"}} {n}",
            label(t)
        );
    }
    let _ = writeln!(out, "# HELP kshana_study_dossiers Dossier jobs queued by study requests, by tenant and job state.\n# TYPE kshana_study_dossiers gauge");
    let mut by: BTreeMap<(String, &str), u64> = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    for r in &studies {
        if let Some(id) = r.job_id.as_deref().filter(|id| seen.insert(id.to_string())) {
            if let Ok(Some(st)) = j.store.status(id) {
                *by.entry((r.tenant.clone(), st.state.as_str())).or_default() += 1;
            }
        }
    }
    for ((t, st), n) in &by {
        let _ = writeln!(
            out,
            "kshana_study_dossiers{{tenant=\"{}\",state=\"{st}\"}} {n}",
            label(t)
        );
    }
    let _ = writeln!(out, "# HELP kshana_uptime_seconds Seconds since this process started serving.\n# TYPE kshana_uptime_seconds gauge\nkshana_uptime_seconds {}", svc.started.elapsed().as_secs());
    let _ = writeln!(out, "# HELP kshana_runs_jobs 1 when this process runs jobs (it holds the lease, or has no high availability).\n# TYPE kshana_runs_jobs gauge\nkshana_runs_jobs {}", u8::from(j.active.load(std::sync::atomic::Ordering::SeqCst)));
    out
}

/// A job's status document with links to its result and artefacts.
pub(crate) fn status_json(s: &JobStatus) -> serde_json::Value {
    let mut v = serde_json::to_value(s).unwrap_or_default();
    let base = format!("/jobs/{}", s.job_id);
    let mut links = serde_json::json!({ "status": base });
    if s.state == super::jobs::JobState::Succeeded {
        links["result"] = serde_json::json!(format!("{base}/result"));
        links["artefacts"] = serde_json::json!(s
            .artefacts
            .iter()
            .map(|a| format!("{base}/artefacts/{}", a.name))
            .collect::<Vec<_>>());
    }
    v["links"] = links;
    if let Some(l) = &s.label {
        if let Some(t) = super::jobs::label_tier(l) {
            v["evidence_tier"] = serde_json::json!(t);
        }
    }
    v
}

/// True for a principal that may read what spans every tenant.
fn operator(p: &Principal) -> bool {
    p.role == Role::Admin
        && (p.tenant == super::auth::ALL_TENANTS || p.tenant == super::auth::DEFAULT_TENANT)
}

/// Another tenant's job answers exactly as a job that does not exist.
fn with_job(
    j: &JobService,
    who: &Principal,
    id: &str,
    f: impl FnOnce(&JobStatus) -> HttpResponse,
) -> HttpResponse {
    if !is_job_id(id) {
        return HttpResponse::error(
            400,
            "a job identifier is 64 lower-case hexadecimal characters",
        );
    }
    match j.store.status(id) {
        Ok(Some(s)) if who.sees(&s.tenant) => f(&s),
        Ok(_) => HttpResponse::error(404, "no such job"),
        Err(e) => HttpResponse::error(500, &format!("cannot read the job: {e}")),
    }
}

fn not_ready(s: &JobStatus) -> HttpResponse {
    let msg = match s.state {
        super::jobs::JobState::Failed => "the job failed; it has no result (see error)",
        _ => "the job has not finished",
    };
    HttpResponse::json(
        409,
        serde_json::json!({
            "error": msg,
            "job_id": s.job_id,
            "state": s.state.as_str(),
            "job_error": s.error,
        })
        .to_string(),
    )
}

fn result(j: &JobService, s: &JobStatus) -> HttpResponse {
    if s.state != super::jobs::JobState::Succeeded {
        return not_ready(s);
    }
    match j.store.read_result(&s.job_id) {
        Ok(body) => HttpResponse::json(200, body),
        Err(e) => HttpResponse::error(500, &format!("cannot read the result: {e}")),
    }
}

/// The media type served for an artefact, from its extension.
pub fn artefact_media_type(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "json" => "application/json",
        "html" => "text/html; charset=utf-8",
        "svg" => "image/svg+xml",
        "csv" => "text/csv; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    }
}

fn artefact(j: &JobService, s: &JobStatus, name: &str) -> HttpResponse {
    if s.state != super::jobs::JobState::Succeeded {
        return not_ready(s);
    }
    if !s.artefacts.iter().any(|a| a.name == name) {
        return HttpResponse::error(404, "no such artefact for this job");
    }
    match j.store.read_artefact(&s.job_id, name) {
        Ok(Some(body)) => {
            let mut r = HttpResponse::json(200, body);
            r.content_type = artefact_media_type(name);
            // A stored page is shown, never run: no script, no network.
            r.headers.push((
                "Content-Security-Policy",
                "sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:".into(),
            ));
            r
        }
        Ok(None) => HttpResponse::error(404, "no such artefact for this job"),
        Err(e) => HttpResponse::error(500, &format!("cannot read the artefact: {e}")),
    }
}

fn op_error(e: OpError) -> HttpResponse {
    match e {
        OpError::Invalid(m) => HttpResponse::error(422, &m),
        OpError::Forbidden(m) => HttpResponse::error(403, &m),
    }
}

/// The tenant a principal's jobs are stored under: its own, or the default tenant for an
/// operator key of every tenant.
fn submit_tenant(who: &Principal) -> String {
    if who.tenant == super::auth::ALL_TENANTS {
        super::auth::DEFAULT_TENANT.to_string()
    } else {
        who.tenant.clone()
    }
}

/// Store and queue a prepared job; the submission response.
fn enqueue(
    j: &JobService,
    tenant: &str,
    op: &str,
    request: &serde_json::Value,
    kind: Option<String>,
    opts: SubmitOptions,
) -> Result<(JobStatus, bool), HttpResponse> {
    let (status, created) = j
        .store
        .submit_with(tenant, op, request, kind, opts)
        .map_err(|e| HttpResponse::error(500, &format!("cannot store the job: {e}")))?;
    if created && j.active.load(std::sync::atomic::Ordering::SeqCst) {
        j.queue.push_with(&status.job_id, status.priority);
    }
    Ok((status, created))
}

fn submit(svc: &Service, j: &JobService, who: &Principal, body: &str, now: i64) -> HttpResponse {
    if body.trim().is_empty() {
        return HttpResponse::error(
            422,
            "empty request body (expected {\"op\": ..., \"request\": ...})",
        );
    }
    let doc: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return HttpResponse::error(400, &format!("malformed JSON: {e}")),
    };
    let Some(obj) = doc.as_object() else {
        return HttpResponse::error(
            422,
            "the body must be a JSON object {\"op\": ..., \"request\": ...}",
        );
    };
    if let Some(extra) = obj
        .keys()
        .find(|k| !["op", "request", "priority", "timeout_secs"].contains(&k.as_str()))
    {
        return HttpResponse::error(
            422,
            &format!("unknown field '{extra}' (expected op, request, priority, timeout_secs)"),
        );
    }
    let mut opts = SubmitOptions::default();
    if let Some(p) = obj.get("priority") {
        match p.as_str().and_then(Priority::parse) {
            Some(p) => opts.priority = p,
            None => return HttpResponse::error(422, "priority must be high, normal or low"),
        }
    }
    if let Some(t) = obj.get("timeout_secs") {
        match t.as_u64() {
            Some(t) if (1..=super::worker::MAX_TIMEOUT_SECS).contains(&t) => {
                opts.timeout_secs = Some(t)
            }
            _ => {
                return HttpResponse::error(
                    422,
                    "timeout_secs must be an integer from 1 to 604800 (one week)",
                )
            }
        }
    }
    let Some(op_name) = obj.get("op").and_then(|v| v.as_str()) else {
        return HttpResponse::error(422, "op is required and must be a string");
    };
    let Some(op) = ops::find(op_name) else {
        return HttpResponse::json(
            422,
            serde_json::json!({
                "error": format!("unknown operation '{op_name}'"),
                "operations": ops::names(),
            })
            .to_string(),
        );
    };
    let Some(request) = obj.get("request") else {
        return HttpResponse::error(422, "request is required");
    };
    let prepared = match (op.prepare)(request, &svc.entitlement, now) {
        Ok(p) => p,
        Err(e) => return op_error(e),
    };
    match enqueue(
        j,
        &submit_tenant(who),
        op.name,
        request,
        prepared.kind,
        opts,
    ) {
        Ok((status, created)) => HttpResponse::json(
            if created { 202 } else { 200 },
            serde_json::json!({
                "job_id": status.job_id,
                "op": status.op,
                "state": status.state.as_str(),
                "created": created,
                "links": { "status": format!("/jobs/{}", status.job_id) },
            })
            .to_string(),
        ),
        Err(r) => r,
    }
}

/// Parse `limit` and `state` from a query string.
fn list(j: &JobService, who: &Principal, query: &str) -> HttpResponse {
    let mut limit = 50usize;
    let mut state: Option<String> = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "limit" => match v.parse::<usize>() {
                Ok(n) if (1..=1000).contains(&n) => limit = n,
                _ => return HttpResponse::error(400, "limit must be an integer from 1 to 1000"),
            },
            "state" => {
                if !["queued", "running", "succeeded", "failed", "cancelled"].contains(&v) {
                    return HttpResponse::error(
                        400,
                        "state must be queued, running, succeeded, failed or cancelled",
                    );
                }
                state = Some(v.to_string());
            }
            _ => return HttpResponse::error(400, &format!("unknown query parameter '{k}'")),
        }
    }
    HttpResponse::json(200, jobs_page(j, who, limit, state.as_deref()).to_string())
}

/// The `GET /jobs` document (also embedded in the job board page).
pub fn jobs_page(
    j: &JobService,
    who: &Principal,
    limit: usize,
    state: Option<&str>,
) -> serde_json::Value {
    let all: Vec<JobStatus> = j
        .store
        .list()
        .into_iter()
        .filter(|s| who.sees(&s.tenant))
        .collect();
    let total = all.len();
    let mut counts = serde_json::Map::new();
    for st in ["queued", "running", "succeeded", "failed", "cancelled"] {
        let n = all.iter().filter(|s| s.state.as_str() == st).count();
        counts.insert(st.to_string(), serde_json::json!(n));
    }
    let jobs: Vec<serde_json::Value> = all
        .iter()
        .filter(|s| state.is_none_or(|st| s.state.as_str() == st))
        .take(limit)
        .map(status_json)
        .collect();
    serde_json::json!({
        "total": total,
        "counts": counts,
        "limit": limit,
        "returned": jobs.len(),
        "jobs": jobs,
    })
}

const BOARD_HTML: &str = include_str!("board.html");

/// The job board page, with the current job list embedded so the page shows real jobs
/// even before (or without) its first refresh.
fn board(j: &JobService, who: &Principal) -> HttpResponse {
    let snapshot = jobs_page(j, who, 100, None).to_string();
    // Inside a <script type="application/json"> block, "<" is the only character that
    // could end the block early; JSON allows it escaped.
    let safe = snapshot.replace('<', "\\u003c");
    let (_, utc) = super::jobs::now_stamp();
    let page = BOARD_HTML
        .replace("__KSHANA_INITIAL_JOBS__", &safe)
        .replace("__KSHANA_SERVED_UTC__", &utc)
        .replace("__KSHANA_VERSION__", crate::version());
    let page = crate::report_brand::finish_html(
        &page,
        &crate::report_brand::DocMark::new("Service job board", format!("served {utc}")),
    );
    let mut r = HttpResponse::json(200, page);
    r.content_type = "text/html; charset=utf-8";
    r.headers.push((
        "Content-Security-Policy",
        "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; font-src data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'".into(),
    ));
    r
}

/// `POST /study` with a data directory: the original acknowledgement, plus a `dossier`
/// job for the request's `inputs.scenario`.
fn study_with_job(
    svc: &Service,
    j: &JobService,
    who: &Principal,
    req: &HttpRequest,
    now: i64,
) -> HttpResponse {
    let ack = super::handle(req, &svc.entitlement, now);
    if ack.status != 200 {
        return ack;
    }
    let Ok(mut body) = serde_json::from_str::<serde_json::Value>(&ack.body) else {
        return ack;
    };
    let study: super::study_intake::StudyRequest = match serde_json::from_str(&req.body) {
        Ok(s) => s,
        Err(_) => return ack, // cannot happen: handle() accepted it
    };
    let Some(scenario) = study.inputs.get("scenario").and_then(|v| v.as_str()) else {
        body["job_id"] = serde_json::Value::Null;
        body["job_note"] = serde_json::json!(
            "no dossier job was created: the request carries no inputs.scenario (the scenario TOML to run)"
        );
        return HttpResponse::json(200, body.to_string());
    };
    if let Some(kind) = crate::registry::peek_kind(scenario) {
        if kind != study.pack {
            return HttpResponse::json(
                400,
                serde_json::json!({
                    "error": "invalid study request",
                    "violations": [format!("inputs.scenario is of kind '{kind}' but pack is '{}'", study.pack)],
                })
                .to_string(),
            );
        }
    }
    let request = serde_json::json!({ "scenario": scenario });
    let op = ops::find("dossier").expect("the dossier operation is in the table");
    let prepared = match (op.prepare)(&request, &svc.entitlement, now) {
        Ok(p) => p,
        Err(e) => return op_error(e),
    };
    match enqueue(
        j,
        &submit_tenant(who),
        op.name,
        &request,
        prepared.kind,
        SubmitOptions::default(),
    ) {
        Ok((status, _)) => {
            body["job_id"] = serde_json::json!(status.job_id);
            body["job_state"] = serde_json::json!(status.state.as_str());
            HttpResponse::json(200, body.to_string())
        }
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_matches_itself_and_nothing_is_ambiguous() {
        for r in ROUTES {
            let concrete = r
                .path
                .replace("{id}", &"a".repeat(64))
                .replace("{name}", "chart.svg");
            let (m, _) = match_route(r.method, &concrete, true)
                .unwrap_or_else(|| panic!("{} {} does not match itself", r.method, r.path));
            assert_eq!(m.id, r.id, "{} {} matched another route", r.method, r.path);
        }
    }

    #[test]
    fn data_dir_routes_are_absent_without_one() {
        assert!(match_route("GET", "/jobs", false).is_none());
        assert!(match_route("GET", "/", false).is_none());
        assert!(match_route("GET", "/health", false).is_some());
        assert!(match_route("GET", "/openapi.json", false).is_some());
    }

    #[test]
    fn greedy_artefact_name_captures_slashes() {
        let id = "b".repeat(64);
        let (r, p) = match_route("GET", &format!("/jobs/{id}/artefacts/a/../b"), true).unwrap();
        assert_eq!(r.id, RouteId::JobArtefact);
        assert_eq!(p[1], "a/../b");
        assert!(match_route("GET", &format!("/jobs/{id}/artefacts/"), true).is_none());
    }

    #[test]
    fn every_route_but_health_needs_a_role() {
        for r in ROUTES {
            assert_eq!(
                r.role.is_none(),
                r.id == RouteId::Health,
                "{} {}",
                r.method,
                r.path
            );
            if r.method == "POST" {
                assert!(r.role >= Some(Role::Submitter), "{} {}", r.method, r.path);
            }
        }
    }
}
