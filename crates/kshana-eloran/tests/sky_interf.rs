// PROPRIETARY — Kshana Pro.
//! Scene-level tests of streams S6 (skywave) and S7 (interference and noise).
use kshana_eloran::interf::dsp::band_power;
use kshana_eloran::interf::{noise_rms_20k, InterfConfig};
use kshana_eloran::scenario::Scenario;

const BASE: &str = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.12
fs_hz = 2000000
seed = 1
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 0.0
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
[propagation.ground]
model = "simple"
SKY
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.0, lon_deg = 0.0 }
time_ref = "UTC(A)"
"#;

#[test]
fn skywave_40us_plus6db_distorts_trailing_edge_only() {
    let sky = r#"
[propagation.sky]
model = "ionospheric"
daylight = "night"
delay_us = 40.0
ratio_db = 6.0
phase_height_offset_km = 0.0
"#;
    let (s0, t0, _) = kshana_eloran::run_to_memory(&BASE.replace("SKY", "")).unwrap();
    let (s1, t1, _) = kshana_eloran::run_to_memory(&BASE.replace("SKY", sky)).unwrap();
    let (x0, x1) = (&s0.channels[0], &s1.channels[0]);
    let fs = 2e6;
    assert_eq!(t0.len(), t1.len());
    let r = &t1[0];
    assert!(
        (r.sky_delay_us.unwrap() - 40.0).abs() < 1e-3,
        "{:?}",
        r.sky_delay_us
    );
    assert!((r.sky_ratio_db.unwrap() - 6.0).abs() < 1e-6);
    for rec in &t1 {
        // every pulse of the group: origin T_k = toa + k ms (8 + master 9th at 9 ms)
        for k in [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 9.0] {
            let t = rec.toa_s + k * 1e-3;
            let idx = |us: f64| ((t + us * 1e-6) * fs).round() as usize;
            if idx(250.0) >= x0.len() {
                continue;
            }
            // before 30 µs (indeed before 40 µs) the samples are bit-identical
            for n in idx(-5.0)..idx(39.5) {
                assert_eq!(
                    x0[n],
                    x1[n],
                    "pulse {k} sample at {:.2} µs",
                    n as f64 / fs * 1e6 - t * 1e6
                );
            }
            // after 40 µs the skywave (2× the groundwave) dominates
            let peak0 = (idx(0.0)..idx(250.0))
                .map(|n| x0[n].abs())
                .fold(0.0, f64::max);
            let dmax = (idx(45.0)..idx(250.0))
                .map(|n| (x1[n] - x0[n]).abs())
                .fold(0.0, f64::max);
            assert!(
                dmax > 1.0 * peak0,
                "pulse {k}: distortion {dmax} vs peak {peak0}"
            );
        }
    }
}

#[test]
fn skywave_night_scenario_runs() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scenarios/skywave-night.toml"
    ))
    .unwrap();
    let (sink, truth, stats) = kshana_eloran::run_to_memory(&src).unwrap();
    assert_eq!(stats.channels, 1);
    assert_eq!(sink.channels[0].len(), 300_000);
    assert!(!truth.is_empty());
    for r in &truth {
        let d = r.sky_delay_us.expect("skywave present");
        assert!(
            (30.0..200.0).contains(&d),
            "{} sky delay {d} µs at {} km",
            r.station,
            r.distance_km
        );
        let q = r.sky_ratio_db.unwrap();
        assert!((-30.0..20.0).contains(&q), "{} ratio {q} dB", r.station);
    }
    // night: midpoint sun far below the horizon → E-region heights: delays of a ~560 km
    // path ≈ 90–130 µs
    let m = truth.iter().find(|r| r.station == "M").unwrap();
    assert!(
        (80.0..140.0).contains(&m.sky_delay_us.unwrap()),
        "{:?}",
        m.sky_delay_us
    );
    // deterministic
    let (sink2, _, _) = kshana_eloran::run_to_memory(&src).unwrap();
    assert_eq!(sink.channels, sink2.channels);
}

#[test]
fn skywave_day_night_follows_the_scene_start_without_sky_start_utc() {
    // the sky table has no start_utc: the scene start drives the sun's position
    let sky = "[propagation.sky]\nmodel = \"ionospheric\"\ndaylight = \"auto\"\n";
    let night = BASE
        .replace("SKY", sky)
        .replace("2026-11-18T00:00:00Z", "2026-12-21T00:00:00Z");
    let day = BASE
        .replace("SKY", sky)
        .replace("2026-11-18T00:00:00Z", "2026-06-21T12:00:00Z");
    let tn = kshana_eloran::run_to_memory(&night).unwrap().1;
    let td = kshana_eloran::run_to_memory(&day).unwrap().1;
    let (rn, rd) = (tn[0].sky_ratio_db.unwrap(), td[0].sky_ratio_db.unwrap());
    assert!(rn - rd > 10.0, "night {rn:.1} dB, day {rd:.1} dB");
    assert!(tn[0].sky_delay_us.unwrap() > td[0].sky_delay_us.unwrap());
    // an explicit start_utc in the sky table overrides the scene start
    let over = format!("{sky}start_utc = \"2026-12-21T00:00:00Z\"\n");
    let to = kshana_eloran::run_to_memory(
        &BASE
            .replace("SKY", &over)
            .replace("2026-11-18T00:00:00Z", "2026-06-21T12:00:00Z"),
    )
    .unwrap()
    .1;
    assert_eq!(to[0].sky_ratio_db, tn[0].sky_ratio_db);
}

