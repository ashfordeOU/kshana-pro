// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Pro OSNMA scenario pack (research 2026-11-20 gaps B1402): every scenario's expected states
//! are reproduced by the reference model, the cryptographic premises hold on the open engine's
//! TESLA functions, and the scorer separates a well-behaved SYNTHETIC receiver from a faulty one
//! (unsafe subframes after a delayed replay and on a revoked chain).

use std::process::Command;

use kshana_pro::osnma_scenarios::{self, Model};

#[test]
fn reference_model_reproduces_every_expected_table() {
    let m = Model::default();
    let sc = osnma_scenarios::scenarios();
    let ids: Vec<&str> = sc.iter().map(|s| s.id).collect();
    for want in [
        "startup-cold",
        "key-renewal",
        "chain-revocation",
        "subframe-loss",
        "replay-delayed",
        "meaconing-short",
        "time-sync-loss",
        "time-sync-attack",
        "pk-revocation",
    ] {
        assert!(ids.contains(&want), "{want}");
    }
    for s in &sc {
        assert_eq!(s.expected.len(), s.subframes.len(), "{}", s.id);
        assert_eq!(
            osnma_scenarios::expected_states(&s.subframes, &m),
            s.expected,
            "{}",
            s.id
        );
    }
    let p = osnma_scenarios::pack();
    assert!(p.model_agrees.values().all(|v| *v));
    let c = &p.crypto;
    assert!(c.key_verifies_across_three_lost_subframes);
    assert!(c.revoked_chain_key_fails_against_new_root);
    assert!(c.replayed_old_key_fails_as_current);
    assert!(c.scorecard_stimulus_check.tesla_genuine_key_verifies);
    assert_eq!(p.not_covered.len(), 2);
}

#[test]
fn scorer_separates_good_and_faulty_receivers() {
    let good = osnma_scenarios::synth_log(true);
    let s = osnma_scenarios::score(&good, b"good").unwrap();
    assert!(
        s.scenarios.iter().all(|x| x.verdict == "met"),
        "{:#?}",
        s.scenarios
    );
    let bad = osnma_scenarios::synth_log(false);
    let s = osnma_scenarios::score(&bad, b"bad").unwrap();
    let r = |id: &str| s.scenarios.iter().find(|x| x.scenario == id).unwrap();
    assert_eq!(r("replay-delayed").verdict, "not met");
    assert_eq!(r("replay-delayed").unsafe_subframes.len(), 19);
    assert_eq!(
        r("chain-revocation").unsafe_subframes,
        vec![19, 20, 21, 22, 23]
    );
    assert_eq!(r("key-renewal").verdict, "met");
}

#[test]
fn cli_lists_free_and_scores_signed() {
    let d = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("b1402-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let home = d.join("home");
    let run = |a: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(a)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr),
        )
    };
    let (ok, m) = run(&["osnma-pack", "list"]);
    assert!(ok && m.contains("chain-revocation"), "{m}");
    let log = d.join("log.json");
    std::fs::write(
        &log,
        serde_json::to_vec(&osnma_scenarios::synth_log(true)).unwrap(),
    )
    .unwrap();
    let seed = [2u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("key.txt"), format!("private_key={hex}\n")).unwrap();
    let out = d.join("score");
    let kf = d.join("key.txt");
    let args = [
        "osnma-pack",
        "score",
        log.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "t",
        "--key-file",
        kf.to_str().unwrap(),
        "--utc",
        "2026-11-20T00:00:00Z",
    ];
    assert!(!run(&args).0, "needs a licence");
    let mut a = args.to_vec();
    a.push("--dev");
    let (ok, m) = run(&a);
    assert!(ok && m.contains("9 met"), "{m}");
    assert!(out.join("pack.json").is_file());
    let v = kshana_pro::signed_dossier::verify(&out, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
}
