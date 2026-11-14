// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Deployment (PR22) — hostable HTTP service
//!
//! Makes the overlay deployable as a service (on-prem / air-gapped / sovereign /
//! hosted). Deliberately **dependency-light**: a blocking, std-only HTTP/1.1 server
//! (no async runtime), so the whole request path is auditable and the container has
//! a tiny attack surface — the right posture for an air-gapped/sovereign install.
//!
//! Routes:
//!  * `GET  /health`  → liveness probe (`ok`)
//!  * `GET  /version` → overlay + embedded-engine info
//!  * `GET  /kinds`   → open + Pro scenario kinds (with min tier)
//!  * `POST /run` → body is a scenario TOML; runs it (entitlement-gated), returns JSON
//!  * `POST /study` → body is a JSON study request; validates + acknowledges it
//!    (entitlement-gated; a study requires at least the Pro tier). Without a data
//!    directory it runs nothing (intake only, see [`study_intake`]); with one it also
//!    queues a `dossier` job for `inputs.scenario` and adds `job_id` to the reply.
//!  * `GET /openapi.json` → the OpenAPI 3.1 document, generated from [`api::ROUTES`]
//!
//! With a data directory (`--data-dir`), the on-premises job service is added
//! ([`api`], [`jobs`], [`worker`], [`ops`], [`delivery`]): `POST /jobs`, `GET /jobs`,
//! `GET /jobs/<id>`, `GET /jobs/<id>/result`, `GET /jobs/<id>/artefacts/<name>`,
//! `GET /ledger`, `GET /ledger/verify` and the job board page at `GET /`. Jobs are
//! stored on disk, run by a bounded worker pool, survive a restart, and every delivery
//! is recorded in a hash-chained ledger. An optional bearer token
//! (`--api-token-file`) guards every route except `/health`. The service makes no
//! outbound connection and sends no telemetry: the only outgoing socket in this module
//! is [`healthcheck`], which probes the server's own address.
//!
//! The pure request handler ([`handle`]) and reader ([`read_request`]) are unit
//! tested without sockets; [`start`] / [`serve_with`] / [`serve`] are the thin socket
//! loop.

pub mod api;
pub mod audit;
pub mod auth;
pub mod backup;
pub mod clients;
pub mod conf;
pub mod delivery;
pub mod fleet;
pub mod ha;
pub mod jobs;
pub mod library;
pub mod oidc;
pub mod openapi;
pub mod ops;
pub mod study_intake;
pub mod tls;
pub mod trial;
#[cfg(windows)]
pub mod winsvc;
pub mod worker;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kshana::api::KshanaError;

use crate::license::{Entitlement, Tier};
use study_intake::{study_id, validate, StudyRequest};

/// Caps to bound resource use (a basic DoS guard for an exposed service).
pub const MAX_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Body cap for `POST /jobs` (16 MiB): room for a Pack C manifest with its scenario
/// texts inline. A body above it is refused with 413 before it is read.
pub const JOBS_MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
/// Per-socket read/write timeout — bounds slow-client (Slowloris) thread holds.
pub const IO_TIMEOUT_SECS: u64 = 30;
/// Max concurrent connections — bounds thread/memory use under a connection flood.
pub const MAX_CONNECTIONS: usize = 256;

/// A parsed HTTP request (only what the service needs).
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: String,
}

/// An HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
    /// Extra response headers (name, value), e.g. `WWW-Authenticate`.
    pub headers: Vec<(&'static str, String)>,
}

impl HttpResponse {
    fn json(status: u16, body: String) -> Self {
        HttpResponse {
            status,
            content_type: "application/json",
            body,
            headers: Vec::new(),
        }
    }
    fn text(status: u16, body: &str) -> Self {
        HttpResponse {
            status,
            content_type: "text/plain",
            body: body.to_string(),
            headers: Vec::new(),
        }
    }
    fn error(status: u16, msg: &str) -> Self {
        Self::json(status, serde_json::json!({ "error": msg }).to_string())
    }
    /// Serialise to raw HTTP/1.1 bytes (Connection: close — one request per conn).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.status,
            reason(self.status),
            self.content_type,
            self.body.len(),
        );
        for (name, value) in &self.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(self.body.as_bytes());
        bytes
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        201 => "Created",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

