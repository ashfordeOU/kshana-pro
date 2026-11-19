// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// D6: LuGRE C/N0 reproduction harness (part of the tests/lunar_pro.rs test binary).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana_pro::lunar_pro::lugre::*;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-d6-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A model with flat tables (EIRP 20 dBW, receive 10 dBi everywhere).
fn flat_model(dir: &Path, t_a: f64, nf: f64, loss: f64, impl_db: f64) -> Model {
    let tx = "angle_deg,eirp_dbw\n0,20\n180,20\n";
    let rx = "angle_deg,gain_dbi\n0,10\n180,10\n";
    std::fs::write(dir.join("tx.csv"), tx).unwrap();
    std::fs::write(dir.join("rx.csv"), rx).unwrap();
    let m = format!(
        "title = \"flat\"\n[receiver]\nboresight = \"earth\"\nantenna_pattern = \"rx.csv\"\n\
         antenna_pattern_sha256 = \"{}\"\nantenna_temp_k = {t_a}\nlna_nf_db = {nf}\npre_lna_loss_db = {loss}\n\
         implementation_loss_db = {impl_db}\n[receiver.site]\nkind = \"surface\"\nlat_deg = 0.0\nlon_deg = 0.0\n\
         [[transmit]]\nkey = \"G\"\nband = \"L1\"\nfile = \"tx.csv\"\nsha256 = \"{}\"\n[signals]\n\"G:1C\" = \"L1\"\n",
        sha256_hex(rx.as_bytes()),
        sha256_hex(tx.as_bytes())
    );
    std::fs::write(dir.join("model.toml"), m).unwrap();
    Model::load(&dir.join("model.toml")).unwrap()
}

fn fsl(r: f64, f: f64) -> f64 {
    20.0 * (4.0 * std::f64::consts::PI * r * f / 299_792_458.0).log10()
}

const FAR: [f64; 3] = [1e13, 0.0, 0.0];

#[test]
fn d6_system_temperature_follows_the_friis_cascade() {
    // No loss, NF 0 dB: T_sys is the antenna temperature.
    assert!((system_temperature_k(130.0, 0.0, 0.0) - 130.0).abs() < 1e-9);
    // NF 3.0103 dB (F = 2): adds T0.
    assert!((system_temperature_k(0.0, 10.0 * 2f64.log10(), 0.0) - 290.0).abs() < 1e-6);
    // Loss L ahead of a noiseless LNA: (L−1)·T0.
    let l = 10f64.powf(0.3 / 10.0);
    assert!((system_temperature_k(0.0, 0.0, 0.3) - (l - 1.0) * 290.0).abs() < 1e-9);
    // Full cascade by hand.
    let f = 10f64.powf(1.5 / 10.0);
    let want = 130.0 + (l - 1.0) * 290.0 + l * (f - 1.0) * 290.0;
    assert!((system_temperature_k(130.0, 1.5, 0.3) - want).abs() < 1e-9);
}

#[test]
fn d6_link_budget_matches_a_hand_computation_at_lunar_distance() {
    let d = tmp("hand");
    let m = flat_model(&d, 130.0, 1.5, 0.3, 1.5);
    // Free-space loss at the mean lunar distance on L1 is about 208.1 dB.
    assert!((fsl(384_400e3, 1_575.42e6) - 208.08).abs() < 0.02);
    let rx = [384_400e3, 0.0, 0.0];
    let tx = [0.0, 26_560e3, 0.0];
    let g = RxGeometry {
        rx,
        boresight: [-1.0, 0.0, 0.0],
        moon: FAR,
    };
    let p = m.predict_link(&g, tx, "G05", "1C", "t").unwrap();
    let range = (384_400e3f64.powi(2) + 26_560e3f64.powi(2)).sqrt();
    let want = 20.0 + 10.0 - fsl(range, 1_575.42e6) + 228.599_166_3
        - 10.0 * system_temperature_k(130.0, 1.5, 0.3).log10()
        - 1.5;
    let got = p.cn0_dbhz.unwrap();
    assert!((got - want).abs() < 1e-6, "{got} vs {want}");
    assert!((p.range_m - range).abs() < 1e-3);
    // Roughly 26 dB-Hz: the regime LuGRE operated in.
    assert!(got > 20.0 && got < 35.0, "{got}");
    // The transmit angle: from (0, 26560 km) toward the receiver, nadir is −y.
    let want_theta = (384_400e3f64 / 26_560e3).atan().to_degrees();
    assert!((p.theta_tx_deg - want_theta).abs() < 1e-6);
    // Unknown signal or band: no prediction.
    assert!(m.predict_link(&g, tx, "G05", "5Q", "t").is_none());
    assert!(m.predict_link(&g, tx, "E05", "1C", "t").is_none());
}

