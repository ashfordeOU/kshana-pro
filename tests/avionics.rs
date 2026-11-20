// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The avionics test library (`kshana_pro::avionics`): SBAS framing and the message-sequence
//! generator, the procedures and their scoring, the ARAIM checks, the recovery-time scorecard,
//! the evidence pack, the tool-qualification support kit and the CLI.
//! Evidence: MODELLED (synthetic equipment logs).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::avionics::procedure::{self, DutLog, ParamsFile, Procedure};
use kshana_pro::avionics::sbas_seq::{self, EventKind, ExpectKind};
use kshana_pro::avionics::{self, araim, kit, library, recovery, sbas_msg};
use kshana_pro::license::{self, Tier};
use kshana_pro::scorecards::levels::Verdict;
use kshana_pro::testkit::{packs, run, score, spec::Spec};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-avionics-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const SEED: [u8; 32] = [11u8; 32];
const UTC: &str = "2026-11-20T09:00:00Z";

/// Demonstration limits (not values from any document).
fn params() -> ParamsFile {
    ParamsFile::parse(
        r#"
format = "kshana-avionics-params/1"
supplied_by = "test (demonstration values)"
[values]
alert_time_s = 10
return_time_s = 30
fast_timeout_s = 30
message_loss_s = 10
hal_m = 40
val_m = 35
[sources]
alert_time_s = "demonstration value"
"#,
    )
    .unwrap()
}

fn sbas_procedures() -> Vec<Procedure> {
    library::PROCEDURES
        .iter()
        .map(|(id, _)| library::get(id).unwrap())
        .filter(|p| p.kind == procedure::Kind::SbasSequence)
        .collect()
}

#[test]
fn every_built_in_procedure_parses_and_round_trips() {
    assert!(library::PROCEDURES.len() >= 10);
    for (id, src) in library::PROCEDURES {
        let p = Procedure::parse(src).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(&p.id, id);
        assert!(
            !p.modelled_on.is_empty(),
            "{id} names what it is modelled on"
        );
        let back = Procedure::parse(&p.to_toml()).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.sha256(), p.sha256());
    }
}

/// The wording rule: no claim words in the procedures, the library source or its docs.
#[test]
fn wording_rule_holds_in_procedures_sources_and_docs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let words = [
        concat!("compl", "iant"),
        concat!("compl", "iance"),
        concat!("con", "form"),
        concat!("certi", "fied"),
        concat!("quali", "fied"),
    ];
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("src/avionics"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.extend(
        std::fs::read_dir(root.join("avionics/procedures"))
            .unwrap()
            .map(|e| e.unwrap().path()),
    );
    files.push(root.join("src/cli/avionics_cmd.rs"));
    files.push(root.join("docs/AVIONICS-TEST-LIBRARY.md"));
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap().to_lowercase();
        for w in words {
            assert!(!text.contains(w), "{}: contains '{w}'", f.display());
        }
    }
    // A procedure that uses one of the words is refused.
    let src = library::find("sbas-mt0-do-not-use").unwrap().replace(
        "takes the equipment out of SBAS-based modes",
        concat!("is com", "pliant"),
    );
    assert!(Procedure::parse(&src).is_err());
}

#[test]
fn sbas_streams_decode_and_carry_the_events() {
    for p in sbas_procedures() {
        let s = sbas_seq::generate(p.sequence.as_ref().unwrap()).unwrap();
        assert_eq!(
            s.frames.len() as u32,
            p.sequence.as_ref().unwrap().duration_s
        );
        let mut seq_prev: Option<u64> = None;
        for f in &s.frames {
            if let Some(h) = &f.hex {
                let fr = sbas_msg::Frame::from_hex(h).unwrap();
                assert!(fr.parity_ok());
                assert_eq!(sbas_msg::decode(&fr).unwrap().mt(), f.mt.unwrap());
                // The preamble rotates frame by frame.
                assert_eq!(
                    fr.preamble(),
                    sbas_msg::PREAMBLES[(f.seq.unwrap() % 3) as usize]
                );
                if let Some(q) = seq_prev {
                    assert_eq!(f.seq.unwrap(), q + 1);
                }
                seq_prev = f.seq;
            }
        }
        // Every scored expectation names a declared parameter or none.
        for x in &s.expectations {
            if !x.limit_param.is_empty() {
                assert!(p.parameters.iter().any(|q| q.name == x.limit_param));
            }
        }
    }
    // MT0 procedure: MT0 every second of the event; silence: no frames.
    let mt0 = library::get("sbas-mt0-do-not-use").unwrap();
    let s = sbas_seq::generate(mt0.sequence.as_ref().unwrap()).unwrap();
    assert!(s.frames[120..180].iter().all(|f| f.mt == Some(0)));
    assert!(s.frames[..120].iter().all(|f| f.mt != Some(0)));
    let loss = library::get("sbas-message-loss").unwrap();
    let s = sbas_seq::generate(loss.sequence.as_ref().unwrap()).unwrap();
    assert!(s.frames[120..165].iter().all(|f| f.hex.is_none()));
    assert_eq!(s.expectations[0].kind, ExpectKind::ModeAtMost);
    assert_eq!(s.expectations[0].t_ref_s, 119.0);
}