struct Head {
    method: String,
    path: String,
    content_length: Option<usize>,
    authorization: Option<String>,
}

/// Parse the request head (everything before the blank line). Pure.
fn parse_request_head(head: &str) -> Option<Head> {
    let mut lines = head.split("\r\n");
    let req_line = lines.next()?;
    let mut parts = req_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut content_length = None;
    let mut authorization = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("authorization") {
                authorization = Some(v.trim().to_string());
            }
            if k.trim().eq_ignore_ascii_case("content-length") {
                // Reject duplicate or malformed Content-Length (request-smuggling guard).
                if content_length.is_some() {
                    return None;
                }
                match v.trim().parse::<usize>() {
                    Ok(n) => content_length = Some(n),
                    Err(_) => return None,
                }
            }
        }
    }
    Some(Head {
        method,
        path,
        content_length,
        authorization,
    })
}

/// Read one HTTP request from `r`, enforcing header/body size caps. Generic over
/// [`Read`] so it is unit-tested with an in-memory cursor (no socket).
pub fn read_request<R: Read>(
    r: R,
    max_header: usize,
    max_body: usize,
) -> Result<HttpRequest, String> {
    read_request_with(r, max_header, |_, _| max_body).map(|(req, _)| req)
}

/// [`read_request`] with a body cap chosen from the method and path, returning the raw
/// `Authorization` header too. A body above the cap is refused before it is read.
pub fn read_request_with<R: Read>(
    mut r: R,
    max_header: usize,
    max_body_for: impl Fn(&str, &str) -> usize,
) -> Result<(HttpRequest, Option<String>), String> {
    // Accumulate bytes until the header terminator `\r\n\r\n`.
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if buf.len() >= max_header {
            return Err("request header too large".into());
        }
        let n = r.read(&mut byte).map_err(|e| format!("read error: {e}"))?;
        if n == 0 {
            return Err("connection closed before full header".into());
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let head_str = String::from_utf8_lossy(&buf[..buf.len() - 4]).to_string();
    let head = parse_request_head(&head_str).ok_or("malformed request line")?;

    let clen = head.content_length.unwrap_or(0);
    let max_body = max_body_for(&head.method, head.path.split('?').next().unwrap_or(""));
    if clen > max_body {
        return Err(format!("body too large ({clen} > {max_body} bytes)"));
    }
    let mut body = vec![0u8; clen];
    r.read_exact(&mut body)
        .map_err(|e| format!("body read error: {e}"))?;
    Ok((
        HttpRequest {
            method: head.method,
            path: head.path,
            body: String::from_utf8_lossy(&body).to_string(),
        },
        head.authorization,
    ))
}

/// The body cap for a request: [`JOBS_MAX_BODY_BYTES`] for `POST /jobs`, else
/// [`MAX_BODY_BYTES`].
pub fn body_cap(method: &str, path: &str) -> usize {
    if method == "POST" && path == "/jobs" {
        JOBS_MAX_BODY_BYTES
    } else {
        MAX_BODY_BYTES
    }
}

/// Current Unix time (seconds) for entitlement expiry checks.
fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The pure router: map a request to a response, enforcing the entitlement on Pro
/// scenarios. No I/O — unit tested directly.
pub fn handle(req: &HttpRequest, entitlement: &Entitlement, now: i64) -> HttpResponse {
    let path = req.path.split('?').next().unwrap_or("");
    match (req.method.as_str(), path) {
        ("GET", "/health") => HttpResponse::text(200, "ok\n"),
        ("GET", "/version") => HttpResponse::json(
            200,
            serde_json::json!({
                "overlay": "kshana-pro",
                "overlay_version": crate::version(),
                "open_engine_kinds": crate::open_engine_kinds(),
                "tier": entitlement.tier.as_str(),
            })
            .to_string(),
        ),
        ("GET", "/kinds") => {
            let pro: Vec<_> = crate::registry::pro_kinds()
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "name": m.name,
                        "min_tier": crate::license::feature_min_tier(m.name).as_str(),
                    })
                })
                .collect();
            let open: Vec<_> = kshana::api::list_scenario_kinds()
                .iter()
                .map(|m| m.name)
                .collect();
            HttpResponse::json(
                200,
                serde_json::json!({ "open": open, "pro": pro }).to_string(),
            )
        }
        ("POST", "/run") => run_route(&req.body, entitlement, now),
        ("POST", "/study") => study_route(&req.body, entitlement, now),
        ("GET", _) | ("POST", _) => HttpResponse::error(404, "not found"),
        _ => HttpResponse::error(404, "not found"),
    }
}

