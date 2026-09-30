// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Mission dossier: one programme of requirements and scenarios to one dossier.
//!
//! The worked programme in `scenarios/pro-power/mission/` runs five scenarios of the
//! open engine (a low Earth orbit navigation chain, a lunar constellation, the L band
//! under interference, a chained jamming, spoofing and holdover campaign, and Mars orbit
//! navigation) plus one Kshana Pro trade study. Every value these tests expect is also
//! read here from a direct `kshana::api::run_scenario` call on the same scenario file,
//! so a verdict in the dossier is checked against the engine, not against a copy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::mbse::evidence::sha256_hex;
use kshana_pro::mbse::trace::{vcrm, Trace};
use kshana_pro::mission::check::{check, differences};
use kshana_pro::mission::compose::{Dossier, MatrixRow};
use kshana_pro::mission::{build, BuildOptions, Built};
use kshana_pro::study_ledger::StudyLedger;
use serde_json::Value;

const UTC: &str = "2026-09-30T00:00:00Z";
const OPEN_RUNS: [(&str, &str, &str); 5] = [
    ("RUN-LEO", "leo-pnt-end-to-end", "leo-pnt-chain"),
    (
        "RUN-LUNAR",
        "lunar-relay-constellation",
        "constellation-design",
    ),
    ("RUN-SPECTRUM", "l-band-waterfall-jamming", "spectrum"),
    (
        "RUN-CAMPAIGN",
        "campaign-jam-spoof-holdover-integrity",
        "campaign",
    ),
    ("RUN-MARS", "mars-orbit-pnt", "body-pnt"),
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/pro-power/mission")
}

fn tmp(tag: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("mission-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn enterprise() -> Entitlement {
    Entitlement {
        licensee: "(test)".into(),
        tier: Tier::Enterprise,
        expires_unix: 0,
        features: vec![],
    }
}

fn opts(ent: Entitlement) -> BuildOptions {
    BuildOptions {
        issued_utc: Some(UTC.into()),
        entitlement: ent,
        now_unix: 1_790_000_000,
    }
}

fn build_worked(tag: &str) -> Built {
    build(
        &fixtures().join("programme.toml"),
        &tmp(tag),
        &opts(enterprise()),
    )
    .expect("the worked programme builds")
}

/// One build of the worked programme that the read-only tests share.
fn shared() -> &'static Built {
    static B: OnceLock<Built> = OnceLock::new();
    B.get_or_init(|| build_worked("shared"))
}

/// The result document of a direct open-engine run of one fixture scenario.
fn direct(stem: &str) -> &'static Value {
    static RUNS: OnceLock<BTreeMap<String, Value>> = OnceLock::new();
    let all = RUNS.get_or_init(|| {
        OPEN_RUNS
            .iter()
            .map(|(_, stem, _)| {
                let src = std::fs::read_to_string(fixtures().join(format!("{stem}.toml")))
                    .expect("fixture scenario is readable");
                let out = kshana::api::run_scenario(&src).expect("the open engine runs it");
                (
                    stem.to_string(),
                    serde_json::from_str(&out.json).expect("result is JSON"),
                )
            })
            .collect()
    });
    &all[stem]
}

fn num(v: &Value, path: &[&str]) -> f64 {
    let mut cur = v;
    for seg in path {
        cur = match seg.parse::<usize>() {
            Ok(i) => &cur[i],
            Err(_) => &cur[*seg],
        };
    }
    cur.as_f64()
        .unwrap_or_else(|| panic!("{path:?} is not a number: {cur}"))
}

fn row<'a>(d: &'a Dossier, id: &str) -> &'a MatrixRow {
    d.body
        .matrix
        .iter()
        .find(|r| r.requirement == id)
        .unwrap_or_else(|| panic!("requirement {id} is in the matrix"))
}

