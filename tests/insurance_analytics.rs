// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Insurance and finance analytics (`kshana_pro::insurance`, docs/INSURANCE-ANALYTICS.md):
//! the parametric trigger on synthetic station series (k-of-n rule, duration threshold,
//! missing data, payout tiers, backtest), basis risk worked by hand, accumulation of the
//! example portfolio by hand, the five BI models and their sensitivity tables, the sample
//! underwriting feed against its schema, claims triage with a signed cause verdict, the licence
//! gates and the command line. All inputs are synthetic.

use std::path::{Path, PathBuf};
use std::process::Command;

use ed25519_dalek::{Signer, SigningKey};
use kshana_pro::insurance::{self, basis, bi, feed, trigger};
use kshana_pro::license::{self, Entitlement, Tier};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");
const SEED: [u8; 32] = [7u8; 32];
const UTC: &str = "2026-11-20T12:00:00Z";
const EXAMPLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/insurance");

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-insurance-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn t(s: &str) -> f64 {
    insurance::utc("", s).unwrap()
}

/// Three sites at 45 dB-Hz, one sample a minute over two days. A and B drop 10 dB together for
/// two hours on day 1 (10:00-12:00); A alone for 40 min on day 2 (03:00); A and B together for
/// 10 min on day 2 (15:00); C has a three-hour gap on day 1 and is never degraded.
fn station_csv() -> String {
    let t0 = t("2026-09-01T00:00:00Z");
    let mut s = String::from("utc,site,cn0_dbhz\n");
    for m in 0..(2 * 1440) {
        let tt = t0 + m as f64 * 60.0;
        let h = (m % 1440) as f64 / 60.0;
        let day2 = m >= 1440;
        for site in ["A", "B", "C"] {
            let mut v = 45.0;
            let joint = (!day2 && (10.0..12.0).contains(&h))
                || (day2 && (15.0..15.0 + 10.0 / 60.0).contains(&h));
            let alone = day2 && (3.0..3.0 + 40.0 / 60.0).contains(&h);
            if (site != "C" && joint) || (site == "A" && alone) {
                v = 35.0;
            }
            if site == "C" && !day2 && (5.0..8.0).contains(&h) {
                continue;
            }
            s.push_str(&format!("{},{site},{v}\n", insurance::iso(tt)));
        }
    }
    s
}

const TRIGGER_TOML: &str = r#"
title = "Synthetic parametric trigger"
period_start_utc = "2026-09-01T00:00:00Z"
period_end_utc = "2026-09-03T00:00:00Z"
series = ["stations.csv"]

[[sites]]
id = "A"
lat_deg = 59.4
lon_deg = 24.7
[[sites]]
id = "B"
lat_deg = 59.5
lon_deg = 24.9
[[sites]]
id = "C"
lat_deg = 58.4
lon_deg = 26.7
baseline_dbhz = 45.0

[trigger]
cn0_drop_db = 6.0
min_sites = 2
min_duration_min = 30.0
merge_gap_min = 5.0

[cover]
name = "Synthetic cover (illustrative)"
currency = "EUR"
synthetic = true
tiers = [{ min_hours = 0.5, amount = 10000.0 }, { min_hours = 2.0, amount = 50000.0 }]
aggregate_limit = 100000.0

[[backtest]]
id = "ARCH-1"
start_utc = "2026-09-01T10:00:00Z"
end_utc = "2026-09-01T12:00:00Z"
basis = "SYNTHETIC archived event: two-site outage"
[[backtest]]
id = "ARCH-2"
start_utc = "2026-09-02T03:00:00Z"
end_utc = "2026-09-02T03:40:00Z"
basis = "SYNTHETIC archived event: single-site outage"
"#;

fn trigger_dir(tag: &str) -> PathBuf {
    let w = tmp(tag);
    std::fs::write(w.join("stations.csv"), station_csv()).unwrap();
    std::fs::write(w.join("trigger.toml"), TRIGGER_TOML).unwrap();
    w
}

fn evaluate(w: &Path) -> trigger::TriggerResult {
    let (q, _, base) =
        insurance::read_request::<trigger::TriggerRequest>(&w.join("trigger.toml")).unwrap();
    trigger::evaluate(&q, &base).unwrap()
}

