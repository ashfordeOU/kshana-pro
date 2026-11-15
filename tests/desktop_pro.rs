// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Desktop Pro functions (roadmap 0.42 U6, U19, U27, U28; `kshana_pro::desktop`), as the
//! app's bridge calls them:
//!
//! * the incident forensics pack (E1) opens and replays: reported and estimated tracks, the
//!   findings as time-bounded episodes for trust shading, the verdict and the area;
//! * packs verify with the public verifiers and need no licence: the bundle under the
//!   licence-free check, each receiver evidence pack inside it under the **open engine's**
//!   `kshana evidence verify` code, and a Desktop-signed pack under the standalone
//!   `scripts/verify-dossier-signature.py`;
//! * replay and signing are Pro; an Evaluation licence is refused, verification is not;
//! * the service connection keeps the credential out of the page and talks to a real Pro
//!   service (in process): the Station fleet, the wall, whoami.

use std::path::{Path, PathBuf};

use kshana_pro::desktop::{self, Transport};
use kshana_pro::forensics::bundle::{self, BundleRequest};
use kshana_pro::forensics::inputs::{iso, parse_utc};
use kshana_pro::forensics::IncidentScenario;
use kshana_pro::license::{public_key_for, Entitlement, Tier};
use kshana_pro::serve::api::{self, Service};
use kshana_pro::serve::auth::{new_key, Authenticator, KeysFile, Role};
use kshana_pro::serve::worker::JobService;
use kshana_pro::serve::HttpRequest;
use serde_json::{json, Value};

const ISSUER: [u8; 32] = [0x61; 32];
const AT: i64 = 1_800_000_000;

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("desktop-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ent(tier: Tier) -> Entitlement {
    Entitlement {
        licensee: "Desktop test".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

/// An incident: the open-sea jamming training stream as the bridge log, and AIS from its
/// own fixes every 10 s (so the replay has an estimated and a reported track).
fn incident(dir: &Path) -> IncidentScenario {
    let base = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/forensics/training/open-sea-jamming.toml"
    ))
    .unwrap();
    let g = kshana::nmea_synth::generate_from_toml(&base, None).unwrap();
    let nmea = g.nmea_text();
    std::fs::write(dir.join("bridge.nmea"), &nmea).unwrap();
    let tl = kshana::receiver_trust::ingest::read_log(
        kshana::receiver_trust::LogFormat::Nmea,
        nmea.as_bytes(),
    )
    .unwrap();
    let t0 = parse_utc(tl.start_label.as_deref().unwrap()).unwrap();
    let mut ais = vec!["timestamp,vessel_id,lat,lon,sog_kn".to_string()];
    for e in tl.epochs.iter().filter(|e| (e.t_s as i64) % 10 == 0) {
        if let Some(f) = e.fix {
            ais.push(format!(
                "{},SUBJECT-1,{:.5},{:.5},14",
                iso(t0 + e.t_s),
                f.lat_deg,
                f.lon_deg
            ));
        }
    }
    std::fs::write(dir.join("ais.csv"), ais.join("\n") + "\n").unwrap();
    let inc = format!(
        "kind = \"pro-incident-forensics\"\n[incident]\nid = \"DESK-1\"\ntitle = \"Desktop replay test\"\nsubject = \"vessel\"\nsubject_id = \"SUBJECT-1\"\n\
         start_utc = \"{}\"\nend_utc = \"{}\"\n\
         [[receiver]]\nlabel = \"Bridge GNSS\"\npath = \"bridge.nmea\"\nformat = \"nmea\"\nrole = \"subject\"\nplatform = \"vessel\"\n\
         [[ais]]\nlabel = \"AIS feed\"\npath = \"ais.csv\"\n",
        iso(t0 + 200.0),
        iso(t0 + 1200.0)
    );
    std::fs::write(dir.join("incident.toml"), inc).unwrap();
    IncidentScenario::from_file(&dir.join("incident.toml")).unwrap()
}