#[test]
fn d6_inverse_square_and_occultation() {
    let d = tmp("occ");
    let m = flat_model(&d, 100.0, 1.0, 0.0, 0.0);
    let tx = [0.0, 26_560e3, 0.0];
    let at = |x: f64| {
        let g = RxGeometry {
            rx: [x, 0.0, 0.0],
            boresight: [-1.0, 0.0, 0.0],
            moon: FAR,
        };
        m.predict_link(&g, tx, "G01", "1C", "t").unwrap()
    };
    // Range doubling costs 20 log10(2) = 6.02 dB (flat patterns, the range dominated by x).
    let (a, b) = (at(200_000e3), at(400_000e3));
    let dr = 20.0 * (b.range_m / a.range_m).log10();
    assert!((a.cn0_dbhz.unwrap() - b.cn0_dbhz.unwrap() - dr).abs() < 1e-9);
    // A satellite behind the Earth is occulted.
    let g = RxGeometry {
        rx: [384_400e3, 0.0, 0.0],
        boresight: [-1.0, 0.0, 0.0],
        moon: FAR,
    };
    let p = m
        .predict_link(&g, [-26_560e3, 0.0, 0.0], "G01", "1C", "t")
        .unwrap();
    assert!(p.occulted_earth && p.cn0_dbhz.is_none());
    // The Moon blocks a receiver on its far side.
    let moon = [384_400e3, 0.0, 0.0];
    let g = RxGeometry {
        rx: [384_400e3 + 1_737_400.0, 0.0, 0.0],
        boresight: [-1.0, 0.0, 0.0],
        moon,
    };
    let p = m.predict_link(&g, tx, "G01", "1C", "t").unwrap();
    assert!(p.occulted_moon && p.cn0_dbhz.is_none());
    // ...but not one on the near side, on the surface itself.
    let g = RxGeometry {
        rx: [384_400e3 - 1_737_400.0, 0.0, 0.0],
        boresight: [-1.0, 0.0, 0.0],
        moon,
    };
    let p = m.predict_link(&g, tx, "G01", "1C", "t").unwrap();
    assert!(!p.occulted_moon && p.cn0_dbhz.is_some());
}

#[test]
fn d6_surface_geometry_puts_the_receiver_on_the_moon_and_the_earth_in_view() {
    let d = tmp("geo");
    let m = flat_model(&d, 100.0, 1.0, 0.0, 0.0);
    let t = parse_epoch("2025-03-03T12:00:00").unwrap();
    let g = m.geometry(&t).unwrap();
    let r = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let off = [
        g.rx[0] - g.moon[0],
        g.rx[1] - g.moon[1],
        g.rx[2] - g.moon[2],
    ];
    assert!((r(off) - R_MOON_M).abs() < 1.0, "{}", r(off));
    assert!(r(g.moon) > 356_000e3 && r(g.moon) < 407_000e3);
    // (0°, 0°) PA faces the Earth: the site is on the Earthward side (within ~10° libration).
    let cosang =
        -(off[0] * g.moon[0] + off[1] * g.moon[1] + off[2] * g.moon[2]) / (r(off) * r(g.moon));
    assert!(
        cosang > 10f64.to_radians().cos(),
        "{}",
        cosang.acos().to_degrees()
    );
}