fn run_route(body: &str, entitlement: &Entitlement, now: i64) -> HttpResponse {
    if body.trim().is_empty() {
        return HttpResponse::error(422, "empty request body (expected a scenario TOML)");
    }
    match crate::registry::run_gated(body, entitlement, now) {
        Ok(out) => HttpResponse::json(200, out.json),
        Err(e) => {
            // Map the engine error to a meaningful HTTP status.
            let status = match e {
                KshanaError::Unsupported(_) => 403, // entitlement/feature gate
                KshanaError::InvalidInput(_) => 422,
                _ => 500,
            };
            HttpResponse::error(status, &format!("{e}"))
        }
    }
}

/// The minimum tier a study request requires. A study is a paid, packaged
/// deliverable, so intake is gated at Pro (Evaluation cannot request one).
const STUDY_MIN_TIER: Tier = Tier::Pro;

/// `POST /study`: parse a JSON study request, validate it, gate it by license tier
/// (reusing the same [`Entitlement`] machinery as `/run`), and acknowledge it. Does
/// NOT execute the pack — intake only. On success returns a small JSON ack with a
/// deterministic `study_id`.
fn study_route(body: &str, entitlement: &Entitlement, now: i64) -> HttpResponse {
    if body.trim().is_empty() {
        return HttpResponse::error(422, "empty request body (expected a JSON study request)");
    }
    let req: StudyRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return HttpResponse::error(400, &format!("invalid study request JSON: {e}")),
    };

    // Validate the request shape + that the pack is a known runnable kind.
    let violations = validate(&req);
    if !violations.is_empty() {
        return HttpResponse::json(
            400,
            serde_json::json!({
                "error": "invalid study request",
                "violations": violations,
            })
            .to_string(),
        );
    }

    // Tier gate, reusing the same Entitlement (expiry + rank) logic `/run` relies on.
    // An expired license, or a tier below Pro, may not request a study. Do NOT echo
    // the licensee name — this 403 body can reach an unauthorised caller.
    if entitlement.is_expired(now) || entitlement.tier.rank() < STUDY_MIN_TIER.rank() {
        return HttpResponse::error(
            403,
            &format!(
                "a study request requires at least the '{}' tier; current license tier is '{}'{}",
                STUDY_MIN_TIER.as_str(),
                entitlement.tier.as_str(),
                if entitlement.is_expired(now) {
                    " (and it has expired)"
                } else {
                    ""
                },
            ),
        );
    }

    // Accepted. The pack is packaged in a later task; for now we acknowledge with a
    // deterministic id. Note whether the kind routes to a Pro pack or the open engine.
    let routes_to = if crate::registry::is_pro_kind(&req.pack) {
        "pro-pack"
    } else {
        "open-engine"
    };
    HttpResponse::json(
        200,
        serde_json::json!({
            "study_id": study_id(&req),
            "pack": req.pack,
            "routes_to": routes_to,
            "status": "accepted",
        })
        .to_string(),
    )
}

/// Run the blocking HTTP server, one thread per connection. Each connection serves
/// exactly one request (Connection: close). No data directory: the routes and behaviour
/// the service had before the job service existed, plus `GET /openapi.json`.
pub fn serve(addr: &str, entitlement: Entitlement) -> std::io::Result<()> {
    serve_with(ServeConfig::new(addr, entitlement))
}

