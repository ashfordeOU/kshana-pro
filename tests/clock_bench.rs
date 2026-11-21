// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Neutral clock benchmarks under a pre-registered protocol (E33 B3302), on synthetic logs.

use std::path::Path;

use kshana::clock_log::ClockLogInput;
use kshana_pro::timing::clock_bench::{self as cb, BenchSpec, ClockSpec, Protocol, Segment};

const UTC: &str = "2026-11-20T12:00:00Z";

/// A small deterministic generator (xorshift) and Box-Muller normal draws.
struct Rng(u64);
impl Rng {
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
    }
}

/// A TimeLab single-column phase log (seconds, tau0 1 s) of white FM with Allan deviation
/// `adev1` at 1 s, plus a constant frequency offset `y`.
fn white_fm_log(seed: u64, n: usize, adev1: f64, y: f64) -> String {
    let mut r = Rng(seed);
    let mut x = 0.0;
    let mut o = String::from("# tau0 = 1\n# units = s\n");
    for _ in 0..n {
        o.push_str(&format!("{x:e}\n"));
        x += adev1 * r.normal() + y;
    }
    o
}

fn seg(phase: &str, text: String) -> Segment {
    Segment {
        phase: phase.into(),
        log: ClockLogInput {
            format: "timelab".into(),
            text: Some(text),
            ..Default::default()
        },
    }
}

fn protocol() -> Protocol {
    Protocol {
        id: "synthetic-bench".into(),
        title: "Synthetic two-clock benchmark".into(),
        registered_utc: "2026-11-01T00:00:00Z".into(),
        reference: "synthetic noiseless reference".into(),
        reference_adev: vec![(1.0, 1e-13), (1000.0, 1e-15)],
        taus: vec![1.0, 10.0, 100.0],
        confidence: 0.95,
        floor_factor: 3.0,
        holdover_train_s: 2000.0,
        holdover_window_s: 1000.0,
        holdover_threshold_s: 1e-7,
        phases: vec!["baseline".into(), "temperature".into()],
        metrics: cb::METRICS.iter().map(|m| m.to_string()).collect(),
    }
}

fn spec() -> BenchSpec {
    let p = protocol();
    BenchSpec {
        protocol_sha256: cb::protocol_sha256(&p),
        protocol: p,
        clock: vec![
            ClockSpec {
                id: "clock-a".into(),
                technology: "csac".into(),
                datasheet: "synthetic claim".into(),
                datasheet_adev: vec![(1.0, 3e-10), (10.0, 1e-10)],
                collected_from_utc: Some("2026-11-02T00:00:00Z".into()),
                segment: vec![
                    seg("baseline", white_fm_log(1, 4000, 3e-10, 0.0)),
                    seg("temperature", white_fm_log(2, 4000, 3e-10, 2e-10)),
                ],
            },
            ClockSpec {
                id: "clock-b".into(),
                technology: "optical".into(),
                datasheet: String::new(),
                datasheet_adev: vec![],
                collected_from_utc: Some("2026-10-01T00:00:00Z".into()),
                // Better than the reference at 1 s: reference-limited there.
                segment: vec![seg("baseline", white_fm_log(3, 4000, 5e-14, 0.0))],
            },
        ],
    }
}

#[test]
fn white_fm_adev_is_recovered_within_its_interval() {
    let r = cb::score(&spec(), Path::new("."), UTC).unwrap();
    let a = &r.clocks[0].phases[0];
    for row in &a.taus {
        let ci = row.adev.as_ref().unwrap();
        let truth = 3e-10 / row.tau_s.sqrt();
        assert!(ci.lo <= truth && truth <= ci.hi, "{row:?}");
        assert!(!row.reference_limited);
        // TDEV of white FM: tau/sqrt(3) * MDEV, MDEV = ADEV/sqrt(2) at large m.
        assert!(row.tdev_s.unwrap() > 0.0 && row.mtie_s.unwrap() > 0.0);
    }
}

#[test]
fn points_below_the_reference_floor_are_flagged_and_withheld() {
    let r = cb::score(&spec(), Path::new("."), UTC).unwrap();
    let b = &r.clocks[1].phases[0];
    assert!(b.taus[0].reference_limited);
    let row = r
        .table
        .iter()
        .find(|t| t.metric == "adev" && t.phase == "baseline" && t.tau_s == Some(1.0))
        .unwrap();
    assert!(row.values[0].1.is_some());
    assert_eq!(row.values[1], ("clock-b".to_string(), None));
}

