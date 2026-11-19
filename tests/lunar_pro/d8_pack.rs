// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// D8: lunar mission-review evidence pack (part of the tests/lunar_pro.rs test binary).

use std::path::{Path, PathBuf};

use kshana_pro::lunar_pro::pack::*;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-d8-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const SEED: [u8; 32] = [3u8; 32];

fn signed_pack(d: &Path, spec_edit: impl Fn(String) -> String) -> (PathBuf, Pack) {
    let spec = write_example(&d.join("in")).unwrap();
    let text = spec_edit(std::fs::read_to_string(&spec).unwrap());
    std::fs::write(&spec, text).unwrap();
    let (s, base) = read_spec(&spec).unwrap();
    let out = d.join("pack");
    let p = write_pack(&out, &s, &base).unwrap();
    kshana_pro::signed_dossier::sign(&out, &SEED, "Kshana Pro test", "2027-02-01T00:00:00Z")
        .unwrap();
    (out, p)
}

fn pk() -> [u8; 32] {
    kshana_pro::license::public_key_for(&SEED)
}

#[test]
fn d8_engine_version_matches_the_cargo_pin() {
    let c = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let pin = c
        .lines()
        .find(|l| l.trim_start().starts_with("kshana = "))
        .unwrap();
    let v = pin.split('"').nth(1).unwrap().trim_start_matches('=');
    assert!(
        ENGINE.contains(&format!("kshana {v} ")),
        "{ENGINE} vs {pin}"
    );
}

#[test]
fn d8_complete_pack_discloses_time_ephemerides_and_verdicts_and_verifies() {
    let d = tmp("ok");
    let (out, p) = signed_pack(&d, |s| s);
    assert!(p.complete, "{:#?}", p.checklist);
    // The time disclosure is D1's: TL − UTC at the reference epoch and its budget.
    let m = kshana_pro::lunar_pro::time::Model::default_model();
    let jd = kshana_pro::lunar_pro::time::parse_iso("2027-03-01T00:00:00").unwrap();
    let b = m.lunar_minus_utc(kshana_pro::lunar_pro::time::LunarOption::Tl, jd);
    assert_eq!(p.time.offset_s, b.value());
    assert_eq!(p.time.u_c_s, b.u_c());
    assert_eq!(p.time.scale, "TL");
    assert!(p.time.cgpm_option.contains("W0_L"));
    assert!(
        p.time.model_terms.len() == b.terms.len()
            && p.time.model_terms.iter().any(|t| t.contains("L_L"))
    );
    assert!((p.time.rate_vs_tt_us_day - 56.03).abs() < 0.02);
    // The ephemeris hash is computed from the file.
    let e = &p.ephemerides[0];
    assert_eq!(e.hash_status, "computed");
    let raw = std::fs::read(d.join("in/ephemeris-standin.bsp")).unwrap();
    assert_eq!(
        e.sha256.as_deref().unwrap(),
        kshana_pro::lunar_pro::lugre::sha256_hex(&raw)
    );
    // P1 62 ± 18 vs ≤ 100: Meets; P2 40 ± 24 vs ≤ 50: Inconclusive.
    let v: Vec<&str> = p.conformance.iter().map(|c| c.verdict.as_str()).collect();
    assert_eq!(v, ["Meets", "Inconclusive"]);
    // The pack verifies, with the key pinned.
    let r = verify_pack(&out, &[pk()]).unwrap();
    assert!(r.passed() && r.complete, "{:?}", r.problems);
    // Not pinned: not passed.
    assert!(!verify_pack(&out, &[]).unwrap().passed());
    assert!(!verify_pack(&out, &[[1u8; 32]]).unwrap().passed());
    // The files are all there; the budget names the label.
    for f in [
        "pack.json",
        "pack.txt",
        "time_budget.json",
        "signature.json",
    ] {
        assert!(out.join(f).exists(), "{f}");
    }
    let tb = std::fs::read_to_string(out.join("time_budget.json")).unwrap();
    assert!(tb.contains("MODELLED") && tb.contains("conversion"));
}