/// The value the verdict of `id` read (the last observation of its one cell).
fn value_read(d: &Dossier, id: &str) -> f64 {
    let r = row(d, id);
    assert_eq!(r.cells.len(), 1, "{id} is linked to one run");
    r.cells[0]
        .values
        .last()
        .and_then(|v| v.value.as_f64())
        .unwrap_or_else(|| panic!("{id} read a number"))
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

/// A one-scenario programme (Mars) with the given requirements file text, for the
/// adversarial cases.
fn small_programme(tag: &str, programme: &str, requirements: &str) -> PathBuf {
    let d = tmp(tag);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::copy(
        fixtures().join("mars-orbit-pnt.toml"),
        d.join("mars-orbit-pnt.toml"),
    )
    .unwrap();
    std::fs::write(d.join("programme.toml"), programme).unwrap();
    std::fs::write(d.join("requirements.csv"), requirements).unwrap();
    d
}

const SMALL: &str = "title = \"Small\"\nrequirements = \"requirements.csv\"\n[[scenario]]\nid = \"RUN-MARS\"\npath = \"mars-orbit-pnt.toml\"\n";

// ------------------------------------------------------------------ provenance

#[test]
fn fixture_scenarios_are_the_files_provenance_names() {
    let prov = std::fs::read_to_string(fixtures().join("PROVENANCE.md")).unwrap();
    for (_, stem, kind) in OPEN_RUNS {
        let bytes = std::fs::read(fixtures().join(format!("{stem}.toml"))).unwrap();
        let line = prov
            .lines()
            .find(|l| l.starts_with(&format!("| `{stem}.toml` |")))
            .unwrap_or_else(|| panic!("PROVENANCE.md has a row for {stem}.toml"));
        assert!(
            line.contains(&sha256_hex(&bytes)),
            "PROVENANCE.md records another SHA-256 for {stem}.toml"
        );
        assert!(line.contains(&format!("`{kind}`")), "{stem}: kind {kind}");
    }
}

/// No file of this track names the engine's optional system preset, and neither does
/// any face of a built dossier. (The needle is assembled here so that this file does
/// not contain it.)
#[test]
fn no_file_of_this_track_names_the_optional_preset() {
    let needle = ["cel", "este"].concat();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = vec![
        root.join("docs/MISSION-DOSSIER.md"),
        root.join("tests/mission_dossier.rs"),
    ];
    for dir in [root.join("src/mission"), fixtures()] {
        for e in std::fs::read_dir(dir).unwrap() {
            files.push(e.unwrap().path());
        }
    }
    let out = &shared().out_dir;
    for f in [
        "dossier.json",
        "dossier.html",
        "dossier.pdf",
        "trace.toml",
        "manifest.json",
    ] {
        files.push(out.join(f));
    }
    assert!(
        files.len() > 20,
        "the scan found its files: {}",
        files.len()
    );
    for f in files {
        let text = String::from_utf8_lossy(&std::fs::read(&f).unwrap()).to_lowercase();
        assert!(
            !text.contains(&needle),
            "{} names the optional preset",
            f.display()
        );
        // The scan is looking at real content.
        assert!(!text.is_empty(), "{} is empty", f.display());
    }
    // And no fixture scenario is one of the preset's own.
    for e in std::fs::read_dir(fixtures()).unwrap() {
        let name = e.unwrap().file_name().to_string_lossy().to_lowercase();
        assert!(!name.contains(&needle));
    }
}

// ------------------------------------------------------------------ 1. verdicts

#[test]
fn the_worked_programme_runs_five_kinds_and_its_verdicts_are_the_engines_findings() {
    let d = &shared().dossier;

    // Five runs of five kinds of the open engine, each the kind the engine says.
    let sheets: Vec<(&str, &str)> = d
        .body
        .sheets
        .iter()
        .map(|s| (s.id.as_str(), s.kind.as_str()))
        .collect();
    assert_eq!(
        sheets,
        OPEN_RUNS
            .iter()
            .map(|(id, _, kind)| (*id, *kind))
            .collect::<Vec<_>>()
    );
    for (id, stem, kind) in OPEN_RUNS {
        let sheet = d.body.sheets.iter().find(|s| s.id == id).unwrap();
        assert!(!sheet.pro_kind);
        // The stored result is the direct run's document.
        let stored: Value = serde_json::from_slice(
            &std::fs::read(shared().out_dir.join(&sheet.result.path)).unwrap(),
        )
        .unwrap();
        assert_eq!(&stored, direct(stem), "{id}: stored result vs direct run");
        let report = kshana::api::list_scenario_kinds();
        assert!(
            report.iter().any(|m| m.name == kind),
            "{kind} is an open kind"
        );
    }

    // Counts.
    let s = &d.body.summary;
    assert_eq!(s.requirements, 12);
    assert_eq!((s.met, s.not_met, s.not_verified), (7, 4, 1));
    assert_eq!(s.runs, 5);
    assert_eq!(s.trade_studies, 1);
    assert_eq!(s.without_evidence, 1);
    assert_eq!(s.verdicts, 11);
    assert_eq!(
        s.verdicts_by_basis.validated + s.verdicts_by_basis.modelled + s.verdicts_by_basis.partner,
        s.verdicts
    );

    // Low Earth orbit chain: PPP convergence with and without the LEO layer.
    let leo = direct("leo-pnt-end-to-end");
    let with_leo = num(leo, &["ppp", "cases", "1", "median_convergence_min"]);
    let gnss_only = num(leo, &["ppp", "cases", "0", "median_convergence_min"]);
    assert_eq!((with_leo, gnss_only), (7.5, 11.5));
    assert_eq!(leo["ppp"]["cases"][0]["n_leo"], 0);
    assert_eq!(leo["ppp"]["cases"][1]["n_leo"], 240);
    assert_eq!(row(d, "LEO-001").verdict, "met");
    assert_eq!(value_read(d, "LEO-001"), with_leo);
    assert_eq!(row(d, "LEO-002").verdict, "not met");
    assert_eq!(value_read(d, "LEO-002"), gnss_only);
    let rms = num(leo, &["fusion", "fused", "rms_error_3d_m"]);
    assert!(rms <= 0.5);
    assert_eq!(row(d, "LEO-003").verdict, "met");
    assert_eq!(value_read(d, "LEO-003"), rms);

    // Lunar constellation: availability misses 95 %; median PDOP is inside 6.
    let lunar = direct("lunar-relay-constellation");
    let avail = num(lunar, &["global", "availability_pct"]);
    assert_eq!(avail, 21.3286);
    assert_eq!(row(d, "LUN-001").verdict, "not met");
    assert_eq!(value_read(d, "LUN-001"), avail);
    let pdop = num(lunar, &["global", "pdop", "median"]);
    assert!(pdop <= 6.0);
    assert_eq!(row(d, "LUN-002").verdict, "met");
    assert_eq!(value_read(d, "LUN-002"), pdop);

    // Spectrum: GPS L5 keeps its C/N0, GPS L1 C/A loses it.
    let spectrum = direct("l-band-waterfall-jamming");
    assert_eq!(spectrum["timeline"]["bands"][3]["name"], "gps-l5");
    assert_eq!(spectrum["timeline"]["bands"][0]["name"], "gps-l1ca");
    let l5 = num(spectrum, &["timeline", "bands", "3", "min_cn0_dbhz"]);
    let l1 = num(spectrum, &["timeline", "bands", "0", "min_cn0_dbhz"]);
    assert!((l5 - 44.08).abs() < 0.005, "GPS L5 minimum C/N0 is {l5}");
    assert!((l1 - 3.23).abs() < 0.005, "GPS L1 C/A minimum C/N0 is {l1}");
    assert_eq!(row(d, "SPC-001").verdict, "met");
    assert_eq!(value_read(d, "SPC-001"), l5);
    assert_eq!(row(d, "SPC-002").verdict, "not met");
    assert_eq!(value_read(d, "SPC-002"), l1);

    // Campaign: the 50 ns guard is exceeded at 183 of 458 timeline steps.
    let campaign = direct("campaign-jam-spoof-holdover-integrity");
    let channels = &campaign["timeline"]["channels"];
    let err = channels["time_error_ns"]["values"].as_array().unwrap();
    let guard = channels["guard_ns"]["values"].as_array().unwrap();
    assert_eq!(err.len(), 458);
    let over: Vec<usize> = (0..err.len())
        .filter(|i| match (err[*i].as_f64(), guard[*i].as_f64()) {
            (Some(e), Some(g)) => e > g,
            _ => false,
        })
        .collect();
    assert_eq!(over.len(), 183);
    assert!(guard.iter().all(|g| g.as_f64() == Some(50.0)));
    let cmp1 = row(d, "CMP-001");
    assert_eq!(cmp1.verdict, "not met");
    assert_eq!(
        value_read(d, "CMP-001"),
        err[over[0]].as_f64().unwrap(),
        "the value read is the first step over the guard"
    );
    let detail = &cmp1.cells[0].values.last().unwrap().detail;
    assert!(
        detail.contains("275 of 458 elements satisfy"),
        "458 - 183 = 275 steps are inside the guard: {detail}"
    );
    let first_alarm = campaign["timeline"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["t_s"].as_f64())
        .fold(f64::INFINITY, f64::min);
    assert_eq!(first_alarm, 1200.0);
    assert_eq!(row(d, "CMP-002").verdict, "met");
    assert_eq!(value_read(d, "CMP-002"), first_alarm);

    // Mars: constellation-only error and availability.
    let mars = direct("mars-orbit-pnt");
    let rms = num(mars, &["fom", "rms_error_relays_m"]);
    assert!((rms - 1.8827).abs() < 5e-5, "Mars RMS error is {rms}");
    assert_eq!(row(d, "MARS-001").verdict, "met");
    assert_eq!(value_read(d, "MARS-001"), rms);
    let av = num(mars, &["fom", "availability_relays"]);
    assert_eq!(row(d, "MARS-002").verdict, "met");
    assert_eq!(value_read(d, "MARS-002"), av);

    // The inspection requirement has no criterion and no run: not verified.
    let ops = row(d, "OPS-001");
    assert_eq!(ops.verdict, "not verified");
    assert!(ops.cells.is_empty());
}

// ------------------------------------------------------------------ 2. open items

#[test]
fn open_items_and_basis_flags_survive_composition() {
    let b = shared();
    let d = &b.dossier;

    // Exactly what the requirements tooling lists for the written trace.
    let trace = Trace::load(&b.out_dir.join("trace.toml")).expect("the written trace loads");
    let v = vcrm(&trace);
    assert_eq!(d.body.open_items, v.open_items);
    assert_eq!(d.body.summary.open_items, v.open_items.len());

    // A test-method requirement whose criterion is met is still an open item.
    let leo3 = row(d, "LEO-003");
    assert_eq!(
        (leo3.method.as_str(), leo3.verdict.as_str()),
        ("test", "met")
    );
    assert!(d.body.open_items.iter().any(|o| {
        o.requirement.as_deref() == Some("LEO-003")
            && o.category == "method"
            && o.text
                .contains("the test itself still needs its own record")
    }));
    // So is a demonstration-method one.
    assert_eq!(row(d, "CMP-002").verdict, "met");
    assert!(d
        .body
        .open_items
        .iter()
        .any(|o| { o.requirement.as_deref() == Some("CMP-002") && o.category == "method" }));
    // An analysis requirement that is met has no method item.
    assert!(!d
        .body
        .open_items
        .iter()
        .any(|o| { o.requirement.as_deref() == Some("MARS-001") && o.category == "method" }));

    // Every verdict on a basis that is not VALIDATED is flagged, and every met one is
    // an open item naming its label.
    let mut flagged_met = 0;
    for r in &d.body.matrix {
        for c in &r.cells {
            assert!(["VALIDATED", "MODELLED", "PARTNER"].contains(&c.basis.as_str()));
            assert_eq!(c.flagged, c.basis != "VALIDATED", "{}", r.requirement);
            if c.flagged && c.verdict == "met" {
                flagged_met += 1;
                assert!(
                    d.body.open_items.iter().any(|o| {
                        o.requirement.as_deref() == Some(r.requirement.as_str())
                            && o.run.as_deref() == Some(c.run.as_str())
                            && o.category == format!("{} basis", c.basis)
                    }),
                    "{} on {} is met on a {} basis and must be an open item",
                    r.requirement,
                    c.run,
                    c.basis
                );
            }
        }
    }
    assert_eq!(flagged_met, d.body.summary.met_on_modelled_basis);
    assert!(flagged_met > 0);
    // Not-met lines are open items too.
    for id in ["LEO-002", "LUN-001", "SPC-002", "CMP-001"] {
        assert!(d
            .body
            .open_items
            .iter()
            .any(|o| o.requirement.as_deref() == Some(id) && o.category == "not met"));
    }
}

/// The integrity checks of the requirements tooling hold on the files the build wrote:
/// each open run's report records the hashes of the scenario and result beside it.
#[test]
fn every_open_run_has_a_report_whose_hashes_match_its_files() {
    let b = shared();
    let trace = Trace::load(&b.out_dir.join("trace.toml")).unwrap();
    assert_eq!(trace.runs.len(), 6);
    for run in &trace.runs {
        assert!(
            run.integrity_problems.is_empty(),
            "{}: {:?}",
            run.id,
            run.integrity_problems
        );
    }
    for (id, stem, kind) in OPEN_RUNS {
        let dir = b.out_dir.join("runs").join(id);
        let report: Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("{stem}.report.json"))).unwrap(),
        )
        .unwrap();
        let repro = &report["reproducibility"];
        assert_eq!(report["kind"], kind);
        assert_eq!(
            repro["scenario_sha256"],
            sha256_hex(&std::fs::read(dir.join(format!("{stem}.toml"))).unwrap())
        );
        assert_eq!(
            repro["result_sha256"],
            sha256_hex(&std::fs::read(dir.join(format!("{stem}.result.json"))).unwrap())
        );
        assert_eq!(repro["command"], format!("kshana {stem}.toml"));
        // The stored scenario is the fixture, byte for byte.
        assert_eq!(
            std::fs::read(dir.join(format!("{stem}.toml"))).unwrap(),
            std::fs::read(fixtures().join(format!("{stem}.toml"))).unwrap()
        );
        assert!(dir.join(format!("{stem}.report.html")).exists());
        assert!(dir.join(format!("{stem}.chart.svg")).exists());
        // The sheet's capability labels are the report's.
        let sheet = b.dossier.body.sheets.iter().find(|s| s.id == id).unwrap();
        let rows = report["capabilities"]["rows"].as_array().unwrap();
        assert_eq!(sheet.capabilities.len(), rows.len());
        for (c, r) in sheet.capabilities.iter().zip(rows) {
            assert_eq!(r["requirement"], c.requirement.as_str());
            assert_eq!(r["label"], c.label.as_str());
        }
        let notes = report["not_modelled"]["items"].as_array().unwrap();
        assert_eq!(sheet.not_modelled.len(), notes.len());
        assert!(
            !sheet.not_modelled.is_empty(),
            "{id} states what it leaves out"
        );
    }
    // The Pro kind has no report, and the dossier says what its integrity rests on.
    let trade = b
        .dossier
        .body
        .reproducibility
        .iter()
        .find(|r| r.run == "TRADE")
        .expect("the trade study is in the reproducibility record");
    assert!(!trade.free_engine);
    assert!(trade.report.is_none());
    assert!(trade.statement.contains("result hash alone"));
    assert!(!b
        .out_dir
        .join("runs/TRADE/trade-lunar-relay.report.json")
        .exists());
}

