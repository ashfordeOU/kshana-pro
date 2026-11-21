// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Marine survey and construction positioning QC under interference (research 2026-11-20 gaps
//! B0807) on a SYNTHETIC survey: the injected intervals are flagged on the right lines, clean
//! lines pass, the risk plan names the shared hour, and the signed QC annex verifies.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::survey_qc::{self, Request};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b0807-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn req() -> Request {
    toml::from_str(
        "title = \"t\"\nlog = \"survey.csv\"\nhorizontal_tolerance_m = 2.0\ntolerance_source = \"SYNTHETIC\"\nrequired_fix = 4\n",
    )
    .unwrap()
}

#[test]
fn injected_intervals_are_flagged_and_clean_lines_pass() {
    let d = tmp("qc");
    let (csv, truth) = survey_qc::synth();
    std::fs::write(d.join("survey.csv"), &csv).unwrap();
    let a = survey_qc::run(&req(), &d).unwrap();
    let qc = |id: &str| a.lines.iter().find(|l| l.line_id == id).unwrap();
    assert_eq!(qc("L1").qc, "PASS");
    assert_eq!(qc("L3").qc, "PASS");
    assert_eq!(qc("L2").qc, "RE-RUN", "300 of 600 s flagged");
    assert_eq!(qc("L4").qc, "DEGRADED", "20 of 600 s flagged");
    for (id, t0, t1) in &truth {
        let l = qc(id);
        assert_eq!(l.intervals.len(), 1, "{id}: {:?}", l.intervals);
        let iv = &l.intervals[0];
        let s = kshana_pro::forensics::inputs::parse_utc(&iv.start_utc).unwrap();
        let e = kshana_pro::forensics::inputs::parse_utc(&iv.end_utc).unwrap();
        assert!(
            (s - t0).abs() <= 1.0 && (e - (t1 - 1.0)).abs() <= 1.0,
            "{id}: {s} {e}"
        );
        for k in ["cn0-drop", "agc-drop", "few-satellites", "fix-degraded"] {
            assert!(iv.indicators.contains_key(k), "{id}: {k}");
        }
    }
    // Both injections fall in the 10:00 UTC hour: the plan names it.
    assert_eq!(a.risk_hours[0].key, "10:00-10:59 UTC");
    assert_eq!(
        a.risk_hours[0].lines,
        vec!["L2".to_string(), "L4".to_string()]
    );
    assert!(a.plan[0].contains("10:00-10:59 UTC"));
    let mut bad = req();
    bad.tolerance_source = " ".into();
    assert!(survey_qc::run(&bad, &d).is_err());
    let mut strict = req();
    strict.reject_share = 0.0;
    let s = survey_qc::run(&strict, &d).unwrap();
    assert_eq!(
        s.lines.iter().find(|l| l.line_id == "L4").unwrap().qc,
        "RE-RUN"
    );
}

#[test]
fn cli_signs_an_annex_that_verifies() {
    let d = tmp("cli");
    let home = d.join("home");
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
    let (ok, m) = run(&["survey-qc", "synth", d.to_str().unwrap()]);
    assert!(ok, "{m}");
    let seed = [5u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("key.txt"), format!("private_key={hex}\n")).unwrap();
    let out = d.join("annex");
    let r = d.join("request.toml");
    let kf = d.join("key.txt");
    let args = [
        "survey-qc",
        "run",
        r.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "t",
        "--key-file",
        kf.to_str().unwrap(),
        "--utc",
        "2026-11-20T00:00:00Z",
    ];
    let (ok, _) = run(&args);
    assert!(!ok, "needs a licence");
    let mut a = args.to_vec();
    a.push("--dev");
    let (ok, m) = run(&a);
    assert!(ok && m.contains("RE-RUN"), "{m}");
    let v = kshana_pro::signed_dossier::verify(&out, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
}
