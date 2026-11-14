// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! MBSE tooling over real open-engine outputs.
//!
//! Every run under `tests/fixtures/mbse/runs/` was produced by the open Kshana engine
//! (see `tests/fixtures/mbse/PROVENANCE.md`). These tests evaluate the worked-example
//! requirements against those real result documents and check each command: import
//! (CSV, ReqIF, SysML v2), trace, the verification cross-reference matrix (VCRM), the
//! SysML v2 export and its grammar-subset check, change impact and the evidence pack,
//! plus the licence gate of the binary.

// Arksa-exclusive (docs/PROTECTION.md): runs only in a build with the `arksa` feature.
#![cfg(feature = "arksa")]

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::mbse::criterion::{self, Verdict};
use kshana_pro::mbse::evidence::{sha256_hex, RunUnits};
use kshana_pro::mbse::trace::{import_requirements, vcrm, Trace};
use kshana_pro::mbse::{impact, pack, reqif, requirements, sysml};

fn fx(p: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mbse")
        .join(p)
}

fn trace() -> Trace {
    Trace::load(&fx("trace.toml")).expect("the worked-example trace loads")
}

fn row<'a>(v: &'a kshana_pro::mbse::trace::Vcrm, id: &str) -> &'a kshana_pro::mbse::trace::Row {
    v.rows
        .iter()
        .find(|r| r.requirement == id)
        .expect("row exists")
}

fn json(p: &str) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(fx(p)).unwrap()).unwrap()
}

fn tmp(tag: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("mbse-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Copy the whole fixture tree so a test can alter a file without touching the repo.
fn copy_fixtures(tag: &str) -> PathBuf {
    fn cp(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let t = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                cp(&e.path(), &t);
            } else {
                std::fs::copy(e.path(), t).unwrap();
            }
        }
    }
    let d = tmp(tag);
    cp(&fx(""), &d);
    d
}

// ------------------------------------------------------------------ provenance

#[test]
fn fixtures_are_the_files_the_engine_wrote() {
    // Each result file is exactly the one its report describes, and each scenario file
    // is exactly the one the run used.
    for n in [
        "clock-holdover",
        "clock-holdover-retuned",
        "integrity-raim",
        "campaign-jam-spoof-holdover-integrity",
        "leo-pnt-end-to-end",
    ] {
        let report = json(&format!("runs/{n}/{n}.report.json"));
        let r = &report["reproducibility"];
        let result = std::fs::read(fx(&format!("runs/{n}/{n}.result.json"))).unwrap();
        let scen = std::fs::read(fx(&format!("runs/{n}/{n}.toml"))).unwrap();
        assert_eq!(
            r["result_sha256"].as_str().unwrap(),
            sha256_hex(&result),
            "{n}"
        );
        assert_eq!(
            r["scenario_sha256"].as_str().unwrap(),
            sha256_hex(&scen),
            "{n}"
        );
        assert_eq!(r["engine_version"], "0.28.0", "{n}");
    }
    assert_eq!(
        sha256_hex(&std::fs::read(fx("engine/field-units-schema.json")).unwrap()),
        "8aa887e6dd7518407ebc982917de7705dceec58b45eefd701e0d96399068a5b7"
    );
    assert_eq!(
        sha256_hex(&std::fs::read(fx("engine/verification-matrix.json")).unwrap()),
        "c17a81fa180eeed2c55067440e47b14441d935897c251ccea47a4580ec40a8ce"
    );
}

#[test]
fn run_labels_match_the_matrix() {
    let t = trace();
    let m = t.matrix.as_ref().unwrap();
    for run in &t.runs {
        assert!(
            !run.capability_rows.is_empty(),
            "{} lists capabilities",
            run.id
        );
        for row in &run.capability_rows {
            assert_eq!(
                m.label(&row.requirement),
                Some(row.label.as_str()),
                "{}: {}",
                run.id,
                row.requirement
            );
        }
    }
    let v = vcrm(&t);
    assert!(!v.open_items.iter().any(|o| o.category == "label drift"));
}

// ------------------------------------------------------------------ import

#[test]
fn csv_import_reads_the_worked_example() {
    let reqs = import_requirements(&fx("requirements.csv")).unwrap();
    assert_eq!(reqs.len(), 15);
    let ops = reqs.iter().find(|r| r.id == "OPS-001").unwrap();
    assert_eq!(ops.method, requirements::Method::Inspection);
    assert!(ops.criterion.is_none());
    let c5 = reqs.iter().find(|r| r.id == "CLK-005").unwrap();
    assert_eq!(c5.capabilities.len(), 2);
    // Every criterion parses.
    for r in &reqs {
        if let Some(c) = &r.criterion {
            criterion::parse(c).unwrap_or_else(|e| panic!("{}: {e}", r.id));
        }
    }
}

