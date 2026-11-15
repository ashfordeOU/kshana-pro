// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Timing scorecards under spoofing scenarios (roadmap 0.42, T4). Development inputs are
//! synthetic clocks written here; the public sample uses examples/timing-fleet.

use std::path::{Path, PathBuf};

use kshana_pro::timing::model::{synth_clock, SplitMix};
use kshana_pro::timing::scorecard::{self, TimingScorecardSpec};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("t4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ticc(path: &Path, seed: u64, q_wf: f64, q_rw: f64) {
    let mut rng = SplitMix::new(seed);
    let x = synth_clock(&mut rng, 6 * 3600, 1.0, q_wf, q_rw, 0.0, 1e-12, 1e-11);
    let mut s = String::from("# SYNTHETIC test clock\n");
    for (i, v) in x.iter().enumerate() {
        s.push_str(&format!("{:.12} chA\n", 1000.0 + i as f64 + v));
    }
    std::fs::write(path, s).unwrap();
}

/// A synthetic fleet: two rubidium-like grandmasters at one site, one downstream boundary clock.
pub fn dev_fleet(d: &Path) -> PathBuf {
    ticc(&d.join("gm-a.ticc"), 7, 1e-24, 1e-32);
    ticc(&d.join("gm-b.ticc"), 8, 1e-24, 1e-32);
    std::fs::write(
        d.join("bc.ptp4l.log"),
        (0..400)
            .map(|i| {
                format!(
                    "ptp4l[{}.000]: master offset {:>10} s2 freq   +2100 path delay       512\n",
                    50000 + 16 * i,
                    (i % 7) as i64 * 10 - 30
                )
            })
            .collect::<String>(),
    )
    .unwrap();
    std::fs::write(
        d.join("fleet.toml"),
        "kind = \"pro-timing-holdover\"\ntitle = \"Dev fleet\"\nwindow_h = 4\nhorizon_h = 12\n\
         [[limits]]\nname = \"1 us\"\nvalue_s = 1e-6\n[[limits]]\nname = \"100 us\"\nvalue_s = 100e-6\n\
         [[clocks]]\nid = \"gm-a\"\nrole = \"grandmaster\"\n[clocks.source]\nformat = \"ticc\"\npath = \"gm-a.ticc\"\nchannel = \"chA\"\n\
         [[clocks]]\nid = \"gm-b\"\nrole = \"grandmaster\"\n[clocks.source]\nformat = \"ticc\"\npath = \"gm-b.ticc\"\nchannel = \"chA\"\n\
         [[clocks]]\nid = \"bc\"\nrole = \"boundary\"\nclass = \"ocxo\"\n[clocks.source]\nformat = \"ptp4l\"\npath = \"bc.ptp4l.log\"\n",
    )
    .unwrap();
    d.join("fleet.toml")
}

fn dev_score() -> scorecard::TimingScorecard {
    let d = tmp("dev");
    dev_fleet(&d);
    let src = "kind = \"pro-timing-scorecard\"\ntitle = \"Dev\"\nfleet = \"fleet.toml\"\ngrandmasters = [\"gm-a\"]\n\
               scenarios = [\"jammertest-time-spoofing\", \"texbat-ds3-matched-power-time-push\", \"baltic-timing-holdover-under-jamming\"]\n\
               [[pairs]]\na = \"gm-a\"\nb = \"gm-b\"\n[[network]]\nclock = \"bc\"\nupstream = \"gm-a\"\n";
    std::fs::write(d.join("req.toml"), src).unwrap();
    let (spec, src) = TimingScorecardSpec::from_file(&d.join("req.toml")).unwrap();
    scorecard::score(&spec, &src).unwrap()
}

#[test]
fn grandmasters_and_their_network_are_scored_under_each_timing_scenario() {
    let s = dev_score();
    assert_eq!(s.results.len(), 3);
    for r in &s.results {
        assert_eq!(
            r.clocks.len(),
            2,
            "the grandmaster and its downstream clock"
        );
        assert!(r.clocks[1].position.contains("gm-a > bc"));
        assert!(r.detection_s.is_some());
    }
    let denial = s
        .results
        .iter()
        .find(|r| r.scenario.starts_with("baltic"))
        .unwrap();
    assert_eq!(denial.detected_by, "loss-of-signal");
    // One-receiver scope with a site pair: the cross-check threshold is computed.
    let ds3 = s
        .results
        .iter()
        .find(|r| r.scenario.starts_with("texbat"))
        .unwrap();
    assert!(ds3.cross_check_threshold_s.is_some());
    let jt = s
        .results
        .iter()
        .find(|r| r.scenario.starts_with("jammertest"))
        .unwrap();
    assert!(
        jt.cross_check_threshold_s.is_none(),
        "an all-receivers event defeats the cross-check"
    );
    assert!(s.label.contains("V5b"));
    let html = scorecard::html(&s).to_ascii_lowercase();
    for w in ["compliant", "conformant", "certified"] {
        assert!(!html.contains(w));
    }
}

