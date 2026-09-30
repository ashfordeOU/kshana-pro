// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Acceptance tests of the on-premises job service (`src/serve`): real sockets on
//! 127.0.0.1 with a port the operating system picks, a data directory under the build's
//! own temporary directory, and every job result compared with a direct
//! `kshana::api::run_scenario` call on the same scenario file.
//!
//! The scenario files are unmodified copies of the open engine's public scenarios; see
//! `tests/fixtures/service/PROVENANCE.md`. No test uses the engine's optional workshop
//! preset, and `fixtures_never_touch_the_optional_preset` proves it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::{self, ServeConfig, ServerHandle};
use serde_json::{json, Value};

// ─── Helpers ────────────────────────────────────────────────────────────────────

const LICENSEE: &str = "LICENSEE-NAME-MUST-NOT-LEAK";

fn fixture(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/service")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn enterprise() -> Entitlement {
    Entitlement {
        licensee: LICENSEE.into(),
        tier: Tier::Enterprise,
        expires_unix: 0,
        features: vec![],
    }
}

fn evaluation_named() -> Entitlement {
    Entitlement {
        licensee: LICENSEE.into(),
        tier: Tier::Evaluation,
        expires_unix: 0,
        features: vec![],
    }
}

/// A fresh, empty directory under the build's temporary directory.
fn fresh_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("service_jobs")
        .join(format!("{label}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn start(data_dir: Option<&Path>, workers: usize, ent: Entitlement) -> ServerHandle {
    let mut cfg = ServeConfig::new("127.0.0.1:0", ent);
    cfg.data_dir = data_dir.map(Path::to_path_buf);
    cfg.workers = workers;
    serve::start(cfg).expect("server starts")
}

struct Resp {
    status: u16,
    head: String,
    body: String,
}

impl Resp {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("not JSON ({e}): {}", &self.body))
    }
}

/// One HTTP/1.1 request; the server closes the connection after its response.
fn http_raw(addr: SocketAddr, raw: &[u8]) -> Resp {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(120))).unwrap();
    s.write_all(raw).expect("send");
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).expect("read");
    let text = String::from_utf8_lossy(&buf).to_string();
    let (head, body) = text.split_once("\r\n\r\n").expect("a response head");
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("a status code");
    Resp {
        status,
        head: head.to_string(),
        body: body.to_string(),
    }
}

