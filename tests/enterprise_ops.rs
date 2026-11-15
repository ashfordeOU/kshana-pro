// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Enterprise operations (roadmap 0.42 F8, U28): single sign-on with SCIM provisioning (a
//! person deactivated in the identity provider is refused at the next request; roles come from
//! groups), audit log export, retention policies, the watch wall, and the edition gate (each
//! is Enterprise; a Pro licence is refused). The OIDC tokens are the independent ones of
//! `tests/fixtures/oidc` (alice of site-a, a submitter by her token; bob of site-b, an admin).

use std::path::{Path, PathBuf};

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::api::{self, Service};
use kshana_pro::serve::audit::AuditLog;
use kshana_pro::serve::auth::{new_key, Authenticator, KeysFile, Role, ALL_TENANTS};
use kshana_pro::serve::oidc::Verifier;
use kshana_pro::serve::worker::JobService;
use kshana_pro::serve::HttpRequest;
use serde_json::{json, Value};

fn tokens() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oidc/tokens.json"),
        )
        .unwrap(),
    )
    .unwrap()
}

fn fresh(label: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ent-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ent(tier: Tier) -> Entitlement {
    Entitlement {
        licensee: "ops test".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

struct W {
    svc: Service,
    a_admin: String,
    ops: String,
    now: i64,
}

fn service(data: &Path, tier: Tier) -> W {
    let now = tokens()["now"].as_i64().unwrap();
    let (a_admin, e1) = new_key("a-admin", "site-a", Role::Admin);
    let (ops, e2) = new_key("ops", ALL_TENANTS, Role::Admin);
    let keys = KeysFile { keys: vec![e1, e2] };
    let v = Verifier::from_file(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oidc/oidc.toml"),
    )
    .unwrap();
    let svc = Service::with_auth(
        ent(tier),
        Some(std::sync::Arc::new(
            JobService::open(data.to_path_buf(), ent(tier)).unwrap(),
        )),
        Authenticator::new(None, Some(&keys)).with_oidc(v),
        Some(AuditLog::open(data).unwrap()),
    );
    W {
        svc,
        a_admin,
        ops,
        now,
    }
}

fn call(w: &W, key: &str, method: &str, path: &str, body: &str) -> (u16, Value) {
    let r = api::dispatch(
        &w.svc,
        &HttpRequest {
            method: method.into(),
            path: path.into(),
            body: body.into(),
        },
        Some(&format!("Bearer {key}")),
        w.now,
    );
    let v = serde_json::from_str(&r.body).unwrap_or(json!({"text": r.body}));
    (r.status, v)
}

const PATCH: &str = "urn:ietf:params:scim:api:messages:2.0:PatchOp";

#[test]
fn scim_provisioning_drives_sso_sign_in_and_roles() {
    let data = fresh("scim");
    let w = service(&data, Tier::Enterprise);
    let t = tokens();
    let alice = t["pass"]["es256"][0].as_str().unwrap().to_string();
    let bob = t["pass"]["rs256"][0].as_str().unwrap().to_string();

    // Before provisioning, alice signs in with her token's role.
    let (st, me) = call(&w, &alice, "GET", "/whoami", "");
    assert_eq!(
        (st, me["role"].as_str(), me["source"].as_str()),
        (200, Some("submitter"), Some("oidc"))
    );

    // Provision site-a: alice as a reader (group kshana-reader), and carol.
    let (st, u) = call(
        &w,
        &w.a_admin,
        "POST",
        "/scim/v2/Users",
        &json!({"userName": "alice", "displayName": "Alice"}).to_string(),
    );
    assert_eq!(st, 201, "{u}");
    let alice_id = u["id"].as_str().unwrap().to_string();
    assert_eq!(
        u["schemas"][0],
        "urn:ietf:params:scim:schemas:core:2.0:User"
    );
    assert_eq!(
        call(
            &w,
            &w.a_admin,
            "POST",
            "/scim/v2/Users",
            &json!({"userName": "alice"}).to_string()
        )
        .0,
        409
    );
    assert_eq!(
        call(
            &w,
            &w.a_admin,
            "POST",
            "/scim/v2/Users",
            &json!({"userName": "carol"}).to_string()
        )
        .0,
        201
    );
    let (st, g) = call(
        &w,
        &w.a_admin,
        "POST",
        "/scim/v2/Groups",
        &json!({"displayName": "kshana-reader", "members": [{"value": alice_id}]}).to_string(),
    );
    assert_eq!(st, 201);
    let reader_group = g["id"].as_str().unwrap().to_string();
    let (_, list) = call(
        &w,
        &w.a_admin,
        "GET",
        "/scim/v2/Users?filter=userName%20eq%20%22alice%22",
        "",
    );
    assert_eq!(list["totalResults"], 1);
    assert_eq!(list["Resources"][0]["id"], alice_id.as_str());

    // The group decides: alice is now a reader and cannot submit.
    let (_, me) = call(&w, &alice, "GET", "/whoami", "");
    assert_eq!(me["role"], "reader");
    let job = json!({"op": "run", "request": {"scenario": "kind = \"link-budget\"\n"}}).to_string();
    assert_eq!(call(&w, &alice, "POST", "/jobs", &job).0, 403);

    // Moved to kshana-submitter: she may submit again.
    let (st, sg) = call(
        &w,
        &w.a_admin,
        "POST",
        "/scim/v2/Groups",
        &json!({"displayName": "kshana-submitter"}).to_string(),
    );
    assert_eq!(st, 201);
    let sub_group = sg["id"].as_str().unwrap().to_string();
    let add = json!({"schemas": [PATCH], "Operations": [{"op": "add", "path": "members", "value": [{"value": alice_id}]}]});
    assert_eq!(
        call(
            &w,
            &w.a_admin,
            "PATCH",
            &format!("/scim/v2/Groups/{sub_group}"),
            &add.to_string()
        )
        .0,
        200
    );
    let rm = json!({"schemas": [PATCH], "Operations": [{"op": "remove", "path": format!("members[value eq \"{alice_id}\"]")}]});
    let (_, g2) = call(
        &w,
        &w.a_admin,
        "PATCH",
        &format!("/scim/v2/Groups/{reader_group}"),
        &rm.to_string(),
    );
    assert_eq!(g2["members"], json!([]));
    assert_eq!(
        call(&w, &alice, "GET", "/whoami", "").1["role"],
        "submitter"
    );

    // Deactivated in the identity provider: refused at once, whatever her token says.
    let off = json!({"schemas": [PATCH], "Operations": [{"op": "replace", "path": "active", "value": false}]});
    let (st, u2) = call(
        &w,
        &w.a_admin,
        "PATCH",
        &format!("/scim/v2/Users/{alice_id}"),
        &off.to_string(),
    );
    assert_eq!((st, &u2["active"]), (200, &json!(false)));
    let (st, why) = call(&w, &alice, "GET", "/whoami", "");
    assert_eq!(st, 401, "{why}");
    // Reactivated (the Azure form: replace with an object), then deleted: refused again.
    let on =
        json!({"schemas": [PATCH], "Operations": [{"op": "Replace", "value": {"active": true}}]});
    assert_eq!(
        call(
            &w,
            &w.a_admin,
            "PATCH",
            &format!("/scim/v2/Users/{alice_id}"),
            &on.to_string()
        )
        .0,
        200
    );
    assert_eq!(call(&w, &alice, "GET", "/whoami", "").0, 200);
    assert_eq!(
        call(
            &w,
            &w.a_admin,
            "DELETE",
            &format!("/scim/v2/Users/{alice_id}"),
            ""
        )
        .0,
        204
    );
    assert_eq!(call(&w, &alice, "GET", "/whoami", "").0, 401);

    // site-b has no provisioned people: bob signs in by his token. API keys are not people.
    assert_eq!(call(&w, &bob, "GET", "/whoami", "").1["role"], "admin");
    assert_eq!(
        call(&w, &w.a_admin, "GET", "/whoami", "").1["role"],
        "admin"
    );
    // SCIM needs a tenant admin: a submitter, and the operator of every tenant, are refused.
    assert_eq!(call(&w, &w.ops, "GET", "/scim/v2/Users", "").0, 403);
    assert_eq!(
        call(&w, &w.a_admin, "GET", "/scim/v2/ServiceProviderConfig", "").1["patch"]["supported"],
        true
    );
    let bad = json!({"schemas": [PATCH], "Operations": [{"op": "replace", "path": "emails", "value": "x"}]});
    let (_, carol) = call(
        &w,
        &w.a_admin,
        "GET",
        "/scim/v2/Users?filter=userName%20eq%20%22carol%22",
        "",
    );
    let carol_id = carol["Resources"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(
        call(
            &w,
            &w.a_admin,
            "PATCH",
            &format!("/scim/v2/Users/{carol_id}"),
            &bad.to_string()
        )
        .0,
        400
    );
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn audit_export_retention_and_the_watch_wall() {
    let data = fresh("ops");
    let w = service(&data, Tier::Enterprise);
    let j = w.svc.jobs.as_ref().unwrap();
    // Two finished jobs of site-a: one old, one recent; one old job delivered (ledger).
    let mk = |tag: &str, age_days: i64| {
        let (mut s, _) = j
            .store
            .submit_for(
                "site-a",
                "run",
                &json!({"scenario": format!("kind = \"link-budget\"\n# {tag}\n")}),
                None,
            )
            .unwrap();
        s.state = kshana_pro::serve::jobs::JobState::Succeeded;
        s.finished_unix = Some(w.now - age_days * 86_400);
        j.store.write_status(&s).unwrap();
        s.job_id
    };
    let old = mk("old", 200);
    let recent = mk("recent", 2);
    let delivered = mk("delivered", 400);
    j.ledger
        .append_delivery(&delivered, "r", "h", "2026-01-01T00:00:00Z")
        .unwrap();
    std::fs::write(
        data.join("retention.toml"),
        "[default]\njobs_days = 365\naudit_days = 30\n[tenant.site-a]\njobs_days = 90\n",
    )
    .unwrap();

    // Dry run as site-a's admin: the old job is due, the delivered one is kept.
    let (st, dry) = call(&w, &w.a_admin, "GET", "/retention", "");
    assert_eq!(st, 200, "{dry}");
    assert_eq!(dry["report"]["applied"], false);
    assert_eq!(dry["report"]["jobs"], json!([["site-a", old]]));
    assert_eq!(dry["report"]["kept_delivered"], 1);
    // Only an operator applies.
    assert_eq!(call(&w, &w.a_admin, "POST", "/retention/apply", "").0, 403);
    let (st, done) = call(&w, &w.ops, "POST", "/retention/apply", "");
    assert_eq!(st, 200, "{done}");
    assert_eq!(done["report"]["jobs"].as_array().unwrap().len(), 1);
    assert!(j.store.status(&old).unwrap().is_none());
    assert!(j.store.status(&recent).unwrap().is_some());
    assert!(j.store.status(&delivered).unwrap().is_some());
    // The ledger chain is intact and still names the kept delivered job.
    assert_eq!(
        call(&w, &w.ops, "GET", "/ledger/verify", "").1["chain_intact"],
        true
    );

    // Audit export: day bounds, per-tenant view, every line's hash, the chain.
    let (st, ex) = call(
        &w,
        &w.a_admin,
        "GET",
        "/audit/export?since=2027-01-01&until=2027-12-31",
        "",
    );
    assert_eq!(st, 200, "{ex}");
    assert_eq!(ex["format"], "kshana-pro-audit-export/1");
    let lines = ex["lines"].as_array().unwrap();
    assert!(!lines.is_empty());
    assert!(lines
        .iter()
        .all(|l| l["tenant"] == "site-a" && l["sha256"].as_str().is_some_and(|h| h.len() == 64)));
    assert!(lines.iter().any(|l| l["route"] == "/retention"));
    assert_eq!(ex["chain"]["verified"], true);
    assert_eq!(
        call(&w, &w.a_admin, "GET", "/audit/export?since=2027-1-1", "").0,
        400
    );
    let (_, none) = call(&w, &w.a_admin, "GET", "/audit/export?until=2020-01-01", "");
    assert_eq!(none["count"], 0);
    let (_, op) = call(&w, &w.ops, "GET", "/audit/export", "");
    assert!(op["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["tenant"] == "*"));

    // The wall: the tenant's Stations, jobs, and what is not configured.
    assert_eq!(
        call(
            &w,
            &w.a_admin,
            "POST",
            "/fleet/stations",
            &json!({"id": "tallinn-1", "name": "Tallinn port"}).to_string()
        )
        .0,
        201
    );
    let (st, wall) = call(&w, &w.a_admin, "GET", "/wall/state", "");
    assert_eq!(st, 200, "{wall}");
    assert_eq!(wall["format"], "kshana-pro-watch-wall/1");
    assert_eq!(wall["fleet"]["stations"][0]["id"], "tallinn-1");
    assert_eq!(wall["fleet"]["counts"]["registered"], 1);
    assert_eq!(wall["area"]["configured"], false);
    assert_eq!(wall["jobs"]["succeeded"], 2);
    let page = api::dispatch(
        &w.svc,
        &HttpRequest {
            method: "GET".into(),
            path: "/wall".into(),
            body: String::new(),
        },
        Some(&format!("Bearer {}", w.a_admin)),
        w.now,
    );
    assert_eq!(page.status, 200);
    assert!(
        page.body.contains("http-equiv=\"refresh\"")
            && page.body.contains("Tallinn port")
            && !page.body.contains("<script")
    );
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn each_enterprise_operation_is_refused_under_pro() {
    let data = fresh("pro");
    let w = service(&data, Tier::Pro);
    std::fs::write(data.join("retention.toml"), "[default]\njobs_days = 30\n").unwrap();
    for (m, p) in [
        ("GET", "/scim/v2/Users"),
        ("GET", "/audit/export"),
        ("GET", "/retention"),
        ("POST", "/retention/apply"),
        ("GET", "/wall"),
        ("GET", "/wall/state"),
    ] {
        let (st, why) = call(&w, &w.ops, m, p, "");
        assert_eq!(st, 403, "{m} {p}: {why}");
    }
    let (_, me) = call(&w, &w.ops, "GET", "/whoami", "");
    assert_eq!(
        (
            me["edition"].as_str(),
            me["enterprise"].as_array().map(Vec::len)
        ),
        (Some("pro"), Some(0))
    );
    let _ = std::fs::remove_dir_all(&data);
}