#[test]
fn an_undetected_push_is_scored_as_breaching_and_misses_level_one() {
    // A detector so wide that nothing is detected: the full push passes to the network.
    let d = tmp("wide");
    dev_fleet(&d);
    let src = "kind = \"pro-timing-scorecard\"\ntitle = \"Wide\"\nfleet = \"fleet.toml\"\ngrandmasters = [\"gm-a\"]\n\
               scenarios = [\"jammertest-time-spoofing\"]\n[detector]\nk_sigma = 1e9\nfloor_s = 1.0\ncross_check = false\n";
    let spec = TimingScorecardSpec::parse(src).unwrap();
    let mut spec = spec;
    spec.base_dir = Some(d.clone());
    let s = scorecard::score(&spec, src).unwrap();
    let r = &s.results[0];
    assert_eq!(r.detection_s, None);
    assert!(r.clocks[0].breached_before_detection["1 us"]);
    assert_eq!(r.levels.l1, kshana_pro::scorecards::levels::Verdict::NotMet);
    assert_eq!(s.levels.highest_supported, 0);
}

#[test]
fn requests_are_checked() {
    assert!(TimingScorecardSpec::parse("kind = \"pro-timing-scorecard\"\ntitle = \"x\"\nfleet = \"f\"\ngrandmasters = []\nscenarios = [\"a\"]\n").is_err());
    let d = tmp("bad");
    dev_fleet(&d);
    let mut spec = TimingScorecardSpec::parse("kind = \"pro-timing-scorecard\"\ntitle = \"x\"\nfleet = \"fleet.toml\"\ngrandmasters = [\"gm-z\"]\nscenarios = [\"jammertest-time-spoofing\"]\n").unwrap();
    spec.base_dir = Some(d.clone());
    assert!(scorecard::score(&spec, "")
        .unwrap_err()
        .to_string()
        .contains("not in the fleet"));
    spec.grandmasters = vec!["gm-a".into()];
    spec.scenarios = vec!["hormuz-meaconing-delay".into()];
    assert!(scorecard::score(&spec, "")
        .unwrap_err()
        .to_string()
        .contains("no [timing] profile"));
}

#[test]
fn the_kind_is_licence_gated_at_pro() {
    use kshana_pro::license::{self, tiers, Entitlement, Tier};
    let ent = |tier| Entitlement {
        licensee: "t".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&ent(Tier::Evaluation), scorecard::KIND, 0).is_err());
    assert!(license::check(&ent(Tier::Pro), scorecard::KIND, 0).is_ok());
    assert_eq!(
        tiers::edition_of(scorecard::KIND),
        Some(tiers::Edition::Pro)
    );
}

// ---------------------------------------------------------------------------------------
// The public sample (pre-registered in docs/evidence/t4-timing-scorecard/).
// ---------------------------------------------------------------------------------------

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sample() -> (TimingScorecardSpec, scorecard::TimingScorecard, String) {
    let (spec, src) =
        TimingScorecardSpec::from_file(&root().join("examples/timing-scorecard/request.toml"))
            .unwrap();
    let s = scorecard::score(&spec, &src).unwrap();
    (spec, s, src)
}