fn http(addr: SocketAddr, method: &str, path: &str, body: &str, headers: &[(&str, &str)]) -> Resp {
    let mut raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        raw.push_str(&format!("{k}: {v}\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    http_raw(addr, raw.as_bytes())
}

fn get(addr: SocketAddr, path: &str) -> Resp {
    http(addr, "GET", path, "", &[])
}

fn submit(addr: SocketAddr, op: &str, request: Value) -> Resp {
    http(
        addr,
        "POST",
        "/jobs",
        &json!({"op": op, "request": request}).to_string(),
        &[("Content-Type", "application/json")],
    )
}

fn submit_run(addr: SocketAddr, scenario: &str) -> String {
    let r = submit(addr, "run", json!({ "scenario": scenario }));
    assert!(
        r.status == 202 || r.status == 200,
        "submit: {} {}",
        r.status,
        r.body
    );
    r.json()["job_id"].as_str().unwrap().to_string()
}

fn wait_finished(addr: SocketAddr, id: &str) -> Value {
    let t0 = Instant::now();
    loop {
        let s = get(addr, &format!("/jobs/{id}"));
        assert_eq!(s.status, 200, "{}", s.body);
        let v = s.json();
        if v["state"] == "succeeded" || v["state"] == "failed" {
            return v;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(300),
            "job {id} did not finish: {v}"
        );
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn direct(src: &str) -> kshana::api::RunOutput {
    kshana::api::run_scenario(src).expect("the direct run succeeds")
}

fn read_status_file(data: &Path, id: &str) -> Value {
    let s = std::fs::read_to_string(data.join("jobs").join(id).join("status.json")).unwrap();
    serde_json::from_str(&s).unwrap()
}

// ─── 1. The 5000-satellite design ───────────────────────────────────────────────

#[test]
fn mega_constellation_job_is_byte_identical_to_the_direct_run() {
    let data = fresh_dir("mega");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let src = fixture("leo-pnt-mega-shell.toml");
    let id = submit_run(addr, &src);
    let st = wait_finished(addr, &id);
    assert_eq!(st["state"], "succeeded", "{st}");
    assert_eq!(st["kind"], "constellation-design");

    let want = direct(&src);
    let got = get(addr, &format!("/jobs/{id}/result"));
    assert_eq!(got.status, 200);
    assert_eq!(
        got.body, want.json,
        "result must equal the direct run byte for byte"
    );
    let v = got.json();
    let wv: Value = serde_json::from_str(&want.json).unwrap();
    assert_eq!(v["total_satellites"], 5000);
    assert_eq!(
        v["global"]["availability_pct"],
        wv["global"]["availability_pct"]
    );
    assert_eq!(v["global"]["availability_pct"].as_f64(), Some(99.2187));

    // Artefacts: the chart and the table exactly as the run produced them.
    if !want.svg.trim().is_empty() {
        let svg = get(addr, &format!("/jobs/{id}/artefacts/chart.svg"));
        assert_eq!(svg.status, 200);
        assert!(svg.head.contains("image/svg+xml"));
        assert_eq!(svg.body, want.svg);
    }
    match &want.csv {
        Some(csv) => assert_eq!(
            &get(addr, &format!("/jobs/{id}/artefacts/table.csv")).body,
            csv
        ),
        None => assert_eq!(
            get(addr, &format!("/jobs/{id}/artefacts/table.csv")).status,
            404
        ),
    }
    // The engine's evidence label travels with the job.
    assert_eq!(st["label"], wv["label"]);
    assert_eq!(st["evidence_tier"], "MODELLED");
    srv.stop();
}

// ─── 2. The 200-seed campaign ───────────────────────────────────────────────────

#[test]
fn two_hundred_seed_campaign_job_matches_the_direct_run() {
    let data = fresh_dir("campaign");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let src = fixture("campaign-monte-carlo-clock-holdover.toml");
    let id = submit_run(addr, &src);
    assert_eq!(wait_finished(addr, &id)["state"], "succeeded");
    let got = get(addr, &format!("/jobs/{id}/result"));
    let want = direct(&src);
    assert_eq!(got.body, want.json);
    let v = got.json();
    let wv: Value = serde_json::from_str(&want.json).unwrap();
    assert_eq!(v["monte_carlo"]["runs"], 200);
    assert_eq!(
        v["reproducibility"]["run_digest"],
        wv["reproducibility"]["run_digest"]
    );
    assert_eq!(
        v["reproducibility"]["run_digest"].as_str().unwrap().len(),
        64
    );
    srv.stop();
}

// ─── 3. Idempotent submission ───────────────────────────────────────────────────

#[test]
fn the_same_request_twice_is_one_job_and_one_execution() {
    let data = fresh_dir("idem");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let src = fixture("ephemeris.toml");
    let a = submit(addr, "run", json!({ "scenario": src }));
    let b = submit(addr, "run", json!({ "scenario": src }));
    assert_eq!(a.status, 202, "{}", a.body);
    assert_eq!(b.status, 200, "{}", b.body);
    assert_eq!(a.json()["created"], true);
    assert_eq!(b.json()["created"], false);
    let id = a.json()["job_id"].as_str().unwrap().to_string();
    assert_eq!(b.json()["job_id"], id.as_str());
    assert_eq!(wait_finished(addr, &id)["attempts"], 1);
    // Submitting again after it finished still runs nothing.
    let c = submit(addr, "run", json!({ "scenario": src }));
    assert_eq!(c.status, 200);
    assert_eq!(c.json()["state"], "succeeded");
    std::thread::sleep(Duration::from_millis(200));
    // Count executions in the store: one job directory, one attempt.
    let dirs = std::fs::read_dir(data.join("jobs")).unwrap().count();
    assert_eq!(dirs, 1);
    assert_eq!(read_status_file(&data, &id)["attempts"], 1);
    srv.stop();
}

// ─── 4. Restart recovery, with a control ────────────────────────────────────────

/// Submit a slow job and four quick ones to a one-worker server, stop it while the
/// quick ones wait, mark one of them as a crash would leave it (running), then start
/// a new server on the same directory. Returns (data dir, job ids, sources, handle).
fn interrupted_sequence(recover: bool) -> (PathBuf, Vec<(String, String)>, ServerHandle) {
    let data = fresh_dir(if recover { "recover" } else { "norecover" });
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let names = [
        "campaign-monte-carlo-clock-holdover.toml",
        "ephemeris.toml",
        "link-budget.toml",
        "passes.toml",
        "space-packet.toml",
    ];
    let jobs: Vec<(String, String)> = names
        .iter()
        .map(|n| {
            let src = fixture(n);
            (submit_run(addr, &src), src)
        })
        .collect();
    srv.stop();

    let unfinished: Vec<&String> = jobs
        .iter()
        .map(|(id, _)| id)
        .filter(|id| {
            let s = read_status_file(&data, id);
            s["state"] == "queued" || s["state"] == "running"
        })
        .collect();
    assert!(
        unfinished.len() >= 2,
        "precondition: the stop must leave jobs unfinished (left {})",
        unfinished.len()
    );
    // A crash mid-run leaves a job in state running: simulate that on one of them.
    let crashed = unfinished.last().unwrap().to_string();
    let p = data.join("jobs").join(&crashed).join("status.json");
    let mut s: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    s["state"] = json!("running");
    std::fs::write(&p, serde_json::to_string_pretty(&s).unwrap()).unwrap();

    let mut cfg = ServeConfig::new("127.0.0.1:0", enterprise());
    cfg.data_dir = Some(data.clone());
    cfg.workers = 1;
    cfg.recover_on_start = recover;
    let srv2 = serve::start(cfg).unwrap();
    (data, jobs, srv2)
}

#[test]
fn a_restart_finishes_every_queued_and_interrupted_job() {
    let (_data, jobs, srv) = interrupted_sequence(true);
    let addr = srv.local_addr();
    for (id, src) in &jobs {
        let st = wait_finished(addr, id);
        assert_eq!(st["state"], "succeeded", "{st}");
        let got = get(addr, &format!("/jobs/{id}/result"));
        assert_eq!(got.body, direct(src).json, "job {id}");
    }
    // A finished job is never re-run: the first job finished before the stop and
    // still has exactly one attempt.
    let first = wait_finished(addr, &jobs[0].0);
    assert_eq!(first["attempts"], 1);
    srv.stop();
}

#[test]
fn control_without_recovery_the_same_sequence_leaves_jobs_unfinished() {
    let (data, jobs, srv) = interrupted_sequence(false);
    std::thread::sleep(Duration::from_millis(1500));
    let left = jobs
        .iter()
        .filter(|(id, _)| {
            let s = read_status_file(&data, id);
            s["state"] == "queued" || s["state"] == "running"
        })
        .count();
    assert!(
        left >= 2,
        "without recovery the interrupted jobs must stay unfinished"
    );
    srv.stop();
}

// ─── 5. Entitlement ─────────────────────────────────────────────────────────────

#[test]
fn a_pro_kind_at_evaluation_is_refused_and_nothing_is_queued() {
    let data = fresh_dir("ent");
    let srv = start(Some(&data), 1, evaluation_named());
    let addr = srv.local_addr();
    let r = submit(addr, "run", json!({"scenario": "kind = \"pro-twin\"\n"}));
    assert_eq!(r.status, 403, "{}", r.body);
    assert!(r.body.contains("tier"));
    assert!(!r.body.contains(LICENSEE));
    let d = submit(
        addr,
        "dossier",
        json!({"scenario": fixture("clock-holdover.toml")}),
    );
    assert_eq!(d.status, 403);
    assert!(!d.body.contains(LICENSEE));
    let list = get(addr, "/jobs").json();
    assert_eq!(list["total"], 0);
    assert_eq!(std::fs::read_dir(data.join("jobs")).unwrap().count(), 0);
    srv.stop();
}

// ─── 6. Failure is never success ────────────────────────────────────────────────

#[test]
fn an_engine_rejection_is_a_failed_job_never_a_delivery() {
    let data = fresh_dir("fail");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let bad = "kind = \"ephemeris\"\nstep_s = 10\n";
    let engine_error = kshana::api::run_scenario(bad).unwrap_err().to_string();
    let id = submit_run(addr, bad);
    let st = wait_finished(addr, &id);
    assert_eq!(st["state"], "failed");
    assert_eq!(st["error"], engine_error.as_str());
    let r = get(addr, &format!("/jobs/{id}/result"));
    assert_eq!(r.status, 409);
    assert_eq!(r.json()["job_error"], engine_error.as_str());
    let ledger = get(addr, "/ledger").body;
    assert!(
        !ledger.contains(&id),
        "a failed job must not be in the ledger"
    );
    assert_eq!(get(addr, "/ledger/verify").json()["verified"], true);
    srv.stop();
}

// ─── 7. Hostile input ───────────────────────────────────────────────────────────

#[test]
fn hostile_input_is_refused() {
    let data = fresh_dir("hostile");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    // Above the cap: refused with 413 before the body is read.
    let over = format!(
        "POST /jobs HTTP/1.1\r\nHost: t\r\nContent-Length: {}\r\n\r\n",
        serve::JOBS_MAX_BODY_BYTES + 1
    );
    assert_eq!(http_raw(addr, over.as_bytes()).status, 413);
    let over_run = format!(
        "POST /run HTTP/1.1\r\nHost: t\r\nContent-Length: {}\r\n\r\n",
        serve::MAX_BODY_BYTES + 1
    );
    assert_eq!(http_raw(addr, over_run.as_bytes()).status, 413);

    let id = "0".repeat(64);
    for name in ["..", "../status.json", "a/b", "..%2Fstatus.json", ".hidden"] {
        let r = get(addr, &format!("/jobs/{id}/artefacts/{name}"));
        assert_eq!(r.status, 400, "artefact name {name:?}: {}", r.body);
    }
    for bad in [
        "xyz".to_string(),
        "a".repeat(63),
        "A".repeat(64),
        format!("{}g", "a".repeat(63)),
    ] {
        assert_eq!(get(addr, &format!("/jobs/{bad}")).status, 400, "{bad}");
        assert_eq!(
            get(addr, &format!("/jobs/{bad}/result")).status,
            400,
            "{bad}"
        );
    }
    assert_eq!(get(addr, &format!("/jobs/{id}")).status, 404);

    let unknown = submit(addr, "launch-missiles", json!({}));
    assert_eq!(unknown.status, 422);
    assert!(unknown.json()["operations"].as_array().unwrap().len() >= 3);
    let malformed = http(addr, "POST", "/jobs", "{\"op\": \"run\",", &[]);
    assert_eq!(malformed.status, 400, "{}", malformed.body);
    assert_eq!(http(addr, "POST", "/jobs", "[1,2]", &[]).status, 422);
    assert_eq!(submit(addr, "run", json!({"scenario": 7})).status, 422);
    assert_eq!(get(addr, "/jobs?limit=0").status, 400);
    assert_eq!(get(addr, "/jobs?state=done").status, 400);
    assert_eq!(std::fs::read_dir(data.join("jobs")).unwrap().count(), 0);
    srv.stop();
}

// ─── 8. The OpenAPI document equals the route table, both ways ──────────────────

#[test]
fn the_openapi_document_lists_exactly_the_served_routes() {
    let data = fresh_dir("openapi");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let id = submit_run(addr, &fixture("ephemeris.toml"));
    assert_eq!(wait_finished(addr, &id)["state"], "succeeded");

    let doc = get(addr, "/openapi.json").json();
    assert_eq!(doc["openapi"], "3.1.0");
    let mut documented: Vec<(String, String)> = Vec::new();
    for (path, ops) in doc["paths"].as_object().unwrap() {
        for m in ops.as_object().unwrap().keys() {
            documented.push((m.to_ascii_uppercase(), path.clone()));
        }
    }
    let mut table: Vec<(String, String)> = serve::api::served_routes(true)
        .map(|r| (r.method.to_string(), r.path.to_string()))
        .collect();
    documented.sort();
    table.sort();
    assert_eq!(documented, table, "document and route table differ");

    // Every documented path is really served: the router's own 404 is
    // {"error":"not found"}; a served route never answers with it.
    let router_404 = get(addr, "/no-such-route");
    assert_eq!(router_404.status, 404);
    assert_eq!(router_404.json()["error"], "not found");
    for (method, path) in &documented {
        let concrete = path.replace("{id}", &id).replace("{name}", "chart.svg");
        let r = http(addr, method, &concrete, "", &[]);
        let is_router_404 = r.status == 404
            && serde_json::from_str::<Value>(&r.body)
                .map(|v| v["error"] == "not found")
                .unwrap_or(false);
        assert!(
            !is_router_404,
            "{method} {path} is documented but not served"
        );
    }
    // And the server without a data directory documents only what it serves.
    let plain = start(None, 1, enterprise());
    let pdoc = get(plain.local_addr(), "/openapi.json").json();
    assert!(pdoc["paths"].get("/jobs").is_none());
    assert_eq!(get(plain.local_addr(), "/jobs").status, 404);
    plain.stop();
    srv.stop();
}

// ─── 9. The ledger ──────────────────────────────────────────────────────────────

#[test]
fn the_ledger_verifies_and_names_a_changed_record() {
    let data = fresh_dir("ledger");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let a = submit_run(addr, &fixture("ephemeris.toml"));
    assert_eq!(wait_finished(addr, &a)["state"], "succeeded");
    let b = submit_run(addr, &fixture("link-budget.toml"));
    assert_eq!(wait_finished(addr, &b)["state"], "succeeded");

    let v = get(addr, "/ledger/verify").json();
    assert_eq!(v["verified"], true, "{v}");
    assert_eq!(v["records"], 6);
    assert_eq!(v["delivered_jobs"], 2);
    assert_eq!(v["store_checked"], 2);

    // A stored result changed after delivery is caught by the store check.
    let result_path = data.join("jobs").join(&a).join("result.json");
    let original = std::fs::read(&result_path).unwrap();
    let mut changed = original.clone();
    let pos = changed.iter().position(|c| c.is_ascii_digit()).unwrap();
    changed[pos] = if changed[pos] == b'9' {
        b'8'
    } else {
        changed[pos] + 1
    };
    std::fs::write(&result_path, &changed).unwrap();
    let v = get(addr, "/ledger/verify").json();
    assert_eq!(v["verified"], false);
    assert_eq!(v["chain_intact"], true);
    assert_eq!(v["store_mismatches"][0]["job_id"], a.as_str());
    std::fs::write(&result_path, &original).unwrap();
    assert_eq!(get(addr, "/ledger/verify").json()["verified"], true);

    // One byte of the ledger file: a hexadecimal digit of record 3's request hash
    // (record 3 is the first record of the second job).
    let ledger_path = data.join("ledger.json");
    let mut bytes = std::fs::read(&ledger_path).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    let key = "\"request_hash\": \"";
    let at = text.match_indices(key).nth(3).expect("four records").0 + key.len();
    bytes[at] = if bytes[at] == b'0' { b'1' } else { b'0' };
    std::fs::write(&ledger_path, &bytes).unwrap();
    let v = get(addr, "/ledger/verify").json();
    assert_eq!(v["verified"], false, "{v}");
    assert_eq!(v["chain_intact"], false);
    assert_eq!(v["first_bad_record"], 3);
    assert_eq!(v["first_bad_job_id"], b.as_str());
    assert!(v["reason"].as_str().unwrap().contains("record 3"));
    srv.stop();
}

// ─── 10. The access token ───────────────────────────────────────────────────────

/// A spawned server process that is killed and reaped when this value is dropped, so a
/// failing assertion (which unwinds past the end of the test) never leaves an orphaned
/// server listening on a port.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_spawned_server_is_killed_when_its_guard_is_dropped() {
    // The token test below spawns the real binary; if any of its assertions fails, the
    // guard must still stop the process. Prove the guard does so on a panic.
    let pid = std::panic::catch_unwind(|| {
        let guard = KillOnDrop(
            Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
                .args(["serve", "--dev", "--addr", "127.0.0.1:0"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn the server binary"),
        );
        let pid = guard.0.id();
        std::panic::panic_any(pid);
    })
    .expect_err("the closure panics")
    .downcast::<u32>()
    .map(|b| *b)
    .expect("the panic carries the process identifier");
    // `kill -0` succeeds only while a process with that identifier exists.
    let alive = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(!alive, "the server process {pid} outlived its guard");
}

#[test]
fn the_access_token_guards_every_route_but_health_and_never_leaks() {
    let dir = fresh_dir("token");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let token = format!("tok-{nanos:x}-q7Zp");
    let token_file = dir.join("token.txt");
    std::fs::write(&token_file, format!("{token}\n")).unwrap();
    let data = dir.join("data");

    let mut child = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(["serve", "--dev", "--addr", "127.0.0.1:0", "--data-dir"])
            .arg(&data)
            .arg("--api-token-file")
            .arg(&token_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn the server binary"),
    );
    let mut err = BufReader::new(child.0.stderr.take().unwrap());
    let mut log = String::new();
    let addr: SocketAddr = loop {
        let mut line = String::new();
        assert!(
            err.read_line(&mut line).unwrap() > 0,
            "server exited: {log}"
        );
        log.push_str(&line);
        if let Some(rest) = line.split("serving on http://").nth(1) {
            break rest.split_whitespace().next().unwrap().parse().unwrap();
        }
    };
    let reader = std::thread::spawn(move || {
        let mut rest = String::new();
        let _ = err.read_to_string(&mut rest);
        rest
    });

    let bearer = format!("Bearer {token}");
    let mut seen = String::new();
    let mut check = |r: Resp, want: u16| {
        assert_eq!(r.status, want, "{}", r.body);
        seen.push_str(&r.head);
        seen.push_str(&r.body);
    };
    check(get(addr, "/jobs"), 401);
    check(
        http(
            addr,
            "GET",
            "/jobs",
            "",
            &[("Authorization", "Bearer wrong")],
        ),
        401,
    );
    check(
        http(
            addr,
            "GET",
            "/jobs",
            "",
            &[("Authorization", &format!("Bearer {token}x"))],
        ),
        401,
    );
    check(
        http(addr, "GET", "/jobs", "", &[("Authorization", &bearer)]),
        200,
    );
    check(get(addr, "/health"), 200);
    check(get(addr, "/openapi.json"), 401);
    check(get(addr, "/no-such-route"), 401);
    let sub = http(
        addr,
        "POST",
        "/jobs",
        &json!({"op": "run", "request": {"scenario": fixture("ephemeris.toml")}}).to_string(),
        &[("Authorization", &bearer)],
    );
    let id = sub.json()["job_id"].as_str().unwrap().to_string();
    check(sub, 202);
    let t0 = Instant::now();
    loop {
        let r = http(
            addr,
            "GET",
            &format!("/jobs/{id}"),
            "",
            &[("Authorization", &bearer)],
        );
        let done = r.json()["state"] == "succeeded";
        check(r, 200);
        if done {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(50));
    }
    check(
        http(
            addr,
            "GET",
            &format!("/jobs/{id}/result"),
            "",
            &[("Authorization", &bearer)],
        ),
        200,
    );
    check(
        http(
            addr,
            "GET",
            "/openapi.json",
            "",
            &[("Authorization", &bearer)],
        ),
        200,
    );
    check(
        http(addr, "GET", "/", "", &[("Authorization", &bearer)]),
        200,
    );

    drop(child); // kills the server and reaps it, so its stderr reaches end of file
    log.push_str(&reader.join().unwrap());
    assert!(log.contains("access token: required"), "{log}");
    assert!(
        log.contains("succeeded"),
        "the job log line is captured: {log}"
    );
    assert!(!log.contains(&token), "the token must never be logged");
    assert!(
        !seen.contains(&token),
        "the token must never be in a response"
    );
    assert!(seen.contains("WWW-Authenticate: Bearer"));
}

// ─── 11. POST /study ────────────────────────────────────────────────────────────

fn study_body(scenario: Option<&str>) -> String {
    let mut req = json!({"pack": "clock", "customer": "Example Organisation", "inputs": {}});
    if let Some(s) = scenario {
        req["inputs"]["scenario"] = json!(s);
    }
    req.to_string()
}

#[test]
fn study_without_a_data_directory_is_unchanged() {
    let srv = start(None, 1, enterprise());
    let body = study_body(Some(&fixture("clock-holdover.toml")));
    let r = http(srv.local_addr(), "POST", "/study", &body, &[]);
    let old = serve::handle(
        &serve::HttpRequest {
            method: "POST".into(),
            path: "/study".into(),
            body: body.clone(),
        },
        &enterprise(),
        0,
    );
    assert_eq!(r.status, old.status);
    assert_eq!(r.body, old.body, "byte-identical to the original handler");
    assert!(r.json().get("job_id").is_none());
    srv.stop();
}

#[test]
fn study_with_a_data_directory_queues_the_dossier_job() {
    let data = fresh_dir("study");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let scenario = fixture("clock-holdover.toml");
    let body = study_body(Some(&scenario));
    let r = http(addr, "POST", "/study", &body, &[]);
    assert_eq!(r.status, 200, "{}", r.body);
    let v = r.json();
    let old: Value = serde_json::from_str(
        &serve::handle(
            &serve::HttpRequest {
                method: "POST".into(),
                path: "/study".into(),
                body: body.clone(),
            },
            &enterprise(),
            0,
        )
        .body,
    )
    .unwrap();
    for (k, val) in old.as_object().unwrap() {
        assert_eq!(&v[k], val, "old field {k} kept");
    }
    let id = v["job_id"].as_str().expect("job_id added").to_string();
    assert_eq!(wait_finished(addr, &id)["state"], "succeeded");
    let dossier = get(addr, &format!("/jobs/{id}/artefacts/dossier.json")).json();
    assert!(get(addr, &format!("/jobs/{id}/artefacts/dossier.html"))
        .body
        .contains("<html"));

    // The same scenario through `kshana-pro study --dossier`.
    let dir = fresh_dir("study-cli");
    let scen = dir.join("clock-holdover.toml");
    std::fs::write(&scen, &scenario).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(["study", "--dossier"])
        .arg(&scen)
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--dev")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cli_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(cli_hash.len(), 64);
    assert_eq!(dossier["result_hash"], cli_hash.as_str());

    // A study with no scenario is still acknowledged, with no job.
    let r = http(addr, "POST", "/study", &study_body(None), &[]);
    assert_eq!(r.status, 200);
    assert!(r.json()["job_id"].is_null());
    srv.stop();
}

// ─── 12. No outbound connection ─────────────────────────────────────────────────

#[test]
fn the_only_outgoing_socket_in_serve_is_the_self_health_check() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/serve");
    let mut hits = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let p = entry.unwrap().path();
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        for pat in [
            "TcpStream::connect",
            "UdpSocket",
            "connect_timeout",
            "ToSocketAddrs",
        ] {
            for (i, _) in text.match_indices(pat) {
                let fn_start = text[..i].rfind("fn ").unwrap_or(0);
                let fn_name: String = text[fn_start + 3..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                hits.push((
                    p.file_name().unwrap().to_string_lossy().to_string(),
                    pat,
                    fn_name,
                ));
            }
        }
    }
    assert_eq!(
        hits,
        vec![(
            "mod.rs".to_string(),
            "TcpStream::connect",
            "healthcheck".to_string()
        )],
        "unexpected outgoing socket use in src/serve"
    );
}

// ─── 13. Concurrency ────────────────────────────────────────────────────────────

#[test]
fn three_workers_finish_six_jobs_each_equal_to_its_direct_run() {
    let data = fresh_dir("concurrent");
    let srv = start(Some(&data), 3, enterprise());
    let addr = srv.local_addr();
    let names = [
        "ephemeris.toml",
        "link-budget.toml",
        "passes.toml",
        "space-packet.toml",
        "launch-window.toml",
        "timetransfer.toml",
    ];
    let jobs: Vec<(String, String)> = names
        .iter()
        .map(|n| {
            let src = fixture(n);
            (submit_run(addr, &src), src)
        })
        .collect();
    let mut ids: Vec<&String> = jobs.iter().map(|(i, _)| i).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 6, "six different jobs");
    for (id, src) in &jobs {
        assert_eq!(wait_finished(addr, id)["state"], "succeeded");
        assert_eq!(
            get(addr, &format!("/jobs/{id}/result")).body,
            direct(src).json
        );
    }
    let list = get(addr, "/jobs?limit=3").json();
    assert_eq!(list["total"], 6);
    assert_eq!(list["returned"], 3);
    assert_eq!(list["counts"]["succeeded"], 6);
    // Newest first.
    assert_eq!(list["jobs"][0]["job_id"], jobs[5].0.as_str());
    assert_eq!(get(addr, "/ledger/verify").json()["delivered_jobs"], 6);
    srv.stop();
}

// ─── Further checks: the ci operation, the job board, the fixtures ──────────────

#[test]
fn a_ci_job_equals_the_pack_c_run_on_the_same_files() {
    let pack_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pack-c");
    let manifest = std::fs::read_to_string(pack_dir.join("pack.toml")).unwrap();
    let pack = kshana_pro::pack_c::parse_pack(&manifest).unwrap();
    let mut scenarios = serde_json::Map::new();
    for e in &pack.entries {
        scenarios.insert(
            e.path.clone(),
            json!(std::fs::read_to_string(pack_dir.join(&e.path)).unwrap()),
        );
    }
    let data = fresh_dir("ci");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let r = submit(
        addr,
        "ci",
        json!({"manifest": manifest, "scenarios": scenarios}),
    );
    assert_eq!(r.status, 202, "{}", r.body);
    let id = r.json()["job_id"].as_str().unwrap().to_string();
    assert_eq!(wait_finished(addr, &id)["state"], "succeeded");
    let want = kshana_pro::pack_c::run_ci(
        &pack,
        &pack_dir,
        None,
        &kshana_pro::pack_c::CiContext::default(),
    )
    .unwrap();
    assert_eq!(
        get(addr, &format!("/jobs/{id}/result")).body,
        want.to_json()
    );
    assert_eq!(
        get(addr, &format!("/jobs/{id}/artefacts/ci.json")).body,
        want.to_json()
    );
    let report = get(addr, &format!("/jobs/{id}/artefacts/report.html")).body;
    assert!(report.contains("<html"));
    // What the job board and the guide say about a ci job's evidence label must stay
    // true: ci.json carries no tier word, and report.html states MODELLED.
    let result = get(addr, &format!("/jobs/{id}/result")).body;
    for tier in ["VALIDATED", "MODELLED", "PARTNER"] {
        assert!(!result.contains(tier), "ci.json now carries {tier}");
    }
    assert!(report.contains("MODELLED"));
    let st = get(addr, &format!("/jobs/{id}")).json();
    assert!(st.get("evidence_tier").is_none(), "{st}");
    srv.stop();
}

#[test]
fn the_job_board_is_self_contained_and_lists_real_jobs() {
    let data = fresh_dir("board");
    let srv = start(Some(&data), 1, enterprise());
    let addr = srv.local_addr();
    let id = submit_run(addr, &fixture("ephemeris.toml"));
    assert_eq!(wait_finished(addr, &id)["state"], "succeeded");
    let page = get(addr, "/");
    assert_eq!(page.status, 200);
    assert!(page.head.contains("text/html"));
    assert!(
        page.body.contains(&id),
        "the embedded snapshot lists the job"
    );
    for external in ["http://", "https://", "<link", "src=\"", "@import"] {
        assert!(
            !page.body.contains(external),
            "external asset marker {external:?}"
        );
    }
    assert!(page.head.contains("Content-Security-Policy"));

    // A label with no leading tier word is carried as it is, and no tier is assigned:
    // the engine's link-budget label begins with a description, not a tier.
    let lb = submit_run(addr, &fixture("link-budget.toml"));
    let st = wait_finished(addr, &lb);
    assert_eq!(st["state"], "succeeded");
    let want: Value = serde_json::from_str(&direct(&fixture("link-budget.toml")).json).unwrap();
    assert_eq!(st["label"], want["label"]);
    let label = st["label"]
        .as_str()
        .expect("the link-budget result has a label");
    assert!(
        !["VALIDATED", "MODELLED", "PARTNER"]
            .iter()
            .any(|t| label.trim_start().starts_with(t)),
        "this check needs a label with no leading tier word: {label}"
    );
    assert!(st.get("evidence_tier").is_none(), "{st}");
    // The page never tells a reader that a result carries labels it does not carry.
    let page = get(addr, "/").body;
    assert!(page.contains("no tier word in the label"));
    assert!(page.contains("its ci.json carries no label"));
    assert!(
        !page.contains("CI (continuous integration) results carry a label per figure"),
        "the board must not claim per-figure labels for ci results"
    );
    srv.stop();
}

#[test]
fn fixtures_never_touch_the_optional_preset() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/service");
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_lowercase();
        let text = std::fs::read_to_string(&p).unwrap().to_lowercase();
        assert!(!name.contains("celeste"), "{name}");
        assert!(
            !text.contains("celeste"),
            "{name} mentions the optional preset"
        );
        n += 1;
    }
    assert!(n >= 12, "the scan saw the fixture files ({n})");
}
