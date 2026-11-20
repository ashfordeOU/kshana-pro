// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Remote verdicts (owner decision, 19 November 2026)
//!
//! Some methods ship in no Pro build: they run only on Ashforde's hosted verdict service. A
//! licensed client sends the method's inputs with its licence token; the service runs the
//! method and returns a **signed verdict** — the outcome and an Ed25519 signature over it,
//! never the method itself. This module is that client. It holds no method code and names no
//! service: the endpoint is configuration (`--url` or `KSHANA_VERDICT_URL`), and the verdict
//! is trusted only after its signature verifies against a pinned public key.
//!
//! Methods served today: `selfcal` (self-calibrating simulator loop), `survey`
//! (integrity-driven ASF survey planner), `cause` (what caused a vessel or aircraft
//! position episode: jamming, spoofing, self-reported track falsification, equipment fault,
//! or insufficient evidence) and `calibrate` (general parametric calibration of any
//! simulator against telemetry: named parameters with priors and bounds, a closed-form or
//! design-of-runs model; the `selfcal` verdict shape) and `flight-integrity` (a signed
//! per-flight positioning-integrity verdict for a drone flight: the telemetry checks
//! recomputed, the local attestation's hash bound and compared, and whether a degradation was
//! shared by other flights nearby or regional reports, or specific to the aircraft).

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// The methods the verdict service serves, with the `method` name its verdicts carry.
pub const METHODS: &[(&str, &str)] = &[
    ("selfcal", "selfcal"),
    ("survey", "survey-plan"),
    ("cause", "cause"),
    ("calibrate", "calibrate"),
    ("flight-integrity", "flight-integrity"),
    ("rtk-quality", "rtk-quality"),
    ("parametric-trigger", "parametric-trigger"),
    ("policy-wording", "policy-wording"),
    ("spaceweather", "spaceweather"),
    ("perception-eval", "perception-eval"),
    ("feeder-trust", "feeder-trust"),
    ("archive-seal", "archive-seal"),
    ("archive-query", "archive-query"),
    ("vision-claims/register", "claims-protocol"),
    ("vision-claims", "vision-claims"),
];

/// A signed verdict as the service returns it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignedVerdict {
    pub service: String,
    pub method: String,
    /// Evidence class of the outcome (MODELLED, VALIDATED, ...).
    pub evidence_class: String,
    pub utc: String,
    pub key_id: String,
    /// The outcome. The service returns outcomes only.
    pub verdict: Value,
    /// SHA-256 (hex) over the canonical signed body.
    pub verdict_sha256: String,
    /// Ed25519 signature (hex) over the canonical signed body.
    pub signature: String,
}

/// The canonical signed body: every field above `signature`, keys in fixed (sorted) order.
fn canonical(v: &SignedVerdict) -> Vec<u8> {
    let body = json!({
        "service": v.service,
        "method": v.method,
        "evidence_class": v.evidence_class,
        "utc": v.utc,
        "key_id": v.key_id,
        "verdict": v.verdict,
    });
    serde_json::to_vec(&body).unwrap_or_default()
}

/// Verify a verdict against the service's public key (64 hex), or any of several keys
/// separated by commas (during a key rotation the old and new keys are both trusted).
/// Returns the problems; empty means the verdict is intact and was signed by a trusted key.
pub fn verify(v: &SignedVerdict, public_keys_hex: &str) -> Vec<String> {
    let keys: Vec<&str> = public_keys_hex
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .collect();
    if keys.is_empty() {
        return vec!["no trusted key given".into()];
    }
    let mut first = None;
    for k in keys {
        let p = verify_one(v, k);
        if p.is_empty() {
            return p;
        }
        first.get_or_insert(p);
    }
    first.unwrap_or_default()
}

