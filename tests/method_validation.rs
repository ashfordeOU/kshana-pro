// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Interlaboratory precision study (research 2026-11-20 gaps B2505 Pro part): the critical
//! values of Cochran and Grubbs against published ISO 5725-2 table entries, r and R on a case
//! worked by hand, the SYNTHETIC study's biased and scattered laboratories found, exclusions
//! only with a reason, and the signed CLI pack behind a licence.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::method_validation as mv;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("mv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn critical_values_match_published_table_entries() {
    // (p, n, 5 %, 1 %) for Cochran; (p, 5 %, 1 %) for Grubbs; values as tabulated.
    for (p, n, c5, c1) in [(10, 2, 0.602, 0.718), (3, 2, 0.967, 0.993)] {
        assert!(
            (mv::cochran_critical(p, n, 0.05) - c5).abs() < 2e-3,
            "C {p} {n}"
        );
        assert!(
            (mv::cochran_critical(p, n, 0.01) - c1).abs() < 2e-3,
            "C {p} {n}"
        );
    }
    for (p, g5, g1) in [(10, 2.290, 2.482), (20, 2.709, 3.001)] {
        assert!((mv::grubbs_critical(p, 0.05) - g5).abs() < 2e-3, "G {p}");
        assert!((mv::grubbs_critical(p, 0.01) - g1).abs() < 2e-3, "G {p}");
    }
}

fn study(src: &str) -> mv::Study {
    toml::from_str(src).unwrap()
}

#[test]
fn repeatability_and_reproducibility_by_hand() {
    let q = study(
        "title = \"t\"\nmethod_id = \"M\"\nquantity = \"q\"\n\n[[level]]\nname = \"A\"\nreference_value = 12.0\n\
         [level.results]\nL1 = [10.0, 12.0]\nL2 = [11.0, 13.0]\nL3 = [14.0, 16.0]\n",
    );
    let r = mv::study(&q).unwrap();
    let p = &r.levels[0].all_labs;
    // s_r² = 2; s_d² = 26/3; n̄ = 2; s_L² = (26/3 − 2)/2 = 10/3; s_R² = 16/3.
    assert!((p.s_r * p.s_r - 2.0).abs() < 1e-12);
    assert!((p.s_l * p.s_l - 10.0 / 3.0).abs() < 1e-12);
    assert!((p.s_rr * p.s_rr - 16.0 / 3.0).abs() < 1e-12);
    assert!((p.r_limit - 2.8 * 2f64.sqrt()).abs() < 1e-12);
    assert!((p.mean - 38.0 / 3.0).abs() < 1e-12);
    // Bias bound 1.96 √((16/3 − 1/2·2)/3).
    let a = 1.96 * ((16.0 / 3.0 - 1.0) / 3.0f64).sqrt();
    assert!((p.bias_bound.unwrap() - a).abs() < 1e-12);
    assert_eq!(p.bias_within_bound, Some(true));
    // Mandel h sums to zero; k is 1 for equal scatter.
    let hs: f64 = r.levels[0].cells.iter().map(|c| c.h).sum();
    assert!(hs.abs() < 1e-12);
    assert!(r.levels[0].cells.iter().all(|c| (c.k - 1.0).abs() < 1e-12));
    // Two laboratories are not a study; an exclusion needs a reason and a known laboratory.
    assert!(mv::study(&study("title = \"t\"\nmethod_id = \"M\"\nquantity = \"q\"\n[[level]]\nname = \"A\"\n[level.results]\nL1 = [1.0, 2.0]\nL2 = [1.0, 2.0]\n")).is_err());
    let mut e = q.clone();
    e.exclude.push(mv::Exclusion {
        lab: "L3".into(),
        reason: " ".into(),
    });
    assert!(mv::study(&e).is_err());
    e.exclude[0] = mv::Exclusion {
        lab: "L9".into(),
        reason: "x".into(),
    };
    assert!(mv::study(&e).is_err());
}

#[test]
fn synthetic_study_finds_the_biased_and_the_scattered_laboratory() {
    let q = study(&mv::synth());
    let r = mv::study(&q).unwrap();
    for l in &r.levels {
        let g = l
            .tests
            .iter()
            .find(|t| t.test == "Grubbs (largest mean)")
            .unwrap();
        assert_eq!(g.lab, "L7", "{}", l.name);
        assert_ne!(g.class, "ok", "{}: {g:?}", l.name);
    }
    let b = &r.levels[1];
    let c = b
        .tests
        .iter()
        .find(|t| t.test.starts_with("Cochran"))
        .unwrap();
    assert_eq!(c.lab, "L3");
    assert_ne!(c.class, "ok", "{c:?}");
    assert!(b.cells.iter().find(|c| c.lab == "L3").unwrap().k > 2.0);
    // Excluding L7 with a reason lowers R and the bias, and both sets are reported.
    let mut q2 = q.clone();
    q2.exclude.push(mv::Exclusion {
        lab: "L7".into(),
        reason: "SYNTHETIC: calibration error found on investigation".into(),
    });
    let r2 = mv::study(&q2).unwrap();
    for l in &r2.levels {
        let after = l.after_exclusions.as_ref().unwrap();
        assert_eq!(after.labs, 7);
        assert!(after.rr_limit < l.all_labs.rr_limit);
        assert!(after.bias.unwrap().abs() < l.all_labs.bias.unwrap().abs());
    }
    assert!(r2.verdict.contains("1 laboratory exclusion"));
    assert!(r2.verdict.contains("scored under method KM-SYNTH-1"));
}

#[test]
fn signed_study_needs_a_licence() {
    let d = tmp("cli");
    let home = d.join("home");
    let seed = [51u8; 32];
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
    let s = d.join("study.toml");
    let s = s.to_str().unwrap();
    let (ok, m) = run(&["method", "synth-study", s]);
    assert!(ok, "{m}");
    let out = d.join("pack");
    let (o, k) = (out.to_str().unwrap(), key.to_str().unwrap());
    let (ok, m) = run(&[
        "method",
        "validate",
        s,
        "--out",
        o,
        "--signer",
        "T",
        "--key-file",
        k,
    ]);
    assert!(!ok, "{m}");
    let (ok, m) = run(&[
        "method",
        "validate",
        s,
        "--out",
        o,
        "--signer",
        "T",
        "--key-file",
        k,
        "--dev",
    ]);
    assert!(ok && m.contains("r = ") && m.contains("R = "), "{m}");
    let pk = kshana_pro::license::public_key_for(&seed);
    assert!(kshana_pro::signed_dossier::verify(&out, &[pk])
        .unwrap()
        .passed());
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(v["format"], mv::FORMAT);
    let html = std::fs::read_to_string(out.join("report.html"))
        .unwrap()
        .to_lowercase();
    for w in ["compliant", "conform", "certified"] {
        assert!(!html.contains(w), "{w}");
    }
}
