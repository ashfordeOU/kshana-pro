// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Lunar and cislunar PNT evidence (`kshana_pro::lunar_evidence`, docs/LUNAR-EVIDENCE.md): each
//! command on the synthetic fixture finds the fault injected into it and nothing else; one
//! command end to end through the binary with signing and an offline check; the licence gates.
//! All inputs are synthetic.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::license::{self, Entitlement, Tier};
use kshana_pro::lunar_evidence::{self as le, clocks, surface, synth};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");

fn fixture(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-lunar-ev-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    synth::write_all(&d).unwrap();
    d
}

fn load(d: &Path, req: &str) -> le::Loaded {
    le::load(&d.join(req)).unwrap()
}

#[test]
fn ura_overbounding_flags_the_node_that_understates_its_ura() {
    let d = fixture("ura");
    let r = le::service::ura(&mut load(&d, "ura.toml")).unwrap();
    let g = |n: &str| r.groups.iter().find(|g| g.node == n).unwrap();
    for n in ["N1", "N2", "N3"] {
        assert_ne!(g(n).verdict, "NOT OVERBOUNDED", "{n}: {:#?}", g(n));
        // SISE at 0.8 URA: overbounded at k = 1 and 2 with 576 independent blocks.
        assert_eq!(g(n).thresholds[0].verdict, "OVERBOUNDED");
        assert_eq!(g(n).thresholds[1].verdict, "OVERBOUNDED");
        assert!(g(n).inflation_needed <= 1.0 + 1e-9);
    }
    let n4 = g("N4");
    assert_eq!(n4.verdict, "NOT OVERBOUNDED");
    assert!((n4.std_z - 1.6).abs() < 0.1);
    // The inflation that makes N4 overbounded is close to its true scale of 1.6.
    assert!(
        n4.inflation_needed > 1.4 && n4.inflation_needed < 2.0,
        "{}",
        n4.inflation_needed
    );
    // k = 4 cannot be shown overbounded from a day and a half of blocks.
    assert_eq!(g("N1").thresholds[3].verdict, "INCONCLUSIVE");
    assert!(g("N1").thresholds[3].n_needed_zero_exceedance > 50_000);
}

#[test]
fn time_consistency_finds_the_step_and_the_ramp_and_nothing_else() {
    let d = fixture("time");
    let r = le::service::time_consistency(&mut load(&d, "time-consistency.toml")).unwrap();
    assert_eq!(r.events.len(), 2, "{:#?}", r.events);
    let step = r.events.iter().find(|e| e.node == "N3").unwrap();
    assert_eq!(step.class, "step");
    assert!((step.onset_s - 40_000.0).abs() <= 60.0, "{step:?}");
    assert!((step.level_ns - 25.0).abs() < 2.0);
    let ramp = r.events.iter().find(|e| e.node == "N5").unwrap();
    assert_eq!(ramp.class, "ramp");
    assert!(
        ramp.onset_s >= 60_000.0 && ramp.onset_s < 63_000.0,
        "{ramp:?}"
    );
    assert!((ramp.slope_ns_per_s - 0.002).abs() < 0.0006);
    // The shared wander cancels against the ensemble median.
    assert!(r.pooled_sigma_ns > 1.0 && r.pooled_sigma_ns < 2.5);
}

#[test]
fn afs_receiver_checks_find_every_injected_fault() {
    let d = fixture("afs");
    let r = le::afs::afs_rx(&mut load(&d, "afs-rx.toml")).unwrap();
    let q: Vec<_> = r.codes.iter().filter(|c| c.channel == "q").collect();
    assert_eq!(q[0].verdict, "MATCH");
    assert_eq!(q[1].verdict, "MATCH");
    assert_eq!(q[2].mismatched_chips, Some(1));
    assert_eq!(q[2].first_mismatch, Some(100));
    let i: Vec<_> = r.codes.iter().filter(|c| c.channel == "i").collect();
    assert_eq!(i[0].g2_delay, Some(5));
    assert!(
        i[0].verdict == "MATCH"
            || i[0].verdict.starts_with("GOLD CODE OK")
            || i[0].verdict == "WRONG PRN"
    );
    assert_eq!(i[1].verdict, "NOT A GOLD CODE OF THE AFS-I PAIR");
    let f = &r.frames[0];
    assert_eq!((f.frames, f.crc_failures, f.toi_slips), (200, 3, 1));
    assert!(f.verdict.starts_with("MEETS") && f.verdict.ends_with("TOI SLIPS"));
    assert_eq!(
        r.collisions,
        vec!["AFS-Q PRN 2 used by nodes 2 and 5".to_string()]
    );
    assert_eq!(r.verdict, "FINDINGS");
    assert!(r.standard.contains("LSIS V1.0"));
}

