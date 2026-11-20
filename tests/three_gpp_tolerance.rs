// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Shared-risk test-tolerance calculator and A-GNSS vector-set scaffold (research 2026-11-20
//! gaps E30 B3004). Every number below is SYNTHETIC, chosen to exercise the arithmetic; none
//! is a TS 37.571-1, TS 37.571-2 or TS 38.171 value (those were not read and Kshana ships none).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::transport::agnss::{self, Field, VectorSet};
use kshana_pro::transport::tolerance::{self, Decision, Outcome, Relax, ToleranceFrom};
use kshana_pro::transport::Table;

const SRC: &str = "SYNTHETIC test value (not from any 3GPP specification)";

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b3004-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn request() -> tolerance::Request {
    toml::from_str(&format!(
        r#"title = "SYNTHETIC tolerance check"
decision = "point"

[[parameters]]
name = "horizontal error"
unit = "m"
minimum_requirement = 100.0
requirement_source = "{SRC}"
relax = "increase"
uncertainty = [
  {{ name = "simulator", value = 3.0, kind = "standard" }},
  {{ name = "reference", value = 7.839855938, kind = "expanded95" }},
  {{ name = "timing", value = 5.196152423, kind = "rectangular-half-width" }},
]
measured = 105.0
measured_standard_uncertainty = 1.0
success_rate = {{ required_fraction = 0.95, required_fraction_source = "{SRC}", trials = 100, successes = 96 }}

[[parameters]]
name = "signal level"
unit = "dBm"
minimum_requirement = -140.0
requirement_source = "{SRC}"
relax = "decrease"
test_tolerance = 1.0
tolerance_source = "{SRC}"
measured = -140.5
measured_standard_uncertainty = 0.5

[[parameters]]
name = "response time"
unit = "s"
relax = "increase"
"#
    ))
    .unwrap()
}

#[test]
fn budget_combines_by_rss_and_relaxes_the_requirement() {
    let r = tolerance::run(&request()).unwrap();
    let p = &r.parameters[0];
    // Standard uncertainties 3, 4 (7.84/1.96) and 3 (5.196/√3): RSS = √34.
    let uc = p.combined_standard_uncertainty.unwrap();
    assert!((uc - 34f64.sqrt()).abs() < 1e-6, "{uc}");
    assert_eq!(p.tolerance_from, ToleranceFrom::Budget);
    let tt = p.test_tolerance.unwrap();
    assert!((tt - 1.96 * 34f64.sqrt()).abs() < 1e-6);
    assert!((p.test_requirement.unwrap() - (100.0 + tt)).abs() < 1e-9);
    // 105 is inside 111.43 by 6.4 >= 1.96: pass with margin.
    assert_eq!(p.outcome, Some(Outcome::PassWithMargin));
    let sr = p.success_rate.as_ref().unwrap();
    assert_eq!(sr.decision, Some(true), "point rule: 0.96 >= 0.95");
    assert!(sr.ci95.0 < 0.95 && sr.ci95.1 > 0.96);
    // A transcribed tolerance relaxes a lower limit downwards.
    let q = &r.parameters[1];
    assert_eq!(q.tolerance_from, ToleranceFrom::Transcribed);
    assert_eq!(q.test_requirement, Some(-141.0));
    assert_eq!(q.outcome, Some(Outcome::PassWithinUncertainty));
    // The third is a placeholder with no test requirement.
    let z = &r.parameters[2];
    assert_eq!(z.status, tolerance::PLACEHOLDER);
    assert!(z.test_requirement.is_none() && z.placeholders.len() == 2);
    assert_eq!(r.placeholders, 1);
    assert!(r.verdict.contains("not a 3GPP test verdict"));
}

#[test]
fn lower_bound_rule_and_refusals() {
    let mut q = request();
    q.decision = Decision::LowerBound;
    let r = tolerance::run(&q).unwrap();
    assert_eq!(
        r.parameters[0].success_rate.as_ref().unwrap().decision,
        Some(false),
        "the exact lower limit of 96/100 is below 0.95"
    );
    assert_eq!(
        tolerance::classify(10.0, 9.0, 0.5, Relax::Increase),
        Outcome::Fail
    );
    // A value without its source is refused.
    let mut bad = request();
    bad.parameters[0].requirement_source = None;
    assert!(tolerance::run(&bad).is_err());
    let mut bad = request();
    bad.parameters[1].tolerance_source = Some(" ".into());
    assert!(tolerance::run(&bad).is_err());
    let mut bad = request();
    bad.parameters[0].success_rate.as_mut().unwrap().successes = 101;
    assert!(tolerance::run(&bad).is_err());
}

