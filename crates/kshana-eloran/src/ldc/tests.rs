// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! End-to-end LDC tests: through `chain::emissions`, scheduling, authentication, BER.

use super::mapper::{MapperSpec, DATA_PULSE, LDC_PULSES};
use super::modulator::LdcModulator;
use super::msg::{DloranSpecEntry, Message, MessageSpec, WarningKind};
use super::rx::LdcDecoder;
use super::*;
use crate::chain::{emissions, Chain, Emission, Station, StationFault, StationStatus};
use crate::geo::Geo;
use crate::signal::{PulseSignal, Role};
use crate::time::{AbsTime, Ps};
use crate::timebase::{gauss, TimeRefModel, Timebase};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

const GRI: u32 = 99_300;

fn chain() -> Chain {
    let st = |id: &str, role, d| Station {
        id: id.into(),
        name: id.into(),
        role,
        position: Geo::new(36.0, 129.0),
        power_kw: 400.0,
        emission_delay_us: d,
        time_ref: "UTC(A)".into(),
        tx_clock: None,
        ecd_us: 0.0,
        status: StationStatus::On,
        faults: vec![],
    };
    Chain {
        name: "t".into(),
        gri_us: GRI,
        signal: PulseSignal::loran_c(),
        stations: vec![st("M", Role::Master, 0.0), st("W", Role::Secondary, 11_000.0), st("X", Role::Secondary, 40_000.0)],
    }
}

fn entry(m: MessageSpec) -> ScheduleEntry {
    ScheduleEntry { message: m, repeat: 1 }
}

fn common() -> LdcCommon {
    LdcCommon {
        schedule: vec![
            entry(MessageSpec::StationId { chain_id: 9, station_code: None, signal_version: 1 }),
            entry(MessageSpec::UtcTime { tai_minus_utc_s: 37, leap_pending: 0, days_to_leap: 0 }),
            entry(MessageSpec::Warning { station_code: b'X', kind: WarningKind::PropagationAnomaly, magnitude_ns: 85.0, valid_s: 300 }),
            entry(MessageSpec::Dloran {
                reference_site: 7,
                age_s: 4,
                corrections: vec![
                    DloranSpecEntry { station_code: b'M', correction_ns: -31.0 },
                    DloranSpecEntry { station_code: b'W', correction_ns: 12.0 },
                ],
            }),
            entry(MessageSpec::Auth { key_id: 1, tag_bytes: 8 }),
        ],
        gri_us: Some(GRI),
        auth_key_hex: Some("000102030405060708090a0b0c0d0e0f".into()),
        ..Default::default()
    }
}

/// Emissions of chain groups `[k0, k0 + n)` with a scene starting at group k0, grouped by
/// (station, group), with clock offsets and a timing fault to show they cancel.
fn group_emissions(c: &Chain, m: &dyn crate::signal::GroupModulator, k0: i64, n: i64) -> Vec<Vec<Emission>> {
    let mut r = TimeRefModel::ideal("UTC(A)");
    r.offset_ns = 37.0;
    let tb = Timebase::new(vec![r], 100.0);
    let gri = c.gri().0;
    let start = AbsTime(k0 as i128 * gri as i128);
    let em = emissions(c, 0, start, Ps::from_us(-500.0), Ps(gri * n), &tb, Some(m));
    let mut out: Vec<Vec<Emission>> = Vec::new();
    for g in k0..k0 + n {
        for s in 0..c.stations.len() {
            let v: Vec<Emission> = em.iter().filter(|e| e.group == g && e.station == s).copied().collect();
            if !v.is_empty() {
                out.push(v);
            }
        }
    }
    out
}