fn verify_one(v: &SignedVerdict, public_key_hex: &str) -> Vec<String> {
    let mut problems = vec![];
    let body = canonical(v);
    if hex::encode(Sha256::digest(&body)) != v.verdict_sha256 {
        problems.push("verdict hash does not match the signed body".into());
    }
    let Some(pk) = hex::decode(public_key_hex.trim())
        .ok()
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .and_then(|b| VerifyingKey::from_bytes(&b).ok())
    else {
        problems.push("trusted key is not 32 valid Ed25519 bytes (64 hex)".into());
        return problems;
    };
    let Some(sig) = hex::decode(&v.signature)
        .ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .map(|b| Signature::from_bytes(&b))
    else {
        problems.push("signature is not 64 hex bytes".into());
        return problems;
    };
    if pk.verify(&body, &sig).is_err() {
        problems.push("signature does not verify against the trusted key".into());
    }
    problems
}

/// The service key(s) built into this release (`KSHANA_VERDICT_KEY` at build time), if any.
pub fn builtin_trusted_keys() -> Option<&'static str> {
    option_env!("KSHANA_VERDICT_KEY").filter(|k| !k.trim().is_empty())
}

/// Sends a request to the verdict service. `Err` is no usable answer.
pub trait VerdictTransport {
    fn post(&self, path: &str, bearer: &str, body: &str) -> Result<String, String>;
}

/// The system `curl`, like the licence check. HTTPS only, except a loopback `http://`
/// address for a local test service. The licence token goes to curl on stdin (never on the
/// command line, where other users could read it) and the body through a temporary file.
pub struct CurlVerdictTransport {
    pub base_url: String,
}

fn allowed_url(base: &str) -> bool {
    base.starts_with("https://")
        || base.starts_with("http://127.0.0.1")
        || base.starts_with("http://localhost")
        || base.starts_with("http://[::1]")
}