// ------------------------------------------------------------------ 3. determinism

/// Blank the two random strings of the PDF trailer `/ID` array (see
/// `tests/report_pdf.rs`): the one field `printpdf` does not let a caller pin.
fn mask_pdf_id(bytes: &[u8]) -> Vec<u8> {
    let needle = b"/ID[";
    let mut out = bytes.to_vec();
    if let Some(pos) = bytes.windows(needle.len()).position(|w| w == needle) {
        let mut i = pos + needle.len();
        let mut blanked = 0;
        while i < out.len() && blanked < 2 {
            if out[i] == b'(' {
                i += 1;
                while i < out.len() && out[i] != b')' {
                    out[i] = b'#';
                    i += 1;
                }
                blanked += 1;
            } else if out[i] == b']' {
                break;
            } else {
                i += 1;
            }
        }
    }
    out
}

#[test]
fn two_builds_are_identical_apart_from_the_pdf_trailer_id() {
    let a = build_worked("det-a");
    let b = build_worked("det-b");
    assert_eq!(a.dossier.dossier_sha256, b.dossier.dossier_sha256);
    assert_eq!(a.dossier.dossier_sha256.len(), 64);
    for f in [
        "dossier.json",
        "dossier.html",
        "trace.toml",
        "evidence-pack.json",
        "evidence-pack.html",
    ] {
        assert_eq!(
            std::fs::read(a.out_dir.join(f)).unwrap(),
            std::fs::read(b.out_dir.join(f)).unwrap(),
            "{f} differs between two builds"
        );
    }
    let (pa, pb) = (
        std::fs::read(a.out_dir.join("dossier.pdf")).unwrap(),
        std::fs::read(b.out_dir.join("dossier.pdf")).unwrap(),
    );
    assert_eq!(pa.len(), pb.len());
    assert_eq!(mask_pdf_id(&pa), mask_pdf_id(&pb));
    assert!(
        pa.windows(4).any(|w| w == b"/ID["),
        "the /ID is there to mask"
    );
    assert_ne!(mask_pdf_id(&pa), pa, "the mask changed something");

    // The issue stamp is outside the hash: another stamp, the same hash, other bytes.
    let c = build(
        &fixtures().join("programme.toml"),
        &tmp("det-c"),
        &BuildOptions {
            issued_utc: Some("2027-01-01T00:00:00Z".into()),
            ..opts(enterprise())
        },
    )
    .unwrap();
    assert_eq!(c.dossier.dossier_sha256, a.dossier.dossier_sha256);
    assert_ne!(
        std::fs::read(c.out_dir.join("dossier.json")).unwrap(),
        std::fs::read(a.out_dir.join("dossier.json")).unwrap()
    );
    // The hash is over the canonical body, and a reader can recompute it.
    let doc: Value =
        serde_json::from_slice(&std::fs::read(a.out_dir.join("dossier.json")).unwrap()).unwrap();
    assert_eq!(
        sha256_hex(serde_json::to_string(&doc["body"]).unwrap().as_bytes()),
        a.dossier.dossier_sha256
    );
    assert_eq!(doc["issued_utc"], UTC);
}

// ------------------------------------------------------------------ 4. the free engine

/// The free engine reproduces every open-kind run of the dossier.
///
/// Needs the open engine's command-line binary: set `KSHANA_OPEN_BIN` to its path. For
/// each run the stored scenario is copied alone into a scratch directory, the binary is
/// run there as the dossier's record says (`kshana <file>`), and the file it writes is
/// compared with the dossier's.
///
/// With a release build of the engine the result files must be byte-identical (set
/// `KSHANA_OPEN_BIN_STRICT=1` to require it on every run). An unoptimised build may
/// differ in floating-point fields; then the test prints exactly which, requires every
/// verdict to be unchanged and every difference (the values the verdicts read
/// included) to be relative 1e-4 or less, and says so. That is the statement the
/// dossier makes.
#[test]
#[ignore = "needs KSHANA_OPEN_BIN, the path of the open engine's command-line binary"]
fn the_free_engine_reproduces_every_open_run() {
    let open_bin = std::env::var("KSHANA_OPEN_BIN")
        .expect("set KSHANA_OPEN_BIN to the open engine's command-line binary");
    let strict = std::env::var("KSHANA_OPEN_BIN_STRICT").is_ok_and(|v| v == "1");
    let b = shared();
    let d = &b.dossier;
    let trace = Trace::load(&b.out_dir.join("trace.toml")).unwrap();
    let mut identical = 0;
    for r in d.body.reproducibility.iter().filter(|r| r.free_engine) {
        let (_, stem, _) = OPEN_RUNS
            .iter()
            .find(|(id, _, _)| *id == r.run)
            .expect("an open run of the worked programme");
        let scratch = tmp(&format!("free-{}", r.run));
        std::fs::create_dir_all(&scratch).unwrap();
        let file = format!("{stem}.toml");
        std::fs::copy(b.out_dir.join(&r.scenario.path), scratch.join(&file)).unwrap();
        assert_eq!(r.command, format!("kshana {file}"));
        let out = Command::new(&open_bin)
            .arg(&file)
            .current_dir(&scratch)
            .output()
            .expect("the open binary runs");
        assert!(
            out.status.success(),
            "{}: {}",
            r.run,
            String::from_utf8_lossy(&out.stderr)
        );
        // The scenario the free engine ran is the dossier's.
        assert_eq!(
            sha256_hex(&std::fs::read(scratch.join(&file)).unwrap()),
            r.scenario.sha256
        );
        let fresh = std::fs::read(scratch.join(format!("{stem}.result.json"))).unwrap();
        let sha = sha256_hex(&fresh);
        if sha == r.result.sha256 {
            identical += 1;
            // Then the report the free engine writes is the dossier's too.
            let report = std::fs::read(scratch.join(format!("{stem}.report.json"))).unwrap();
            assert_eq!(
                Some(sha256_hex(&report)),
                r.report.as_ref().map(|f| f.sha256.clone()),
                "{}: same result bytes but another report",
                r.run
            );
            eprintln!("{}: result and report byte-identical ({sha})", r.run);
            continue;
        }
        let stored: Value =
            serde_json::from_slice(&std::fs::read(b.out_dir.join(&r.result.path)).unwrap())
                .unwrap();
        let fresh: Value = serde_json::from_slice(&fresh).unwrap();
        let (n, list) = differences(&stored, &fresh, 400);
        eprintln!(
            "{}: the free engine's result hashes to {sha}, the dossier records {}; {n} fields differ:",
            r.run, r.result.sha256
        );
        for line in &list {
            eprintln!("    {line}");
        }
        assert!(
            !strict,
            "{}: KSHANA_OPEN_BIN_STRICT=1 requires byte identity; {n} fields differ",
            r.run
        );
        // What does reproduce: every verdict, and every field to a relative 1e-4.
        let mut run = trace.run(&r.run).expect("the run is in the trace").clone();
        run.result = fresh.clone();
        for row in &d.body.matrix {
            let req = trace
                .requirements
                .iter()
                .find(|q| q.id == row.requirement)
                .unwrap();
            for c in row.cells.iter().filter(|c| c.run == r.run) {
                let again = kshana_pro::mbse::trace::evaluate_cell(&trace, req, &run);
                assert_eq!(
                    again.verdict.as_str(),
                    c.verdict,
                    "{} on {}: the verdict changes on the free engine's result",
                    row.requirement,
                    r.run
                );
                let read: Vec<&Value> = again
                    .evidence
                    .observations
                    .iter()
                    .map(|o| &o.value)
                    .collect();
                let recorded: Vec<&Value> = c.values.iter().map(|v| &v.value).collect();
                assert_eq!(read.len(), recorded.len());
                for (a, b) in read.iter().zip(&recorded) {
                    match (a.as_f64(), b.as_f64()) {
                        (Some(x), Some(y)) if x != y => {
                            let rel = (x - y).abs() / x.abs().max(y.abs());
                            eprintln!(
                                "    {} reads {x} on the free engine's result, {y} in the dossier (relative difference {rel:.1e}); the verdict is unchanged",
                                row.requirement
                            );
                            assert!(rel <= 1e-4, "{}: {x} vs {y}", row.requirement);
                        }
                        _ => assert_eq!(a, b, "{}: the value read changes", row.requirement),
                    }
                }
            }
        }
        for line in &list {
            let rel = line
                .split("relative difference ")
                .nth(1)
                .and_then(|s| s.trim_end_matches(')').parse::<f64>().ok())
                .unwrap_or_else(|| panic!("{}: a non-numeric field differs: {line}", r.run));
            assert!(rel <= 1e-4, "{}: {line}", r.run);
        }
    }
    eprintln!(
        "{identical} of {} open runs are byte-identical on {open_bin}",
        OPEN_RUNS.len()
    );
    assert!(
        identical >= 4,
        "at most the LEO chain differs on an unoptimised build"
    );
}

