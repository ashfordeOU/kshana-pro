// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Re-scoring schedule and per-test GUM uncertainty budgets (research 2026-11-20 gaps B2505,
//! Pro part) on SYNTHETIC registers and runs, checked against hand calculations.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::method_uncertainty::{self as mu, Kind, MethodRequest, Test, TypeB};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b2505-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn t95_matches_tables() {
    assert!((mu::t95(f64::INFINITY) - 1.959_964).abs() < 1e-5);
    assert!((mu::t95(10.0) - 2.228_139).abs() < 1e-4);
    assert!((mu::t95(4.0) - 2.776_445).abs() < 1e-4);
    assert!((mu::t95(1.0) - 12.706_2).abs() < 1e-3);
}

#[test]
fn schedule_flags_overdue_and_due_soon_with_month_ends() {
    let reg = "product_id,method_id,scored_utc\n\
               P1,M-A,2025-10-01T00:00:00Z\n\
               P1,M-A,2025-01-01T00:00:00Z\n\
               P2,M-A,2025-12-20T00:00:00Z\n\
               P3,M-B,2024-02-29T00:00:00Z\n";
    let s = mu::schedule(reg, &mu::Policy::default(), "2026-11-20T00:00:00Z").unwrap();
    let get = |p: &str| s.items.iter().find(|i| i.product_id == p).unwrap();
    assert_eq!(
        get("P1").last_scored_utc,
        "2025-10-01T00:00:00Z",
        "latest scoring counts"
    );
    assert_eq!(get("P1").status, "OVERDUE");
    assert_eq!(get("P2").next_due_utc, "2026-12-20T00:00:00Z");
    assert_eq!(get("P2").status, "DUE SOON");
    assert_eq!(
        get("P3").next_due_utc,
        "2025-02-28T00:00:00Z",
        "29 February to 28 February"
    );
    assert!(mu::schedule(
        "product_id,method_id\nx,y\n",
        &mu::Policy::default(),
        "2026-11-20T00:00:00Z"
    )
    .is_err());
}

#[test]
fn budget_matches_a_hand_calculation() {
    let t = Test {
        id: "T1".into(),
        quantity: "time error".into(),
        unit: "ns".into(),
        runs: vec![10.0, 10.2, 9.8, 10.1, 9.9],
        type_b: vec![TypeB {
            name: "reference".into(),
            value: 0.1,
            kind: Kind::RectangularHalfWidth,
            sensitivity: 1.0,
            dof: None,
            source: "SYNTHETIC".into(),
        }],
        reference: Some(10.3),
        reference_u95: Some(0.1),
    };
    let b = mu::budget(&t).unwrap();
    let ua = (0.025f64).sqrt() / 5f64.sqrt();
    let ub = 0.1 / 3f64.sqrt();
    let uc = (ua * ua + ub * ub).sqrt();
    assert!((b.combined_standard_uncertainty - uc).abs() < 1e-12);
    let veff = uc.powi(4) / (ua.powi(4) / 4.0);
    assert!((b.effective_dof.unwrap() - veff).abs() < 1e-9);
    assert!((b.coverage_factor - mu::t95(veff)).abs() < 1e-12);
    assert!((b.lines.iter().map(|l| l.share).sum::<f64>() - 1.0).abs() < 1e-12);
    let en = -0.3 / ((b.coverage_factor * uc).powi(2) + 0.01).sqrt();
    assert!((b.en.unwrap() - en).abs() < 1e-9 && b.en.unwrap() < -1.0);
    let r = mu::report(&MethodRequest {
        method_id: "M-A".into(),
        title: "SYNTHETIC".into(),
        tests: vec![t.clone()],
    })
    .unwrap();
    assert!(r.statement.contains("scored under method M-A"));
    assert!(r.verdict.contains("1 with |E_n| > 1"));
    // Refusals: one run; a contribution without a source.
    let mut one = t.clone();
    one.runs = vec![1.0];
    assert!(mu::budget(&one).is_err());
    let mut nosrc = t;
    nosrc.type_b[0].source = " ".into();
    assert!(mu::budget(&nosrc).is_err());
}

#[test]
fn cli_gates_and_signs() {
    let d = tmp("cli");
    let home = d.join("home");
    std::fs::write(
        d.join("m.toml"),
        "method_id = \"M-A\"\ntitle = \"t\"\n[[tests]]\nid = \"T1\"\nquantity = \"q\"\nruns = [1.0, 1.1, 0.9]\n",
    )
    .unwrap();
    let hex: String = [8u8; 32].iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("k.txt"), format!("private_key={hex}\n")).unwrap();
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let m = d.join("m.toml");
    let (ok, _) = run(&["method", "budget", m.to_str().unwrap()]);
    assert!(!ok);
    let out = d.join("o");
    let (ok, e) = run(&[
        "method",
        "budget",
        m.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "t",
        "--key-file",
        d.join("k.txt").to_str().unwrap(),
        "--dev",
    ]);
    assert!(ok, "{e}");
    let v = kshana_pro::signed_dossier::verify(
        &out,
        &[kshana_pro::license::public_key_for(&[8u8; 32])],
    )
    .unwrap();
    assert!(v.passed());
}
