// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The eLoran/GNSS simulator over the service (`src/serve/eloran.rs`, `src/eloran_service.rs`):
//! real sockets on 127.0.0.1. The free routes (example, validate, GRI database) answer at
//! Evaluation; run and rx are refused below Enterprise (feature `pro-eloran`) and succeed with
//! the Enterprise test entitlement, synchronously without a data directory and as a job with
//! one; the receiver report carries the truth comparison. Hardware output is not reachable.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kshana_eloran::Scenario;
use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::{self, ServeConfig, ServerHandle};
use serde_json::{json, Value};

/// One chain, four stations, no noise, E-field (the crate's receiver test scenario).
const RX_SCENARIO: &str = include_str!("../crates/kshana-eloran/scenarios/rx-high-snr.toml");

fn ent(tier: Tier) -> Entitlement {
    Entitlement {
        licensee: "ELORAN-TEST-LICENSEE".into(),
        tier,
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
        .join("eloran_service")
        .join(format!("{label}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn start(data_dir: Option<&Path>, e: Entitlement) -> ServerHandle {
    let mut cfg = ServeConfig::new("127.0.0.1:0", e);
    cfg.data_dir = data_dir.map(Path::to_path_buf);
    serve::start(cfg).expect("server starts")
}

struct Resp {
    status: u16,
    body: String,
}

impl Resp {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.body))
    }
}

fn http(addr: SocketAddr, method: &str, path: &str, body: &str) -> Resp {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(300))).unwrap();
    let raw = format!(
        "{method} {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(raw.as_bytes()).expect("send");
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).expect("read");
    let text = String::from_utf8_lossy(&buf).to_string();
    let (head, body) = text.split_once("\r\n\r\n").expect("a response head");
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("status");
    Resp {
        status,
        body: body.to_string(),
    }
}

fn get(addr: SocketAddr, path: &str) -> Resp {
    http(addr, "GET", path, "")
}

/// The receiver report's truth comparison: every station tracked, no cycle error, ToA error
/// RMS within a few nanoseconds, and a position fix within a few metres.
fn check_report(rep: &Value, run_id: &str) {
    assert_eq!(rep["format"], "kshana-eloran-rx", "{rep}");
    assert_eq!(rep["run_id"], run_id);
    assert_eq!(
        rep["scenario_sha256"],
        Scenario::from_toml(RX_SCENARIO).unwrap().hash()
    );
    assert_eq!(rep["stations_tracked"], 4, "{rep}");
    let st = rep["truth_comparison"]["stations"]
        .as_array()
        .expect("truth comparison");
    assert_eq!(st.len(), 4, "{rep}");
    for s in st {
        assert_eq!(s["cycle_errors"], 0, "{s}");
        assert!(s["toa_rms_ns"].as_f64().unwrap() < 10.0, "{s}");
    }
    let perr = rep["truth_comparison"]["position_error_m"]
        .as_f64()
        .expect("position error");
    assert!(perr < 10.0, "position error {perr} m");
    assert!(rep["position_solution"]["lat_deg"].is_number(), "{rep}");
    let refs = rep["time_by_reference"].as_array().unwrap();
    assert_eq!(refs[0]["time_ref"], "UTC(A)");
    assert!(
        (refs[0]["ref_minus_rx_ns"].as_f64().unwrap() - 50.0).abs() < 5.0,
        "{rep}"
    );
}

#[test]
fn free_routes_answer_at_evaluation() {
    let srv = start(None, Entitlement::evaluation());
    let addr = srv.local_addr();

    let ex = get(addr, "/eloran/example");
    assert_eq!(ex.status, 200);
    assert!(ex.body.contains("[[chains]]") && ex.body.contains("[scene]"));

    let ok = http(addr, "POST", "/eloran/validate", &ex.body).json();
    assert_eq!(ok["ok"], true);
    assert_eq!(
        ok["scenario_sha256"],
        Scenario::from_toml(&ex.body).unwrap().hash()
    );
    assert_eq!(ok["channels"], 2);

    let bad = http(addr, "POST", "/eloran/validate", "[scene]\nstart_utc = 3\n");
    assert_eq!(bad.status, 422);
    assert_eq!(bad.json()["ok"], false);
    assert!(bad.json()["error"].as_str().unwrap().contains("scenario"));
    // A station on an undefined time reference is refused by the build, not only the parser.
    let undefined = ex.body.replace("id = \"UTC(B)\"", "id = \"UTC(Z)\"");
    let r = http(addr, "POST", "/eloran/validate", &undefined);
    assert_eq!(r.status, 422, "{}", r.body);
    assert!(r.body.contains("UTC(B)"), "{}", r.body);

    let list = get(addr, "/eloran/gri").json();
    let chains = list["chains"].as_array().unwrap();
    assert!(chains.len() >= 30);
    for c in chains {
        assert!(
            c["region"].is_string() && c["status"].is_string() && c["confidence"].is_string(),
            "{c}"
        );
    }
    let complete = chains
        .iter()
        .find(|c| c["complete"] == true)
        .expect("a complete chain");
    let id = complete["id"].as_str().unwrap();
    let shown = get(addr, &format!("/eloran/gri/{id}")).json();
    assert_eq!(shown["chains"][0]["summary"]["id"], id);
    let snippet = shown["chains"][0]["snippet"].as_str().unwrap();
    assert!(snippet.contains("[[chains]]") && snippet.contains("[[chains.stations]]"));
    let region = complete["region"].as_str().unwrap();
    let by_region = get(
        addr,
        &format!("/eloran/gri?region={}", region.replace(' ', "%20")),
    )
    .json();
    assert!(by_region["count"].as_u64().unwrap() >= 1);
    assert!(by_region["chains"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["region"] == region));
    assert_eq!(get(addr, "/eloran/gri/no-such-chain-xyz").status, 404);
    assert_eq!(get(addr, "/eloran/gri?colour=red").status, 400);
}