#[test]
fn clock_characterisation_separates_a_rafs_from_a_coarser_clock() {
    let d = fixture("clock");
    let r = clocks::clock(&mut load(&d, "clock.toml")).unwrap();
    assert!(
        !r.points.iter().any(|p| p.verdict == "EXCEEDS"),
        "{:#?}",
        r.points
    );
    assert!((r.drift_per_s - 1e-17).abs() < 4e-18, "{}", r.drift_per_s);
    let p1 = &r.points[0];
    assert!((p1.adev / (1e-11 / p1.tau_s.sqrt()) - 1.0).abs() < 0.1);
    // The same record scaled to a mini-RAFS level exceeds the RAFS class.
    let text = synth::clock_csv(5e-10, 9);
    let (t, x): (Vec<f64>, Vec<f64>) = text
        .lines()
        .skip(1)
        .map(|l| {
            let mut f = l.split(',').map(|v| v.parse::<f64>().unwrap());
            (f.next().unwrap(), f.next().unwrap())
        })
        .unzip();
    let (_, _, _, pts) = clocks::characterise(&t, &x, Some("rafs"), None).unwrap();
    assert!(pts.iter().all(|p| p.verdict == "EXCEEDS"));
    let (_, _, _, pts) = clocks::characterise(&t, &x, Some("mini-rafs"), None).unwrap();
    assert!(!pts.iter().any(|p| p.verdict == "EXCEEDS"));
    // A mask is interpolated in log-log.
    let m = [[1.0, 1e-10], [100.0, 1e-12]];
    let v = clocks::limit_at(None, Some(&m), 10.0).unwrap();
    assert!((v / 1e-11 - 1.0).abs() < 1e-9);
    assert!(clocks::limit_at(Some("cesium-fountain"), None, 1.0).is_err());
}

#[test]
fn station_delay_budget_and_en_number() {
    let d = fixture("delay");
    let r = clocks::station_delay(&mut load(&d, "station-delay.toml")).unwrap();
    assert!((r.delay_ns - 1250.05).abs() < 1e-9);
    // u_A = s/sqrt(8); rectangular 0.5 -> 0.2887; recomputed here.
    let v = [
        1249.8, 1250.3, 1250.1, 1249.7, 1250.2, 1250.0, 1249.9, 1250.4,
    ];
    let m = v.iter().sum::<f64>() / 8.0;
    let s = (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / 7.0).sqrt();
    let uc = ((s / 8f64.sqrt()).powi(2) + 0.04 + 0.25 / 3.0 + 0.0225).sqrt();
    assert!((r.u_c_ns - uc).abs() < 1e-3);
    assert!(r.k > 1.96 && r.k < 2.1);
    assert!(r.en.unwrap() > 1.0);
    assert!(r.verdict.starts_with("MOVED"));
    // Student t quantiles at small degrees of freedom.
    assert!((clocks::t95(10.0) - 2.228).abs() < 0.005);
    assert!((clocks::t95(30.0) - 2.042).abs() < 0.002);
}

#[test]
fn radiometric_forensics_and_track_quality() {
    let d = fixture("radio");
    let r = clocks::radiometric(&mut load(&d, "radiometric.toml")).unwrap();
    let p = |n: &str| r.passes.iter().find(|x| x.pass == n).unwrap();
    assert!(p("P1").findings.is_empty(), "{:?}", p("P1").findings);
    assert_eq!(p("P1").grade, "A");
    assert_eq!(p("P2").outliers, 2);
    assert!(
        p("P2").findings.iter().any(|f| f.starts_with("step")),
        "{:?}",
        p("P2").findings
    );
    assert!(
        p("P3").findings.iter().any(|f| f.starts_with("ramp")),
        "{:?}",
        p("P3").findings
    );
    assert_eq!(p("P3").gaps.len(), 1);
    assert!(p("P3").coverage < 0.75);
    assert_eq!(p("P4").grade, "D");
    assert!(p("P2").score < 50.0 && p("P3").score < 50.0);
}