/// T4-B1. Regenerate with KSHANA_PRO_REGEN_T4_SAMPLE=<key file>.
#[test]
fn t4_b1_the_sample_is_reproducible_and_verifies_with_its_published_demo_key() {
    let (_, a, src) = sample();
    let (_, b, _) = sample();
    let ja = serde_json::to_string_pretty(&a).unwrap() + "\n";
    assert_eq!(ja, serde_json::to_string_pretty(&b).unwrap() + "\n");
    let dir = root().join("report-templates/samples/timing-scorecard-public");
    let keyfile =
        root().join("report-templates/samples/timing-scorecard-public.demo-public-key.txt");
    if let Some(kf) = std::env::var_os("KSHANA_PRO_REGEN_T4_SAMPLE") {
        let seed = kshana_pro::license::parse_key_hex(std::fs::read_to_string(kf).unwrap().trim())
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        scorecard::write_dir(&dir, &a, &src).unwrap();
        kshana_pro::signed_dossier::sign(
            &dir,
            &seed,
            "Kshana Pro sample (demo key)",
            "2026-11-15T14:30:00Z",
        )
        .unwrap();
        std::fs::write(
            &keyfile,
            hex::encode(kshana_pro::license::public_key_for(&seed)) + "\n",
        )
        .unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("timing-scorecard.json")).unwrap(),
        ja,
        "the sample is stale; regenerate it"
    );
    let pk = kshana_pro::license::parse_key_hex(std::fs::read_to_string(&keyfile).unwrap().trim())
        .unwrap();
    assert!(kshana_pro::signed_dossier::verify(&dir, &[pk])
        .unwrap()
        .passed());
    let t = tmp("t4-tamper");
    for f in [
        "timing-scorecard.json",
        "report.html",
        "request.toml",
        "README.txt",
        "signature.json",
    ] {
        std::fs::copy(dir.join(f), t.join(f)).unwrap();
    }
    let mut j = std::fs::read(t.join("request.toml")).unwrap();
    j[10] ^= 1;
    std::fs::write(t.join("request.toml"), j).unwrap();
    assert!(!kshana_pro::signed_dossier::verify(&t, &[pk])
        .unwrap()
        .passed());
}

/// T4-B2 (a): a bisection on the continuous crossing agrees with the 1 s grid; (b) the denial
/// holdover warnings agree with T1's own fleet assessment.
#[test]
fn t4_b2_an_independent_route_agrees() {
    let (spec, s, _) = sample();
    let fleet_path = root().join("examples/timing-fleet/fleet.toml");
    let mut fs =
        kshana_pro::timing::fleet::FleetSpec::parse(&std::fs::read_to_string(&fleet_path).unwrap())
            .unwrap();
    fs.resolve_paths(fleet_path.parent().unwrap());
    let a = kshana_pro::timing::fleet::assess(&fs).unwrap();
    let d = &spec.detector;
    for r in s.results.iter().filter(|r| r.detected_by == "own-envelope") {
        let m = &a.clock(&r.grandmaster).unwrap().holdover;
        let f = |u: f64| {
            r.profile.offset(u).abs() - (d.k_sigma * m.sigma(u) + m.mean(u).abs() + d.floor_s)
        };
        // Bracket on a 0.01 s scan, then bisect.
        let mut lo = 0.0;
        while f(lo + 0.01) <= 0.0 {
            lo += 0.01;
            assert!(lo < r.profile.duration_s);
        }
        let mut hi = lo + 0.01;
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                hi = mid
            } else {
                lo = mid
            }
        }
        let det = r.detection_s.unwrap();
        assert!(
            (det - hi).abs() <= 1.0,
            "{} {}: grid {det} bisection {hi}",
            r.grandmaster,
            r.scenario
        );
    }
    let mut compared = 0;
    for r in s.results.iter().filter(|r| r.profile.kind == "denial") {
        let ca = a.clock(&r.grandmaster).unwrap();
        let gm = r.clocks.iter().find(|c| c.clock == r.grandmaster).unwrap();
        let t1_step = ca
            .tau0_s
            .max(a.horizon_s / kshana_pro::timing::fleet::MAX_GRID);
        let t4_step = (r.profile.duration_s / 20_000.0).max(1.0);
        for b in &ca.breaches {
            let t4 = gm.holdover_warning_s[&b.name];
            match (b.t_warn_s.filter(|t| *t <= r.profile.duration_s), t4) {
                (Some(x), Some(y)) => {
                    assert!(
                        (x - y).abs() <= t1_step.max(t4_step),
                        "{} {}: T1 {x} T4 {y}",
                        r.grandmaster,
                        b.name
                    );
                    compared += 1;
                }
                (None, None) => {}
                other => panic!("{} {}: {other:?}", r.grandmaster, b.name),
            }
        }
    }
    assert!(compared > 0);
}

/// T4-B3: every time push detected at the grandmaster before its tightest limit. Published
/// outcome (RESULTS.md): met.
const T4_B3_MET: bool = true;

#[test]
fn t4_b3_detection_before_breach_is_as_published() {
    let (_, s, _) = sample();
    let met = s
        .results
        .iter()
        .filter(|r| r.profile.kind == "time-push")
        .all(|r| {
            let gm = r.clocks.iter().find(|c| c.clock == r.grandmaster).unwrap();
            r.detection_s.is_some()
                && !gm.breached_before_detection[gm.tightest_limit.as_ref().unwrap()]
        });
    assert_eq!(met, T4_B3_MET);
    assert_eq!(
        s.results
            .iter()
            .filter(|r| r.profile.kind == "time-push")
            .count(),
        4
    );
}
