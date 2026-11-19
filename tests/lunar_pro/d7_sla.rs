// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// D7: LNSP SLA verification (part of the tests/lunar_pro.rs test binary).

use kshana_pro::lunar_pro::budget::std_normal;
use kshana_pro::lunar_pro::sla::*;
use rand::SeedableRng;

fn month(csv: &str, m: &str, slas: &[Sla]) -> MonthlyReport {
    let s = parse_samples(csv).unwrap();
    verify(
        &s,
        slas,
        &Options {
            month: m.into(),
            ..Options::default()
        },
        "x",
    )
    .unwrap()
}

fn builtins() -> Vec<Sla> {
    vec![
        Sla::builtin("lcrns-srd").unwrap(),
        Sla::builtin("moonlight-owr").unwrap(),
    ]
}

#[test]
fn d7_meeting_and_breaching_providers_get_the_right_verdicts() {
    let csv = synthetic_csv(2029, 6, 300.0, 0.5, &example_providers(), 3);
    let r = month(&csv, "2029-06", &builtins());
    assert_eq!(r.providers.len(), 2);
    let p = |n: &str| r.providers.iter().find(|p| p.provider == n).unwrap();
    let (good, bad) = (p("MEETS-CO"), p("BREACH-CO"));
    for x in &good.results {
        assert_eq!(x.verdict, Verdict::Meets, "{}: {x:#?}", x.sla);
        assert_eq!(x.integrity_events, 0);
    }
    for x in &bad.results {
        assert_eq!(x.verdict, Verdict::Breach, "{}", x.sla);
        assert_eq!(x.sise_verdict, Verdict::Breach, "{}", x.sla);
    }
    // 3σ estimates are close to 3 × the injected SISE σ (2.5 m and 7 m).
    let k3 = |p: &ProviderReport| p.results[0].sise.as_ref().unwrap().value;
    assert!((k3(good) - 7.5).abs() < 0.8, "{}", k3(good));
    assert!((k3(bad) - 21.0).abs() < 2.0, "{}", k3(bad));
    // Availability ≈ 1 − outage fraction; the breaching provider misses Moonlight's 95 %.
    let ml = &bad.results[1];
    assert!((ml.availability - 0.90).abs() < 0.03, "{}", ml.availability);
    assert_eq!(ml.availability_verdict, Verdict::Breach);
    assert!(good.results[1].availability > 0.97);
    // Moonlight is judged on the worst 24 h window, which is worse than the month as a whole.
    assert!(ml.worst_window_start.is_some());
    // The samples of the next month (synthetic 30 days spill none into July here) are counted apart.
    assert_eq!(r.samples_other_months, 0);
}

#[test]
fn d7_ksigma_interval_covers_the_true_value_at_the_stated_rate() {
    // White Gaussian SISE (decorrelation = sample step), σ = 4 m, 400 samples: the 95 % χ²
    // interval of 3·RMS must contain 12 m about 95 % of the time.
    let trials = 400;
    let mut cover = 0;
    let mut pcover = 0;
    for seed in 0..trials {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let e: Vec<f64> = (0..400).map(|_| 4.0 * std_normal(&mut rng)).collect();
        let u = vec![0.0; e.len()];
        let t: Vec<f64> = (0..e.len()).map(|k| k as f64 * 60.0).collect();
        let k = estimate_sise(&e, &u, &t, &Metric::Ksigma { k: 3.0 }, 60.0, 0.95).unwrap();
        assert_eq!(k.n_eff, 400);
        if k.lo <= 12.0 && 12.0 <= k.hi {
            cover += 1;
        }
        let p = estimate_sise(&e, &u, &t, &Metric::Percentile { p: 0.95 }, 60.0, 0.95).unwrap();
        assert!(p.method.contains("distribution-free"));
        if p.lo <= 4.0 * 1.959_964 && 4.0 * 1.959_964 <= p.hi {
            pcover += 1;
        }
    }
    let c = cover as f64 / trials as f64;
    let pc = pcover as f64 / trials as f64;
    assert!((0.92..=0.98).contains(&c), "k-sigma coverage {c}");
    assert!((0.90..=0.99).contains(&pc), "percentile coverage {pc}");
}

