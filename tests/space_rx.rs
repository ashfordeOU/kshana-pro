// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Space-segment GNSS receiver evidence (`kshana_pro::space_rx`, epic E20): the
//! ground-interference envelope from orbit, campaign scorecards, the selection database and
//! template, radiation-fault signatures with injected fixtures, OBC interface vectors, the SEE
//! rate forecast and the CLI. Evidence: MODELLED (SYNTHETIC samples and fixtures).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::license::{self, Tier};
use kshana_pro::space_rx::{self, exposure, obc, qual, see, see_rate};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-space-rx-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const SEED: [u8; 32] = [23u8; 32];
const UTC: &str = "2026-11-20T09:00:00Z";

fn sample_exposure(extra: &str) -> exposure::Scenario {
    exposure::Scenario::parse(&format!("{}{extra}", exposure::SAMPLE)).unwrap()
}

#[test]
fn exposure_envelope_over_areas_reproduces_the_deaf_zone_pattern() {
    // A third area far from any reports, small, in the South Pacific.
    let s = sample_exposure(
        "\n[[area]]\nname = \"south-pacific\"\nlat_deg = -50\nlon_deg = -140\nradius_km = 50\neirp_dbw = [20.0]\n",
    );
    let doc = exposure::run(&s, Path::new(".")).unwrap();
    assert_eq!(doc["schema"], exposure::SCHEMA);
    assert_eq!(doc["evidence"], "MODELLED");
    let period = doc["orbit"]["period_s"].as_f64().unwrap();
    assert!((5600.0..5800.0).contains(&period), "{period}");
    let alt = doc["orbit"]["altitude_km"]["p50"].as_f64().unwrap();
    assert!((490.0..540.0).contains(&alt), "{alt}");
    let passes = doc["passes"].as_array().unwrap();
    for a in ["eastern-mediterranean", "baltic"] {
        let n = passes.iter().filter(|p| p["area"] == a).count();
        assert!((2..=12).contains(&n), "{a}: {n} passes in a day");
    }
    let f = doc["in_view_fraction"].as_f64().unwrap();
    assert!(f > 0.02 && f < 0.6, "{f}");
    // J/S rises 20 dB per 20 dB of EIRP; the strongest assumed EIRP drops tracking near
    // closest approach, the weakest never does (the published pattern: loss over the
    // interference regions only, and only for strong emitters).
    let env = doc["envelope"]["per_eirp"].as_array().unwrap();
    let max = |i: usize| env[i]["js_db"]["max"].as_f64().unwrap();
    assert!((max(1) - max(0) - 20.0).abs() < 0.1 && (max(2) - max(1) - 20.0).abs() < 0.1);
    assert_eq!(env[0]["time_below_threshold_s"].as_f64().unwrap(), 0.0);
    assert!(env[2]["time_below_threshold_s"].as_f64().unwrap() > 0.0);
    assert!(env[2]["time_above_tested_js_s"].as_f64().unwrap() > 0.0);
    // Below-threshold time sits inside the passes over the areas.
    for p in passes {
        let dwell = p["dwell_s"].as_f64().unwrap();
        for e in p["per_eirp"].as_array().unwrap() {
            assert!(e["below_threshold_s"].as_f64().unwrap() <= dwell);
        }
    }
    // Doppler offsets of a ground transmitter seen from LEO: tens of kHz at L1, bounded by
    // the orbital speed.
    let dmax = doc["envelope"]["abs_doppler_offset_hz"]["max"]
        .as_f64()
        .unwrap();
    assert!(dmax > 15_000.0 && dmax < 41_000.0, "{dmax}");
    let tp = doc["test_points"].as_array().unwrap();
    assert_eq!(tp.len(), 3);
    assert_eq!(tp[2]["beyond_tested"], true);
    assert_eq!(tp[0]["beyond_tested"], false);
    let csv = exposure::passes_csv(&doc);
    assert!(csv.lines().count() > passes.len());
    assert!(exposure::summary_md(&doc).contains("MODELLED"));
}