#[test]
fn a_changed_protocol_is_refused() {
    let mut s = spec();
    s.protocol.taus.push(1000.0);
    let e = cb::score(&s, Path::new("."), UTC).unwrap_err();
    assert!(e.contains("changed after registration"), "{e}");
    // The hash is stable and depends on every field.
    let p = protocol();
    assert_eq!(cb::protocol_sha256(&p), cb::protocol_sha256(&p.clone()));
    let mut q = p.clone();
    q.floor_factor = 2.0;
    assert_ne!(cb::protocol_sha256(&p), cb::protocol_sha256(&q));
}

#[test]
fn registration_order_is_reported_as_stated() {
    let r = cb::score(&spec(), Path::new("."), UTC).unwrap();
    assert_eq!(r.clocks[0].collected_after_registration, Some(true));
    assert_eq!(r.clocks[1].collected_after_registration, Some(false));
}

#[test]
fn environmental_phase_shows_the_frequency_shift() {
    let r = cb::score(&spec(), Path::new("."), UTC).unwrap();
    let t = &r.clocks[0].phases[1];
    let shift = t.freq_shift_from_baseline.unwrap();
    // 2e-10 injected; the white FM adds about 3e-10/sqrt(4000) of noise to each mean.
    assert!((shift - 2e-10).abs() < 3e-11, "{shift}");
    for ratio in &t.adev_ratio_to_baseline {
        let x = ratio.unwrap();
        assert!(x > 0.7 && x < 1.4, "{x}");
    }
}

#[test]
fn holdover_test_and_claims() {
    let r = cb::score(&spec(), Path::new("."), UTC).unwrap();
    let h = r.clocks[0].phases[0].holdover.clone().unwrap();
    // White FM of 3e-10 over 1000 s: prediction error of order 3e-10 * sqrt(1000) ~ 1e-8 s.
    assert!(h.max_error_s > 1e-9 && h.max_error_s < 1e-7, "{h:?}");
    let c = &r.clocks[0].claims;
    assert_eq!(c.len(), 2);
    for k in c {
        let ratio = k.ratio.unwrap();
        assert!(ratio > 0.8 && ratio < 1.2, "{k:?}");
    }
}

#[test]
fn phases_outside_the_protocol_are_refused() {
    let mut s = spec();
    s.clock[0]
        .segment
        .push(seg("vibration", white_fm_log(9, 100, 1e-10, 0.0)));
    assert!(cb::score(&s, Path::new("."), UTC).is_err());
    let mut p = protocol();
    p.phases = vec!["temperature".into()];
    assert!(cb::validate_protocol(&p).is_err());
    let mut p = protocol();
    p.metrics.push("rank".into());
    assert!(cb::validate_protocol(&p).is_err());
}

#[test]
fn outputs_list_clocks_in_input_order_without_a_ranking() {
    let r = cb::score(&spec(), Path::new("."), UTC).unwrap();
    let csv = cb::csv(&r);
    assert!(csv.starts_with("metric,phase,tau_s,clock-a,clock-b\n"));
    let html = cb::html(&r);
    for w in ["winner", "best clock", "certified", "compliant"] {
        assert!(!html.to_lowercase().contains(w), "{w}");
    }
    let dir = std::env::temp_dir().join(format!("ks-clock-bench-{}", std::process::id()));
    cb::write_dir(&dir, &r, "# bench").unwrap();
    for f in [
        "scorecard.json",
        "scorecard.html",
        "table.csv",
        "protocol.json",
    ] {
        assert!(dir.join(f).exists(), "{f}");
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("protocol.json")).unwrap(),
        cb::canonical_protocol(&r.protocol)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_example_request_scores() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/clock-bench");
    let src = std::fs::read_to_string(root.join("bench.toml")).unwrap();
    let s: BenchSpec = toml::from_str(&src).unwrap();
    let r = cb::score(&s, &root, UTC).unwrap();
    assert_eq!(r.clocks.len(), 3);
}

// ---------------------------------------------------------------------------------------------
// Digital-twin recalibration evidence pack on the hosted calibrate verdict (mock service).

mod twin_recal {
    use ed25519_dalek::{Signer, SigningKey};
    use kshana_pro::remote_verdict::{SignedVerdict, VerdictTransport};
    use kshana_pro::twin_recal as tr;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::cell::RefCell;

    const SEED: [u8; 32] = *b"twin-recal-mock-service-key-0001";

    fn pk() -> String {
        hex::encode(SigningKey::from_bytes(&SEED).verifying_key().to_bytes())
    }

    /// A stand-in for the hosted service: it answers each calibrate request with a verdict
    /// signed as the service signs one. Its "fit" is test code (a straight-line least squares
    /// for the clock), not part of the Pro build.
    struct Mock {
        seed: [u8; 32],
        seen: RefCell<Vec<Value>>,
    }

