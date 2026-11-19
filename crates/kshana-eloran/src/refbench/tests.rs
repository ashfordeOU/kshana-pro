// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
use super::*;
use crate::elof::Record;
use std::sync::OnceLock;

/// Scenario ids and files, in registration order.
const SCENARIOS: &[(&str, &str)] = &[
    ("ELB-STATIC", "static.toml"),
    ("ELB-SHIP", "ship.toml"),
    ("ELB-DRONE", "drone.toml"),
    ("ELB-SKY-NIGHT", "skywave-night.toml"),
    ("ELB-CRI", "cri.toml"),
    ("ELB-IMPULSIVE", "impulsive.toml"),
    ("ELB-LDC", "ldc.toml"),
    ("ELB-MULTI-UTCK", "multi-utck.toml"),
];

/// Maintainer step (run once, by hand, before any scoring):
/// `cargo test --lib refbench::tests::write_registration -- --ignored`.
#[test]
#[ignore]
fn write_registration() {
    register(&bench_dir(), "0.1", "2026-11-19T00:00:00Z", SCENARIOS).unwrap();
}

fn bench() -> &'static Benchmark {
    static B: OnceLock<Benchmark> = OnceLock::new();
    B.get_or_init(|| Benchmark::load(&bench_dir()).unwrap())
}

/// The in-house software receiver's ELOF observations on every scenario.
fn reference() -> &'static BTreeMap<String, ElofFile> {
    static R: OnceLock<BTreeMap<String, ElofFile>> = OnceLock::new();
    R.get_or_init(|| {
        bench()
            .scenarios
            .iter()
            .map(|s| {
                let run = s.simulate().unwrap();
                (
                    s.id.clone(),
                    software_receiver_observations(s, &run).unwrap(),
                )
            })
            .collect()
    })
}

fn copy_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-elb-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    for e in std::fs::read_dir(bench_dir()).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(&p, d.join(p.file_name().unwrap())).unwrap();
    }
    d
}

#[test]
fn shipped_registration_verifies_and_covers_the_scenario_set() {
    let b = bench();
    let ids: Vec<&str> = b.scenarios.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, SCENARIOS.iter().map(|s| s.0).collect::<Vec<_>>());
    // every scenario has bars, and every bar names a known metric of the scorer
    let known = [
        "tracked_fraction",
        "toa_rms_ns",
        "toa_bias_ns",
        "toa_std_ns",
        "toa_max_abs_ns",
        "ecd_rms_us",
        "snr_error_db",
        "cycle_errors",
        "cycle_slips",
        "time_error_ns",
        "ldc_symbol_error_rate",
        "ldc_symbols",
    ];
    for s in &b.scenarios {
        let bars = b.bars_of(&s.id);
        assert!(bars.len() >= 3, "{}", s.id);
        assert!(bars.iter().any(|c| c.metric == "cycle_errors"), "{}", s.id);
        for c in bars {
            assert!(known.contains(&c.metric.as_str()), "{}", c.metric);
            assert!(c.illustrative);
        }
    }
    assert_eq!(b.registration_sha256.len(), 64);
    // the scenarios differ in what they stress
    let sc = |id: &str| b.scenario(id).unwrap().simulate().unwrap();
    let sky = sc("ELB-SKY-NIGHT");
    assert!(sky.truth.iter().all(|r| r.sky_delay_us.is_some()));
    let multi = sc("ELB-MULTI-UTCK");
    let refs: std::collections::BTreeSet<i64> = multi
        .truth
        .iter()
        .map(|r| r.time_ref_offset_ns.round() as i64)
        .collect();
    assert_eq!(
        refs.into_iter().collect::<Vec<_>>(),
        vec![-30, 50],
        "two UTC(k)"
    );
}

#[test]
fn software_receiver_scores_within_its_bars() {
    let b = bench();
    let card = score_receiver(b, "kshana-software-rx", reference()).unwrap();
    for r in &card.results {
        assert_eq!(
            r.verdict,
            Verdict::Pass,
            "{}: {:?} {:?}",
            r.id,
            r.scored
                .iter()
                .filter(|s| s.verdict != Verdict::Pass)
                .collect::<Vec<_>>(),
            r.error
        );
        assert_eq!(r.observations_sha256.len(), 64);
    }
    assert_eq!(card.fail, 0);
    assert_eq!(card.not_evaluated, 0);
    assert_eq!(card.pass, b.bars.bars.len());
    assert_eq!(card.scenarios_passed, SCENARIOS.len());
    assert_eq!(card.score(), 1.0);
    assert_eq!(card.receiver_model, "Kshana software receiver");
    assert!(card.verify_seal());
    // the LDC scenario really compared symbols
    let ldc = card.results.iter().find(|r| r.id == "ELB-LDC").unwrap();
    assert!(ldc.metrics["ldc_symbols"] >= 100.0);
    assert_eq!(ldc.metrics["ldc_symbol_error_rate"], 0.0);
    // JSON round trip keeps the seal valid
    let back: Scorecard = serde_json::from_str(&serde_json::to_string(&card).unwrap()).unwrap();
    assert!(back.verify_seal());
    let mut lb = Leaderboard::new(b);
    assert_eq!(lb.add(&card, "2026-11-19T00:00:00Z").unwrap(), 1);
    let lb2 = Leaderboard::from_json(&serde_json::to_string(&lb).unwrap()).unwrap();
    assert_eq!(lb2, lb);
}