impl VerdictTransport for CurlVerdictTransport {
    fn post(&self, path: &str, bearer: &str, body: &str) -> Result<String, String> {
        use std::io::Write;
        let base = self.base_url.trim_end_matches('/');
        if !allowed_url(base) {
            return Err("the verdict service must be an https:// URL (or a loopback http:// address for testing)".into());
        }
        if bearer.contains(['\n', '\r', '"']) {
            return Err("licence token contains invalid characters".into());
        }
        let tmp = std::env::temp_dir().join(format!(
            "kshana-verdict-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&tmp, body).map_err(|e| format!("{}: {e}", tmp.display()))?;
        let mut child = std::process::Command::new("curl")
            .args([
                "-sS",
                "--max-time",
                "600",
                "--config",
                "-",
                "-H",
                "Content-Type: application/json",
                "--data-binary",
                &format!("@{}", tmp.display()),
                "-w",
                "\n%{http_code}",
                &format!("{base}{path}"),
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not run curl: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = writeln!(stdin, "header = \"Authorization: Bearer {bearer}\"");
        }
        let out = child.wait_with_output();
        let _ = std::fs::remove_file(&tmp);
        let out = out.map_err(|e| format!("curl: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let (resp, code) = text.rsplit_once('\n').unwrap_or((text.as_str(), "0"));
        match code.trim().parse::<u16>().unwrap_or(0) {
            200..=299 => Ok(resp.to_string()),
            c => {
                let msg = serde_json::from_str::<Value>(resp)
                    .ok()
                    .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_else(|| resp.trim().to_string());
                Err(format!("verdict service answered {c}: {msg}"))
            }
        }
    }
}

/// Ask the service for a verdict on `method` (one of [`METHODS`]) with the JSON request
/// `body`, authorised by the licence token `bearer`. Checks the answer is a verdict for the
/// method asked. Signature verification is separate ([`verify`]) so the caller chooses the
/// trusted key.
pub fn request(
    t: &dyn VerdictTransport,
    method: &str,
    bearer: &str,
    body: &str,
) -> Result<SignedVerdict, String> {
    let Some((_, expect)) = METHODS.iter().find(|(m, _)| *m == method) else {
        let known: Vec<_> = METHODS.iter().map(|(m, _)| *m).collect();
        return Err(format!(
            "unknown method '{method}' (known: {})",
            known.join(", ")
        ));
    };
    if bearer.trim().is_empty() {
        return Err("a licence token is required (--license)".into());
    }
    serde_json::from_str::<Value>(body).map_err(|e| format!("request is not JSON: {e}"))?;
    let resp = t.post(&format!("/verdict/{method}"), bearer.trim(), body)?;
    let v: SignedVerdict =
        serde_json::from_str(&resp).map_err(|e| format!("answer is not a signed verdict: {e}"))?;
    if v.method != *expect {
        return Err(format!(
            "answer is a verdict for '{}', not '{expect}'",
            v.method
        ));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::cell::RefCell;

    const SEED: [u8; 32] = *b"remote-verdict-client-test-key!!";

    fn pk() -> String {
        hex::encode(SigningKey::from_bytes(&SEED).verifying_key().to_bytes())
    }

    /// A verdict signed exactly as the service signs one.
    fn signed(method: &str, verdict: Value) -> SignedVerdict {
        let mut v = SignedVerdict {
            service: "verdict-service/1".into(),
            method: method.into(),
            evidence_class: "MODELLED".into(),
            utc: "2026-11-19T10:00:00Z".into(),
            key_id: "k1".into(),
            verdict,
            verdict_sha256: String::new(),
            signature: String::new(),
        };
        let body = canonical(&v);
        v.verdict_sha256 = hex::encode(Sha256::digest(&body));
        v.signature = hex::encode(SigningKey::from_bytes(&SEED).sign(&body).to_bytes());
        v
    }

    struct Fake {
        answer: Result<String, String>,
        seen: RefCell<Vec<(String, String)>>,
    }
    impl VerdictTransport for Fake {
        fn post(&self, path: &str, bearer: &str, _body: &str) -> Result<String, String> {
            self.seen.borrow_mut().push((path.into(), bearer.into()));
            self.answer.clone()
        }
    }
    fn fake(answer: Result<String, String>) -> Fake {
        Fake {
            answer,
            seen: RefCell::new(vec![]),
        }
    }

    #[test]
    fn survey_round_trip_verifies() {
        let v = signed(
            "survey-plan",
            json!({"coverage_before": 0.4, "coverage_after": 0.9}),
        );
        let t = fake(Ok(serde_json::to_string(&v).unwrap()));
        let got = request(&t, "survey", "tok", r#"{"config":{}}"#).unwrap();
        assert_eq!(got, v);
        assert!(verify(&got, &pk()).is_empty(), "{:?}", verify(&got, &pk()));
        assert_eq!(
            t.seen.borrow()[0],
            ("/verdict/survey".to_string(), "tok".to_string())
        );
    }

    #[test]
    fn cause_round_trip_verifies() {
        let v = signed(
            "cause",
            json!({
                "classification": "external_spoofing",
                "confidence": 0.9,
                "questions": {"reported_track": "false", "origin": "external"},
            }),
        );
        let t = fake(Ok(serde_json::to_string(&v).unwrap()));
        let got = request(&t, "cause", "tok", r#"{"platform":"vessel","reported":[]}"#).unwrap();
        assert_eq!(got, v);
        assert!(verify(&got, &pk()).is_empty());
        assert_eq!(t.seen.borrow()[0].0, "/verdict/cause");
        // A verdict of another method is not accepted as a cause verdict, nor the reverse.
        let other = serde_json::to_string(&signed("survey-plan", json!({}))).unwrap();
        assert!(request(&fake(Ok(other)), "cause", "tok", "{}")
            .unwrap_err()
            .contains("not 'cause'"));
        let cause = serde_json::to_string(&v).unwrap();
        assert!(request(&fake(Ok(cause)), "survey", "tok", "{}")
            .unwrap_err()
            .contains("not 'survey-plan'"));
    }

    #[test]
    fn calibrate_round_trip_verifies() {
        let v = signed(
            "calibrate",
            json!({
                "converged": true,
                "stop_reason": "converged",
                "passes": 7,
                "fidelity_first": 40.0,
                "fidelity_last": 96.0,
                "posterior": [{"name": "tau", "value": 1800.0, "std_err": 12.0}],
                "lineage": {"head": "ab".repeat(32), "entries": 2},
            }),
        );
        let t = fake(Ok(serde_json::to_string(&v).unwrap()));
        let got = request(&t, "calibrate", "tok", r#"{"parameters":[]}"#).unwrap();
        assert_eq!(got, v);
        assert!(verify(&got, &pk()).is_empty());
        assert_eq!(t.seen.borrow()[0].0, "/verdict/calibrate");
        // A selfcal verdict is not accepted as a calibrate verdict.
        let other = serde_json::to_string(&signed("selfcal", json!({}))).unwrap();
        assert!(request(&fake(Ok(other)), "calibrate", "tok", "{}")
            .unwrap_err()
            .contains("not 'calibrate'"));
    }

    #[test]
    fn methods_are_unique() {
        let mut names: Vec<_> = METHODS.iter().map(|(m, _)| *m).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), METHODS.len());
        assert!(names.contains(&"cause") && names.contains(&"calibrate"));
    }

    #[test]
    fn tampered_or_foreign_verdicts_fail() {
        let v = signed("selfcal", json!({"converged": true}));
        let mut t = v.clone();
        t.verdict["converged"] = json!(false);
        assert!(!verify(&t, &pk()).is_empty(), "tampered outcome must fail");
        let other = hex::encode(
            SigningKey::from_bytes(&[7u8; 32])
                .verifying_key()
                .to_bytes(),
        );
        assert!(!verify(&v, &other).is_empty(), "wrong key must fail");
        assert!(!verify(&v, "zz").is_empty(), "bad key must fail");
        assert!(!verify(&v, "").is_empty(), "no key must fail");
        // During a rotation either trusted key is accepted.
        assert!(verify(&v, &format!("{other},{}", pk())).is_empty());
        assert!(verify(&v, &format!("{}, {other}", pk())).is_empty());
        assert!(!verify(&t, &format!("{other},{}", pk())).is_empty());
    }

    #[test]
    fn request_checks_inputs_and_answers() {
        let v = signed("selfcal", json!({}));
        let ok = serde_json::to_string(&v).unwrap();
        assert!(request(&fake(Ok(ok.clone())), "nope", "tok", "{}")
            .unwrap_err()
            .contains("unknown method"));
        assert!(request(&fake(Ok(ok.clone())), "selfcal", " ", "{}")
            .unwrap_err()
            .contains("licence token"));
        assert!(request(&fake(Ok(ok.clone())), "selfcal", "tok", "not json")
            .unwrap_err()
            .contains("not JSON"));
        // A verdict for another method is refused.
        assert!(request(&fake(Ok(ok)), "survey", "tok", "{}")
            .unwrap_err()
            .contains("not 'survey-plan'"));
        assert!(request(&fake(Ok("{}".into())), "selfcal", "tok", "{}")
            .unwrap_err()
            .contains("not a signed verdict"));
        assert_eq!(
            request(&fake(Err("offline".into())), "selfcal", "tok", "{}").unwrap_err(),
            "offline"
        );
    }

    #[test]
    fn curl_transport_refuses_plain_http_and_bad_tokens() {
        let t = CurlVerdictTransport {
            base_url: "http://example.com".into(),
        };
        assert!(t
            .post("/verdict/survey", "tok", "{}")
            .unwrap_err()
            .contains("https"));
        let t = CurlVerdictTransport {
            base_url: "https://example.invalid".into(),
        };
        assert!(t
            .post("/verdict/survey", "to\"k", "{}")
            .unwrap_err()
            .contains("invalid characters"));
    }
}