#[test]
fn eurofix_end_to_end_through_emissions() {
    let cfg = LdcConfig::eurofix(common());
    let m = build_modulator(&cfg).unwrap().unwrap();
    let dec = LdcDecoder::new(&cfg).unwrap().unwrap();
    assert_eq!(dec.groups_per_frame(), 30);
    assert_eq!(dec.codec.data_bits(), 56);
    let mut c = chain();
    c.stations[2].faults.push(StationFault::TimingStep { from_s: 0.0, to_s: 1e9, error_ns: 123.0 });
    // a frame-aligned start, several messages long
    let k0 = 30 * 700_000_000i64;
    let frames = 10;
    let groups = group_emissions(&c, &m, k0, 30 * frames);
    let mut rx: Vec<Vec<u32>> = vec![vec![]; 3];
    for g in &groups {
        let s = g[0].station;
        let role = c.stations[s].role;
        let k = g[0].group;
        let truth = m.symbol(s, role, k).unwrap();
        let (t, _) = dec.offsets_from_emissions(&c.signal, role, g);
        let (ideal, _) = m.mapper().ideal(truth);
        // pulses 1–2 unshifted, pulses 3–8 by the expected ±1 µs balanced pattern
        for i in 0..8 {
            assert!((t[i] - ideal[i]).abs() < 1e-6, "station {s} group {k} pulse {i}: {} vs {}", t[i], ideal[i]);
            assert!(t[i].abs() < 1e-6 || (t[i].abs() - 1000.0).abs() < 1e-6);
        }
        assert_eq!(ideal[..8].iter().sum::<f64>(), 0.0);
        assert!(ideal[2..8].iter().any(|x| *x != 0.0));
        assert!(t[DATA_PULSE].is_nan(), "eurofix adds no pulse");
        // the master ID pulse is not moved
        if role == Role::Master {
            let id = g.iter().find(|e| e.pulse.k == 8).unwrap();
            assert_eq!(id.pulse.offset, Ps::from_us(9000.0));
        }
        let d = dec.decide(&t, None);
        assert_eq!(d, truth);
        rx[s].push(d);
    }
    for (s, syms) in rx.iter().enumerate() {
        let role = c.stations[s].role;
        assert_eq!(syms.len(), 30 * frames as usize);
        let rep = dec.decode_symbols(syms);
        assert_eq!(rep.frames_failed, 0);
        assert_eq!(rep.symbols_corrected, 0);
        let f0 = k0 / 30;
        let truth: Vec<Message> = m.messages_starting(s, role, f0, f0 + frames).into_iter().map(|x| x.message).collect();
        // messages that complete inside the window are all recovered
        let got: Vec<Message> = rep.messages.iter().map(|x| x.message.clone()).collect();
        assert!(!got.is_empty());
        assert_eq!(got[..], truth[..got.len()]);
        assert!(matches!(got[0], Message::StationId { .. } | Message::UtcTime { .. } | Message::Warning { .. } | Message::Dloran { .. } | Message::Auth { .. }));
        // the log holds exactly what was modulated
        let logged = m.transmitted(s);
        assert!(logged.len() >= syms.len());
        for (g, sym) in logged.iter().filter(|(g, _)| *g >= k0 && *g < k0 + 30 * frames) {
            assert_eq!(Some(*sym), m.symbol(s, role, *g));
        }
    }
}

#[test]
fn ninth_pulse_end_to_end_through_emissions() {
    let cfg = LdcConfig::ninth_pulse(common());
    let m = build_modulator(&cfg).unwrap().unwrap();
    let dec = LdcDecoder::new(&cfg).unwrap().unwrap();
    assert_eq!(dec.bits_per_symbol(), 5);
    assert_eq!(dec.groups_per_frame(), 24);
    let c = chain();
    let k0 = 24 * 900_000_000i64;
    let frames = 12;
    let groups = group_emissions(&c, &m, k0, 24 * frames);
    let mut rx: Vec<Vec<u32>> = vec![vec![]; 3];
    for g in &groups {
        let s = g[0].station;
        let role = c.stations[s].role;
        let truth = m.symbol(s, role, g[0].group).unwrap();
        // one extra pulse: 10 for the master, 9 for secondaries
        assert_eq!(g.len(), if role == Role::Master { 10 } else { 9 });
        let data = g.iter().find(|e| e.pulse.k == if role == Role::Master { 9 } else { 8 }).unwrap();
        let p8 = g.iter().find(|e| e.pulse.k == 7).unwrap();
        let shift_us = (data.t_emit - p8.t_emit).as_us() - 1000.0;
        assert!(shift_us.abs() <= 15.5 && (shift_us.fract().abs() - 0.5).abs() < 1e-6, "{shift_us}");
        let (t, _) = dec.offsets_from_emissions(&c.signal, role, g);
        assert!(t[..8].iter().all(|x| x.abs() < 1e-6));
        assert!((t[DATA_PULSE] - shift_us * 1e3).abs() < 1e-3);
        let d = dec.decide(&t, None);
        assert_eq!(d, truth);
        rx[s].push(d);
    }
    for (s, syms) in rx.iter().enumerate() {
        let rep = dec.decode_symbols(syms);
        assert_eq!(rep.frames_failed, 0);
        let f0 = k0 / 24;
        let truth = m.messages_starting(s, c.stations[s].role, f0, f0 + frames);
        assert!(!rep.messages.is_empty());
        for (a, b) in rep.messages.iter().zip(&truth) {
            assert_eq!(a.message, b.message);
        }
        // auth messages verify with the right key
        let checks = dec.verify_auth(s, &rep, f0);
        for ch in checks.iter().filter(|c| c.verified.is_some()) {
            assert_eq!(ch.verified, Some(true));
        }
    }
}

