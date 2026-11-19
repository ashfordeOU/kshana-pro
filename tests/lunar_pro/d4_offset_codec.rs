// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// D4: KLTO-1 broadcast offset codec (a proposal pending LNIS AD5, not a standard). Part of the
// tests/lunar_pro.rs test binary.

use kshana_pro::lunar_pro::offset_codec::*;
use kshana_pro::lunar_pro::time::{parse_iso, Model, Scale};
use rand::{Rng, SeedableRng};

const VECTORS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/lunar_pro/klto1_conformance_vectors.json"
);

fn random_message(rng: &mut rand_chacha::ChaCha20Rng) -> Message {
    Message {
        version: 1,
        scale: if rng.gen() {
            BroadcastScale::Tl
        } else {
            BroadcastScale::Tcl
        },
        reference: rng.gen_range(0..16),
        iod: rng.gen(),
        wn_ot: rng.gen_range(0..8192),
        t_ot: rng.gen_range(0..37_800),
        n: rng.gen(),
        a0: rng.gen(),
        a1: rng.gen_range(-(1 << 23)..(1 << 23)),
        a2: rng.gen_range(-(1 << 19)..(1 << 19)),
        validity: rng.gen_range(0..9),
        uclass: rng.gen_range(0..16),
    }
}

#[test]
fn d4_encode_decode_round_trips_every_field() {
    let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(5);
    for _ in 0..5_000 {
        let m = random_message(&mut rng);
        let b = m.encode().unwrap();
        assert_eq!(b.len(), BYTES);
        // The five pad bits are zero.
        assert_eq!(b[BYTES - 1] & 0x1f, 0);
        assert_eq!(Message::decode(&b).unwrap(), m);
    }
}

#[test]
fn d4_quantisation_stays_inside_half_an_lsb_and_the_stated_bound() {
    let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(9);
    for _ in 0..5_000 {
        let p = Physical {
            scale: BroadcastScale::Tl,
            reference: 0,
            iod: 1,
            wn_ot: 46,
            t_ot_s: 16.0 * rng.gen_range(0..37_800) as f64,
            offset_s: rng.gen_range(-100.0..100.0),
            rate: rng.gen_range(-7e-9..7e-9),
            rate_change: rng.gen_range(-2.8e-14..2.8e-14),
            validity_s: VALIDITY_S[rng.gen_range(0..9)],
            u_bound_ns: Some(rng.gen_range(0.1..2e6)),
        };
        let m = p.quantise().unwrap();
        let q = m.physical();
        assert!((q.offset_s - p.offset_s).abs() <= A0_LSB / 2.0 + 1e-13);
        assert!((q.rate - p.rate).abs() <= A1_LSB / 2.0 * (1.0 + 1e-9));
        assert!((q.rate_change - p.rate_change).abs() <= A2_LSB / 2.0 * (1.0 + 1e-9));
        assert!(q.u_bound_ns.unwrap() >= p.u_bound_ns.unwrap() || m.uclass == 14);
        for f in [-0.5, -0.1, 0.0, 0.3, 0.5] {
            let dt = f * p.validity_s;
            let exact = p.offset_s + p.rate * dt + p.rate_change * dt * dt;
            assert!((m.offset_at(dt) - exact).abs() <= Message::quantisation_bound(dt) + 1e-13);
        }
    }
    // a0 carry: an offset exactly half-way rounds without leaving the 32-bit field.
    for off in [-70.5, 70.5, -0.5, 0.5] {
        let p = Physical {
            scale: BroadcastScale::Tl,
            reference: 0,
            iod: 0,
            wn_ot: 0,
            t_ot_s: 0.0,
            offset_s: off,
            rate: 0.0,
            rate_change: 0.0,
            validity_s: 3_600.0,
            u_bound_ns: None,
        };
        let m = p.quantise().unwrap();
        assert!((m.offset_at(0.0) - off).abs() <= A0_LSB, "{off}");
        assert_eq!(m.uclass, 15);
    }
}

#[test]
fn d4_out_of_range_and_malformed_input_is_refused() {
    let p = Physical {
        scale: BroadcastScale::Tl,
        reference: 0,
        iod: 0,
        wn_ot: 0,
        t_ot_s: 0.0,
        offset_s: 70.2,
        rate: 6.48e-10,
        rate_change: 0.0,
        validity_s: 86_400.0,
        u_bound_ns: Some(5.0),
    };
    assert!(Physical { rate: 1e-8, ..p }.quantise().is_err());
    assert!(Physical {
        rate_change: 1e-13,
        ..p
    }
    .quantise()
    .is_err());
    assert!(Physical { t_ot_s: 17.0, ..p }.quantise().is_err());
    assert!(Physical {
        t_ot_s: 604_800.0,
        ..p
    }
    .quantise()
    .is_err());
    assert!(Physical {
        validity_s: 5_000.0,
        ..p
    }
    .quantise()
    .is_err());
    assert!(Physical {
        offset_s: 40_000.0,
        ..p
    }
    .quantise()
    .is_err());
    let good = p.quantise().unwrap().encode().unwrap();
    assert!(Message::decode(&good[..18]).is_err());
    let mut pad = good;
    pad[BYTES - 1] |= 0x01;
    assert!(Message::decode(&pad).is_err());
    let mut reserved = good;
    reserved[0] = (reserved[0] & 0xf3) | 0x08; // scale code 2
    assert!(Message::decode(&reserved).is_err());
    let mut ver = good;
    ver[0] = (ver[0] & 0x0f) | 0x20; // version 2
    assert!(Message::decode(&ver).is_err());
    assert!(week_and_tow(parse_iso("2025-12-31T00:00:00").unwrap()).is_err());
}

