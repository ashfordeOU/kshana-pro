// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
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