#[test]
fn d7_uee_is_removed_and_correlation_shrinks_n_eff() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(9);
    // SISE σ 3 m plus station UEE σ 2 m: the residual RMS is √13 ≈ 3.61 m; corrected → 3 m.
    let e: Vec<f64> = (0..20_000)
        .map(|_| 3.0 * std_normal(&mut rng) + 2.0 * std_normal(&mut rng))
        .collect();
    let u = vec![2.0; e.len()];
    let t: Vec<f64> = (0..e.len()).map(|k| k as f64 * 30.0).collect();
    let k = estimate_sise(&e, &u, &t, &Metric::Ksigma { k: 1.0 }, 30.0, 0.95).unwrap();
    assert!((k.value - 3.0).abs() < 0.06, "{}", k.value);
    let raw = estimate_sise(
        &e,
        &vec![0.0; e.len()],
        &t,
        &Metric::Ksigma { k: 1.0 },
        30.0,
        0.95,
    )
    .unwrap();
    assert!((raw.value - 13f64.sqrt()).abs() < 0.06);
    let p = estimate_sise(&e, &u, &t, &Metric::Percentile { p: 0.95 }, 30.0, 0.95).unwrap();
    assert!((p.value - 3.0 * 1.959_964).abs() < 0.25, "{}", p.value);
    // A one-hour decorrelation time leaves one independent sample per 120 and widens the interval.
    let k60 = estimate_sise(&e, &u, &t, &Metric::Ksigma { k: 1.0 }, 3600.0, 0.95).unwrap();
    assert_eq!(k60.n_eff, (20_000.0f64 * 30.0 / 3600.0).ceil() as usize);
    assert!(k60.hi - k60.lo > 5.0 * (k.hi - k.lo));
}

#[test]
fn d7_worst_window_finds_a_bad_day_the_monthly_figure_hides() {
    // A 3 m provider whose node degrades to 25 m for one day.
    let mut csv = String::from("time,station,provider,node,status,range_residual_m,uee_sigma_m\n");
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(4);
    for d in 1..=30 {
        for h in 0..24 {
            for m in (0..60).step_by(5) {
                let s = if d == 17 { 25.0 } else { 3.0 };
                let e = s * std_normal(&mut rng);
                csv.push_str(&format!(
                    "2029-06-{d:02}T{h:02}:{m:02}:00,S,P,N1,ok,{e:.3},0.3\n"
                ));
            }
        }
    }
    let mut month_sla = Sla::builtin("moonlight-owr").unwrap();
    month_sla.window_h = None;
    month_sla.id = "month".into();
    month_sla.alarm_limit_m = 1e3;
    let mut day_sla = Sla::builtin("moonlight-owr").unwrap();
    day_sla.alarm_limit_m = 1e3;
    let s = parse_samples(&csv).unwrap();
    let r = verify(
        &s,
        &[month_sla, day_sla],
        &Options {
            month: "2029-06".into(),
            decorrelation_s: 300.0,
            confidence: 0.95,
        },
        "x",
    )
    .unwrap();
    let res = &r.providers[0].results;
    assert_eq!(res[0].sise_verdict, Verdict::Meets, "{:?}", res[0].sise);
    assert_eq!(res[1].sise_verdict, Verdict::Breach, "{:?}", res[1].sise);
    let w = res[1].worst_window_start.as_ref().unwrap();
    assert!(
        w.starts_with("2029-06-16") || w.starts_with("2029-06-17"),
        "{w}"
    );
}

#[test]
fn d7_integrity_events_availability_and_continuity() {
    // One node every 60 s for 10 days; 5 outages of 10 minutes; 3 integrity events (100 m).
    let mut csv = String::from("time,station,provider,node,status,range_residual_m,uee_sigma_m\n");
    let mut n = 0;
    for d in 1..=10 {
        for h in 0..24 {
            for m in 0..60 {
                let k = ((d - 1) * 24 + h) * 60 + m;
                let t = format!("2029-07-{d:02}T{h:02}:{m:02}:00");
                let outage = [1000, 3000, 5000, 7000, 9000]
                    .iter()
                    .any(|s| k >= *s && k < s + 10);
                if outage {
                    csv.push_str(&format!("{t},S,P,N1,missing,,0.5\n"));
                } else if [200, 400, 600].contains(&k) {
                    csv.push_str(&format!("{t},S,P,N1,ok,100.0,0.5\n"));
                } else if k == 8000 {
                    csv.push_str(&format!("{t},S,P,N1,unhealthy,,0.5\n"));
                } else {
                    csv.push_str(&format!("{t},S,P,N1,ok,1.0,0.5\n"));
                }
                n += 1;
            }
        }
    }
    let mut sla = Sla::builtin("moonlight-owr").unwrap();
    sla.window_h = None;
    sla.continuity_target = Some(0.5);
    let r = month(&csv, "2029-07", &[sla]);
    let x = &r.providers[0].results[0];
    assert_eq!(x.integrity_events, 3);
    let unavailable = 5 * 10 + 3 + 1;
    assert!((x.availability - (n - unavailable) as f64 / n as f64).abs() < 1e-12);
    // Losses of service: 5 outages, 3 integrity events, 1 unhealthy epoch.
    assert_eq!(x.losses_of_service, 9);
    let hours = (n - unavailable) as f64 / 60.0;
    assert!((x.available_hours - hours).abs() < 1e-9);
    assert!((x.loss_rate_per_h - 9.0 / hours).abs() < 1e-12);
    assert!((x.continuity - (-9.0 / hours).exp()).abs() < 1e-12);
    assert!(x.continuity_interval[0] < x.continuity && x.continuity < x.continuity_interval[1]);
    assert_eq!(x.continuity_verdict, Verdict::Meets);
    // The longest daily run on day 1 is the whole day minus the event minutes (200, 400, 600, 1000).
    let d1 = &x.longest_run_h_per_day[0];
    assert_eq!(d1.0, "2029-07-01");
    assert!(d1.1 < 24.0 && d1.1 > 6.0);
}