#[test]
fn reqif_round_trip_and_golden_file() {
    let reqs = import_requirements(&fx("requirements.csv")).unwrap();
    let text = reqif::to_reqif("Kshana worked example", &reqs, None);
    assert_eq!(
        reqif::from_reqif(&text).unwrap(),
        reqs,
        "CSV → ReqIF → model is lossless"
    );
    // The committed ReqIF is this export; importing it gives the same model.
    let golden = fx("requirements.reqif");
    if std::env::var("KSHANA_PRO_UPDATE_GOLDEN").is_ok() {
        std::fs::write(&golden, &text).unwrap();
    }
    assert_eq!(std::fs::read_to_string(&golden).unwrap(), text);
    assert_eq!(import_requirements(&golden).unwrap(), reqs);
    // CSV → model → CSV is lossless too.
    assert_eq!(
        requirements::from_csv(&requirements::to_csv(&reqs)).unwrap(),
        reqs
    );
}

#[test]
fn sysml_requirements_only_round_trip() {
    let mut reqs = import_requirements(&fx("requirements.csv")).unwrap();
    let text = sysml::export_requirements("Kshana worked example", &reqs);
    sysml::check(&text).expect("requirements-only export is in the subset");
    let (back, links) = sysml::requirements_from_sysml(&text).unwrap();
    assert!(
        links.is_empty(),
        "a requirements-only model has no verify links"
    );
    // Run links are carried by verify relationships, which a requirements-only model
    // does not have; everything else survives.
    for r in &mut reqs {
        r.runs.clear();
    }
    assert_eq!(back, reqs);
}

// ------------------------------------------------------------------ criteria on real runs

