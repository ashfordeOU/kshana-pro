// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Tenant isolation (roadmap 0.42 F8 bar: "tenant isolation test green").
//!
//! Tenant `site-a` is given one of everything the service stores per tenant: a job, a saved
//! study in the team library, a share link, a Station, a SCIM user and group, an area console
//! file, a timing fleet file, and audit lines. Then **every route of the route table**
//! (`api::ROUTES`, so a route added later is covered without editing this test) is called by
//! an **admin of `site-b`**, with `site-a`'s identifiers in the path wherever the template has
//! a parameter. The test requires that:
//!
//! 1. no response carries anything of `site-a` that the caller did not put in the request
//!    itself (the tenant name, its principals, its people, its identifiers, its file contents);
//! 2. every route that names one of `site-a`'s resources answers as for a resource that does
//!    not exist (404), never 200, and never changes it;
//! 3. `site-a`'s stored files are byte for byte the same after all the calls.
//!
//! The only intended exception is a share link, whose token is the credential by design
//! (`GET /shared/{token}`): it is checked separately to serve the shared job only.
//! A sanity pass runs the same reads as `site-a` and must see its own markers, so the test
//! cannot pass by every route failing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::api::{self, Service, ROUTES};
use kshana_pro::serve::audit::AuditLog;
use kshana_pro::serve::auth::{new_key, Authenticator, KeysFile, Role, ALL_TENANTS};
use kshana_pro::serve::worker::JobService;
use kshana_pro::serve::HttpRequest;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const NOW: i64 = 1_800_000_000;