#[test]
fn authentication_verifies_and_rejects() {
    let cfg = LdcConfig::eurofix(common());
    let m = build_modulator(&cfg).unwrap().unwrap();
    let dec = LdcDecoder::new(&cfg).unwrap().unwrap();
    let plan_frames = m.c.plans[0].cycle_frames;
    let f0 = plan_frames * 1000; // cycle start
    let syms = m.symbols(1, Role::Secondary, f0 * 30, (f0 + plan_frames) * 30);
    let rep = dec.decode_symbols(&syms);
    assert_eq!(rep.messages.len(), 5);
    let checks = dec.verify_auth(1, &rep, f0);
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].verified, Some(true));
    if let Message::Auth { covered, tag, .. } = &rep.messages[4].message {
        assert_eq!(*covered, 4);
        assert_eq!(tag.len(), 8);
    } else {
        panic!("last message is auth");
    }
    // wrong station index, wrong frame number (replay), wrong key all fail
    assert_eq!(dec.verify_auth(2, &rep, f0)[0].verified, Some(false));
    assert_eq!(dec.verify_auth(1, &rep, f0 + plan_frames)[0].verified, Some(false));
    let mut c2 = common();
    c2.auth_key_hex = Some("ff".into());
    let dec2 = LdcDecoder::new(&LdcConfig::eurofix(c2)).unwrap().unwrap();
    assert_eq!(dec2.verify_auth(1, &rep, f0)[0].verified, Some(false));
    // a different cycle has a different time message, hence a different tag
    let syms2 = m.symbols(1, Role::Secondary, (f0 + plan_frames) * 30, (f0 + 2 * plan_frames) * 30);
    let rep2 = dec.decode_symbols(&syms2);
    assert_ne!(rep.messages[4].message, rep2.messages[4].message);
    assert_eq!(dec.verify_auth(1, &rep2, f0 + plan_frames)[0].verified, Some(true));
}