#[test]
fn fast_correction_timeout_counts_from_the_last_block_message() {
    let p = library::get("sbas-fast-correction-timeout").unwrap();
    let seq = p.sequence.as_ref().unwrap();
    let s = sbas_seq::generate(seq).unwrap();
    let last_mt2 = s.frames[..90]
        .iter()
        .rev()
        .find(|f| f.mt == Some(2))
        .unwrap()
        .t_s as f64;
    let x = &s.expectations[0];
    assert_eq!(x.kind, ExpectKind::Exclude);
    assert_eq!(x.t_ref_s, last_mt2);
    assert_eq!(x.prns, seq.prns); // nine satellites, all in MT2
    assert!(!s.frames[90..210].iter().any(|f| f.mt == Some(2)));
    assert_eq!(seq.events[0].kind, EventKind::WithholdFast);
}

#[test]
fn scoring_separates_good_bad_and_missing_limits() {
    let pf = params();
    for p in sbas_procedures() {
        let params = p.resolve_params(Some(&pf));
        let good = procedure::synth_log(&p, true).unwrap();
        let v = procedure::score(&p, &params, Some(&good), None).unwrap();
        assert_eq!(v.verdict, "pass", "{}: {:#?}", p.id, v.checks);
        let bad = procedure::synth_log(&p, false).unwrap();
        let v = procedure::score(&p, &params, Some(&bad), None).unwrap();
        if p.id == "sbas-fast-correction-step" {
            assert_eq!(
                v.verdict, "pass",
                "a unit that never reacts keeps the satellite"
            );
        } else {
            assert_eq!(v.verdict, "fail", "{}", p.id);
        }
        // Without the applicant's limits nothing that needs one is met.
        let none = p.resolve_params(None);
        let v = procedure::score(&p, &none, Some(&good), None).unwrap();
        for c in &v.checks {
            if !c.limit_param.is_empty() {
                assert_eq!(c.verdict, Verdict::NotAssessed, "{}: {}", p.id, c.id);
            }
        }
        if p.parameters.is_empty() {
            assert_eq!(v.verdict, "pass");
        } else {
            assert_eq!(v.verdict, "incomplete", "{}", p.id);
        }
        // No log: incomplete.
        let v = procedure::score(&p, &params, None, None).unwrap();
        assert_eq!(v.verdict, "incomplete");
    }
}

#[test]
fn a_late_exclusion_is_not_met_and_a_short_log_is_not_assessed() {
    let p = library::get("sbas-udrei-do-not-use").unwrap();
    let params = p.resolve_params(Some(&params()));
    let mut log = procedure::synth_log(&p, true).unwrap();
    // Keep PRN 6 in use for 20 s after the alarm (limit 10 s).
    for e in log.epochs.iter_mut() {
        if (90.0..111.0).contains(&e.t_s) {
            e.used_prns.as_mut().unwrap().push(6);
        }
    }
    let v = procedure::score(&p, &params, Some(&log), None).unwrap();
    let c = v.checks.iter().find(|c| c.id == "udrei15-exclude").unwrap();
    assert_eq!(c.verdict, Verdict::NotMet);
    assert_eq!(c.measured, Some(21.0));
    // A log that ends 5 s after the alarm cannot show the exclusion either way.
    let mut short = procedure::synth_log(&p, false).unwrap();
    short.epochs.retain(|e| e.t_s <= 95.0);
    let v = procedure::score(&p, &params, Some(&short), None).unwrap();
    let c = v.checks.iter().find(|c| c.id == "udrei15-exclude").unwrap();
    assert_eq!(c.verdict, Verdict::NotAssessed);
    // A log for another procedure is refused; bad modes are refused.
    let mut other = log.clone();
    other.procedure = "x".into();
    assert!(procedure::score(&p, &params, Some(&other), None).is_err());
    let mut j: serde_json::Value = serde_json::from_str(&log.to_json()).unwrap();
    j["epochs"][3]["mode"] = "lpv-1".into();
    assert!(DutLog::parse(&j.to_string()).is_err());
}