/// How to run the server. [`ServeConfig::new`] gives the defaults: no data directory
/// (no job service), one worker, no access token.
pub struct ServeConfig {
    /// Address to listen on, e.g. `127.0.0.1:8088` (port 0: the system picks one).
    pub addr: String,
    /// The single entitlement the whole process serves at.
    pub entitlement: Entitlement,
    /// The job store, ledger and artefacts live here; `None` turns the job service off.
    pub data_dir: Option<PathBuf>,
    /// Worker threads for jobs (clamped to 1..=[`worker::MAX_WORKERS`]).
    pub workers: usize,
    /// When set, every route except `/health` needs `Authorization: Bearer <token>`.
    /// Only its SHA-256 is kept once the server starts.
    pub api_token: Option<String>,
    /// Named API keys (`--keys-file`, roadmap A9).
    pub keys: Option<auth::KeysFile>,
    /// OIDC bearer tokens, verified offline (`--oidc <oidc.toml>`).
    pub oidc: Option<PathBuf>,
    /// Days of audit log kept (whole day files; default 400). Only with a data directory.
    pub audit_retention_days: u32,
    /// High availability (`--ha-node`): several nodes on one shared data directory, one
    /// running jobs at a time ([`ha`]). Needs the `pro-service-ha` feature.
    pub ha: Option<ha::HaConfig>,
    /// Serve HTTPS with this certificate chain and private key (PEM files) instead of
    /// plain HTTP ([`tls`]).
    pub tls: Option<(PathBuf, PathBuf)>,
    /// The hosted trial sandbox (`--trial`): open, Evaluation tier, quotas ([`trial`]).
    pub trial: Option<trial::TrialConfig>,
    /// The licence's `stations` claim, for the Station fleet limit ([`fleet`]).
    pub stations_claim: u32,
    /// Re-queue jobs left queued or running by an earlier process (default true). A
    /// test switches it off to prove that recovery is what finishes those jobs.
    #[doc(hidden)]
    pub recover_on_start: bool,
}

impl ServeConfig {
    /// The defaults for `addr` and `entitlement`.
    pub fn new(addr: &str, entitlement: Entitlement) -> ServeConfig {
        ServeConfig {
            addr: addr.to_string(),
            entitlement,
            data_dir: None,
            workers: 1,
            api_token: None,
            keys: None,
            oidc: None,
            audit_retention_days: 400,
            ha: None,
            tls: None,
            trial: None,
            stations_claim: 0,
            recover_on_start: true,
        }
    }
}

/// A running server, returned by [`start`].
pub struct ServerHandle {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
    pool: Option<worker::WorkerPool>,
    service: Arc<api::Service>,
    ha: Option<ha::HaNode>,
}

impl ServerHandle {
    /// The address actually bound (useful when the configured port was 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The job service, when the server has a data directory.
    pub fn jobs(&self) -> Option<&Arc<worker::JobService>> {
        self.service.jobs.as_ref()
    }

    /// Clean stop: stop accepting connections, let each worker finish the job it is
    /// running, and return. Jobs still queued stay queued on disk for the next start.
    pub fn stop(mut self) {
        self.shutdown();
    }

    /// Block until the accept loop ends (it ends only when [`stop`](Self::stop) is
    /// called from another handle, so a command-line server runs until killed).
    pub fn wait(mut self) -> std::io::Result<()> {
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
        self.shutdown();
        Ok(())
    }

