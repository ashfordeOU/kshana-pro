// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! PNT-DP reference implementation (W1119 C2): the test vectors' integrity flags, the
//! committed vector files, a Monte Carlo check of the protection level, input checks and the
//! CLI. Every input here is SYNTHETIC.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana::interference_map::grid::haversine_m;
use kshana_pro::pntdp::vectors::{self, Vector};
use kshana_pro::pntdp::{self, Config, Integrity};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pntdp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn vector(name: &str) -> Vector {
    vectors::all().into_iter().find(|v| v.name == name).unwrap()
}

fn outputs(v: &Vector) -> Vec<pntdp::Output> {
    pntdp::run(&Config::parse(&v.config).unwrap(), &v.samples).unwrap()
}

#[test]
fn every_vector_meets_its_expected_flags() {
    for v in vectors::all() {
        let p = vectors::check(&v).unwrap();
        assert!(
            p.is_empty(),
            "{}: {} mismatches, first: {:?}",
            v.name,
            p.len(),
            &p[..p.len().min(8)]
        );
        // A vector pins most of its epochs.
        let pinned = v.expected.iter().filter(|e| e.integrity.is_some()).count();
        assert!(
            pinned * 10 >= v.expected.len() * 7,
            "{}: only {pinned} pinned",
            v.name
        );
    }
}

#[test]
fn a_coherent_gnss_spoof_is_excluded_and_never_reported_at_high_integrity() {
    let v = vector("gnss-spoof");
    let out = outputs(&v);
    for (o, tr) in out.iter().zip(&v.truth) {
        let err = haversine_m(tr.0, tr.1, o.lat_deg.unwrap(), o.lon_deg.unwrap());
        // High integrity is never claimed on a solution more than its protection level away
        // from the truth.
        if o.integrity >= Integrity::Medium {
            let pl = o.protection_level_m.unwrap();
            assert!(
                err <= pl,
                "t = {}: error {err:.1} m > PL {pl:.1} m ({:?})",
                o.t_s,
                o.selected
            );
        }
        // Once the drag is well beyond the consistency bound, the solution is eLoran's and
        // within its own error (sqrt(6) * 20 m).
        if o.t_s >= 130.0 {
            assert!(err < 50.0, "t = {}: {err}", o.t_s);
        }
    }
    // Integrity falls from high when the spoof is first seen, and the fall is raised.
    let first_drop = out
        .iter()
        .position(|o| o.t_s > 60.0 && o.integrity < Integrity::High)
        .unwrap();
    assert_eq!(out[first_drop].status, "raised");
    assert!(
        out[first_drop].t_s <= 75.0,
        "seen at {}",
        out[first_drop].t_s
    );
}

#[test]
fn jump_and_frozen_sensors_are_flagged_at_the_right_epoch() {
    let out = outputs(&vector("jump"));
    let at = |t: f64| out.iter().find(|o| o.t_s == t).unwrap();
    assert_eq!(at(79.0).flags_text(), "");
    assert_eq!(at(80.0).flags_text(), "gps:jump");
    assert_eq!(at(81.0).flags_text(), "gps:inconsistent");
    assert_eq!(at(80.0).selected.as_deref(), Some("galileo"));
    let out = outputs(&vector("frozen"));
    let at = |t: f64| out.iter().find(|o| o.t_s == t).unwrap();
    assert!(!at(103.0).flags_text().contains("frozen"));
    assert_eq!(at(104.0).flags_text(), "galileo:frozen");
    assert_eq!(at(240.0).integrity, Integrity::High);
}

#[test]
fn disagreement_without_a_third_source_gives_no_integrity() {
    let out = outputs(&vector("disagreement"));
    assert!(out
        .iter()
        .filter(|o| o.t_s < 50.0)
        .all(|o| o.integrity == Integrity::Medium));
    // The step itself fails eLoran's rate check (GPS alone: low), then the two disagree.
    let at50 = out.iter().find(|o| o.t_s == 50.0).unwrap();
    assert_eq!((at50.integrity, at50.status), (Integrity::Low, "raised"));
    let after: Vec<_> = out.iter().filter(|o| o.t_s >= 51.0).collect();
    assert!(after
        .iter()
        .all(|o| o.integrity == Integrity::None && o.protection_level_m.is_none()));
    assert_eq!(after[0].status, "raised");
    assert_eq!(after[1].status, "ok");
}

#[test]
fn committed_vector_files_match_the_generator_and_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("pntdp/vectors");
    if std::env::var("PNTDP_WRITE_VECTORS").is_ok() {
        vectors::write_all(&root).unwrap();
    }
    let d = tmp("gen");
    vectors::write_all(&d).unwrap();
    for v in vectors::all() {
        for f in ["config.toml", "inputs.csv", "expected.csv", "README.txt"] {
            let a = std::fs::read_to_string(d.join(v.name).join(f)).unwrap();
            let b = std::fs::read_to_string(root.join(v.name).join(f))
                .unwrap_or_else(|_| panic!("pntdp/vectors/{}/{f} missing", v.name));
            assert_eq!(a, b, "pntdp/vectors/{}/{f} is out of date", v.name);
        }
        // Round trip through the files: same flags.
        let cfg =
            Config::parse(&std::fs::read_to_string(root.join(v.name).join("config.toml")).unwrap())
                .unwrap();
        let s = pntdp::parse_inputs(
            &cfg,
            &std::fs::read_to_string(root.join(v.name).join("inputs.csv")).unwrap(),
        )
        .unwrap();
        let e = vectors::parse_expected(
            &std::fs::read_to_string(root.join(v.name).join("expected.csv")).unwrap(),
        )
        .unwrap();
        let p = vectors::compare(&pntdp::run(&cfg, &s).unwrap(), &e);
        assert!(p.is_empty(), "{}: {:?}", v.name, &p[..p.len().min(5)]);
    }
}