#[test]
fn d8_tampering_is_detected() {
    let d = tmp("tamper");
    let (out, _) = signed_pack(&d, |s| s);
    // A changed verdict in the text.
    let t = out.join("pack.txt");
    let orig = std::fs::read_to_string(&t).unwrap();
    std::fs::write(&t, orig.replace("Inconclusive", "Meets       ")).unwrap();
    let r = verify_pack(&out, &[pk()]).unwrap();
    assert!(
        !r.passed() && r.problems.iter().any(|p| p.contains("pack.txt")),
        "{:?}",
        r.problems
    );
    std::fs::write(&t, &orig).unwrap();
    assert!(verify_pack(&out, &[pk()]).unwrap().passed());
    // A changed time offset in pack.json, re-signed by an attacker with another key: the
    // signature then holds but is not trusted, and the recomputation catches the number.
    let j = out.join("pack.json");
    let pj = std::fs::read_to_string(&j).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&pj).unwrap();
    let off = v["time"]["offset_s"].as_f64().unwrap();
    v["time"]["offset_s"] = serde_json::json!(off + 1e-6);
    std::fs::write(&j, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    kshana_pro::signed_dossier::sign(&out, &[8u8; 32], "attacker", "2027-02-02T00:00:00Z").unwrap();
    let r = verify_pack(&out, &[pk()]).unwrap();
    assert_eq!(r.trusted, Some(false));
    assert!(
        r.problems.iter().any(|p| p.starts_with("time:")),
        "{:?}",
        r.problems
    );
    // A checklist item flipped and re-signed: caught as not following from the content.
    let mut v: serde_json::Value = serde_json::from_str(&pj).unwrap();
    v["checklist"][4]["provided"] = serde_json::json!(false);
    std::fs::write(&j, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    kshana_pro::signed_dossier::sign(&out, &SEED, "Kshana Pro test", "2027-02-01T00:00:00Z")
        .unwrap();
    let r = verify_pack(&out, &[pk()]).unwrap();
    assert!(
        r.problems.iter().any(|p| p.contains("checklist")),
        "{:?}",
        r.problems
    );
    // A verdict flipped in pack.json and re-signed with the real key: caught.
    let mut v: serde_json::Value = serde_json::from_str(&pj).unwrap();
    v["conformance"][1]["verdict"] = serde_json::json!("Meets");
    std::fs::write(&j, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    kshana_pro::signed_dossier::sign(&out, &SEED, "Kshana Pro test", "2027-02-01T00:00:00Z")
        .unwrap();
    let r = verify_pack(&out, &[pk()]).unwrap();
    assert!(
        r.problems
            .iter()
            .any(|p| p.contains("verdict does not follow")),
        "{:?}",
        r.problems
    );
    // A deleted file.
    std::fs::write(&j, &pj).unwrap();
    kshana_pro::signed_dossier::sign(&out, &SEED, "Kshana Pro test", "2027-02-01T00:00:00Z")
        .unwrap();
    std::fs::remove_file(out.join("time_budget.json")).unwrap();
    assert!(!verify_pack(&out, &[pk()]).unwrap().passed());
}

#[test]
fn d8_checklist_flags_missing_disclosure() {
    let d = tmp("missing");
    let (_, p) = signed_pack(&d, |s| {
        s.replace("utc_traceability = \"UTC(k) ground laboratory -> Earth-Moon TWTT link -> lander clock (example)\"\n", "")
            .replace("file = \"ephemeris-standin.bsp\"\n", "")
    });
    assert!(!p.complete);
    let missing: Vec<&str> = p
        .checklist
        .iter()
        .filter(|c| !c.provided)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(missing, ["C5", "C7"]);
    assert_eq!(p.ephemerides[0].hash_status, "unverified");
    let t = render_text(&p);
    assert!(t.contains("NO (items missing)") && t.contains("[ ] C5"));
}

#[test]
fn d8_declared_hash_must_match_and_tcl_option_and_attachments() {
    let d = tmp("att");
    // A wrong declared hash is refused.
    let spec = write_example(&d.join("in")).unwrap();
    let text = std::fs::read_to_string(&spec).unwrap();
    std::fs::write(
        &spec,
        text.replace(
            "source = \"JPL NAIF\"",
            &format!("source = \"JPL NAIF\"\nsha256 = \"{}\"", "0".repeat(64)),
        ),
    )
    .unwrap();
    let (s, base) = read_spec(&spec).unwrap();
    assert!(build(&s, &base).unwrap_err().contains("declared"));
    // TCL, with a D7 report attached: its verdicts join the conformance table.
    let csv = kshana_pro::lunar_pro::sla::synthetic_csv(
        2029,
        6,
        900.0,
        0.5,
        &kshana_pro::lunar_pro::sla::example_providers(),
        2,
    );
    let r = kshana_pro::lunar_pro::sla::verify(
        &kshana_pro::lunar_pro::sla::parse_samples(&csv).unwrap(),
        &[kshana_pro::lunar_pro::sla::Sla::builtin("lcrns-srd").unwrap()],
        &kshana_pro::lunar_pro::sla::Options {
            month: "2029-06".into(),
            ..Default::default()
        },
        "x",
    )
    .unwrap();
    kshana_pro::lunar_pro::sla::write_dir(&d.join("in/sla"), &r).unwrap();
    std::fs::write(
        &spec,
        text.replace("option = \"tl\"", "option = \"tcl\"")
            + "\n[[attachment]]\npath = \"sla/report.json\"\nrole = \"LNSP SLA verification\"\n",
    )
    .unwrap();
    let (s, base) = read_spec(&spec).unwrap();
    let out = d.join("pack");
    let p = write_pack(&out, &s, &base).unwrap();
    assert_eq!(p.time.scale, "TCL");
    assert!(p.time.cgpm_option.starts_with("TCL"));
    assert_eq!(p.attachments[0].kind, "kshana-d7-sla-report");
    let d7: Vec<&Conformance> = p
        .conformance
        .iter()
        .filter(|c| c.id.starts_with("D7"))
        .collect();
    assert_eq!(d7.len(), 2);
    assert!(d7
        .iter()
        .any(|c| c.id.contains("BREACH-CO") && c.verdict == "Breach"));
    assert!(d7
        .iter()
        .any(|c| c.id.contains("MEETS-CO") && c.verdict == "Meets"));
    kshana_pro::signed_dossier::sign(&out, &SEED, "t", "2027-02-01T00:00:00Z").unwrap();
    assert!(verify_pack(&out, &[pk()]).unwrap().passed());
    // An attachment changed inside the pack (then re-signed): its hash no longer matches pack.json.
    let a = out.join(&p.attachments[0].file);
    std::fs::write(&a, "{}").unwrap();
    kshana_pro::signed_dossier::sign(&out, &SEED, "t", "2027-02-01T00:00:00Z").unwrap();
    let v = verify_pack(&out, &[pk()]).unwrap();
    assert!(
        v.problems.iter().any(|x| x.contains("SHA-256 differs")),
        "{:?}",
        v.problems
    );
    // A non-empty output directory is refused.
    assert!(write_pack(&out, &s, &base).is_err());
}

#[test]
fn d8_verdict_rule() {
    assert_eq!(verdict(10.0, 1.0, 2.0, "le", 12.0), "Meets");
    assert_eq!(verdict(10.0, 1.0, 2.0, "le", 11.0), "Inconclusive");
    assert_eq!(verdict(10.0, 1.0, 2.0, "le", 7.9), "Breach");
    assert_eq!(verdict(0.99, 0.002, 2.0, "ge", 0.98), "Meets");
    assert_eq!(verdict(0.97, 0.002, 2.0, "ge", 0.98), "Breach");
}