#[test]
fn trigger_applies_the_k_of_n_rule_duration_and_tiers() {
    let w = trigger_dir("trig-k");
    let r = evaluate(&w);
    assert!(r.label.starts_with("MODELLED"));
    // Two joint episodes form trigger events; A alone does not (min_sites = 2).
    assert_eq!(r.events.len(), 2, "{:#?}", r.events);
    let (e1, e2) = (&r.events[0], &r.events[1]);
    assert_eq!(e1.start_utc, "2026-09-01T10:00:00Z");
    assert!((e1.duration_h - 2.0).abs() < 1e-9, "{}", e1.duration_h);
    assert!(e1.fired && e1.payout == 50_000.0);
    assert_eq!(e1.sites, vec!["A", "B"]);
    // 10 dB × 2 h × 2 sites.
    assert!((e1.index_db_h - 40.0).abs() < 1e-6, "{}", e1.index_db_h);
    assert!(!e2.fired && e2.payout == 0.0 && (e2.duration_h * 60.0 - 10.0).abs() < 1e-6);
    assert_eq!(r.totals.fired, 1);
    assert_eq!(r.totals.total_payout, 50_000.0);
    // Site baselines: median for A and B, stated for C; C's gap is missing data, not outage.
    let c = r.sites.iter().find(|s| s.id == "C").unwrap();
    assert_eq!(c.baseline_source, "stated");
    assert_eq!(c.episodes, 0);
    assert!(
        (c.observed_h - (48.0 - 3.0)).abs() < 0.05,
        "{}",
        c.observed_h
    );
    let a = r.sites.iter().find(|s| s.id == "A").unwrap();
    assert_eq!((a.baseline_dbhz, a.episodes), (45.0, 3));
    // Backtest: the two-site event is caught, the single-site one is not.
    assert!(r.backtest[0].fired && !r.backtest[1].fired);
    assert_eq!(r.totals.archived_hit, 1);
    assert_eq!(r.totals.fired_unmatched, 0);
    assert!(r.notes.iter().any(|n| n.contains("SYNTHETIC")));
    // Reproducible: same inputs, same fingerprint.
    assert_eq!(r.fingerprint, evaluate(&w).fingerprint);
}

#[test]
fn single_site_trigger_and_aggregate_limit() {
    let w = trigger_dir("trig-one");
    let src = TRIGGER_TOML
        .replace("min_sites = 2", "min_sites = 1")
        .replace("aggregate_limit = 100000.0", "aggregate_limit = 55000.0");
    std::fs::write(w.join("trigger.toml"), src).unwrap();
    let r = evaluate(&w);
    // A alone (40 min) now fires at the first tier, capped by what is left of the aggregate.
    let fired: Vec<_> = r.events.iter().filter(|e| e.fired).collect();
    assert_eq!(fired.len(), 2);
    assert_eq!(fired[1].payout, 5_000.0);
    assert_eq!(r.totals.total_payout, 55_000.0);
    assert!(r.backtest.iter().all(|b| b.fired));
}

const LOSSES: &str = "insured,start_utc,end_utc,loss,lat,lon
X,2026-09-01T10:30:00Z,2026-09-01T13:00:00Z,40000,59.45,24.75
Y,2026-09-02T03:00:00Z,2026-09-02T04:00:00Z,5000,59.40,24.70
Z,2026-09-02T15:05:00Z,2026-09-02T15:30:00Z,1000,61.50,24.70
";