#[test]
fn exposure_nadir_antenna_sees_more_interference_and_maps_give_areas() {
    let z = exposure::run(&sample_exposure(""), Path::new(".")).unwrap();
    let mut n = sample_exposure("");
    n.receiver.boresight = "nadir".into();
    let n = exposure::run(&n, Path::new(".")).unwrap();
    let m = |d: &serde_json::Value| {
        d["envelope"]["per_eirp"][1]["js_db"]["max"]
            .as_f64()
            .unwrap()
    };
    assert!(m(&n) > m(&z) + 20.0, "{} vs {}", m(&n), m(&z));
    assert_eq!(exposure::gain_at(&[], 0.0), 5.0);
    assert_eq!(exposure::gain_at(&[], 180.0), -25.0);
    assert!((exposure::gain_at(&[], 45.0) - 2.5).abs() < 1e-9);
    // An open-engine map: one degraded cell, one not degraded.
    let d = tmp("map");
    std::fs::write(
        d.join("map.geojson"),
        r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"status":"degraded"},"geometry":{"type":"Polygon","coordinates":[[[33,34],[34,34],[34,35],[33,35],[33,34]]]}},
          {"type":"Feature","properties":{"status":"not_degraded"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#,
    )
    .unwrap();
    let mut s = sample_exposure("");
    s.area.clear();
    s.map = vec!["map.geojson".into()];
    let a = exposure::areas(&s, &d).unwrap();
    assert_eq!(a.len(), 1);
    assert!((a[0].lat_deg - 34.5).abs() < 1e-9 && a[0].radius_km > 50.0 && a[0].radius_km < 90.0);
    let doc = exposure::run(&s, &d).unwrap();
    assert!(!doc["passes"].as_array().unwrap().is_empty());
    // Refusals.
    let mut bad = sample_exposure("");
    bad.receiver.boresight = "sideways".into();
    assert!(exposure::Scenario::parse(&toml::to_string(&bad).unwrap()).is_err());
    assert!(
        exposure::Scenario::parse(&exposure::SAMPLE.replace("step_s = 10", "step_s = 0")).is_err()
    );
}

fn sample_score() -> (qual::Campaign, qual::Scorecard) {
    let c = qual::Campaign::parse(qual::SAMPLE_CAMPAIGN).unwrap();
    let m = qual::Mission::parse(qual::SAMPLE_MISSION).unwrap();
    let s = qual::score(&c, &m);
    (c, s)
}

