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
            arksa: false,
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
            ..TrialConfig::default()
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
            ..TrialConfig::default()
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

fn post(a: SocketAddr, path: &str, body: &str) -> (u16, String, String) {
    http(a, "POST", path, body, None)
}

#[test]
fn trial_tokens_last_fourteen_days_one_per_caller_and_ask_for_nothing() {
    use kshana_pro::license::{public_key_for, verify_with, Keyring, VerifyOptions};
    const SEED: [u8; 32] = [0x5a; 32];
    let d = dir("tokens");
    let mut cfg = ServeConfig::new("127.0.0.1:0", Entitlement::evaluation());
    cfg.data_dir = Some(d.clone());
    cfg.trial = Some(TrialConfig {
        tokens_per_day: 5,
        ..TrialConfig::default()
    });
    cfg.trial_signer = Some(std::sync::Arc::new(SEED));
    let srv = serve::start(cfg).unwrap();
    let a = srv.local_addr();

    // Nothing but an optional machine hash is accepted: no name, no e-mail.
    let (st, _, b) = post(a, "/trial/token", r#"{"email": "someone@example.com"}"#);
    assert_eq!(st, 422, "{b}");
    assert_eq!(post(a, "/trial/token", r#"{"machine": "nothex"}"#).0, 422);
    assert_eq!(http(a, "GET", "/trial/token", "", None).0, 405);

    let (st, _, b) = post(a, "/trial/token", "");
    assert_eq!(st, 201, "{b}");
    let v: Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["days"], 14);
    let token = v["token"].as_str().unwrap();
    let ring = Keyring::single(public_key_for(&SEED));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let lic = verify_with(&ring, token, &VerifyOptions::at(now)).unwrap();
    assert!(lic.claims.trial);
    assert_eq!(lic.claims.tier, "pro");
    assert_eq!(
        lic.claims.expires_unix - lic.claims.issued_unix,
        14 * 86_400
    );
    assert!(lic.claims.licensee.starts_with("Kshana Pro trial trial-"));
    assert!(!token.contains("127.0.0.1"));
    // It expires: refused fifteen days on.
    assert!(verify_with(&ring, token, &VerifyOptions::at(now + 15 * 86_400)).is_err());

    // One per caller: the same address is refused, with Retry-After.
    let (st, head, b) = post(a, "/trial/token", "");
    assert_eq!(st, 429, "{b}");
    assert!(head.contains("Retry-After:"));
    assert!(b.contains("one trial token per caller"));
    srv.stop();

    // The server keeps only salted tenant hashes and times.
    let kept = std::fs::read_to_string(d.join("trial-usage.json")).unwrap();
    assert!(!kept.contains("127.0.0.1"), "{kept}");

    // A server-wide daily cap, even for new callers (cooldown off, cap one).
    let d2 = dir("tokens-cap");
    let mut cfg = ServeConfig::new("127.0.0.1:0", Entitlement::evaluation());
    cfg.data_dir = Some(d2);
    cfg.trial = Some(TrialConfig {
        tokens_per_day: 1,
        token_cooldown_days: 0,
        ..TrialConfig::default()
    });
    cfg.trial_signer = Some(std::sync::Arc::new(SEED));
    let srv = serve::start(cfg).unwrap();
    assert_eq!(post(srv.local_addr(), "/trial/token", "").0, 201);
    let (st, _, b) = post(srv.local_addr(), "/trial/token", "");
    assert_eq!(st, 429);
    assert!(b.contains("today's trial tokens"));
    srv.stop();

    // Without a signer the route is closed.
    let srv = start(&dir("tokens-off"), TrialConfig::default());
    assert_eq!(post(srv.local_addr(), "/trial/token", "").0, 404);
    srv.stop();
}

#[test]
fn a_trial_token_longer_than_fourteen_days_is_refused_by_every_build() {
    use kshana_pro::license::{
        issue_v2, public_key_for, verify_with, Keyring, LicenseClaims, VerifyOptions,
    };
    const SEED: [u8; 32] = [0x5b; 32];
    let ring = Keyring::single(public_key_for(&SEED));
    let t0 = 1_800_000_000;
    let mk = |days: i64, issued: i64| {
        issue_v2(
            &SEED,
            &LicenseClaims {
                licensee: "x".into(),
                tier: "pro".into(),
                expires_unix: t0 + days * 86_400,
                products: vec!["cli".into()],
                issued_unix: issued,
                trial: true,
                ..Default::default()
            },
        )
        .unwrap()
    };
    assert!(verify_with(&ring, &mk(14, t0), &VerifyOptions::at(t0 + 1)).is_ok());
    assert!(verify_with(&ring, &mk(30, t0), &VerifyOptions::at(t0 + 1)).is_err());
    assert!(verify_with(&ring, &mk(7, 0), &VerifyOptions::at(t0 + 1)).is_err());
}

#[test]
fn trial_jobs_are_deleted_after_retention_and_the_quota_stays_counted() {
    use kshana_pro::serve::jobs::JobState;
    use kshana_pro::serve::trial::Trial;
    use kshana_pro::serve::worker::JobService;
    let d = dir("retention");
    std::fs::create_dir_all(&d).unwrap();
    let t = Trial::open(
        &d,
        TrialConfig {
            jobs_per_day: 1,
            retention_secs: 3600,
            ..TrialConfig::default()
        },
    )
    .unwrap();
    let js = JobService::open(d.clone(), Entitlement::evaluation()).unwrap();
    let t0 = 1_800_000_000;
    let tenant = "trial-00000000000000aa";
    let mut ids = Vec::new();
    for (who, scenario) in [
        (tenant, "customer scenario"),
        ("acme", "a paying tenant's job"),
    ] {
        let (st, _) = js
            .store
            .submit_for(who, "run", &json!({ "scenario": scenario }), None)
            .unwrap();
        let mut done = st.clone();
        done.state = JobState::Succeeded;
        done.finished_unix = Some(t0);
        js.store.write_status(&done).unwrap();
        ids.push(st.job_id);
    }
    t.record_job(tenant, t0).unwrap();
    assert_eq!(t.purge_now(&js, t0 + 10), 0, "kept within retention");
    assert_eq!(t.purge_now(&js, t0 + 3600), 1, "only the trial job goes");
    assert!(js.store.status(&ids[0]).unwrap().is_none());
    assert!(!js.store.job_dir(&ids[0]).exists());
    assert!(
        js.store.status(&ids[1]).unwrap().is_some(),
        "non-trial jobs stay"
    );
    // Deleting the job does not refund the day's quota.
    let r = t.may_submit(&js, tenant, t0 + 3600);
    assert_eq!(r.err().map(|r| r.status), Some(429));
    // An unfinished trial job is deleted a day after submission.
    let (st, _) = js
        .store
        .submit_for(tenant, "run", &json!({ "scenario": "stuck" }), None)
        .unwrap();
    assert_eq!(t.purge_now(&js, st.submitted_unix + 86_400), 1);
}