#[test]
fn basis_risk_worked_by_hand() {
    let w = trigger_dir("basis");
    std::fs::write(w.join("losses.csv"), LOSSES).unwrap();
    std::fs::write(
        w.join("basis.toml"),
        "title = \"Basis\"\ntrigger = \"trigger.toml\"\nlosses = [\"losses.csv\"]\nmatch_tolerance_h = 1.0\nbootstrap = 500\nseed = 3\nmax_site_distance_km = 100\n",
    )
    .unwrap();
    let (r, _) = basis::basis(&w.join("basis.toml")).unwrap();
    assert!(r.label.starts_with("MODELLED"));
    let m = &r.metrics;
    // Clusters: (P 50 000, L 40 000), (0, 5 000), (0, 1 000).
    assert_eq!(r.clusters.len(), 3, "{:#?}", r.clusters);
    assert_eq!(m.excess, 10_000.0);
    assert_eq!(m.shortfall, 6_000.0);
    assert!((m.basis_risk_ratio.unwrap() - 16_000.0 / 46_000.0).abs() < 1e-6);
    assert!((m.hit_rate.unwrap() - 1.0 / 3.0).abs() < 1e-6);
    assert_eq!(m.payout_without_loss_rate, Some(0.0));
    let ci = m.basis_risk_ratio_90.unwrap();
    assert!(ci[0] <= ci[1]);
    // Z's loss overlaps the 10-minute event below the duration threshold.
    assert!(r.clusters[2].below_duration_event && !r.clusters[1].below_duration_event);
    // Location basis: Z is ~230 km from the nearest site, 1 000 of 46 000 beyond 100 km.
    let z = r.location.iter().find(|l| l.insured == "Z").unwrap();
    assert!(z.distance_km > 200.0 && !z.paid);
    assert!(r.location.iter().find(|l| l.insured == "X").unwrap().paid);
    assert!((m.loss_beyond_distance_share.unwrap() - 1_000.0 / 46_000.0).abs() < 1e-6);
    // Seeded: reproducible.
    let (r2, _) = basis::basis(&w.join("basis.toml")).unwrap();
    assert_eq!(r.fingerprint, r2.fingerprint);
}

#[test]
fn accumulation_of_the_example_portfolio_by_hand() {
    let (r, _) = basis::accumulation(&Path::new(EXAMPLES).join("accumulation.toml")).unwrap();
    assert!(r.label.starts_with("MODELLED"));
    let s1 = &r.footprints[0];
    // Two ports inside 60 km for 24 h at 0.2 %/h: 2e6·0.8·0.048 + 1.5e6·0.6·0.048.
    assert_eq!(s1.insureds, 2);
    assert!(
        (s1.modelled_loss - 120_000.0).abs() < 1e-3,
        "{}",
        s1.modelled_loss
    );
    // All four inside 400 km for 72 h, capped at 10 %: 0.1 · Σ exposure × dependency.
    let s2 = &r.footprints[1];
    assert_eq!(s2.insureds, 4);
    assert!((s2.modelled_loss - 480_000.0).abs() < 1e-3);
    assert_eq!(r.totals.largest_scenario_loss, 480_000.0);
    assert_eq!(r.totals.historical_events, 0);
    // Cells: the two ports share the 54/18 cell, 3.5e6 of 8.8e6.
    assert_eq!(r.cells[0].cell, [52.0, 21.0]);
    let port_cell = r.cells.iter().find(|c| c.cell == [54.0, 18.0]).unwrap();
    assert_eq!(port_cell.insureds, 2);
    assert!((port_cell.share - 3.5 / 8.8).abs() < 1e-6);
    let h: f64 = r.cells.iter().map(|c| c.share * c.share).sum();
    assert!((r.totals.herfindahl - h).abs() < 1e-6);
}