// ------------------------------------------------------------------ 5. tamper

#[test]
fn a_changed_result_byte_fails_the_check_and_names_the_file() {
    let b = build_worked("tamper");
    let clean = check(&b.out_dir, None).unwrap();
    assert!(clean.passed(), "{:?}", clean.problems);
    // The count is of distinct files: every file the manifest lists, each once, although
    // the files the body lists as well are compared with both records.
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(b.out_dir.join("manifest.json")).unwrap()).unwrap();
    let listed = manifest["files"].as_array().unwrap().len();
    assert_eq!(
        listed, 35,
        "three faces, the evidence pack twice, and 30 more"
    );
    assert_eq!(clean.files_checked, listed);
    assert_eq!(
        b.dossier.body.identification.files.len(),
        30,
        "the trace, the requirements copy, 5 files for each of 5 open runs and 3 for the trade"
    );

    let victim = "runs/RUN-MARS/mars-orbit-pnt.result.json";
    let path = b.out_dir.join(victim);
    let mut bytes = std::fs::read(&path).unwrap();
    // One byte: a digit of the first number that has one.
    let i = bytes.iter().position(|c| *c == b'7').unwrap();
    bytes[i] = b'8';
    std::fs::write(&path, &bytes).unwrap();

    let report = check(&b.out_dir, None).unwrap();
    assert!(!report.passed());
    assert!(
        report.problems.iter().any(|p| p.starts_with(victim)),
        "{:?}",
        report.problems
    );
    // Only that file is named as changed.
    assert!(report
        .problems
        .iter()
        .all(|p| p.starts_with(victim) || p.starts_with("manifest.json")));

    let out = bin()
        .args(["mission", "check"])
        .arg(&b.out_dir)
        .arg("--dev")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(&format!("FAIL: {victim}")), "{stdout}");

    // A changed face of the dossier is caught by the manifest.
    let b2 = build_worked("tamper-html");
    let html = b2.out_dir.join("dossier.html");
    let mut text = std::fs::read_to_string(&html).unwrap();
    text = text.replacen("not met", "met", 1);
    std::fs::write(&html, text).unwrap();
    let r2 = check(&b2.out_dir, None).unwrap();
    assert!(r2.problems.iter().any(|p| p.starts_with("dossier.html")));

    // A changed verdict inside dossier.json no longer matches the dossier hash.
    let b3 = build_worked("tamper-json");
    let json = b3.out_dir.join("dossier.json");
    let text = std::fs::read_to_string(&json).unwrap().replacen(
        "\"verdict\": \"not met\"",
        "\"verdict\": \"met\"",
        1,
    );
    std::fs::write(&json, text).unwrap();
    let r3 = check(&b3.out_dir, None).unwrap();
    assert!(r3
        .problems
        .iter()
        .any(|p| p.starts_with("dossier.json: the body hashes to")));
}

#[test]
fn check_with_rerun_passes_on_an_untouched_dossier() {
    let b = build_worked("rerun");
    let ent = enterprise();
    let report = check(&b.out_dir, Some((&ent, 1_790_000_000))).unwrap();
    assert!(report.passed(), "{:?}", report.problems);
    assert_eq!(report.reruns.len(), 6);
    for r in &report.reruns {
        assert!(r.matches, "{} re-ran to other bytes", r.run);
        assert_eq!(r.rerun_sha256.as_deref(), Some(r.recorded_sha256.as_str()));
    }
    let out = bin()
        .args(["mission", "check"])
        .arg(&b.out_dir)
        .args(["--rerun", "--dev"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("RUN-LUNAR: re-run gives the same result bytes"));
    assert!(stdout.contains("6 runs repeated"), "{stdout}");

    // A stored scenario edited after delivery: the re-run no longer matches, and the
    // check says which fields moved.
    let scenario = b.out_dir.join("runs/RUN-MARS/mars-orbit-pnt.toml");
    let text = std::fs::read_to_string(&scenario)
        .unwrap()
        .replace("seed = 7", "seed = 8");
    std::fs::write(&scenario, text).unwrap();
    let moved = check(&b.out_dir, Some((&ent, 1_790_000_000))).unwrap();
    let mars = moved.reruns.iter().find(|r| r.run == "RUN-MARS").unwrap();
    assert!(!mars.matches);
    assert!(!mars.differences.is_empty());
    assert!(moved
        .problems
        .iter()
        .any(|p| p.starts_with("RUN-MARS: the re-run result hashes to")));
}

// ------------------------------------------------------------------ 6. figures

#[test]
fn headline_figures_are_computed_fields_never_echoed_inputs() {
    let d = &shared().dossier;
    let lunar = d.body.sheets.iter().find(|s| s.id == "RUN-LUNAR").unwrap();
    let avail = lunar
        .headline
        .iter()
        .find(|f| f.path == "global.availability_pct")
        .expect("availability is a headline figure");
    assert_eq!(
        avail.value,
        num(
            direct("lunar-relay-constellation"),
            &["global", "availability_pct"]
        )
    );
    assert_eq!(avail.unit.as_deref(), Some("%"));
    for list in [&lunar.headline, &lunar.figures] {
        assert!(list.iter().all(|f| f.provenance == "computed"));
        for input in ["body.mu_km3_s2", "inputs.mask_deg"] {
            assert!(
                !list.iter().any(|f| f.path == input),
                "{input} is not a figure"
            );
        }
    }
    // They are listed for what they are: inputs.
    for input in ["body.mu_km3_s2", "inputs.mask_deg"] {
        let f = lunar
            .inputs
            .iter()
            .find(|f| f.path == input)
            .unwrap_or_else(|| panic!("{input} is listed as an input"));
        assert_ne!(f.provenance, "computed");
    }
    // Every sheet: each figure's provenance is what the result's own units block says.
    for (id, stem, _) in OPEN_RUNS {
        let sheet = d.body.sheets.iter().find(|s| s.id == id).unwrap();
        let units = direct(stem)["units"].as_object().unwrap();
        assert!(!sheet.figures.is_empty(), "{id} has computed scalars");
        for f in sheet
            .figures
            .iter()
            .chain(&sheet.other_outputs)
            .chain(&sheet.inputs)
        {
            // The units key has `[]` where the figure path has an index.
            let mut key = String::new();
            let mut in_index = false;
            for ch in f.path.chars() {
                match ch {
                    '[' => {
                        in_index = true;
                        key.push('[');
                    }
                    ']' => {
                        in_index = false;
                        key.push(']');
                    }
                    c if !in_index => key.push(c),
                    _ => {}
                }
            }
            let (_, entry) = kshana::field_schema::lookup(units, &key)
                .unwrap_or_else(|| panic!("{id}: {key} has a units entry"));
            assert_eq!(entry["provenance"], f.provenance.as_str(), "{id}: {key}");
        }
        assert!(sheet.headline.len() <= sheet.figures.len());
    }
    // Naming an input as a figure is refused, with the reason.
    let dir = small_programme(
        "figure-input",
        &format!("{SMALL}figures = [\"sigma_relay_range_m\"]\n"),
        "id,text,method,criterion,runs\nR1,x,analysis,fom.median_pdop <= 6,RUN-MARS\n",
    );
    let e = build(
        &dir.join("programme.toml"),
        &dir.join("out"),
        &opts(enterprise()),
    )
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("sigma_relay_range_m") && e.contains("only `computed` fields are figures"),
        "{e}"
    );
}

