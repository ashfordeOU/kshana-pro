// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Maritime AIS evidence (research 2026-11-20 gaps E07, Pro parts) on the SYNTHETIC scenario
//! of `kshana_pro::ais_evidence::synth`: cross-source consistency against a radar, and the
//! sanctions-review, IUU-review and subsea-asset packs, each checked against the planted
//! truth; the CLI refuses without a licence and writes a signed directory that verifies.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::ais_evidence::consistency::{self, Outcome};
use kshana_pro::ais_evidence::packs::{iuu, sanctions, subsea};
use kshana_pro::ais_evidence::{self as ais, synth, Area, Params, Tracks};

fn scenario(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e07-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    synth::write(&d).unwrap();
    d
}

fn req<T: serde::de::DeserializeOwned>(d: &Path, f: &str) -> T {
    toml::from_str(&std::fs::read_to_string(d.join(f)).unwrap()).unwrap()
}

fn run(home: &Path, args: &[&str]) -> (bool, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        o.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

#[test]
fn track_engine_parses_and_refuses_bad_rows() {
    let csv = "timestamp,mmsi,lat,lon,sog_kn,nav_status\n\
               2026-06-01T00:00:00Z,1,35.0,20.0,102.3,15\n\
               2026-06-01T00:10:00Z,1,91,181,5,0\n\
               bad,1,35.0,20.0,5,0\n\
               2026-06-01T00:20:00Z,1,35.01,20.0,5,7\n";
    let t = Tracks::parse(csv).unwrap();
    assert_eq!((t.rows, t.refused), (4, 2));
    let v = &t.by_mmsi["1"];
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].sog_kn, None, "102.3 is 'not available'");
    assert_eq!(v[0].nav_status, None, "15 is 'not defined'");
    assert_eq!(v[1].nav_status, Some(7));
    // Interpolation halfway, and none across a span longer than the limit.
    let (lat, _, _) = t.at("1", v[0].t + 600.0, 1800.0).unwrap();
    assert!((lat - 35.005).abs() < 1e-9);
    assert!(t.at("1", v[0].t + 600.0, 600.0).is_none());
    assert!(Tracks::parse("timestamp,lat,lon\n").is_err());
}

#[test]
fn geometry_contains_and_distances() {
    let a = Area {
        name: "box".into(),
        kind: String::new(),
        polygon: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    };
    assert!(a.contains(0.5, 0.5));
    assert!(!a.contains(1.5, 0.5));
    assert_eq!(a.distance_m(0.5, 0.5), 0.0);
    // 0.1 degree of latitude north of the box is about 11.1 km.
    let d = a.distance_m(1.1, 0.5);
    assert!((d - 11_119.0).abs() < 50.0, "{d}");
    let bad = Area {
        polygon: vec![[0.0, 0.0], [1.0, 0.0]],
        ..a
    };
    assert!(bad.validate().is_err());
}

#[test]
fn cross_source_consistency_finds_offset_phantom_and_dark_target() {
    let d = scenario("consistency");
    let q: consistency::Request = req(&d, "consistency.toml");
    let r = consistency::run(&q, &d).unwrap();
    assert_eq!(r.format, consistency::FORMAT);
    let v5 = r.vessels.iter().find(|v| v.mmsi == "211000005").unwrap();
    assert!(v5.inconsistent >= 10 && v5.consistent >= 50, "{v5:?}");
    assert_eq!(r.episodes.len(), 1);
    let e = &r.episodes[0];
    assert_eq!(e.mmsi, "211000005");
    assert!(
        e.start_utc.starts_with("2026-06-01T04:00") && e.end_utc.starts_with("2026-06-01T06:00")
    );
    assert!((e.max_residual_m - 2000.0).abs() < 5.0);
    // The phantom AIS target inside coverage is unconfirmed; the true vessel 5 is not.
    let v6 = r.vessels.iter().find(|v| v.mmsi == "211000006").unwrap();
    assert!(v6.unconfirmed > 20, "{v6:?}");
    assert_eq!(v5.unconfirmed, 0);
    // R9 has no AIS.
    assert_eq!(r.dark_targets.len(), 1);
    assert_eq!(r.dark_targets[0].track_id, "R9");
    assert!(r
        .associations
        .iter()
        .filter(|a| a.track_id == "R9")
        .all(|a| a.outcome == Outcome::TargetWithoutAis));
    assert!(r.verdict.contains("not a finding of manipulation"));
    // Requests are checked.
    let mut bad = q.clone();
    bad.sources[0].sigma_m = 0.0;
    assert!(consistency::run(&bad, &d).is_err());
    let mut bad = q;
    bad.sources.clear();
    assert!(consistency::run(&bad, &d).is_err());
}