#[test]
fn d6_type_b_pattern_term_is_correlated_within_a_pattern_only() {
    let mk = |sat: &str, pat: &str, v: f64| Residual {
        time: "t".into(),
        sat: sat.into(),
        signal: "1C".into(),
        pattern: pat.into(),
        observed_dbhz: 0.0,
        predicted_dbhz: 0.0,
        residual_db: v,
        slope_db_per_deg: 0.0,
    };
    let rs: Vec<Residual> = ["G01", "G02", "G03", "G04"]
        .iter()
        .enumerate()
        .map(|(i, s)| {
            mk(
                s,
                if i < 2 { "A/L1" } else { "B/L1" },
                -9.0 + i as f64 * 0.1,
            )
        })
        .collect();
    let a = Analysis {
        signals: vec!["G:1C".into()],
        min_observed_dbhz: None,
        min_cluster_obs: 1,
        coverage_k: 2.0,
        bootstrap_n: 100,
        bootstrap_seed: 1,
        published_shortfall_db: PUBLISHED_SHORTFALL_DB,
    };
    let mut inp = TypeBInputs::none();
    inp.pattern_u_db = BTreeMap::from([("A/L1".into(), 2.0), ("B/L1".into(), 2.0)]);
    let (s, per, used, dropped) = score_residuals(&rs, &a, &inp);
    // Two independent patterns, each on half the clusters: sqrt(2·(0.5·2)²) = √2.
    assert!((s.u_b_db - 2f64.sqrt()).abs() < 1e-12, "{}", s.u_b_db);
    assert_eq!((used, dropped, per.len()), (4, 0, 1));
    assert!((s.bias_db + 8.85).abs() < 1e-12 && (s.shortfall_db - 8.85).abs() < 1e-12);
    // One shared pattern: fully correlated, u_B = 2.
    inp.pattern_u_db.insert("A/L1".into(), 2.0);
    let rs2: Vec<Residual> = rs
        .iter()
        .map(|r| Residual {
            pattern: "A/L1".into(),
            ..r.clone()
        })
        .collect();
    let (s2, ..) = score_residuals(&rs2, &a, &inp);
    assert!((s2.u_b_db - 2.0).abs() < 1e-12);
}

fn synthetic(tag: &str, bias: f64, seed: u64) -> (PathBuf, SyntheticTruth) {
    let d = tmp(tag);
    let t = write_synthetic(&d, bias, seed).unwrap();
    (d, t)
}

fn inputs<'a>(d: &'a Path, sha: &'a str) -> (PathBuf, PathBuf, PathBuf, PathBuf, &'a str) {
    (
        d.join("prereg.toml"),
        d.join("model.toml"),
        d.join("gnss.sp3"),
        d.join("obs.csv"),
        sha,
    )
}

fn run_score(d: &Path, sha: &str) -> Result<(ScoreReport, Vec<Residual>), String> {
    let (p, m, s, o, sha) = inputs(d, sha);
    score(&ScoreInputs {
        prereg: &p,
        prereg_sha256: sha,
        model: &m,
        sp3: &s,
        observations: &o,
    })
}

#[test]
fn d6_recovers_an_injected_shortfall_and_scores_the_bars() {
    let (d, t) = synthetic("rec", -9.5, 11);
    let (r, res) = run_score(&d, &t.prereg_sha256).unwrap();
    let o = &r.overall;
    // Every observation was predicted (they were generated from the model).
    assert_eq!(r.counts.matched, r.counts.observations);
    assert_eq!(r.counts.no_prediction, 0);
    assert!(o.n_clusters >= 8, "{} clusters", o.n_clusters);
    assert!(res.len() > 200, "{}", res.len());
    // The shortfall comes back within 3 u_A of the truth (the model is perfect here).
    assert!(
        (o.shortfall_db - 9.5).abs() < 3.0 * o.u_a_db,
        "{} ± {}",
        o.shortfall_db,
        o.u_a_db
    );
    // The Type B budget is the model's: pattern 1.5 dB shared within each of 4 tables,
    // receive gain 0.5, implementation 0.5, noise terms: about 1 dB.
    assert!(o.u_b_db > 0.7 && o.u_b_db < 1.6, "{}", o.u_b_db);
    assert!(o.overlap_published && o.shortfall_lo_db > 0.0);
    // The bootstrap interval brackets the estimate.
    assert!(o.bootstrap_95_db[0] < o.shortfall_db && o.shortfall_db < o.bootstrap_95_db[1]);
    // Each of the four signals has its own statistic, each consistent with the truth.
    assert_eq!(r.per_signal.len(), 4);
    for s in &r.per_signal {
        assert!(
            (s.shortfall_db - 9.5).abs() <= s.k * s.u_total_db + 1e-9,
            "{}: {}",
            s.signal,
            s.shortfall_db
        );
    }
    assert!(r.all_bars_met, "{:#?}", r.bars);
    // Writing the report directory works and the text names the published source.
    let out = d.join("score");
    write_dir(&out, &r, &res).unwrap();
    let txt = std::fs::read_to_string(out.join("report.txt")).unwrap();
    assert!(txt.contains("navi.756") && txt.contains("MODELLED"));
}