    /// Stop as a failed node would: stop serving and running jobs at once and leave the
    /// high-availability lease to expire (failover tests).
    #[doc(hidden)]
    pub fn abandon(mut self) {
        if let Some(h) = self.ha.take() {
            h.stop(false);
        }
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if let Some(h) = self.ha.take() {
            h.stop(true);
        }
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
        if let Some(p) = self.pool.take() {
            p.stop();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// How often the accept loop checks for a stop request while no connection arrives.
const ACCEPT_POLL: Duration = Duration::from_millis(20);

/// Bind, open the job store (when configured), re-queue unfinished jobs, start the
/// workers and the accept loop, and return at once.
pub fn start(config: ServeConfig) -> std::io::Result<ServerHandle> {
    let listener = TcpListener::bind(&config.addr)?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;

    if config.ha.is_some() {
        if config.data_dir.is_none() {
            return Err(std::io::Error::other(
                "high availability needs a shared --data-dir",
            ));
        }
        if !config.entitlement.allows("pro-service-ha", now_unix()) {
            return Err(std::io::Error::other(format!(
                "high availability requires the '{}' tier (or the 'pro-service-ha' feature on the licence)",
                crate::license::feature_min_tier("pro-service-ha").as_str()
            )));
        }
    }
    let mut config = config;
    if config.trial.is_some() {
        if config.data_dir.is_none() {
            return Err(std::io::Error::other("the trial needs a --data-dir"));
        }
        if config.api_token.is_some() || config.keys.is_some() || config.oidc.is_some() {
            return Err(std::io::Error::other(
                "the trial is open by design: it takes no access token, keys or OIDC",
            ));
        }
        // Whatever licence the server holds, the trial serves the Evaluation tier.
        config.entitlement = Entitlement::evaluation();
    }
    let tls = match &config.tls {
        Some((cert, key)) => Some(tls::server_config(cert, key)?),
        None => None,
    };
    let (jobs, pool, recovered) = match &config.data_dir {
        Some(dir) => {
            let svc = Arc::new(worker::JobService::open(
                dir.clone(),
                config.entitlement.clone(),
            )?);
            if config.ha.is_some() {
                // A node runs jobs only once it holds the lease.
                svc.active.store(false, Ordering::SeqCst);
            }
            let recovered = if config.recover_on_start && config.ha.is_none() {
                svc.recover()
            } else {
                0
            };
            let pool = worker::WorkerPool::spawn(config.workers, svc.clone());
            (Some(svc), Some(pool), recovered)
        }
        None => (None, None, 0),
    };
    let mut authn = auth::Authenticator::new(config.api_token.as_deref(), config.keys.as_ref());
    if let Some(p) = &config.oidc {
        authn = authn.with_oidc(oidc::Verifier::from_file(p).map_err(std::io::Error::other)?);
    }
    let audit = match &config.data_dir {
        Some(dir) => {
            let log = match &config.ha {
                Some(h) => audit::AuditLog::open_dir(dir.join("audit").join(&h.node))?,
                None => audit::AuditLog::open(dir)?,
            };
            log.prune(config.audit_retention_days, now_unix())?;
            Some(log)
        }
        None => None,
    };
    let mut svc = api::Service::with_auth(config.entitlement.clone(), jobs, authn, audit);
    svc.stations_claim = config.stations_claim;
    if let (Some(tc), Some(dir)) = (&config.trial, &config.data_dir) {
        svc.trial = Some(trial::Trial::open(dir, tc.clone())?);
        eprintln!(
            "TRIAL: open to anyone at the Evaluation tier; {} jobs a day per caller, {} s per job, {} requests a minute",
            tc.jobs_per_day, tc.max_run_secs, tc.requests_per_minute
        );
    }
    let service = Arc::new(svc);

    eprintln!(
        "kshana-pro serving on {}://{addr}  (tier: {}, licensee: {})",
        if config.tls.is_some() {
            "https"
        } else {
            "http"
        },
        config.entitlement.tier.as_str(),
        config.entitlement.licensee
    );
    let routes: Vec<String> = api::served_routes(service.jobs.is_some())
        .map(|r| format!("{} {}", r.method, r.path))
        .collect();
    eprintln!("routes: {}", routes.join("  "));
    if let Some(dir) = &config.data_dir {
        eprintln!(
            "job service: data directory {}, {} worker(s), {} unfinished job(s) re-queued",
            dir.display(),
            config.workers.clamp(1, worker::MAX_WORKERS),
            recovered
        );
    }
    if service.has_token() {
        let mut how = Vec::new();
        if config.api_token.is_some() {
            how.push("the access token".to_string());
        }
        if let Some(k) = &config.keys {
            how.push(format!(
                "{} API key(s)",
                k.keys.iter().filter(|k| !k.revoked).count()
            ));
        }
        if config.oidc.is_some() {
            how.push("OIDC tokens".to_string());
        }
        eprintln!(
            "credentials: {} required on every route except /health",
            how.join(", ")
        );
    }
    if let Some(log) = service.audit() {
        eprintln!(
            "audit log: {} ({} days kept)",
            log.dir().display(),
            config.audit_retention_days
        );
    }

    let stop = Arc::new(AtomicBool::new(false));
    let accept = {
        let stop = stop.clone();
        let service = service.clone();
        std::thread::Builder::new()
            .name("kshana-accept".into())
            .spawn(move || accept_loop(listener, service, stop, tls))?
    };
    let ha = match (&config.ha, &service.jobs) {
        (Some(h), Some(j)) => {
            eprintln!(
                "high availability: node {}, lease {} ms, shared data directory",
                h.node, h.lease_ms
            );
            Some(ha::HaNode::start(h.clone(), j.clone()))
        }
        _ => None,
    };
    Ok(ServerHandle {
        addr,
        stop,
        accept: Some(accept),
        pool,
        service,
        ha,
    })
}

/// Start the server and serve until the process is stopped.
pub fn serve_with(config: ServeConfig) -> std::io::Result<()> {
    start(config)?.wait()
}

fn accept_loop(
    listener: TcpListener,
    service: Arc<api::Service>,
    stop: Arc<AtomicBool>,
    tls: Option<Arc<rustls::ServerConfig>>,
) {
    let active = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut s, peer_addr)) => {
                let peer = Some(peer_addr.ip());
                // An accepted socket may inherit the listener's non-blocking mode.
                let _ = s.set_nonblocking(false);
                // Shed load when at the connection ceiling (bounds thread/memory use).
                if active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                    let _ = s.set_write_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS)));
                    let _ = s.write_all(&HttpResponse::error(503, "server busy").to_bytes());
                    continue;
                }
                active.fetch_add(1, Ordering::Relaxed);
                let svc = service.clone();
                let active = active.clone();
                let tls = tls.clone();
                std::thread::spawn(move || {
                    let _guard = ConnGuard(active); // decrements on exit, even on panic
                    let _ = s.set_read_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS)));
                    let _ = s.set_write_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS)));
                    let r = match tls {
                        Some(cfg) => match rustls::ServerConnection::new(cfg) {
                            Ok(conn) => {
                                let mut ts = rustls::StreamOwned::new(conn, s);
                                let r = handle_conn(&mut ts, &svc, peer);
                                ts.conn.send_close_notify();
                                let _ = ts.flush();
                                r
                            }
                            Err(e) => Err(std::io::Error::other(e)),
                        },
                        None => {
                            let mut s = s;
                            handle_conn(&mut s, &svc, peer)
                        }
                    };
                    if let Err(e) = r {
                        eprintln!("connection error: {e}");
                    }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_POLL);
            }
            Err(e) => {
                eprintln!("accept error: {e}");
                std::thread::sleep(ACCEPT_POLL);
            }
        }
    }
}