#[test]
fn scheduler_round_robin_repeat_and_per_station() {
    let mut cm = LdcCommon {
        schedule: vec![
            ScheduleEntry { message: MessageSpec::StationId { chain_id: 1, station_code: None, signal_version: 0 }, repeat: 2 },
            entry(MessageSpec::Null),
        ],
        station_schedules: vec![StationSchedule {
            station: 2,
            schedule: vec![entry(MessageSpec::Raw { hex: "deadbeef".into() })],
        }],
        stations: Some(vec![0, 2]),
        ..Default::default()
    };
    let m = build_modulator(&LdcConfig::eurofix(cm.clone())).unwrap().unwrap();
    // station id: 4+38+24 = 66 bits → 2 frames of 56; null 28 bits → 1 frame; cycle 5 frames
    assert_eq!(m.c.plans[0].cycle_frames, 5);
    let msgs = m.messages_starting(0, Role::Master, 0, 10);
    let kinds: Vec<u8> = msgs.iter().map(|x| x.message.type_code()).collect();
    assert_eq!(kinds, vec![1, 1, 0, 1, 1, 0]);
    assert!(matches!(msgs[0].message, Message::StationId { station_code: b'A', secondary: false, .. }));
    let s2 = m.messages_starting(2, Role::Secondary, 0, 3);
    assert!(s2.iter().all(|x| x.message == Message::Raw { data: vec![0xde, 0xad, 0xbe, 0xef] }));
    // station 1 is not modulated
    assert_eq!(m.symbol(1, Role::Secondary, 5), None);
    let mut p = crate::signal::PulseSignal::loran_c().group(Role::Secondary, crate::signal::Parity::A);
    let p0 = p.clone();
    crate::signal::GroupModulator::modulate(&m, 1, Role::Secondary, 0, &mut p);
    assert_eq!(p, p0);
    // time messages without a GRI are rejected
    cm.schedule.push(entry(MessageSpec::UtcTime { tai_minus_utc_s: 37, leap_pending: 0, days_to_leap: 0 }));
    assert!(build_modulator(&LdcConfig::eurofix(cm)).is_err());
}

#[test]
fn generic_phase_and_ppm_schemes_roundtrip() {
    let mk = |mapper: MapperSpec, dp: Option<DataPulseSpec>| {
        LdcConfig::Generic(GenericLdc {
            mapper,
            fec: FecSpec { m: 4, n: 15, k: 9 },
            frame_crc: crc::FrameCrc::Crc16,
            data_pulse: dp,
            common: common(),
        })
    };
    let cfgs = [
        mk(MapperSpec::Phase { pulses: vec![6, 7], states: 4 }, None),
        mk(MapperSpec::Ppm { pulses: vec![4, 5], states: 4, step_us: 0.5 }, None),
        mk(MapperSpec::TriState { pulses: vec![3, 4, 5, 6], step_us: 1.0, balanced: false }, None),
        mk(MapperSpec::Ppm { pulses: vec![DATA_PULSE], states: 16, step_us: 2.0 }, Some(DataPulseSpec::default())),
    ];
    assert!(mk(MapperSpec::Ppm { pulses: vec![DATA_PULSE], states: 16, step_us: 2.0 }, None).compile().is_err());
    let c = chain();
    for cfg in &cfgs {
        let m = build_modulator(cfg).unwrap().unwrap();
        let dec = LdcDecoder::new(cfg).unwrap().unwrap();
        let g = dec.groups_per_frame() as i64;
        let k0 = g * 1_000_000;
        let n = g * 40;
        let groups = group_emissions(&c, &m, k0, n);
        let mut syms = vec![];
        for gr in groups.iter().filter(|gr| gr[0].station == 1) {
            let (t, p) = dec.offsets_from_emissions(&c.signal, Role::Secondary, gr);
            syms.push(dec.decide(&t, Some(&p)));
        }
        assert_eq!(syms, m.symbols(1, Role::Secondary, k0, k0 + n), "{}", m.c.name);
        let rep = dec.decode_symbols(&syms);
        assert_eq!(rep.frames_failed, 0);
        assert!(!rep.messages.is_empty());
    }
}

#[test]
fn align_finds_frame_boundary() {
    let cfg = LdcConfig::eurofix(common());
    let m = build_modulator(&cfg).unwrap().unwrap();
    let dec = LdcDecoder::new(&cfg).unwrap().unwrap();
    let k0 = 30 * 5000 + 17; // 17 groups into a frame
    let syms = m.symbols(0, Role::Master, k0, k0 + 30 * 8);
    assert_eq!(dec.align(&syms), Some(13));
    let rep = dec.decode_symbols(&syms[13..]);
    assert!(rep.frames_ok >= 7);
}