#[test]
fn accumulation_under_historical_footprints_and_exceedance() {
    let w = tmp("acc");
    std::fs::copy(
        Path::new(EXAMPLES).join("portfolio.csv"),
        w.join("portfolio.csv"),
    )
    .unwrap();
    let (_, _, events, _) = feed::sample();
    // Two synthetic events far from the portfolio, one over Gdansk Bay for 10 h.
    let mut ev: Vec<serde_json::Value> = serde_json::from_str(&events).unwrap();
    let mut g = ev[0].clone();
    g["id"] = "SYN-G".into();
    g["start_utc"] = "2026-03-01T00:00:00Z".into();
    g["end_utc"] = "2026-03-01T10:00:00Z".into();
    g["location"] = serde_json::json!({"lat_deg": 54.45, "lon_deg": 18.6, "radius_m": 50000.0});
    ev[0]["start_utc"] = "2026-10-01T06:00:00Z".into();
    ev.push(g);
    ev.push(serde_json::json!({"id": "SYN-NOLOC", "start_utc": "2026-05-01T00:00:00Z",
        "end_utc": "2026-05-01T01:00:00Z", "systems": [{"system": "gnss", "band": "L1"}],
        "kind": "jamming", "area": "Synthetic named area", "observers": ["x"], "confidence": 0.5, "basis": "SYNTHETIC"}));
    std::fs::write(w.join("events.json"), serde_json::to_string(&ev).unwrap()).unwrap();
    std::fs::write(
        w.join("acc.toml"),
        "title = \"Acc\"\ncurrency = \"EUR\"\nportfolio = \"portfolio.csv\"\nperiod_start_utc = \"2026-01-01T00:00:00Z\"\nperiod_end_utc = \"2027-01-01T00:00:00Z\"\nevents = [\"events.json\"]\nloss_ratio_per_hour = 0.002\nmax_loss_ratio = 0.1\n",
    )
    .unwrap();
    let (r, _) = basis::accumulation(&w.join("acc.toml")).unwrap();
    assert_eq!(r.totals.historical_events, 3);
    assert_eq!(r.totals.events_without_location, 1);
    // 10 h at 0.2 %/h on the two ports: (1.6e6 + 0.9e6) · 0.02 = 50 000.
    assert!((r.totals.largest_event_loss - 50_000.0).abs() < 1e-3);
    assert!((r.totals.period_aggregate_loss - 50_000.0).abs() < 1e-3);
    assert_eq!(r.exceedance[0].loss, r.totals.largest_event_loss);
    assert!((r.exceedance[0].empirical_return_period_y - 365.0 / 365.25).abs() < 1e-6);
}

fn bi_of(model: &str, params: &[(&str, f64)], extra: &str, dir: &Path) -> bi::BiResult {
    let mut src =
        format!("title = \"t\"\nmodel = \"{model}\"\ncurrency = \"EUR\"\n{extra}\n[params]\n");
    for (k, v) in params {
        src.push_str(&format!("{k} = {v}\n"));
    }
    let p = dir.join(format!("{model}.toml"));
    std::fs::write(&p, src).unwrap();
    bi::run(&p).unwrap().0
}

