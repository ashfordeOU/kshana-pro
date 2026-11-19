// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
use super::*;
use crate::chain::StationFault;
use crate::geo::{direct, Geo};
use crate::prop::ground::{self, GroundConfig};
use crate::prop::BuildContext;
use crate::scenario::Scenario;
use crate::scene::MemSink;
use crate::signal::Role;
use crate::threat::Meaconer;
use crate::timebase::gauss;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

fn land_model() -> Box<dyn PropagationModel> {
    let cfg = GroundConfig::SmoothEarth {
        sigma_s_m: 0.005,
        eps_r: None,
        k_factor: crate::prop::ground::K_LORAN,
        temporal: None,
    };
    let ctx = BuildContext::new(
        AbsTime::from_utc_iso("2026-11-18T12:00:00Z").unwrap(),
        1.0,
        0,
    );
    ground::build(&cfg, &ctx).unwrap()
}

fn station(id: &str, pos: Geo) -> Station {
    Station {
        id: id.into(),
        name: id.into(),
        role: Role::Secondary,
        position: pos,
        power_kw: 400.0,
        emission_delay_us: 0.0,
        time_ref: String::new(),
        tx_clock: None,
        ecd_us: 0.0,
        status: crate::chain::StationStatus::On,
        faults: vec![],
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Truth {
    Clean,
    Relay,
    Fault,
}

/// One measurement-domain case: 4–6 stations 250–1100 km away; model errors drawn inside
/// the stated σ; an unknown receiver clock. `Relay` replaces one station by a replica as
/// the crate's `Meaconer` builds it (regenerated from the known pulse structure: the
/// transmitted ECD, no path dispersion; re-radiated with a capture margin over the real
/// field; late by geometry plus processing). `Fault` adds a transmitter timing step.
fn mc_case(
    rng: &mut ChaCha8Rng,
    model: &dyn PropagationModel,
    truth: Truth,
) -> (Vec<StationVerdict>, usize) {
    let rx_pos = Geo::new(52.0, 0.0);
    let rx = RxState {
        t_s: 0.0,
        pos: rx_pos,
        heading_rad: 0.0,
        speed_mps: 0.0,
    };
    let n = rng.gen_range(4..=6);
    let clock_ns = rng.gen_range(-5000.0..5000.0);
    let target = rng.gen_range(0..n);
    let mut obs = Vec::new();
    let mut exp = Vec::new();
    for i in 0..n {
        let az = (i as f64 + rng.gen_range(-0.3..0.3)) * std::f64::consts::TAU / n as f64;
        let d = rng.gen_range(250e3..1100e3);
        let st = station(&format!("S{i}"), direct(rx_pos, az, d));
        let e = Expected::from_model(model, "C", &st, &rx, ModelSigmas::default()).unwrap();
        let s = e.sigma.clone();
        // the real (clean) received signature: prediction + model error inside σ
        let mut delay = e.delay_ns + s.delay_ns * gauss(rng);
        let mut ecd = e.ecd_us + s.ecd_us * gauss(rng);
        let mut field = e.field_dbuv_m + s.field_db * gauss(rng);
        if i == target {
            match truth {
                Truth::Clean => {}
                Truth::Relay => {
                    delay += rng.gen_range(500.0..25_000.0);
                    ecd = st.ecd_us; // regenerated replica: no path dispersion
                    field += rng.gen_range(12.0..24.0);
                }
                Truth::Fault => {
                    let m = rng.gen_range(600.0..2000.0);
                    delay += if rng.gen::<bool>() { m } else { -m };
                }
            }
        }
        obs.push(Observed {
            station: e.station.clone(),
            pseudo_delay_ns: Some(delay + clock_ns + 20.0 * gauss(rng)),
            sigma_delay_ns: 20.0,
            ecd_us: Some(ecd + 0.05 * gauss(rng)),
            sigma_ecd_us: 0.05,
            field_dbuv_m: Some(field + 0.3 * gauss(rng)),
            sigma_field_db: 0.3,
            ..Default::default()
        });
        exp.push(e);
    }
    (screen(&obs, &exp, None, &MeaconConfig::default()), target)
}

/// Confusion matrix over N cases per class (measurement domain). Stated rates:
/// clean false alarm ≤ 2.5e-3 per station and ≤ 1.5 % per case (any station not
/// `Consistent`; nominal 1e-3 per station from two tests at p_fa = 5e-4); relay identified as `Relay` on
/// the attacked station ≥ 97 %; transmitter timing fault identified as `TimingOnly` ≥ 97 %
/// and mistaken for a relay ≤ 2 %.
#[test]
fn confusion_matrix_measurement_domain() {
    let model = land_model();
    let mut rng = ChaCha8Rng::seed_from_u64(0xE4);
    let n = 1500;
    let (mut fa, mut st_fa, mut n_st) = (0, 0, 0);
    let (mut relay_ok, mut fault_ok, mut fault_as_relay) = (0, 0, 0);
    for _ in 0..n {
        let (v, _) = mc_case(&mut rng, model.as_ref(), Truth::Clean);
        n_st += v.len();
        st_fa += v.iter().filter(|s| s.class != Class::Consistent).count();
        if v.iter().any(|s| s.class != Class::Consistent) {
            fa += 1;
        }
        let (v, t) = mc_case(&mut rng, model.as_ref(), Truth::Relay);
        if v[t].class == Class::Relay {
            relay_ok += 1;
        }
        let (v, t) = mc_case(&mut rng, model.as_ref(), Truth::Fault);
        if v[t].class == Class::TimingOnly {
            fault_ok += 1;
        }
        if v.iter().any(|s| s.class == Class::Relay) {
            fault_as_relay += 1;
        }
    }
    let r = |k: usize| k as f64 / n as f64;
    println!(
        "E4 MC (N={n}/class): clean FA per case {:.4}, per station {:.5}, relay→Relay {:.3}, fault→TimingOnly {:.3}, fault→Relay {:.3}",
        r(fa),
        st_fa as f64 / n_st as f64,
        r(relay_ok),
        r(fault_ok),
        r(fault_as_relay)
    );
    // two tests per station at p_fa = 5e-4 each: nominal 1e-3 per station, ≈ 0.5 % per case
    let st_rate = st_fa as f64 / n_st as f64;
    assert!(
        st_rate <= 2.5e-3,
        "clean per-station false-alarm rate {st_rate}"
    );
    assert!(r(fa) <= 0.015, "clean per-case false-alarm rate {}", r(fa));
    assert!(r(relay_ok) >= 0.97, "relay detection {}", r(relay_ok));
    assert!(r(fault_ok) >= 0.97, "fault attribution {}", r(fault_ok));
    assert!(
        r(fault_as_relay) <= 0.02,
        "fault taken for relay {}",
        r(fault_as_relay)
    );
}

#[test]
fn each_feature_moves_the_statistic_the_right_way() {
    let model = land_model();
    let rx = RxState {
        t_s: 0.0,
        pos: Geo::new(52.0, 0.0),
        heading_rad: 0.0,
        speed_mps: 0.0,
    };
    let st = station("S", direct(rx.pos, 1.0, 800e3));
    let mut e =
        Expected::from_model(model.as_ref(), "C", &st, &rx, ModelSigmas::default()).unwrap();
    // long land path: the envelope leads the carrier (negative propagation ECD)
    assert!(e.ecd_us < -0.3, "propagation ECD {}", e.ecd_us);
    assert!((e.bearing_deg - 1f64.to_degrees()).abs() < 1.0);
    e.sky = Some(SkyExpected {
        delay_after_ground_us: 40.0,
        ratio_db: -6.0,
    });
    let clean = Observed {
        station: e.station.clone(),
        pseudo_delay_ns: Some(e.delay_ns + 1000.0),
        sigma_delay_ns: 10.0,
        ecd_us: Some(e.ecd_us),
        sigma_ecd_us: 0.05,
        field_dbuv_m: Some(e.field_dbuv_m),
        sigma_field_db: 0.3,
        sky_delay_after_ground_us: Some(40.0),
        sky_ratio_db: Some(-6.0),
        phase_code_offset_us: Some(0.0),
        bearing_deg: Some(e.bearing_deg),
    };
    let clk = Some(ClockRef {
        bias_ns: 1000.0,
        sigma_ns: 10.0,
    });
    let cfg = MeaconConfig::default();
    let v = test_station(&clean, &e, clk, &cfg);
    assert_eq!(v.class, Class::Consistent);
    assert_eq!(v.signature_dof, 5);
    assert!(v.signature_stat < 1e-9 && v.relay_llr < 0.0);
    // each signature feature alone, pushed to 5σ, makes a relay
    let mut cases: Vec<(&str, Observed)> = Vec::new();
    let mut o = clean.clone();
    o.ecd_us = Some(e.ecd_us + 5.0 * 0.158);
    cases.push(("ECD", o));
    let mut o = clean.clone();
    o.field_dbuv_m = Some(e.field_dbuv_m + 13.0);
    cases.push(("field", o));
    let mut o = clean.clone();
    o.sky_ratio_db = Some(-40.0);
    cases.push(("sky ratio", o));
    let mut o = clean.clone();
    o.sky_delay_after_ground_us = Some(10.0);
    cases.push(("sky delay", o));
    let mut o = clean.clone();
    o.bearing_deg = Some((e.bearing_deg + 180.0).rem_euclid(360.0));
    cases.push(("bearing", o));
    for (name, o) in cases {
        let v = test_station(&o, &e, clk, &cfg);
        assert_eq!(v.class, Class::Relay, "{name}: {v:?}");
        assert!(v.relay_llr > 5.0, "{name}");
        assert!(!v.explanation.is_empty());
        if name == "bearing" {
            assert!(v.relay_bearing_deg.is_some());
        }
    }
    // timing alone (late or early) is a timing anomaly, not a relay
    for dt in [600.0, -600.0] {
        let mut o = clean.clone();
        o.pseudo_delay_ns = Some(e.delay_ns + 1000.0 + dt);
        let v = test_station(&o, &e, clk, &cfg);
        assert_eq!(v.class, Class::TimingOnly, "{dt}");
        assert!((v.excess_delay_ns.unwrap() - dt).abs() < 1e-6);
    }
    // a phase-code misalignment alone is a timing anomaly
    let mut o = clean.clone();
    o.phase_code_offset_us = Some(10.0);
    assert_eq!(test_station(&o, &e, clk, &cfg).class, Class::TimingOnly);
    // late + modest signature evidence (between the weak and the strict bar) → relay;
    // early + the same evidence → not a relay (a relay is never early)
    let mut o = clean.clone();
    o.field_dbuv_m = Some(e.field_dbuv_m + 2.502 * 4.2);
    let s_only = test_station(&o, &e, clk, &cfg);
    assert!(
        s_only.signature_stat > chi2_quantile(0.99, 5)
            && s_only.signature_stat < s_only.signature_threshold
    );
    assert_eq!(s_only.class, Class::Consistent);
    o.pseudo_delay_ns = Some(e.delay_ns + 1000.0 + 800.0);
    assert_eq!(test_station(&o, &e, clk, &cfg).class, Class::Relay);
    o.pseudo_delay_ns = Some(e.delay_ns + 1000.0 - 800.0);
    assert_eq!(test_station(&o, &e, clk, &cfg).class, Class::TimingOnly);
}

#[test]
fn cross_station_geometry_supplies_the_clock() {
    // without a clock reference, a late station stands out against the other stations'
    // common clock; with only two stations no leave-one-out clock exists
    let model = land_model();
    let rx = RxState {
        t_s: 0.0,
        pos: Geo::new(52.0, 0.0),
        heading_rad: 0.0,
        speed_mps: 0.0,
    };
    let exp: Vec<Expected> = (0..4)
        .map(|i| {
            let st = station(
                &format!("S{i}"),
                direct(rx.pos, i as f64 * 1.5, 400e3 + 100e3 * i as f64),
            );
            Expected::from_model(model.as_ref(), "C", &st, &rx, ModelSigmas::default()).unwrap()
        })
        .collect();
    let mk = |late: f64, i: usize| Observed {
        station: exp[i].station.clone(),
        pseudo_delay_ns: Some(exp[i].delay_ns + 3210.0 + if i == 2 { late } else { 0.0 }),
        sigma_delay_ns: 10.0,
        ..Default::default()
    };
    let obs: Vec<Observed> = (0..4).map(|i| mk(900.0, i)).collect();
    let v = screen(&obs, &exp, None, &MeaconConfig::default());
    assert_eq!(v[2].class, Class::TimingOnly);
    assert!((v[2].excess_delay_ns.unwrap() - 900.0).abs() < 1.0);
    for i in [0, 1, 3] {
        // the late station pulls the others' leave-one-out clock by 900/3 ns, still < 3.3σ
        assert_ne!(v[i].class, Class::Relay);
    }
    let v2 = screen(&obs[..2], &exp, None, &MeaconConfig::default());
    assert!(v2.iter().all(|s| s.timing.is_none()));
}

// ------------------------------------------------------------------ receiver level

const SCN: &str = r#"
[scene]
start_utc = "2026-11-18T12:00:00Z"
duration_s = 1.2
fs_hz = 1000000
seed = 3
output_scale_uv_m = 1000000.0

[receiver]
type = "static"
lat_deg = 52.0
lon_deg = 0.0

[[outputs]]
type = "e_field"

[propagation.ground]
model = "smooth_earth"
sigma_s_m = 0.005

[[interference]]
type = "wgn"
rms_uv_m_20khz = 5.0

[[chains]]
name = "G"
gri_us = 79800
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 55.5, lon_deg = 0.5 }
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 49.0, lon_deg = -3.5 }
emission_delay_us = 13000.0
[[chains.stations]]
id = "Y"
role = "secondary"
position = { lat_deg = 51.0, lon_deg = 6.0 }
emission_delay_us = 29000.0
[[chains.stations]]
id = "Z"
role = "secondary"
position = { lat_deg = 54.5, lon_deg = -5.0 }
emission_delay_us = 46000.0
"#;

