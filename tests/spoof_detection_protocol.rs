// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Spoofing-detection output verification protocol (research 2026-11-20 gaps B1208): detection
//! latency, clearance and false alerts scored from a SYNTHETIC equipment log with the
//! applicant's limits; no detection or mitigation algorithm is involved.

use kshana_pro::avionics::library;
use kshana_pro::avionics::procedure::{self, ParamsFile};
use kshana_pro::scorecards::levels::Verdict;

fn pf(v: &[(&str, f64)]) -> ParamsFile {
    ParamsFile {
        format: procedure::PARAMS_FORMAT.into(),
        supplied_by: "SYNTHETIC".into(),
        values: v.iter().map(|(k, x)| (k.to_string(), *x)).collect(),
        sources: Default::default(),
    }
}

#[test]
fn detection_clearance_and_false_alerts_are_scored() {
    let p = library::get("spoof-detection-output").unwrap();
    let ps = p.resolve_params(Some(&pf(&[
        ("detection_time_s", 5.0),
        ("clear_time_s", 5.0),
        ("max_false_alert_s", 0.0),
    ])));
    let good = procedure::synth_log(&p, true).unwrap();
    let v = procedure::score(&p, &ps, Some(&good), None).unwrap();
    assert_eq!(v.verdict, "pass", "{:#?}", v.checks);
    let det = v.checks.iter().find(|c| c.id == "meacon-detected").unwrap();
    assert_eq!(det.measured, Some(2.0));
    let bad = procedure::synth_log(&p, false).unwrap();
    let v = procedure::score(&p, &ps, Some(&bad), None).unwrap();
    assert_eq!(v.verdict, "fail");
    for id in ["meacon-detected", "drag-detected", "false-alerts"] {
        assert_eq!(
            v.checks.iter().find(|c| c.id == id).unwrap().verdict,
            Verdict::NotMet,
            "{id}"
        );
    }
    // Tighter detection limit than the equipment achieves.
    let tight = p.resolve_params(Some(&pf(&[
        ("detection_time_s", 1.0),
        ("max_false_alert_s", 0.0),
    ])));
    let v = procedure::score(&p, &tight, Some(&good), None).unwrap();
    assert_eq!(v.verdict, "fail");
    assert_eq!(
        v.checks
            .iter()
            .find(|c| c.id == "meacon-cleared")
            .unwrap()
            .verdict,
        Verdict::NotAssessed,
        "no clear_time_s supplied"
    );
    // Without limits: incomplete, never pass.
    let v = procedure::score(&p, &p.resolve_params(None), Some(&good), None).unwrap();
    assert_eq!(v.verdict, "incomplete");
}

#[test]
fn malformed_timelines_are_refused() {
    let src = library::find("spoof-detection-output").unwrap();
    for (a, b) in [
        ("t_s = 300", "t_s = 120"),
        ("kind = \"meaconing\"", "kind = \"transmit\""),
        ("name = \"max_false_alert_s\"", "name = \"other_s\""),
    ] {
        let s = src.replacen(a, b, 1);
        assert!(
            procedure::Procedure::parse(&s)
                .and_then(|p| p.validate())
                .is_err(),
            "{a} -> {b} must be refused"
        );
    }
}
