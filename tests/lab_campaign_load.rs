//! The lab-campaign loader and aggregation (`kshana_pro::lab_campaign`) on the synthetic fixture.
//!
//! The fixture (`tests/fixtures/lab_campaign_synthetic/basic/`) is real output of the engine's
//! `kshana iq campaign` (branch `feat/034-campaign`, see that folder's README) on two
//! Kshana-generated recordings; `make_fixture.py` there regenerates it. Every refusal is
//! exercised on a scratch copy, so the committed fixture is never changed.

use kshana_pro::lab_campaign::aggregate::{aggregate, median, rank, Op};
use kshana_pro::lab_campaign::hash::{digest, sha256_hex};
use kshana_pro::lab_campaign::{load, LoadError};
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
    for f in ["campaign.json", "scorecard.json", "DIGEST"] {
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

fn edit_json(path: &Path, f: impl FnOnce(&mut serde_json::Value)) {
    let mut v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    f(&mut v);
    fs::write(path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

#[test]
fn the_engine_fixture_loads_complete_and_verified() {
    let c = load(&fixture()).expect("fixture loads");
    assert!(c.is_complete());
    assert_eq!(c.cells.len(), 8, "2 recordings x 2 chains x 2 designs");
    assert_eq!(c.manifest.schema, "kshana.campaign-plan/1");
    assert!(c.manifest.name.starts_with("synthetic-"));
    let stated = fs::read_to_string(fixture().join("DIGEST")).unwrap();
    assert_eq!(
        c.digest,
        stated.trim(),
        "the engine's DIGEST re-derives in the overlay"
    );
    assert_eq!(c.scorecard.digest.as_deref(), Some(c.digest.as_str()));
    // The campaign's own order is kept.
    let keys: Vec<&str> = c.cells.iter().map(|x| x.key.as_str()).collect();
    let planned: Vec<&str> = c.manifest.cells.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(keys, planned);
    // The interference event carries the MODELLED reference; the outage does not.
    let events: Vec<_> = c
        .cells
        .iter()
        .flat_map(|x| &x.satellites)
        .flat_map(|s| &s.events)
        .collect();
    assert!(events
        .iter()
        .any(|e| e.kind == "interference" && e.modelled.is_some()));
    assert!(events
        .iter()
        .any(|e| e.kind == "outage" && e.modelled.is_none()));
}

#[test]
fn the_digest_byte_form_is_the_engines() {
    // Recompute independently of the loader: sorted "<key> <sha256(file)>\n" lines.
    let d = fixture();
    let mut lines: Vec<String> = cell_files(&d)
        .iter()
        .map(|p| {
            format!(
                "{} {}\n",
                p.file_stem().unwrap().to_string_lossy(),
                sha256_hex(&fs::read(p).unwrap())
            )
        })
        .collect();
    lines.sort();
    assert_eq!(
        sha256_hex(lines.concat().as_bytes()),
        fs::read_to_string(d.join("DIGEST")).unwrap().trim()
    );
}

#[test]
fn a_changed_cell_breaks_the_digest() {
    let d = scratch();
    edit_json(&cell_files(&d)[0], |v| v["samples_processed"] = 1.into());
    assert!(matches!(load(&d), Err(LoadError::DigestMismatch { .. })));
}

#[test]
fn a_complete_campaign_without_its_digest_is_refused() {
    let d = scratch();
    fs::remove_file(d.join("DIGEST")).unwrap();
    assert_eq!(load(&d).unwrap_err(), LoadError::MissingDigest);
}

#[test]
fn a_missing_cell_is_reported_as_incomplete() {
    let d = scratch();
    let gone = cell_files(&d)[3].clone();
    let key = gone.file_stem().unwrap().to_string_lossy().into_owned();
    fs::remove_file(&gone).unwrap();
    // As the engine leaves an incomplete campaign: no DIGEST, a scorecard of the cells present.
    fs::remove_file(d.join("DIGEST")).unwrap();
    edit_json(&d.join("scorecard.json"), |v| {
        v["complete"] = false.into();
        v["digest"] = serde_json::Value::Null;
        let rows = v["rows"].as_array_mut().unwrap();
        rows.retain(|r| r["cell"] != key.as_str());
    });
    let c = load(&d).expect("an incomplete campaign still loads");
    assert!(!c.is_complete());
    assert_eq!(c.missing, vec![key]);
    assert_eq!(c.cells.len(), 7);
    // The digest of the cells present is still verified in the overlay.
    let pairs: Vec<(String, String)> = cell_files(&d)
        .iter()
        .map(|p| {
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                sha256_hex(&fs::read(p).unwrap()),
            )
        })
        .collect();
    assert_eq!(
        c.digest,
        digest(pairs.iter().map(|(k, s)| (k.as_str(), s.as_str())))
    );
}

#[test]
fn an_unknown_schema_version_is_refused() {
    let d = scratch();
    edit_json(&d.join("campaign.json"), |v| {
        v["schema"] = "kshana.campaign-plan/2".into()
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
    edit_json(&cell_files(&d)[0], |v| {
        v["design"]["name"] = "renamed".into()
    });
    assert!(matches!(load(&d), Err(LoadError::KeyMismatch { .. })));
}

#[test]
fn a_cell_of_another_data_class_is_refused() {
    // The data class is part of the cell key, so a relabelled cell no longer re-derives.
    let d = scratch();
    edit_json(&cell_files(&d)[0], |v| {
        v["data_class"] = "client-confidential".into()
    });
    assert!(matches!(load(&d), Err(LoadError::KeyMismatch { .. })));
    // A plan relabelled without its cells is refused too.
    let d = scratch();
    edit_json(&d.join("campaign.json"), |v| {
        v["data_class"] = "client-confidential".into()
    });
    assert!(matches!(load(&d), Err(LoadError::NotInCampaign { .. })));
}

#[test]
fn an_unknown_lock_source_is_refused() {
    let d = scratch();
    edit_json(&cell_files(&d)[0], |v| {
        v["lock_source"] = "lock-indicators".into()
    });
    assert!(matches!(load(&d), Err(LoadError::UnknownLockSource { .. })));
}

/// Point the first cell at an epoch file holding `bytes`, stating `stated_sha` and the path
/// `rel`, then re-seal the campaign (DIGEST and scorecard digest) so only the epoch check can
/// fail.
fn with_epochs(d: &Path, rel: &str, bytes: &[u8], stated_sha: Option<&str>) {
    fs::create_dir_all(d.join("epochs")).unwrap();
    fs::write(d.join("epochs/e.jsonl"), bytes).unwrap();
    fs::write(d.join("epochs/e.events.jsonl"), b"{}\n").unwrap();
    let sha = stated_sha.map_or_else(|| sha256_hex(bytes), str::to_owned);
    edit_json(&cell_files(d)[0], |v| {
        v["epochs"] = serde_json::json!({
            "format": "jsonl",
            "path": rel,
            "sha256": sha,
            "bytes": bytes.len(),
            "events_path": "epochs/e.events.jsonl",
            "events_sha256": sha256_hex(b"{}\n"),
        })
    });
    let pairs: Vec<(String, String)> = cell_files(d)
        .iter()
        .map(|p| {
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                sha256_hex(&fs::read(p).unwrap()),
            )
        })
        .collect();
    let dg = digest(pairs.iter().map(|(k, s)| (k.as_str(), s.as_str())));
    fs::write(d.join("DIGEST"), format!("{dg}\n")).unwrap();
    edit_json(&d.join("scorecard.json"), |v| {
        v["digest"] = dg.clone().into()
    });
}

#[test]
fn epoch_files_a_cell_names_are_verified() {
    let d = scratch();
    with_epochs(&d, "epochs/e.jsonl", b"epoch\n", None);
    load(&d).expect("matching epoch files load");

    let d = scratch();
    with_epochs(&d, "epochs/e.jsonl", b"epoch\n", Some(&"0".repeat(64)));
    assert!(matches!(load(&d), Err(LoadError::Epochs { .. })));

    let d = scratch();
    with_epochs(&d, "../e.jsonl", b"epoch\n", None);
    assert!(matches!(load(&d), Err(LoadError::Epochs { .. })));

    let d = scratch();
    with_epochs(&d, "epochs/e.jsonl", b"epoch\n", None);
    fs::remove_file(d.join("epochs/e.jsonl")).unwrap();
    assert!(matches!(load(&d), Err(LoadError::Epochs { .. })));
}

#[test]
fn a_different_engine_version_is_refused() {
    let d = scratch();
    edit_json(&d.join("campaign.json"), |v| {
        v["engine_version"] = "9.9.9".into()
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
        for s in v["satellites"].as_array_mut().unwrap() {
            for e in s["events"].as_array_mut().unwrap() {
                if e["modelled"].is_object() {
                    e["modelled"]["label"] = "VALIDATED".into();
                }
            }
        }
    });
    assert!(matches!(load(&d), Err(LoadError::ModelledLabel { .. })));
}

#[test]
fn the_scorecard_must_be_present_and_belong_to_the_campaign() {
    let d = scratch();
    fs::remove_file(d.join("scorecard.json")).unwrap();
    assert_eq!(load(&d).unwrap_err(), LoadError::MissingScorecard);
    let d = scratch();
    edit_json(&d.join("scorecard.json"), |v| {
        v["campaign"] = "another".into()
    });
    assert!(matches!(load(&d), Err(LoadError::ScorecardMismatch(_))));
    let d = scratch();
    edit_json(&d.join("scorecard.json"), |v| {
        v["data_class"] = "client-confidential".into()
    });
    assert!(matches!(load(&d), Err(LoadError::ScorecardMismatch(_))));
}

// ── aggregation over the engine fixture ────────────────────────────────────────────────

#[test]
fn aggregation_keeps_campaign_order_and_one_row_per_chain_and_design() {
    let a = aggregate(&load(&fixture()).unwrap());
    assert_eq!(a.frontends, vec!["raw", "agc-3bit"]);
    assert_eq!(a.designs, vec!["baseline", "narrow-pll"]);
    assert_eq!(a.scorecard.len(), 4);
    assert_eq!(a.events.len(), 8, "2 events x 2 chains x 2 designs");
    assert_eq!(
        a.curves.len(),
        4,
        "only the interference event has a J/S curve"
    );
}

#[test]
fn scorecard_values_are_declared_aggregates_of_cell_fields() {
    let c = load(&fixture()).unwrap();
    let a = aggregate(&c);
    for row in &a.scorecard {
        let sats: Vec<_> = c
            .cells
            .iter()
            .filter(|x| x.frontend.name == row.frontend && x.design.name == row.design)
            .flat_map(|x| &x.satellites)
            .collect();
        let expect = median(sats.iter().filter_map(|s| s.whole_run.availability));
        assert_eq!(row.availability.value, expect.value);
        assert_eq!(
            (row.availability.op, row.availability.n),
            (Op::Median, sats.len())
        );
        let losses: u64 = sats.iter().map(|s| s.whole_run.loss_count).sum();
        assert_eq!(row.losses.value, Some(losses as f64));
    }
}

#[test]
fn verdicts_are_the_engines_counted_not_recomputed() {
    let c = load(&fixture()).unwrap();
    let a = aggregate(&c);
    let rows = &c.scorecard.rows;
    let (p, e) = rows
        .iter()
        .fold((0, 0), |(p, e), r| match r.verdict.as_str() {
            "PASS" => (p + 1, e + 1),
            "FAIL" => (p, e + 1),
            _ => (p, e),
        });
    let (ap, ae) = a
        .scorecard
        .iter()
        .map(|r| r.verdicts)
        .chain(a.events.iter().map(|r| r.verdicts))
        .fold((0, 0), |(p, e), (bp, be)| (p + bp, e + be));
    assert_eq!((ap, ae), (p, e));
    assert!(e > 0, "the fixture campaign sets bars");
}

#[test]
fn curves_carry_the_reference_and_its_q() {
    let a = aggregate(&load(&fixture()).unwrap());
    for c in &a.curves {
        assert!(!c.bins.is_empty());
        assert!(c.bins.windows(2).all(|w| w[0].js_db < w[1].js_db));
        assert!(c.q.is_some() && c.formula.is_some());
        assert!(c
            .bins
            .iter()
            .all(|b| b.modelled_degradation_db.value.is_some()));
    }
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
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].1, 1);
    let best = vals.iter().find(|(n, _)| *n == r[0].0).unwrap().1.unwrap();
    assert!(vals.iter().all(|(_, v)| v.is_none_or(|v| v >= best)));
}
