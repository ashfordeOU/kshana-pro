// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Kshana Station fleet (roadmap L3 Pro part, F10e): one Station without the Pro fleet
//! feature; with it, as many as the licence's `stations` claim; retiring frees a place;
//! heartbeats keep a Station live.

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::Path;

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::serve::{self, fleet, ServeConfig, ServerHandle};
use serde_json::{json, Value};

fn start(tag: &str, tier: Tier, stations: u32) -> ServerHandle {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("station_fleet-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let mut cfg = ServeConfig::new(
        "127.0.0.1:0",
        Entitlement {
            licensee: "site".into(),
            tier,
            expires_unix: 0,
            features: vec![],
        },
    );
    cfg.data_dir = Some(d);
    cfg.stations_claim = stations;
    serve::start(cfg).unwrap()
}

fn http(a: SocketAddr, method: &str, path: &str, body: &str) -> (u16, Value) {
    let mut s = std::net::TcpStream::connect(a).unwrap();
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let st = out[9..12].parse().unwrap();
    (
        st,
        serde_json::from_str(out.split_once("\r\n\r\n").unwrap().1).unwrap_or(Value::Null),
    )
}

fn reg(a: SocketAddr, id: &str) -> (u16, Value) {
    http(
        a,
        "POST",
        "/fleet/stations",
        &json!({"id": id, "name": format!("Station {id}"), "url": "https://10.0.0.5:8443"})
            .to_string(),
    )
}

#[test]
fn one_station_without_the_fleet_feature() {
    let srv = start("open", Tier::Evaluation, 5);
    let a = srv.local_addr();
    assert_eq!(reg(a, "harbour-1").0, 201);
    let (st, v) = reg(a, "harbour-2");
    assert_eq!(st, 403);
    assert!(
        v["error"].as_str().unwrap().contains("Pro Station fleet"),
        "{v}"
    );
    // Re-registering the one Station is fine.
    assert_eq!(reg(a, "harbour-1").0, 200);
    srv.stop();
}

#[test]
fn the_licence_caps_the_fleet_and_retiring_frees_a_place() {
    let srv = start("pro", Tier::Pro, 3);
    let a = srv.local_addr();
    for id in ["s1", "s2", "s3"] {
        assert_eq!(reg(a, id).0, 201, "{id}");
    }
    let (st, v) = reg(a, "s4");
    assert_eq!(st, 403);
    assert!(v["error"].as_str().unwrap().contains("allows 3"), "{v}");
    assert_eq!(http(a, "POST", "/fleet/stations/s2/retire", "").0, 200);
    assert_eq!(reg(a, "s4").0, 201);
    // A retired Station coming back is past the limit again.
    assert_eq!(reg(a, "s2").0, 403);
    // Heartbeats.
    let (st, v) = http(
        a,
        "POST",
        "/fleet/stations/s1/heartbeat",
        &json!({"receivers": 2, "obs_per_s": 10, "disk_free_gb": 40}).to_string(),
    );
    assert_eq!(st, 200);
    assert_eq!(v["state"], "live");
    assert_eq!(
        http(a, "POST", "/fleet/stations/nope/heartbeat", "{}").0,
        404
    );
    assert_eq!(
        http(a, "POST", "/fleet/stations/s1/heartbeat", "[1]").0,
        400
    );
    let (_, list) = http(a, "GET", "/fleet/stations", "");
    assert_eq!(list["limit"], 3);
    assert_eq!(list["active"], 3);
    let s1 = list["stations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "s1")
        .unwrap();
    assert_eq!(s1["state"], "live");
    assert_eq!(s1["health"]["receivers"], 2);
    let s3 = list["stations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "s3")
        .unwrap();
    assert_eq!(s3["state"], "registered");
    // Bad ids are refused.
    assert_eq!(
        http(
            a,
            "POST",
            "/fleet/stations",
            &json!({"id": "../x"}).to_string()
        )
        .0,
        422
    );
    srv.stop();
}

#[test]
fn a_station_goes_stale_without_heartbeats() {
    let s = fleet::Station {
        id: "x".into(),
        name: "x".into(),
        url: None,
        tenant: "t".into(),
        registered_unix: 0,
        last_seen_unix: Some(1_000),
        health: Value::Null,
        retired: false,
    };
    assert_eq!(fleet::state(&s, 1_000 + fleet::STALE_AFTER_SECS), "live");
    assert_eq!(fleet::state(&s, 1_001 + fleet::STALE_AFTER_SECS), "stale");
    let ent = Entitlement::evaluation();
    assert_eq!(fleet::limit_for(&ent, 10, 0), 1);
    let pro = Entitlement {
        licensee: "p".into(),
        tier: Tier::Pro,
        expires_unix: 0,
        features: vec![],
    };
    assert_eq!(fleet::limit_for(&pro, 0, 0), 1);
    assert_eq!(fleet::limit_for(&pro, 12, 0), 12);
}