/// Run the scene through the crate's receiver with an optional meacon (the crate's
/// `threat::Meaconer`) or transmitter timing fault on station `target`; return the
/// detector's verdicts.
fn receiver_case(
    seed: u64,
    meacon: Option<(f64, f64)>,
    fault_ns: Option<f64>,
    target: usize,
) -> Vec<StationVerdict> {
    let mut sc = Scenario::from_toml(SCN).unwrap();
    sc.scene.params.seed = seed;
    if let Some(e) = fault_ns {
        sc.chains[0].chain.stations[target]
            .faults
            .push(StationFault::TimingStep {
                from_s: 0.0,
                to_s: 10.0,
                error_ns: e,
            });
    }
    let chains: Vec<Chain> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let start = sc.start().unwrap();
    let model = crate::prop::build(&sc.propagation, &sc.build_context().unwrap()).unwrap();
    let rx = RxState {
        t_s: 0.0,
        pos: Geo::new(52.0, 0.0),
        heading_rad: 0.0,
        speed_mps: 0.0,
    };
    let exp: Vec<Expected> = chains[0]
        .stations
        .iter()
        .map(|st| {
            Expected::from_model(model.as_ref(), "G", st, &rx, ModelSigmas::default()).unwrap()
        })
        .collect();
    let mut scene = sc.build().unwrap();
    if let Some((delay_s, gain_db)) = meacon {
        // record-and-delay: the replica leaves the relay late by the path delay to the
        // receiver plus the relay's excess (`delay_s`), re-radiated with a capture margin
        let real = 10f64.powf(exp[target].field_dbuv_m / 20.0);
        let total = exp[target].delay_ns * 1e-9 + delay_s;
        let m = Meaconer::new(&chains[0], target, start, total, real, gain_db, 0.0, 10.0).unwrap();
        scene.interferers.push(Box::new(m));
    }
    let mut sink = MemSink::default();
    let mut truth = vec![];
    scene.run(&mut sink, &mut truth).unwrap();
    let input = crate::rx::RxInput::new(sink.channels, sc.scene.params.fs_hz, start);
    let mut cfg = crate::rx::RxConfig::default();
    cfg.output_scale_uv_m = sc.scene.params.output_scale_uv_m;
    cfg.approx_position = Some(rx.pos);
    let (_rx, out) = crate::rx::process(&input, &chains, cfg).unwrap();
    let obs = Observed::from_measurements(&out.measurements, &chains, start, 0.0);
    screen(&obs, &exp, None, &MeaconConfig::default())
}