#[test]
fn fdir_against_a_preregistered_plan() {
    let d = fixture("fdir");
    let r = surface::fdir(&mut load(&d, "fdir.toml")).unwrap();
    assert!(r.preregistered);
    let c = |n: &str| r.cases.iter().find(|x| x.id == n).unwrap();
    assert_eq!(c("alt-dropout").verdict, "PASS");
    assert_eq!(c("lrf-false-reject").verdict, "FAIL");
    assert_eq!(c("lrf-false-reject").events, 290);
    assert_eq!(c("dust-dropout").verdict, "INSUFFICIENT RUNS");
    assert_eq!(c("nominal").verdict, "PASS");
    assert_eq!(r.unplanned_cases, vec!["extra-case".to_string()]);
    assert_eq!(r.verdict, "FAIL");
    // Editing the plan after registration is caught.
    let plan = d.join("fdir-plan.toml");
    let t = std::fs::read_to_string(&plan)
        .unwrap()
        .replace("0.99", "0.95");
    std::fs::write(&plan, t).unwrap();
    let r = surface::fdir(&mut load(&d, "fdir.toml")).unwrap();
    assert!(!r.preregistered);
    assert!(r.verdict.starts_with("PLAN CHANGED"));
}

#[test]
fn configuration_attestation_lists_every_difference() {
    let d = fixture("config");
    let r = surface::config(&mut load(&d, "config.toml")).unwrap();
    assert_eq!(
        r.mismatched,
        vec![["lrf.enabled".to_string(), "true".into(), "false".into()]]
    );
    assert!(r.missing.is_empty());
    assert_eq!(r.unexpected, vec!["debug.patch".to_string()]);
    assert!(r.verdict.starts_with("DIFFERS"));
}

#[test]
fn datum_tie_catches_a_mean_earth_map_declared_principal_axis() {
    let d = fixture("datum");
    let r = surface::datum(&mut load(&d, "datum.toml")).unwrap();
    assert_eq!(r.verdict, "NOT TIED");
    let off = r.centroid_offset_m;
    let mag = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt();
    assert!(mag > 500.0 && mag < 1000.0, "{mag}");
    assert!(r
        .frame_finding
        .as_deref()
        .unwrap()
        .contains("looks like mean-Earth"));
    let ok = surface::datum(&mut load(&d, "datum-ok.toml")).unwrap();
    assert_eq!(ok.verdict, "TIED", "{ok:#?}");
    assert!((ok.centroid_offset_m[0] + 2.0).abs() < 0.6);
    assert!(ok.frame_finding.is_none());
    assert!(ok.model.starts_with("translation"));
}

#[test]
fn tipover_probability_grows_with_dispersion_toward_the_block_field() {
    let d = fixture("tip");
    let r = surface::tipover(&mut load(&d, "tipover.toml")).unwrap();
    assert!((r.static_tip_angle_deg - (2.0f64 / 1.5).atan().to_degrees()).abs() < 1e-3);
    let (base, half) = (&r.cases[0], &r.cases[1]);
    assert!(base.p_tip > 0.0 && base.p_tip < 0.2, "{base:?}");
    assert!(half.p_tip < base.p_tip);
    assert!(base.p_design_exceed.unwrap() >= base.p_tip);
    assert!(r.cases[2].off_map > 0);
}

#[test]
fn signed_result_through_the_binary_and_the_licence_gates() {
    let d = fixture("cli");
    let key = d.join("key.hex");
    std::fs::write(&key, hex::encode([5u8; 32])).unwrap();
    let out = d.join("out-afs");
    let o = Command::new(BIN)
        .args([
            "lunar-evidence",
            "afs-rx",
            d.join("afs-rx.toml").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--signer",
            "test",
            "--key-file",
            key.to_str().unwrap(),
            "--utc",
            "2026-11-20T12:00:00Z",
            "--dev",
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.join("afs-rx.json").exists() && out.join("inputs/afs-codes.csv").exists());
    let s = std::fs::read_to_string(out.join("summary.txt")).unwrap();
    assert!(s.contains("MODELLED") && !s.to_lowercase().contains("compliant"));
    let o = Command::new(BIN)
        .args(["lunar-evidence", "check", out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).contains("signature valid"));
    // Without a licence a producing command is refused.
    let o = Command::new(BIN)
        .args([
            "lunar-evidence",
            "ura",
            d.join("ura.toml").to_str().unwrap(),
            "--out",
            "x",
            "--signer",
            "s",
            "--key-file",
            key.to_str().unwrap(),
        ])
        .env_remove("KSHANA_PRO_LICENSE")
        .output()
        .unwrap();
    assert!(!o.status.success());
    for f in [le::FEATURE_SERVICE, le::FEATURE_CLOCK, le::FEATURE_SURFACE] {
        assert_eq!(license::explicit_tier(f), Some(Tier::Pro));
        assert!(license::PRO_COMMAND_FEATURES.contains(&f));
        assert!(license::check(&Entitlement::evaluation(), f, 0).is_err());
        let mut pro = Entitlement::evaluation();
        pro.tier = Tier::Pro;
        assert!(license::check(&pro, f, 0).is_ok());
    }
}
