// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Service hardening (roadmap F2a), over real sockets: cancellation and time limits,
//! priority lanes, the server-sent event stream, TLS in the binary, failover between
//! two nodes on one shared data directory, backup and restore, and a concurrent load.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::{self, ha, ServeConfig, ServerHandle};
use serde_json::{json, Value};

fn enterprise() -> Entitlement {
    Entitlement {
        licensee: "hardening".into(),
        tier: Tier::Enterprise,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

fn fresh_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("service_hardening")
        .join(format!("{label}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
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

/// A Monte Carlo job of a few seconds (100 seeds of a 2-hour clock run at 1 s steps).
fn slow_scenario() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/batch-clock-mc.toml"),
    )
    .unwrap()
    .replace("seed_count = 64 ", "seed_count = 100")
    .replace("step_s = 10.0", "step_s = 1.0")
}

fn start(dir: &Path, workers: usize) -> ServerHandle {
    let mut cfg = ServeConfig::new("127.0.0.1:0", enterprise());
    cfg.data_dir = Some(dir.to_path_buf());
    cfg.workers = workers;
    serve::start(cfg).expect("server starts")
}

fn http(addr: SocketAddr, method: &str, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let status = out[9..12].parse().unwrap();
    let body = out
        .split_once("\r\n\r\n")
        .map(|x| x.1)
        .unwrap_or("")
        .to_string();
    (status, body)
}

fn submit(addr: SocketAddr, doc: Value) -> (u16, Value) {
    let (st, b) = http(addr, "POST", "/jobs", &doc.to_string());
    (st, serde_json::from_str(&b).unwrap_or(Value::Null))
}

fn state(addr: SocketAddr, id: &str) -> Value {
    let (_, b) = http(addr, "GET", &format!("/jobs/{id}"), "");
    serde_json::from_str(&b).unwrap()
}

fn wait_state(addr: SocketAddr, id: &str, want: &[&str], secs: u64) -> Value {
    let t = Instant::now();
    loop {
        let v = state(addr, id);
        if want.contains(&v["state"].as_str().unwrap_or("")) {
            return v;
        }
        assert!(
            t.elapsed() < Duration::from_secs(secs),
            "job {id} stuck: {v}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn cancel_queued_and_running_jobs_and_time_limit() {
    let dir = fresh_dir("cancel");
    let srv = start(&dir, 1);
    let a = srv.local_addr();
    let (st, slow) = submit(
        a,
        json!({"op": "run", "request": {"scenario": slow_scenario()}}),
    );
    assert_eq!(st, 202);
    let slow = slow["job_id"].as_str().unwrap().to_string();
    wait_state(a, &slow, &["running"], 30);
    // A second job waits behind it; cancel it while queued.
    let (_, q) = submit(
        a,
        json!({"op": "run", "request": {"scenario": fixture("ephemeris.toml")}}),
    );
    let q = q["job_id"].as_str().unwrap().to_string();
    let (st, b) = http(a, "POST", &format!("/jobs/{q}/cancel"), "");
    assert_eq!(st, 202, "{b}");
    assert!(b.contains("\"done\""), "{b}");
    assert_eq!(state(a, &q)["state"], "cancelled");
    // Cancel the running job: it stops at once.
    let t = Instant::now();
    let (st, b) = http(a, "POST", &format!("/jobs/{slow}/cancel"), "");
    assert_eq!(st, 202, "{b}");
    let v = wait_state(a, &slow, &["cancelled"], 10);
    assert!(
        t.elapsed() < Duration::from_secs(2),
        "cancel took {:?}",
        t.elapsed()
    );
    assert!(v["result_sha256"].is_null());
    let (st, _) = http(a, "GET", &format!("/jobs/{slow}/result"), "");
    assert_eq!(st, 409);
    let (st, _) = http(a, "POST", &format!("/jobs/{slow}/cancel"), "");
    assert_eq!(st, 409, "a finished job cannot be cancelled");
    assert!(!dir.join("jobs").join(&slow).join("result.json").exists());
    // The abandoned run's output never lands, even after it would have finished.
    std::thread::sleep(Duration::from_secs(4));
    assert!(!dir.join("jobs").join(&slow).join("result.json").exists());
    let (_, list) = http(a, "GET", "/jobs?state=cancelled", "");
    assert!(list.contains(&slow) && list.contains(&q));

    // Time limit: ten times the slow work (tens of seconds even on a fast runner) with
    // timeout_secs = 1 fails "timed out".
    let slow2 = slow_scenario()
        .replace("seed_start = 1", "seed_start = 2")
        .replace("seed_count = 100", "seed_count = 1000");
    let (st, j) = submit(
        a,
        json!({"op": "run", "request": {"scenario": slow2}, "timeout_secs": 1, "priority": "high"}),
    );
    assert_eq!(st, 202, "{j}");
    let id = j["job_id"].as_str().unwrap().to_string();
    let v = wait_state(a, &id, &["failed", "succeeded"], 30);
    assert_eq!(v["state"], "failed");
    assert!(v["error"].as_str().unwrap().contains("timed out"), "{v}");
    assert_eq!(v["priority"], "high");
    assert_eq!(v["timeout_secs"], 1);

    // Bad options are refused.
    for bad in [
        json!({"op": "run", "request": {}, "priority": "urgent"}),
        json!({"op": "run", "request": {}, "timeout_secs": 0}),
    ] {
        assert_eq!(submit(a, bad).0, 422);
    }
    // The ledger holds only real deliveries.
    let (_, v) = http(a, "GET", "/ledger/verify", "");
    assert!(v.contains("\"verified\": true"), "{v}");
    srv.stop();
    std::thread::sleep(Duration::from_secs(3)); // let abandoned runs end before exit
}

#[test]
fn priority_lanes_run_high_before_normal_before_low() {
    let dir = fresh_dir("priority");
    // Store the jobs with no worker running, then start one worker.
    let mut cfg = ServeConfig::new("127.0.0.1:0", enterprise());
    cfg.data_dir = Some(dir.clone());
    cfg.workers = 1;
    cfg.recover_on_start = false;
    let srv = serve::start(cfg).unwrap();
    let a = srv.local_addr();
    // Occupy the single worker so the next three wait in the queue together.
    let (_, s) = submit(
        a,
        json!({"op": "run", "request": {"scenario": slow_scenario().replace("seed_count = 100", "seed_count = 30")}}),
    );
    let s = s["job_id"].as_str().unwrap().to_string();
    wait_state(a, &s, &["running"], 30);
    let mut ids = Vec::new();
    for (p, f) in [
        ("low", "ephemeris.toml"),
        ("normal", "passes.toml"),
        ("high", "link-budget.toml"),
    ] {
        let (st, j) = submit(
            a,
            json!({"op": "run", "request": {"scenario": fixture(f)}, "priority": p}),
        );
        assert_eq!(st, 202, "{j}");
        ids.push(j["job_id"].as_str().unwrap().to_string());
    }
    let started: Vec<i64> = ids
        .iter()
        .map(|id| {
            wait_state(a, id, &["succeeded", "failed"], 60);
            let v = state(a, id);
            assert_eq!(v["state"], "succeeded", "{v}");
            // Order of execution: the job directory's status was rewritten when it
            // started; compare the started stamps and, for ties, the attempts order
            // through the file modification time of result.json.
            v["started_unix"].as_i64().unwrap()
        })
        .collect();
    let finished = |id: &str| {
        std::fs::metadata(dir.join("jobs").join(id).join("result.json"))
            .unwrap()
            .modified()
            .unwrap()
    };
    assert!(started[2] <= started[1] && started[1] <= started[0]);
    assert!(
        finished(&ids[2]) <= finished(&ids[1]),
        "high ran after normal"
    );
    assert!(
        finished(&ids[1]) <= finished(&ids[0]),
        "normal ran after low"
    );
    srv.stop();
}

#[test]
fn event_stream_follows_a_job_to_the_end() {
    let dir = fresh_dir("events");
    let srv = start(&dir, 1);
    let a = srv.local_addr();
    let (_, j) = submit(
        a,
        json!({"op": "run", "request": {"scenario": slow_scenario().replace("seed_count = 100", "seed_count = 20")}}),
    );
    let id = j["job_id"].as_str().unwrap().to_string();
    let (st, body) = http(a, "GET", &format!("/jobs/{id}/events"), "");
    assert_eq!(st, 200);
    let events: Vec<Value> = body
        .split("\n\n")
        .filter_map(|e| e.lines().find_map(|l| l.strip_prefix("data: ")))
        .map(|d| serde_json::from_str(d).unwrap())
        .collect();
    assert!(events.len() >= 2, "{body}");
    assert_eq!(events.last().unwrap()["state"], "succeeded");
    assert!(body.starts_with("event: status\n"));
    // The internal stream header is never sent.
    let mut s = TcpStream::connect(a).unwrap();
    write!(s, "GET /jobs/{id}/events HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut raw = String::new();
    s.read_to_string(&mut raw).unwrap();
    assert!(raw.contains("Content-Type: text/event-stream"));
    assert!(!raw.contains("X-Kshana"), "{raw}");
    let (st, _) = http(a, "GET", &format!("/jobs/{}/events", "0".repeat(64)), "");
    assert_eq!(st, 404);
    srv.stop();
}

fn openssl(dir: &Path, args: &[&str]) -> bool {
    Command::new("openssl")
        .args(args)
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn tls_in_the_binary() {
    let dir = fresh_dir("tls");
    // A throwaway CA and a server key pair for 127.0.0.1, made with the openssl command.
    std::fs::write(
        dir.join("ext.cnf"),
        "subjectAltName=IP:127.0.0.1,DNS:localhost\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n",
    )
    .unwrap();
    let ok = openssl(
        &dir,
        &[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
            "-days",
            "2",
            "-subj",
            "/CN=test ca",
        ],
    ) && openssl(
        &dir,
        &[
            "req",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            "server.key",
            "-out",
            "server.csr",
            "-subj",
            "/CN=127.0.0.1",
        ],
    ) && openssl(
        &dir,
        &[
            "x509",
            "-req",
            "-in",
            "server.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-out",
            "server.pem",
            "-days",
            "2",
            "-extfile",
            "ext.cnf",
        ],
    );
    if !ok {
        eprintln!("skipped: the openssl command is not available");
        return;
    }
    let mut cfg = ServeConfig::new("127.0.0.1:0", enterprise());
    cfg.data_dir = Some(dir.join("data"));
    cfg.tls = Some((dir.join("server.pem"), dir.join("server.key")));
    let srv = serve::start(cfg).unwrap();
    let a = srv.local_addr();
    let curl = |extra: &[&str], url: String| {
        Command::new("curl")
            .args(["-sS", "--max-time", "10"])
            .args(extra)
            .arg(url)
            .output()
            .unwrap()
    };
    let out = curl(
        &["--cacert", dir.join("ca.pem").to_str().unwrap()],
        format!("https://{a}/health"),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "ok\n");
    let out = curl(
        &["--cacert", dir.join("ca.pem").to_str().unwrap()],
        format!("https://{a}/version"),
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("kshana-pro"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Without the CA the certificate is not trusted; plain HTTP gets no answer.
    assert!(!curl(&[], format!("https://{a}/health")).status.success());
    let plain = curl(&[], format!("http://{a}/health"));
    assert!(!String::from_utf8_lossy(&plain.stdout).contains("ok"));
    // A missing key file stops the server from starting.
    let mut bad = ServeConfig::new("127.0.0.1:0", enterprise());
    bad.tls = Some((dir.join("server.pem"), dir.join("absent.key")));
    assert!(serve::start(bad).is_err());
    srv.stop();
}

fn ha_node(dir: &Path, name: &str, ent: Entitlement) -> std::io::Result<ServerHandle> {
    let mut cfg = ServeConfig::new("127.0.0.1:0", ent);
    cfg.data_dir = Some(dir.to_path_buf());
    cfg.workers = 2;
    cfg.ha = Some(ha::HaConfig::new(name, 600).unwrap());
    serve::start(cfg)
}

#[test]
fn failover_between_two_nodes_on_one_data_directory() {
    let dir = fresh_dir("ha");
    // High availability is an Enterprise feature.
    let pro = Entitlement {
        tier: Tier::Pro,
        ..enterprise()
    };
    assert!(ha_node(&dir, "node-p", pro).is_err());

    let na = ha_node(&dir, "node-a", enterprise()).unwrap();
    let t = Instant::now();
    while ha::read_lease(&dir).map(|l| l.node) != Some("node-a".into()) {
        assert!(t.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    }
    let nb = ha_node(&dir, "node-b", enterprise()).unwrap();
    let (a, b) = (na.local_addr(), nb.local_addr());

    // A job submitted through the standby is run by the lease holder.
    let (st, j) = submit(
        b,
        json!({"op": "run", "request": {"scenario": fixture("ephemeris.toml")}}),
    );
    assert_eq!(st, 202);
    let j1 = j["job_id"].as_str().unwrap().to_string();
    wait_state(a, &j1, &["succeeded"], 30);
    assert_eq!(ha::read_lease(&dir).unwrap().node, "node-a");

    // node-a fails: it stops without giving up the lease.
    na.abandon();
    // Work submitted meanwhile, and a job node-a "was running" when it died.
    let (_, j) = submit(
        b,
        json!({"op": "run", "request": {"scenario": fixture("passes.toml")}}),
    );
    let j2 = j["job_id"].as_str().unwrap().to_string();
    let store = kshana_pro::serve::jobs::JobStore::open(&dir).unwrap();
    let (mut orphan, _) = store
        .submit_for(
            "default",
            "run",
            &json!({"scenario": fixture("link-budget.toml")}),
            Some("link-budget".into()),
        )
        .unwrap();
    orphan.state = kshana_pro::serve::jobs::JobState::Running;
    orphan.attempts = 1;
    store.write_status(&orphan).unwrap();

    // node-b takes the lease within about one lease time and finishes both.
    let t = Instant::now();
    wait_state(b, &j2, &["succeeded"], 30);
    let v = wait_state(b, &orphan.job_id, &["succeeded"], 30);
    assert!(t.elapsed() < Duration::from_secs(20));
    assert_eq!(v["attempts"], 2, "the dead node's job ran again");
    let l = ha::read_lease(&dir).unwrap();
    assert_eq!((l.node.as_str(), l.term), ("node-b", 2));
    let (_, v) = http(b, "GET", "/ledger/verify", "");
    assert!(v.contains("\"verified\": true"), "{v}");
    // Each node kept its own audit log.
    assert!(dir.join("audit/node-a").is_dir() && dir.join("audit/node-b").is_dir());
    nb.stop();
    assert!(
        ha::read_lease(&dir).is_none(),
        "a clean stop gives the lease up"
    );
}

#[test]
fn backup_while_serving_then_restore() {
    let dir = fresh_dir("backup");
    let data = dir.join("data");
    let srv = start(&data, 2);
    let a = srv.local_addr();
    let mut ids = Vec::new();
    for f in ["ephemeris.toml", "passes.toml", "link-budget.toml"] {
        let (_, j) = submit(a, json!({"op": "run", "request": {"scenario": fixture(f)}}));
        ids.push(j["job_id"].as_str().unwrap().to_string());
    }
    for id in &ids {
        wait_state(a, id, &["succeeded"], 60);
    }
    let bin = env!("CARGO_BIN_EXE_kshana-pro");
    let out = Command::new(bin)
        .args([
            "backup",
            "create",
            "--data-dir",
            data.to_str().unwrap(),
            "--out",
            dir.join("b1").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    srv.stop();
    let verify = |p: &Path| {
        let o = Command::new(bin)
            .args(["backup", "verify", p.to_str().unwrap()])
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).to_string(),
        )
    };
    let (ok, text) = verify(&dir.join("b1"));
    assert!(ok && text.contains("3 delivered job(s)"), "{text}");
    // A second backup to the same place is refused.
    assert!(!Command::new(bin)
        .args([
            "backup",
            "create",
            "--data-dir",
            data.to_str().unwrap(),
            "--out",
            dir.join("b1").to_str().unwrap()
        ])
        .status()
        .unwrap()
        .success());

    // Restore: the original is lost; the backup becomes the data directory.
    std::fs::remove_dir_all(&data).unwrap();
    std::fs::rename(dir.join("b1"), &data).unwrap();
    std::fs::remove_file(data.join("backup-manifest.json")).unwrap();
    let srv = start(&data, 1);
    let a = srv.local_addr();
    let (_, v) = http(a, "GET", "/ledger/verify", "");
    assert!(v.contains("\"verified\": true"), "{v}");
    let (st, _) = http(a, "GET", &format!("/jobs/{}/result", ids[0]), "");
    assert_eq!(st, 200);
    srv.stop();

    // A changed file in a backup is found.
    let out = Command::new(bin)
        .args([
            "backup",
            "create",
            "--data-dir",
            data.to_str().unwrap(),
            "--out",
            dir.join("b2").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let res = dir.join("b2/jobs").join(&ids[1]).join("result.json");
    let mut t = std::fs::read_to_string(&res).unwrap();
    t.push(' ');
    std::fs::write(&res, t).unwrap();
    let (ok, text) = verify(&dir.join("b2"));
    assert!(!ok && text.contains("changed"), "{text}");
}

#[test]
fn concurrent_load() {
    let dir = fresh_dir("load");
    let srv = start(&dir, 4);
    let a = srv.local_addr();
    let scen = fixture("ephemeris.toml");
    let t = Instant::now();
    let handles: Vec<_> = (0..32)
        .map(|i| {
            let scen = scen.clone();
            std::thread::spawn(move || {
                let mut codes = Vec::new();
                for k in 0..10 {
                    let (st, _) = match k % 3 {
                        0 => http(a, "GET", "/health", ""),
                        1 => http(a, "GET", "/jobs?limit=5", ""),
                        _ => http(
                            a,
                            "POST",
                            "/jobs",
                            &json!({"op": "run", "request": {"scenario": scen.replace("kind", &format!("# client {} {}\nkind", i % 8, k % 2))}}).to_string(),
                        ),
                    };
                    codes.push(st);
                }
                codes
            })
        })
        .collect();
    let codes: Vec<u16> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    assert_eq!(codes.len(), 320);
    assert!(codes.iter().all(|c| [200, 202].contains(c)), "{codes:?}");
    // 16 distinct submissions (8 clients x 2 variants), however many times each was sent.
    let (_, list) = http(a, "GET", "/jobs?limit=1000", "");
    let v: Value = serde_json::from_str(&list).unwrap();
    assert_eq!(v["total"], 16, "{v}");
    let t_all = Instant::now();
    loop {
        let (_, l) = http(a, "GET", "/jobs?state=succeeded&limit=1000", "");
        let v: Value = serde_json::from_str(&l).unwrap();
        if v["jobs"].as_array().map(|x| x.len()) == Some(16) {
            break;
        }
        assert!(t_all.elapsed() < Duration::from_secs(120), "{l}");
        std::thread::sleep(Duration::from_millis(100));
    }
    eprintln!("320 requests and 16 jobs in {:?}", t.elapsed());
    let (_, v) = http(a, "GET", "/ledger/verify", "");
    assert!(v.contains("\"verified\": true"), "{v}");
    srv.stop();
}
