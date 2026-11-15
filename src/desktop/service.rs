// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The Desktop's connection to a Kshana Pro service (the Station fleet, the area console, the
//! timing fleet, the team library, the audit log, the watch wall, SCIM).
//!
//! The base URL and the credential (an API key, or an OIDC token from the organisation's
//! single sign-on) are kept in `<licence home>/service.json`, readable by the owner only; the
//! credential is never returned to the page. A plain `http://` URL is accepted only for this
//! machine (`localhost`, `127.0.0.1`, `[::1]`), so a credential never crosses a network in
//! clear. Requests go through the system `curl`, the credential on its standard input, never
//! on the command line (as the payments transport does).

use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The connection file, in the licence home.
pub const FILE: &str = "service.json";

/// One HTTP exchange: (status, body).
pub trait Transport {
    fn send(
        &self,
        method: &str,
        url: &str,
        credential: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), String>;
}

/// The system `curl`, 30 s.
pub struct CurlTransport;

/// A value in curl's config-file syntax (a double-quoted string).
fn quoted(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// The curl configuration for one request (what goes to curl's standard input).
pub fn curl_config(method: &str, url: &str, credential: &str, body: Option<&str>) -> String {
    let mut c = format!(
        "url = {}\nrequest = {}\nheader = {}\nsilent\nshow-error\nmax-time = 30\nproto = \"=https,http\"\nwrite-out = \"\\n%{{http_code}}\"\n",
        quoted(url),
        quoted(method),
        quoted(&format!("Authorization: Bearer {credential}"))
    );
    if let Some(b) = body {
        c.push_str(&format!(
            "header = \"Content-Type: application/json\"\ndata-binary = {}\n",
            quoted(b)
        ));
    }
    c
}

impl Transport for CurlTransport {
    fn send(
        &self,
        method: &str,
        url: &str,
        credential: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), String> {
        let mut child = std::process::Command::new("curl")
            .args(["-K", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("curl could not start: {e}"))?;
        child
            .stdin
            .take()
            .ok_or("curl has no input")?
            .write_all(curl_config(method, url, credential, body).as_bytes())
            .map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "the service could not be reached: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
        let status = code
            .trim()
            .parse()
            .map_err(|_| "no HTTP status from curl")?;
        Ok((status, body.to_string()))
    }
}

/// A remembered connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub base: String,
    credential: String,
}

/// True when `base` is an `https://` URL, or `http://` to this machine.
pub fn base_ok(base: &str) -> bool {
    let local = ["http://localhost", "http://127.0.0.1", "http://[::1]"];
    let host_end = |rest: &str| rest.is_empty() || rest.starts_with(':') || rest.starts_with('/');
    if let Some(rest) = base.strip_prefix("https://") {
        return !rest.is_empty() && !rest.starts_with('/') && !base.contains(char::is_whitespace);
    }
    local.iter().any(|l| {
        base.strip_prefix(l)
            .is_some_and(|rest| host_end(rest) && !base.contains(char::is_whitespace))
    })
}

/// Remember the service at `base` with `credential` (checked by a `GET /whoami` first is the
/// page's business; this only stores).
pub fn connect(home: &Path, base: &str, credential: &str) -> Result<Connection, String> {
    let base = base.trim().trim_end_matches('/').to_string();
    if !base_ok(&base) {
        return Err(
            "the service address must be https://…, or http:// to this machine only".into(),
        );
    }
    let credential = credential.trim().to_string();
    if credential.is_empty()
        || credential
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(
            "the credential is one line with no spaces (an API key or an SSO token)".into(),
        );
    }
    let c = Connection { base, credential };
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    super::write_private(
        &home.join(FILE),
        (serde_json::to_string_pretty(&c).unwrap_or_default() + "\n").as_bytes(),
    )?;
    Ok(c)
}

/// The remembered connection, if any.
pub fn load(home: &Path) -> Option<Connection> {
    serde_json::from_slice(&std::fs::read(home.join(FILE)).ok()?).ok()
}

/// Forget the connection. Returns whether there was one.
pub fn disconnect(home: &Path) -> Result<bool, String> {
    match std::fs::remove_file(home.join(FILE)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

/// The credential's kind, for display: `sso` for a token with three dot-separated parts.
fn credential_kind(c: &str) -> &'static str {
    if c.matches('.').count() == 2 {
        "sso"
    } else {
        "api-key"
    }
}

impl Connection {
    /// What the page may see: never the credential.
    pub fn public_json(&self) -> Value {
        json!({"connected": true, "base": self.base, "credential": credential_kind(&self.credential)})
    }

    /// One request. `path` starts with `/` and holds no `..`; the answer is
    /// `{status, json}` (or `{status, text}` for a body that is not JSON).
    pub fn request(
        &self,
        t: &dyn Transport,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<Value, String> {
        if !["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method) {
            return Err(format!("{method} is not a method the Desktop sends"));
        }
        if !path.starts_with('/')
            || path.contains("..")
            || path.contains("//")
            || path.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(format!("'{path}' is not a service path"));
        }
        let (status, text) = t.send(
            method,
            &format!("{}{path}", self.base),
            &self.credential,
            body,
        )?;
        Ok(match serde_json::from_str::<Value>(&text) {
            Ok(v) => json!({"status": status, "json": v}),
            Err(_) => json!({"status": status, "text": text}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_or_this_machine() {
        for ok in [
            "https://kshana.example.org",
            "https://10.0.0.5:8443",
            "http://localhost:8080",
            "http://127.0.0.1",
            "http://[::1]:9",
        ] {
            assert!(base_ok(ok), "{ok}");
        }
        for bad in [
            "http://kshana.example.org",
            "http://localhost.evil.org",
            "ftp://x",
            "https://",
            "https:///x",
            "https://a b",
        ] {
            assert!(!base_ok(bad), "{bad}");
        }
    }

    #[test]
    fn the_curl_config_quotes_everything() {
        let c = curl_config("POST", "https://h/x", "k\"y", Some("{\"a\":\"b\\n\"}\n"));
        assert!(c.contains("header = \"Authorization: Bearer k\\\"y\""));
        assert!(c.contains("data-binary = \"{\\\"a\\\":\\\"b\\\\n\\\"}\\n\""));
        assert!(!c.contains("\nk\"y"));
    }
}