#[test]
fn toml_config_parses() {
    #[derive(serde::Deserialize)]
    struct W {
        ldc: LdcConfig,
    }
    let w: W = toml::from_str(
        r#"
ldc = { scheme = "eurofix", gri_us = 67310, auth_key_hex = "0b0b", schedule = [
  { type = "station_id", chain_id = 3 },
  { type = "utc_time", repeat = 2 },
  { type = "almanac", station_code = 77, gri_us = 67310, lat_deg = 54.9, lon_deg = -3.3, emission_delay_us = 0.0, power_kw = 250 },
  { type = "dgnss", sats = [ { prn = 5, prc_m = -3.2, rrc_m_s = 0.01 } ] },
  { type = "auth" },
] }
"#,
    )
    .unwrap();
    let LdcConfig::Eurofix(e) = &w.ldc else { panic!() };
    assert_eq!(e.rs_n, 30);
    assert_eq!(e.common.schedule[1].repeat, 2);
    assert_eq!(e.common.gri_us, Some(67310));
    build(&w.ldc).unwrap().unwrap();
    let w2: W = toml::from_str(r#"ldc = { scheme = "ninth_pulse", states = 16, step_us = 2.0, rs_n = 15, rs_k = 7 }"#).unwrap();
    let d = LdcDecoder::new(&w2.ldc).unwrap().unwrap();
    assert_eq!(d.bits_per_symbol(), 4);
    let w3: W = toml::from_str(
        r#"ldc = { scheme = "generic", mapper = { type = "tri_state", pulses = [2,3,4,5,6,7], step_us = 1.0, balanced = true }, fec = { m = 7, n = 30, k = 10 } }"#,
    )
    .unwrap();
    assert!(build(&w3.ldc).unwrap().is_some());
    let none: W = toml::from_str(r#"ldc = { scheme = "none" }"#).unwrap();
    assert!(build(&none.ldc).unwrap().is_none());
    // serialisation (scenario hashing) works
    serde_json::to_string(&w.ldc).unwrap();
}

/// One BER-table row.
#[derive(Debug)]
struct BerRow {
    sigma_ns: f64,
    symbols: usize,
    symbol_errors: usize,
    bit_errors: usize,
    frames: usize,
    frame_failures: usize,
    messages: usize,
    messages_lost: usize,
    residual_errors: usize,
}

/// Generate symbols, turn them into ideal pulse offsets plus N(0, σ²) timing noise on every
/// pulse (and a random common offset), decide, decode; compare with truth.
fn ber_row(cfg: &LdcConfig, sigma_ns: f64, frames: i64, seed: u64) -> BerRow {
    let m: LdcModulator = build_modulator(cfg).unwrap().unwrap();
    let dec = LdcDecoder::new(cfg).unwrap().unwrap();
    let g = dec.groups_per_frame() as i64;
    let f0 = 123_456;
    let k0 = f0 * g;
    let truth = m.symbols(1, Role::Secondary, k0, k0 + frames * g);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let rx: Vec<u32> = truth
        .iter()
        .map(|s| {
            let (ideal, _) = m.mapper().ideal(*s);
            let common = 500.0 * gauss(&mut rng);
            let x: Vec<f64> = (0..LDC_PULSES).map(|i| ideal[i] + common + sigma_ns * gauss(&mut rng)).collect();
            dec.decide(&x, None)
        })
        .collect();
    let symbol_errors = truth.iter().zip(&rx).filter(|(a, b)| a != b).count();
    let bit_errors = truth.iter().zip(&rx).map(|(a, c)| (a ^ c).count_ones() as usize).sum();
    let rep = dec.decode_symbols(&rx);
    let mut expect = m.messages_starting(1, Role::Secondary, f0, f0 + frames);
    expect.retain(|t| t.first_frame + t.frames <= f0 + frames);
    let mut lost = 0;
    let mut residual = 0;
    for t in &expect {
        match rep.messages.iter().find(|d| d.first_frame as i64 == t.first_frame - f0) {
            Some(d) if d.message == t.message => {}
            Some(_) => residual += 1,
            None => lost += 1,
        }
    }
    BerRow {
        sigma_ns,
        symbols: truth.len(),
        symbol_errors,
        bit_errors,
        frames: rep.frames.len(),
        frame_failures: rep.frames_failed,
        messages: expect.len(),
        messages_lost: lost,
        residual_errors: residual,
    }
}

fn ber_table(name: &str, cfg: &LdcConfig, sigmas: &[f64], frames: i64) -> Vec<BerRow> {
    let b = LdcDecoder::new(cfg).unwrap().unwrap().bits_per_symbol() as f64;
    let mut t = format!("\nLDC BER table: {name} (Gaussian timing noise σ on every pulse)\n");
    t += &format!("{:>8} {:>8} {:>10} {:>10} {:>10} {:>10} {:>9}\n", "σ (ns)", "symbols", "SER", "BER(raw)", "FER", "MER", "residual");
    let rows: Vec<BerRow> = sigmas.iter().enumerate().map(|(i, s)| ber_row(cfg, *s, frames, 1000 + i as u64)).collect();
    for r in &rows {
        t += &format!(
            "{:>8.0} {:>8} {:>10.2e} {:>10.2e} {:>10.2e} {:>10.2e} {:>9}\n",
            r.sigma_ns,
            r.symbols,
            r.symbol_errors as f64 / r.symbols as f64,
            r.bit_errors as f64 / (r.symbols as f64 * b),
            r.frame_failures as f64 / r.frames as f64,
            r.messages_lost as f64 / r.messages as f64,
            r.residual_errors
        );
    }
    t += "SER/BER: raw channel symbol/bit error rates before FEC; FER: RS/frame-CRC failures; MER: messages lost after FEC + CRC-24Q; residual: wrong messages accepted\n";
    println!("{t}");
    rows
}

fn assert_monotonic(rows: &[BerRow]) {
    for w in rows.windows(2) {
        assert!(w[1].symbol_errors >= w[0].symbol_errors, "SER not monotonic: {:?} → {:?}", w[0], w[1]);
        assert!(w[1].bit_errors >= w[0].bit_errors, "BER not monotonic: {:?} → {:?}", w[0], w[1]);
        assert!(w[1].messages_lost + 1 >= w[0].messages_lost, "MER not monotonic: {:?} → {:?}", w[0], w[1]);
    }
}

#[test]
fn ber_vs_timing_noise_eurofix() {
    let cfg = LdcConfig::eurofix(common());
    let sigmas = [0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 500.0, 700.0];
    let rows = ber_table("eurofix", &cfg, &sigmas, 150);
    assert_monotonic(&rows);
    for r in rows.iter().filter(|r| r.sigma_ns <= 100.0) {
        assert_eq!((r.symbol_errors, r.frame_failures, r.messages_lost), (0, 0, 0), "{r:?}");
    }
    // moderate noise: raw symbol errors are frequent but RS(30,10) corrects them all
    let r300 = rows.iter().find(|r| r.sigma_ns == 300.0).unwrap();
    assert!(r300.symbol_errors as f64 / r300.symbols as f64 > 0.01, "{r300:?}");
    assert_eq!((r300.frame_failures, r300.messages_lost), (0, 0), "{r300:?}");
    // heavy noise overwhelms the code; nothing wrong is ever accepted
    let last = rows.last().unwrap();
    assert!(last.frame_failures > last.frames / 2, "{last:?}");
    assert!(rows.iter().all(|r| r.residual_errors == 0));
}

#[test]
fn ber_vs_timing_noise_ninth_pulse() {
    let cfg = LdcConfig::ninth_pulse(common());
    let sigmas = [0.0, 25.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 400.0, 600.0];
    let rows = ber_table("ninth_pulse", &cfg, &sigmas, 200);
    assert_monotonic(&rows);
    for r in rows.iter().filter(|r| r.sigma_ns <= 50.0) {
        assert_eq!((r.symbol_errors, r.frame_failures, r.messages_lost), (0, 0, 0), "{r:?}");
    }
    let r200 = rows.iter().find(|r| r.sigma_ns == 200.0).unwrap();
    assert!(r200.symbol_errors > 0, "{r200:?}");
    assert_eq!((r200.frame_failures, r200.messages_lost), (0, 0), "{r200:?}");
    let last = rows.last().unwrap();
    assert!(last.frame_failures > last.frames / 2, "{last:?}");
    assert!(rows.iter().all(|r| r.residual_errors == 0));
}