#[test]
fn araim_published_vectors_are_reproduced() {
    let checks = araim::published_vector_checks().unwrap();
    let add: Vec<_> = checks
        .iter()
        .filter(|c| c.id.starts_with("add-v3.1-appendix-d"))
        .collect();
    assert_eq!(add.len(), 4);
    assert!(add.iter().all(|c| c.verdict == Verdict::Met), "{add:#?}");
    let ms3_pl: Vec<_> = checks
        .iter()
        .filter(|c| c.id.starts_with("milestone3") && !c.id.ends_with("sigma-v-acc"))
        .collect();
    assert!(ms3_pl.iter().all(|c| c.verdict == Verdict::NotAssessed));
    let p = library::get("araim-published-vectors").unwrap();
    let v = procedure::score(&p, &p.resolve_params(None), None, None).unwrap();
    assert_eq!(v.not_met, 0, "{:#?}", v.checks);
}

#[test]
fn araim_cases_institutional_and_leo() {
    let nominal = library::get("araim-h-dual-constellation").unwrap();
    let r = araim::run_cases(nominal.araim.as_ref().unwrap()).unwrap();
    let base = &r[0];
    assert!(base.vpl_m > 0.0 && base.hpl_m > 0.0 && base.hpl_m.is_finite());
    assert_eq!(base.satellites, 14);
    let inst = library::get("araim-institutional-unusable").unwrap();
    let ri = &araim::run_cases(inst.araim.as_ref().unwrap()).unwrap()[0];
    assert_eq!(ri.declared_unusable, vec!["galileo".to_string()]);
    assert_eq!(ri.satellites, 7);
    assert!(ri.vpl_m > base.vpl_m, "{} vs {}", ri.vpl_m, base.vpl_m);
    let leo = library::get("araim-leo-augmentation").unwrap();
    let rl = &araim::run_cases(leo.araim.as_ref().unwrap()).unwrap()[0];
    let c = &rl.comparisons[0];
    assert_eq!(c.without, "leo");
    // Without LEO the geometry is exactly the dual-constellation case.
    assert!((c.vpl_m.unwrap() - base.vpl_m).abs() < 1e-6);
    assert!(
        c.delta_vpl_m.unwrap() < 0.0,
        "LEO lowers the VPL here: {c:?}"
    );

    // Equipment outputs: a close one agrees, a far one does not; the institutional case
    // checks the exclusion.
    let pf = params();
    for p in [&nominal, &inst, &leo] {
        let params = p.resolve_params(Some(&pf));
        assert_eq!(params.value("tol_pl_m"), Some(0.05));
        let good = procedure::synth_log(p, true).unwrap();
        let v = procedure::score(p, &params, Some(&good), None).unwrap();
        assert_eq!(v.verdict, "pass", "{}: {:#?}", p.id, v.checks);
        let bad = procedure::synth_log(p, false).unwrap();
        let v = procedure::score(p, &params, Some(&bad), None).unwrap();
        assert_eq!(v.verdict, "fail", "{}", p.id);
    }
    let params = inst.resolve_params(Some(&pf));
    let bad = procedure::synth_log(&inst, false).unwrap();
    let v = procedure::score(&inst, &params, Some(&bad), None).unwrap();
    let c = v
        .checks
        .iter()
        .find(|c| c.id.ends_with("institutional-exclusion"))
        .unwrap();
    assert_eq!(c.verdict, Verdict::NotMet);
    // Reference availability rows are reported, not scored.
    assert!(v
        .checks
        .iter()
        .any(|c| c.kind == "reference" && c.detail.contains("vertical")));
}

fn verdict_for(id: &str, receiver: &str, good: bool) -> score::ScenarioVerdict {
    let spec = Spec::parse(packs::find(id).unwrap().1)
        .unwrap()
        .resolve()
        .unwrap();
    let log = run::synth_log(&spec, good);
    score::score(&spec, &log, "b1", receiver).unwrap()
}

