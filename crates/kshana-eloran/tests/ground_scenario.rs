// PROPRIETARY — Kshana Pro.
//! The groundwave mixed-path scenario runs end to end, deterministically, with ASF and ECD
//! in the truth, and the propagation of a 10-station scene with the full model is much
//! faster than real time (checked in release builds only).

use kshana_eloran::scenario::Scenario;

fn src() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios/groundwave-mixed-path.toml")).unwrap()
}

#[test]
fn groundwave_mixed_path_scenario_runs() {
    let (sink, truth, stats) = kshana_eloran::run_to_memory(&src()).unwrap();
    assert_eq!(stats.channels, 2);
    assert!(stats.truth_records > 0);
    assert!(sink.channels[0].iter().any(|v| *v != 0.0));
    for r in &truth {
        // delay decomposition adds up (ps rounding)
        assert!((r.delay_ns - (r.geom_ns + r.pf_ns + r.sf_ns + r.asf_ns)).abs() < 0.01, "{r:?}");
        assert!(r.sf_ns > 0.0 && r.field_uv_m > 0.0);
    }
    // the master of chain-A is inland (σ = 3 mS/m): ASF of hundreds of ns at least
    let m = truth.iter().find(|r| r.chain == "chain-A" && r.station == "M").unwrap();
    assert!(m.asf_ns > 200.0, "{}", m.asf_ns);
    assert!(m.ecd_us < 0.0);
    // deterministic
    let (_, truth2, _) = kshana_eloran::run_to_memory(&src()).unwrap();
    assert_eq!(truth, truth2);
}

#[test]
fn ten_station_propagation_faster_than_real_time() {
    let mut sc = Scenario::from_toml(&src()).unwrap();
    let base = sc.chains[1].chain.stations[1].clone();
    for k in 0..5 {
        let mut s = base.clone();
        s.id = format!("S{k}");
        s.position.lat_deg += 0.7 * k as f64 - 1.5;
        s.position.lon_deg += 1.1 * k as f64 - 3.0;
        s.emission_delay_us = 30_000.0 + 9_000.0 * k as f64;
        sc.chains[1].chain.stations.push(s);
    }
    let n: usize = sc.chains.iter().map(|c| c.chain.stations.len()).sum();
    assert_eq!(n, 10);
    let scene = sc.build().unwrap();
    // 60 s of 10 ms blocks along the moving receiver track (cold caches included)
    let t0 = std::time::Instant::now();
    let blocks = 6000;
    for b in 0..blocks {
        let rx = scene.trajectory.state(b as f64 * 0.01);
        for c in &scene.chains {
            for s in &c.stations {
                std::hint::black_box(scene.prop.paths(s, &rx));
            }
        }
    }
    let wall = t0.elapsed().as_secs_f64();
    if !cfg!(debug_assertions) {
        // propagation must take < 10 % of real time
        assert!(wall < 60.0 * 0.1, "propagation for 60 s of scene took {wall} s");
    }
}