#[test]
fn charts_hold_the_results_own_columns() {
    let d = &shared().dossier;
    let campaign = d
        .body
        .sheets
        .iter()
        .find(|s| s.id == "RUN-CAMPAIGN")
        .unwrap();
    let chart = &campaign.charts[0];
    assert_eq!(chart.style, "line");
    let direct_err = direct("campaign-jam-spoof-holdover-integrity")["timeline"]["channels"]
        ["time_error_ns"]["values"]
        .as_array()
        .unwrap();
    assert_eq!(chart.points, 458);
    assert_eq!(chart.stride, 1);
    assert_eq!(chart.series[0].values.len(), 458);
    for (a, b) in chart.series[0].values.iter().zip(direct_err) {
        assert_eq!(*a, b.as_f64());
    }
    assert_eq!(chart.series[0].unit.as_deref(), Some("ns"));
    let spectrum = d
        .body
        .sheets
        .iter()
        .find(|s| s.id == "RUN-SPECTRUM")
        .unwrap();
    let bars = &spectrum.charts[0];
    assert_eq!(bars.style, "bar");
    assert_eq!(
        bars.labels,
        vec!["gps-l1ca", "galileo-e1", "gps-l2c", "gps-l5", "galileo-e5a"]
    );
    // A chart on a path the result does not have is refused, naming the chart.
    let dir = small_programme(
        "chart-bad",
        &format!("{SMALL}[[scenario.chart]]\ntitle = \"Nope\"\nx = \"epochs[].t_s\"\ny = [\"epochs[].nope\"]\n"),
        "id,text,method,criterion,runs\nR1,x,analysis,fom.median_pdop <= 6,RUN-MARS\n",
    );
    let e = build(
        &dir.join("programme.toml"),
        &dir.join("out"),
        &opts(enterprise()),
    )
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("chart 'Nope'") && e.contains("epochs[].nope"),
        "{e}"
    );
}

// ------------------------------------------------------------------ 7. the PDF

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}

#[test]
fn the_pdf_parses_and_carries_the_requirements_and_the_honesty_words() {
    let b = shared();
    let pdf = std::fs::read(b.out_dir.join("dossier.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let tail = String::from_utf8_lossy(&pdf[pdf.len() - 32..]).to_string();
    assert!(tail.trim_end().ends_with("%%EOF"), "{tail}");
    assert!(count(&pdf, b"startxref") == 1 && count(&pdf, b"trailer") >= 1);

    // Page count, three ways: what the renderer laid out, the page objects in the file,
    // and the count the page tree states.
    let page_objects = count(&pdf, b"/Type/Page") - count(&pdf, b"/Type/Pages");
    assert_eq!(page_objects, b.pdf_pages);
    assert_eq!(
        count(&pdf, format!("/Count {}", b.pdf_pages).as_bytes()),
        1,
        "the page tree states {} pages",
        b.pdf_pages
    );
    // A cover, the matrix, one sheet or more per each of five runs, the trade, and the
    // closing pages: nine at least.
    assert!(b.pdf_pages >= 9, "{} pages", b.pdf_pages);
    // A4 (210 mm by 297 mm is 595 by 842 points), on every page.
    assert_eq!(count(&pdf, b"/MediaBox[0 0 595 842]"), b.pdf_pages);
    // Every page is numbered.
    for i in 1..=b.pdf_pages {
        let label = format!("page {i} of {}", b.pdf_pages);
        assert_eq!(count(&pdf, label.as_bytes()), 1, "{label}");
    }

    let text = String::from_utf8_lossy(&pdf);
    for r in &b.dossier.body.matrix {
        assert!(
            text.contains(&format!("({}) Tj", r.requirement)),
            "requirement {} is printed in the PDF",
            r.requirement
        );
    }
    for s in &b.dossier.body.sheets {
        assert!(text.contains(&format!("(Run {}) Tj", s.id)));
    }
    // The banner, as its own lines of text on the cover.
    assert_eq!(
        count(&pdf, b"(MODELLED RESULTS - every number in this dossier"),
        1
    );
    assert_eq!(
        count(
            &pdf,
            b"(This is an engineering study, not a certification and not acceptance by any authority.) Tj"
        ),
        1
    );
    // On every page: the footer carries the words.
    assert!(count(&pdf, b"MODELLED engineering study - not a certification") >= b.pdf_pages);
    assert!(text.contains(&b.dossier.dossier_sha256));
    // Filled shapes are painted, not used as clipping paths (a clip would hide
    // everything drawn after it on the page).
    assert_eq!(count(&pdf, b"\nW\nn\n"), 0, "no clipping path in the PDF");
    assert!(
        count(&pdf, b"\nf\n") > 20,
        "the cover band, banner, tiles and bars are filled"
    );
    // Charts are vectors: no image object anywhere.
    assert_eq!(count(&pdf, b"/Subtype/Image"), 0);
    assert_eq!(count(&pdf, b"/Subtype /Image"), 0);
}

// ------------------------------------------------------------------ the HTML

#[test]
fn the_html_is_self_contained_and_shows_the_same_dossier() {
    let b = shared();
    let html = std::fs::read_to_string(b.out_dir.join("dossier.html")).unwrap();
    for banned in [
        "<script",
        "<link",
        "<iframe",
        "@import",
        "url(",
        "src=\"http",
        "src='http",
        "href=\"http",
        "href='http",
        "srcset",
    ] {
        assert!(!html.contains(banned), "the page contains {banned}");
    }
    assert!(html.contains("@media print"));
    assert!(html.contains(
        "<div class=\"banner\">MODELLED RESULTS - every number in this dossier is simulation output"
    ));
    assert!(html.contains(
        "This is an engineering study, not a certification and not acceptance by any authority.</div>"
    ));
    assert!(html.contains(&b.dossier.dossier_sha256));
    for r in &b.dossier.body.matrix {
        assert!(html.contains(&format!("<b>{}</b>", r.requirement)));
    }
    // One sheet per run, each with the engine's own chart embedded and the programme's
    // charts drawn inline.
    assert_eq!(html.matches("<section class=\"sheet\"").count(), 5);
    assert_eq!(html.matches("data:image/svg+xml;charset=utf-8,").count(), 5);
    let charts: usize = b.dossier.body.sheets.iter().map(|s| s.charts.len()).sum();
    assert_eq!(html.matches("<svg class=\"chart\"").count(), charts);
    assert!(charts >= 5);
    assert!(html.contains("id=\"trade\""));
    assert!(html.contains("id=\"signoff\""));
}

// ------------------------------------------------------------------ the trade

#[test]
fn the_trade_section_is_read_from_the_variants_own_runs() {
    let t = shared()
        .dossier
        .body
        .trade
        .as_ref()
        .expect("the worked programme has a trade study");
    assert_eq!(t.metric, "global.availability_pct");
    assert!(!t.lower_better);
    // Run each variant's scenario text directly on the open engine.
    let file: toml::Value = std::fs::read_to_string(fixtures().join("trade-lunar-relay.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let variants = file["variant"].as_array().unwrap();
    assert_eq!(variants.len(), 2);
    let mut best: Option<(String, f64)> = None;
    for v in variants {
        let label = v["label"].as_str().unwrap();
        let out = kshana::api::run_scenario(v["scenario"].as_str().unwrap()).unwrap();
        let doc: Value = serde_json::from_str(&out.json).unwrap();
        let value = num(&doc, &["global", "availability_pct"]);
        let row = t.variants.iter().find(|x| x.label == label).unwrap();
        assert_eq!(row.metric_value, Some(value), "{label}");
        if best.as_ref().is_none_or(|(_, b)| value > *b) {
            best = Some((label.to_string(), value));
        }
    }
    let (winner, best_value) = best.unwrap();
    assert_eq!(t.winner.as_deref(), Some(winner.as_str()));
    // The first variant is the design RUN-LUNAR runs: the same availability.
    assert_eq!(
        t.variants
            .iter()
            .find(|v| v.label == "south-biased-14")
            .unwrap()
            .metric_value,
        Some(num(
            direct("lunar-relay-constellation"),
            &["global", "availability_pct"]
        ))
    );
    // Neither design reaches the 95 % of LUN-001; the dossier shows the real number.
    assert!(best_value < 95.0 && best_value > 21.3286);
}

// ------------------------------------------------------------------ 8. licence

#[test]
fn the_command_and_each_pro_kind_are_gated_by_the_licence() {
    let programme = fixtures().join("programme.toml");

    // Evaluation tier: the whole command is refused, and nothing is written.
    let out = tmp("gate-eval");
    let e = build(&programme, &out, &opts(Entitlement::evaluation())).unwrap_err();
    assert!(
        e.to_string()
            .contains("the mission dossier requires the 'enterprise' tier"),
        "{e}"
    );
    assert!(!out.exists());

    // The feature granted by name on an Evaluation licence: the command is allowed, but
    // the programme holds a Pro kind (the trade study) that tier does not carry.
    let by_name = |tier: Tier| Entitlement {
        licensee: "(test)".into(),
        tier,
        expires_unix: 0,
        features: vec![kshana_pro::mission::FEATURE.to_string()],
    };
    let out = tmp("gate-kind");
    let e = build(&programme, &out, &opts(by_name(Tier::Evaluation)))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("'pro-trade-study'") && e.contains("requires the 'pro' tier"),
        "{e}"
    );
    assert!(
        !e.contains("(test)"),
        "the message never names the licensee"
    );
    assert!(!out.exists(), "a refused programme runs and writes nothing");

    // The run itself is gated, not only the check before it: the trade scenario run on
    // its own under that licence is refused, and so is its re-run in a check.
    let loaded = kshana_pro::mission::programme::Programme::load(&programme).unwrap();
    let trade = loaded.scenarios.iter().find(|s| s.is_trade).unwrap();
    let scratch = tmp("gate-run-one");
    let e = kshana_pro::mission::runner::run_one(trade, &scratch, &by_name(Tier::Evaluation), 0)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("'pro-trade-study' requires the 'pro' tier"),
        "{e}"
    );
    assert!(!scratch.join("runs").exists());
    let lunar = loaded
        .scenarios
        .iter()
        .find(|s| s.id == "RUN-LUNAR")
        .unwrap();
    kshana_pro::mission::runner::run_one(lunar, &scratch, &by_name(Tier::Evaluation), 0)
        .expect("an open kind runs at any tier");
    let rerun = check(
        &shared().out_dir,
        Some((&by_name(Tier::Evaluation), 1_790_000_000)),
    )
    .unwrap();
    assert!(
        rerun
            .problems
            .iter()
            .any(|p| p.starts_with("TRADE: re-run failed") && p.contains("requires the 'pro' tier")),
        "{:?}",
        rerun.problems
    );
    assert_eq!(rerun.reruns.iter().filter(|r| r.matches).count(), 5);

    // The same licence at Pro tier carries the trade study.
    let ok = build(&programme, &tmp("gate-pro"), &opts(by_name(Tier::Pro))).unwrap();
    assert_eq!(ok.dossier.dossier_sha256, shared().dossier.dossier_sha256);

    // An expired Enterprise licence is refused.
    let expired = Entitlement {
        expires_unix: 1_000_000_000,
        ..enterprise()
    };
    assert!(build(&programme, &tmp("gate-expired"), &opts(expired)).is_err());

    // The binary: no licence is Evaluation tier and refuses; --dev runs.
    let out = tmp("gate-bin");
    let refused = bin()
        .args(["mission", "build"])
        .arg(&programme)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr)
        .contains("the mission dossier requires the 'enterprise' tier"));
    assert!(!out.exists());
    let checked = bin()
        .args(["mission", "check"])
        .arg(&shared().out_dir)
        .output()
        .unwrap();
    assert!(!checked.status.success(), "check is gated too");

    let ran = bin()
        .args(["mission", "build"])
        .arg(&programme)
        .arg("--out")
        .arg(&out)
        .args(["--utc", UTC, "--dev"])
        .output()
        .unwrap();
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout).trim(),
        shared().dossier.dossier_sha256
    );
    for f in [
        "dossier.pdf",
        "dossier.html",
        "dossier.json",
        "trace.toml",
        "evidence-pack.html",
        "evidence-pack.json",
        "manifest.json",
        "ledger.json",
        "requirements.csv",
    ] {
        assert!(out.join(f).exists(), "{f} was written");
    }
    assert_eq!(
        std::fs::read(out.join("dossier.json")).unwrap(),
        std::fs::read(shared().out_dir.join("dossier.json")).unwrap(),
        "the binary and the library write the same dossier"
    );
}