#[test]
fn vcrm_verdicts_on_the_real_runs() {
    let t = trace();
    let v = vcrm(&t);
    let clock = json("runs/clock-holdover/clock-holdover.result.json");
    let raim = json("runs/integrity-raim/integrity-raim.result.json");

    // A scalar with a unit, read straight from the result document.
    let c1 = row(&v, "CLK-001");
    assert_eq!(c1.verdict, Verdict::Met);
    let obs = &c1.cells[0].evidence.observations[0];
    assert_eq!(obs.path, "classical.fom.timing_p95_ns");
    assert_eq!(obs.value, clock["classical"]["fom"]["timing_p95_ns"]);
    assert_eq!(obs.unit.as_deref(), Some("ns"));
    assert!(obs
        .unit_source
        .as_deref()
        .unwrap()
        .contains("kinds.clock.fields.\"classical.fom.timing_p95_ns\""));
    // Evidence carries the reproducibility stamp.
    let ev = &c1.cells[0].evidence;
    assert_eq!(ev.scenario_hash.as_deref(), clock["scenario_hash"].as_str());
    assert_eq!(ev.seed, Some(42));
    assert_eq!(ev.engine_version.as_deref(), Some("0.28.0"));
    assert_eq!(ev.schema_version.as_deref(), Some("0.7"));

    // 30 min converted to the field's seconds.
    let c2 = row(&v, "CLK-002");
    assert_eq!(c2.verdict, Verdict::Met);
    assert!(c2.cells[0].evidence.observations[0]
        .detail
        .contains("2610 s >= 1800 s"));
    // Aggregate over an array, and the trailing-aggregate spelling.
    assert_eq!(row(&v, "CLK-003").verdict, Verdict::Met);
    let c4 = row(&v, "CLK-004");
    assert_eq!(c4.verdict, Verdict::Met);
    let max_err = clock["classical"]["series"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["error_ns"].as_f64().unwrap())
        .fold(f64::MIN, f64::max);
    assert_eq!(
        c4.cells[0].evidence.observations[0].value,
        serde_json::json!(max_err)
    );
    // Indexed element; boolean.
    assert_eq!(row(&v, "CLK-005").verdict, Verdict::Met);
    assert_eq!(row(&v, "CLK-006").verdict, Verdict::Met);

    // `for all epochs`: a real not-met finding, with the first failing epoch named.
    let i1 = row(&v, "INT-001");
    assert_eq!(i1.verdict, Verdict::NotMet);
    let n_ok = raim["epochs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["hpl_m"].as_f64().unwrap() < raim["al_h_m"].as_f64().unwrap())
        .count();
    assert!(i1.cells[0].evidence.observations[0]
        .detail
        .contains(&format!("{n_ok} of 361 elements satisfy")));
    // fraction(...) and an element-wise abs() with a quantifier.
    assert_eq!(row(&v, "INT-002").verdict, Verdict::Met);
    assert_eq!(row(&v, "INT-003").verdict, Verdict::Met);
    // Campaign results: events and channels.
    assert_eq!(row(&v, "CMP-001").verdict, Verdict::Met);
    assert_eq!(row(&v, "CMP-002").verdict, Verdict::NotMet);
    // LEO: cm converted to m; an indexed case in minutes.
    assert_eq!(row(&v, "LEO-001").verdict, Verdict::Met);
    let l2 = row(&v, "LEO-002");
    assert!(l2.cells[0].evidence.observations[0]
        .detail
        .contains("<= 0.3 m"));
    assert_eq!(row(&v, "LEO-003").verdict, Verdict::Met);
    // No run, no criterion: not verified, with open items.
    let o1 = row(&v, "OPS-001");
    assert_eq!(o1.verdict, Verdict::NotVerified);
    assert!(v.open_items.iter().any(
        |o| o.requirement.as_deref() == Some("OPS-001") && o.category == "manual verification"
    ));

    assert_eq!(v.summary.requirements, 15);
    assert_eq!(v.summary.met, 12);
    assert_eq!(v.summary.not_met, 2);
    assert_eq!(v.summary.not_verified, 1);
}

#[test]
fn a_modelled_basis_is_flagged_and_a_validated_one_is_not() {
    let v = vcrm(&trace());
    // The clock timing figures are MODELLED in the open matrix (figure_tiers say so).
    let c1 = &row(&v, "CLK-001").cells[0];
    assert_eq!(c1.basis.label, "MODELLED");
    assert!(c1.basis.modelled_basis);
    assert!(c1
        .basis
        .contributions
        .iter()
        .any(|b| b.source.contains("figure_tiers")
            && b.capability.as_deref() == Some("GNSS-denied clock holdover")));
    assert!(v
        .open_items
        .iter()
        .any(|o| o.requirement.as_deref() == Some("CLK-001") && o.category == "MODELLED basis"));
    // RAIM rests on VALIDATED rows only.
    let i2 = &row(&v, "INT-002").cells[0];
    assert_eq!(i2.basis.label, "VALIDATED");
    assert!(!i2.basis.modelled_basis);
    // Pinned capabilities decide the basis when the requirement names them.
    assert_eq!(row(&v, "CLK-005").cells[0].basis.label, "VALIDATED");
    // The campaign relies on a partner-owned capability: flagged PARTNER.
    let cm = &row(&v, "CMP-001").cells[0];
    assert_eq!(cm.basis.label, "PARTNER");
    assert!(cm.basis.modelled_basis);
    // The campaign's MODELLED run-level label is read.
    assert!(cm
        .basis
        .contributions
        .iter()
        .any(|b| b.source.contains("`label`")));
    // A field whose evidence tier the field-units schema states as modelled.
    let l1 = &row(&v, "LEO-001").cells[0];
    assert_eq!(l1.basis.label, "MODELLED");
    assert!(l1.basis.contributions[0]
        .source
        .contains("evidence tier `modelled` of `fusion.fused.rms_error_3d_m`"));
    // Met: CLK-001..006 (CLK-005 VALIDATED), INT-002, INT-003 (VALIDATED), CMP-001
    // (PARTNER), LEO-001..003 (MODELLED): 12 met, 9 of them on a non-VALIDATED basis.
    assert_eq!(v.summary.met_on_modelled_basis, 9);
}

#[test]
fn unit_mismatch_and_unitless_literals_are_not_verified_on_real_data() {
    let t = trace();
    let run = t.run("RUN-CLOCK").unwrap();
    let units = RunUnits {
        run,
        schema: t.schema.as_ref(),
    };
    let check_on_run =
        |src: &str| criterion::evaluate(&criterion::parse(src).unwrap(), &run.result, &units);
    let e = check_on_run("classical.fom.timing_p95_ns <= 20 m");
    assert_eq!(e.verdict, Verdict::NotVerified);
    assert!(e.reason.as_deref().unwrap().contains("unit mismatch"));
    let e = check_on_run("classical.fom.timing_p95_ns <= 20");
    assert_eq!(e.verdict, Verdict::NotVerified);
    assert!(e.reason.as_deref().unwrap().contains("has no unit"));
    // Field against field of different dimensions.
    let e = check_on_run("classical.fom.holdover_s > classical.fom.availability");
    assert!(e.reason.as_deref().unwrap().contains("unit mismatch"));
    // Same dimension, different scales: 20 ns == 0.02 us.
    assert_eq!(
        check_on_run("classical.fom.timing_p95_ns <= 0.02 us").verdict,
        Verdict::Met
    );
    // An array compared without an aggregate or quantifier.
    assert_eq!(
        check_on_run("classical.series.error_ns <= 50 ns").verdict,
        Verdict::NotVerified
    );
    // A field that is not there.
    assert!(check_on_run("classical.fom.nonexistent_ns <= 1 ns")
        .reason
        .unwrap()
        .contains("not in the result document"));
    // Without the schema, the result's own units block still gives the unit.
    let own = RunUnits { run, schema: None };
    let e = criterion::evaluate(
        &criterion::parse("classical.fom.timing_p95_ns <= 20 ns").unwrap(),
        &run.result,
        &own,
    );
    assert_eq!(e.verdict, Verdict::Met);
    assert!(e.observations[0]
        .unit_source
        .as_deref()
        .unwrap()
        .starts_with("result units block"));
}

#[test]
fn altered_evidence_is_not_verified() {
    let d = copy_fixtures("tamper");
    // Change the scenario after the run: the report's scenario SHA-256 no longer matches.
    let scen = d.join("runs/integrity-raim/integrity-raim.toml");
    let s = std::fs::read_to_string(&scen)
        .unwrap()
        .replace("al_h_m = 40.0", "al_h_m = 400.0");
    std::fs::write(&scen, s).unwrap();
    // Edit a result file: its SHA-256 no longer matches the report.
    let res = d.join("runs/leo-pnt-end-to-end/leo-pnt-end-to-end.result.json");
    let r = std::fs::read_to_string(&res).unwrap().replacen(
        "\"rms_error_3d_m\"",
        "\"rms_error_3d_m\" ",
        1,
    );
    std::fs::write(&res, r).unwrap();
    let t = Trace::load(&d.join("trace.toml")).unwrap();
    let v = vcrm(&t);
    for id in ["INT-001", "INT-002", "INT-003"] {
        let c = &row(&v, id).cells[0];
        assert_eq!(c.verdict, Verdict::NotVerified, "{id}");
        assert!(c
            .reason
            .as_deref()
            .unwrap()
            .contains("scenario file changed"));
    }
    let l = &row(&v, "LEO-001").cells[0];
    assert_eq!(l.verdict, Verdict::NotVerified);
    assert!(l
        .reason
        .as_deref()
        .unwrap()
        .contains("result file is not the one"));
    assert!(v
        .open_items
        .iter()
        .any(|o| o.category == "evidence integrity"));
    // The clock run is untouched and still verifies.
    assert_eq!(row(&v, "CLK-001").verdict, Verdict::Met);
}

#[test]
fn a_scenario_file_from_another_run_is_not_verified() {
    // A trace may name a scenario without a report, and then no hash ties the two. The
    // scenario's own kind and seed must still agree with what the run stamped, or the
    // SysML parts and change impact would describe a system the run never simulated.
    let d = copy_fixtures("wrong-scenario");
    std::fs::write(
        d.join("reqs.csv"),
        "id,text,method,criterion,runs\n\
         W-1,CSAC p95.,analysis,classical.fom.timing_p95_ns <= 20 ns,RUN-A\n\
         W-2,SISRE.,analysis,navmsg.sisre_rms_m <= 30 cm,RUN-B\n\
         W-3,CSAC p95.,analysis,classical.fom.timing_p95_ns <= 20 ns,RUN-C\n",
    )
    .unwrap();
    let t = |a_scen: &str, b_scen: &str| {
        format!(
            "title = \"t\"\nrequirements = \"reqs.csv\"\n\
             field_units_schema = \"engine/field-units-schema.json\"\n\
             [[run]]\nid = \"RUN-A\"\n\
             result = \"runs/clock-holdover/clock-holdover.result.json\"\n\
             scenario = \"{a_scen}\"\n\
             [[run]]\nid = \"RUN-B\"\n\
             result = \"runs/leo-pnt-end-to-end/leo-pnt-end-to-end.result.json\"\n\
             scenario = \"{b_scen}\"\n\
             [[run]]\nid = \"RUN-C\"\n\
             result = \"runs/clock-holdover/clock-holdover.result.json\"\n\
             scenario = \"runs/clock-holdover-retuned/clock-holdover-retuned.toml\"\n"
        )
    };
    // Right scenarios: both verify (no report, so nothing else is checked). RUN-C shows
    // the limit of the check: a retuned clock scenario has the same kind and seed.
    std::fs::write(
        d.join("good.toml"),
        t(
            "runs/clock-holdover/clock-holdover.toml",
            "runs/leo-pnt-end-to-end/leo-pnt-end-to-end.toml",
        ),
    )
    .unwrap();
    let v = vcrm(&Trace::load(&d.join("good.toml")).unwrap());
    for id in ["W-1", "W-2", "W-3"] {
        assert_eq!(row(&v, id).verdict, Verdict::Met, "{id}");
    }
    // Swapped: the clock result (seed 42, no kind) with the RAIM scenario (seed 7), and
    // the LEO result (kind leo-pnt-chain) with the clock scenario (default kind clock).
    std::fs::write(
        d.join("bad.toml"),
        t(
            "runs/integrity-raim/integrity-raim.toml",
            "runs/clock-holdover/clock-holdover.toml",
        ),
    )
    .unwrap();
    let v = vcrm(&Trace::load(&d.join("bad.toml")).unwrap());
    let a = &row(&v, "W-1").cells[0];
    assert_eq!(a.verdict, Verdict::NotVerified);
    assert!(
        a.reason.as_deref().unwrap().contains("sets seed 7"),
        "{:?}",
        a.reason
    );
    let b = &row(&v, "W-2").cells[0];
    assert_eq!(b.verdict, Verdict::NotVerified);
    assert!(
        b.reason
            .as_deref()
            .unwrap()
            .contains("of kind 'clock', the run is of kind 'leo-pnt-chain'"),
        "{:?}",
        b.reason
    );
}

#[test]
fn trace_links_carry_the_stamps() {
    let v = vcrm(&trace());
    let links = v.links_json();
    let arr = links["links"].as_array().unwrap();
    assert_eq!(arr.len(), 15);
    let cmp = arr.iter().find(|l| l["requirement"] == "CMP-001").unwrap();
    assert_eq!(cmp["runs"][0]["run"], "RUN-CAMPAIGN");
    assert_eq!(cmp["runs"][0]["seed"], 20260928);
    assert_eq!(
        cmp["runs"][0]["scenario_hash"],
        json("runs/campaign-jam-spoof-holdover-integrity/campaign-jam-spoof-holdover-integrity.result.json")["scenario_hash"]
    );
    // The integrity result carries no scenario_hash: the stamp says so rather than
    // inventing one, and the report's scenario SHA-256 identifies the input.
    let t = trace();
    let raim = t.run("RUN-RAIM").unwrap();
    assert!(raim.stamp.scenario_hash.is_none());
    assert!(raim.stamp.scenario_hash_source.contains("not stamped"));
    assert_eq!(raim.stamp.seed, Some(7));
    assert_eq!(raim.stamp.seed_source, "report.json `reproducibility.seed`");
}

#[test]
fn vcrm_formats() {
    let v = vcrm(&trace());
    let md = v.to_markdown();
    assert!(md.contains("| INT-001 | analysis |"));
    assert!(md.contains("MODELLED (flagged)"));
    let csv = v.to_csv();
    assert_eq!(csv.lines().count(), 1 + v.summary.cells);
    let back: serde_json::Value = serde_json::from_str(&v.to_json()).unwrap();
    assert_eq!(back["summary"]["met"], 12);
}

// ------------------------------------------------------------------ SysML

#[test]
fn sysml_export_is_golden_valid_and_round_trips() {
    let t = trace();
    let v = vcrm(&t);
    let text = sysml::export(&t, &v);
    let golden = fx("model.golden.sysml");
    if std::env::var("KSHANA_PRO_UPDATE_GOLDEN").is_ok() {
        std::fs::write(&golden, &text).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&golden).unwrap(),
        text,
        "SysML export drifted from the golden file (KSHANA_PRO_UPDATE_GOLDEN=1 rewrites it)"
    );
    // Valid in the checked subset, including name resolution.
    sysml::check(&text).expect("the export is in the SysML v2 subset");
    // The systems the scenarios simulated, with units from the field-units schema.
    assert!(text.contains("part def Clock {"));
    assert!(text.contains("part def Receiver {"));
    assert!(text.contains("part def Constellation {"));
    assert!(text.contains("part def Jammer {"));
    assert!(text.contains("kinds.clock.fields.\"classical.spec.params.q_wf\""));
    assert!(text.contains("attribute :>> q_wf = 9.0E-20;"));
    // satisfy only where met; verify for every link.
    assert!(text.contains("satisfy Requirements::'CLK-001' by Scenarios::'RUN-CLOCK';"));
    assert!(!text.contains("satisfy Requirements::'INT-001'"));
    assert!(text.contains("No satisfy relationship: INT-001 is not met by RUN-RAIM"));
    // Round trip: the model's requirement defs and verify links give back the trace.
    let (reqs, links) = sysml::requirements_from_sysml(&text).unwrap();
    assert_eq!(
        reqs, t.requirements,
        "requirements survive SysML export and import"
    );
    let mut want = t.links.clone();
    want.retain(|_, v| !v.is_empty());
    assert_eq!(links, want, "verify relationships carry the trace links");
}