#[test]
fn recovery_scorecard_per_type() {
    let fleet = recovery::Fleet::parse(
        r#"
format = "kshana-avionics-fleet/1"
operator = "Example operator"
[[type]]
id = "type-a"
receivers = ["unit-a1", "unit-a2"]
aircraft = 30
[[type]]
id = "type-b"
receivers = ["unit-b1"]
aircraft = 10
"#,
    )
    .unwrap();
    let vs = vec![
        (
            "a1".to_string(),
            verdict_for("aviation-post-spoof-recovery", "unit-a1", true),
        ),
        (
            "a2".to_string(),
            verdict_for("aviation-post-spoof-reset", "unit-a2", true),
        ),
        (
            "b1".to_string(),
            verdict_for("aviation-post-spoof-recovery", "unit-b1", false),
        ),
        (
            "x".to_string(),
            verdict_for("aviation-post-spoof-recovery", "unit-x", true),
        ),
    ];
    let sc = recovery::scorecard(&fleet, &vs, Some(60.0), "operator target (test)").unwrap();
    assert_eq!(sc.unassigned_receivers, vec!["unit-x".to_string()]);
    let a = sc.types.iter().find(|t| t.id == "type-a").unwrap();
    let b = sc.types.iter().find(|t| t.id == "type-b").unwrap();
    assert!(a.recovered >= 2, "{a:?}");
    assert_eq!(a.within_target, Some(1.0));
    assert_eq!(a.target_verdict, Verdict::Met);
    assert!(
        b.median_s.unwrap_or(f64::INFINITY) > 60.0 || b.not_recovered > 0,
        "{b:?}"
    );
    assert_eq!(b.target_verdict, Verdict::NotMet);
    // Worst first.
    assert_eq!(sc.types[0].id, "type-b");
    let w = sc.fleet_weighted_within_target.unwrap();
    assert!((w - 0.75).abs() < 1e-9, "{w}");
    assert!(recovery::markdown(&sc).contains("| type-a | 30 |"));
    // No target: shares not assessed.
    let sc = recovery::scorecard(&fleet, &vs, None, "").unwrap();
    assert!(sc
        .types
        .iter()
        .all(|t| t.target_verdict == Verdict::NotAssessed));
    // A receiver in two types is refused.
    assert!(recovery::Fleet::parse(
        "format = \"kshana-avionics-fleet/1\"\n[[type]]\nid=\"a\"\nreceivers=[\"r\"]\n[[type]]\nid=\"b\"\nreceivers=[\"r\"]\n"
    )
    .is_err());
}

#[test]
fn tool_kit_verification_cases_all_pass() {
    let cases = kit::verification_cases();
    assert!(cases.len() >= 20, "{}", cases.len());
    let failed: Vec<_> = cases.iter().filter(|c| c.result != "pass").collect();
    assert!(failed.is_empty(), "{failed:#?}");
    let cfg = kit::config_index();
    assert_eq!(cfg.procedures.len(), library::PROCEDURES.len());
    assert!(!cfg.open_engine_version.is_empty());
    let d = tmp("kit");
    let r = kit::write_kit(&d).unwrap();
    assert!(kit::all_passed(&r));
    for f in [
        "VERIFICATION-RESULTS.json",
        "CONFIGURATION-INDEX.json",
        "TOOL-OPERATIONAL-REQUIREMENTS.md",
        "APPLICANT-ACTIONS.md",
        "KNOWN-LIMITATIONS.md",
    ] {
        assert!(d.join(f).is_file(), "{f}");
    }
}

#[test]
fn licence_features_and_tiers() {
    assert_eq!(license::explicit_tier(avionics::FEATURE), Some(Tier::Pro));
    assert_eq!(
        license::explicit_tier(avionics::FEATURE_RECOVERY),
        Some(Tier::Pro)
    );
    assert_eq!(
        license::explicit_tier(avionics::FEATURE_KIT),
        Some(Tier::Enterprise)
    );
    for f in [
        avionics::FEATURE,
        avionics::FEATURE_RECOVERY,
        avionics::FEATURE_KIT,
    ] {
        assert!(license::PRO_COMMAND_FEATURES.contains(&f));
    }
}