#[test]
fn campaign_scorecard_against_the_mission_profile() {
    let (c, s) = sample_score();
    let v = |id: &str| {
        s.checks
            .iter()
            .find(|x| x.id == id)
            .unwrap_or_else(|| panic!("{id}"))
            .verdict
            .clone()
    };
    assert_eq!(v("tid.functional"), "met");
    assert_eq!(v("tid.position_error"), "met");
    assert_eq!(v("tid.ttff"), "met");
    assert_eq!(v("tid.current"), "met");
    assert_eq!(v("see.latchup"), "met");
    assert_eq!(v("see.sefi"), "met");
    assert_eq!(v("vibration"), "not_met", "Z axis lost tracking");
    assert_eq!(v("thermal.range"), "met");
    assert_eq!(v("thermal.cycles"), "met");
    assert_eq!(v("thermal.vacuum"), "met");
    assert_eq!(v("tracking.leo-550km"), "met");
    assert_eq!(v("interference.js"), "met");
    assert_eq!(s.not_met, 1);
    assert_eq!(s.met + s.not_met + s.not_assessed, s.checks.len());
    assert_eq!(s.specs.tid_functional_krad, Some(30.0));
    assert_eq!(s.specs.latchup_let_observed, Some(62.0));
    assert_eq!(s.specs.js_held_db, Some(30.0));
    assert!(s.evidence.contains("SYNTHETIC"));
    // A stricter profile: dose beyond what was tested is not assessed; a latch-up threshold
    // above the observed LET is not met.
    let mut m = qual::Mission::parse(qual::SAMPLE_MISSION).unwrap();
    m.tid_krad = Some(15.0);
    m.rdm = 3.0;
    m.latchup_let_min = Some(80.0);
    let s2 = qual::score(&c, &m);
    let v2 = |id: &str| {
        s2.checks
            .iter()
            .find(|x| x.id == id)
            .unwrap()
            .verdict
            .clone()
    };
    assert_eq!(v2("tid.functional"), "not_met", "failed at 40 <= 45");
    assert_eq!(v2("see.latchup"), "not_met");
    // Needed 36 krad: functional to 30, failed only at 40, so the gap is not assessed.
    m.tid_krad = Some(12.0);
    let s3 = qual::score(&c, &m);
    assert_eq!(
        s3.checks
            .iter()
            .find(|x| x.id == "tid.functional")
            .unwrap()
            .verdict,
        "not_assessed"
    );
    // An empty profile assesses nothing.
    let empty =
        qual::Mission::parse("format = \"kshana-space-rx-mission/1\"\nname = \"none\"\n").unwrap();
    let s4 = qual::score(&c, &empty);
    assert_eq!(s4.met + s4.not_met, 0);
    // Claim words are refused in inputs.
    let bad = qual::SAMPLE_CAMPAIGN.replace("example lab", concat!("a certi", "fied lab"));
    assert!(qual::Campaign::parse(&bad).is_err());
    assert!(qual::Campaign::parse(&qual::SAMPLE_CAMPAIGN.replace("synthetic", "made-up")).is_err());
    assert!(qual::scorecard_md(&s).contains("| vibration | not_met |"));
}

#[test]
fn template_is_populated_from_one_campaign_and_the_database_schema_holds() {
    let (c, s) = sample_score();
    let t = qual::template_md(&c, &s);
    for want in [
        "3 Radiation: TID",
        "5 steps, Co-60",
        "tid.functional met",
        "vibration not_met",
        "ECSS-Q-ST-60-15C",
        "9 EMC | Conducted and radiated emissions and susceptibility | ECSS-E-ST-20-07C | OPEN",
        "Open phases: 2.",
    ] {
        assert!(t.contains(want), "{want}\n{t}");
    }
    let mut db = qual::Db::new();
    db.add(&s);
    db.add(&s);
    assert_eq!(db.entries.len(), 1);
    assert_eq!(db.entries[0].scorecards.len(), 1);
    let mut m2 = qual::Mission::parse(qual::SAMPLE_MISSION).unwrap();
    m2.name = "second profile".into();
    db.add(&qual::score(&c, &m2));
    assert_eq!(db.entries[0].scorecards.len(), 2);
    let back = qual::Db::parse(&serde_json::to_string(&db).unwrap()).unwrap();
    assert_eq!(back, db);
    assert_eq!(db.select(Some(20.0), Some(25.0)).len(), 1);
    assert_eq!(db.select(Some(50.0), None).len(), 0);
    assert_eq!(db.select(None, Some(35.0)).len(), 0);
    // The published schema names every field the database writes.
    let schema = qual::db_schema();
    assert_eq!(schema["$id"], qual::DB_FORMAT);
    let entry = serde_json::to_value(&db.entries[0]).unwrap();
    let props = &schema["properties"]["entries"]["items"]["properties"];
    for k in entry.as_object().unwrap().keys() {
        assert!(props.get(k).is_some(), "schema lacks {k}");
    }
    for k in entry["specs"].as_object().unwrap().keys() {
        assert!(
            props["specs"]["properties"].get(k).is_some(),
            "schema lacks specs.{k}"
        );
    }
}