/// Monte Carlo: with Gaussian errors, the protection level bounds the true error unless the
/// agreeing reference it was computed from is itself beyond k = 4 sigma (2-D probability
/// exp(-8) = 3.4e-4 per epoch). Over 20 runs x 241 epochs the expected number of misses is
/// below 2; the bar is 10.
#[test]
fn the_protection_level_bounds_the_error_over_many_runs() {
    let cfg = Config::parse(vectors::CONFIG_FULL).unwrap();
    let (mut assessed, mut misses, mut high) = (0usize, 0usize, 0usize);
    for seed in 0..20u64 {
        for gen in [vectors::nominal_gaussian, vectors::spoof_gaussian] {
            let (s, truth) = gen(1000 + seed);
            for (o, tr) in pntdp::run(&cfg, &s).unwrap().iter().zip(&truth) {
                if o.integrity >= Integrity::Medium {
                    assessed += 1;
                    high += (o.integrity == Integrity::High) as usize;
                    let err = haversine_m(tr.0, tr.1, o.lat_deg.unwrap(), o.lon_deg.unwrap());
                    if err > o.protection_level_m.unwrap() {
                        misses += 1;
                    }
                }
            }
        }
    }
    assert!(assessed > 8000, "{assessed}");
    assert!(high > 4000, "{high}");
    assert!(misses <= 10, "{misses} misses in {assessed} epochs");
}

#[test]
fn input_checks_mark_incomplete_invalid_and_stale_data() {
    let cfg = Config::parse(vectors::CONFIG_FULL).unwrap();
    let text = "t_s,source,lat_deg,lon_deg,sigma_m,valid,vn_mps,ve_mps\n\
        0,gps,59.4,24.7,,1,,\n0,eloran,59.4,24.7,,1,,\n\
        1,gps,,24.7,,1,,\n1,eloran,59.4,24.7,,0,,\n1,galileo,95.0,24.7,,1,,\n\
        2,eloran,59.4,24.7,,1,,\n6,eloran,59.4,24.7,,1,,\n";
    let s = pntdp::parse_inputs(&cfg, text).unwrap();
    let out = pntdp::run(&cfg, &s).unwrap();
    assert_eq!(out[0].integrity, Integrity::Medium);
    assert_eq!(
        out[1].flags_text(),
        "eloran:invalid;galileo:incomplete;gps:incomplete"
    );
    assert_eq!(out[1].integrity, Integrity::None);
    assert_eq!(out[1].selected, None);
    assert_eq!(out[1].status, "raised");
    assert!(
        out[3].flags_text().contains("gps:stale"),
        "{}",
        out[3].flags_text()
    );
    assert!(pntdp::parse_inputs(&cfg, "0,nope,1,1,,1,,\n").is_err());
    assert!(pntdp::parse_inputs(&cfg, "1,gps,1,1,,1,,\n0,gps,1,1,,1,,\n").is_err());
    assert!(
        Config::parse("[[source]]\nid = \"a\"\nkind = \"gnss\"\ngroup = \"g\"\nsigma_m = 0\n")
            .is_err()
    );
}

#[test]
fn cli_runs_writes_and_checks_vectors() {
    let d = tmp("cli");
    let home = d.join("home");
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
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
    };
    let vd = d.join("v");
    let (ok, out) = run(&["pntdp", "vectors", "--out", vd.to_str().unwrap()]);
    assert!(ok, "{out}");
    let (ok, out) = run(&["pntdp", "check", vd.join("jump").to_str().unwrap()]);
    assert!(ok, "{out}");
    let inputs = vd.join("gnss-spoof/inputs.csv");
    let (ok, out) = run(&["pntdp", "run", inputs.to_str().unwrap()]);
    assert!(!ok, "needs a licence: {out}");
    let o = d.join("out.csv");
    let (ok, out) = run(&[
        "pntdp",
        "run",
        inputs.to_str().unwrap(),
        "--out",
        o.to_str().unwrap(),
        "--dev",
    ]);
    assert!(ok, "{out}");
    let csv = std::fs::read_to_string(&o).unwrap();
    assert!(csv.starts_with(pntdp::OUTPUT_HEADER));
    assert!(csv.lines().last().unwrap().contains("eloran") && csv.contains("gps:inconsistent"));
    // A broken expectation is reported.
    let e = vd.join("jump/expected.csv");
    let t = std::fs::read_to_string(&e)
        .unwrap()
        .replace("80,galileo,high,gps:jump", "80,gps,high,");
    std::fs::write(&e, t).unwrap();
    let (ok, out) = run(&["pntdp", "check", vd.join("jump").to_str().unwrap()]);
    assert!(!ok && out.contains("mismatch"), "{out}");
}