// ------------------------------------------------------------------ 9. adversarial

#[test]
fn bad_programmes_are_refused_and_a_wrong_unit_is_never_met() {
    let go = |dir: &Path| {
        build(
            &dir.join("programme.toml"),
            &dir.join("out"),
            &opts(enterprise()),
        )
    };
    let reqs = "id,text,method,criterion,runs\nR1,x,analysis,fom.median_pdop <= 6,RUN-MARS\n";

    // A scenario file that is not there.
    let d = small_programme(
        "adv-missing",
        &SMALL.replace("mars-orbit-pnt.toml", "nope.toml"),
        reqs,
    );
    let e = go(&d).unwrap_err().to_string();
    assert!(e.contains("nope.toml") && e.contains("RUN-MARS"), "{e}");
    assert!(!d.join("out").exists());

    // A requirement linked to a run the programme does not define: in the requirements
    // file, and in a [[link]].
    let d = small_programme(
        "adv-link-csv",
        SMALL,
        "id,text,method,criterion,runs\nR1,x,analysis,fom.median_pdop <= 6,RUN-NOPE\n",
    );
    let e = go(&d).unwrap_err().to_string();
    assert!(
        e.contains("'R1'") && e.contains("'RUN-NOPE'") && e.contains("does not define"),
        "{e}"
    );
    assert!(!d.join("out").exists());
    let d = small_programme(
        "adv-link-toml",
        &format!("{SMALL}[[link]]\nrequirement = \"R1\"\nruns = [\"RUN-GONE\"]\n"),
        reqs,
    );
    let e = go(&d).unwrap_err().to_string();
    assert!(
        e.contains("'RUN-GONE'") && e.contains("does not define"),
        "{e}"
    );
    let d = small_programme(
        "adv-link-req",
        &format!("{SMALL}[[link]]\nrequirement = \"R9\"\nruns = [\"RUN-MARS\"]\n"),
        reqs,
    );
    assert!(go(&d).unwrap_err().to_string().contains("'R9'"));

    // An empty requirements file, and one with a header and no rows.
    let d = small_programme("adv-empty", SMALL, "");
    let e = go(&d).unwrap_err().to_string();
    assert!(e.contains("requirements.csv"), "{e}");
    assert!(!d.join("out").exists());
    let d = small_programme("adv-header", SMALL, "id,text,method,criterion,runs\n");
    let e = go(&d).unwrap_err().to_string();
    assert!(e.contains("requirements.csv"), "{e}");

    // A criterion with the wrong unit (seconds against a field in metres, and metres
    // against a dimensionless field): not verified, never met. The same limits with the
    // right units are met, so the unit alone decides.
    let d = small_programme(
        "adv-units",
        SMALL,
        "id,text,method,criterion,runs\n\
         U1,RMS error in the wrong unit,analysis,fom.rms_error_relays_m <= 5 s,RUN-MARS\n\
         U2,PDOP with a length,analysis,fom.median_pdop <= 6 m,RUN-MARS\n\
         U3,RMS error with no unit,analysis,fom.rms_error_relays_m <= 5,RUN-MARS\n\
         OK1,RMS error,analysis,fom.rms_error_relays_m <= 5 m,RUN-MARS\n\
         OK2,RMS error in centimetres,analysis,fom.rms_error_relays_m <= 500 cm,RUN-MARS\n\
         NO1,RMS error too tight,analysis,fom.rms_error_relays_m <= 50 cm,RUN-MARS\n",
    );
    let built = go(&d).expect("a wrong unit is a finding, not a crash");
    let dossier = &built.dossier;
    for id in ["U1", "U2", "U3"] {
        let r = row(dossier, id);
        assert_eq!(r.verdict, "not verified", "{id}");
        assert_eq!(r.cells[0].verdict, "not verified");
        assert!(r.cells[0].reason.is_some(), "{id} says why");
        assert!(dossier
            .body
            .open_items
            .iter()
            .any(|o| o.requirement.as_deref() == Some(id) && o.category == "not verified"));
    }
    assert_eq!(row(dossier, "OK1").verdict, "met");
    assert_eq!(row(dossier, "OK2").verdict, "met");
    assert_eq!(row(dossier, "NO1").verdict, "not met");
    assert_eq!(
        (
            dossier.body.summary.met,
            dossier.body.summary.not_met,
            dossier.body.summary.not_verified
        ),
        (2, 1, 3)
    );
    assert!(dossier.body.trade.is_none());
    assert!(check(&built.out_dir, None).unwrap().passed());

    // A requirements file named like a file the build writes.
    let d = small_programme(
        "adv-name",
        &SMALL.replace("requirements.csv", "manifest.json"),
        reqs,
    );
    std::fs::rename(d.join("requirements.csv"), d.join("manifest.json")).unwrap();
    let e = go(&d).unwrap_err().to_string();
    assert!(e.contains("manifest.json"), "{e}");

    // A run identifier that would leave the dossier directory.
    let d = small_programme("adv-id", &SMALL.replace("RUN-MARS", "../x"), reqs);
    assert!(go(&d)
        .unwrap_err()
        .to_string()
        .contains("not usable as a directory name"));
    // A scenario that does not run.
    let d = small_programme("adv-run", SMALL, reqs);
    std::fs::write(
        d.join("mars-orbit-pnt.toml"),
        "kind = \"body-pnt\"\nbody = \"Nowhere\"\n",
    )
    .unwrap();
    let e = go(&d).unwrap_err().to_string();
    assert!(e.contains("RUN-MARS") && e.contains("did not run"), "{e}");
    // A [trade] that is not a trade study.
    let d = small_programme(
        "adv-trade",
        &format!("{SMALL}[trade]\npath = \"mars-orbit-pnt.toml\"\n"),
        reqs,
    );
    assert!(go(&d)
        .unwrap_err()
        .to_string()
        .contains("must be a pro-trade-study scenario"));
}