fn filled() -> VectorSet {
    let mut s = agnss::scaffold();
    let f = |v: f64| Field {
        value: Some(v),
        source: Some(SRC.into()),
    };
    s.release = Field {
        value: Some("SYNTHETIC".into()),
        source: Some(SRC.into()),
    };
    let c = &mut s.cases[2];
    c.accuracy_limit_m = f(30.0);
    c.tolerance_accuracy_m = f(2.0);
    c.required_fraction = f(0.9);
    c.response_time_limit_s = f(20.0);
    c.tolerance_response_time_s = f(1.0);
    c.trials = Field {
        value: Some(20),
        source: Some(SRC.into()),
    };
    s
}

#[test]
fn scaffold_is_all_placeholders_and_status_tracks_filling() {
    let s = agnss::scaffold();
    assert_eq!(s.cases.len(), agnss::CATEGORIES.len());
    let st = agnss::status(&s).unwrap();
    assert!(!st.complete && st.cases.iter().all(|c| c.filled == 0 && !c.scorable));
    // Every value in the shipped scaffold is null.
    let json = serde_json::to_value(&s).unwrap();
    for c in json["cases"].as_array().unwrap() {
        for (k, v) in c.as_object().unwrap() {
            if k != "id" && k != "category" {
                assert!(v["value"].is_null(), "{k} must be a placeholder");
            }
        }
    }
    let f = filled();
    let st = agnss::status(&f).unwrap();
    assert!(st.release_filled);
    assert!(st.cases[2].scorable && !st.cases[0].scorable);
    // A value without a source is refused.
    let mut bad = f;
    bad.cases[0].accuracy_limit_m = Field {
        value: Some(1.0),
        source: None,
    };
    assert!(agnss::status(&bad).is_err());
}

#[test]
fn score_applies_tolerances_and_skips_placeholder_cases() {
    let set = filled();
    let mut csv = String::from("case_id,trial,error_m,response_time_s\n");
    // 20 trials: 18 within 32 m and 21 s, one at 31.9 m (inside only with the tolerance),
    // one too slow.
    for i in 0..17 {
        csv.push_str(&format!("nominal-accuracy,{i},{},10\n", 5.0 + i as f64));
    }
    csv.push_str("nominal-accuracy,17,31.9,10\n");
    csv.push_str("nominal-accuracy,18,10,20.5\n");
    csv.push_str("nominal-accuracy,19,10,25\n");
    csv.push_str("sensitivity-coarse-time,0,1,1\n");
    let t = Table::parse(&csv).unwrap();
    let r = agnss::score(&set, &t, Decision::Point).unwrap();
    assert_eq!(r.cases.len(), 1);
    let c = &r.cases[0];
    assert_eq!(c.accuracy_test_limit_m, 32.0);
    assert_eq!(c.response_time_test_limit_s, Some(21.0));
    assert_eq!(c.successes, 19);
    assert!(c.decision, "19/20 = 0.95 >= 0.9");
    assert!(r.not_scored["sensitivity-coarse-time"].contains("placeholder"));
    let lb = agnss::score(&set, &t, Decision::LowerBound).unwrap();
    assert!(!lb.cases[0].decision);
}

#[test]
fn cli_scaffold_status_and_gated_commands() {
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
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
        )
    };
    let set = d.join("set.json");
    let (ok, out) = run(&["nr", "agnss-scaffold", "--out", set.to_str().unwrap()]);
    assert!(ok, "{out}");
    let (ok, out) = run(&["nr", "agnss-status", set.to_str().unwrap()]);
    assert!(ok && out.contains("0 of 6 case(s) scorable"), "{out}");
    let req = d.join("tol.toml");
    std::fs::write(&req, toml::to_string(&request()).unwrap()).unwrap();
    let (ok, out) = run(&["nr", "tolerance", req.to_str().unwrap()]);
    assert!(!ok, "needs a licence: {out}");
    let (ok, out) = run(&["nr", "tolerance", req.to_str().unwrap(), "--dev"]);
    assert!(
        ok && out.contains("kshana-pro-3gpp-test-tolerance/1"),
        "{out}"
    );
    let res = d.join("res.csv");
    std::fs::write(&res, "case_id,trial,error_m\nnominal-accuracy,0,1\n").unwrap();
    let (ok, out) = run(&[
        "nr",
        "agnss-score",
        set.to_str().unwrap(),
        res.to_str().unwrap(),
        "--dev",
    ]);
    assert!(ok && out.contains("not scored"), "{out}");
}