#[test]
fn sysml_export_never_writes_a_nan_scenario_value_as_a_number() {
    // TOML allows `nan` and `inf`; SysML v2 has no literal for them. The export must
    // leave such a key out rather than write a number the scenario does not hold.
    let d = copy_fixtures("sysml-nan");
    let sc = d.join("runs/clock-holdover/clock-holdover.toml");
    let src = std::fs::read_to_string(&sc).unwrap();
    assert!(src.contains("q_rw = 0.0"));
    std::fs::write(&sc, src.replacen("q_rw = 0.0", "q_rw = nan", 1)).unwrap();
    let t = Trace::load(&d.join("trace.toml")).unwrap();
    let text = sysml::export(&t, &vcrm(&t));
    sysml::check(&text).expect("still in the subset");
    let clock = text
        .split("part 'RUN-CLOCK' {")
        .nth(1)
        .and_then(|s| s.split("part 'RUN-RAIM' {").next())
        .expect("the clock run's parts");
    // Two clocks in the scenario; only the untouched one keeps its q_rw.
    assert_eq!(
        clock.matches("attribute :>> q_rw = 0.0;").count(),
        1,
        "{clock}"
    );
}

#[test]
fn sysml_checker_rejects_a_broken_export() {
    let t = trace();
    let text = sysml::export(&t, &vcrm(&t));
    for (from, to) in [
        (
            "satisfy Requirements::'CLK-001'",
            "satisfy Requirements::'CLK-999'",
        ),
        ("attribute :>> seed = 42;", "attribute :>> seed = 42"),
        ("part def Clock {", "part def part {"),
        ("ISQ::LengthValue", "ISQ::LenghtValue"),
    ] {
        assert!(text.contains(from), "{from}");
        assert!(
            sysml::check(&text.replacen(from, to, 1)).is_err(),
            "{to} must be rejected"
        );
    }
}

