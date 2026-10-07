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