#[test]
fn d6_no_injected_bias_fails_the_detection_and_published_bars() {
    let (d, t) = synthetic("zero", 0.0, 12);
    let (r, _) = run_score(&d, &t.prereg_sha256).unwrap();
    let o = &r.overall;
    assert!(o.shortfall_db.abs() < 3.0 * o.u_a_db, "{}", o.shortfall_db);
    assert!(!o.overlap_published);
    let met: BTreeMap<_, _> = r.bars.iter().map(|b| (b.id.as_str(), b.met)).collect();
    assert!(met["B1-sufficiency"] && met["B2-precision"]);
    assert!(!met["B3-shortfall-detected"] && !met["B4-consistent-with-published"]);
    assert!(!r.all_bars_met);
}

#[test]
fn d6_type_a_interval_covers_the_truth_at_the_stated_rate() {
    // Monte Carlo over 300 synthetic campaigns on the same geometry: the k = 2 cluster interval
    // (Type A only, the model being perfect) must cover the injected bias about 95 % of the time.
    use rand::SeedableRng;
    let (d, _) = synthetic("cov", -9.5, 1);
    let m = Model::load(&d.join("model.toml")).unwrap();
    let src = Sp3Source::parse(&std::fs::read_to_string(d.join("gnss.sp3")).unwrap()).unwrap();
    let preds = predict_series(
        &m,
        &src,
        &parse_epoch("2025-03-03T00:00:00").unwrap(),
        &parse_epoch("2025-03-04T00:00:00").unwrap(),
        1800.0,
    )
    .unwrap();
    let base: Vec<Residual> = preds
        .iter()
        .filter(|p| p.cn0_dbhz.is_some_and(|c| c - 9.5 >= 15.0))
        .map(|p| Residual {
            time: p.time.clone(),
            sat: p.sat.clone(),
            signal: p.signal.clone(),
            pattern: p.pattern.clone(),
            observed_dbhz: 0.0,
            predicted_dbhz: p.cn0_dbhz.unwrap(),
            residual_db: 0.0,
            slope_db_per_deg: p.slope_db_per_deg,
        })
        .collect();
    let a = Analysis {
        signals: vec!["G:1C".into(), "G:5Q".into(), "E:1C".into(), "E:5Q".into()],
        min_observed_dbhz: None,
        min_cluster_obs: 3,
        coverage_k: 2.0,
        bootstrap_n: 200,
        bootstrap_seed: 3,
        published_shortfall_db: PUBLISHED_SHORTFALL_DB,
    };
    let trials = 300;
    let mut cover = 0;
    let mut boot_cover = 0;
    let mut j = 0;
    for seed in 0..trials {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let mut off = BTreeMap::new();
        let rs: Vec<Residual> = base
            .iter()
            .map(|r| {
                let o = *off
                    .entry(format!("{}{}", r.sat, r.signal))
                    .or_insert_with(|| 0.8 * kshana_pro::lunar_pro::budget::std_normal(&mut rng));
                let v = -9.5 + o + 0.5 * kshana_pro::lunar_pro::budget::std_normal(&mut rng);
                Residual {
                    residual_db: v,
                    ..r.clone()
                }
            })
            .collect();
        let (s, ..) = score_residuals(&rs, &a, &TypeBInputs::none());
        j = s.n_clusters;
        if (s.shortfall_lo_db..=s.shortfall_hi_db).contains(&9.5) {
            cover += 1;
        }
        if (s.bootstrap_95_db[0]..=s.bootstrap_95_db[1]).contains(&9.5) {
            boot_cover += 1;
        }
    }
    let c = cover as f64 / trials as f64;
    let b = boot_cover as f64 / trials as f64;
    assert!(j >= 8, "{j} clusters");
    assert!(
        (0.89..=0.99).contains(&c),
        "k=2 coverage {c} with {j} clusters"
    );
    // The percentile bootstrap undercovers somewhat at small cluster counts; it must not collapse.
    assert!((0.80..=0.99).contains(&b), "bootstrap coverage {b}");
}

