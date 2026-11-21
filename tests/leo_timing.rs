// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro leo-timing` (E21 B2102): pre-registered LEO timing scorecard on synthetic logs.

use kshana_pro::leo_timing::{self, synth};

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "kshana-pro-leo-timing-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn synthetic_logs_score_as_modelled_with_every_part() {
    let d = scratch("synth");
    synth::write_all(&d).unwrap();
    let l = leo_timing::load(&d.join("plan.toml")).unwrap();
    let s = leo_timing::score(&l).unwrap();
    assert_eq!(s.evidence_class, "MODELLED");
    assert!(!s.all_logs_real);
    assert_eq!(s.environments.len(), 2);
    let open = s
        .environments
        .iter()
        .find(|e| e.environment == "open")
        .unwrap();
    let indoor = s
        .environments
        .iter()
        .find(|e| e.environment == "indoor")
        .unwrap();
    assert_eq!(open.availability, 1.0);
    assert!((indoor.availability - (3600.0 - 300.0) / 3600.0).abs() < 1e-12);
    assert!(indoor.max_abs_te_ns.unwrap() > open.max_abs_te_ns.unwrap());
    assert!(!open.mtie_ns.is_empty() && !open.tdev_ns.is_empty());
    // The indoor log has a gap, so its curves are not computed from unevenly spaced samples.
    assert!(indoor.mtie_ns.is_empty());
    assert_eq!(open.prtc_a_limit_ns, Some(100.0));
    let r = s.replay.as_ref().unwrap();
    assert_eq!(r.accepted_outside_window, 1);
    assert_eq!(r.outcome, "breached");
    assert_eq!(s.claims.len(), 2);
    assert!(s.claims.iter().all(|c| c.outcome != "not-evaluated"));
    let text = leo_timing::summary(&s);
    assert!(text.contains("MODELLED"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn validated_needs_real_logs_and_a_matching_registration() {
    let d = scratch("prereg");
    synth::write_all(&d).unwrap();
    let real = synth::PLAN.replace("synthetic = true", "synthetic = false");
    let p: leo_timing::Plan = toml::from_str(&real).unwrap();
    let hash = leo_timing::method_sha256(&p);
    // Not registered: MODELLED.
    std::fs::write(d.join("plan.toml"), &real).unwrap();
    let s = leo_timing::score(&leo_timing::load(&d.join("plan.toml")).unwrap()).unwrap();
    assert_eq!(s.evidence_class, "MODELLED");
    assert!(!s.preregistered);
    // Registered with the right hash: VALIDATED.
    let reg = format!("prereg_sha256 = \"{hash}\"\n{real}");
    std::fs::write(d.join("plan.toml"), &reg).unwrap();
    let s = leo_timing::score(&leo_timing::load(&d.join("plan.toml")).unwrap()).unwrap();
    assert!(s.preregistered);
    assert_eq!(s.evidence_class, "VALIDATED");
    // The method changed after registration (a looser threshold): the hash no longer matches.
    let moved = reg.replace("max_abs_te_ns = 1000.0", "max_abs_te_ns = 5000.0");
    std::fs::write(d.join("plan.toml"), &moved).unwrap();
    let s = leo_timing::score(&leo_timing::load(&d.join("plan.toml")).unwrap()).unwrap();
    assert!(!s.preregistered);
    assert_eq!(s.evidence_class, "MODELLED");
    // The logs are not part of the method hash.
    let mut p2 = p.clone();
    p2.log.clear();
    assert_eq!(leo_timing::method_sha256(&p2), hash);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn claims_follow_the_direction_of_their_metric() {
    let (s, _) = leo_timing::parse_te("t_s,te_ns,valid\n0,10,1\n1,-20,1\n2,5,0\n").unwrap();
    let th = leo_timing::Thresholds {
        max_abs_te_ns: 15.0,
        availability_min: 0.9,
    };
    let e = leo_timing::score_env("urban", &[(s, 0)], &th);
    assert_eq!(e.max_abs_te_ns, Some(20.0));
    assert_eq!(e.threshold_check, "exceeds");
    assert_eq!(e.availability_check, "below");
    assert!((e.rms_te_ns.unwrap() - (250.0f64).sqrt()).abs() < 1e-12);
}

#[test]
fn the_replay_check_counts_what_should_have_been_refused() {
    let log = "rx_s,tx_s,accepted\n10,9,1\n50,9,1\n8,9,1\n20,9,0\n100,9,0\n";
    let r = leo_timing::replay_check(log, 30.0, true).unwrap();
    assert_eq!(r.messages, 5);
    assert_eq!(r.accepted, 3);
    assert_eq!(r.accepted_outside_window, 1);
    assert_eq!(r.accepted_negative_latency, 1);
    assert_eq!(r.refused_inside_window, 1);
    assert_eq!(r.max_accepted_latency_s, Some(41.0));
    assert_eq!(r.outcome, "breached");
    assert!(leo_timing::replay_check(log, 0.0, true).is_err());
}

#[test]
fn bad_logs_and_plans_are_refused() {
    assert!(leo_timing::parse_te("").is_err());
    assert!(leo_timing::parse_te("t_s,te_ns\n0,1\n").is_err());
    assert!(leo_timing::parse_te("t_s,te_ns,valid\n1,0,1\n0,0,1\n").is_err());
    let (_, bad) = leo_timing::parse_te("t_s,te_ns,valid\n0,1,1\n1,x,1\n2,1,1\n").unwrap();
    assert_eq!(bad, 1);
    let d = scratch("bad");
    synth::write_all(&d).unwrap();
    let plan = synth::PLAN.replace("metric = \"availability\"", "metric = \"accuracy\"");
    std::fs::write(d.join("plan.toml"), plan).unwrap();
    assert!(leo_timing::load(&d.join("plan.toml")).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

/// Frame audit: the replay latency is taken on one time scale. Transmit times in GPST against
/// receive times in UTC look like 18 s replays until the scale offset is stated.
#[test]
fn replay_latency_needs_one_time_scale() {
    let mut log = String::from("rx_s,tx_s,accepted\n");
    for k in 0..20 {
        let tx = 1000.0 + k as f64 * 10.0;
        // Received 0.4 s after transmission; transmit time written in GPST (UTC + 18 s).
        log.push_str(&format!("{},{},1\n", tx + 0.4, tx + 18.0));
    }
    let naive = leo_timing::replay_check(&log, 5.0, true).unwrap();
    assert_eq!(naive.outcome, "breached");
    assert!(naive.time_scale.contains("ASSUMED"));
    assert!(
        naive.scale_warning.as_deref().unwrap().contains("-18"),
        "{naive:?}"
    );
    let ok =
        leo_timing::replay_check_scaled(&log, 5.0, true, Some("UTC receive, GPST transmit"), 18.0)
            .unwrap();
    assert_eq!(ok.outcome, "held", "{ok:?}");
    assert!(ok.scale_warning.is_none());
    assert!((ok.max_accepted_latency_s.unwrap() - 0.4).abs() < 1e-9);
    assert!(leo_timing::replay_check_scaled(&log, 5.0, true, None, f64::NAN).is_err());
}