#[test]
fn stated_sky_ratio_holds_over_smooth_earth_ground() {
    let sky = "[propagation.sky]\nmodel = \"ionospheric\"\ndaylight = \"night\"\ndelay_us = 40.0\nratio_db = -6.0\nphase_height_offset_km = 0.0\n";
    let src = BASE.replace("SKY", sky).replace(
        "[propagation.ground]\nmodel = \"simple\"",
        "[propagation.ground]\nmodel = \"smooth_earth\"\nsigma_s_m = 0.003",
    );
    let sc = Scenario::from_toml(&src).unwrap();
    assert!(matches!(
        sc.propagation.ground,
        kshana_eloran::prop::ground::GroundConfig::SmoothEarth { .. }
    ));
    let t = kshana_eloran::run_to_memory(&src).unwrap().1;
    assert!(!t.is_empty());
    for r in &t {
        assert!(
            (r.sky_ratio_db.unwrap() + 6.0).abs() < 1e-6,
            "{:?}",
            r.sky_ratio_db
        );
        assert!(
            (r.sky_delay_us.unwrap() - 40.0).abs() < 1e-3,
            "{:?}",
            r.sky_delay_us
        );
    }
}

#[test]
fn rfi_railway_scenario_runs() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scenarios/rfi-railway.toml"
    ))
    .unwrap();
    let sc = Scenario::from_toml(&src).unwrap();
    let scale = sc.scene.params.output_scale_uv_m;
    let (sink, truth, stats) = kshana_eloran::run_to_memory(&src).unwrap();
    assert_eq!(stats.samples_per_channel, 500_000);
    assert!(!truth.is_empty() && truth[0].snr_db.is_finite());
    // the same scene without interference: the difference is the interference alone
    let mut clean = sc.clone();
    clean.interference.clear();
    let mut scene = clean.build().unwrap();
    let mut s0 = kshana_eloran::MemSink::default();
    scene.run(&mut s0, &mut vec![]).unwrap();
    let x: Vec<f64> = sink.channels[0]
        .iter()
        .zip(&s0.channels[0])
        .map(|(a, b)| (a - b) * scale)
        .collect();
    let fs = 1e6;
    // receiver-band power outside vs during the train passage (centre 0.2–0.3 s)
    let quiet = band_power(&x[0..100_000], fs, 90e3, 110e3, 1000);
    let passage = band_power(&x[200_000..300_000], fs, 90e3, 110e3, 1000);
    assert!(
        passage > 4.0 * quiet,
        "passage {passage:.1} quiet {quiet:.1}"
    );
    let (sink2, _, _) = kshana_eloran::run_to_memory(&src).unwrap();
    assert_eq!(sink.channels, sink2.channels);
}

const MULTI: &str = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 1.0
fs_hz = 400000
seed = SEED
block_s = BLOCK
output_scale_uv_m = 1000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 0.0
[[outputs]]
type = "e_field"
[[outputs]]
type = "h_loop"
axis_deg = 0.0
[[interference]]
type = "wgn"
rms_uv_m_20khz = 5.0
[[interference]]
type = "atmospheric"
fam_db = 95.0
[[interference]]
type = "band_noise"
centre_hz = 100000.0
bw_hz = 10000.0
rms_uv_m = 8.0
taps = 127
[[interference]]
type = "pulsed"
period_s = 0.002
duty = 0.25
rms_uv_m = 6.0
[[interference]]
type = "impulsive"
rate_hz = 300.0
median_uv_m = 50.0
sigma_db = 3.0
ring_hz = 100000.0
decay_us = 20.0
CW
chains = []
"#;

fn multi(seed: u64, block: f64, cw: bool) -> String {
    let cw = if cw {
        "[[interference]]\ntype = \"cw\"\nfreq_hz = 100000.0\nrms_uv_m = 4.0\n"
    } else {
        ""
    };
    // `chains = []` must precede the array-of-tables in TOML
    let s = MULTI
        .replace("SEED", &seed.to_string())
        .replace("BLOCK", &format!("{block}"))
        .replace("CW", cw);
    let s = s.replace("chains = []\n", "");
    s.replacen("[scene]", "chains = []\n[scene]", 1)
}

#[test]
fn several_sources_at_once_sum_to_the_configured_level() {
    let src = multi(5, 0.01, true);
    let sc = Scenario::from_toml(&src).unwrap();
    let (sink, _, _) = kshana_eloran::run_to_memory(&src).unwrap();
    let x: Vec<f64> = sink.channels[0].iter().map(|v| v * 1000.0).collect();
    let meas = band_power(&x, 4e5, 90e3, 110e3, 400);
    // stochastic sources (noise_rms_20k) plus the CW tone (16 µV²/m²)
    let n = noise_rms_20k(&sc.interference);
    let pred = n * n + 16.0;
    let err_db = 10.0 * (meas / pred).log10();
    assert!(
        err_db.abs() < 0.6,
        "measured {meas:.1} predicted {pred:.1} µV²/m² ({err_db:.2} dB)"
    );
    // components: 25 + 178.0 (Fam 95) + 64 + 9 (6² × 25 %) + ≈2.7 (impulsive)
    assert!((n * n - 278.8).abs() < 2.0, "{}", n * n);
    assert!(sc
        .interference
        .iter()
        .any(|c| matches!(c, InterfConfig::Impulsive(_))));
}

#[test]
fn stochastic_sources_reproduce_and_ignore_block_size() {
    let a = kshana_eloran::run_to_memory(&multi(5, 0.01, false))
        .unwrap()
        .0;
    let b = kshana_eloran::run_to_memory(&multi(5, 0.0037, false))
        .unwrap()
        .0;
    let c = kshana_eloran::run_to_memory(&multi(6, 0.01, false))
        .unwrap()
        .0;
    assert_eq!(a.channels, b.channels, "block size changed the output");
    assert_ne!(a.channels[0], c.channels[0]);
    // diffuse noise differs between antenna channels; coherent sources do not cancel it
    assert_ne!(a.channels[0], a.channels[1]);
}