#[test]
fn d6_pre_registration_refuses_every_tamper() {
    let (d, t) = synthetic("tamper", -9.5, 5);
    // A wrong published hash.
    let e = run_score(&d, &"0".repeat(64)).unwrap_err();
    assert!(e.contains("not the published"), "{e}");
    // The pre-registration edited after its hash was published.
    let pre = d.join("prereg.toml");
    let orig = std::fs::read_to_string(&pre).unwrap();
    std::fs::write(&pre, orig.replace("value = 4.0", "value = 1.0")).unwrap();
    assert!(run_score(&d, &t.prereg_sha256)
        .unwrap_err()
        .contains("not the published"));
    // A pre-registration that admits the data were seen is refused even with its own hash.
    let seen = orig.replace("data_seen = false", "data_seen = true");
    std::fs::write(&pre, &seen).unwrap();
    let e = run_score(&d, &sha256_hex(seen.as_bytes())).unwrap_err();
    assert!(e.contains("data_seen"), "{e}");
    std::fs::write(&pre, &orig).unwrap();
    assert!(run_score(&d, &t.prereg_sha256).is_ok());
    // The model edited after registration.
    let mp = d.join("model.toml");
    let model = std::fs::read_to_string(&mp).unwrap();
    std::fs::write(&mp, model.replace("lna_nf_db = 1.5", "lna_nf_db = 0.5")).unwrap();
    let e = run_score(&d, &t.prereg_sha256).unwrap_err();
    assert!(e.contains("model changed"), "{e}");
    std::fs::write(&mp, &model).unwrap();
    // A pattern table edited behind an unchanged model file.
    let tp = d.join("gps_l1_eirp.csv");
    let tab = std::fs::read_to_string(&tp).unwrap();
    std::fs::write(&tp, tab.replace("\n13,27.00", "\n13,37.00")).unwrap();
    let e = run_score(&d, &t.prereg_sha256).unwrap_err();
    assert!(e.contains("table changed"), "{e}");
    std::fs::write(&tp, &tab).unwrap();
    assert!(run_score(&d, &t.prereg_sha256).is_ok());
}

#[test]
fn d6_observation_reader_and_exclusions() {
    let (d, t) = synthetic("excl", -9.5, 9);
    let op = d.join("obs.csv");
    let mut obs = std::fs::read_to_string(&op).unwrap();
    let parsed = read_observations(&obs).unwrap();
    let lines = obs.lines().filter(|l| l.starts_with("2025")).count();
    assert_eq!(parsed.len(), lines);
    assert!(parsed
        .iter()
        .all(|o| o.cn0_dbhz.is_finite() && o.cn0_dbhz >= 10.0));
    // An unregistered signal and a satellite with no ephemeris are counted, not scored.
    obs.push_str("2025-03-03T00:00:00,G01,2W,,,,30.0\n2025-03-03T00:00:00,G99,1C,,,,30.0\n");
    std::fs::write(&op, obs).unwrap();
    let (r, _) = run_score(&d, &t.prereg_sha256).unwrap();
    assert_eq!(r.counts.excluded_signal, 1);
    assert_eq!(r.counts.no_prediction, 1);
    assert_eq!(r.counts.matched + 2, r.counts.observations);
    // Malformed observations are refused.
    std::fs::write(&op, "time,sv,signal\n2025-03-03T00:00:00,G01\n").unwrap();
    assert!(run_score(&d, &t.prereg_sha256).is_err());
}

#[test]
fn d6_pre_registration_template_round_trips_and_validates() {
    let p = PreRegistration::template(
        &"a".repeat(64),
        "t",
        "r",
        "2026-11-19T00:00:00Z",
        vec!["G:1C".into()],
    );
    let text = p.to_toml().unwrap();
    assert_eq!(PreRegistration::parse(&text).unwrap(), p);
    assert!(PreRegistration::parse(&text.replace("op = \"ge\"", "op = \"gt\"")).is_err());
    assert!(PreRegistration::parse(
        &text.replace("quantity = \"n_clusters\"", "quantity = \"luck\"")
    )
    .is_err());
    assert!(PreRegistration::parse(&text.replace(PREREG_FORMAT, "other/1")).is_err());
}

#[test]
fn d6_epoch_seconds_round_trip_across_midnight_and_month_ends() {
    for s in [
        "2025-03-03T00:00:00",
        "2025-03-02T23:59:59.5",
        "2029-06-30T12:00:00",
        "2029-07-01T00:00:00",
        "2000-01-01T00:00:00",
        "2024-02-29T06:30:15.25",
    ] {
        let e = parse_epoch(s).unwrap();
        let t = kshana_pro::lunar_pro::lunex::secs(&e);
        let back = epoch_from_secs(t);
        assert_eq!(
            (back.year, back.month, back.day, back.hour, back.minute),
            (e.year, e.month, e.day, e.hour, e.minute),
            "{s}"
        );
        assert!((back.second - e.second).abs() < 1e-6, "{s}");
    }
}
