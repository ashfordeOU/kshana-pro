//! The lab-campaign loader (`kshana_pro::lab_campaign`) on the synthetic fixture.
//!
//! The fixture (`tests/fixtures/lab_campaign_synthetic/basic/`, written by `make_fixture.py`
//! there) is hand-made and shaped like the engine's 0.34.0 lab-replay output. Every refusal is
//! exercised on a scratch copy, so the committed fixture is never changed.

use kshana_pro::lab_campaign::hash::{digest, sha256_hex};
use kshana_pro::lab_campaign::{load, DataClass, LoadError};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lab_campaign_synthetic/basic")
}

static N: AtomicUsize = AtomicUsize::new(0);

/// A scratch copy of the fixture.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "kshana-pro-labcampaign-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("cells")).unwrap();
    let src = fixture();
    for f in ["campaign.json", "DIGEST"] {
        fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    for e in fs::read_dir(src.join("cells")).unwrap().flatten() {
        fs::copy(e.path(), dir.join("cells").join(e.file_name())).unwrap();
    }
    dir
}

fn cell_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir.join("cells"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

/// Rewrite DIGEST from the cells present (as the engine's `campaign report` would).
fn redigest(dir: &Path) {
    let pairs: Vec<(String, String)> = cell_files(dir)
        .iter()
        .map(|p| {
            let key = p.file_stem().unwrap().to_string_lossy().into_owned();
            (key, sha256_hex(&fs::read(p).unwrap()))
        })
        .collect();
    let d = digest(pairs.iter().map(|(k, s)| (k.as_str(), s.as_str())));
    fs::write(dir.join("DIGEST"), format!("{d}\n")).unwrap();
}

fn edit_json(path: &Path, f: impl FnOnce(&mut serde_json::Value)) {
    let mut v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    f(&mut v);
    fs::write(path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

#[test]
fn the_synthetic_fixture_loads_complete_and_verified() {
    let c = load(&fixture()).expect("fixture loads");
    assert!(c.is_complete());
    assert_eq!(c.cells.len(), 12, "2 recordings x 2 chains x 3 designs");
    assert_eq!(c.data_class(), Some(DataClass::Synthetic));
    assert_eq!(c.manifest.name, "synthetic-basic");
    let stated = fs::read_to_string(fixture().join("DIGEST")).unwrap();
    assert_eq!(c.digest, stated.trim());
    // The campaign's own order is kept.
    let keys: Vec<&str> = c.cells.iter().map(|x| x.key.as_str()).collect();
    let listed: Vec<&str> = c.manifest.cells.iter().map(String::as_str).collect();
    assert_eq!(keys, listed);
    // The spoofing/outage-style event has no reference curve; the interference event has one.
    let has_curve = c
        .cells
        .iter()
        .flat_map(|x| &x.satellites)
        .flat_map(|s| &s.events)
        .any(|e| e.modelled.is_some());
    assert!(has_curve);
}

#[test]
fn a_changed_cell_breaks_the_digest() {
    let d = scratch();
    let f = &cell_files(&d)[0];
    edit_json(f, |v| {
        v["satellites"][0]["whole_run"]["availability"] = 0.5.into()
    });
    match load(&d) {
        Err(LoadError::DigestMismatch { .. }) => {}
        other => panic!("expected a DIGEST mismatch, got {other:?}"),
    }
}

#[test]
fn a_missing_digest_is_refused() {
    let d = scratch();
    fs::remove_file(d.join("DIGEST")).unwrap();
    assert_eq!(load(&d).unwrap_err(), LoadError::MissingDigest);
}

#[test]
fn a_missing_cell_is_reported_as_incomplete() {
    let d = scratch();
    let gone = cell_files(&d)[3].clone();
    fs::remove_file(&gone).unwrap();
    redigest(&d);
    let c = load(&d).expect("incomplete campaign still loads");
    assert!(!c.is_complete());
    assert_eq!(
        c.missing,
        vec![gone.file_stem().unwrap().to_string_lossy().into_owned()]
    );
    assert_eq!(c.cells.len(), 11);
}

#[test]
fn an_unknown_schema_version_is_refused() {
    let d = scratch();
    edit_json(&d.join("campaign.json"), |v| {
        v["schema"] = "kshana.campaign/2".into()
    });
    assert!(matches!(load(&d), Err(LoadError::UnknownSchema { .. })));
}

#[test]
fn an_unknown_field_is_refused() {
    let d = scratch();
    edit_json(&cell_files(&d)[0], |v| v["surprise"] = 1.into());
    assert!(matches!(load(&d), Err(LoadError::Parse { .. })));
}

#[test]
fn a_cell_whose_key_does_not_re_derive_is_refused() {
    let d = scratch();
    edit_json(&cell_files(&d)[0], |v| v["design"]["hash"] = "00".into());
    assert!(matches!(load(&d), Err(LoadError::KeyMismatch { .. })));
}

#[test]
fn a_different_engine_version_is_refused() {
    let d = scratch();
    edit_json(&d.join("campaign.json"), |v| {
        v["engine_version"] = "0.35.0".into()
    });
    assert!(matches!(load(&d), Err(LoadError::EngineVersion { .. })));
}

#[test]
fn a_reference_curve_must_be_labelled_modelled() {
    let d = scratch();
    let target = cell_files(&d)
        .into_iter()
        .find(|p| fs::read_to_string(p).unwrap().contains("\"MODELLED\""))
        .unwrap();
    edit_json(&target, |v| {
        v["satellites"][0]["events"][0]["modelled"]["label"] = "VALIDATED".into()
    });
    assert!(matches!(load(&d), Err(LoadError::ModelledLabel { .. })));
}

// ── L2: aggregation over the synthetic fixture ─────────────────────────────────────────

use kshana_pro::lab_campaign::aggregate::{aggregate, rank, Op};

#[test]
fn aggregation_keeps_campaign_order_and_one_row_per_chain_and_design() {
    let a = aggregate(&load(&fixture()).unwrap());
    assert_eq!(a.frontends, vec!["raw", "notch+2bit"]);
    assert_eq!(a.designs, vec!["baseline", "narrow-pll", "wide-pll"]);
    assert_eq!(a.scorecard.len(), 6);
    assert_eq!(a.events.len(), 12, "2 events x 2 chains x 3 designs");
    assert_eq!(
        a.curves.len(),
        6,
        "only the interference event has a J/S curve"
    );
    let first = &a.scorecard[0];
    assert_eq!(
        (first.frontend.as_str(), first.design.as_str()),
        ("raw", "baseline")
    );
    assert_eq!((first.cells, first.satellites), (2, 4));
}

#[test]
fn scorecard_values_are_declared_aggregates_of_cell_fields() {
    let a = aggregate(&load(&fixture()).unwrap());
    let row = |fe: &str, de: &str| {
        a.scorecard
            .iter()
            .find(|r| r.frontend == fe && r.design == de)
            .unwrap()
    };
    // Whole-run availability per satellite in the fixture: 0.9, 0.9, 0.9, 1.0 -> median 0.9.
    let b = row("raw", "baseline");
    assert_eq!(b.availability.value, Some(0.9));
    assert_eq!((b.availability.op, b.availability.n), (Op::Median, 4));
    assert_eq!(b.losses.value, Some(3.0));
    assert_eq!(b.false_lock_per_locked_hour.value, Some(0.0));
    assert_eq!(b.false_lock_per_locked_hour.op, Op::RatioOfSums);
    assert_eq!(row("raw", "narrow-pll").availability.value, Some(0.88));
}

#[test]
fn event_rows_stay_per_event_and_count_bars() {
    let a = aggregate(&load(&fixture()).unwrap());
    let ev = |id: &str, fe: &str, de: &str| {
        a.events
            .iter()
            .find(|r| r.event_id == id && r.frontend == fe && r.design == de)
            .unwrap()
    };
    let r = ev("ramp-1", "raw", "baseline");
    assert_eq!((r.satellites, r.held), (2, 0));
    assert_eq!(r.time_to_loss_s.value, Some(21.25));
    assert_eq!(r.js_at_loss_db.value, Some(25.25));
    assert_eq!(r.reacq_time_s.value, Some(3.5));
    assert_eq!(r.bars.get("max_reacq_s"), Some(&(2, 2)));
    assert_eq!(
        ev("ramp-1", "raw", "narrow-pll").bars.get("max_reacq_s"),
        Some(&(0, 2))
    );
    // The outage event affects one satellite and has no J/S.
    let g = ev("gap-1", "raw", "baseline");
    assert_eq!(g.satellites, 1);
    assert_eq!(g.js_at_loss_db.value, None);
    assert!(g.bars.is_empty());
}

#[test]
fn curves_keep_empty_bins_visible_and_carry_the_reference() {
    let a = aggregate(&load(&fixture()).unwrap());
    let c = &a.curves[0];
    assert_eq!(
        c.bins.iter().map(|b| b.js_db).collect::<Vec<_>>(),
        vec![20.0, 25.0, 30.0]
    );
    // The 30 dB bin has no locked epochs: no measured value, but the MODELLED value stays.
    assert_eq!(c.bins[2].n.value, Some(0.0));
    assert_eq!(c.bins[2].measured_degradation_db.value, None);
    assert_eq!(c.bins[2].modelled_degradation_db.value, Some(7.6));
    assert_eq!(c.q, Some(2.0));
}

#[test]
fn ranking_by_reacquisition_time_on_the_fixture() {
    let a = aggregate(&load(&fixture()).unwrap());
    let vals: Vec<(String, Option<f64>)> = a
        .events
        .iter()
        .filter(|r| r.event_id == "ramp-1" && r.frontend == "raw")
        .map(|r| (r.design.clone(), r.reacq_time_s.value))
        .collect();
    let r = rank(&vals, false);
    assert_eq!(r[0], ("wide-pll".to_string(), 1));
    assert_eq!(r[2], ("narrow-pll".to_string(), 3));
}