fn clean() -> Vec<see::Epoch> {
    see::synth_trace(600, 1.0, 550.0, 97.6, 7)
}

#[test]
fn clean_trace_has_no_events_and_round_trips() {
    let t = clean();
    let back = see::parse_trace(&see::write_trace(&t)).unwrap();
    assert_eq!(back.len(), t.len());
    let ev = see::detect(&back, &see::Params::default());
    assert!(ev.is_empty(), "{ev:?}");
    // A week rollover in the time tags is not a fault.
    let mut w = back.clone();
    for e in w.iter_mut() {
        e.tow_s = (e.tow_s + 604_800.0 - 345_600.0 - 300.0).rem_euclid(604_800.0);
    }
    assert!(see::detect(&w, &see::Params::default()).is_empty());
}

#[test]
fn injected_signatures_are_all_detected() {
    let t = see::parse_trace(&see::write_trace(&clean())).unwrap();
    let f = see::InjectFile::parse(see::SAMPLE_INJECT).unwrap();
    let (fx, truth) = see::inject(&t, &f).unwrap();
    let fx = see::parse_trace(&see::write_trace(&fx)).unwrap();
    let ev = see::detect(&fx, &see::Params::default());
    let (found, missed, unexpected) = see::match_truth(&truth, &ev);
    assert!(missed.is_empty(), "missed {missed:?}; events {ev:?}");
    assert!(unexpected.is_empty(), "unexpected {unexpected:?}");
    assert_eq!(found, truth["truth"].as_array().unwrap().len());
    let flip = ev.iter().find(|e| e.kind == "output_upset").unwrap();
    assert!(flip.detail.contains("flip of bit 16"), "{}", flip.detail);
    assert!(ev
        .iter()
        .any(|e| e.kind == "latchup_suspect" && e.detail.contains("followed by a reset")));
    let doc = see::result(&fx, &see::Params::default(), &ev, "x");
    // The latch-up suspect plus the two resets (class SEFI/SEL).
    assert_eq!(doc["counts"]["SEL"].as_u64().unwrap(), 3);
    assert_eq!(see::bitflip_bit(655.36, 0.01, 0.5), Some(16));
    assert_eq!(see::bitflip_bit(-655.36, 0.01, 0.5), Some(16));
    assert_eq!(see::bitflip_bit(500.0, 0.01, 5.0), None);
}

#[test]
fn normal_behaviour_is_not_called_radiation() {
    let p = see::Params::default();
    // A one-millisecond clock steering step that stays.
    let mut t = clean();
    for e in t.iter_mut().skip(200) {
        e.clock_bias_m += see::MS_M;
    }
    let ev = see::detect(&t, &p);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(
        (ev[0].kind.as_str(), ev[0].class.as_str()),
        ("clock_ms_jump", "other")
    );
    // A solution step that persists (for example a manoeuvre the trace does not model).
    let mut t = clean();
    for e in t.iter_mut().skip(300) {
        e.r[2] += 400.0;
    }
    let ev = see::detect(&t, &p);
    assert!(ev.iter().all(|e| e.class == "other"), "{ev:?}");
    assert!(ev.iter().any(|e| e.kind == "solution_step"));
    // A time-tag step that persists.
    let mut t = clean();
    for e in t.iter_mut().skip(100) {
        e.tow_s += 3.0;
    }
    let ev = see::detect(&t, &p);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0].kind, "timetag_step");
    // Loss of fix without a reset.
    let mut t = clean();
    for e in t.iter_mut().skip(400).take(5) {
        e.fix = false;
    }
    let ev = see::detect(&t, &p);
    assert!(
        ev.iter()
            .any(|e| e.kind == "loss_of_fix" && e.class == "other"),
        "{ev:?}"
    );
    assert!(ev.iter().all(|e| e.class == "other"), "{ev:?}");
}