/// End to end through the crate's simulator, threat model and receiver: N runs per class
/// (different noise seeds, meacon delays/gains, fault sizes and targets). Stated rates
/// over this small sample: every clean run consistent on every station; every meacon
/// identified as `Relay` on the attacked station; no transmitter fault taken for a relay.
#[test]
fn receiver_level_meacon_and_fault_runs() {
    let n = 3;
    let mut report = Vec::new();
    for k in 0..n {
        let seed = 100 + k as u64;
        let target = 1 + k % 3;
        let clean = receiver_case(seed, None, None, target);
        report.push(format!(
            "clean {k}: {:?}",
            clean
                .iter()
                .map(|v| (v.class, v.signature_stat))
                .collect::<Vec<_>>()
        ));
        assert!(
            clean.iter().all(|v| v.class == Class::Consistent),
            "clean run {k}: {clean:#?}"
        );
        let delay = [3e-6, 10e-6, 25e-6][k];
        let gain = [14.0, 18.0, 20.0][k];
        let m = receiver_case(seed, Some((delay, gain)), None, target);
        report.push(format!(
            "meacon {k}: {:?}",
            m.iter()
                .map(|v| (v.class, v.signature_stat))
                .collect::<Vec<_>>()
        ));
        assert_eq!(
            m[target].class,
            Class::Relay,
            "meacon run {k}: {:#?}",
            m[target]
        );
        let f = receiver_case(seed, None, Some([700.0, -900.0, 1500.0][k]), target);
        report.push(format!(
            "fault {k}: {:?}",
            f.iter()
                .map(|v| (v.class, v.signature_stat))
                .collect::<Vec<_>>()
        ));
        assert!(
            f.iter().all(|v| v.class != Class::Relay),
            "fault run {k}: {f:#?}"
        );
        assert_eq!(
            f[target].class,
            Class::TimingOnly,
            "fault run {k}: {:#?}",
            f[target]
        );
    }
    println!("{}", report.join("\n"));
}