    impl VerdictTransport for Mock {
        fn post(&self, path: &str, _bearer: &str, body: &str) -> Result<String, String> {
            assert_eq!(path, "/verdict/calibrate");
            let b: Value = serde_json::from_str(body).unwrap();
            self.seen.borrow_mut().push(b.clone());
            let obs = b["observations"].as_array().unwrap();
            let pts: Vec<(f64, f64)> = obs
                .iter()
                .map(|o| (o["t_s"].as_f64().unwrap(), o["value"].as_f64().unwrap()))
                .collect();
            let n = pts.len() as f64;
            let (sx, sy) = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
            let (mx, my) = (sx / n, sy / n);
            let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
            let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
            let y = sxy / sxx;
            let x0 = my - y * mx;
            let verdict = json!({
                "converged": true,
                "posterior": [
                    {"name": "x0", "value": x0, "std_err": 2.0 / n.sqrt()},
                    {"name": "y", "value": y, "std_err": 2.0 / sxx.sqrt()},
                    {"name": "d", "value": 0.0, "std_err": 1e-9},
                ],
            });
            let mut v = SignedVerdict {
                service: "verdict-service/1".into(),
                method: "calibrate".into(),
                evidence_class: "MODELLED".into(),
                utc: "2026-11-21T00:00:00Z".into(),
                key_id: "k1".into(),
                verdict,
                verdict_sha256: String::new(),
                signature: String::new(),
            };
            let canon = serde_json::to_vec(&json!({
                "service": v.service, "method": v.method, "evidence_class": v.evidence_class,
                "utc": v.utc, "key_id": v.key_id, "verdict": v.verdict,
            }))
            .unwrap();
            v.verdict_sha256 = hex::encode(Sha256::digest(&canon));
            v.signature = hex::encode(SigningKey::from_bytes(&self.seed).sign(&canon).to_bytes());
            Ok(serde_json::to_string(&v).unwrap())
        }
    }

    #[test]
    fn windows_are_calibrated_by_the_service_and_drift_is_found() {
        let (csv, req) = tr::synth_clock();
        let q: tr::Request = toml::from_str(&req).unwrap();
        let m = Mock {
            seed: SEED,
            seen: RefCell::new(vec![]),
        };
        let r = tr::run(&q, &csv, &m, "tok", &pk()).unwrap();
        assert_eq!(r.pack.windows.len(), 4);
        assert_eq!(r.exchanges.len(), 4);
        // The request carries the stated model as an expression and the window's observations.
        let first = &m.seen.borrow()[0];
        assert_eq!(first["model"]["kind"], "expression");
        assert_eq!(first["observations"].as_array().unwrap().len(), 144);
        for w in &r.pack.windows {
            assert!(w.rms_after < w.rms_before, "{w:?}");
            assert!(w.rms_after < 3.0, "noise is 2 ns: {w:?}");
        }
        // The frequency grows 0.0005 ns/s per day: the drift of y is found and is significant.
        let y = r.pack.drift.iter().find(|d| d.parameter == "y").unwrap();
        assert!((y.slope_per_day.unwrap() - 0.0005).abs() < 0.0001, "{y:?}");
        assert_eq!(y.significant, Some(true));
        assert!(tr::EVIDENCE.contains("hosted calibrate verdict"));
    }

    #[test]
    fn a_verdict_from_another_key_is_refused() {
        let (csv, req) = tr::synth_clock();
        let q: tr::Request = toml::from_str(&req).unwrap();
        let m = Mock {
            seed: [9u8; 32],
            seen: RefCell::new(vec![]),
        };
        let e = tr::run(&q, &csv, &m, "tok", &pk()).err().unwrap();
        assert!(e.contains("does not verify"), "{e}");
    }

    #[test]
    fn orbit_decay_request_is_a_design_of_local_runs() {
        let q = tr::Request {
            title: "t".into(),
            data_source: "SYNTHETIC".into(),
            observable: "altitude_km".into(),
            telemetry: String::new(),
            window_s: 86_400.0,
            model: tr::Model::OrbitDecay {
                altitude0_km: 300.0,
                b_design: vec![0.01, 0.02, 0.03],
            },
            parameters: vec![],
            drift_k: 3.0,
        };
        let body = tr::request_body(&q, &[(0.0, 300.0, 0.1), (43_200.0, 299.0, 0.1)]).unwrap();
        let runs = body["model"]["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 3);
        // Larger b decays faster in the local US76 model.
        let last = |i: usize| {
            runs[i]["predictions"]["altitude_km"][1][1]
                .as_f64()
                .unwrap()
        };
        assert!(last(0) > last(1) && last(1) > last(2));
    }
}