#[test]
fn obc_vectors_reference_parser_and_scoring() {
    let v = obc::build(&clean()).unwrap();
    let classes: std::collections::BTreeSet<&str> = v.iter().map(|x| x.class.as_str()).collect();
    assert_eq!(classes.len(), 16, "{classes:?}");
    let limits = obc::Limits::default();
    let refs = obc::reference_actions(&v, &limits);
    for x in &v {
        assert!(
            x.allowed.contains(&refs[&x.id]),
            "{} {}: reference {}",
            x.id,
            String::from_utf8_lossy(&hex::decode(&x.hex).unwrap()),
            refs[&x.id]
        );
    }
    // Nominal sentences are within the length limit with a valid checksum.
    for x in v.iter().filter(|x| x.class == "nominal") {
        let b = hex::decode(&x.hex).unwrap();
        assert!(b.len() <= obc::MAX_LEN);
    }
    let stream = obc::stream_bytes(&v);
    assert!(stream.ends_with(b"\r\n"));
    // The reference log scores all met; an OBC that accepts everything does not.
    let sc = obc::score(&v, &refs);
    assert_eq!(sc["not_met"], 0);
    assert_eq!(sc["not_assessed"], 0);
    let naive: std::collections::BTreeMap<String, String> = v
        .iter()
        .map(|x| (x.id.clone(), "accept".to_string()))
        .collect();
    let sc = obc::score(&v, &naive);
    assert_eq!(
        sc["not_met"].as_u64().unwrap() as usize,
        v.iter()
            .filter(|x| !x.allowed.contains(&"accept".to_string()))
            .count()
    );
    let mut partial = refs.clone();
    partial.retain(|k, _| !k.contains("garbage"));
    let sc = obc::score(&v, &partial);
    assert_eq!(sc["not_assessed"], 1);
    assert!(obc::parse_log("id,action\nv0001-nominal,accept\n").is_ok());
    assert!(obc::parse_log("v0001-nominal,maybe\n").is_err());
    assert!(obc::build(&clean()[..20]).is_err());
}

#[test]
fn see_rate_forecast() {
    let m = see_rate::Model::parse(see_rate::SAMPLE).unwrap();
    let doc = see_rate::run(&m);
    let rates = doc["rates"].as_array().unwrap();
    assert_eq!(rates.len(), 6);
    let r = |env: &str, eff: &str| {
        rates
            .iter()
            .find(|x| x["environment"] == env && x["effect"] == eff)
            .unwrap()["per_day"]
            .as_f64()
            .unwrap()
    };
    for eff in ["latchup", "sefi", "upset"] {
        assert!(r("solar minimum", eff) > 0.0);
        assert!(r("solar minimum", eff) > r("solar maximum", eff), "{eff}");
    }
    assert!(r("solar minimum", "upset") > r("solar minimum", "latchup"));
    // A saturated step cross-section folds to sigma_sat times the flux above the onset.
    let w = see_rate::Weibull {
        sigma_sat_cm2: 1e-3,
        onset: 0.0,
        width: 1e-6,
        shape: 1.0,
    };
    let f = see_rate::fold(&w, &[[1.0, 10.0], [10.0, 1.0], [100.0, 0.0]]);
    assert!((f - 1e-3 * 10.0).abs() < 1e-9, "{f}");
    assert!(see_rate::markdown(&doc).contains("mean days between"));
    assert!(see_rate::Model::parse(
        &see_rate::SAMPLE.replace("[0.1, 5.0], [1.0, 2e-2]", "[0.1, 5.0], [1.0, 9.0]")
    )
    .is_err());
}

#[test]
fn licence_features_and_tiers() {
    for f in [
        space_rx::FEATURE_EXPOSURE,
        space_rx::FEATURE_QUAL,
        space_rx::FEATURE_SEE,
    ] {
        assert_eq!(license::explicit_tier(f), Some(Tier::Pro), "{f}");
        assert!(license::PRO_COMMAND_FEATURES.contains(&f));
    }
}

