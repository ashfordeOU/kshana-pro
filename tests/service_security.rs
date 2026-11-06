// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Integration security of the job service (roadmap A9, `docs/design/A9-INTEGRATION-SECURITY.md`),
//! over real sockets: API keys and roles, tenant isolation on every job route, and the
//! hash-chained audit log, which records every request and never a key, a body or a query.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::auth::{new_key, KeysFile, Role, ALL_TENANTS};
use kshana_pro::serve::{self, ServeConfig};
use serde_json::{json, Value};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/service")
            .join(name),
    )
    .unwrap()
}

fn fresh_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("service_security")
        .join(format!("{label}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Resp {
    status: u16,
    body: String,
}

impl Resp {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("{e}: {}", self.body))
    }
}

fn http(addr: SocketAddr, method: &str, path: &str, body: &str, key: Option<&str>) -> Resp {
    let mut raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(k) = key {
        raw.push_str(&format!("Authorization: Bearer {k}\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(120))).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf).to_string();
    let (head, body) = text.split_once("\r\n\r\n").unwrap();
    Resp {
        status: head.split_whitespace().nth(1).unwrap().parse().unwrap(),
        body: body.to_string(),
    }
}

struct Keys {
    a_sub: String,
    a_read: String,
    b_sub: String,
    ops: String,
    revoked: String,
    file: KeysFile,
}

fn keys() -> Keys {
    let (a_sub, e1) = new_key("a-ingest", "site-a", Role::Submitter);
    let (a_read, e2) = new_key("a-viewer", "site-a", Role::Reader);
    let (b_sub, e3) = new_key("b-ingest", "site-b", Role::Submitter);
    let (ops, e4) = new_key("ops", ALL_TENANTS, Role::Admin);
    let (revoked, mut e5) = new_key("gone", "site-a", Role::Admin);
    e5.revoked = true;
    Keys {
        a_sub,
        a_read,
        b_sub,
        ops,
        revoked,
        file: KeysFile {
            keys: vec![e1, e2, e3, e4, e5],
        },
    }
}

fn wait(addr: SocketAddr, id: &str, key: &str) {
    let t0 = Instant::now();
    loop {
        let v = http(addr, "GET", &format!("/jobs/{id}"), "", Some(key)).json();
        if v["state"] == "succeeded" || v["state"] == "failed" {
            return;
        }
        assert!(t0.elapsed() < Duration::from_secs(300));
        std::thread::sleep(Duration::from_millis(40));
    }
}

#[test]
fn roles_tenants_and_the_audit_log() {
    let k = keys();
    let data = fresh_dir("sec");
    let mut cfg = ServeConfig::new(
        "127.0.0.1:0",
        Entitlement {
            licensee: "test".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
        },
    );
    cfg.data_dir = Some(data.clone());
    cfg.keys = Some(k.file.clone());
    let srv = serve::start(cfg).unwrap();
    let addr = srv.local_addr();
    let run =
        json!({"op": "run", "request": {"scenario": fixture("link-budget.toml")}}).to_string();

    // Credentials and roles.
    assert_eq!(http(addr, "GET", "/health", "", None).status, 200);
    assert_eq!(http(addr, "GET", "/jobs", "", None).status, 401);
    assert_eq!(
        http(addr, "GET", "/jobs", "", Some("kpk_wrong")).status,
        401
    );
    assert_eq!(http(addr, "GET", "/jobs", "", Some(&k.revoked)).status, 401);
    assert_eq!(
        http(addr, "POST", "/jobs", &run, Some(&k.a_read)).status,
        403
    );
    assert_eq!(http(addr, "GET", "/ledger", "", Some(&k.a_sub)).status, 403);

    // Tenant A submits; the same request from tenant B is a different job.
    let ra = http(addr, "POST", "/jobs", &run, Some(&k.a_sub));
    assert_eq!(ra.status, 202, "{}", ra.body);
    let a = ra.json()["job_id"].as_str().unwrap().to_string();
    let rb = http(addr, "POST", "/jobs", &run, Some(&k.b_sub));
    assert_eq!(rb.status, 202, "{}", rb.body);
    let b = rb.json()["job_id"].as_str().unwrap().to_string();
    assert_ne!(a, b);
    wait(addr, &a, &k.a_read);
    wait(addr, &b, &k.b_sub);

    // B cannot see A's job on any job route: it answers as a job that does not exist.
    let missing = format!("/jobs/{}", "f".repeat(64));
    for path in [
        format!("/jobs/{a}"),
        format!("/jobs/{a}/result"),
        format!("/jobs/{a}/artefacts/result.json"),
    ] {
        let r = http(addr, "GET", &path, "", Some(&k.b_sub));
        assert_eq!(r.status, 404, "{path}: {}", r.body);
        assert_eq!(r.body, http(addr, "GET", &missing, "", Some(&k.b_sub)).body);
    }
    assert_eq!(
        http(
            addr,
            "GET",
            &format!("/jobs/{a}/result"),
            "",
            Some(&k.a_read)
        )
        .status,
        200
    );
    let list_b = http(addr, "GET", "/jobs", "", Some(&k.b_sub)).json();
    assert_eq!(list_b["total"], 1);
    assert!(!list_b.to_string().contains(&a));
    let board_b = http(addr, "GET", "/", "", Some(&k.b_sub)).body;
    assert!(!board_b.contains(&a));
    // The operator sees both, and the ledger and audit routes.
    assert_eq!(
        http(addr, "GET", "/jobs", "", Some(&k.ops)).json()["total"],
        2
    );
    assert_eq!(
        http(addr, "GET", "/ledger/verify", "", Some(&k.ops)).status,
        200
    );
    let audit = http(addr, "GET", "/audit/verify", "", Some(&k.ops)).json();
    assert_eq!(audit["verified"], true, "{audit}");

    // The audit log: one line per request but /health, with who and which route, and never
    // a key, a body or a query string.
    srv.stop();
    let mut text = String::new();
    for e in std::fs::read_dir(data.join("audit")).unwrap() {
        text.push_str(&std::fs::read_to_string(e.unwrap().path()).unwrap());
    }
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(lines.iter().all(|l| l["route"] != "/health"));
    assert!(lines.iter().any(|l| l["who"] == "-" && l["status"] == 401));
    assert!(lines.iter().any(|l| l["who"] == "a-ingest"
        && l["route"] == "/jobs"
        && l["method"] == "POST"
        && l["job"] == a.as_str()));
    assert!(lines
        .iter()
        .any(|l| l["who"] == "b-ingest" && l["status"] == 404 && l["route"] == "/jobs/{id}"));
    for secret in [&k.a_sub, &k.a_read, &k.b_sub, &k.ops, &k.revoked] {
        assert!(!text.contains(secret.as_str()));
    }
    assert!(!text.contains("link-budget") && !text.contains("scenario"));

    // Editing one line breaks the chain at the next one.
    let report = kshana_pro::serve::audit::verify(&data.join("audit"));
    assert!(report.verified && report.lines == lines.len());
    let day = std::fs::read_dir(data.join("audit"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let t = std::fs::read_to_string(&day).unwrap();
    std::fs::write(&day, t.replacen("\"status\":401", "\"status\":200", 1)).unwrap();
    assert!(!kshana_pro::serve::audit::verify(&data.join("audit")).verified);
}

#[test]
fn an_open_server_still_answers_as_before_and_audits() {
    let data = fresh_dir("open");
    let mut cfg = ServeConfig::new("127.0.0.1:0", Entitlement::evaluation());
    cfg.data_dir = Some(data.clone());
    let srv = serve::start(cfg).unwrap();
    let addr = srv.local_addr();
    assert_eq!(http(addr, "GET", "/jobs", "", None).status, 200);
    assert_eq!(http(addr, "GET", "/audit/verify", "", None).status, 200);
    srv.stop();
    let report = kshana_pro::serve::audit::verify(&data.join("audit"));
    assert!(report.verified);
    assert_eq!(report.lines, 2);
}