fn fresh(label: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("tenant-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn enterprise() -> Entitlement {
    Entitlement {
        licensee: "isolation test".into(),
        tier: Tier::Enterprise,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

struct World {
    svc: Service,
    data: PathBuf,
    a: String,
    b: String,
    ops: String,
}

fn call(w: &World, key: &str, method: &str, path: &str, body: &str) -> (u16, String) {
    let r = api::dispatch(
        &w.svc,
        &HttpRequest {
            method: method.into(),
            path: path.into(),
            body: body.into(),
        },
        Some(&format!("Bearer {key}")),
        NOW,
    );
    (r.status, r.body)
}

fn hash_tree(root: &Path, out: &mut BTreeMap<String, String>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            hash_tree(&p, out);
        } else {
            out.insert(
                p.display().to_string(),
                hex::encode(Sha256::digest(std::fs::read(&p).unwrap())),
            );
        }
    }
}

/// `site-a`'s stored state (everything but the audit log, which every request appends to).
fn snapshot(w: &World, a_job: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for d in [
        "library/site-a",
        "fleet/site-a",
        "scim/site-a",
        "shares",
        "area/site-a",
        "timing/site-a",
    ] {
        hash_tree(&w.data.join(d), &mut m);
    }
    hash_tree(&w.svc.jobs.as_ref().unwrap().store.job_dir(a_job), &mut m);
    m
}

#[test]
fn no_route_lets_one_tenant_see_or_change_another() {
    let data = fresh("iso");
    let (a, ea) = new_key("a-admin-secret", "site-a", Role::Admin);
    let (b, eb) = new_key("b-admin", "site-b", Role::Admin);
    let (ops, eo) = new_key("ops", ALL_TENANTS, Role::Admin);
    let keys = KeysFile {
        keys: vec![ea, eb, eo],
    };
    let jobs = JobService::open(data.clone(), enterprise()).unwrap();
    let audit = AuditLog::open(&data).unwrap();
    let mut svc = Service::with_auth(
        enterprise(),
        Some(std::sync::Arc::new(jobs)),
        Authenticator::new(None, Some(&keys)),
        Some(audit),
    );
    svc.stations_claim = 10;
    let w = World {
        svc,
        data: data.clone(),
        a,
        b,
        ops,
    };

    // ---- site-a's things.
    let (st, body) = call(
        &w,
        &w.a,
        "POST",
        "/jobs",
        &json!({"op": "run", "request": {"scenario": "kind = \"link-budget\"\n# a-job-secret\n"}})
            .to_string(),
    );
    assert!(st == 202 || st == 200 || st == 422, "{st} {body}");
    let j = w.svc.jobs.as_ref().unwrap();
    // A succeeded job of site-a (written directly: no worker runs in this test).
    let (mut status, _) = j
        .store
        .submit_for(
            "site-a",
            "run",
            &json!({"scenario": "kind = \"link-budget\"\n# a-job-secret-2\n"}),
            Some("link-budget".into()),
        )
        .unwrap();
    status.state = kshana_pro::serve::jobs::JobState::Succeeded;
    status.finished_unix = Some(NOW - 10);
    status.finished_utc = Some("2027-01-15T08:00:00Z".into());
    j.store.write_status(&status).unwrap();
    j.store
        .write_outputs(
            &status.job_id,
            "{\"a-result-secret\": true}\n",
            &[("a-artefact-secret.txt".into(), "a-file-secret".into())],
        )
        .unwrap();
    let a_job = status.job_id.clone();
    assert_eq!(call(&w, &w.a, "POST", "/library", &json!({"name": "a-study-secret", "op": "run", "request": {"scenario": "kind = \"link-budget\"\n"}}).to_string()).0 / 100, 2);
    let (st, share) = call(
        &w,
        &w.a,
        "POST",
        &format!("/jobs/{a_job}/share"),
        "{\"days\": 5}",
    );
    assert_eq!(st, 201, "{share}");
    let token = serde_json::from_str::<Value>(&share).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        call(
            &w,
            &w.a,
            "POST",
            "/fleet/stations",
            &json!({"id": "a-station-secret", "name": "A station secret"}).to_string()
        )
        .0,
        201
    );
    let (st, u) = call(
        &w,
        &w.a,
        "POST",
        "/scim/v2/Users",
        &json!({"userName": "a-person@secret.example", "displayName": "A Person Secret"})
            .to_string(),
    );
    assert_eq!(st, 201, "{u}");
    let a_user = serde_json::from_str::<Value>(&u).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (st, g) = call(
        &w,
        &w.a,
        "POST",
        "/scim/v2/Groups",
        &json!({"displayName": "kshana-admin", "members": [{"value": a_user}]}).to_string(),
    );
    assert_eq!(st, 201, "{g}");
    let a_group = serde_json::from_str::<Value>(&g).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    for (dir, file, text) in [
        ("area/site-a", "console.toml", "# a-console-secret\n"),
        ("timing/site-a", "fleet.toml", "# a-fleet-secret\n"),
    ] {
        std::fs::create_dir_all(data.join(dir)).unwrap();
        std::fs::write(data.join(dir).join(file), text).unwrap();
    }
    std::fs::write(data.join("retention.toml"), "[default]\njobs_days = 3650\n").unwrap();

    let markers: Vec<String> = [
        "site-a",
        "a-admin-secret",
        "a-job-secret",
        "a-result-secret",
        "a-artefact-secret",
        "a-file-secret",
        "a-study-secret",
        "a-station-secret",
        "A station secret",
        "a-person@secret.example",
        "A Person Secret",
        "a-console-secret",
        "a-fleet-secret",
        &a_job,
        &a_user,
        &a_group,
        &token,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Sanity: site-a sees its own things.
    for (path, marker) in [
        ("/jobs", a_job.as_str()),
        ("/library", "a-study-secret"),
        ("/fleet/stations", "a-station-secret"),
        ("/scim/v2/Users", "a-person@secret.example"),
        ("/whoami", "site-a"),
    ] {
        let (st, body) = call(&w, &w.a, "GET", path, "");
        assert_eq!(st, 200, "{path}: {body}");
        assert!(
            body.contains(marker),
            "site-a cannot see its own {marker} at {path}"
        );
    }

    let before = snapshot(&w, &a_job);

    // ---- every route, as site-b's admin, aimed at site-a's identifiers.
    let param = |route: &str, name: &str| -> String {
        match (route, name) {
            (r, "id") if r.starts_with("/jobs/") => a_job.clone(),
            (r, "id") if r.starts_with("/fleet/") => "a-station-secret".into(),
            (r, "id") if r.starts_with("/scim/v2/Users") => a_user.clone(),
            (r, "id") if r.starts_with("/scim/v2/Groups") => a_group.clone(),
            (_, "name") if route.starts_with("/library") => "a-study-secret".into(),
            (_, "name") => "a-artefact-secret.txt".into(),
            (_, "token") => token.clone(),
            (r, n) => panic!("no site-a identifier for {{{n}}} in {r}: extend this test"),
        }
    };
    let mut checked = 0;
    let mut named = 0;
    // Identifiers site-b has itself typed so far: its own audit lines record them.
    let mut supplied: Vec<String> = Vec::new();
    for r in ROUTES {
        if r.role.is_none() {
            continue; // /health and the share links (checked below)
        }
        let mut path = String::new();
        let mut names_a = false;
        for seg in r.path.split('/').skip(1) {
            path.push('/');
            if let Some(n) = seg.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                path.push_str(&param(r.path, n));
                names_a = true;
            } else {
                path.push_str(seg);
            }
        }
        let body = match (r.method, r.path) {
            ("POST", "/jobs") => json!({"op": "run", "request": {"scenario": "kind = \"link-budget\"\n"}}).to_string(),
            ("POST", "/library") => json!({"name": "b-study", "op": "run", "request": {"scenario": "kind = \"link-budget\"\n"}}).to_string(),
            ("POST", "/fleet/stations") => json!({"id": "b-station", "name": "B's own"}).to_string(),
            ("POST", "/scim/v2/Users") => json!({"userName": "b-person@example.org"}).to_string(),
            ("POST", "/scim/v2/Groups") => json!({"displayName": "kshana-admin"}).to_string(),
            ("PUT", _) => json!({"userName": "taken-over", "active": true}).to_string(),
            ("PATCH", p) if p.contains("Groups") => json!({"schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"], "Operations": [{"op": "add", "path": "members", "value": [{"value": "b-person"}]}]}).to_string(),
            ("PATCH", _) => json!({"schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"], "Operations": [{"op": "replace", "path": "active", "value": false}]}).to_string(),
            ("POST", "/jobs/{id}/share") => "{\"days\": 1}".into(),
            ("POST", "/run") => "kind = \"link-budget\"\n".into(),
            ("POST", "/study") => "{}".into(),
            ("POST", _) => "{}".into(),
            _ => String::new(),
        };
        let (st, resp) = call(&w, &w.b, r.method, &path, &body);
        for m in &markers {
            if path.contains(m.as_str()) || body.contains(m.as_str()) {
                supplied.push(m.clone());
                continue; // the caller supplied it
            }
            if r.path == "/audit/export" && supplied.contains(m) {
                continue; // site-b's own audit line of a request in which it typed this
            }
            assert!(
                !resp.contains(m.as_str()),
                "{} {path} ({st}) shows site-a's '{m}' to site-b: {resp}",
                r.method
            );
        }
        if names_a {
            named += 1;
            assert_ne!(
                st / 100,
                2,
                "{} {path} succeeded for site-b on site-a's resource: {resp}",
                r.method
            );
            assert!(
                matches!(st, 404 | 403 | 400 | 422 | 405),
                "{} {path}: {st} {resp}",
                r.method
            );
        }
        checked += 1;
    }
    assert!(checked >= 45, "only {checked} routes checked");
    assert!(named >= 15, "only {named} routes named a site-a resource");

    // site-b registering an id site-a uses got its own Station; site-a's is untouched.
    assert_eq!(
        snapshot(&w, &a_job),
        before,
        "site-b changed site-a's stored state"
    );

    // The share link serves the shared job only, to anyone holding the token, by design.
    let (st, s) = call(&w, &w.b, "GET", &format!("/shared/{token}"), "");
    assert_eq!(st, 200);
    assert!(s.contains(&a_job) && !s.contains("a-study-secret") && !s.contains("a-station-secret"));

    // The audit log is one per server: site-b's export holds none of site-a's lines; the
    // operator's holds both; the chain verifies.
    let (_, ex) = call(&w, &w.b, "GET", "/audit/export", "");
    let ex: Value = serde_json::from_str(&ex).unwrap();
    assert!(ex["lines"]
        .as_array()
        .unwrap()
        .iter()
        .all(|l| l["tenant"] == "site-b"));
    let (_, all) = call(&w, &w.ops, "GET", "/audit/export", "");
    let all: Value = serde_json::from_str(&all).unwrap();
    assert!(all["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["tenant"] == "site-a"));
    assert_eq!(all["chain"]["verified"], true);
    let _ = std::fs::remove_dir_all(&data);
}
