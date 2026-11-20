// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Lunar surface operations evidence, second B2404 slice (research 2026-11-20 gaps E24) on
//! the SYNTHETIC inputs of `kshana_pro::lunar_surface::synth`: the parametric plume model
//! (geometry, linearity in the erosion rate, the Rayleigh RF limit, required parameters),
//! traverse safety evidence (sigma honesty, hazard margin, walk-back breach), and RF
//! self-interference forensics (spurs, harmonic match, time correlation); the CLI refuses
//! without a licence and its signed directory verifies.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::lunar_surface::{plume, rf, synth, traverse};

fn inputs(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b2404-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    synth::write(&d).unwrap();
    d
}

fn req<T: serde::de::DeserializeOwned>(d: &Path, f: &str) -> T {
    toml::from_str(&std::fs::read_to_string(d.join(f)).unwrap()).unwrap()
}

#[test]
fn rayleigh_efficiency_matches_the_closed_form() {
    let (q, x) = plume::rayleigh_q(10e-6, 2.0e9, 3.0, 0.03);
    let x_hand = 2.0 * std::f64::consts::PI * 10e-6 * 2.0e9 / 299_792_458.0;
    assert!((x - x_hand).abs() < 1e-15);
    // Absorption 12·x·ε″/((ε′+2)² + ε″²) dominates at this size.
    let q_abs = 12.0 * x * 0.03 / (25.0 + 0.0009);
    assert!((q - q_abs).abs() / q_abs < 1e-6, "{q} vs {q_abs}");
    // A lossless particle only scatters: Q ∝ x⁴.
    let (q0, _) = plume::rayleigh_q(10e-6, 2.0e9, 3.0, 0.0);
    let k = 2.0 / 5.0;
    assert!((q0 - 8.0 / 3.0 * x.powi(4) * k * k).abs() < 1e-24);
}

#[test]
fn plume_obscures_the_low_sight_only_and_scales_with_erosion() {
    let d = inputs("plume");
    let q: plume::Request = req(&d, "plume.toml");
    let r = plume::run(&q).unwrap();
    assert_eq!(r.assumptions.len(), plume::ASSUMPTIONS.len());
    let low = &r.sights[0];
    let high = &r.sights[1];
    assert_eq!(
        high.max_optical_db, 0.0,
        "the high sight stays above the sheet"
    );
    let h = low.obscured_from_height_m.unwrap();
    // Hand estimate (docs): tau = 0.69 at Mdot ~ 17 kg/s, i.e. about 9 m engine height.
    assert!((6.0..=11.0).contains(&h), "obscured from {h} m");
    assert!(low.max_optical_db > 8.0, "{}", low.max_optical_db);
    assert!(
        low.max_rf_db < 1e-3,
        "RF passes a 10 µm-particle sheet at 2 GHz"
    );
    // Nothing after the last particles have crossed the sheet.
    assert_eq!(low.steps.last().unwrap().optical_db, 0.0);
    // Optical depth is linear in the erosion rate.
    let mut q2 = q.clone();
    q2.ejecta.mdot0_kg_s *= 2.0;
    let r2 = plume::run(&q2).unwrap();
    assert!((r2.sights[0].max_optical_db / low.max_optical_db - 2.0).abs() < 1e-9);
    // Validity flags and refusals.
    let mut big = q.clone();
    big.ejecta.particle_radius_m = 0.01;
    assert!(plume::run(&big)
        .unwrap()
        .warnings
        .iter()
        .any(|w| w.contains("Rayleigh")));
    let mut bad = q.clone();
    bad.ejecta.r_max_m = 1.0;
    assert!(plume::run(&bad).is_err());
    let mut bad = q;
    bad.ejecta.angle_deg = 60.0;
    assert!(plume::run(&bad).is_err());
    // The physical parameters have no defaults.
    let text = std::fs::read_to_string(d.join("plume.toml"))
        .unwrap()
        .replace("mdot0_kg_s = 100.0\n", "");
    assert!(toml::from_str::<plume::Request>(&text).is_err());
}

