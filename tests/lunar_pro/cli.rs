// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// `kshana-pro lunar ...`: licence gating and the free commands (part of the tests/lunar_pro.rs
// test binary).

use std::process::ExitCode;

fn run(args: &[&str]) -> ExitCode {
    kshana_pro::cli::main_with_args(args.iter().map(|s| s.to_string()).collect())
}

fn ok(c: ExitCode) -> bool {
    c == ExitCode::SUCCESS
}

#[test]
fn cli_lunar_gates_computing_commands_and_frees_verification() {
    // A bad licence token is refused for a computing command.
    assert!(!ok(run(&["lunar", "rates", "--license", "not-a-token"])));
    // The development entitlement (debug builds) runs it.
    if cfg!(debug_assertions) {
        assert!(ok(run(&["lunar", "rates", "--dev"])));
        assert!(ok(run(&[
            "lunar",
            "time",
            "--from",
            "utc",
            "--to",
            "tcl",
            "--at",
            "2026-11-19T00:00:00",
            "--dev"
        ])));
        assert!(ok(run(&[
            "lunar",
            "holdover",
            "--clock",
            "rafs",
            "--link-ns",
            "1",
            "--interval",
            "3600",
            "--dev"
        ])));
    }
    // Decoding a KLTO-1 message needs no licence.
    let v: kshana_pro::lunar_pro::offset_codec::VectorFile = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/lunar_pro/klto1_conformance_vectors.json"
        ))
        .unwrap(),
    )
    .unwrap();
    assert!(ok(run(&["lunar", "offset", "decode", &v.vectors[0].hex])));
    assert!(!ok(run(&["lunar", "offset", "decode", "00"])));
    // Validating a LUNEX file needs no licence; an invalid file fails.
    let dir = std::env::temp_dir().join(format!("kshana-lunar-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv = "time,sv,signal,pseudorange_m,carrier_cycles,doppler_hz,cn0_dbhz\n\
               2025-03-03T10:00:00,G05,1C,380000123.456,,-650.2,24.1\n";
    let l = kshana_pro::lunar_pro::lunex::from_csv(csv, &Default::default()).unwrap();
    let good = dir.join("good.lnx");
    std::fs::write(&good, l.to_text()).unwrap();
    assert!(ok(run(&[
        "lunar",
        "lunex",
        "validate",
        good.to_str().unwrap()
    ])));
    let bad = dir.join("bad.lnx");
    std::fs::write(&bad, l.to_text().replace("MOON_PA_DE440", "MOON_XX")).unwrap();
    assert!(!ok(run(&[
        "lunar",
        "lunex",
        "validate",
        bad.to_str().unwrap()
    ])));
    // The synthetic example is free and feeds the Pro chain.
    let ex = dir.join("ex");
    assert!(ok(run(&[
        "lunar",
        "circular",
        "example",
        "--out",
        ex.to_str().unwrap()
    ])));
    if cfg!(debug_assertions) {
        let c = ex.join("comparisons.csv");
        let e = ex.join("earth.csv");
        let o = ex.join("out");
        assert!(ok(run(&[
            "lunar",
            "circular",
            "--comparisons",
            c.to_str().unwrap(),
            "--earth-csv",
            e.to_str().unwrap(),
            "--month",
            "2026-11",
            "--out",
            o.to_str().unwrap(),
            "--w0",
            "conventional:2821013.7",
            "--dev",
        ])));
        let text = std::fs::read_to_string(o.join("circular.txt")).unwrap();
        let sec = kshana::realdata::clk::parse_circular_t_section1(&text).unwrap();
        assert_eq!(sec.rows.len(), 2);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

pub(crate) fn key_file(dir: &std::path::Path) -> (std::path::PathBuf, [u8; 32]) {
    let seed = [7u8; 32];
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join("key");
    std::fs::write(&p, format!("private_key={}\n", hex::encode(seed))).unwrap();
    (p, seed)
}

pub(crate) fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn cli_lunar_lugre_example_is_free_and_score_is_gated_and_signed() {
    let dir = std::env::temp_dir().join(format!("kshana-lunar-cli-d6-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ex = dir.join("ex");
    assert!(ok(run(&[
        "lunar",
        "lugre",
        "example",
        "--out",
        ex.to_str().unwrap()
    ])));
    let sha = std::fs::read_to_string(ex.join("prereg.sha256")).unwrap();
    let p = |n: &str| ex.join(n).to_str().unwrap().to_string();
    let out = dir.join("score");
    let (kf, seed) = key_file(&dir);
    let (pre, model, sp3, obs) = (
        p("prereg.toml"),
        p("model.toml"),
        p("gnss.sp3"),
        p("obs.csv"),
    );
    let args = |lic: &str| {
        s(&[
            "lunar",
            "lugre",
            "score",
            "--prereg",
            &pre,
            "--prereg-sha256",
            sha.trim(),
            "--model",
            &model,
            "--sp3",
            &sp3,
            "--obs",
            &obs,
            "--out",
            out.to_str().unwrap(),
            "--signer",
            "test",
            "--key-file",
            kf.to_str().unwrap(),
            "--utc",
            "2026-11-19T00:00:00Z",
            lic,
        ])
    };
    assert!(!ok(kshana_pro::cli::main_with_args(args("--no-licence"))));
    if cfg!(debug_assertions) {
        assert!(ok(kshana_pro::cli::main_with_args(args("--dev"))));
        let pk = kshana_pro::license::public_key_for(&seed);
        let v = kshana_pro::signed_dossier::verify(&out, &[pk]).unwrap();
        assert!(v.passed(), "{:?}", v.problems);
        let r: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap())
                .unwrap();
        assert!(r["overall"]["overlap_published"].as_bool().unwrap());
        let pred = dir.join("pred.csv");
        assert!(ok(run(&[
            "lunar",
            "lugre",
            "predict",
            "--model",
            &model,
            "--sp3",
            &sp3,
            "--start",
            "2025-03-03T00:00:00",
            "--end",
            "2025-03-03T01:00:00",
            "--step",
            "600",
            "--out",
            pred.to_str().unwrap(),
            "--dev"
        ])));
        assert!(std::fs::read_to_string(&pred).unwrap().contains("cn0_dbhz"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