#[test]
fn bi_models_by_hand_with_sensitivity() {
    let w = tmp("bi");
    // Timing: the SVN-23 example: 12 h above 1.5 µs.
    let (r, _) = bi::run(&Path::new(EXAMPLES).join("bi-timing-svn23.toml")).unwrap();
    assert!(r.label.starts_with("MODELLED"));
    assert_eq!(r.parts["revenue_lost"], 16_800.0);
    assert_eq!(r.headline, 16_800.0 + 25_000.0 + 1_600.0);
    assert!(r.notes.iter().any(|n| n.contains("SVN-23")));
    let oh = r.params.iter().find(|p| p.name == "outage_h").unwrap();
    assert!(oh.value == 12.0 && oh.source.starts_with("preset"));
    // Sensitivity: sorted by swing; a linear term swings by 2 × 20 % of its contribution.
    assert!(r.sensitivity.windows(2).all(|x| x[0].swing >= x[1].swing));
    let pen = r
        .sensitivity
        .iter()
        .find(|s| s.param == "penalty_per_breach")
        .unwrap();
    assert!((pen.swing - 10_000.0).abs() < 1e-6);
    // A tolerance above the step: no outage.
    let r = bi_of(
        "timing",
        &[("tolerance_ns", 20_000.0), ("revenue_per_hour", 1000.0)],
        "preset = \"svn23\"",
        &w,
    );
    assert_eq!(r.parts["revenue_lost"], 0.0);
    // Timing from a time-error log: 3 samples a minute apart above tolerance = 3 min, 1 breach.
    let mut te = String::from("utc,time_error_ns\n");
    for i in 0..10 {
        let v = if (3..6).contains(&i) { 2500 } else { 100 };
        te.push_str(&format!("2026-01-26T00:{i:02}:00Z,{v}\n"));
    }
    std::fs::write(w.join("te.csv"), te).unwrap();
    let r = bi_of(
        "timing",
        &[("tolerance_ns", 1500.0), ("revenue_per_hour", 6000.0)],
        "te_log = \"te.csv\"",
        &w,
    );
    assert!((r.headline - 300.0).abs() < 1e-6, "{}", r.headline);

    // Yield: 5 h outage, 8 ha/h → 40 ha delayed, 0.5 day, 1 %/day, 8 t/ha, 200/t.
    let r = bi_of(
        "yield",
        &[
            ("area_ha", 100.0),
            ("outage_h", 5.0),
            ("operating_rate_ha_per_h", 8.0),
            ("yield_t_per_ha", 8.0),
            ("price_per_t", 200.0),
            ("yield_loss_pct_per_day", 1.0),
            ("idle_cost_per_hour", 50.0),
        ],
        "",
        &w,
    );
    assert!((r.parts["yield_loss"] - 40.0 * 8.0 * 200.0 * 0.005).abs() < 1e-6);
    assert!((r.headline - (320.0 + 250.0)).abs() < 1e-6);

    // Port: delays from a vessel file (6 + 18 h) at 24 000/day, berth loss 1 000/h for 4 h.
    std::fs::write(w.join("vessels.csv"), "vessel,delay_h\nV1,6\nV2,18\n").unwrap();
    let r = bi_of(
        "port",
        &[
            ("outage_h", 4.0),
            ("demurrage_per_day", 24_000.0),
            ("berth_loss_per_hour", 1000.0),
        ],
        "vessels = \"vessels.csv\"",
        &w,
    );
    assert_eq!(r.headline, 24_000.0 + 4_000.0);

    // Grounding: 10 000 t at 10 kn: ½ · 1e7 kg · (5.14444 m/s)² = 132.3 MJ.
    let r = bi_of(
        "grounding",
        &[
            ("displacement_t", 10_000.0),
            ("speed_kn", 10.0),
            ("bottom_factor", 0.5),
            ("cost_per_mj", 1000.0),
        ],
        "",
        &w,
    );
    assert!((r.parts["kinetic_energy_mj"] - 132.326).abs() < 0.01);
    assert_eq!(r.parts["severity_band"], 3.0);

    // Cascade: the example; correlation fattens the tail against independence.
    let (c, _) = bi::run(&Path::new(EXAMPLES).join("bi-cascade.toml")).unwrap();
    let ind = bi_of(
        "cascade",
        &[
            ("fleet_size", 40.0),
            ("p_unit", 0.05),
            ("loss_per_unit", 150_000.0),
            ("threshold_units", 3.0),
        ],
        "",
        &w,
    );
    assert_eq!(c.parts["expected_units"], 2.0);
    assert!(c.headline > ind.headline);
    assert_eq!(c.headline, c.parts["units_p99"] * 150_000.0);

    // Unknown parameters and missing required ones are refused.
    std::fs::write(
        w.join("bad.toml"),
        "title = \"t\"\nmodel = \"port\"\ncurrency = \"EUR\"\n[params]\nwhatever = 1\n",
    )
    .unwrap();
    assert!(bi::run(&w.join("bad.toml"))
        .unwrap_err()
        .contains("unknown parameter"));
}

fn sample_dir(tag: &str) -> PathBuf {
    let w = tmp(tag);
    let (req, tracks, events, areas) = feed::sample();
    for (n, s) in [
        ("request.toml", req),
        ("tracks.csv", tracks),
        ("events.json", events),
        ("areas.toml", areas),
    ] {
        std::fs::write(w.join(n), s).unwrap();
    }
    w
}

