// PROPRIETARY — Kshana Pro.
use kshana_eloran::scenario::Scenario;
use kshana_eloran::scene::MemSink;
use sha2::{Digest, Sha256};

const ONE: &str = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.2
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
offset_ns = OFFSET
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.0, lon_deg = 0.0 }
time_ref = "UTC(A)"
"#;

fn run(offset_ns: f64) -> (Vec<f64>, Vec<kshana_eloran::TruthRecord>) {
    let src = ONE.replace("OFFSET", &format!("{offset_ns:.3}"));
    let sc = Scenario::from_toml(&src).unwrap();
    let mut scene = sc.build().unwrap();
    let mut sink = MemSink::default();
    let mut truth = vec![];
    scene.run(&mut sink, &mut truth).unwrap();
    (sink.channels.remove(0), truth)
}

/// Positive-going zero crossing nearest `t_guess` (s), refined with a 4-point cubic
/// (Lagrange) interpolant and Newton iteration (error well under 0.01 ns at 2 MS/s).
fn zero_crossing(x: &[f64], fs: f64, t_guess: f64) -> f64 {
    let c = (t_guess * fs).round() as usize;
    for d in 0..20usize {
        for i in [c + d, c.saturating_sub(d)] {
            if i >= 1 && i + 2 < x.len() && x[i] <= 0.0 && x[i + 1] > 0.0 {
                let y = [x[i - 1], x[i], x[i + 1], x[i + 2]];
                let nodes = [-1.0, 0.0, 1.0, 2.0];
                let f = |u: f64| -> (f64, f64) {
                    let mut v = 0.0;
                    let mut dv = 0.0;
                    for j in 0..4 {
                        let mut l = 1.0;
                        let mut dl = 0.0;
                        for m in 0..4 {
                            if m == j {
                                continue;
                            }
                            let den = nodes[j] - nodes[m];
                            dl = dl * (u - nodes[m]) / den + l / den;
                            l *= (u - nodes[m]) / den;
                        }
                        v += y[j] * l;
                        dv += y[j] * dl;
                    }
                    (v, dv)
                };
                let mut u = x[i] / (x[i] - x[i + 1]);
                for _ in 0..20 {
                    let (v, dv) = f(u);
                    u -= v / dv;
                }
                return (i as f64 + u) / fs;
            }
        }
    }
    panic!("no crossing near {t_guess}");
}

#[test]
fn pulse_arrives_at_truth_toa_with_30us_zero_crossing() {
    let (x, truth) = run(0.0);
    assert!(!truth.is_empty());
    let r = &truth[0];
    // distance ≈ 111 km; delay = d·n/c
    assert!((r.distance_km - 111.3).abs() < 0.5, "{}", r.distance_km);
    let zc = zero_crossing(&x, 2e6, r.toa_s + 30e-6);
    let err_ns = (zc - (r.toa_s + 30e-6)) * 1e9;
    // linear interpolation of a 100 kHz sine at 2 MS/s is accurate to well under 1 ns
    assert!(err_ns.abs() < 0.05, "zero-crossing error {err_ns} ns");
}

#[test]
fn ten_ns_reference_offset_moves_the_signal_by_ten_ns() {
    let (x0, t0) = run(0.0);
    let (x1, t1) = run(-10.0); // UTC(A) 10 ns behind → emission 10 ns later
    let a = zero_crossing(&x0, 2e6, t0[0].toa_s + 30e-6);
    let b = zero_crossing(&x1, 2e6, t0[0].toa_s + 30e-6);
    let d_ns = (b - a) * 1e9;
    assert!((d_ns - 10.0).abs() < 0.05, "shift {d_ns} ns");
    assert!(((t1[0].toa_s - t0[0].toa_s) * 1e9 - 10.0).abs() < 0.01);
}

#[test]
fn runs_are_bit_for_bit_deterministic() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scenarios/synthetic-two-chain.toml"
    ))
    .unwrap();
    let h = |_: ()| {
        let (sink, truth, stats) = kshana_eloran::run_to_memory(&src).unwrap();
        assert_eq!(stats.channels, 2);
        assert!(stats.truth_records > 0);
        let mut hs = Sha256::new();
        for c in &sink.channels {
            for v in c {
                hs.update(v.to_le_bytes());
            }
        }
        (hex::encode(hs.finalize()), truth.len())
    };
    assert_eq!(h(()), h(()));
}

#[test]
fn two_chains_two_time_refs_appear_in_truth() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scenarios/synthetic-two-chain.toml"
    ))
    .unwrap();
    let (_s, truth, stats) = kshana_eloran::run_to_memory(&src).unwrap();
    let chains: std::collections::BTreeSet<_> = truth.iter().map(|r| r.chain.clone()).collect();
    assert_eq!(chains.len(), 2);
    let b = truth.iter().find(|r| r.chain == "chain-B").unwrap();
    assert!((b.time_ref_offset_ns + 12.0).abs() < 0.1);
    assert!((b.emission_error_ns - 12.0).abs() < 0.1);
    assert!(stats.realtime_factor > 0.0);
    let sc = Scenario::from_toml(&src).unwrap();
    assert_eq!(sc.hash().len(), 64);
}
