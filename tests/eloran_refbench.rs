// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! eLoran-Bench on the command line: the list is free; the in-house receiver's reference
//! observations score PASS on every pre-registered bar; the scorecard is signed with a dossier
//! key and verifies offline; a leaderboard is written; a bar changed after registration makes
//! scoring refuse.

use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .env_remove("KSHANA_PRO_LICENSE")
        .output()
        .expect("run kshana-pro")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-elb-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn bench_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates/kshana-eloran/scenarios/bench-eloran")
}

#[test]
fn list_reference_score_sign_and_tamper() {
    let o = run(&["eloran", "bench-eloran", "list"]);
    assert!(o.status.success(), "{}", text(&o));
    let t = text(&o);
    for id in [
        "ELB-STATIC",
        "ELB-SHIP",
        "ELB-DRONE",
        "ELB-SKY-NIGHT",
        "ELB-CRI",
        "ELB-IMPULSIVE",
        "ELB-LDC",
        "ELB-MULTI-UTCK",
    ] {
        assert!(t.contains(id), "{id}: {t}");
    }
    assert!(t.contains("illustrative"), "{t}");

    let d = tmp("main");
    let obs = d.join("obs");
    let out = d.join("card");
    // scoring is licensed
    let o = run(&[
        "eloran",
        "bench-eloran",
        "score",
        "--obs",
        obs.to_str().unwrap(),
        "--receiver",
        "x",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(!o.status.success(), "{}", text(&o));
    let o = run(&[
        "eloran",
        "bench-eloran",
        "reference",
        "--out",
        obs.to_str().unwrap(),
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    // every reference file is valid ELOF
    let o = run(&[
        "eloran",
        "elof",
        "validate",
        obs.join("ELB-LDC.elof.jsonl").to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", text(&o));
    // a signing key
    let o = run(&["dossier", "keygen"]);
    assert!(o.status.success());
    let keys = String::from_utf8_lossy(&o.stdout).to_string();
    let kf = d.join("key.txt");
    std::fs::write(&kf, keys.lines().next().unwrap()).unwrap();
    let pubkey = keys
        .lines()
        .find_map(|l| l.strip_prefix("public_key="))
        .unwrap()
        .to_string();
    let lb = d.join("leaderboard.json");
    let o = run(&[
        "eloran",
        "bench-eloran",
        "score",
        "--obs",
        obs.to_str().unwrap(),
        "--receiver",
        "kshana-software-rx",
        "--out",
        out.to_str().unwrap(),
        "--key-file",
        kf.to_str().unwrap(),
        "--signer",
        "Kshana bench",
        "--leaderboard",
        lb.to_str().unwrap(),
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        text(&o).contains("0 fail, 0 not evaluated (8/8"),
        "{}",
        text(&o)
    );
    let card: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("scorecard.json")).unwrap())
            .unwrap();
    assert_eq!(card["format"], "eloran-bench-scorecard");
    assert_eq!(card["scenarios_passed"], 8);
    assert_eq!(card["seal_sha256"].as_str().unwrap().len(), 64);
    let board: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&lb).unwrap()).unwrap();
    assert_eq!(board["entries"][0]["rank"], 1);
    assert_eq!(board["entries"][0]["score"], 1.0);
    // the signed scorecard verifies offline, and a change is detected
    let o = run(&[
        "dossier",
        "verify",
        out.to_str().unwrap(),
        "--trust",
        &pubkey,
    ]);
    assert!(o.status.success(), "{}", text(&o));
    let sc = out.join("scorecard.json");
    let body = std::fs::read_to_string(&sc).unwrap();
    std::fs::write(&sc, body.replacen("\"fail\": 0", "\"fail\": 1", 1)).unwrap();
    let o = run(&[
        "dossier",
        "verify",
        out.to_str().unwrap(),
        "--trust",
        &pubkey,
    ]);
    assert!(!o.status.success(), "{}", text(&o));

    // a relaxed bar after registration: scoring refuses
    let bd = d.join("bench");
    std::fs::create_dir_all(&bd).unwrap();
    for e in std::fs::read_dir(bench_dir()).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(&p, bd.join(p.file_name().unwrap())).unwrap();
    }
    let bars = bd.join("bars.toml");
    let b = std::fs::read_to_string(&bars).unwrap();
    std::fs::write(&bars, b.replacen("limit = 0.01", "limit = 0.5", 1)).unwrap();
    let o = run(&[
        "eloran",
        "bench-eloran",
        "score",
        "--obs",
        obs.to_str().unwrap(),
        "--receiver",
        "x",
        "--out",
        d.join("card2").to_str().unwrap(),
        "--bench",
        bd.to_str().unwrap(),
        "--dev",
    ]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("pre-registered"), "{}", text(&o));
    let _ = std::fs::remove_dir_all(&d);
}