/// Run identifiers that differ only in letter case would share one directory on a file
/// system that ignores case (the default on macOS and Windows): the second run would
/// overwrite the first one's files and the first run's verdict would be read from the
/// second run's result. The programme is refused before anything runs.
#[test]
fn run_ids_that_differ_only_in_letter_case_are_refused() {
    // Two Mars runs with the same file name, different seeds and different errors: the
    // limit of 1.9 m is met on seed 7 (1.8827 m) and not on seed 99.
    let seed7 = std::fs::read_to_string(fixtures().join("mars-orbit-pnt.toml")).unwrap();
    assert!(seed7.contains("seed = 7"));
    let seed99 = seed7.replace("seed = 7", "seed = 99");
    let rms = |src: &str| {
        let out = kshana::api::run_scenario(src).expect("the open engine runs it");
        let doc: Value = serde_json::from_str(&out.json).unwrap();
        num(&doc, &["fom", "rms_error_relays_m"])
    };
    let (a, b) = (rms(&seed7), rms(&seed99));
    assert!(
        a <= 1.9 && b > 1.9,
        "the two runs give different verdicts: {a} and {b}"
    );

    let programme = |second_id: &str| {
        format!("{SMALL}[[scenario]]\nid = \"{second_id}\"\npath = \"other/mars-orbit-pnt.toml\"\n")
    };
    let requirements = |second_id: &str| {
        format!(
            "id,text,method,criterion,runs\n\
             R1,seed 7,analysis,fom.rms_error_relays_m <= 1.9 m,RUN-MARS\n\
             R2,seed 99,analysis,fom.rms_error_relays_m <= 1.9 m,{second_id}\n"
        )
    };
    let setup = |tag: &str, second_id: &str| {
        let d = small_programme(tag, &programme(second_id), &requirements(second_id));
        std::fs::create_dir_all(d.join("other")).unwrap();
        std::fs::write(d.join("other/mars-orbit-pnt.toml"), &seed99).unwrap();
        d
    };
    let go = |dir: &Path| {
        build(
            &dir.join("programme.toml"),
            &dir.join("out"),
            &opts(enterprise()),
        )
    };

    let d = setup("case-ids", "run-mars");
    let e = go(&d).unwrap_err().to_string();
    assert!(
        e.contains("'RUN-MARS'")
            && e.contains("'run-mars'")
            && e.contains("differ only in letter case"),
        "{e}"
    );
    assert!(!d.join("out").exists(), "nothing was run or written");

    // The trade study's identifier is reserved in any letter case.
    let d = small_programme(
        "case-trade",
        &SMALL.replace("RUN-MARS", "Trade"),
        &requirements("Trade").replace("RUN-MARS", "Trade"),
    );
    let e = go(&d).unwrap_err().to_string();
    assert!(e.contains("reserved for the [trade] study"), "{e}");

    // Identifiers that differ in more than case build, each verdict is read from its
    // own run, and the delivered dossier passes its own check.
    let d = setup("case-ok", "RUN-MARS-99");
    let built = go(&d).expect("distinct identifiers build");
    assert_eq!(row(&built.dossier, "R1").verdict, "met");
    assert_eq!(value_read(&built.dossier, "R1"), a);
    assert_eq!(row(&built.dossier, "R2").verdict, "not met");
    assert_eq!(value_read(&built.dossier, "R2"), b);
    let report = check(&built.out_dir, None).unwrap();
    assert!(report.passed(), "{:?}", report.problems);
}

/// TOML allows `nan` and `inf`, and the engine takes them: a threshold of `nan` is
/// never crossed, the result echoes it as `null`, and no sheet would show it. A
/// programme with such a scenario is refused before anything runs, and so is a
/// criterion whose limit overflows to infinity (it would hold for every value).
#[test]
fn values_that_are_not_finite_numbers_are_refused() {
    let go = |dir: &Path| {
        build(
            &dir.join("programme.toml"),
            &dir.join("out"),
            &opts(enterprise()),
        )
    };
    let reqs = "id,text,method,criterion,runs\nR1,x,analysis,fom.median_pdop <= 6,RUN-MARS\n";
    let mars = std::fs::read_to_string(fixtures().join("mars-orbit-pnt.toml")).unwrap();
    assert!(mars.contains("step_s = 300.0") && mars.contains("altitude_km = 300.0"));

    for (tag, from, to, names) in [
        ("nan-top", "step_s = 300.0", "step_s = nan", "step_s = NaN"),
        (
            "inf-nested",
            "altitude_km = 300.0",
            "altitude_km = inf",
            "user.altitude_km = inf",
        ),
    ] {
        let d = small_programme(tag, SMALL, reqs);
        std::fs::write(d.join("mars-orbit-pnt.toml"), mars.replacen(from, to, 1)).unwrap();
        let e = go(&d).unwrap_err().to_string();
        assert!(
            e.contains("RUN-MARS") && e.contains("not a finite number") && e.contains(names),
            "{tag}: {e}"
        );
        assert!(!d.join("out").exists(), "{tag}: nothing was run or written");
    }

    // Inside a variant of the trade study.
    let d = small_programme(
        "nan-trade",
        &format!("{SMALL}[trade]\npath = \"trade-lunar-relay.toml\"\n"),
        reqs,
    );
    let trade = std::fs::read_to_string(fixtures().join("trade-lunar-relay.toml")).unwrap();
    assert!(trade.contains("mask_deg = 10.0"));
    std::fs::write(
        d.join("trade-lunar-relay.toml"),
        trade.replacen("mask_deg = 10.0", "mask_deg = nan", 1),
    )
    .unwrap();
    let e = go(&d).unwrap_err().to_string();
    assert!(
        e.contains("TRADE") && e.contains("variant[0].scenario: mask_deg = NaN"),
        "{e}"
    );
    assert!(!d.join("out").exists());

    // A criterion limit too large for a double.
    let d = small_programme(
        "inf-limit",
        SMALL,
        "id,text,method,criterion,runs\nR1,x,analysis,fom.rms_error_relays_m <= 1e999 m,RUN-MARS\n",
    );
    let e = go(&d).unwrap_err().to_string();
    assert!(
        e.contains("'R1'") && e.contains("is not a finite number"),
        "{e}"
    );
    assert!(!d.join("out").exists());
}

// ------------------------------------------------------------------ 10. ledger