#[test]
fn traverse_evidence_finds_dishonest_sigma_and_walkback_breach() {
    let d = inputs("traverse");
    let q: traverse::Request = req(&d, "traverse.toml");
    let r = traverse::run(&q, &d).unwrap();
    assert_eq!((r.epochs, r.truth_epochs), (3601, 3601));
    let ob = r.overbound.as_ref().unwrap();
    assert!(!ob.bounded && ob.inflation_needed > 1.5, "{ob:?}");
    let st = r.stanford.as_ref().unwrap();
    assert!(st.mi + st.hmi > 0);
    let w = r.walkback.as_ref().unwrap();
    // Analytic breach: (0.5 t + 5) > 3400 - t  =>  t > 2263.3 s.
    let t = w.first_breach_t_s.unwrap();
    assert!((2258.0..=2270.0).contains(&t), "{t}");
    let h = &r.hazards[0];
    assert_eq!(h.incursions, Some(0));
    assert!(h.min_margin_m < 6.0 && h.min_margin_m > -6.0, "{h:?}");
    // Honest rows only: the stated sigma overbounds the errors.
    let text = synth::traverse_csv();
    let honest: Vec<traverse::Row> = traverse::parse_log(&text)
        .unwrap()
        .into_iter()
        .filter(|x| x.t < 1700.0)
        .collect();
    let r2 = traverse::analyse(&q, &honest, String::new());
    assert!(r2.overbound.unwrap().bounded);
    assert!(r2.walkback.unwrap().first_breach_t_s.is_none());
    // Without truth there is no integrity statistic, but margins remain.
    let no_truth: Vec<traverse::Row> = honest
        .iter()
        .map(|x| traverse::Row { truth: None, ..*x })
        .collect();
    let r3 = traverse::analyse(&q, &no_truth, String::new());
    assert!(r3.stanford.is_none() && r3.hazards[0].incursions.is_none());
    assert!(traverse::parse_log("t_s,east_m,north_m,sigma_m\n0,0,0,0\n1,0,0,1\n").is_err());
}

#[test]
fn rf_forensics_attributes_spurs_and_correlates_the_motor() {
    let d = inputs("rf");
    let q: rf::Request = req(&d, "rf.toml");
    let r = rf::run(&q, &d).unwrap();
    assert_eq!(r.samples, synth::N);
    assert_eq!(r.spurs.len(), 3, "{:?}", r.spurs);
    let (n_dc, off_dc) = synth::harmonic_offset(synth::DCDC_HZ);
    let (n_mo, off_mo) = synth::harmonic_offset(synth::MOTOR_HZ);
    let find = |off: f64| {
        r.spurs
            .iter()
            .find(|s| (s.offset_hz - off).abs() <= r.bin_hz)
            .unwrap_or_else(|| panic!("no spur near {off}"))
    };
    let dc = find(off_dc);
    assert_eq!(dc.matches[0].source, "dcdc-converter");
    assert_eq!(dc.matches[0].harmonic, n_dc);
    let mo = find(off_mo);
    assert_eq!(mo.matches[0].source, "motor-drive");
    assert_eq!(mo.matches[0].harmonic, n_mo);
    assert_eq!(mo.time_correlated_with, vec!["motor-drive".to_string()]);
    assert!(mo.correlation["motor-drive"] > 0.9);
    let stray = find(synth::STRAY_OFFSET_HZ);
    assert!(stray.matches.is_empty() && stray.time_correlated_with.is_empty());
    assert_eq!(r.unattributed, 1);
    assert!(
        r.cn0_loss_db > 0.0 && r.cn0_loss_db < 3.0,
        "{}",
        r.cn0_loss_db
    );
    // Without an activity log there is no correlation.
    let mut q2 = q.clone();
    q2.activity = None;
    assert!(rf::run(&q2, &d)
        .unwrap()
        .spurs
        .iter()
        .all(|s| s.correlation.is_empty()));
    let mut bad = q;
    bad.nfft = 1000;
    assert!(rf::run(&bad, &d).is_err());
}

#[test]
fn cli_gates_and_signs() {
    let d = inputs("cli");
    let home = d.join("home");
    let seed = [9u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={hex}\n")).unwrap();
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
        )
    };
    for (cmd, file) in [
        ("plume", "plume.toml"),
        ("traverse", "traverse.toml"),
        ("rf-forensics", "rf.toml"),
    ] {
        let p = d.join(file);
        let (ok, out) = run(&["lunar-surface", cmd, p.to_str().unwrap()]);
        assert!(!ok, "{cmd} must need a licence: {out}");
        let o = d.join(format!("out-{cmd}"));
        let (ok, out) = run(&[
            "lunar-surface",
            cmd,
            p.to_str().unwrap(),
            "--dev",
            "--out",
            o.to_str().unwrap(),
            "--signer",
            "test",
            "--key-file",
            key.to_str().unwrap(),
            "--utc",
            "2026-11-20T00:00:00Z",
        ]);
        assert!(ok, "{cmd}: {out}");
        let v =
            kshana_pro::signed_dossier::verify(&o, &[kshana_pro::license::public_key_for(&seed)])
                .unwrap();
        assert!(v.passed(), "{cmd}: {:?}", v.problems);
        assert!(std::fs::read_to_string(o.join("report.html"))
            .unwrap()
            .contains("MODELLED"));
    }
    let (ok, _) = run(&["lunar-surface", "nonsense"]);
    assert!(!ok);
}
