// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-core`: the Kshana-hosted Core service binary (D-K11). Built only with the
//! `core-methods` feature, so it exists only in Ashforde's hosted build (and vetted Sovereign
//! deployments) and never in the shipped `kshana-pro` binary, Desktop Pro or the packages.
//!
//! It serves two Core methods over HTTP, each returning a signed verdict and nothing else:
//! `POST /core/selfcal` (F1, the self-calibrating simulator loop) and `POST /core/survey`
//! (F2, the integrity-driven ASF survey planner). One request per connection, `Content-Length`
//! framed; put it behind a reverse proxy for TLS and rate limiting.
//!
//! Configuration (environment):
//! * `KSHANA_CORE_ADDR` — listen address (default `127.0.0.1:8099`).
//! * `KSHANA_CORE_KEY` — 64-hex Ed25519 seed that signs verdicts (required).
//! * `KSHANA_CORE_KEY_ID` — key identifier put in the verdict (default `core`).
//! * `KSHANA_CORE_TOKEN` — if set, every request must carry `Authorization: Bearer <token>`.

use kshana_pro::core_service::{selfcal_verdict, survey_verdict, SelfCalRequest, SurveyRequest};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

const MAX_BODY: usize = 8 * 1024 * 1024;

fn env_var(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        sod / 3600,
        sod % 3600 / 60,
        sod % 60
    )
}

fn respond(stream: &mut TcpStream, status: u16, reason: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn err(stream: &mut TcpStream, status: u16, reason: &str, msg: &str) {
    respond(
        stream,
        status,
        reason,
        &serde_json::json!({ "error": msg }).to_string(),
    );
}

fn handle(mut stream: TcpStream, seed: &[u8; 32], key_id: &str, token: Option<&str>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let (mut content_length, mut auth) = (0usize, String::new());
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h == "\r\n" || h == "\n" || h.is_empty() {
            break;
        }
        let lower = h.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        } else if lower.starts_with("authorization:") {
            auth = h["authorization:".len()..].trim().to_string();
        }
    }
    if let Some(tok) = token {
        if auth != format!("Bearer {tok}") {
            return err(
                &mut stream,
                401,
                "Unauthorized",
                "missing or wrong bearer token",
            );
        }
    }
    if content_length > MAX_BODY {
        return err(&mut stream, 413, "Payload Too Large", "body above the cap");
    }
    let mut body = vec![0u8; content_length];
    if reader.read_exact(&mut body).is_err() {
        return err(&mut stream, 400, "Bad Request", "short body");
    }
    if path == "/health" {
        return respond(&mut stream, 200, "OK", "{\"ok\":true}");
    }
    if method != "POST" {
        return err(&mut stream, 405, "Method Not Allowed", "use POST");
    }
    let utc = now_iso();
    let out = match path.as_str() {
        "/core/selfcal" => serde_json::from_slice::<SelfCalRequest>(&body)
            .map_err(|e| format!("request: {e}"))
            .and_then(|r| selfcal_verdict(&r, seed, key_id, &utc)),
        "/core/survey" => serde_json::from_slice::<SurveyRequest>(&body)
            .map_err(|e| format!("request: {e}"))
            .and_then(|r| survey_verdict(&r, seed, key_id, &utc)),
        _ => return err(&mut stream, 404, "Not Found", "unknown path"),
    };
    match out {
        Ok(v) => respond(
            &mut stream,
            200,
            "OK",
            &serde_json::to_string(&v).unwrap_or_default(),
        ),
        Err(e) => err(&mut stream, 422, "Unprocessable Entity", &e),
    }
}

fn main() {
    let addr = env_var("KSHANA_CORE_ADDR").unwrap_or_else(|| "127.0.0.1:8099".into());
    let key_hex = match env_var("KSHANA_CORE_KEY") {
        Some(k) => k,
        None => {
            eprintln!("KSHANA_CORE_KEY (64-hex Ed25519 seed) is required");
            std::process::exit(2);
        }
    };
    let seed: [u8; 32] = match hex::decode(key_hex.trim())
        .ok()
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
    {
        Some(s) => s,
        None => {
            eprintln!("KSHANA_CORE_KEY must be 64 hex characters");
            std::process::exit(2);
        }
    };
    let key_id = env_var("KSHANA_CORE_KEY_ID").unwrap_or_else(|| "core".into());
    let token = env_var("KSHANA_CORE_TOKEN");
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "kshana-core listening on {addr} (signing key {key_id}){}",
        if token.is_some() {
            ", bearer-token required"
        } else {
            ""
        }
    );
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                let (s, k, t) = (seed, key_id.clone(), token.clone());
                std::thread::spawn(move || handle(stream, &s, &k, t.as_deref()));
            }
            Err(e) => eprintln!("accept: {e}"),
        }
    }
}