#[test]
fn the_ledger_verifies_and_an_altered_record_does_not() {
    let b = build_worked("ledger");
    let path = b.out_dir.join("ledger.json");
    let ledger = StudyLedger::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(ledger.verify());
    let recs = ledger.records();
    assert_eq!(recs.len(), 3, "request, run, delivered");
    let request = &b.dossier.body.identification.request_sha256;
    assert!(recs.iter().all(|r| &r.request_hash == request));
    assert_eq!(recs[0].run_hash, None);
    assert!(recs[1].run_hash.is_some() && recs[1].delivered_utc.is_none());
    assert_eq!(
        recs[2].run_hash.as_deref(),
        Some(b.dossier.dossier_sha256.as_str())
    );
    assert_eq!(recs[2].delivered_utc.as_deref(), Some(UTC));
    assert_eq!(
        recs[0].prev_hash,
        kshana_pro::study_ledger::GENESIS_PREV_HASH
    );

    // A second issue into the same directory appends three more records.
    build(
        &fixtures().join("programme.toml"),
        &b.out_dir,
        &opts(enterprise()),
    )
    .unwrap();
    let twice = StudyLedger::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(twice.records().len(), 6);
    assert!(twice.verify());
    assert_eq!(&twice.records()[..3], recs);
    assert!(check(&b.out_dir, None).unwrap().passed());

    // Alter one record: the chain no longer verifies, the check says so, and a further
    // build refuses to add to it.
    let mut altered = twice.clone();
    altered.records_mut()[1].run_hash = Some("0".repeat(64));
    assert!(!altered.verify());
    std::fs::write(&path, altered.to_json()).unwrap();
    let report = check(&b.out_dir, None).unwrap();
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.starts_with("ledger.json: the hash chain does not verify")),
        "{:?}",
        report.problems
    );
    let e = build(
        &fixtures().join("programme.toml"),
        &b.out_dir,
        &opts(enterprise()),
    )
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("ledger.json") && e.contains("does not verify"),
        "{e}"
    );
}

/// The worked example copied elsewhere builds the same dossier: nothing depends on
/// where the programme lives.
#[test]
fn the_dossier_does_not_depend_on_where_the_programme_is() {
    let copy = tmp("moved");
    copy_dir(&fixtures(), &copy);
    let built = build(
        &copy.join("programme.toml"),
        &copy.join("out"),
        &opts(enterprise()),
    )
    .unwrap();
    assert_eq!(
        built.dossier.dossier_sha256,
        shared().dossier.dossier_sha256
    );
    let json = std::fs::read_to_string(built.out_dir.join("dossier.json")).unwrap();
    assert!(
        !json.contains(env!("CARGO_MANIFEST_DIR")) && !json.contains(&*copy.to_string_lossy()),
        "the dossier holds no absolute path"
    );
}

/// The summary's split of the verdicts by label is the split of the matrix cells, and
/// of the requirements tooling's own matrix: no PARTNER verdict is counted as MODELLED.
#[test]
fn the_summary_counts_each_label_as_the_matrix_carries_it() {
    let b = shared();
    let d = &b.dossier;
    let cells_with = |label: &str| {
        d.body
            .matrix
            .iter()
            .flat_map(|r| &r.cells)
            .filter(|c| c.basis == label)
            .count()
    };
    let by = &d.body.summary.verdicts_by_basis;
    assert_eq!(
        (by.validated, by.modelled, by.partner),
        (
            cells_with("VALIDATED"),
            cells_with("MODELLED"),
            cells_with("PARTNER")
        )
    );
    // The worked example: the spectrum and campaign runs rely on a PARTNER capability.
    assert_eq!((by.validated, by.modelled, by.partner), (0, 7, 4));
    let trace = Trace::load(&b.out_dir.join("trace.toml")).unwrap();
    let v = vcrm(&trace);
    for label in ["VALIDATED", "MODELLED", "PARTNER"] {
        let tooling = v
            .rows
            .iter()
            .flat_map(|r| &r.cells)
            .filter(|c| c.basis.label == label)
            .count();
        assert_eq!(cells_with(label), tooling, "{label}");
    }
    for r in &d.body.matrix {
        for c in &r.cells {
            let expected = match c.run.as_str() {
                "RUN-SPECTRUM" | "RUN-CAMPAIGN" => "PARTNER",
                _ => "MODELLED",
            };
            assert_eq!(c.basis, expected, "{} on {}", r.requirement, c.run);
        }
    }
    // The page states the split.
    let html = std::fs::read_to_string(b.out_dir.join("dossier.html")).unwrap();
    assert!(html.contains("7 on a MODELLED basis") && html.contains("4 on a PARTNER basis"));
}

/// Text a programme supplies (title, reference, requirement text, descriptions, chart
/// titles, sign-off rows) is shown as text: markup in it is escaped in the HTML page, so
/// a delivered dossier cannot carry a script or fetch anything.
#[test]
fn markup_in_programme_text_is_escaped_in_the_html() {
    let programme = "title = \"T <script>alert(1)</script> & \\\"q\\\" (a) )(\"\n\
        reference = \"<b>ref</b>\"\n\
        classification = \"<i>Restricted</i>\"\n\
        requirements = \"requirements.csv\"\n\
        [[scenario]]\n\
        id = \"RUN-MARS\"\n\
        path = \"mars-orbit-pnt.toml\"\n\
        description = \"<iframe src=http://example.invalid></iframe>\"\n\
        [[scenario.chart]]\n\
        title = \"</svg><script>alert(2)</script>\"\n\
        x = \"epochs[].t_s\"\n\
        y = [\"epochs[].error_relays_m\"]\n\
        [[sign_off]]\n\
        role = \"<script>alert(3)</script>\"\n\
        name = \"<img src=http://example.invalid/n.png>\"\n";
    let requirements = "id,text,method,criterion,runs\n\
        R<1>,\"<img src=\"\"http://example.invalid/x.png\"\"> & <script>alert(4)</script>\",analysis,fom.median_pdop <= 6,RUN-MARS\n";
    let d = small_programme("markup", programme, requirements);
    let built = build(
        &d.join("programme.toml"),
        &d.join("out"),
        &opts(enterprise()),
    )
    .expect("text with markup builds");
    assert_eq!(row(&built.dossier, "R<1>").verdict, "met");
    for face in ["dossier.html", "evidence-pack.html"] {
        let html = std::fs::read_to_string(built.out_dir.join(face)).unwrap();
        for raw in [
            "<script",
            "<iframe",
            "<img src=",
            "<b>ref</b>",
            "<i>Restricted",
        ] {
            assert!(!html.contains(raw), "{face} carries {raw} unescaped");
        }
    }
    let html = std::fs::read_to_string(built.out_dir.join("dossier.html")).unwrap();
    for escaped in [
        "T &lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;q&quot; (a) )(",
        "&lt;b&gt;ref&lt;/b&gt;",
        "&lt;iframe src=http://example.invalid&gt;&lt;/iframe&gt;",
        "&lt;/svg&gt;&lt;script&gt;alert(2)&lt;/script&gt;",
        "&lt;script&gt;alert(3)&lt;/script&gt;",
        "&lt;script&gt;alert(4)&lt;/script&gt;",
        "<b>R&lt;1&gt;</b>",
    ] {
        assert!(html.contains(escaped), "the page shows {escaped}");
    }
    // The PDF is still a PDF with every page numbered, and the dossier checks.
    let pdf = std::fs::read(built.out_dir.join("dossier.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(
        count(&pdf, format!("page {0} of {0}", built.pdf_pages).as_bytes()),
        1
    );
    assert!(check(&built.out_dir, None).unwrap().passed());
}

/// A ledger that verifies but belongs to another dossier is not accepted: its last
/// record must carry the hash of the dossier it lies beside.
#[test]
fn a_ledger_from_another_dossier_fails_the_check() {
    let go = |tag: &str, limit: &str| {
        let d = small_programme(
            tag,
            SMALL,
            &format!(
                "id,text,method,criterion,runs\nR1,x,analysis,fom.median_pdop <= {limit},RUN-MARS\n"
            ),
        );
        build(
            &d.join("programme.toml"),
            &d.join("out"),
            &opts(enterprise()),
        )
        .unwrap()
    };
    let (a, b) = (go("ledger-a", "6"), go("ledger-b", "7"));
    assert_ne!(a.dossier.dossier_sha256, b.dossier.dossier_sha256);
    assert!(check(&a.out_dir, None).unwrap().passed());
    std::fs::copy(b.out_dir.join("ledger.json"), a.out_dir.join("ledger.json")).unwrap();
    let ledger =
        StudyLedger::from_json(&std::fs::read_to_string(a.out_dir.join("ledger.json")).unwrap())
            .unwrap();
    assert!(
        ledger.verify(),
        "the other dossier's ledger is itself intact"
    );
    let report = check(&a.out_dir, None).unwrap();
    assert_eq!(
        report.problems,
        vec!["ledger.json: the last record does not carry this dossier's hash".to_string()]
    );
}
