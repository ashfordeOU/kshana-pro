// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The one outbound call in payments: a Stripe API POST through the system `curl`, used
//! only by [`super::stripe::create_checkout_session`], which refuses before getting here
//! while `[stripe] enabled` is false. The secret key is passed to curl on its standard
//! input (`-K -`), never on the command line, so it does not show in the process list.

use std::io::Write;
use std::process::Stdio;

use super::stripe::StripeTransport;

/// `curl`, HTTPS only, 30 s.
pub struct CurlStripe;

fn quote(v: &str) -> String {
    v.replace('\\', "\\\\").replace('"', "\\\"")
}

impl StripeTransport for CurlStripe {
    fn post_form(
        &self,
        url: &str,
        secret_key: &str,
        idempotency_key: &str,
        form: &[(String, String)],
    ) -> Result<String, String> {
        if !url.starts_with("https://api.stripe.com/") {
            return Err("Stripe calls go to https://api.stripe.com only".into());
        }
        if secret_key.contains(['\n', '\r', '"']) || idempotency_key.contains(['\n', '\r', '"']) {
            return Err("bad key".into());
        }
        let mut cfg = String::new();
        cfg.push_str(&format!(
            "header = \"Authorization: Bearer {secret_key}\"\n"
        ));
        cfg.push_str(&format!(
            "header = \"Idempotency-Key: {idempotency_key}\"\n"
        ));
        for (k, v) in form {
            cfg.push_str(&format!("data-urlencode = \"{}={}\"\n", quote(k), quote(v)));
        }
        let mut child = std::process::Command::new("curl")
            .args([
                "-sS",
                "--proto",
                "=https",
                "--max-time",
                "30",
                "-K",
                "-",
                url,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not run curl: {e}"))?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(cfg.as_bytes())
            .map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}
