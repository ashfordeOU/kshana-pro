// PROPRIETARY — Kshana Pro.
//! LDC decoding inside the software receiver, end to end: scenario → `run_to_memory` →
//! receiver with the chain's LDC configuration → decoded messages compared with what the
//! modulator transmitted (its truth: per-group symbols and scheduled messages).
use kshana_eloran::ldc::build_modulator;
use kshana_eloran::ldc::modulator::LdcModulator;
use kshana_eloran::rx::{self, RxConfig, RxInput, StationLdc};
use kshana_eloran::scenario::Scenario;
use kshana_eloran::time::{civil_from_days, days_from_civil, tai_minus_utc_s, AbsTime, PS_PER_S};

fn scenario(name: &str) -> String {
    std::fs::read_to_string(format!("{}/scenarios/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// UTC ISO string of the whole second at or before `t`.
fn utc_iso(t: AbsTime) -> String {
    let tai = t.0.div_euclid(PS_PER_S as i128);
    let epoch = days_from_civil(1958, 1, 1) as i128;
    let (y, m, d) = civil_from_days((tai.div_euclid(86_400) + epoch) as i64);
    let utc = tai - tai_minus_utc_s(y, m, d) as i128;
    let (y, m, d) = civil_from_days((utc.div_euclid(86_400) + epoch) as i64);
    let s = utc.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

struct Decoded {
    modulator: LdcModulator,
    ldc: Vec<StationLdc>,
    roles: Vec<kshana_eloran::signal::Role>,
}

/// Run `src` from 0.5–1.5 s before the first frame number divisible by `frame_mod` after
/// its start (a cycle start, so whole messages are recorded) for `frames` frames, with
/// optional WGN, and decode.
fn run(src: &str, frame_mod: i64, frames: i64, wgn_uv_m: Option<f64>) -> Decoded {
    let sc = Scenario::from_toml(src).unwrap();
    let chain = &sc.chains[0].chain;
    let ldc = sc.ldc_configs().unwrap()[0].clone().unwrap();
    let modulator = build_modulator(&ldc, Some(chain)).unwrap().unwrap();
    let g_per_f = modulator.groups_per_frame() as i128;
    let gri = chain.gri_us as i128 * 1_000_000;
    let frame_ps = g_per_f * gri;
    let start = sc.start().unwrap();
    let f = (start.0 / frame_ps / frame_mod as i128 + 1) * frame_mod as i128;
    let begin = utc_iso(AbsTime(f * frame_ps - PS_PER_S as i128 / 2));
    let lead_ps = f * frame_ps - AbsTime::from_utc_iso(&begin).unwrap().0;
    let duration_s = ((lead_ps + frames as i128 * frame_ps) as f64 / PS_PER_S as f64 + 0.4).ceil();
    let mut text: String = src
        .replace(&sc.scene.start_utc, &begin)
        .lines()
        .map(|l| {
            if l.starts_with("duration_s =") {
                format!("duration_s = {duration_s}\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    if let Some(n) = wgn_uv_m {
        text.push_str(&format!(
            "\n[[interference]]\ntype = \"wgn\"\nrms_uv_m_20khz = {n}\n"
        ));
    }
    let sc2 = Scenario::from_toml(&text).unwrap();
    assert_eq!(sc2.scene.params.duration_s, duration_s);
    let (sink, _truth, _stats) = kshana_eloran::run_to_memory(&text).unwrap();
    let input = RxInput::new(sink.channels, sc2.scene.params.fs_hz, sc2.start().unwrap());
    let chains: Vec<_> = sc2.chains.iter().map(|c| c.chain.clone()).collect();
    let cfg = RxConfig {
        output_scale_uv_m: sc2.scene.params.output_scale_uv_m,
        ldc: sc2.ldc_configs().unwrap(),
        ..Default::default()
    };
    let (_rx, out) = rx::process(&input, &chains, cfg).unwrap();
    assert!(
        out.acquisitions.iter().all(|a| a.found),
        "{:?}",
        out.acquisitions
    );
    Decoded {
        modulator,
        ldc: out.ldc,
        roles: chain.stations.iter().map(|s| s.role).collect(),
    }
}

/// Compare one station's decoded stream with the modulator's truth; returns the channel
/// symbol errors before FEC.
fn check(d: &Decoded, s: &StationLdc, tag: &str) -> usize {
    let role = d.roles[s.station_index];
    let g0 = s.first_group;
    let tx = d
        .modulator
        .symbols(s.station_index, role, g0, g0 + s.symbols.len() as i64);
    assert_eq!(tx.len(), s.symbols.len());
    let sym_err = tx.iter().zip(&s.symbols).filter(|(a, b)| a != b).count();
    let frames = s.report.frames.len() as i64;
    let truth =
        d.modulator
            .messages_starting(s.station_index, role, s.first_frame, s.first_frame + frames);
    // complete messages only (the last may run past the recording)
    let complete: Vec<_> = truth
        .iter()
        .filter(|m| m.first_frame + m.frames <= s.first_frame + frames)
        .collect();
    eprintln!(
        "[{tag}] {}/{} groups {} frames {} (ok {}, failed {}) symbol errors {} corrected {} msgs {}/{} crc fails {} auth {:?}",
        s.chain,
        s.station,
        s.symbols.len(),
        frames,
        s.report.frames_ok,
        s.report.frames_failed,
        sym_err,
        s.report.symbols_corrected,
        s.report.messages.len(),
        complete.len(),
        s.report.crc_failures,
        s.auth.iter().map(|a| a.verified).collect::<Vec<_>>()
    );
    assert!(
        !complete.is_empty(),
        "{tag}: recording holds no complete message"
    );
    assert_eq!(
        s.report.messages.len(),
        complete.len(),
        "{tag}: {}",
        s.station
    );
    for (dm, tm) in s.report.messages.iter().zip(&complete) {
        assert_eq!(dm.message, tm.message, "{tag}");
        assert_eq!(s.message_frame(dm), tm.first_frame, "{tag}");
        assert_eq!(dm.frames as i64, tm.frames);
    }
    assert_eq!(s.report.frames_failed, 0, "{tag}");
    assert_eq!(
        s.report.symbols_corrected, sym_err,
        "{tag}: every symbol error corrected and counted"
    );
    sym_err
}

/// Frames per message cycle of the scenario's chain 0 (default schedule).
fn cycle_frames(src: &str) -> i64 {
    let sc = Scenario::from_toml(src).unwrap();
    let m = build_modulator(
        &sc.ldc_configs().unwrap()[0].clone().unwrap(),
        Some(&sc.chains[0].chain),
    )
    .unwrap()
    .unwrap();
    m.c.plans[0].cycle_frames
}

#[test]
fn eurofix_scenario_decodes_in_the_receiver() {
    let src = scenario("ldc-eurofix.toml");
    // Eurofix: 30 GRIs (2.02 s) per frame; the scenario's cycle is 20 frames (8 messages).
    // Record the first 7 frames of a cycle: station ID, UTC time (rendered with the
    // chain's GRI), almanac.
    let cycle = cycle_frames(&src);
    assert_eq!(cycle, 20);
    // high SNR: every message, no symbol errors
    let d = run(&src, cycle, 7, None);
    assert_eq!(d.ldc.len(), 2);
    for s in &d.ldc {
        assert_eq!(s.scheme, "eurofix");
        assert_eq!(s.groups_missing, 0);
        assert_eq!(s.first_frame % cycle, 0);
        assert_eq!(check(&d, s, "eurofix clean"), 0);
        let kinds: Vec<u8> = s.messages().map(|m| m.type_code()).collect();
        assert_eq!(kinds, vec![1, 2, 3], "{kinds:?}");
    }
    // moderate noise: symbol errors, all corrected by the RS code
    let d = run(&src, cycle, 7, Some(NOISE_EUROFIX));
    let errs: usize = d.ldc.iter().map(|s| check(&d, s, "eurofix noisy")).sum();
    assert!(errs > 0, "noise produced no symbol errors");
}

/// WGN (µV/m in 20 kHz) giving symbol errors (≈ 10 % at the weaker station) on the
/// Eurofix scenario.
const NOISE_EUROFIX: f64 = 1500.0;

#[test]
fn ninth_pulse_scenario_decodes_in_the_receiver_with_authentication() {
    let src = scenario("ldc-ninth-pulse.toml");
    // 24 GRIs (1.44 s) per frame; station ID, UTC time and an auth tag over both
    let cycle = cycle_frames(&src);
    for (noise, tag) in [(None, "9th clean"), (Some(NOISE_NINTH), "9th noisy")] {
        let d = run(&src, cycle, cycle, noise);
        assert_eq!(d.ldc.len(), 2);
        let mut errs = 0;
        for s in &d.ldc {
            assert_eq!(s.scheme, "ninth_pulse");
            let e = check(&d, s, tag);
            if noise.is_none() {
                assert_eq!(e, 0, "{tag}");
            }
            errs += e;
            let kinds: Vec<u8> = s.messages().map(|m| m.type_code()).collect();
            assert_eq!(kinds.len(), 3, "{kinds:?}");
            assert_eq!(s.auth.len(), 1);
            assert_eq!(s.auth[0].verified, Some(true), "{tag}");
        }
        if noise.is_some() {
            assert!(errs > 0, "noise produced no symbol errors");
        }
    }
}

/// WGN (µV/m in 20 kHz) giving symbol errors on the 9th-pulse scenario.
const NOISE_NINTH: f64 = 600.0;