#[test]
fn run_and_rx_are_refused_below_enterprise() {
    for tier in [Tier::Evaluation, Tier::Pro] {
        let srv = start(None, ent(tier));
        let addr = srv.local_addr();
        let r = http(addr, "POST", "/eloran/run", RX_SCENARIO);
        assert_eq!(r.status, 403, "{tier:?}: {}", r.body);
        assert!(
            r.body.contains("pro-eloran") && r.body.contains("enterprise"),
            "{}",
            r.body
        );
        assert!(
            !r.body.contains("ELORAN-TEST-LICENSEE"),
            "a refusal never names the licensee"
        );
        let r = http(
            addr,
            "POST",
            "/eloran/rx",
            &json!({"run": "a".repeat(64)}).to_string(),
        );
        assert_eq!(r.status, 403, "{tier:?}: {}", r.body);
    }
    // With a data directory, neither the route nor the job operation queues anything.
    let data = fresh_dir("refused");
    let srv = start(Some(&data), Entitlement::evaluation());
    let addr = srv.local_addr();
    assert_eq!(http(addr, "POST", "/eloran/run", RX_SCENARIO).status, 403);
    let r = http(
        addr,
        "POST",
        "/jobs",
        &json!({"op": "eloran-run", "request": {"scenario": RX_SCENARIO}}).to_string(),
    );
    assert_eq!(r.status, 403, "{}", r.body);
    assert_eq!(get(addr, "/jobs").json()["total"], 0);
    drop(srv);
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn synchronous_run_then_receiver_with_truth() {
    let srv = start(None, ent(Tier::Enterprise));
    let addr = srv.local_addr();
    let r = http(addr, "POST", "/eloran/run", RX_SCENARIO);
    assert_eq!(r.status, 200, "{}", r.body);
    let rec = r.json();
    assert_eq!(rec["mode"], "sync");
    assert_eq!(rec["format"], "kshana-eloran-run");
    assert_eq!(
        rec["scenario_sha256"],
        Scenario::from_toml(RX_SCENARIO).unwrap().hash()
    );
    assert_eq!(rec["evidence_label"], "MODELLED");
    assert!(rec["stats"]["pulses"].as_u64().unwrap() > 0);
    assert_eq!(rec["stats"]["samples_per_channel"], 1_200_000);
    let run_id = rec["run_id"].as_str().unwrap().to_string();
    assert_eq!(run_id.len(), 64);
    let samples = rec["files"]["samples"].as_str().unwrap();
    assert_eq!(
        std::fs::metadata(samples).unwrap().len(),
        1_200_000 * 2,
        "ri16, one channel"
    );
    assert!(Path::new(rec["files"]["truth"].as_str().unwrap()).is_file());
    assert!(rec["artefacts"]["samples_sha256"].as_str().unwrap().len() == 64);
    let pv = &rec["truth_preview"];
    assert_eq!(pv["columns"][0], "t_emit_s", "{pv}");
    assert!(pv["rows"].as_array().unwrap().len() <= 20 && pv["total"].as_u64().unwrap() > 20);

    let rep = http(
        addr,
        "POST",
        "/eloran/rx",
        &json!({"run": run_id}).to_string(),
    );
    assert_eq!(rep.status, 200, "{}", rep.body);
    check_report(&rep.json(), &run_id);

    // Refusals: an unknown run, a malformed id, an unknown field, a run too large to run
    // synchronously, a bad format.
    assert_eq!(
        http(
            addr,
            "POST",
            "/eloran/rx",
            &json!({"run": "b".repeat(64)}).to_string()
        )
        .status,
        404
    );
    assert_eq!(
        http(
            addr,
            "POST",
            "/eloran/rx",
            &json!({"run": "../x"}).to_string()
        )
        .status,
        400
    );
    assert_eq!(
        http(
            addr,
            "POST",
            "/eloran/rx",
            &json!({"run": run_id, "device": "soapy"}).to_string()
        )
        .status,
        422
    );
    let big = RX_SCENARIO.replace("duration_s = 1.2", "duration_s = 60.0");
    let r = http(addr, "POST", "/eloran/run", &big);
    assert_eq!(r.status, 422, "{}", r.body);
    assert!(r.body.contains("--data-dir"), "{}", r.body);
    assert_eq!(
        http(addr, "POST", "/eloran/run?format=wav", RX_SCENARIO).status,
        400
    );
    // A truth-only run has no recording for the receiver.
    let r = http(addr, "POST", "/eloran/run?format=none", RX_SCENARIO).json();
    assert!(r["files"]["samples"].is_null(), "{r}");
    let id = r["run_id"].as_str().unwrap();
    assert_ne!(id, run_id, "the format is part of the run's identity");
    assert_eq!(
        http(addr, "POST", "/eloran/rx", &json!({"run": id}).to_string()).status,
        409
    );
    let _ = std::fs::remove_dir_all(kshana_pro::serve::eloran::sync_runs_dir().join(&run_id));
    let _ = std::fs::remove_dir_all(kshana_pro::serve::eloran::sync_runs_dir().join(id));
}

#[test]
fn job_run_then_receiver_with_truth() {
    let data = fresh_dir("job");
    let srv = start(Some(&data), ent(Tier::Enterprise));
    let addr = srv.local_addr();
    let r = http(addr, "POST", "/eloran/run", RX_SCENARIO);
    assert_eq!(r.status, 202, "{}", r.body);
    let t = r.json();
    assert_eq!(t["mode"], "job");
    let id = t["run_id"].as_str().unwrap().to_string();
    assert_eq!(t["job_id"], id);
    // Not finished yet, or finished: the receiver waits for success either way.
    let t0 = Instant::now();
    let st = loop {
        let s = get(addr, &format!("/jobs/{id}")).json();
        if s["state"] == "succeeded" || s["state"] == "failed" {
            break s;
        }
        assert!(t0.elapsed() < Duration::from_secs(300), "{s}");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(st["state"], "succeeded", "{st}");
    let names: Vec<&str> = st["artefacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["recording.sigmf-meta", "run.json", "truth.csv"]);
    let rec = get(addr, &format!("/jobs/{id}/result")).json();
    assert_eq!(rec["format"], "kshana-eloran-run");
    assert_eq!(rec["mode"], "job");
    assert!(rec["truth_preview"]["rows"].as_array().unwrap().len() > 1);
    let truth = get(addr, &format!("/jobs/{id}/artefacts/truth.csv"));
    assert_eq!(truth.status, 200);
    assert!(truth.body.starts_with("t_emit_s,"));
    let meta = get(addr, &format!("/jobs/{id}/artefacts/recording.sigmf-meta")).json();
    assert_eq!(
        meta["global"]["kshana:eloran"]["scenario_sha256"],
        rec["scenario_sha256"]
    );

    // The same request is the same run.
    assert_eq!(
        http(addr, "POST", "/eloran/run", RX_SCENARIO).json()["run_id"],
        id
    );

    let rep = http(
        addr,
        "POST",
        "/eloran/rx",
        &json!({"run": id, "scenario": RX_SCENARIO}).to_string(),
    );
    assert_eq!(rep.status, 200, "{}", rep.body);
    check_report(&rep.json(), &id);
    // A job of another operation is not a run.
    let other = http(
        addr,
        "POST",
        "/jobs",
        &json!({"op": "run", "request": {"scenario": "kind = \"link-budget\"\n"}}).to_string(),
    )
    .json();
    let oid = other["job_id"].as_str().unwrap();
    assert_eq!(
        http(addr, "POST", "/eloran/rx", &json!({"run": oid}).to_string()).status,
        404
    );
    drop(srv);
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn hardware_output_is_never_served_and_routes_are_documented() {
    for r in kshana_pro::serve::api::ROUTES
        .iter()
        .filter(|r| r.path.starts_with("/eloran"))
    {
        for word in ["stream", "hw", "device", "soapy", "calibrate"] {
            assert!(
                !r.path.contains(word),
                "{} {} exposes hardware",
                r.method,
                r.path
            );
        }
    }
    let srv = start(None, ent(Tier::Enterprise));
    let addr = srv.local_addr();
    assert_eq!(
        http(addr, "POST", "/eloran/stream", RX_SCENARIO).status,
        404
    );
    let doc = get(addr, "/openapi.json").json();
    for (m, p) in [
        ("get", "/eloran/example"),
        ("post", "/eloran/validate"),
        ("post", "/eloran/run"),
        ("post", "/eloran/rx"),
        ("get", "/eloran/gri"),
        ("get", "/eloran/gri/{chain}"),
    ] {
        assert!(
            doc["paths"][p][m].is_object(),
            "{m} {p} is not in the OpenAPI document"
        );
    }
    assert!(doc["x-kshana-operations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["name"] == "eloran-run" && o["feature"] == "pro-eloran"));
}