#[test]
fn tampered_bars_or_scenarios_are_detected() {
    // a relaxed bar after registration
    let d = copy_dir("bars");
    let p = d.join("bars.toml");
    let t = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, t.replacen("limit = 60.0", "limit = 600.0", 1)).unwrap();
    let e = Benchmark::load(&d).unwrap_err();
    assert!(e.contains("pre-registered SHA-256"), "{e}");
    assert!(e.contains("bars"), "{e}");
    // a one-byte whitespace change is detected too
    std::fs::write(&p, format!("{t}\n")).unwrap();
    assert!(Benchmark::load(&d).is_err());
    std::fs::write(&p, &t).unwrap();
    assert!(Benchmark::load(&d).is_ok(), "restored file verifies");
    // an easier scenario (lower noise) after registration
    let s = d.join("static.toml");
    let st = std::fs::read_to_string(&s).unwrap();
    std::fs::write(
        &s,
        st.replace("rms_uv_m_20khz = 2600.0", "rms_uv_m_20khz = 26.0"),
    )
    .unwrap();
    let e = Benchmark::load(&d).unwrap_err();
    assert!(e.contains("static.toml"), "{e}");
    // re-registering is possible, but changes the registration hash every scorecard carries
    let reg = register(&d, "0.1", "2026-11-20T00:00:00Z", SCENARIOS).unwrap();
    let b2 = Benchmark::load(&d).unwrap();
    assert_ne!(
        reg.scenarios[0].sha256,
        bench().registration.scenarios[0].sha256
    );
    assert_ne!(b2.registration_sha256, bench().registration_sha256);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn degraded_receiver_fails_and_tampered_scorecards_are_refused() {
    let b = bench();
    let good = &reference()["ELB-STATIC"];
    // a receiver one carrier cycle late on station X: cycle errors
    let mut bad = good.clone();
    for r in &mut bad.records {
        if let Record::Obs(o) = r {
            if o.station == "X" {
                o.toa_ns += 10_000.0;
            }
        }
    }
    let mut obs = BTreeMap::new();
    obs.insert("ELB-STATIC".to_string(), bad);
    let card = score_receiver(b, "late-receiver", &obs).unwrap();
    let st = &card.results[0];
    assert_eq!(st.verdict, Verdict::Fail);
    assert!(st.metrics["cycle_errors"] > 0.0);
    let ce = st
        .scored
        .iter()
        .find(|s| s.metric == "cycle_errors")
        .unwrap();
    assert_eq!(ce.verdict, Verdict::Fail);
    // scenarios without observations are not evaluated, never passed
    assert!(card.results[1..]
        .iter()
        .all(|r| r.verdict == Verdict::NotEvaluated && r.error.is_some()));
    assert!(card.not_evaluated > 0);
    // observations of another scenario are refused for this one
    let mut obs = BTreeMap::new();
    obs.insert("ELB-SHIP".to_string(), good.clone());
    let c = score_receiver(b, "mixed-up", &obs).unwrap();
    let ship = c.results.iter().find(|r| r.id == "ELB-SHIP").unwrap();
    assert!(
        ship.error.as_deref().unwrap().contains("do not reference"),
        "{:?}",
        ship.error
    );
    // leaderboard: the reference ranks above the degraded receiver
    let reference_card = score_receiver(b, "kshana-software-rx", reference()).unwrap();
    let mut lb = Leaderboard::new(b);
    lb.add(&card, "2026-11-19T01:00:00Z").unwrap();
    lb.add(&reference_card, "2026-11-19T02:00:00Z").unwrap();
    assert_eq!(lb.entries[0].receiver, "kshana-software-rx");
    assert_eq!(lb.entries[1].rank, 2);
    // a scorecard edited after scoring breaks its seal and is refused
    let mut forged = card.clone();
    forged.results[0].verdict = Verdict::Pass;
    assert!(!forged.verify_seal());
    let e = lb.add(&forged, "2026-11-19T03:00:00Z").unwrap_err();
    assert!(e.contains("seal"), "{e}");
    // and one scored against other bars (re-sealed) is refused
    let mut other = card.clone();
    other.bars_sha256 = "0".repeat(64);
    other.seal();
    assert!(lb.add(&other, "2026-11-19T03:00:00Z").is_err());
    // unknown scenario ids are an error
    let mut obs = BTreeMap::new();
    obs.insert("ELB-NOPE".to_string(), good.clone());
    assert!(score_receiver(b, "x", &obs).is_err());
}
