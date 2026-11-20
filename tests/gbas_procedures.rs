// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! GBAS airborne test automation (research 2026-11-20 gaps B1204): the `gbas-events` procedure
//! kind builds the expected-behaviour timeline, scores a SYNTHETIC equipment log with the
//! applicant's limits, leaves checks unassessed without them, and refuses malformed timelines.

use kshana_pro::avionics::procedure::{self, ParamsFile, Procedure};
use kshana_pro::avionics::{gbas, library};

fn params(v: &[(&str, f64)]) -> ParamsFile {
    ParamsFile {
        format: procedure::PARAMS_FORMAT.into(),
        supplied_by: "SYNTHETIC test values (not from any document)".into(),
        values: v.iter().map(|(k, x)| (k.to_string(), *x)).collect(),
        sources: Default::default(),
    }
}

#[test]
fn good_log_meets_bad_log_does_not_and_missing_limits_are_not_assessed() {
    let p = library::get("gbas-vdb-loss-and-fas").unwrap();
    let pf = params(&[
        ("vdb_loss_limit_s", 5.0),
        ("correction_timeout_s", 5.0),
        ("fas_alert_s", 5.0),
    ]);
    let ps = p.resolve_params(Some(&pf));
    let good = procedure::synth_log(&p, true).unwrap();
    let v = procedure::score(&p, &ps, Some(&good), None).unwrap();
    assert_eq!(v.verdict, "pass", "{:#?}", v.checks);
    assert_eq!(v.checks[0].id, "precondition-gbas-approach");
    assert_eq!(v.met, 4, "precondition and three events");
    let bad = procedure::synth_log(&p, false).unwrap();
    let v = procedure::score(&p, &ps, Some(&bad), None).unwrap();
    assert_eq!(v.verdict, "fail");
    assert_eq!(v.not_met, 3);
    // Without the applicant's limits nothing is assessed as met.
    let none = p.resolve_params(None);
    let v = procedure::score(&p, &none, Some(&good), None).unwrap();
    assert_eq!(v.verdict, "incomplete");
    // A limit tighter than the equipment's reaction fails it.
    let tight = p.resolve_params(Some(&params(&[
        ("vdb_loss_limit_s", 0.5),
        ("correction_timeout_s", 5.0),
        ("fas_alert_s", 5.0),
    ])));
    let v = procedure::score(&p, &tight, Some(&good), None).unwrap();
    assert_eq!(v.verdict, "fail");
}

#[test]
fn timeline_is_deterministic_and_refuses_malformed_specs() {
    let p = library::get("gbas-vdb-loss-and-fas").unwrap();
    let g = p.gbas.clone().unwrap();
    let a = gbas::generate(&g).unwrap();
    let b = gbas::generate(&g).unwrap();
    assert_eq!(a.frames_sha256, b.frames_sha256);
    assert!(a.frames.is_empty(), "no VDB message is encoded");
    assert_eq!(
        a.expectations
            .iter()
            .filter(|x| !x.limit_param.is_empty())
            .count(),
        3
    );
    let mut bad = g.clone();
    bad.events[0].kind = "jam".into();
    assert!(gbas::generate(&bad).is_err());
    let mut bad = g.clone();
    bad.events[0].t_s = 290.0;
    assert!(gbas::generate(&bad).is_err(), "beyond the duration");
    // A procedure that names a limit it does not declare is refused.
    let src = library::find("gbas-vdb-loss-and-fas")
        .unwrap()
        .replace("name = \"fas_alert_s\"", "name = \"other_s\"");
    assert!(Procedure::parse(&src).and_then(|p| p.validate()).is_err());
    // An informational event adds no scored check.
    let mut info = g;
    info.events.push(gbas::Event {
        id: "ssid".into(),
        kind: "ssid-mismatch".into(),
        t_s: 280.0,
        duration_s: 10.0,
    });
    let t = gbas::generate(&info).unwrap();
    assert!(t
        .expectations
        .iter()
        .any(|x| x.id == "ssid-info" && x.limit_param.is_empty()));
}