fn call(name: &str, args: Value, e: &Entitlement, home: &Path) -> Result<Value, String> {
    desktop::call(name, args.as_array().unwrap(), e, Some("tok-1"), AT, home)
        .expect("a Desktop Pro function")
}

#[test]
fn the_forensics_pack_opens_verifies_free_and_replays_under_pro() {
    let work = tmp("e1");
    let home = work.join("home");
    let scn = incident(&work);
    let out = work.join("bundle");
    bundle::build_bundle(&BundleRequest {
        scenario: &scn,
        out: &out,
        utc: "2026-11-15T12:00:00Z",
        seed: &ISSUER,
        signer: "Ashforde OÜ (test issuer)",
    })
    .unwrap();
    let issuer = hex::encode(public_key_for(&ISSUER));
    let d = out.display().to_string();

    // Free: verified, trusted with the pinned issuer, under an Evaluation licence.
    let v = call(
        "pro_pack_verify",
        json!([d, issuer]),
        &Entitlement::evaluation(),
        &home,
    )
    .unwrap();
    assert_eq!(
        (
            v["kind"].as_str(),
            v["ok"].as_bool(),
            v["trusted"].as_bool()
        ),
        (Some("incident-forensics"), Some(true), Some(true)),
        "{v}"
    );
    assert!(v["evidence_packs_ok"].as_u64().unwrap() >= 1);

    // Each receiver evidence pack verifies under the open engine's own verifier.
    let mut packs = 0;
    for e in std::fs::read_dir(out.join("evidence")).unwrap().flatten() {
        let files = kshana::evidence::cli::read_dir(&e.path()).unwrap();
        let r = kshana::evidence::verify::verify_bundle(
            &files,
            &kshana::evidence::verify::VerifyOptions {
                expected_public_key: Some(public_key_for(&ISSUER)),
                full_log: None,
                require_timestamp: false,
                tsa_anchors: None,
            },
        );
        assert!(
            r.ok && r.signer_pinned,
            "{}: {:?}",
            e.path().display(),
            r.failures
        );
        packs += 1;
    }
    assert!(packs >= 1);

    // Replay is Pro.
    let e = call(
        "pro_incident_replay",
        json!([d]),
        &Entitlement::evaluation(),
        &home,
    )
    .unwrap_err();
    assert!(e.to_lowercase().contains("pro"), "{e}");
    let r = call("pro_incident_replay", json!([d]), &ent(Tier::Pro), &home).unwrap();
    assert_eq!(r["format"], "kshana-pro-incident-replay/1");
    assert_eq!(r["verification"]["ok"], true);
    let kinds: Vec<&str> = r["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["kind"].as_str())
        .collect();
    assert!(
        kinds.contains(&"estimated") && kinds.contains(&"reported"),
        "{kinds:?}"
    );
    for t in r["tracks"].as_array().unwrap() {
        let pts = t["points"].as_array().unwrap();
        assert!(pts.len() > 10);
        assert!(pts
            .windows(2)
            .all(|w| w[0]["t"].as_f64() <= w[1]["t"].as_f64()));
    }
    let jam = r["episodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["category"] == "jamming")
        .expect("a jamming episode");
    let (s, en) = (jam["start"].as_f64().unwrap(), jam["end"].as_f64().unwrap());
    let (w0, w1) = (
        r["window"]["start"].as_f64().unwrap(),
        r["window"]["end"].as_f64().unwrap(),
    );
    assert!(
        w0 <= s && s < en && en <= w1 + 1.0,
        "{s} {en} in {w0}..{w1}"
    );
    assert_eq!(r["verdict"]["jamming"]["status"], "indicated");
    assert!(
        r["limits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap_or("").contains("V2")),
        "the V2 limit stays on every report"
    );

    // A changed file: still replayed, but marked as not verified, with the reason.
    let p = out.join("timeline.csv");
    let mut t = std::fs::read_to_string(&p).unwrap();
    t.push_str("tampered\n");
    std::fs::write(&p, t).unwrap();
    let r = call("pro_incident_replay", json!([d]), &ent(Tier::Pro), &home).unwrap();
    assert_eq!(r["verification"]["ok"], false);
    assert!(!r["verification"]["problems"].as_array().unwrap().is_empty());
    // Not a bundle: a clear refusal.
    assert!(call(
        "pro_incident_replay",
        json!([work.display().to_string()]),
        &ent(Tier::Pro),
        &home
    )
    .is_err());
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn signed_packs_verify_with_the_public_verifier_and_signing_is_pro() {
    let work = tmp("sign");
    let home = work.join("home");
    let pack = work.join("review-pack");
    let files = json!({"review.json": "{\"decision\": \"accepted\"}\n", "report/report.html": "<!doctype html><title>r</title>\n"});
    let e = call(
        "pro_sign_files",
        json!([pack.display().to_string(), files, "Analyst"]),
        &Entitlement::evaluation(),
        &home,
    )
    .unwrap_err();
    assert!(e.to_lowercase().contains("pro"), "{e}");
    for bad in [
        "../x.txt",
        "/etc/x",
        "a/../b",
        "signature.json",
        "pack.json",
        "c:x",
        "",
    ] {
        let r = call(
            "pro_sign_files",
            json!([pack.display().to_string(), {bad: "x"}, "Analyst"]),
            &ent(Tier::Pro),
            &home,
        );
        assert!(r.is_err(), "{bad}");
    }
    let s = call(
        "pro_sign_files",
        json!([pack.display().to_string(), files, "Analyst"]),
        &ent(Tier::Pro),
        &home,
    )
    .unwrap();
    assert_eq!(s["verification"]["ok"], true);
    assert_eq!(s["verification"]["trusted"], true);
    assert_eq!(s["verification"]["pack"]["licensee"], "Desktop test");
    assert_eq!(s["verification"]["pack"]["licence_token"], "tok-1");
    let key = s["public_key"].as_str().unwrap().to_string();
    // The same device key every time; the page can show it for a recipient to pin.
    let k = call(
        "pro_evidence_key",
        json!([]),
        &Entitlement::evaluation(),
        &home,
    )
    .unwrap();
    assert_eq!(k["public_key"], key.as_str());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let m = std::fs::metadata(home.join(desktop::KEY_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(m & 0o077, 0, "the evidence key is owner-only");
    }
    // Free verification in the app, with the key pinned.
    let v = call(
        "pro_pack_verify",
        json!([pack.display().to_string(), key]),
        &Entitlement::evaluation(),
        &home,
    )
    .unwrap();
    assert_eq!(
        (
            v["kind"].as_str(),
            v["ok"].as_bool(),
            v["trusted"].as_bool()
        ),
        (Some("signed-pack"), Some(true), Some(true))
    );
    // The standalone public verifier (Python, no Kshana).
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scripts/verify-dossier-signature.py"
    );
    let py = std::process::Command::new("python3")
        .args(["-I", script])
        .arg(&pack)
        .args(["--trust", &key])
        .output();
    let python_ok = py
        .as_ref()
        .is_ok_and(|o| !String::from_utf8_lossy(&o.stderr).contains("No module named"));
    if python_ok {
        let o = py.unwrap();
        assert!(
            o.status.success(),
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        std::fs::write(pack.join("review.json"), "{\"decision\": \"rejected\"}\n").unwrap();
        let o = std::process::Command::new("python3")
            .args(["-I", script])
            .arg(&pack)
            .args(["--trust", &key])
            .output()
            .unwrap();
        assert!(
            !o.status.success(),
            "a changed review must fail the public verifier"
        );
    } else {
        eprintln!("python3 with the cryptography package not found: standalone check skipped");
    }
    let v = call(
        "pro_pack_verify",
        json!([pack.display().to_string(), key]),
        &Entitlement::evaluation(),
        &home,
    )
    .unwrap();
    if python_ok {
        assert_eq!(v["ok"], false);
    }
    let _ = std::fs::remove_dir_all(&work);
}

/// The Pro service in process: the transport hands each request to the router.
struct InProcess(Service);
impl Transport for InProcess {
    fn send(
        &self,
        method: &str,
        url: &str,
        credential: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), String> {
        let path = url
            .strip_prefix("http://localhost:7300")
            .ok_or("unexpected base")?;
        let r = api::dispatch(
            &self.0,
            &HttpRequest {
                method: method.into(),
                path: path.into(),
                body: body.unwrap_or("").into(),
            },
            Some(&format!("Bearer {credential}")),
            AT,
        );
        Ok((r.status, r.body))
    }
}

#[test]
fn the_service_connection_keeps_the_credential_and_reads_the_fleet() {
    let work = tmp("svc");
    let home = work.join("home");
    let data = work.join("data");
    let (key, e) = new_key("desk", "site-a", Role::Submitter);
    let ent_e = ent(Tier::Enterprise);
    let mut svc = Service::with_auth(
        ent_e.clone(),
        Some(std::sync::Arc::new(
            JobService::open(data.clone(), ent_e.clone()).unwrap(),
        )),
        Authenticator::new(None, Some(&KeysFile { keys: vec![e] })),
        None,
    );
    svc.stations_claim = 5;
    let t = InProcess(svc);
    let pro = ent(Tier::Pro);
    let go = |name: &str, args: Value| {
        desktop::call_with(name, args.as_array().unwrap(), &pro, None, AT, &home, &t).unwrap()
    };

    assert_eq!(
        go("pro_service_status", json!([])).unwrap()["connected"],
        false
    );
    assert!(go("pro_service", json!(["GET", "/whoami"])).is_err());
    assert!(
        go(
            "pro_service_connect",
            json!(["http://kshana.example.org", key])
        )
        .is_err(),
        "plain http to another host"
    );
    let c = go(
        "pro_service_connect",
        json!(["http://localhost:7300/", key]),
    )
    .unwrap();
    assert_eq!(
        (c["base"].as_str(), c["credential"].as_str()),
        (Some("http://localhost:7300"), Some("api-key"))
    );
    assert!(
        !c.to_string().contains(&key),
        "the credential never reaches the page"
    );
    assert!(!go("pro_service_status", json!([]))
        .unwrap()
        .to_string()
        .contains(&key));

    let me = go("pro_service", json!(["GET", "/whoami"])).unwrap();
    assert_eq!(
        (me["status"].as_u64(), me["json"]["tenant"].as_str()),
        (Some(200), Some("site-a"))
    );
    for (id, name) in [("tallinn-1", "Tallinn port"), ("muuga-2", "Muuga")] {
        let r = go(
            "pro_service",
            json!([
                "POST",
                "/fleet/stations",
                json!({"id": id, "name": name}).to_string()
            ]),
        )
        .unwrap();
        assert_eq!(r["status"], 201, "{r}");
    }
    let r = go(
        "pro_service",
        json!([
            "POST",
            "/fleet/stations/tallinn-1/heartbeat",
            "{\"trust\": 0.92, \"lat\": 59.44, \"lon\": 24.77}"
        ]),
    )
    .unwrap();
    assert_eq!(r["status"], 200);
    let f = go("pro_service", json!(["GET", "/fleet/stations"])).unwrap();
    assert_eq!(f["json"]["stations"].as_array().unwrap().len(), 2);
    assert_eq!(f["json"]["limit"], 5);
    let w = go("pro_service", json!(["GET", "/wall/state"])).unwrap();
    assert_eq!(w["json"]["fleet"]["counts"]["live"], 1);
    for bad in ["/../etc", "fleet", "/a b", "//x"] {
        assert!(go("pro_service", json!(["GET", bad])).is_err(), "{bad}");
    }
    assert!(go("pro_service", json!(["TRACE", "/whoami"])).is_err());
    assert_eq!(
        go("pro_service_disconnect", json!([])).unwrap()["disconnected"],
        true
    );
    assert_eq!(
        go("pro_service_status", json!([])).unwrap()["connected"],
        false
    );
    // Unknown names are left to the bridge.
    assert!(desktop::call("run", &[], &pro, None, AT, &home).is_none());
    let _ = std::fs::remove_dir_all(&work);
}
