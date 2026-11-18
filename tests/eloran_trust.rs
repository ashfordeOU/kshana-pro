// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! eLoran time solutions as the `eloran` source of multi-source trust: the adapter on the
//! software receiver's real output, and a selector run where GNSS fails (outage, then a
//! timing spoof) and eLoran is chosen.

use kshana_eloran::rx::{self, Corrections, RxConfig, RxInput, StationClock, TimeSolution};
use kshana_eloran::time::AbsTime;
use kshana_eloran::Scenario;
use kshana_pro::eloran_trust::{
    epoch_from_time_solution, merge_table, source_def, table, time_solutions_by_window,
    EloranTrustConfig,
};
use kshana_pro::multi_source::{parse_table, run_table, Choice, Config, SourceDef, Status};

fn cfg_utc_a() -> EloranTrustConfig {
    let mut c = EloranTrustConfig::default();
    // UTC(A) runs 50 ns ahead of the common reference (the scenario's time_refs offset).
    c.time_ref_offsets_ns.insert("UTC(A)".into(), 50.0);
    c
}

#[test]
fn receiver_time_solutions_become_eloran_readings() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/kshana-eloran/scenarios/rx-high-snr.toml"
    ))
    .unwrap();
    let sc = Scenario::from_toml(&src).unwrap();
    let (sink, _truth, _) = kshana_eloran::run_to_memory(&src).unwrap();
    let start = AbsTime::from_utc_iso(&sc.scene.start_utc).unwrap();
    let chains: Vec<_> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let input = RxInput::new(sink.channels, sc.scene.params.fs_hz, start);
    let (_rx, out) = rx::process(&input, &chains, RxConfig::default()).unwrap();
    let rx_pos = sc.receiver.state(0.0).pos;
    let sols = time_solutions_by_window(
        &out.measurements,
        &chains,
        start,
        rx_pos,
        &Corrections::default(),
        0.5,
    );
    assert!(!sols.is_empty());
    let cfg = cfg_utc_a();
    for (t, ts) in &sols {
        let e = epoch_from_time_solution(*t, ts, &cfg);
        assert_eq!(e.status, Status::Ok, "{e:?}");
        // receiver clock = truth time, UTC(A) = truth + 50 ns, common reference = truth
        assert!(e.offset_ns.unwrap().abs() < 3.0, "{e:?}");
        assert!(e.stations.iter().all(|s| s.used));
        assert_eq!(e.stations.len(), 4);
    }
    // a station far off the others is rejected by the consistency check
    let mut ts = sols[0].1.clone();
    ts.stations[0].ref_minus_rx_ns += 2000.0;
    let e = epoch_from_time_solution(0.0, &ts, &cfg);
    assert!(!e.stations[0].used && e.stations[1..].iter().all(|s| s.used));
    assert!(e.offset_ns.unwrap().abs() < 3.0);
}

/// Deterministic normal draws.
struct Rng(u64);
impl Rng {
    fn u(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn n(&mut self) -> f64 {
        (-2.0 * self.u().ln()).sqrt() * (2.0 * std::f64::consts::PI * self.u()).cos()
    }
}

#[test]
fn gnss_fails_and_the_selector_chooses_eloran() {
    let truth = |t: f64| 20.0 + 1e-3 * t; // local clock − UTC (ns): 1e-12 drift
    let mut rng = Rng(7);
    let (outage, spoof) = ((1500.0, 2200.0), (2200.0, 2600.0));
    let mut gnss = String::from("t_s,gnss_status,gnss_offset_ns\n");
    let mut epochs = Vec::new();
    let cfg = cfg_utc_a();
    let mut t = 0.0;
    while t <= 3200.0 {
        // GNSS: 5 ns noise; out, then spoofed (+800 ns, still reporting ok)
        if t >= outage.0 && t < outage.1 {
            gnss.push_str(&format!("{t},unavailable,\n"));
        } else {
            let bias = if t >= spoof.0 && t < spoof.1 {
                800.0
            } else {
                0.0
            };
            gnss.push_str(&format!("{t},ok,{:.3}\n", truth(t) + bias + 5.0 * rng.n()));
        }
        // eLoran: three stations on UTC(A), each UTC(A) − rx with 15 ns noise
        let ts = TimeSolution {
            stations: ["M", "X", "Y"]
                .iter()
                .map(|s| StationClock {
                    chain: "rx-A".into(),
                    station: s.to_string(),
                    time_ref: "UTC(A)".into(),
                    ref_minus_rx_ns: 50.0 - truth(t) + 15.0 * rng.n(),
                    std_ns: 20.0,
                    n: 4,
                })
                .collect(),
            by_time_ref: vec![],
        };
        epochs.push(epoch_from_time_solution(t, &ts, &cfg));
        t += 10.0;
    }
    let merged = merge_table(&gnss, &epochs, &cfg, 1.0).unwrap();
    assert!(merged.starts_with("t_s,gnss_status,gnss_offset_ns,eloran_status,eloran_offset_ns\n"));
    // the eLoran-only table has the same rows
    assert_eq!(table(&epochs, &cfg).lines().count(), merged.lines().count());

    let config = Config {
        sources: vec![
            SourceDef {
                id: "gnss".into(),
                group: "gnss".into(),
                sigma_ns: 15.0,
                authenticates_group: false,
            },
            source_def(&cfg),
        ],
        clock_class: "rafs".into(),
        tau0_s: 10.0,
    };
    let rows = parse_table(&config, &merged).unwrap();
    let run = run_table(&config, &rows, "").unwrap();
    assert!(run.evidence.verify(&run.decisions_csv).is_empty());
    let mut sel = kshana_pro::multi_source::Selector::new(config.clone()).unwrap();
    let mut n_eloran_outage = 0;
    let mut n_eloran_spoof = 0;
    for (t, obs) in &rows {
        let d = sel.step(*t, obs);
        let in_out = *t >= outage.0 + 10.0 && *t < outage.1;
        let in_spoof = *t >= spoof.0 && *t < spoof.1;
        if *t > 700.0 && *t < outage.0 {
            assert_eq!(
                d.choice,
                Choice::Source(0),
                "GNSS chosen while healthy at {t}"
            );
        }
        if in_out || in_spoof {
            assert_eq!(d.choice, Choice::Source(1), "eLoran chosen at {t}: {d:?}");
            let off = d.offset_s.unwrap() * 1e9;
            assert!((off - truth(*t)).abs() < 4.0 * 60.0, "{t}: {off}");
            if in_out {
                n_eloran_outage += 1;
            } else {
                n_eloran_spoof += 1;
                assert!(!d.verdicts[0].qualified(), "spoofed GNSS must not qualify");
            }
        }
    }
    assert!(n_eloran_outage >= 60 && n_eloran_spoof >= 35);
    let by = &run.evidence.counts.by_source;
    assert!(by[1].1 >= n_eloran_outage + n_eloran_spoof, "{by:?}");
}