#[test]
fn d7_inconclusive_near_the_target_with_little_data_and_month_filtering() {
    let mut csv = String::from("time,station,provider,node,status,range_residual_m,uee_sigma_m\n");
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(2);
    // A day of samples, true 3σ ≈ 13.5 m against 13.43 m, with a one-hour decorrelation.
    for h in 0..24 {
        for m in (0..60).step_by(10) {
            let e = 4.5 * std_normal(&mut rng);
            csv.push_str(&format!(
                "2029-08-01T{h:02}:{m:02}:00,S,P,N1,ok,{e:.3},0.2\n"
            ));
        }
    }
    csv.push_str("2029-09-01T00:00:00,S,P,N1,ok,1.0,0.2\n");
    let s = parse_samples(&csv).unwrap();
    let r = verify(
        &s,
        &[Sla::builtin("lcrns-srd").unwrap()],
        &Options {
            month: "2029-08".into(),
            decorrelation_s: 3600.0,
            confidence: 0.95,
        },
        "x",
    )
    .unwrap();
    assert_eq!(
        r.providers[0].results[0].sise_verdict,
        Verdict::Inconclusive
    );
    assert_eq!(r.samples_other_months, 1);
    assert!(verify(
        &s,
        &builtins(),
        &Options {
            month: "2030-01".into(),
            ..Options::default()
        },
        "x"
    )
    .is_err());
}

#[test]
fn d7_custom_sla_file_and_bad_input() {
    let f = "[[sla]]\nid = \"custom\"\nsource = \"contract annex B (illustrative)\"\nsise_target_m = 5.0\n\
             alarm_limit_m = 15.0\navailability_target = 0.99\nmin_nodes = 2\n\
             [sla.sise_metric]\nkind = \"percentile\"\np = 0.99\n";
    let v = parse_sla_file(f).unwrap();
    assert_eq!(v[0].sise_metric, Metric::Percentile { p: 0.99 });
    assert_eq!(v[0].min_nodes, 2);
    assert!(parse_sla_file(&f.replace("p = 0.99", "p = 1.5")).is_err());
    assert!(parse_sla_file(&f.replace("min_nodes = 2", "min_nodes = 0")).is_err());
    assert!(Sla::builtin("nope").is_err());
    let h = "time,station,provider,node,status,range_residual_m,uee_sigma_m\n";
    assert!(parse_samples(&format!("{h}2029-06-01T00:00:00,S,P,N1,ok,,0.5\n")).is_err());
    assert!(parse_samples(&format!("{h}2029-06-01T00:00:00,S,P,N1,maybe,1,0.5\n")).is_err());
    assert!(parse_samples(&format!("{h}2029-06-01T00:00:00,S,P,N1,ok,1\n")).is_err());
    assert!(parse_samples(h).is_err());
    // min_nodes = 2 with one node: never available.
    let csv = synthetic_csv(2029, 6, 3600.0, 0.5, &example_providers()[..1], 1);
    let mut one = example_providers()[0].clone();
    one.nodes = 1;
    let csv1 = synthetic_csv(2029, 6, 3600.0, 0.5, &[one], 1);
    let r = month(&csv1, "2029-06", &v);
    assert_eq!(r.providers[0].results[0].availability, 0.0);
    let r = month(&csv, "2029-06", &v);
    assert!(r.providers[0].results[0].availability > 0.9);
}

#[test]
fn d7_signed_monthly_report_verifies_and_tampering_is_detected() {
    let dir = std::env::temp_dir().join(format!("kshana-d7-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let csv = synthetic_csv(2029, 6, 900.0, 0.5, &example_providers(), 5);
    let r = month(&csv, "2029-06", &builtins());
    let out = dir.join("report");
    write_dir(&out, &r).unwrap();
    let seed = [9u8; 32];
    kshana_pro::signed_dossier::sign(&out, &seed, "Kshana Pro test", "2029-07-01T00:00:00Z")
        .unwrap();
    let pk = kshana_pro::license::public_key_for(&seed);
    let v = kshana_pro::signed_dossier::verify(&out, &[pk]).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    let p = out.join("report.txt");
    let t = std::fs::read_to_string(&p).unwrap();
    assert!(t.contains("BREACH-CO") && t.contains("illustrative"));
    std::fs::write(&p, t.replace("Breach", "Meets")).unwrap();
    let v = kshana_pro::signed_dossier::verify(&out, &[pk]).unwrap();
    assert!(!v.passed() && !v.problems.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