/// The wording rule: no claim words in the module, the CLI or the docs page.
#[test]
fn wording_rule_holds() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("src/space_rx"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.push(root.join("src/cli/space_rx_cmd.rs"));
    files.push(root.join("docs/SPACE-RECEIVER.md"));
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap().to_lowercase();
        for w in qual::claim_words() {
            assert!(!text.contains(&w), "{}: contains '{w}'", f.display());
        }
        for w in [concat!("compl", "iance"), concat!("con", "formance")] {
            assert!(!text.contains(w), "{}: contains '{w}'", f.display());
        }
    }
}

fn bin(args: &[&str], home: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn cli_end_to_end() {
    let d = tmp("cli");
    let home = d.join("home");
    let p = |x: &Path| x.to_str().unwrap().to_string();
    for s in ["campaign", "mission", "exposure", "trace", "inject", "rate"] {
        let (ok, out) = bin(
            &[
                "space-rx",
                "sample",
                s,
                "--out",
                &p(&d.join(format!("{s}.in"))),
            ],
            &home,
        );
        assert!(ok, "{s}: {out}");
    }
    // Gated without a licence.
    let (ok, out) = bin(
        &[
            "space-rx",
            "exposure",
            &p(&d.join("exposure.in")),
            "--out",
            &p(&d.join("x")),
        ],
        &home,
    );
    assert!(!ok && out.contains("requires the 'pro' tier"), "{out}");
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={}\n", hex::encode(SEED))).unwrap();
    let sig = [
        "--signer",
        "Lab",
        "--key-file",
        key.to_str().unwrap(),
        "--utc",
        UTC,
        "--dev",
    ];
    let run = |a: &[&str]| {
        let mut v: Vec<&str> = a.to_vec();
        v.extend_from_slice(&sig);
        bin(&v, &home)
    };
    let (ok, out) = run(&[
        "space-rx",
        "exposure",
        &p(&d.join("exposure.in")),
        "--out",
        &p(&d.join("exp")),
    ]);
    assert!(ok && out.contains("Test points"), "{out}");
    assert!(d.join("exp/passes.csv").is_file());
    let (ok, out) = run(&[
        "space-rx",
        "score",
        "--campaign",
        &p(&d.join("campaign.in")),
        "--mission",
        &p(&d.join("mission.in")),
        "--out",
        &p(&d.join("score")),
    ]);
    assert!(ok && out.contains("1 not met"), "{out}");
    assert!(d.join("score/template.md").is_file());
    let pk = hex::encode(license::public_key_for(&SEED));
    let (ok, out) = bin(
        &["space-rx", "verify", &p(&d.join("score")), "--trust", &pk],
        &home,
    );
    assert!(ok && out.contains("trusted"), "{out}");
    let (ok, out) = run(&[
        "space-rx",
        "score",
        "--campaign",
        &p(&d.join("campaign.in")),
        "--mission",
        &p(&d.join("mission.in")),
        "--out",
        &p(&d.join("score")),
    ]);
    assert!(!ok && out.contains("never overwritten"), "{out}");
    let db = d.join("db.json");
    let (ok, out) = bin(
        &[
            "space-rx",
            "db",
            "add",
            "--db",
            &p(&db),
            "--scorecard",
            &p(&d.join("score")),
            "--trust",
            &pk,
            "--dev",
        ],
        &home,
    );
    assert!(ok && out.contains("1 entries"), "{out}");
    let (ok, out) = bin(
        &[
            "space-rx",
            "db",
            "select",
            "--db",
            &p(&db),
            "--min-js",
            "25",
        ],
        &home,
    );
    assert!(
        ok && out.contains("EX-1") && out.contains("1 of 1"),
        "{out}"
    );
    let (ok, out) = bin(&["space-rx", "db", "schema"], &home);
    assert!(ok && out.contains("kshana-space-rx-db/1"), "{out}");
    // Fixture, detection with ground truth.
    let (ok, out) = bin(
        &[
            "space-rx",
            "inject",
            "--trace",
            &p(&d.join("trace.in")),
            "--events",
            &p(&d.join("inject.in")),
            "--out",
            &p(&d.join("fx")),
        ],
        &home,
    );
    assert!(ok && out.contains("SYNTHETIC fixture"), "{out}");
    let (ok, out) = run(&[
        "space-rx",
        "detect",
        "--trace",
        &p(&d.join("fx/trace.csv")),
        "--truth",
        &p(&d.join("fx/truth.json")),
        "--out",
        &p(&d.join("det")),
    ]);
    assert!(ok && out.contains("\"missed\":[]"), "{out}");
    // Interface vectors and an OBC log (the reference parser's own actions).
    let (ok, out) = bin(
        &[
            "space-rx",
            "vectors",
            "--trace",
            &p(&d.join("trace.in")),
            "--out",
            &p(&d.join("vec")),
            "--dev",
        ],
        &home,
    );
    assert!(ok && out.contains("corrupt"), "{out}");
    let (ok, out) = run(&[
        "space-rx",
        "tolerance",
        "--vectors",
        &p(&d.join("vec/vectors.json")),
        "--log",
        &p(&d.join("vec/reference-actions.csv")),
        "--out",
        &p(&d.join("tol")),
    ]);
    assert!(ok && out.contains("not met 0"), "{out}");
    let (ok, out) = bin(
        &["space-rx", "rate", &p(&d.join("rate.in")), "--dev"],
        &home,
    );
    assert!(ok && out.contains("solar minimum"), "{out}");
}

// ---------------------------------------------------------------------------------------------
// Resilience evaluation under degradation scenarios (evaluation only).

mod resilience {
    use kshana_pro::space_rx::resilience as rs;

    fn run(good: bool) -> rs::Report {
        let (l, r, q) = rs::synth(good);
        rs::score(&toml::from_str(&q).unwrap(), &l, &r).unwrap()
    }

    #[test]
    fn good_and_faulty_receivers_are_separated() {
        let g = run(true);
        assert!(
            g.scenarios.iter().all(|s| s.verdict == "met"),
            "{:#?}",
            g.scenarios
        );
        let eph = g
            .scenarios
            .iter()
            .find(|s| s.id == "ephemeris-ramp")
            .unwrap();
        assert_eq!(eph.time_to_flag_s, Some(15.0));
        assert_eq!(eph.misleading_unflagged, 0);
        let out = g.scenarios.iter().find(|s| s.id == "outage-120s").unwrap();
        assert_eq!(out.availability, 0.0);
        assert!(out.holdover_max_time_error_ns.unwrap() < 300.0);

        let f = run(false);
        let s = |id: &str| f.scenarios.iter().find(|s| s.id == id).unwrap().clone();
        // Unflagged biased fixes during the ephemeris fault, and slow recovery.
        assert!(s("ephemeris-ramp").misleading_unflagged > 100);
        assert_eq!(s("ephemeris-ramp").verdict, "not met");
        // The clock drifts beyond the time limit in the outage; recovery is late.
        assert!(s("outage-120s").holdover_max_time_error_ns.unwrap() > 300.0);
        assert_eq!(s("outage-120s").verdict, "not met");
        // Availability above the constellation below the mission's minimum.
        assert!(s("high-altitude-weak").availability < 0.7);
        assert_eq!(s("scintillation").verdict, "met");
        assert!(rs::EVIDENCE.contains("Evaluation only"));
    }

    #[test]
    fn limits_need_a_source() {
        let (l, r, q) = rs::synth(true);
        let mut q: rs::Request = toml::from_str(&q).unwrap();
        q.limits.source = " ".into();
        assert!(rs::score(&q, &l, &r).is_err());
    }
}
