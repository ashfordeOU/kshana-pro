// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Tests for the study-request intake schema + validation + deterministic study id
//! (P6). The pure `validate` and `study_id` helpers are exercised directly; a small
//! routing test confirms `POST /study` is wired into the std-only HTTP handler with
//! the same tier gating `/run` uses.

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::study_intake::{study_id, validate, StudyRequest};
use serde_json::json;

fn pro() -> Entitlement {
    Entitlement {
        licensee: "ACME".into(),
        tier: Tier::Pro,
        expires_unix: 0,
        features: vec![],
    }
}

fn valid_request() -> StudyRequest {
    StudyRequest {
        pack: "pro-twin".into(),
        customer: "ACME Space".into(),
        prepared_for: Some("Mission Ops".into()),
        inputs: json!({ "threshold_ns": 50 }),
    }
}

#[test]
fn valid_pack_request_has_no_violations() {
    let req = valid_request();
    let violations = validate(&req);
    assert!(
        violations.is_empty(),
        "valid request should produce no violations, got: {violations:?}"
    );
}

#[test]
fn study_id_is_stable_for_the_same_request() {
    let req = valid_request();
    let a = study_id(&req);
    let b = study_id(&req);
    assert_eq!(a, b, "study_id must be deterministic");
    // Short hex digest.
    assert!(!a.is_empty());
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn study_id_changes_when_request_changes() {
    let a = study_id(&valid_request());
    let mut other = valid_request();
    other.customer = "Other Customer".into();
    let b = study_id(&other);
    assert_ne!(a, b, "different requests must hash to different ids");
}

#[test]
fn unknown_pack_produces_a_violation() {
    let mut req = valid_request();
    req.pack = "pro-does-not-exist".into();
    let violations = validate(&req);
    assert!(
        violations.iter().any(|v| v.to_lowercase().contains("pack")),
        "unknown pack should be flagged, got: {violations:?}"
    );
}

#[test]
fn empty_customer_produces_a_violation() {
    let mut req = valid_request();
    req.customer = "  ".into();
    let violations = validate(&req);
    assert!(
        violations
            .iter()
            .any(|v| v.to_lowercase().contains("customer")),
        "empty customer should be flagged, got: {violations:?}"
    );
}

#[test]
fn open_engine_kind_is_a_valid_pack() {
    // A known open scenario kind (delegated to the embedded engine) is also a valid
    // study pack — not just `pro-*` kinds.
    let mut req = valid_request();
    req.pack = "ephemeris".into();
    let violations = validate(&req);
    assert!(
        violations.is_empty(),
        "a known open kind should validate, got: {violations:?}"
    );
}

#[test]
fn study_route_accepts_valid_request_on_pro_tier() {
    use kshana_pro::serve::{handle, HttpRequest};
    let body = serde_json::to_string(&valid_request()).unwrap();
    let req = HttpRequest {
        method: "POST".into(),
        path: "/study".into(),
        body,
    };
    let resp = handle(&req, &pro(), 0);
    assert_eq!(resp.status, 200, "valid study on Pro tier: {}", resp.body);
    assert!(
        resp.body.contains("\"status\":\"accepted\""),
        "{}",
        resp.body
    );
    assert!(resp.body.contains("study_id"), "{}", resp.body);
    // The study_id in the ack matches the pure helper.
    assert!(
        resp.body.contains(&study_id(&valid_request())),
        "{}",
        resp.body
    );
}

#[test]
fn study_route_rejects_invalid_request_with_400() {
    use kshana_pro::serve::{handle, HttpRequest};
    let mut bad = valid_request();
    bad.customer = "".into();
    bad.pack = "pro-nope".into();
    let body = serde_json::to_string(&bad).unwrap();
    let req = HttpRequest {
        method: "POST".into(),
        path: "/study".into(),
        body,
    };
    let resp = handle(&req, &pro(), 0);
    assert_eq!(resp.status, 400, "invalid study must be 400: {}", resp.body);
}

#[test]
fn study_route_gates_below_pro_tier_with_403() {
    use kshana_pro::serve::{handle, HttpRequest};
    let body = serde_json::to_string(&valid_request()).unwrap();
    let req = HttpRequest {
        method: "POST".into(),
        path: "/study".into(),
        body,
    };
    let resp = handle(&req, &Entitlement::evaluation(), 0);
    assert_eq!(
        resp.status, 403,
        "a study requires at least Pro tier: {}",
        resp.body
    );
}

/// F3c: a structured study request produces a queued dossier job and is counted in the
/// study funnel (`studies.jsonl`, `kshana_study_requests_total`, `kshana_study_dossiers`).
#[test]
fn study_request_queues_a_dossier_and_is_counted() {
    use std::io::{Read, Write};
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("study_intake_funnel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut cfg = kshana_pro::serve::ServeConfig::new("127.0.0.1:0", pro());
    cfg.data_dir = Some(dir.clone());
    let srv = kshana_pro::serve::start(cfg).unwrap();
    let a = srv.local_addr();
    let http = |method: &str, path: &str, body: &str| {
        let mut s = std::net::TcpStream::connect(a).unwrap();
        write!(
            s,
            "{method} {path} HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let st: u16 = out[9..12].parse().unwrap();
        (st, out.split_once("\r\n\r\n").unwrap().1.to_string())
    };
    let scenario = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scenarios/quantum-systematics-sr.toml"),
    )
    .unwrap();
    let req = json!({
        "pack": "pro-quantum-systematics",
        "customer": "Pilot Lab",
        "prepared_for": "Timing team",
        "inputs": {"scenario": scenario},
    });
    let (st, body) = http("POST", "/study", &req.to_string());
    assert_eq!(st, 200, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let job = v["job_id"].as_str().unwrap().to_string();
    let t = std::time::Instant::now();
    loop {
        let (_, b) = http("GET", &format!("/jobs/{job}"), "");
        if b.contains("\"succeeded\"") {
            break;
        }
        assert!(!b.contains("\"failed\""), "{b}");
        assert!(t.elapsed() < std::time::Duration::from_secs(120), "{b}");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // An invalid request is counted too.
    let (st, _) = http(
        "POST",
        "/study",
        &json!({"pack": "no-such-pack", "customer": "x", "inputs": {}}).to_string(),
    );
    assert_eq!(st, 400);
    let (_, m) = http("GET", "/metrics", "");
    assert!(
        m.contains("kshana_study_requests_total{tenant=\"default\",outcome=\"queued\"} 1"),
        "{m}"
    );
    assert!(
        m.contains("kshana_study_requests_total{tenant=\"default\",outcome=\"invalid\"} 1"),
        "{m}"
    );
    assert!(
        m.contains("kshana_study_dossiers{tenant=\"default\",state=\"succeeded\"} 1"),
        "{m}"
    );
    let log = std::fs::read_to_string(dir.join("studies.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 2);
    assert!(log.contains("\"pack\":\"pro-quantum-systematics\""));
    srv.stop();
}
