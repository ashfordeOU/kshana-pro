// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Timing evidence packs (W1119 C4): MiFID II RTS 25, DORA RTS 2024/1774 Art. 12, the EU CER
//! and the India draft IST rules, built from synthetic inputs (a clock record, a Circular T
//! layout with made-up numbers, an open-engine holdover-test run, a PICS spoof scorecard and
//! a fleet assessment), with the verdicts pinned, the wording rule, and the signed pack.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::pics::log::{self, Behaviour};
use kshana_pro::pics::{score, Mapping, Suite, LOCK_V1, SUITE_V1};
use kshana_pro::timing::audit::FORBIDDEN_WORDS;
use kshana_pro::timing::compliance::{self, Inputs, Pack, Verdict, TO_CONFIRM};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("tc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// Layout of a Circular T text issue (as the open engine's parser expects); numbers made up.
const CIRT: &str = "CIRCULAR T 999                                   ISSN 1143-1393
2026 NOVEMBER 10, 07h UTC

1 - Difference between UTC and its local realizations UTC(k) and corresponding uncertainties.

Date 2026    0h UTC         OCT 4   OCT 9   Uncertainty/ns Notes
       MJD                   61317    61322     uA    uB    u
Laboratory k                       [UTC-UTC(k)]/ns

NPL  (Teddington)              3.2     -1.5    0.2   1.0   1.0
NPLI (New Delhi)               8.0      6.5    0.5   4.0   4.0

2 - Something else
";

/// Write the fixtures: a clock record with offsets of amplitude `amp_s`, Circular T, a
/// holdover-test result whose holdover drifts at `drift_ns_per_s`, a PICS scorecard of a good
/// (or bad) timing device, and a fleet assessment with `breached` clocks.
fn fixtures(d: &Path, amp_s: f64, drift_ns_per_s: f64, good_spoof: bool, breached: usize) {
    let clock: String = (0..3600)
        .map(|i| format!("{:e}\n", amp_s * ((i as f64) * 0.01).sin()))
        .collect();
    std::fs::write(d.join("clock.txt"), clock).unwrap();
    std::fs::write(d.join("cirt.txt"), CIRT).unwrap();
    // Holdover test: GNSS removed at 600 s, restored at 1600 s, 1 s samples.
    let samples: Vec<String> = (0..=2400)
        .map(|i| {
            let t = i as f64;
            let te = if (600.0..1600.0).contains(&t) {
                1.0 + drift_ns_per_s * (t - 600.0)
            } else {
                1.0 + 0.5 * (t * 0.3).sin()
            };
            format!("[{t}, {te}]")
        })
        .collect();
    let scen = format!(
        "kind = \"holdover-test\"\n[test]\ngnss_removed_s = 600.0\ngnss_restored_s = 1600.0\n[series]\nsamples = [{}]\n",
        samples.join(", ")
    );
    let out = kshana::api::run_scenario(&scen).expect("holdover-test runs");
    std::fs::write(d.join("holdover.json"), out.json).unwrap();
    // Spoof test: PICS time push and meaconing on a synthetic device.
    let suite = Suite::parse(SUITE_V1).unwrap();
    let b = if good_spoof {
        Behaviour::Good
    } else {
        Behaviour::Bad
    };
    let logs: Vec<_> = ["PICS-SPF-TPUSH", "PICS-SPF-MEACON"]
        .iter()
        .map(|id| log::synth(&suite, suite.scenario(id).unwrap(), b, 3.0))
        .collect();
    let sc = score::score(SUITE_V1, LOCK_V1, &Mapping::builtin(), &logs).unwrap();
    std::fs::write(
        d.join("spoof.json"),
        serde_json::to_string_pretty(&sc).unwrap(),
    )
    .unwrap();
    std::fs::write(
        d.join("fleet.json"),
        format!(
            "{{\"format\":\"kshana-pro-timing-fleet/1\",\"counts\":{{\"clocks\":3,\"ok\":{},\"watch\":0,\"act\":0,\"breached\":{breached}}}}}",
            3 - breached
        ),
    )
    .unwrap();
}

fn inputs(lab: &str, extra_operator: &str) -> String {
    format!(
        "organisation = \"SYNTHETIC Example Venue\"\nperiod = \"2026\"\n\
[clock]\nreference = \"UTC({lab})\"\n[clock.source]\nformat = \"timelab\"\npath = \"clock.txt\"\ntau0_s = 1.0\n\
[circular_t]\npath = \"cirt.txt\"\nlab = \"{lab}\"\n\
[holdover]\npath = \"holdover.json\"\n[spoof]\npath = \"spoof.json\"\n[fleet]\npath = \"fleet.json\"\n\
[operator]\n{extra_operator}\n"
    )
}

const UTC: &str = "2026-11-19T12:00:00Z";

fn build(fw: &str, d: &Path, src: &str) -> Pack {
    compliance::build(fw, &Inputs::parse(src).unwrap(), d, UTC).unwrap()
}

fn verdict(p: &Pack, id: &str) -> Verdict {
    p.requirements
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .verdict
}

const MIFID_OP: &str = "utc_source = \"UTC(NPL) via NPLTime\"\ntraceability_chain = \"GM -> PTP -> servers\"\nreview_date = \"2026-03-01\"\nactivity = \"high-frequency\"\ntimestamp_granularity_s = 1e-6";

#[test]
fn rts25_pack_bounds_the_divergence_to_utc_with_the_circular_t_term() {
    let d = tmp("rts25");
    fixtures(&d, 20e-6, 0.02, true, 0);
    let p = build("mifid2-rts25", &d, &inputs("NPL", MIFID_OP));
    assert!(p.evidence.problems.is_empty(), "{:?}", p.evidence.problems);
    assert_eq!(verdict(&p, "RTS25-1"), Verdict::Supported);
    assert_eq!(verdict(&p, "RTS25-2"), Verdict::Supported);
    assert_eq!(verdict(&p, "RTS25-3"), Verdict::OperatorStatement);
    assert_eq!(verdict(&p, "RTS25-4"), Verdict::OperatorStatement);
    assert_eq!(verdict(&p, "RTS25-5"), Verdict::Supported);
    // The bound is max |offset| + max |UTC - UTC(NPL)| + 2u = 20 us + 3.2 ns + 2 ns.
    let c = p.evidence.circular_t.as_ref().unwrap();
    assert_eq!(c.max_abs_utc_minus_utck_ns, Some(3.2));
    assert!((c.bound_s().unwrap() - 5.2e-9).abs() < 1e-15);
    let div = p.evidence.divergence.as_ref().unwrap();
    assert!(
        (div.max_abs_offset_s - 20e-6).abs() < 1e-8,
        "{}",
        div.max_abs_offset_s
    );
    let r2 = p.requirements.iter().find(|r| r.id == "RTS25-2").unwrap();
    assert!(
        r2.limit.as_deref().unwrap().contains("100.000 µs"),
        "{:?}",
        r2.limit
    );
    assert!(
        r2.evidence.iter().any(|e| e.contains("20.005 µs")),
        "{:?}",
        r2.evidence
    );
    assert!(r2.limit_basis.contains("kshana::rts25"));
    // Every input file is listed with its hash.
    assert_eq!(p.evidence.files.len(), 5);
    assert_eq!(p.counts.supported, 3);
}

#[test]
fn rts25_verdicts_follow_the_activity_class_and_the_missing_evidence() {
    let d = tmp("rts25-bad");
    fixtures(&d, 150e-6, 0.02, true, 0);
    // 150 us is beyond the high-frequency 100 us limit but within the 1 ms of other trading.
    let p = build("mifid2-rts25", &d, &inputs("NPL", MIFID_OP));
    assert_eq!(verdict(&p, "RTS25-2"), Verdict::NotSupported);
    let p = build(
        "mifid2-rts25",
        &d,
        &inputs("NPL", &MIFID_OP.replace("high-frequency", "other-trading")),
    );
    assert_eq!(verdict(&p, "RTS25-2"), Verdict::Supported);
    // Coarse granularity, a stale review, no Circular T, an unknown activity.
    let op = MIFID_OP
        .replace("1e-6", "1e-3")
        .replace("2026-03-01", "2025-06-01");
    let p = build("mifid2-rts25", &d, &inputs("NPL", &op));
    assert_eq!(verdict(&p, "RTS25-3"), Verdict::NotSupported);
    assert_eq!(verdict(&p, "RTS25-4"), Verdict::NotSupported);
    let src =
        inputs("NPL", MIFID_OP).replace("[circular_t]\npath = \"cirt.txt\"\nlab = \"NPL\"\n", "");
    let d2 = tmp("rts25-noct");
    fixtures(&d2, 20e-6, 0.02, true, 0);
    let p = build("mifid2-rts25", &d2, &src);
    assert_eq!(verdict(&p, "RTS25-1"), Verdict::OperatorStatement);
    assert_eq!(verdict(&p, "RTS25-2"), Verdict::Insufficient);
    let e = compliance::build(
        "mifid2-rts25",
        &Inputs::parse(&inputs("NPL", "activity = \"crypto\"")).unwrap(),
        &d,
        UTC,
    );
    assert!(e.is_err());
    // A laboratory missing from Circular T is an input problem.
    let p = build("mifid2-rts25", &d, &inputs("XYZ", MIFID_OP));
    assert!(p
        .evidence
        .problems
        .iter()
        .any(|x| x.contains("XYZ is not in section 1")));
    assert_eq!(verdict(&p, "RTS25-1"), Verdict::OperatorStatement);
}

#[test]
fn dora_pack_uses_the_operator_tolerance_and_marks_the_limit_unconfirmed() {
    let d = tmp("dora");
    fixtures(&d, 20e-6, 0.02, true, 0);
    let op = "reference_time_source = \"UTC(NPL) via NPLTime, documented in ICT-TIME-01\"\nsync_tolerance_s = 50e-6\ninventory_complete = true";
    let p = build("dora-rts-2024-1774", &d, &inputs("NPL", op));
    assert_eq!(verdict(&p, "DORA12-1"), Verdict::Supported);
    assert_eq!(verdict(&p, "DORA12-2"), Verdict::Supported);
    assert_eq!(verdict(&p, "DORA12-3"), Verdict::Supported);
    assert_eq!(verdict(&p, "DORA12-4"), Verdict::OperatorStatement);
    for id in ["DORA12-2", "DORA12-3"] {
        let r = p.requirements.iter().find(|r| r.id == id).unwrap();
        assert!(r.limit_basis.contains(TO_CONFIRM), "{id}");
    }
    let p = build(
        "dora-rts-2024-1774",
        &d,
        &inputs("NPL", &op.replace("50e-6", "10e-6")),
    );
    assert_eq!(verdict(&p, "DORA12-2"), Verdict::NotSupported);
    let p = build(
        "dora-rts-2024-1774",
        &d,
        &inputs("NPL", "reference_time_source = \"x\""),
    );
    assert_eq!(verdict(&p, "DORA12-2"), Verdict::Insufficient);
    assert_eq!(verdict(&p, "DORA12-4"), Verdict::ToComplete);
    let d2 = tmp("dora-breach");
    fixtures(&d2, 20e-6, 0.02, true, 1);
    let p = build("dora-rts-2024-1774", &d2, &inputs("NPL", op));
    assert_eq!(verdict(&p, "DORA12-2"), Verdict::NotSupported);
}

#[test]
fn cer_pack_reports_the_gnss_loss_impact_and_the_spoof_result() {
    let d = tmp("cer");
    fixtures(&d, 20e-6, 0.02, true, 0);
    let op = "pnt_dependency_register = \"Risk register R-17, timing dependencies\"";
    let p = build("eu-cer", &d, &inputs("NPL", op));
    assert_eq!(verdict(&p, "CER-1"), Verdict::Supported);
    assert_eq!(verdict(&p, "CER-2"), Verdict::Supported);
    assert_eq!(verdict(&p, "CER-3"), Verdict::Supported);
    let h = p.evidence.holdover.as_ref().unwrap();
    assert!(h.held(), "{:?}", h.budgets);
    // A holdover drifting 1 ns/s passes the 100 ns budget after about 100 s; a device that
    // follows the spoofer is not detected.
    let d2 = tmp("cer-bad");
    fixtures(&d2, 20e-6, 1.0, false, 0);
    let p = build("eu-cer", &d2, &inputs("NPL", op));
    assert_eq!(
        verdict(&p, "CER-2"),
        Verdict::Supported,
        "the impact is measured either way"
    );
    let r2 = p.requirements.iter().find(|r| r.id == "CER-2").unwrap();
    assert!(r2.reason.contains("exceeded after"), "{}", r2.reason);
    let first = p
        .evidence
        .holdover
        .as_ref()
        .unwrap()
        .budgets
        .iter()
        .filter(|b| b.exceeded)
        .filter_map(|b| b.time_to_exceed_s)
        .fold(f64::INFINITY, f64::min);
    assert!((95.0..=105.0).contains(&first), "{first}");
    assert_eq!(verdict(&p, "CER-3"), Verdict::NotSupported);
    assert_eq!(p.evidence.spoof.as_ref().unwrap().detected, 0);
    let p = build("eu-cer", &d, &inputs("NPL", ""));
    assert_eq!(verdict(&p, "CER-1"), Verdict::Insufficient);
    assert!(p
        .requirements
        .iter()
        .all(|r| r.limit_basis.contains(TO_CONFIRM)));
}

#[test]
fn india_pack_checks_npl_traceability_and_foreign_references() {
    let d = tmp("ist");
    fixtures(&d, 20e-6, 0.02, true, 0);
    let op = "npl_traceability = \"NPL India time dissemination, certificate T-2026-04\"\nprimary_reference = \"NPL time service over NTP/PTP\"\nnavic_receiver = \"dual NavIC/GPS timing receiver, NavIC primary\"";
    let p = build("india-ist-draft", &d, &inputs("NPLI", op));
    assert_eq!(verdict(&p, "IST-1"), Verdict::Supported);
    assert_eq!(verdict(&p, "IST-2"), Verdict::OperatorStatement);
    assert_eq!(verdict(&p, "IST-3"), Verdict::OperatorStatement);
    assert_eq!(verdict(&p, "IST-4"), Verdict::Supported);
    assert!(p
        .requirements
        .iter()
        .all(|r| r.limit_basis.contains(TO_CONFIRM)));
    let p = build(
        "india-ist-draft",
        &d,
        &inputs("NPLI", &op.replace("NPL time service over NTP/PTP", "GPS")),
    );
    assert_eq!(verdict(&p, "IST-2"), Verdict::NotSupported);
    let p = build(
        "india-ist-draft",
        &d,
        &inputs(
            "NPLI",
            &format!(
                "{}\nforeign_reference_approval = \"approval 12/2026\"",
                op.replace("NPL time service over NTP/PTP", "GPS")
            ),
        ),
    );
    assert_eq!(verdict(&p, "IST-2"), Verdict::OperatorStatement);
    // A Circular T entry of another laboratory does not show traceability to NPL India.
    let p = build("india-ist-draft", &d, &inputs("NPL", op));
    assert_eq!(verdict(&p, "IST-1"), Verdict::Insufficient);
}

#[test]
fn no_pack_says_compliant_or_certified() {
    let d = tmp("words");
    fixtures(&d, 20e-6, 0.02, true, 0);
    for (fw, _, _) in compliance::FRAMEWORKS {
        let op = "utc_source = \"x\"\nreference_time_source = \"x\"\nactivity = \"high-frequency\"";
        let p = build(fw, &d, &inputs("NPL", op));
        let text = format!(
            "{}\n{}\n{}",
            serde_json::to_string(&p).unwrap(),
            compliance::html(&p),
            compliance::requirements_csv(&p)
        )
        .to_lowercase();
        for w in FORBIDDEN_WORDS {
            assert!(!text.contains(w), "{fw}: '{w}'");
        }
        assert!(!p.requirements.is_empty());
        assert!(p
            .requirements
            .iter()
            .all(|r| !r.source.is_empty() && !r.limit_basis.is_empty()));
    }
    assert!(compliance::build("sox", &Inputs::default(), &d, UTC).is_err());
}

#[test]
fn cli_writes_a_signed_pack_and_a_changed_input_fails_verification() {
    let d = tmp("cli");
    fixtures(&d, 20e-6, 0.02, true, 0);
    std::fs::write(d.join("inputs.toml"), inputs("NPL", MIFID_OP)).unwrap();
    let key = d.join("key.txt");
    let seed = [5u8; 32];
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    std::fs::write(&key, format!("private_key={}\n", hex(&seed))).unwrap();
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
    let (ok, out) = run(&["timing", "compliance", "list"]);
    assert!(
        ok && out.contains("dora-rts-2024-1774") && out.contains("india-ist-draft"),
        "{out}"
    );
    let outdir = d.join("pack");
    let args = [
        "timing",
        "compliance",
        "mifid2-rts25",
        d.join("inputs.toml").to_str().unwrap().to_string().leak(),
        "--out",
        outdir.to_str().unwrap().to_string().leak(),
        "--signer",
        "Auditor",
        "--key-file",
        key.to_str().unwrap().to_string().leak(),
        "--utc",
        UTC,
    ];
    let (ok, out) = run(&args);
    assert!(!ok, "needs a licence: {out}");
    let mut dev = args.to_vec();
    dev.push("--dev");
    let (ok, out) = run(&dev);
    assert!(ok, "{out}");
    assert!(out.contains("3 supported"), "{out}");
    for f in [
        "evidence-pack.json",
        "evidence-pack.html",
        "requirements.csv",
        "inputs/inputs.toml",
        "inputs/clock.txt",
        "inputs/cirt.txt",
        "inputs/holdover.json",
        "inputs/spoof.json",
        "inputs/fleet.json",
        "signature.json",
    ] {
        assert!(outdir.join(f).exists(), "{f}");
    }
    let pk = hex(&kshana_pro::license::public_key_for(&seed));
    let (ok, out) = run(&[
        "dossier",
        "verify",
        outdir.to_str().unwrap(),
        "--trust",
        &pk,
    ]);
    assert!(ok, "{out}");
    std::fs::write(outdir.join("inputs/clock.txt"), "0\n").unwrap();
    let (ok, out) = run(&[
        "dossier",
        "verify",
        outdir.to_str().unwrap(),
        "--trust",
        &pk,
    ]);
    assert!(!ok && out.contains("clock.txt"), "{out}");
}