#[test]
fn sanctions_review_finds_gap_encounter_identity_and_collision() {
    let d = scenario("sanctions");
    let q: sanctions::Request = req(&d, "sanctions.toml");
    let p = sanctions::run(&q, &d).unwrap();
    let v1 = p.vessels.iter().find(|v| v.mmsi == "211000001").unwrap();
    assert_eq!(v1.gaps.len(), 1);
    assert!((v1.gaps[0].duration_h - 3.5).abs() < 1e-9);
    assert_eq!(v1.encounters.len(), 1);
    let e = &v1.encounters[0];
    assert_eq!((e.a.as_str(), e.b.as_str()), ("211000001", "211000002"));
    assert!(e.duration_h >= 3.0 && e.min_separation_m < 200.0, "{e:?}");
    assert_eq!(v1.identity_changes.len(), 1);
    assert_eq!(v1.identity_changes[0].field, "name");
    let v7 = p.vessels.iter().find(|v| v.mmsi == "211000007").unwrap();
    assert!(v7.collision.as_ref().unwrap().alternations >= 4);
    assert!(!v7.jumps.is_empty());
    assert_eq!(
        p.shared_imo.get("9000001").unwrap(),
        &vec!["211000001".to_string(), "211000006".to_string()]
    );
    assert!(p.verdict.contains("Not a sanctions determination"));
    // A named vessel with no report is refused.
    let mut bad = q;
    bad.vessels = vec!["999999999".into()];
    assert!(sanctions::run(&bad, &d).is_err());
}

#[test]
fn encounters_need_both_slow_and_sustained() {
    let d = scenario("encounter");
    let t = ais::load_tracks(&d, "positions.csv", None).unwrap();
    let all: BTreeSet<String> = t.by_mmsi.keys().cloned().collect();
    let p = Params::default();
    let e = ais::encounters(&t, &all, &p);
    assert_eq!(e.len(), 1, "{e:?}");
    let longer = Params {
        encounter_min_s: 5.0 * 3600.0,
        ..p.clone()
    };
    assert!(ais::encounters(&t, &all, &longer).is_empty());
    let slower = Params {
        encounter_max_sog_kn: 0.2,
        ..p
    };
    assert!(ais::encounters(&t, &all, &slower).is_empty());
}

#[test]
fn iuu_review_counts_zone_activity() {
    let d = scenario("iuu");
    let q: iuu::Request = req(&d, "iuu.toml");
    let p = iuu::run(&q, &d).unwrap();
    let z = &p.vessels[0].zones[0];
    assert!((z.hours_inside - 4.0).abs() <= 0.34, "{z:?}");
    assert!(
        z.hours_status_fishing > 3.5 && z.hours_in_band > 3.5,
        "{z:?}"
    );
    let codes: Vec<&str> = p.vessels[0].indicators.iter().map(|i| i.code).collect();
    assert!(codes.contains(&"status-fishing-in-zone") && codes.contains(&"fishing-band-in-zone"));
    let mut bad = q.clone();
    bad.fishing_band_kn = [5.0, 1.0];
    assert!(iuu::run(&bad, &d).is_err());
    let mut bad = q;
    bad.zones.clear();
    assert!(iuu::run(&bad, &d).is_err());
}

#[test]
fn subsea_pack_finds_anchor_drag_and_presence_at_fault() {
    let d = scenario("subsea");
    let q: subsea::Request = req(&d, "subsea.toml");
    let p = subsea::run(&q, &d).unwrap();
    let v4 = p.vessels.iter().find(|v| v.mmsi == "211000004").unwrap();
    let c = &v4.corridors[0];
    assert!(c.anchor_status_moving >= 12, "{c:?}");
    assert_eq!(
        c.first_anchor_status_moving_utc.as_deref(),
        Some("2026-06-01T07:00:00Z")
    );
    // Only vessel 4 is flagged; the others never enter the corridor slowly.
    assert_eq!(
        p.vessels
            .iter()
            .filter(|v| !v.indicators.is_empty())
            .count(),
        1
    );
    assert_eq!(p.incident_presence.len(), 1);
    let x = &p.incident_presence[0];
    assert_eq!(x.mmsi, "211000004");
    assert!(x.min_distance_m < 500.0 && x.anchor_status_moving > 0 && !x.gap_over_fault);
    assert!(p.verdict.contains("Presence is not attribution"));
    let mut bad = q;
    bad.assets[0].path.truncate(1);
    assert!(subsea::run(&bad, &d).is_err());
}

#[test]
fn cli_gates_signs_and_verifies() {
    let d = scenario("cli");
    let home = d.join("home");
    let seed = [7u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={hex}\n")).unwrap();
    for (cmd, file) in [
        ("consistency", "consistency.toml"),
        ("sanctions", "sanctions.toml"),
        ("iuu", "iuu.toml"),
        ("subsea", "subsea.toml"),
    ] {
        let reqp = d.join(file);
        let out = d.join(format!("out-{cmd}"));
        let base = [
            "ais",
            cmd,
            reqp.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--signer",
            "test",
            "--key-file",
            key.to_str().unwrap(),
            "--utc",
            "2026-11-20T00:00:00Z",
        ];
        let (ok, msg) = run(&home, &base);
        assert!(!ok, "{cmd} must need a licence: {msg}");
        let mut with_dev = base.to_vec();
        with_dev.push("--dev");
        let (ok, msg) = run(&home, &with_dev);
        assert!(ok, "{cmd}: {msg}");
        let v =
            kshana_pro::signed_dossier::verify(&out, &[kshana_pro::license::public_key_for(&seed)])
                .unwrap();
        assert!(v.passed(), "{cmd}: {:?}", v.problems);
        let html = std::fs::read_to_string(out.join("report.html")).unwrap();
        assert!(html.contains("MODELLED"));
    }
    let (ok, msg) = run(&home, &["ais", "synth", d.join("again").to_str().unwrap()]);
    assert!(ok && d.join("again/positions.csv").exists(), "{msg}");
    let (ok, _) = run(&home, &["ais", "nonsense"]);
    assert!(!ok);
}