fn bin(args: &[&str], home: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn cli_end_to_end() {
    let d = tmp("cli");
    let home = d.join("home");
    let p = |x: &Path| x.to_str().unwrap().to_string();
    let (ok, out) = bin(&["avionics", "list"], &home);
    assert!(
        ok && out.contains("sbas-udrei-do-not-use") && out.contains("araim-leo-augmentation"),
        "{out}"
    );
    let (ok, out) = bin(
        &["avionics", "validate", "sbas-fast-correction-timeout"],
        &home,
    );
    assert!(
        ok && out.contains("fast_timeout_s") && out.contains("applicant supplies"),
        "{out}"
    );
    let (ok, out) = bin(&["avionics", "show", "araim-h-dual-constellation"], &home);
    assert!(ok && out.contains("public_value = 0.05"), "{out}");
    // Gated without a licence.
    let (ok, out) = bin(
        &[
            "avionics",
            "generate",
            "sbas-mt0-do-not-use",
            "--out",
            &p(&d.join("g")),
        ],
        &home,
    );
    assert!(!ok && out.contains("requires the 'pro' tier"), "{out}");
    let (ok, out) = bin(
        &[
            "avionics",
            "generate",
            "sbas-mt0-do-not-use",
            "--out",
            &p(&d.join("g")),
            "--dev",
        ],
        &home,
    );
    assert!(ok && d.join("g/frames.csv").is_file(), "{out}");
    // Sample, params, run (good passes, bad exits 1), verify.
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={}\n", hex::encode(SEED))).unwrap();
    let pfile = d.join("params.toml");
    std::fs::write(&pfile, toml::to_string(&params()).unwrap()).unwrap();
    for (b, want_ok) in [("good", true), ("bad", false)] {
        let log = d.join(format!("{b}.json"));
        let (ok, out) = bin(
            &[
                "avionics",
                "sample",
                "sbas-udrei-do-not-use",
                "--behaviour",
                b,
                "--out",
                &p(&log),
            ],
            &home,
        );
        assert!(ok, "{out}");
        let (ok, out) = bin(
            &[
                "avionics",
                "run",
                "sbas-udrei-do-not-use",
                "--params",
                &p(&pfile),
                "--log",
                &p(&log),
                "--out",
                &p(&d.join(format!("runs/{b}"))),
                "--signer",
                "Lab",
                "--key-file",
                &p(&key),
                "--utc",
                UTC,
                "--dev",
            ],
            &home,
        );
        assert_eq!(ok, want_ok, "{out}");
    }
    let pk = hex::encode(license::public_key_for(&SEED));
    let (ok, out) = bin(
        &[
            "avionics",
            "verify",
            &p(&d.join("runs/good")),
            "--trust",
            &pk,
        ],
        &home,
    );
    assert!(ok && out.contains("trusted"), "{out}");
    // Never overwrites a signed directory.
    let (ok, out) = bin(
        &[
            "avionics",
            "run",
            "araim-published-vectors",
            "--out",
            &p(&d.join("runs/good")),
            "--signer",
            "Lab",
            "--key-file",
            &p(&key),
            "--dev",
        ],
        &home,
    );
    assert!(!ok && out.contains("never overwritten"), "{out}");
    // Evidence pack over the two runs.
    let trace = d.join("trace.toml");
    std::fs::write(
        &trace,
        "format = \"kshana-avionics-trace/1\"\nproject = \"demo\"\n[[requirement]]\nid = \"REQ-1\"\ntext = \"Drop a satellite set to do not use\"\nprocedures = [\"sbas-udrei-do-not-use\"]\n[[requirement]]\nid = \"REQ-2\"\nprocedures = [\"sbas-message-loss\"]\n",
    )
    .unwrap();
    let (ok, out) = bin(
        &[
            "avionics",
            "evidence",
            "--trace",
            &p(&trace),
            "--runs",
            &p(&d.join("runs")),
            "--out",
            &p(&d.join("evidence")),
            "--signer",
            "Lab",
            "--key-file",
            &p(&key),
            "--trust",
            &pk,
            "--dev",
        ],
        &home,
    );
    assert!(
        ok && out.contains("REQ-1") && out.contains("REQ-2"),
        "{out}"
    );
    let csv = std::fs::read_to_string(d.join("evidence/TRACEABILITY.csv")).unwrap();
    assert!(csv.contains("REQ-1,sbas-udrei-do-not-use,fail"), "{csv}");
    assert!(csv.contains("REQ-2,sbas-message-loss,no run"), "{csv}");
    // Kit (Enterprise via --dev).
    let (ok, out) = bin(
        &[
            "avionics",
            "kit",
            "--out",
            &p(&d.join("kit")),
            "--signer",
            "Lab",
            "--key-file",
            &p(&key),
            "--utc",
            UTC,
            "--dev",
        ],
        &home,
    );
    assert!(ok && out.contains("0 failed"), "{out}");
    let (ok, out) = bin(
        &["avionics", "verify", &p(&d.join("kit")), "--trust", &pk],
        &home,
    );
    assert!(ok, "{out}");
}
