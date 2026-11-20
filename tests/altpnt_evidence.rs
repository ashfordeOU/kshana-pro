// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Alternative-PNT evaluation and civil test-campaign evidence (`kshana_pro::altpnt`,
//! docs/ALTPNT-EVIDENCE.md): the failure-mode pack's findings and its verification of cited
//! signed evidence, the twin-versus-flight labels, the campaign scores and claim checks with
//! the civil-releasability screen, the resilient-time interface tests, signing and checking
//! with the Pro client, and the licence gates. All inputs are synthetic with known answers.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::altpnt::{self, campaign, failure_modes, labels, synth, time_iface};
use kshana_pro::license::{self, Entitlement, Tier};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");
const SEED: [u8; 32] = [7u8; 32];
const UTC: &str = "2026-11-20T12:00:00Z";

fn fixture(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-altpnt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    synth::write_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (bool, String) {
    let o = Command::new(BIN).args(args).output().unwrap();
    (
        o.status.success(),
        String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr),
    )
}

fn produce(dir: &Path, cmd: &str, req: &str, extra: &[&str]) -> (bool, String, PathBuf) {
    let key = dir.join("key.hex");
    std::fs::write(&key, hex::encode(SEED)).unwrap();
    let out = dir.join(format!("out-{cmd}-{req}"));
    let mut a: Vec<String> = [
        "altpnt",
        cmd,
        dir.join(req).to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "test",
        "--key-file",
        key.to_str().unwrap(),
        "--utc",
        UTC,
        "--dev",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();
    a.extend(extra.iter().map(|x| x.to_string()));
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let (ok, text) = run(&a);
    (ok, text, out)
}

fn lab_key() -> String {
    hex::encode(license::public_key_for(&synth::EVIDENCE_SEED))
}

fn no_forbidden_words(text: &str) {
    for w in altpnt::FORBIDDEN_WORDS {
        assert!(!text.to_lowercase().contains(w), "'{w}' in:\n{text}");
    }
}

#[test]
fn the_failure_mode_pack_finds_common_causes_and_labels_evidence() {
    let d = fixture("fm");
    let (req, _) = failure_modes::load(&d.join("architecture.toml")).unwrap();
    let key = license::parse_key_hex(&lab_key()).unwrap();
    let p = failure_modes::analyse(&req, &d, &[key]).unwrap();
    assert_eq!(p.single_points_of_failure, vec!["pwr-a"]);
    let rule = |r: &str| {
        p.findings
            .iter()
            .filter(|f| f.rule == r)
            .map(|f| f.subject.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(rule("shared-resource-undeclared"), vec!["pwr-a"]);
    assert_eq!(rule("source-undocumented"), vec!["clock", "inertial"]);
    assert_eq!(rule("common-cause-fallback"), vec!["FM-ANT"]);
    assert_eq!(rule("no-detection"), vec!["FM-ELORAN"]);
    assert_eq!(rule("untested"), vec!["FM-ELORAN"]);
    assert!(rule("no-independent-fallback").is_empty());
    let m = |id: &str| p.modes.iter().find(|m| m.id == id).unwrap();
    assert_eq!(m("FM-GNSS-RFI").label, "MEASURED-OPERATIONAL");
    assert_eq!(m("FM-GNSS-RFI").status, "TESTED");
    assert_eq!(m("FM-ANT").affected_sources, vec!["gnss", "leo"]);
    assert_eq!(m("FM-ANT").fallback_affected, vec!["leo"]);
    assert_eq!(m("FM-ANT").status, "MODELLED-ONLY");
    assert_eq!(m("FM-LEO").tests[0].evidence_check, "verified");
    assert_eq!(m("FM-LEO").label, "MEASURED-LAB");
    assert_eq!(p.coverage["modes"], 4);
    assert_eq!(p.coverage["with_detection"], 3);

    // Without a pinned key the evidence counts but says so; with a wrong key it does not.
    let p = failure_modes::analyse(&req, &d, &[]).unwrap();
    let leo = p.modes.iter().find(|m| m.id == "FM-LEO").unwrap();
    assert_eq!(leo.tests[0].evidence_check, "signature-ok-untrusted");
    let wrong = license::public_key_for(&[1u8; 32]);
    let p = failure_modes::analyse(&req, &d, &[wrong]).unwrap();
    let leo = p.modes.iter().find(|m| m.id == "FM-LEO").unwrap();
    assert!(leo.tests[0].evidence_check.starts_with("failed"));
    assert_eq!(leo.status, "UNTESTED");
    // Tampered evidence is not counted.
    std::fs::write(d.join("evidence/fm-leo-hil/result.txt"), "edited\n").unwrap();
    let p = failure_modes::analyse(&req, &d, &[key]).unwrap();
    assert!(p
        .findings
        .iter()
        .any(|f| f.rule == "evidence-not-verified" && f.subject == "T-05"));
    let md = failure_modes::markdown(&p);
    assert!(md.contains("| FM-ANT") && md.contains("architectural target"));
    no_forbidden_words(&md);
}

#[test]
fn claims_are_labelled_twin_against_flight() {
    let d = fixture("labels");
    let (req, _) = labels::load(&d.join("labels.toml")).unwrap();
    let l = labels::label(&req).unwrap();
    let c = |id: &str| l.claims.iter().find(|c| c.id == id).unwrap();
    assert_eq!(c("C1").label, "MEASURED-OPERATIONAL");
    assert_eq!(c("C1").met, Some(false));
    assert_eq!(c("C1").deciding_value, Some(70.0));
    assert_eq!(
        c("C1").model_gap.as_ref().unwrap().reading,
        "model-optimistic"
    );
    assert_eq!(c("C2").label, "VALIDATED");
    assert_eq!(c("C2").met, Some(true));
    assert_eq!(c("C3").label, "MODELLED");
    assert!(c("C3").statement.contains("architectural target"));
    assert_eq!(c("C4").label, "CLAIMED");
    assert_eq!(l.counts.values().sum::<usize>(), 4);
    no_forbidden_words(&labels::markdown(&l));
}

#[test]
fn the_campaign_scores_outputs_against_truth_and_checks_claims() {
    let d = fixture("campaign");
    let l = campaign::load(&d.join("campaign.toml")).unwrap();
    let c = campaign::score(&l.req, &l.texts).unwrap();
    assert!(c.screen.passed);
    let r1 = &c.runs[0].scores;
    assert_eq!(r1.availability, Some(1.0));
    assert!((r1.h50_m.unwrap() - 4.0).abs() < 0.01, "{r1:?}");
    assert!((r1.h95_m.unwrap() - 6.0).abs() < 0.01);
    assert!((r1.v95_m.unwrap() - 3.0).abs() < 0.01);
    assert_eq!(r1.time95_ns, Some(12.0));
    assert_eq!(r1.hmi_epochs, 0);
    let r2 = &c.runs[1].scores;
    assert_eq!(r2.availability, Some(0.9));
    assert_eq!(r2.misleading_epochs, 50);
    assert_eq!(r2.hmi_epochs, 30);
    assert_eq!(r2.alert_epochs, 20);
    assert_eq!(r2.epochs_with_protection_level, 90);
    let denied = c
        .conditions
        .iter()
        .find(|k| k.condition == "gnss-denied")
        .unwrap();
    assert_eq!(denied.runs_pooled, vec!["R2"]);
    assert_eq!(denied.runs_not_pooled, vec!["R3"]);
    assert_eq!(denied.label, "MEASURED-OPERATIONAL");
    assert!((denied.scores.h95_m.unwrap() - 48.5).abs() < 0.01);
    let (lo, hi) = denied.scores.hmi_rate_ci95.unwrap();
    assert!(lo < 1.0 / 3.0 && hi > 1.0 / 3.0);
    let k = |id: &str| c.claims.iter().find(|k| k.id == id).unwrap();
    assert_eq!(k("K1").verdict, "NOT-MET");
    assert_eq!(k("K2").verdict, "MET");
    assert_eq!(k("K2").met_at_interval_bound, Some(false));
    assert_eq!(k("K3").verdict, "NOT-MET");
    assert_eq!(k("K4").verdict, "NOT-TESTED");
    assert_eq!(k("K4").label, "CLAIMED");
    no_forbidden_words(&campaign::markdown(&c));

    let (ok, text, out) = produce(&d, "campaign", "campaign.toml", &[]);
    assert!(ok, "{text}");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("campaign.json")).unwrap()).unwrap();
    assert_eq!(v["format"], "kshana-pro-altpnt-campaign/1");
    assert_eq!(v["evidence"], "MODELLED");
    assert_eq!(out.join("inputs").read_dir().unwrap().count(), 5);
}

#[test]
fn a_military_signal_or_weapons_end_use_is_refused() {
    let d = fixture("refused");
    let e = campaign::load(&d.join("campaign-refused.toml"))
        .err()
        .unwrap();
    assert!(
        e.contains("civil-releasability") && e.contains("PRS"),
        "{e}"
    );
    let (ok, text, out) = produce(&d, "campaign", "campaign-refused.toml", &[]);
    assert!(!ok && text.contains("civil-releasability"), "{text}");
    assert!(!out.exists());
    let t = synth::campaign(false).replace("civil maritime", "guided munition");
    let req: campaign::Request = toml::from_str(&t).unwrap();
    assert!(campaign::screen(&req).unwrap_err().contains("munition"));
    let t = synth::campaign(false).replace("\"unclassified\"", "\"restricted\"");
    let req: campaign::Request = toml::from_str(&t).unwrap();
    assert!(campaign::screen(&req).is_err());
}

#[test]
fn the_resilient_time_interface_tests_find_the_overstated_holdover() {
    let d = fixture("time");
    let l = time_iface::load(&d.join("time-iface.toml")).unwrap();
    let (log, bad) = time_iface::parse_log(&l.log).unwrap();
    assert_eq!((log.len(), bad), (3595, 0));
    let r = time_iface::test(&l.req, &log, bad).unwrap();
    let t = |id: &str| r.tests.iter().find(|t| t.id == id).unwrap();
    assert_eq!(t("locked-accuracy").verdict, "WITHIN-LIMIT");
    assert_eq!(t("locked-accuracy").epochs_tested, 2335);
    assert_eq!(t("declared-quality").verdict, "EXCEEDS-LIMIT");
    assert_eq!(t("declared-quality").exceedances, 1039);
    assert_eq!(t("holdover-growth").verdict, "WITHIN-LIMIT");
    assert_eq!(t("holdover-growth").epochs_tested, 1200);
    assert!((t("holdover-growth").worst.unwrap() - 619.5 / 699.5).abs() < 1e-3);
    assert_eq!(t("recovery").verdict, "WITHIN-LIMIT");
    assert_eq!(t("recovery").worst, Some(60.0));
    assert_eq!(t("continuity").verdict, "EXCEEDS-LIMIT");
    assert_eq!(t("continuity").worst, Some(6.0));
    assert_eq!(t("mtie-locked").verdict, "WITHIN-LIMIT");
    assert_eq!(t("mtie-locked").epochs_tested, 1200);
    no_forbidden_words(&time_iface::markdown(&r));
}

#[test]
fn results_are_signed_and_checked_with_the_pro_client() {
    let d = fixture("sign");
    let lab = lab_key();
    let (ok, text, out) = produce(
        &d,
        "failure-modes",
        "architecture.toml",
        &["--trust", lab.as_str()],
    );
    assert!(ok, "{text}");
    assert!(text.contains("single points of failure: pwr-a"), "{text}");
    assert!(out.join("failure-modes.md").exists());
    for (cmd, req) in [("labels", "labels.toml"), ("time-iface", "time-iface.toml")] {
        let (ok, text, _) = produce(&d, cmd, req, &[]);
        assert!(ok, "{cmd}: {text}");
    }
    let pk = hex::encode(license::public_key_for(&SEED));
    let (ok, text) = run(&["altpnt", "check", out.to_str().unwrap(), "--trust", &pk]);
    assert!(ok, "{text}");
    let f = out.join("failure-modes.json");
    let t = std::fs::read_to_string(&f).unwrap();
    std::fs::write(&f, t.replace("MODELLED-ONLY", "TESTED")).unwrap();
    let (ok, _) = run(&["dossier", "verify", out.to_str().unwrap(), "--trust", &pk]);
    assert!(!ok);
}

#[test]
fn features_are_tiered_and_gated() {
    for f in [altpnt::FEATURE_EVIDENCE, altpnt::FEATURE_CAMPAIGN] {
        assert_eq!(license::explicit_tier(f), Some(Tier::Pro));
        assert!(license::PRO_COMMAND_FEATURES.contains(&f));
    }
    let free = Entitlement {
        licensee: "ACME".into(),
        tier: Tier::Evaluation,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&free, altpnt::FEATURE_CAMPAIGN, 0).is_err());
    let d = fixture("gate");
    let (ok, text) = run(&["altpnt", "synth", d.join("s").to_str().unwrap()]);
    assert!(ok, "{text}");
    let (ok, _) = run(&["altpnt"]);
    assert!(!ok);
}
