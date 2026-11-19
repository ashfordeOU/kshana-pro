// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Holdover under attack: PRTC/ePRTC drag-and-step spoof and the substation PTP case (W1119 C8).

use kshana_pro::timing::attack::{run, AttackScenario, DetectorKind, Fallback, Spoof};

fn base() -> AttackScenario {
    AttackScenario::default()
}

fn with(det: DetectorKind, fb: Fallback) -> AttackScenario {
    let mut s = base();
    s.detector.kind = det;
    s.fallback = fb;
    s
}

#[test]
fn no_attack_stays_inside_the_prtc_a_mask() {
    let mut s = base();
    s.spoof = Spoof {
        enabled: false,
        ..Spoof::default()
    };
    let r = run(&s).unwrap();
    assert!(r.alarm_s.is_none());
    let a = r.masks.iter().find(|m| m.name == "PRTC-A").unwrap();
    assert!(a.met, "max |TE| {}", a.max_abs_te_ns);
    assert!(r.max_abs_te_after_attack_ns < 30.0);
}

#[test]
fn an_undetected_drag_and_step_move_the_clock_by_their_sum() {
    let s = with(DetectorKind::None, Fallback::Holdover);
    let r = run(&s).unwrap();
    assert!(r.alarm_s.is_none());
    let expect = s.spoof.drag_ns_per_s * (s.spoof.step_s - s.spoof.start_s) + s.spoof.step_ns;
    assert!(
        (r.final_te_ns - expect).abs() < 0.05 * expect,
        "final {} vs {expect}",
        r.final_te_ns
    );
    assert!(r
        .masks
        .iter()
        .all(|m| m.name == "TDD end application" || !m.met));
    let a = r.masks.iter().find(|m| m.name == "PRTC-A").unwrap();
    // The drag alone passes 100 ns after 500 s (0.2 ns/s).
    let first = a.first_exceeded_s.unwrap();
    assert!(
        (first - (s.spoof.start_s + 100.0 / s.spoof.drag_ns_per_s)).abs() < 120.0,
        "{first}"
    );
}

#[test]
fn the_rate_guard_catches_the_step_but_not_the_drag() {
    let s = with(DetectorKind::RateGuard, Fallback::Holdover);
    let r = run(&s).unwrap();
    let lat = r.latency_from_step_s.expect("caught at the step");
    assert!(
        (0.0..=s.detector.window_s + 2.0).contains(&lat),
        "latency {lat}"
    );
    // The drag had already moved the clock by about drag × (step − start) = 360 ns.
    let te = r.te_at_alarm_ns.unwrap();
    assert!((te - 360.0).abs() < 60.0, "TE at alarm {te}");
    assert!(!r.masks.iter().find(|m| m.name == "PRTC-A").unwrap().met);
}

#[test]
fn the_eloran_cross_check_catches_the_drag_and_eloran_holds_the_clock() {
    let mut s = with(DetectorKind::CrossCheck, Fallback::Eloran);
    s.detector.window_s = 120.0;
    let r = run(&s).unwrap();
    let lat = r.latency_from_start_s.expect("caught during the drag");
    // The window mean passes 50 ns when drag × (t − start) − half the window's ramp ≈ 50.
    let expect = s.detector.threshold_ns / s.spoof.drag_ns_per_s + s.detector.window_s / 2.0;
    assert!((lat - expect).abs() < 90.0, "latency {lat} vs {expect}");
    assert!(r.alarm_s.unwrap() < s.spoof.step_s, "before the step");
    // On eLoran the output settles to the eLoran bias (20 ns) and never sees the step.
    assert!(
        (r.final_te_ns - s.eloran.bias_ns).abs() < 25.0,
        "final {}",
        r.final_te_ns
    );
    assert!(
        r.max_abs_te_after_attack_ns < 150.0,
        "{}",
        r.max_abs_te_after_attack_ns
    );
    assert!(
        r.masks
            .iter()
            .find(|m| m.name == "TDD end application")
            .unwrap()
            .met
    );
}

#[test]
fn holdover_rollback_keeps_the_drag_out_of_the_holdover_frequency() {
    let mut a = with(DetectorKind::CrossCheck, Fallback::Holdover);
    a.detector.window_s = 120.0;
    a.spoof.drag_ns_per_s = 0.5;
    let mut b = a.clone();
    b.rollback_s = 0.0;
    let (ra, rb) = (run(&a).unwrap(), run(&b).unwrap());
    let drift = |r: &kshana_pro::timing::attack::AttackReport| {
        (r.final_te_ns - r.te_at_alarm_ns.unwrap()).abs()
    };
    assert!(
        drift(&ra) < 0.25 * drift(&rb),
        "rollback {} vs none {}",
        drift(&ra),
        drift(&rb)
    );
}

#[test]
fn substation_unprotected_breaks_the_budget_and_the_tlv_misleads() {
    let mut s = AttackScenario::substation();
    s.detector.kind = DetectorKind::None;
    let r = run(&s).unwrap();
    let p = r.ptp.unwrap();
    assert!(!p.within_budget);
    assert!(p.misleading_epochs > 1000);
    assert!((0.0..30.0).contains(&(p.first_misleading_s.unwrap() - 3600.0)));
}

#[test]
fn substation_with_the_guard_holds_the_budget_and_the_tlv_is_honest() {
    let r = run(&AttackScenario::substation()).unwrap();
    assert!((r.latency_from_step_s.unwrap()) <= 12.0);
    let p = r.ptp.unwrap();
    assert!(p.within_budget, "max |TE| {}", p.max_abs_te_ns);
    assert_eq!(p.misleading_epochs, 0, "{p:?}");
    assert!(r.masks[0].met);
}

#[test]
fn mtie_is_reported_and_non_decreasing() {
    let r = run(&with(DetectorKind::None, Fallback::Holdover)).unwrap();
    assert!(r.mtie.len() > 5);
    assert!(r
        .mtie
        .windows(2)
        .all(|w| w[1].value_ns >= w[0].value_ns - 1e-9));
    assert!(r.evidence.contains("illustrative"));
}
