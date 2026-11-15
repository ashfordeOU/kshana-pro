// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The licence and pack service (vendor side; `kshana-pro packs serve`). Blocking,
//! standard library only, like the payments listener; TLS is terminated in front of it.
//!
//! | Route | Body | Answer |
//! |---|---|---|
//! | `POST /v1/licence/activate` | `{"token", "device"}` | `{"token"}`: a token bound to the device (one seat) |
//! | `POST /v1/licence/check` | [`crate::license::online::CheckRequest`] | the signed check answer (text) |
//! | `GET /v1/revocations` | | the signed revocation list (text) |
//! | `POST /v1/packs/fetch` | `{"token", "device", "pack_id", "platform", "version"?}` | the pack file; header `Kshana-Pack-Secret` carries the licence secret |
//! | `GET /health` | | `ok` |

use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::issuer::PackIssuer;
use crate::license::lifecycle::LicenceService;
use crate::license::online::CheckRequest;
use crate::payments::server::{read_raw, RawRequest};

/// The header carrying the licence secret with a pack.
pub const SECRET_HEADER: &str = "Kshana-Pack-Secret";

/// One answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    fn text(status: u16, body: impl Into<String>) -> Reply {
        Reply {
            status,
            content_type: "text/plain; charset=utf-8",
            headers: Vec::new(),
            body: body.into().into_bytes(),
        }
    }
    fn json(status: u16, v: serde_json::Value) -> Reply {
        Reply {
            status,
            content_type: "application/json",
            headers: Vec::new(),
            body: v.to_string().into_bytes(),
        }
    }
    fn error(status: u16, msg: impl std::fmt::Display) -> Reply {
        Reply::json(
            status,
            serde_json::json!({ "status": "error", "message": msg.to_string() }),
        )
    }
}

/// The service state.
pub struct Service {
    pub licences: LicenceService,
    pub packs: PackIssuer,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Activate {
    token: String,
    device: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Fetch {
    token: String,
    device: String,
    pack_id: String,
    platform: String,
    #[serde(default)]
    version: Option<String>,
}

/// Route one request.
pub fn route(svc: &Service, req: &RawRequest, now: i64) -> Reply {
    let path = req.path.split('?').next().unwrap_or("");
    match (req.method.as_str(), path) {
        ("GET", "/health") => Reply::text(200, "ok"),
        ("GET", "/v1/revocations") => match svc.licences.revocation_list(now) {
            Ok(t) => Reply::text(200, t),
            Err(e) => Reply::error(500, e),
        },
        ("POST", "/v1/licence/activate") => {
            let Ok(a) = serde_json::from_slice::<Activate>(&req.body) else {
                return Reply::error(400, "body must be {\"token\", \"device\"}");
            };
            match svc.licences.activate(&a.token, &a.device, now) {
                Ok(i) => Reply::json(
                    200,
                    serde_json::json!({ "token": i.token, "expires_unix": i.expires_unix }),
                ),
                Err(e) => Reply::error(403, e),
            }
        }
        ("POST", "/v1/licence/check") => {
            let Ok(c) = serde_json::from_slice::<CheckRequest>(&req.body) else {
                return Reply::error(400, "body must be a licence check request");
            };
            match svc.licences.check(&c, now) {
                Ok(t) => Reply::text(200, t),
                Err(e) => Reply::error(403, e),
            }
        }
        ("POST", "/v1/packs/fetch") => {
            let Ok(f) = serde_json::from_slice::<Fetch>(&req.body) else {
                return Reply::error(
                    400,
                    "body must be {\"token\", \"device\", \"pack_id\", \"platform\"}",
                );
            };
            match svc.packs.issue(
                &svc.licences,
                &f.token,
                &f.device,
                &f.pack_id,
                &f.platform,
                f.version.as_deref(),
                now,
            ) {
                Ok(p) => Reply {
                    status: 200,
                    content_type: "application/octet-stream",
                    headers: vec![(SECRET_HEADER, p.licence_secret_hex)],
                    body: p.file,
                },
                Err(e) => Reply::error(403, e),
            }
        }
        _ => Reply::error(404, "not found"),
    }
}

fn respond(s: &mut TcpStream, r: &Reply) {
    let reason = match r.status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        r.status,
        r.content_type,
        r.body.len()
    );
    for (k, v) in &r.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(&r.body);
}

/// A running listener.
pub struct Listener {
    pub addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
    pub fn wait(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Start listening; requests are handled one at a time (the register and ledger are
/// single-writer).
pub fn start(svc: Service, addr: &str) -> std::io::Result<Listener> {
    let l = TcpListener::bind(addr)?;
    let addr = l.local_addr()?;
    l.set_nonblocking(true)?;
    let svc = Arc::new(Mutex::new(svc));
    let stop = Arc::new(AtomicBool::new(false));
    let st = stop.clone();
    let thread = std::thread::spawn(move || {
        while !st.load(Ordering::SeqCst) {
            match l.accept() {
                Ok((mut s, _)) => {
                    let _ = s.set_nonblocking(false);
                    let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
                    let _ = s.set_write_timeout(Some(Duration::from_secs(60)));
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    let r = match read_raw(&mut s) {
                        Ok(req) => {
                            let g = svc.lock().unwrap_or_else(|p| p.into_inner());
                            route(&g, &req, now)
                        }
                        Err(e) => Reply::error(400, e),
                    };
                    respond(&mut s, &r);
                }
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    });
    Ok(Listener {
        addr,
        stop,
        thread: Some(thread),
    })
}