#[test]
fn sample_feed_matches_its_schema_and_the_synthetic_story() {
    let w = sample_dir("feed");
    let (q, _, base) =
        insurance::read_request::<feed::FeedRequest>(&w.join("request.toml")).unwrap();
    let f = feed::build_feed(&q, feed::load(&q, &base).unwrap()).unwrap();
    assert!(f.header.label.starts_with("MODELLED"));
    let a = f.records.iter().find(|r| r.subject_id == "SYN-A").unwrap();
    let b = f.records.iter().find(|r| r.subject_id == "SYN-B").unwrap();
    assert!(a.jamming_exposure_h > 3.0 && a.spoofing_exposure_h > 0.0);
    assert_eq!(a.event_ids, vec!["SYN-J1", "SYN-S1"]);
    assert_eq!(a.position_jumps, 2);
    assert!(a.max_jump_km > 120.0 && a.listed_area_h > 0.0);
    assert_eq!(a.cyber_exposure_indicator, "high");
    assert_eq!(a.data_quality, "sufficient");
    assert_eq!(
        (b.rfi_exposure_h, b.position_jumps, b.listed_area_h),
        (0.0, 0, 0.0)
    );
    assert_eq!(b.cyber_exposure_indicator, "low");
    assert!(f.header.listed_area_sources[0].contains("SYNTHETIC"));
    // Every line carries the fields the schema requires, with the right enums.
    let schema = feed::schema();
    let jsonl = feed::to_jsonl(&f);
    let lines: Vec<serde_json::Value> = jsonl
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    for (i, l) in lines.iter().enumerate() {
        let part = &schema["oneOf"][if i == 0 { 0 } else { 1 }];
        for k in part["required"].as_array().unwrap() {
            assert!(l.get(k.as_str().unwrap()).is_some(), "line {i}: {k}");
        }
        for (k, spec) in part["properties"].as_object().unwrap() {
            if let (Some(v), Some(e)) = (l.get(k), spec.get("enum")) {
                assert!(e.as_array().unwrap().contains(v), "{k} = {v}");
            }
            if let (Some(v), Some(c)) = (l.get(k), spec.get("const")) {
                assert_eq!(v, c);
            }
        }
    }
}

fn signed_verdict(class: &str, sk: &SigningKey) -> serde_json::Value {
    let body = serde_json::json!({
        "service": "test-service",
        "method": "cause",
        "evidence_class": "MODELLED",
        "utc": UTC,
        "key_id": "k1",
        "verdict": {"classification": class, "confidence": 0.8},
    });
    let bytes = serde_json::to_vec(&body).unwrap();
    let mut v = body.clone();
    v["verdict_sha256"] = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&bytes)).into();
    v["signature"] = hex::encode(sk.sign(&bytes).to_bytes()).into();
    v
}

#[test]
fn triage_outcomes() {
    let w = sample_dir("triage");
    let claim = |cause: &str, extra: &str| {
        format!(
            "events = [\"events.json\"]\n{extra}\n[claim]\nclaim_id = \"C-1\"\nsubject_id = \"SYN-A\"\nstart_utc = \"2026-10-01T09:40:00Z\"\nend_utc = \"2026-10-01T10:20:00Z\"\nlat_deg = 34.0\nlon_deg = 34.25\nradius_km = 10.0\nclaimed_cause = \"{cause}\"\n"
        )
    };
    let run = |src: String, key: Option<&str>| {
        std::fs::write(w.join("claim.toml"), src).unwrap();
        feed::triage(&w.join("claim.toml"), &[], key).unwrap().0
    };
    // A single-observer spoofing event: partly corroborated.
    let r = run(claim("spoofing", ""), None);
    assert!(r.label.starts_with("MODELLED"));
    assert_eq!(r.outcome, "partly corroborated");
    assert!(r
        .items
        .iter()
        .any(|i| i.id == "SYN-S1" && i.bearing == "supports"));
    // Nothing in a far place: no evidence found, and it says that is not evidence against.
    let far = claim("spoofing", "").replace("lat_deg = 34.0", "lat_deg = 10.0");
    let r = run(far, None);
    assert_eq!(r.outcome, "no Kshana evidence found");
    assert!(r.reasons[0].contains("not evidence against"));
    // A signed cause verdict: supporting, conflicting, and one with a wrong key set aside.
    let sk = SigningKey::from_bytes(&SEED);
    let pk = hex::encode(sk.verifying_key().to_bytes());
    for (class, expect) in [
        ("external_spoofing", "corroborated"),
        ("equipment_fault", "conflicting evidence"),
    ] {
        std::fs::write(
            w.join("verdict.json"),
            signed_verdict(class, &sk).to_string(),
        )
        .unwrap();
        let r = run(
            claim("spoofing", "cause_verdict = \"verdict.json\""),
            Some(&pk),
        );
        assert_eq!(r.outcome, expect, "{class}");
    }
    let other = hex::encode(
        SigningKey::from_bytes(&[1u8; 32])
            .verifying_key()
            .to_bytes(),
    );
    let r = run(
        claim("spoofing", "cause_verdict = \"verdict.json\""),
        Some(&other),
    );
    assert_eq!(r.outcome, "partly corroborated");
    assert!(r.items.iter().any(|i| i.bearing == "set aside"));
}