// ------------------------------------------------------------------ impact

#[test]
fn impact_of_a_changed_scenario_across_traces() {
    let old = trace();
    let new = Trace::load(&fx("trace-retuned.toml")).unwrap();
    let im = impact::compare(&old, &new);
    let flagged: Vec<&str> = im.reverify.iter().map(|r| r.requirement.as_str()).collect();
    assert_eq!(
        flagged,
        vec!["CLK-001", "CLK-002", "CLK-003", "CLK-004", "CLK-005", "CLK-006"]
    );
    let c1 = &im.reverify[0];
    assert!(c1
        .reasons
        .iter()
        .any(|r| r.contains("scenario_hash changed")));
    // The re-specified clock fails the timing requirement it used to meet.
    assert_eq!(c1.old_verdict.as_deref(), Some("met"));
    assert_eq!(c1.new_verdict.as_deref(), Some("not met"));
    for id in ["INT-001", "CMP-001", "LEO-001", "OPS-001"] {
        assert!(im.unaffected.iter().any(|u| u == id), "{id}");
    }
    assert!(im.to_markdown().contains("Requirements to re-verify (6)"));
}

#[test]
fn impact_of_a_capability_label_change_between_engine_versions() {
    // Same runs, labels read through the v0.23.0 matrix (old) and the current one
    // (new): "IEEE-1139 power-law clock noise + flicker-FM floor" became VALIDATED.
    let old = Trace::load(&fx("trace-matrix-v0.23.0.toml")).unwrap();
    let new = trace();
    let im = impact::compare(&old, &new);
    assert_eq!(im.reverify.len(), 1, "{:#?}", im.reverify);
    assert_eq!(im.reverify[0].requirement, "CLK-005");
    assert!(im.reverify[0].reasons[0].contains("MODELLED → VALIDATED"));
    // And the basis moved with it.
    assert_eq!(row(&vcrm(&old), "CLK-005").cells[0].basis.label, "MODELLED");
    assert_eq!(
        row(&vcrm(&new), "CLK-005").cells[0].basis.label,
        "VALIDATED"
    );
}

