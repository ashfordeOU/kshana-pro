// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The webhook listener: `POST /webhooks/paddle`, `POST /webhooks/lemonsqueezy`,
//! `GET /health`. Blocking, standard library only, like the Pro service; the raw body
//! is kept as bytes because the signature is over the exact bytes sent. TLS is
//! terminated in front of it (a reverse proxy), as for the Pro service.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::config::Provider;
use super::webhook::{Issuer, Outcome};

/// Header and body caps for a webhook (providers send small JSON bodies).
pub const MAX_HEADER: usize = 16 * 1024;
pub const MAX_BODY: usize = 512 * 1024;

/// A request as read off the socket.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RawRequest {
    /// The first header named `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Read one request, enforcing the caps; a duplicate or malformed Content-Length is
/// refused.
pub fn read_raw<R: Read>(mut r: R) -> Result<RawRequest, String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        if buf.len() >= MAX_HEADER {
            return Err("header too large".into());
        }
        if r.read(&mut byte).map_err(|e| e.to_string())? == 0 {
            return Err("connection closed".into());
        }
        buf.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&buf[..buf.len() - 4]).to_string();
    let mut lines = head.split("\r\n");
    let mut rl = lines.next().ok_or("empty request")?.split_whitespace();
    let method = rl.next().ok_or("no method")?.to_string();
    let path = rl.next().ok_or("no path")?.to_string();
    let mut headers = Vec::new();
    let mut clen: Option<usize> = None;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            let (k, v) = (k.trim().to_string(), v.trim().to_string());
            if k.eq_ignore_ascii_case("content-length") {
                if clen.is_some() {
                    return Err("duplicate Content-Length".into());
                }
                clen = Some(v.parse().map_err(|_| "bad Content-Length")?);
            }
            headers.push((k, v));
        }
    }
    let n = clen.unwrap_or(0);
    if n > MAX_BODY {
        return Err("body too large".into());
    }
    let mut body = vec![0u8; n];
    r.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(RawRequest {
        method,
        path,
        headers,
        body,
    })
}

/// Route one request (pure apart from the issuer's files).
pub fn route(issuer: &Issuer, req: &RawRequest, now: i64) -> Outcome {
    let path = req.path.split('?').next().unwrap_or("");
    match (req.method.as_str(), path) {
        ("GET", "/health") => Outcome {
            status: 200,
            message: "ok".into(),
            token_id: None,
            revoked: None,
        },
        ("POST", "/webhooks/paddle") => issuer.handle(
            Provider::Paddle,
            req.header("Paddle-Signature"),
            &req.body,
            now,
        ),
        ("POST", "/webhooks/lemonsqueezy") => issuer.handle(
            Provider::LemonSqueezy,
            req.header("X-Signature"),
            &req.body,
            now,
        ),
        _ => Outcome {
            status: 404,
            message: "not found".into(),
            token_id: None,
            revoked: None,
        },
    }
}

fn respond(s: &mut TcpStream, o: &Outcome) {
    let body = serde_json::json!({
        "status": if o.status < 300 { "ok" } else { "error" },
        "message": o.message,
    })
    .to_string();
    let reason = match o.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Error",
    };
    let _ = write!(
        s,
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        o.status,
        body.len()
    );
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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

/// Start listening. Deliveries are handled one at a time (the register and state are
/// single-writer), which is ample for webhook rates.
pub fn start(issuer: Issuer, addr: &str) -> std::io::Result<Listener> {
    let l = TcpListener::bind(addr)?;
    let addr = l.local_addr()?;
    l.set_nonblocking(true)?;
    let issuer = Arc::new(Mutex::new(issuer));
    let stop = Arc::new(AtomicBool::new(false));
    let st = stop.clone();
    let thread = std::thread::spawn(move || {
        while !st.load(Ordering::SeqCst) {
            match l.accept() {
                Ok((mut s, _)) => {
                    let _ = s.set_nonblocking(false);
                    let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
                    let _ = s.set_write_timeout(Some(Duration::from_secs(30)));
                    let o = match read_raw(&mut s) {
                        Ok(req) => {
                            let guard = issuer.lock().unwrap_or_else(|p| p.into_inner());
                            route(&guard, &req, now_unix())
                        }
                        Err(e) => Outcome {
                            status: 400,
                            message: e,
                            token_id: None,
                            revoked: None,
                        },
                    };
                    respond(&mut s, &o);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
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
