// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
use super::*;
use crate::scenario::Scenario;
use crate::scene::MemSink;

const SCN: &str = include_str!("../../scenarios/rx-high-snr.toml");

fn energy(x: &[f64]) -> f64 {
    x.iter().map(|v| v * v).sum()
}

#[test]
fn jammer_noise_and_cw_are_confined_to_their_window() {
    let fs = 1e6;
    let mut j = Jammer::noise(1000.0, 0.01, 0.02, 7);
    let mut a = vec![0.0; 10_000];
    j.add(Ps::ZERO, fs, 0, &mut a);
    assert_eq!(energy(&a), 0.0, "jammer active before its window");
    let mut b = vec![0.0; 10_000];
    j.add(Ps::from_s(0.01), fs, 0, &mut b);
    assert!(energy(&b) > 0.0, "jammer silent inside its window");
    let t = j.truth();
    assert_eq!(t.kind, AttackKind::JammerNoise);
    assert_eq!((t.from_s, t.to_s), (0.01, 0.02));

    let mut cw = Jammer::cw(100e3, 500.0, 0.0, 1.0);
    let mut c = vec![0.0; 20_000];
    cw.add(Ps::ZERO, fs, 0, &mut c);
    let rms = (energy(&c) / c.len() as f64).sqrt();
    assert!((rms - 500.0).abs() / 500.0 < 0.02, "CW RMS {rms}");
    assert_eq!(cw.truth().kind, AttackKind::JammerCw);
}

#[test]
fn spoofer_emits_loran_pulses_and_pulls_time() {
    let sc = Scenario::from_toml(SCN).unwrap();
    let chain = sc.chains[0].chain.clone();
    let start = sc.start().unwrap();
    let fs = 1e6;
    let n = (0.2 * fs) as usize;
    let mut s = Spoofer::new(&chain, start, 1000.0, 5e-6, 0.0, 0.0, 1.0).unwrap();
    let mut x = vec![0.0; n];
    s.add(Ps::ZERO, fs, 0, &mut x);
    assert!(energy(&x) > 0.0, "spoofer produced no pulses");
    // a pulled spoofer differs from the unpulled one later in the window
    let mut p = Spoofer::new(&chain, start, 1000.0, 5e-6, 1000.0, 0.0, 1.0).unwrap();
    let mut y = vec![0.0; n];
    p.add(Ps::ZERO, fs, 0, &mut y);
    let tail = n / 2..n;
    let d: f64 = x[tail.clone()].iter().zip(&y[tail]).map(|(a, b)| (a - b).powi(2)).sum();
    assert!(d > 0.0, "time pull had no effect");
    assert_eq!(s.truth().kind, AttackKind::Spoof);
}

#[test]
fn meaconer_is_a_delayed_copy_of_one_station() {
    let sc = Scenario::from_toml(SCN).unwrap();
    let chain = sc.chains[0].chain.clone();
    let start = sc.start().unwrap();
    assert!(Meaconer::new(&chain, 99, start, 50e-6, 1000.0, 0.0, 0.0, 1.0).is_err());
    let mut m = Meaconer::new(&chain, 1, start, 50e-6, 1000.0, 6.0, 0.0, 1.0).unwrap();
    let mut x = vec![0.0; 200_000];
    m.add(Ps::ZERO, 1e6, 0, &mut x);
    assert!(energy(&x) > 0.0);
    let t = m.truth();
    assert_eq!(t.kind, AttackKind::Meacon);
    assert!((t.field_uv_m - 1000.0 * 10f64.powf(6.0 / 20.0)).abs() < 1e-6);
    assert_eq!(t.target.as_ref().map(|x| x.1.as_str()), Some(chain.stations[1].id.as_str()));
}

#[test]
fn forged_data_channel_messages_are_flagged_by_the_auth_check() {
    let genuine = b"genuine-key-0123456789".to_vec();
    let corr = vec![DloranSpecEntry { station_code: 1, correction_ns: 250.0 }];
    // wrong key: the tag does not verify with the genuine key
    let wrong = DataChannelAttack::forged_dloran(7, corr.clone(), ForgedAuth::WrongKey(b"attacker".to_vec()), 0, 100);
    let (checks, caught) = wrong.check(&genuine);
    assert!(caught);
    assert!(checks.iter().any(|c| c.verified == Some(false)), "{checks:?}");
    // absent authentication: the data is unauthenticated, so it is treated as caught
    let absent = DataChannelAttack::forged_dloran(7, corr.clone(), ForgedAuth::Absent, 0, 100);
    assert!(absent.check(&genuine).1);
    assert!(!absent.decode_report().messages.is_empty());
    // a message set tagged with the genuine key verifies (control: the check is not trivially failing)
    let control = DataChannelAttack::forged_dloran(7, corr, ForgedAuth::WrongKey(genuine.clone()), 0, 100);
    let (cc, caught_c) = control.check(&genuine);
    assert!(!caught_c, "{cc:?}");
    assert!(cc.iter().all(|c| c.verified != Some(false)));
}

#[test]
fn meaconing_shifts_the_tracked_toa_and_is_detected() {
    let sc = Scenario::from_toml(SCN).unwrap();
    let chains: Vec<Chain> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let start = sc.start().unwrap();
    let run = |attack: bool| {
        let mut scene = sc.build().unwrap();
        if attack {
            // a strong delayed copy of station X (+20 dB over a 100 µV/m-class source) from 0.5 s
            let m = Meaconer::new(&chains[0], 1, start, 25e-6, 5000.0, 20.0, 0.5, 10.0).unwrap();
            scene.interferers.push(Box::new(m));
        }
        let mut sink = MemSink::default();
        let mut truth = vec![];
        scene.run(&mut sink, &mut truth).unwrap();
        let input = crate::rx::RxInput::new(sink.channels, sc.scene.params.fs_hz, start);
        let mut cfg = crate::rx::RxConfig::default();
        cfg.output_scale_uv_m = sc.scene.params.output_scale_uv_m;
        let (_rx, out) = crate::rx::process(&input, &chains, cfg).unwrap();
        out.measurements
    };
    let clean = run(false);
    let attacked = run(true);
    let lim = 100.0;
    assert!(toa_anomalies(&clean, &chains, start, lim).is_empty(), "false alarm on the clean run");
    let flagged = toa_anomalies(&attacked, &chains, start, lim);
    assert!(
        flagged.iter().any(|m| m.station == chains[0].stations[1].id),
        "meaconed station not flagged: {:?}",
        station_metrics(&attacked, &chains, start)
            .iter()
            .map(|m| (m.station.clone(), m.toa_max_jump_ns))
            .collect::<Vec<_>>()
    );
}
