// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The hosted trial sandbox (roadmap F3b), over real sockets: open to anyone at the
//! Evaluation tier whatever licence the server holds, one tenant per caller address,
//! a daily job quota, a forced time limit, a per-minute request limit, and the closed
//! routes.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::trial::TrialConfig;
use kshana_pro::serve::{self, ServeConfig, ServerHandle};
use serde_json::{json, Value};

fn dir(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("service_trial-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/service")
            .join(name),
    )
    .unwrap()
}

fn start(d: &Path, t: TrialConfig) -> ServerHandle {
    // The server holds an Enterprise licence; the trial must still serve Evaluation.
    let mut cfg = ServeConfig::new(
        "127.0.0.1:0",
        Entitlement {
            licensee: "host".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
        },
    );
    cfg.data_dir = Some(d.to_path_buf());
    cfg.trial = Some(t);
    serve::start(cfg).unwrap()
}

fn http(
    a: SocketAddr,
    method: &str,
    path: &str,
    body: &str,
    auth: Option<&str>,
) -> (u16, String, String) {
    let mut s = TcpStream::connect(a).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    let auth = auth
        .map(|k| format!("Authorization: Bearer {k}\r\n"))
        .unwrap_or_default();
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: x\r\n{auth}Content-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    (
        head[9..12].parse().unwrap(),
        head.to_string(),
        body.to_string(),
    )
}

fn job(scenario: &str) -> String {
    json!({"op": "run", "request": {"scenario": scenario}, "timeout_secs": 100000}).to_string()
}

#[test]
fn the_trial_is_open_evaluation_tier_with_quotas() {
    let d = dir("main");
    let srv = start(
        &d,
        TrialConfig {
            jobs_per_day: 3,
            max_run_secs: 30,
            requests_per_minute: 1000,
            max_queued: 100,
        },
    );
    let a = srv.local_addr();

    // Open: no credential; a bogus one is ignored.
    let (st, _, b) = http(
        a,
        "POST",
        "/jobs",
        &job(&fixture("ephemeris.toml")),
        Some("kpk_whatever"),
    );
    assert_eq!(st, 202, "{b}");
    let id = serde_json::from_str::<Value>(&b).unwrap()["job_id"]
        .as_str()
        .unwrap()
        .to_string();
    let t = Instant::now();
    let status = loop {
        let (_, _, b) = http(a, "GET", &format!("/jobs/{id}"), "", None);
        let v: Value = serde_json::from_str(&b).unwrap();
        if v["state"] == "succeeded" || v["state"] == "failed" {
            break v;
        }
        assert!(t.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(status["state"], "succeeded", "{status}");
    assert_eq!(
        status["timeout_secs"], 30,
        "the time limit is capped at the trial's"
    );
    assert!(status["tenant"].as_str().unwrap().starts_with("trial-"));
    assert!(
        !status.to_string().contains("127.0.0.1"),
        "no address is stored or shown"
    );

    // Evaluation tier: a Pro kind is refused although the server holds Enterprise.
    let pro = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/quantum-systematics-sr.toml"),
    )
    .unwrap();
    let (st, _, b) = http(a, "POST", "/jobs", &job(&pro), None);
    assert_eq!(st, 403, "{b}");

    // Resubmitting the same job is free; two more new jobs reach the quota of three.
    assert_eq!(
        http(a, "POST", "/jobs", &job(&fixture("ephemeris.toml")), None).0,
        200
    );
    assert_eq!(
        http(a, "POST", "/jobs", &job(&fixture("passes.toml")), None).0,
        202
    );
    assert_eq!(
        http(a, "POST", "/jobs", &job(&fixture("link-budget.toml")), None).0,
        202
    );
    let (st, head, b) = http(
        a,
        "POST",
        "/jobs",
        &job(&fixture("launch-window.toml")),
        None,
    );
    assert_eq!(st, 429, "{b}");
    assert!(head.contains("Retry-After:"), "{head}");
    assert!(b.contains("3 jobs a day"));

    // Closed routes.
    assert_eq!(
        http(a, "POST", "/run", &fixture("ephemeris.toml"), None).0,
        403
    );
    assert_eq!(http(a, "POST", "/study", "{}", None).0, 403);
    assert_eq!(http(a, "GET", "/ledger", "", None).0, 403);
    assert_eq!(http(a, "GET", "/audit/verify", "", None).0, 403);

    // Another tenant's job (not this caller's) answers as absent.
    let store = kshana_pro::serve::jobs::JobStore::open(&d).unwrap();
    let (other, _) = store
        .submit_for(
            "trial-0000000000000000",
            "run",
            &json!({"scenario": "x"}),
            None,
        )
        .unwrap();
    assert_eq!(
        http(a, "GET", &format!("/jobs/{}", other.job_id), "", None).0,
        404
    );
    let (_, _, list) = http(a, "GET", "/jobs?limit=100", "", None);
    assert!(!list.contains(&other.job_id));
    srv.stop();
}

#[test]
fn the_trial_limits_requests_per_minute_and_refuses_credentials_config() {
    let d = dir("rate");
    let srv = start(
        &d,
        TrialConfig {
            jobs_per_day: 20,
            max_run_secs: 60,
            requests_per_minute: 10,
            max_queued: 100,
        },
    );
    let a = srv.local_addr();
    let codes: Vec<u16> = (0..12)
        .map(|_| http(a, "GET", "/jobs", "", None).0)
        .collect();
    // Unless the minute turned over during the burst, the 11th and 12th are refused.
    let refused = codes.iter().filter(|c| **c == 429).count();
    assert!(refused >= 1, "{codes:?}");
    assert!(
        codes[..10].iter().filter(|c| **c == 200).count() >= 8,
        "{codes:?}"
    );
    // The health probe is never limited.
    assert_eq!(http(a, "GET", "/health", "", None).0, 200);
    srv.stop();

    // A trial with keys or a token is refused: it is open by design.
    let mut cfg = ServeConfig::new("127.0.0.1:0", Entitlement::evaluation());
    cfg.data_dir = Some(dir("conf"));
    cfg.trial = Some(TrialConfig::default());
    cfg.api_token = Some("secret".into());
    assert!(serve::start(cfg).is_err());
}

#[test]
fn trial_tenants_differ_per_address_and_hide_it() {
    let d = dir("tenant");
    std::fs::create_dir_all(&d).unwrap();
    let t = kshana_pro::serve::trial::Trial::open(&d, TrialConfig::default()).unwrap();
    let a = t.tenant_for("192.0.2.1".parse().unwrap());
    let b = t.tenant_for("192.0.2.2".parse().unwrap());
    assert_ne!(a, b);
    assert_eq!(a, t.tenant_for("192.0.2.1".parse().unwrap()));
    assert!(a.starts_with("trial-") && a.len() == 22);
    // The salt persists, so a restart keeps the same tenants (and quota counts).
    let t2 = kshana_pro::serve::trial::Trial::open(&d, TrialConfig::default()).unwrap();
    assert_eq!(a, t2.tenant_for("192.0.2.1".parse().unwrap()));
}