/// Decrements the active-connection counter when a worker thread ends.
struct ConnGuard(Arc<AtomicUsize>);
impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Probe `addr`'s `/health` route; returns true on a 200. Used by the container
/// HEALTHCHECK so the probe actually exercises the HTTP listener.
pub fn healthcheck(addr: &str) -> bool {
    match TcpStream::connect(addr) {
        Ok(mut s) => {
            let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
            let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
            if s.write_all(b"GET /health HTTP/1.0\r\n\r\n").is_err() {
                return false;
            }
            let mut buf = String::new();
            let _ = s.take(256).read_to_string(&mut buf);
            buf.contains("200 OK")
        }
        Err(_) => false,
    }
}

/// Internal response header that turns a response into a server-sent event stream for
/// the job it names; it is never sent to the client.
pub(crate) const STREAM_HEADER: &str = "X-Kshana-Stream-Job";
/// The longest an event stream stays open.
pub const STREAM_MAX_SECS: u64 = 3600;

/// One server-sent event carrying a job status.
pub(crate) fn sse_event(status: &serde_json::Value) -> String {
    format!("event: status\ndata: {status}\n\n")
}

fn handle_conn<S: Read + Write>(
    stream: &mut S,
    service: &api::Service,
    peer: Option<std::net::IpAddr>,
) -> std::io::Result<()> {
    let mut resp = match read_request_with(&mut *stream, MAX_HEADER_BYTES, body_cap) {
        Ok((req, auth)) => api::dispatch_from(service, &req, auth.as_deref(), now_unix(), peer),
        Err(e) => {
            let status = if e.contains("too large") { 413 } else { 400 };
            HttpResponse::error(status, &e)
        }
    };
    let stream_job = resp
        .headers
        .iter()
        .position(|(k, _)| *k == STREAM_HEADER)
        .map(|i| resp.headers.remove(i).1);
    match (stream_job, service.jobs.as_ref()) {
        (Some(id), Some(jobs)) => stream_job_events(stream, &resp, jobs, &id),
        _ => {
            stream.write_all(&resp.to_bytes())?;
            stream.flush()
        }
    }
}

