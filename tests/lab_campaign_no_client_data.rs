//! Lab-campaign data hygiene guard (export-control item N6).
//!
//! The Pro lab campaign report renders the engine's `kshana iq campaign` output. Until
//! export-control counsel answers item N6
//! (`commercial/export-control/CLASSIFICATION-REQUEST-PACK.md` §2.1a), no client
//! recording, campaign result, score or test-condition file may enter this repository.
//! Only synthetic fixtures are allowed, and only under [`FIXTURE_DIR`].
//!
//! This test walks the working tree and fails on:
//! - any campaign, campaign-cell or test-condition document (recognised by its `schema`
//!   string) outside [`FIXTURE_DIR`];
//! - any SigMF recording anywhere;
//! - any fixture campaign whose name does not start with `synthetic-`, or that states a
//!   `data_class` other than `synthetic`.

use std::fs;
use std::path::{Path, PathBuf};

const FIXTURE_DIR: &str = "tests/fixtures/lab_campaign_synthetic";

/// Schema prefixes of the engine's lab-replay documents (public `docs/design/LAB-CAMPAIGN.md`).
const SCHEMAS: &[&str] = &[
    "kshana.campaign/",
    "kshana.campaign-plan/",
    "kshana.campaign-cell/",
    "kshana.campaign-scorecard/",
    "kshana.test-conditions/",
];

/// Directories never scanned: build output and VCS metadata.
const SKIP_DIRS: &[&str] = &["target", ".git", "node_modules"];

/// Files larger than this are not parsed (a campaign document is small); they are still
/// checked by extension.
const MAX_SCAN_BYTES: u64 = 8 << 20;

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        match e.file_type() {
            Ok(t) if t.is_dir() => {
                if !SKIP_DIRS.contains(&name.as_ref()) {
                    walk(&p, out);
                }
            }
            Ok(t) if t.is_file() => out.push(p),
            _ => {}
        }
    }
}

/// The `schema` value of a JSON or TOML document, if it names a lab-replay schema.
fn lab_schema(path: &Path, text: &str) -> Option<String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let schema = match ext {
        "json" => serde_json::from_str::<serde_json::Value>(text)
            .ok()?
            .get("schema")?
            .as_str()?
            .to_owned(),
        "toml" => toml::from_str::<toml::Value>(text)
            .ok()?
            .get("schema")?
            .as_str()?
            .to_owned(),
        _ => return None,
    };
    SCHEMAS
        .iter()
        .any(|s| schema.starts_with(s))
        .then_some(schema)
}

#[test]
fn no_client_campaign_data_outside_the_synthetic_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = root.join(FIXTURE_DIR);
    let mut files = Vec::new();
    walk(root, &mut files);

    let mut violations = Vec::new();
    let mut fixture_campaigns = 0;
    for p in &files {
        let rel = p.strip_prefix(root).unwrap_or(p).display().to_string();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "sigmf-data" || ext == "sigmf-meta" || rel.ends_with(".sigmf-data") {
            violations.push(format!("{rel}: SigMF recording in the repository"));
            continue;
        }
        if fs::metadata(p).map(|m| m.len()).unwrap_or(0) > MAX_SCAN_BYTES {
            continue;
        }
        let Ok(text) = fs::read_to_string(p) else {
            continue;
        };
        let Some(schema) = lab_schema(p, &text) else {
            continue;
        };
        if !p.starts_with(&fixtures) {
            violations.push(format!(
                "{rel}: `{schema}` document outside {FIXTURE_DIR} (client data must not be committed; N6)"
            ));
            continue;
        }
        if schema.starts_with("kshana.campaign-plan/") || schema.starts_with("kshana.campaign/") {
            fixture_campaigns += 1;
            let v: serde_json::Value = if ext == "json" {
                serde_json::from_str(&text).unwrap()
            } else {
                serde_json::to_value(toml::from_str::<toml::Value>(&text).unwrap()).unwrap()
            };
            let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if !name.starts_with("synthetic-") {
                violations.push(format!(
                    "{rel}: fixture campaign name {name:?} must start with `synthetic-`"
                ));
            }
            // An engine that carries a data class must say synthetic here.
            if let Some(dc) = v.get("data_class") {
                if dc.as_str() != Some("synthetic") {
                    violations.push(format!(
                        "{rel}: fixture campaign data_class must be \"synthetic\""
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "lab-campaign data hygiene (N6):\n  {}",
        violations.join("\n  ")
    );
    assert!(
        fixture_campaigns > 0,
        "expected at least one synthetic fixture campaign under {FIXTURE_DIR}"
    );
}

#[test]
fn a_planted_client_cell_would_be_caught() {
    // The guard's own detector, exercised on an in-memory document so the test never writes
    // a non-synthetic file into the tree.
    let cell = r#"{"schema":"kshana.campaign-cell/1","key":"00"}"#;
    assert_eq!(
        lab_schema(Path::new("x/cells/00.json"), cell).as_deref(),
        Some("kshana.campaign-cell/1")
    );
    let cond = "schema = \"kshana.test-conditions/1\"\n";
    assert!(lab_schema(Path::new("lab/run.toml"), cond).is_some());
    assert!(lab_schema(Path::new("Cargo.toml"), "[package]\nname = \"x\"\n").is_none());
}
