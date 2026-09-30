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

use sha2::{Digest, Sha256};

use super::jobs::{is_job_id, is_safe_name, JobStatus};
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
    Ledger,
    LedgerVerify,
}

/// One route: the method, the path template (OpenAPI form, `{param}` for a segment)
/// and what the OpenAPI document says about it.
#[derive(Debug)]
pub struct Route {
    pub method: &'static str,
    pub path: &'static str,
    pub id: RouteId,
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
    "Missing or wrong bearer token (only when the server has a token)",
);

/// Every route the server answers. The router and the OpenAPI document both read this.
pub static ROUTES: &[Route] = &[
    Route { method: "GET", path: "/health", id: RouteId::Health, summary: "Liveness probe", needs_data_dir: false, greedy_tail: false, request: None, query: &[], responses: &[(200, TEXT, "ok")] },
    Route { method: "GET", path: "/version", id: RouteId::Version, summary: "Overlay version, embedded engine kinds, serving tier", needs_data_dir: false, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Version document"), AUTH_401] },
    Route { method: "GET", path: "/kinds", id: RouteId::Kinds, summary: "Open and Pro scenario kinds, with the minimum tier of each Pro kind", needs_data_dir: false, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Kind catalogue"), AUTH_401] },
    Route { method: "POST", path: "/run", id: RouteId::Run, summary: "Run one scenario synchronously (body: scenario TOML, at most 1 MiB)", needs_data_dir: false, greedy_tail: false, request: Some(("text/plain", "A scenario in TOML")), query: &[], responses: &[(200, JSON, "The run's result document"), (403, JSON, "The entitlement does not allow this kind"), (413, JSON, "Body above the cap"), (422, JSON, "Invalid scenario"), AUTH_401] },
    Route { method: "POST", path: "/study", id: RouteId::Study, summary: "Validate and acknowledge a study request; with a data directory, also queue its dossier job", needs_data_dir: false, greedy_tail: false, request: Some((JSON, "A study request: pack, customer, prepared_for, inputs (inputs.scenario: the scenario TOML for the dossier job)")), query: &[], responses: &[(200, JSON, "Accepted (with job_id when a data directory is set)"), (400, JSON, "Invalid request"), (403, JSON, "Tier below Pro"), (422, JSON, "Empty body"), AUTH_401] },
    Route { method: "GET", path: "/", id: RouteId::Board, summary: "Job board: a self-contained page listing the jobs", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, HTML, "The job board page"), AUTH_401] },
    Route { method: "GET", path: "/openapi.json", id: RouteId::OpenApi, summary: "This OpenAPI 3.1 document, generated from the route table", needs_data_dir: false, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "OpenAPI document"), AUTH_401] },
    Route { method: "POST", path: "/jobs", id: RouteId::JobsSubmit, summary: "Submit a job: {op, request}. The same request always gives the same job", needs_data_dir: true, greedy_tail: false, request: Some((JSON, "{\"op\": <operation name>, \"request\": <operation request>}")), query: &[], responses: &[(202, JSON, "Queued (a new job)"), (200, JSON, "The job already exists; nothing new is run"), (400, JSON, "Malformed JSON"), (403, JSON, "The entitlement does not allow it; nothing is queued"), (413, JSON, "Body above the cap"), (422, JSON, "Unknown operation or invalid request"), AUTH_401] },
    Route { method: "GET", path: "/jobs", id: RouteId::JobsList, summary: "List jobs, newest first", needs_data_dir: true, greedy_tail: false, request: None, query: &[("limit", "Maximum jobs returned (1 to 1000, default 50)"), ("state", "Only jobs in this state: queued, running, succeeded or failed")], responses: &[(200, JSON, "Job list"), (400, JSON, "Bad query parameter"), AUTH_401] },
    Route { method: "GET", path: "/jobs/{id}", id: RouteId::JobStatus, summary: "One job's status, with server-clock times and the error text of a failed job", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Job status"), (400, JSON, "Not a 64-character hexadecimal identifier"), (404, JSON, "No such job"), AUTH_401] },
    Route { method: "GET", path: "/jobs/{id}/result", id: RouteId::JobResult, summary: "The result document of a succeeded job, byte for byte as stored", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Result document"), (400, JSON, "Bad identifier"), (404, JSON, "No such job"), (409, JSON, "Not finished, or failed"), AUTH_401] },
    Route { method: "GET", path: "/jobs/{id}/artefacts/{name}", id: RouteId::JobArtefact, summary: "One named artefact of a succeeded job", needs_data_dir: true, greedy_tail: true, request: None, query: &[], responses: &[(200, "application/octet-stream", "The artefact (media type from its extension)"), (400, JSON, "Bad identifier or artefact name"), (404, JSON, "No such job or artefact"), (409, JSON, "Not finished, or failed"), AUTH_401] },
    Route { method: "GET", path: "/ledger", id: RouteId::Ledger, summary: "The hash-chained delivery ledger as stored", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Ledger document"), AUTH_401] },
    Route { method: "GET", path: "/ledger/verify", id: RouteId::LedgerVerify, summary: "Verify the ledger chain and every delivered job's stored files", needs_data_dir: true, greedy_tail: false, request: None, query: &[], responses: &[(200, JSON, "Verification report (verified true or false, first bad record named)"), AUTH_401] },
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
    token_sha256: Option<[u8; 32]>,
}

impl Service {
    /// A service; `api_token` is hashed at once and the plain text is not kept.
    pub fn new(
        entitlement: Entitlement,
        jobs: Option<Arc<JobService>>,
        api_token: Option<&str>,
    ) -> Service {
        Service {
            entitlement,
            jobs,
            token_sha256: api_token.map(|t| Sha256::digest(t.as_bytes()).into()),
        }
    }

    /// True when requests must carry the bearer token.
    pub fn has_token(&self) -> bool {
        self.token_sha256.is_some()
    }

    /// Check an `Authorization` header value against the token, in constant time.
    fn authorized(&self, authorization: Option<&str>) -> bool {
        let Some(expected) = &self.token_sha256 else {
            return true;
        };
        let presented = authorization
            .and_then(|h| {
                let h = h.trim();
                let (scheme, rest) = h.split_once(' ')?;
                scheme.eq_ignore_ascii_case("bearer").then(|| rest.trim())
            })
            .unwrap_or("");
        let got: [u8; 32] = Sha256::digest(presented.as_bytes()).into();
        let mut diff = 0u8;
        for (a, b) in got.iter().zip(expected.iter()) {
            diff |= a ^ b;
        }
        diff == 0 && !presented.is_empty()
    }
}

/// Route one request. `authorization` is the raw `Authorization` header, if any.
pub fn dispatch(
    svc: &Service,
    req: &HttpRequest,
    authorization: Option<&str>,
    now: i64,
) -> HttpResponse {
    let (path, query) = match req.path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (req.path.as_str(), ""),
    };
    let is_health = req.method == "GET" && path == "/health";
    if !is_health && !svc.authorized(authorization) {
        let mut r = HttpResponse::error(
            401,
            "authorization required: send Authorization: Bearer <token>",
        );
        r.headers
            .push(("WWW-Authenticate", "Bearer realm=\"kshana-pro\"".into()));
        return r;
    }
    let Some((route, params)) = match_route(&req.method, path, svc.jobs.is_some()) else {
        // Unchanged behaviour for anything else: the original router's 404.
        return super::handle(req, &svc.entitlement, now);
    };
    let jobs = svc.jobs.as_deref();
    match (route.id, jobs) {
        (RouteId::Health | RouteId::Version | RouteId::Kinds | RouteId::Run, _) => {
            super::handle(req, &svc.entitlement, now)
        }
        (RouteId::Study, None) => super::handle(req, &svc.entitlement, now),
        (RouteId::Study, Some(j)) => study_with_job(svc, j, req, now),
        (RouteId::OpenApi, _) => HttpResponse::json(
            200,
            serde_json::to_string_pretty(&super::openapi::document(
                svc.jobs.is_some(),
                svc.has_token(),
            ))
            .unwrap_or_default(),
        ),
        (RouteId::Board, Some(j)) => board(j),
        (RouteId::JobsSubmit, Some(j)) => submit(svc, j, &req.body, now),
        (RouteId::JobsList, Some(j)) => list(j, query),
        (RouteId::JobStatus, Some(j)) => with_job(j, &params[0], |s| {
            HttpResponse::json(200, status_json(s).to_string())
        }),
        (RouteId::JobResult, Some(j)) => with_job(j, &params[0], |s| result(j, s)),
        (RouteId::JobArtefact, Some(j)) => {
            if !is_safe_name(&params[1]) {
                return HttpResponse::error(
                    400,
                    "an artefact name is a plain file name: letters, digits, '.', '_' and '-', not starting with '.', no '..' and no '/'",
                );
            }
            with_job(j, &params[0], |s| artefact(j, s, &params[1]))
        }
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

/// A job's status document with links to its result and artefacts.
fn status_json(s: &JobStatus) -> serde_json::Value {
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

fn with_job(j: &JobService, id: &str, f: impl FnOnce(&JobStatus) -> HttpResponse) -> HttpResponse {
    if !is_job_id(id) {
        return HttpResponse::error(
            400,
            "a job identifier is 64 lower-case hexadecimal characters",
        );
    }
    match j.store.status(id) {
        Ok(Some(s)) => f(&s),
        Ok(None) => HttpResponse::error(404, "no such job"),
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

/// Store and queue a prepared job; the submission response.
fn enqueue(
    j: &JobService,
    op: &str,
    request: &serde_json::Value,
    kind: Option<String>,
) -> Result<(JobStatus, bool), HttpResponse> {
    let (status, created) = j
        .store
        .submit(op, request, kind)
        .map_err(|e| HttpResponse::error(500, &format!("cannot store the job: {e}")))?;
    if created {
        j.queue.push(&status.job_id);
    }
    Ok((status, created))
}

fn submit(svc: &Service, j: &JobService, body: &str, now: i64) -> HttpResponse {
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
    if let Some(extra) = obj.keys().find(|k| *k != "op" && *k != "request") {
        return HttpResponse::error(
            422,
            &format!("unknown field '{extra}' (expected op and request)"),
        );
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
    match enqueue(j, op.name, request, prepared.kind) {
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
fn list(j: &JobService, query: &str) -> HttpResponse {
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
                if !["queued", "running", "succeeded", "failed"].contains(&v) {
                    return HttpResponse::error(
                        400,
                        "state must be queued, running, succeeded or failed",
                    );
                }
                state = Some(v.to_string());
            }
            _ => return HttpResponse::error(400, &format!("unknown query parameter '{k}'")),
        }
    }
    HttpResponse::json(200, jobs_page(j, limit, state.as_deref()).to_string())
}

/// The `GET /jobs` document (also embedded in the job board page).
pub fn jobs_page(j: &JobService, limit: usize, state: Option<&str>) -> serde_json::Value {
    let all = j.store.list();
    let total = all.len();
    let mut counts = serde_json::Map::new();
    for st in ["queued", "running", "succeeded", "failed"] {
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
fn board(j: &JobService) -> HttpResponse {
    let snapshot = jobs_page(j, 100, None).to_string();
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
fn study_with_job(svc: &Service, j: &JobService, req: &HttpRequest, now: i64) -> HttpResponse {
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
    match enqueue(j, op.name, &request, prepared.kind) {
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
    fn bearer_check_is_exact() {
        let svc = Service::new(Entitlement::evaluation(), None, Some("s3cret"));
        assert!(svc.authorized(Some("Bearer s3cret")));
        assert!(svc.authorized(Some("bearer s3cret")));
        assert!(!svc.authorized(Some("Bearer s3cre")));
        assert!(!svc.authorized(Some("s3cret")));
        assert!(!svc.authorized(None));
        let open = Service::new(Entitlement::evaluation(), None, None);
        assert!(open.authorized(None));
    }
}