/// Write a server-sent event stream: the first event (in `first.body`), then one event
/// each time the job's status changes, until it finishes or [`STREAM_MAX_SECS`] pass. A
/// comment line every 15 s keeps proxies from closing an idle stream.
fn stream_job_events<S: Write>(
    stream: &mut S,
    first: &HttpResponse,
    jobs: &worker::JobService,
    id: &str,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n{}",
        first.body
    )?;
    stream.flush()?;
    let start = std::time::Instant::now();
    let mut last = first.body.clone();
    let mut last_write = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(STREAM_MAX_SECS) {
        std::thread::sleep(Duration::from_millis(200));
        let Ok(Some(st)) = jobs.store.status(id) else {
            break;
        };
        let ev = sse_event(&api::status_json(&st));
        if ev != last {
            stream.write_all(ev.as_bytes())?;
            stream.flush()?;
            last = ev;
            last_write = std::time::Instant::now();
        } else if last_write.elapsed() >= Duration::from_secs(15) {
            stream.write_all(b": keep-alive\n\n")?;
            stream.flush()?;
            last_write = std::time::Instant::now();
        }
        if st.state.is_finished() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::{Entitlement, Tier};
    use std::io::Cursor;

    fn req(method: &str, path: &str, body: &str) -> HttpRequest {
        HttpRequest {
            method: method.into(),
            path: path.into(),
            body: body.into(),
        }
    }

    fn enterprise() -> Entitlement {
        Entitlement {
            licensee: "ACME".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
            arksa: false,
        }
    }

    #[test]
    fn health_route() {
        let r = handle(&req("GET", "/health", ""), &Entitlement::evaluation(), 0);
        assert_eq!(r.status, 200);
        assert!(r.body.contains("ok"));
    }

    #[test]
    fn version_and_kinds_routes() {
        let v = handle(&req("GET", "/version", ""), &Entitlement::evaluation(), 0);
        assert_eq!(v.status, 200);
        assert!(v.body.contains("kshana-pro"));
        let k = handle(&req("GET", "/kinds", ""), &Entitlement::evaluation(), 0);
        assert_eq!(k.status, 200);
        assert!(k.body.contains("pro-seam-check"));
        assert!(k.body.contains("\"open\""));
    }

    #[test]
    fn run_route_open_scenario_is_free() {
        let body = "kind = \"ephemeris\"\n\
            tle = \"\"\"\n\
            1 25544U 98067A   20045.18587073  .00000950  00000-0  25302-4 0  9990\n\
            2 25544  51.6443 242.0161 0004885 264.6060 207.3845 15.49165514212791\n\
            \"\"\"\n\
            step_s = 60.0\n\
            duration_s = 120.0\n";
        let r = handle(&req("POST", "/run", body), &Entitlement::evaluation(), 0);
        assert_eq!(
            r.status, 200,
            "open scenario should run on eval: {}",
            r.body
        );
        assert!(r.body.contains("scenario_hash"));
    }

    #[test]
    fn run_route_gates_pro_feature_with_403() {
        // Eval tier may not run a Pro value pack → 403.
        let r = handle(
            &req("POST", "/run", "kind = \"pro-twin\"\n"),
            &Entitlement::evaluation(),
            0,
        );
        assert_eq!(
            r.status, 403,
            "pro-twin on eval must be forbidden: {}",
            r.body
        );
        assert!(r.body.contains("tier"));
    }

    #[test]
    fn run_route_allows_pro_feature_when_entitled() {
        let r = handle(
            &req("POST", "/run", "kind = \"pro-seam-check\"\n"),
            &enterprise(),
            0,
        );
        assert_eq!(r.status, 200);
        assert!(r.body.contains("seam_ok"));
    }

    #[test]
    fn run_route_invalid_input_is_422() {
        // A pro- prefixed but unknown kind is unsupported → here it's gate-denied
        // (Enterprise can't get a non-existent feature)… use a malformed open one.
        let r = handle(
            &req("POST", "/run", "kind = \"ephemeris\"\nstep_s = 10\n"),
            &enterprise(),
            0,
        );
        assert_eq!(
            r.status, 422,
            "missing tle/orbit → invalid input: {}",
            r.body
        );
    }

    #[test]
    fn empty_body_is_422() {
        let r = handle(&req("POST", "/run", ""), &enterprise(), 0);
        assert_eq!(r.status, 422);
    }

    #[test]
    fn unknown_route_is_404() {
        let r = handle(&req("GET", "/nope", ""), &Entitlement::evaluation(), 0);
        assert_eq!(r.status, 404);
    }

    #[test]
    fn query_string_is_stripped_from_path() {
        let r = handle(
            &req("GET", "/health?probe=1", ""),
            &Entitlement::evaluation(),
            0,
        );
        assert_eq!(r.status, 200);
    }

    #[test]
    fn read_request_parses_method_path_and_body() {
        let raw = "POST /run HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello";
        let req = read_request(
            Cursor::new(raw.as_bytes().to_vec()),
            MAX_HEADER_BYTES,
            MAX_BODY_BYTES,
        )
        .unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, "/run");
        assert_eq!(req.body, "hello");
    }

    #[test]
    fn read_request_no_body_when_no_content_length() {
        let raw = "GET /health HTTP/1.1\r\nHost: x\r\n\r\n";
        let req = read_request(
            Cursor::new(raw.as_bytes().to_vec()),
            MAX_HEADER_BYTES,
            MAX_BODY_BYTES,
        )
        .unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/health");
        assert!(req.body.is_empty());
    }

    #[test]
    fn read_request_rejects_oversized_body() {
        let raw = "POST /run HTTP/1.1\r\nContent-Length: 100\r\n\r\n";
        let e =
            read_request(Cursor::new(raw.as_bytes().to_vec()), MAX_HEADER_BYTES, 10).unwrap_err();
        assert!(e.contains("body too large"), "got: {e}");
    }

    #[test]
    fn read_request_rejects_oversized_header() {
        let big = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(100));
        let e = read_request(Cursor::new(big.into_bytes()), 32, MAX_BODY_BYTES).unwrap_err();
        assert!(e.contains("header too large"), "got: {e}");
    }

    #[test]
    fn read_request_rejects_duplicate_content_length() {
        // Request-smuggling guard: two Content-Length headers must be rejected.
        let raw = "POST /run HTTP/1.1\r\nContent-Length: 0\r\nContent-Length: 5\r\n\r\nhello";
        let e = read_request(
            Cursor::new(raw.as_bytes().to_vec()),
            MAX_HEADER_BYTES,
            MAX_BODY_BYTES,
        )
        .unwrap_err();
        assert!(e.contains("malformed"), "got: {e}");
    }

    #[test]
    fn read_request_rejects_malformed_content_length() {
        let raw = "POST /run HTTP/1.1\r\nContent-Length: notanumber\r\n\r\n";
        assert!(read_request(
            Cursor::new(raw.as_bytes().to_vec()),
            MAX_HEADER_BYTES,
            MAX_BODY_BYTES
        )
        .is_err());
    }

    #[test]
    fn healthcheck_false_when_nothing_listening() {
        // Nothing is bound here; the probe must fail fast, not hang.
        assert!(!healthcheck("127.0.0.1:1"));
    }

    #[test]
    fn response_to_bytes_is_well_formed() {
        let r = HttpResponse::json(200, "{\"a\":1}".into());
        let s = String::from_utf8(r.to_bytes()).unwrap();
        assert!(s.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(s.contains("Content-Type: application/json\r\n"));
        assert!(s.contains("Content-Length: 7\r\n"));
        assert!(s.ends_with("\r\n\r\n{\"a\":1}"));
    }
}