#[test]
fn features_have_explicit_tiers() {
    for (f, tier) in [
        (insurance::FEATURE_TRIGGER, Tier::Pro),
        (insurance::FEATURE_BASIS, Tier::Pro),
        (insurance::FEATURE_ACCUMULATION, Tier::Enterprise),
        (insurance::FEATURE_BI, Tier::Pro),
        (insurance::FEATURE_FEED, Tier::Pro),
        (insurance::FEATURE_TRIAGE, Tier::Pro),
    ] {
        assert_eq!(license::explicit_tier(f), Some(tier), "{f}");
        assert!(license::PRO_COMMAND_FEATURES.contains(&f));
    }
    assert_eq!(insurance::FEATURES.len(), 6);
    let ent = |tier| Entitlement {
        licensee: "ACME".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&ent(Tier::Pro), insurance::FEATURE_BI, 0).is_ok());
    assert!(license::check(&ent(Tier::Pro), insurance::FEATURE_ACCUMULATION, 0).is_err());
    assert!(license::check(&ent(Tier::Enterprise), insurance::FEATURE_ACCUMULATION, 0).is_ok());
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(args)
        .env_remove("KSHANA_PRO_LICENSE")
        .env_remove("KSHANA_VERDICT_KEY")
        .output()
        .unwrap()
}

#[test]
fn command_line_end_to_end() {
    let w = tmp("cli");
    let s = |p: &Path| p.display().to_string();
    // Free: schema, sample, bi models.
    let o = cli(&["insurance", "schema"]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains(feed::FEED_FORMAT));
    let sdir = w.join("sample");
    let o = cli(&["insurance", "sample", "--out", &s(&sdir)]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(sdir.join(feed::FEED_FILE).exists() && sdir.join("schema.json").exists());
    let o = cli(&["insurance", "bi", "models"]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("yield_loss_pct_per_day"));

    let key = w.join("signer.key");
    std::fs::write(&key, format!("private_key={}\n", hex::encode(SEED))).unwrap();
    let pk = hex::encode(license::public_key_for(&SEED));
    let bi_req = Path::new(EXAMPLES).join("bi-timing-svn23.toml");
    // Without a licence: refused, naming the feature.
    let o = cli(&[
        "insurance",
        "bi",
        &s(&bi_req),
        "--out",
        &s(&w.join("nolic")),
        "--signer",
        "Ops",
        "--key-file",
        &s(&key),
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains(insurance::FEATURE_BI));

    let td = trigger_dir("cli-trig");
    for (cmd, req, file) in [
        ("bi", bi_req.clone(), bi::RESULT_FILE),
        ("trigger", td.join("trigger.toml"), trigger::RESULT_FILE),
        (
            "accumulation",
            Path::new(EXAMPLES).join("accumulation.toml"),
            basis::ACC_FILE,
        ),
        ("feed", sdir.join("request.toml"), feed::FEED_FILE),
    ] {
        let out = w.join(format!("out-{cmd}"));
        let o = cli(&[
            "insurance",
            cmd,
            &s(&req),
            "--out",
            &s(&out),
            "--signer",
            "Ops",
            "--key-file",
            &s(&key),
            "--utc",
            UTC,
            "--dev",
        ]);
        assert!(
            o.status.success(),
            "{cmd}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        for f in [file, "report.html", "request.toml", "signature.json"] {
            assert!(out.join(f).exists(), "{cmd}: {f}");
        }
        let text = std::fs::read_to_string(out.join(file)).unwrap();
        assert!(text.contains("MODELLED") && text.contains("Not insurance or financial advice"));
        let o = cli(&["insurance", "verify", &s(&out), "--trust", &pk]);
        assert!(
            o.status.success(),
            "{cmd}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
    }
    assert!(w.join("out-bi/sensitivity.csv").exists());
    std::fs::write(w.join("out-bi/report.html"), "changed").unwrap();
    let o = cli(&["insurance", "verify", &s(&w.join("out-bi")), "--trust", &pk]);
    assert!(!o.status.success());
}