#[test]
fn impact_catches_a_changed_result_when_no_stamp_identifies_the_run() {
    // The integrity kind stamps no scenario_hash, and a trace may give only the result
    // file (no report, no scenario). Then no stamp tells the two runs apart, and only the
    // result file's own SHA-256 shows that the evidence changed.
    let d = copy_fixtures("impact-result-only");
    let raim = "runs/integrity-raim/integrity-raim.result.json";
    let mut v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.join(raim)).unwrap()).unwrap();
    v["al_h_m"] = serde_json::json!(8.0);
    std::fs::write(d.join("changed.result.json"), v.to_string()).unwrap();
    std::fs::write(
        d.join("reqs.csv"),
        "id,text,method,criterion,runs\n\
         INT-X,HPL below HAL.,analysis,epochs.hpl_m < al_h_m for all epochs,RUN-RAIM\n",
    )
    .unwrap();
    let t = |result: &str| {
        format!(
            "title = \"t\"\nrequirements = \"reqs.csv\"\n\
             field_units_schema = \"engine/field-units-schema.json\"\n\
             [[run]]\nid = \"RUN-RAIM\"\nresult = \"{result}\"\n"
        )
    };
    std::fs::write(d.join("a.toml"), t(raim)).unwrap();
    std::fs::write(d.join("b.toml"), t("changed.result.json")).unwrap();
    std::fs::write(d.join("c.toml"), t(raim)).unwrap();
    let a = Trace::load(&d.join("a.toml")).unwrap();
    let im = impact::compare(&a, &Trace::load(&d.join("b.toml")).unwrap());
    assert_eq!(im.reverify.len(), 1, "{}", im.to_markdown());
    assert!(
        im.reverify[0]
            .reasons
            .iter()
            .any(|r| r.contains("result file changed")),
        "{:?}",
        im.reverify[0].reasons
    );
    // The same file on both sides is not a change.
    let same = impact::compare(&a, &Trace::load(&d.join("c.toml")).unwrap());
    assert!(same.reverify.is_empty(), "{}", same.to_markdown());
}

