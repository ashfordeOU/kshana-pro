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