#[test]
fn d4_fit_from_the_d1_model_holds_its_class_bound_over_the_window() {
    let model = Model::default_model();
    let centre = parse_iso("2027-03-10T05:00:00").unwrap();
    for (scale, lunar) in [
        (BroadcastScale::Tl, Scale::Tl),
        (BroadcastScale::Tcl, Scale::Tcl),
    ] {
        for vi in [0usize, 5, 8] {
            let f = fit(&model, scale, centre, vi, 9, 0).unwrap();
            let m = f.message;
            let v = VALIDITY_S[vi];
            let bound_s = f.physical.u_bound_ns.unwrap() * 1e-9;
            assert!(bound_s >= f.bound_s);
            let (wn, tow) = (m.wn_ot as f64, m.t_ot as f64 * T_OT_LSB_S);
            let t_ot =
                kshana::timescales::TwoPartJd::from_f64(EPOCH_JD).add_seconds(wn * WEEK_S + tow);
            let mut worst: f64 = 0.0;
            for k in 0..=100 {
                let dt = v * (k as f64 / 100.0 - 0.5);
                let truth = -model
                    .convert(lunar, t_ot.add_seconds(dt), Scale::Utc)
                    .offset
                    .value();
                worst = worst.max((m.offset_at(dt) - truth).abs());
            }
            // The broadcast reproduces the model inside the residual it reported (plus
            // inter-sample slack) and therefore inside its class bound.
            assert!(
                worst <= f.max_residual_s * 1.5 + 1e-9,
                "{scale:?} {vi}: {worst:e} vs {:e}",
                f.max_residual_s
            );
            assert!(worst <= bound_s);
            // TL − UTC rate ≈ 6.48e-10, TCL − UTC ≈ 6.80e-10 (plus the monthly modulation).
            let rate = m.a1 as f64 * A1_LSB;
            let want = if scale == BroadcastScale::Tl {
                6.484e-10
            } else {
                6.798e-10
            };
            assert!((rate - want).abs() < 4e-10 * 0.25 + 3.5e-10, "{rate:e}");
        }
    }
    // Over a week the monthly term's curvature needs a2: dropping it costs microseconds.
    let f = fit(&model, BroadcastScale::Tl, centre, 8, 0, 0).unwrap();
    let no_a2 = Message { a2: 0, ..f.message };
    let dt = VALIDITY_S[8] / 2.0;
    assert!((no_a2.offset_at(dt) - f.message.offset_at(dt)).abs() > 1e-6);
}

#[test]
fn d4_conformance_vectors_are_self_consistent_and_reproducible() {
    if std::env::var("KSHANA_REGEN_KLTO1").is_ok() {
        let v = conformance_vectors().unwrap();
        std::fs::create_dir_all(std::path::Path::new(VECTORS).parent().unwrap()).unwrap();
        std::fs::write(VECTORS, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
    }
    let file: VectorFile =
        serde_json::from_str(&std::fs::read_to_string(VECTORS).unwrap()).unwrap();
    assert!(file.status.contains("PROPOSAL") && file.status.contains("AD5"));
    assert_eq!(file.bits, BITS);
    assert!(file.vectors.len() >= 5);
    for v in &file.vectors {
        let bytes = hex::decode(&v.hex).unwrap();
        assert_eq!(Message::decode(&bytes).unwrap(), v.message, "{}", v.name);
        assert_eq!(to_hex(&v.message.encode().unwrap()), v.hex, "{}", v.name);
        for &(dt, off) in &v.eval {
            assert!((v.message.offset_at(dt) - off).abs() < 1e-12, "{}", v.name);
        }
    }
    // Regenerating gives the same vectors (the D1-fitted ones within one LSB of each
    // coefficient, to allow last-bit floating-point differences between platforms).
    let now = conformance_vectors().unwrap();
    for (a, b) in now.vectors.iter().zip(&file.vectors) {
        assert_eq!(a.name, b.name);
        if a.name.starts_with("d1-") {
            let (x, y) = (a.message, b.message);
            assert_eq!(
                (x.n, x.wn_ot, x.t_ot, x.uclass),
                (y.n, y.wn_ot, y.t_ot, y.uclass)
            );
            assert!(
                (x.a0 - y.a0).abs() <= 1 && (x.a1 - y.a1).abs() <= 1 && (x.a2 - y.a2).abs() <= 1
            );
        } else {
            assert_eq!(a, b);
        }
    }
}