#[test]
fn impact_of_two_scenario_files() {
    let t = trace();
    let im = impact::compare_scenarios(
        &t,
        &fx("runs/clock-holdover/clock-holdover.toml"),
        &fx("runs/clock-holdover-retuned/clock-holdover-retuned.toml"),
    )
    .unwrap();
    assert_eq!(
        im.changed_scenario_keys.len(),
        1,
        "{:?}",
        im.changed_scenario_keys
    );
    assert!(im.changed_scenario_keys[0].starts_with("clock_classical.q_wf"));
    assert_eq!(im.reverify.len(), 6);
    // A comment-only edit changes nothing that matters.
    let d = tmp("comment");
    let src = std::fs::read_to_string(fx("runs/clock-holdover/clock-holdover.toml")).unwrap();
    let edited = d.join("clock-holdover.toml");
    std::fs::write(&edited, format!("# reviewed\n{src}")).unwrap();
    let im = impact::compare_scenarios(&t, &fx("runs/clock-holdover/clock-holdover.toml"), &edited)
        .unwrap();
    assert!(im.changed_scenario_keys.is_empty());
    assert!(im.reverify.is_empty());
}

// ------------------------------------------------------------------ pack

#[test]
fn evidence_pack_is_deterministic_self_contained_and_complete() {
    let t = trace();
    let a = pack::build(&t, None);
    let b = pack::build(&t, Some("2026-09-29T12:00:00Z".into()));
    assert_eq!(
        a.pack_sha256, b.pack_sha256,
        "the issue stamp is outside the hash"
    );
    assert_eq!(a.to_html(), pack::build(&t, None).to_html());
    let html = b.to_html();
    // Offline: no scripts, stylesheets or images fetched from anywhere.
    for bad in [
        "<script",
        "<link",
        "src=\"http",
        "href=\"http",
        "url(http",
        "@import",
    ] {
        assert!(!html.contains(bad), "pack HTML contains {bad}");
    }
    for section in [
        "Verification cross-reference matrix",
        "Traceability graph",
        "Reproducibility record",
        "Open items",
        "Sign-off",
        "2026-09-29T12:00:00Z",
    ] {
        assert!(html.contains(section), "{section}");
    }
    // Every run's stamp and result hash is in the reproducibility record.
    for run in &t.runs {
        if let Some(h) = &run.stamp.scenario_hash {
            assert!(html.contains(h.as_str()));
        }
        assert!(html.contains(&run.result_file.sha256));
    }
    let j: serde_json::Value = serde_json::from_str(&a.to_json()).unwrap();
    assert_eq!(j["body"]["graph_edges"].as_array().unwrap().len(), 14);
    assert_eq!(j["body"]["sign_off"].as_array().unwrap().len(), 3);
    assert_eq!(j["pack_sha256"].as_str().unwrap().len(), 64);
}

// ------------------------------------------------------------------ binary

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
}

fn ok(args: &[&str]) -> (String, String) {
    let out = bin().args(args).output().expect("binary runs");
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn every_command_runs_end_to_end() {
    let tr = fx("trace.toml");
    let trs = tr.to_str().unwrap();
    let d = tmp("cli");
    let csv_path = fx("requirements.csv");
    let (out, _) = ok(&["mbse", "import", csv_path.to_str().unwrap(), "--dev"]);
    let parsed: Vec<requirements::Requirement> = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed.len(), 15);
    let reqif_path = fx("requirements.reqif");
    let (out, _) = ok(&[
        "mbse",
        "import",
        reqif_path.to_str().unwrap(),
        "--to",
        "csv",
        "--dev",
    ]);
    assert_eq!(
        requirements::from_csv(&out).unwrap(),
        import_requirements(&csv_path).unwrap()
    );
    let (out, _) = ok(&["mbse", "trace", trs, "--dev"]);
    assert!(out.contains("\"RUN-CAMPAIGN\""));
    let (out, err) = ok(&["mbse", "vcrm", trs, "--format", "md", "--dev"]);
    assert!(out.contains("| CLK-001 |"));
    assert!(err.contains("12 met"));
    let failing = bin()
        .args(["mbse", "vcrm", trs, "--fail-on-open", "--dev"])
        .output()
        .unwrap();
    assert_eq!(
        failing.status.code(),
        Some(1),
        "open requirements fail the gate"
    );
    let model = d.join("model.sysml");
    ok(&[
        "mbse",
        "sysml",
        trs,
        "--out",
        model.to_str().unwrap(),
        "--dev",
    ]);
    assert_eq!(
        std::fs::read_to_string(&model).unwrap(),
        std::fs::read_to_string(fx("model.golden.sysml")).unwrap()
    );
    let (out, _) = ok(&["mbse", "sysml", "--check", model.to_str().unwrap(), "--dev"]);
    assert!(out.contains(
        "valid in the supported SysML v2 subset (15 requirement definitions, 14 verify links)"
    ));
    let retuned = fx("trace-retuned.toml");
    let (out, _) = ok(&[
        "mbse",
        "impact",
        "--old",
        trs,
        "--new",
        retuned.to_str().unwrap(),
        "--format",
        "json",
        "--dev",
    ]);
    let im: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(im["reverify"].as_array().unwrap().len(), 6);
    let so = fx("runs/clock-holdover/clock-holdover.toml");
    let sn = fx("runs/clock-holdover-retuned/clock-holdover-retuned.toml");
    let (out, _) = ok(&[
        "mbse",
        "impact",
        trs,
        "--scenario-old",
        so.to_str().unwrap(),
        "--scenario-new",
        sn.to_str().unwrap(),
        "--dev",
    ]);
    assert!(out.contains("clock_classical.q_wf"));
    let pd = d.join("pack");
    let (_, err) = ok(&[
        "mbse",
        "pack",
        trs,
        "--out",
        pd.to_str().unwrap(),
        "--utc",
        "2026-09-29T00:00:00Z",
        "--dev",
    ]);
    assert!(err.contains("pack SHA-256"));
    assert!(pd.join("evidence-pack.html").exists());
    let j: serde_json::Value =
        serde_json::from_slice(&std::fs::read(pd.join("evidence-pack.json")).unwrap()).unwrap();
    assert_eq!(j["issued_utc"], "2026-09-29T00:00:00Z");
}

#[test]
fn the_mbse_commands_are_licence_gated() {
    let tr = fx("trace.toml");
    let trs = tr.to_str().unwrap();
    // No licence: Evaluation tier, refused.
    let denied = bin().args(["mbse", "vcrm", trs]).output().unwrap();
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr)
        .contains(kshana_pro::license::tiers::NOT_IN_THIS_EDITION));
    // No standalone token carries the feature (roadmap 0.42 X1), not even Enterprise; an
    // Arksa-entitled one does. Tokens are signed with the debug demo key and the placeholder
    // Arksa issuer test key, which only debug builds accept.
    let issue = |tier: &str| {
        let out = bin()
            .args([
                "license",
                "issue",
                "--tier",
                tier,
                "--licensee",
                "mbse-test",
                "--days",
                "30",
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .find(|l| !l.trim().is_empty() && !l.contains(' '))
            .expect("a token line")
            .trim()
            .to_string()
    };
    let pro = issue("pro");
    let refused = bin()
        .args(["mbse", "vcrm", trs, "--license", &pro])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let enterprise = issue("enterprise");
    let refused = bin()
        .args(["mbse", "vcrm", trs, "--license", &enterprise])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    // The placeholder Arksa issuer test key (tests/arksa_entitlement.rs).
    let ent = kshana_pro::license::issue_arksa(
        b"arksa-issuer-test-key-not-real!!",
        &kshana_pro::license::LicenseClaims {
            licensee: "mbse-test".into(),
            tier: "pro".into(),
            products: vec!["cli".into()],
            ..Default::default()
        },
    )
    .unwrap();
    let allowed = bin()
        .args(["mbse", "vcrm", trs, "--license", &ent])
        .output()
        .unwrap();
    assert!(
        allowed.status.success(),
        "{}",
        String::from_utf8_lossy(&allowed.stderr)
    );
    // A tampered token is refused.
    let tampered = format!("{}x", &ent[..ent.len() - 1]);
    let bad = bin()
        .args(["mbse", "vcrm", trs, "--license", &tampered])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}
