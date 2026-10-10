//! Lab-campaign data hygiene guard (export-control item N6).
//!
//! The Pro lab campaign report renders the engine's `kshana iq campaign` output. Until
//! export-control counsel answers item N6
//! (`commercial/export-control/CLASSIFICATION-REQUEST-PACK.md` §2.1a), no client
//! recording, campaign result, score or test-condition file may enter this repository.
//! Only synthetic fixtures are allowed, and only under [`FIXTURE_DIR`].
//!
//! This test walks the working tree and fails on:
//! - any campaign, cell, scorecard or test-condition document (recognised by its `schema`
//!   string, found in the first and last [`SNIFF_BYTES`] bytes whatever the file size) outside
//!   [`FIXTURE_DIR`];
//! - any campaign output by name outside [`FIXTURE_DIR`]: an `epochs/` directory, `*.jsonl`,
//!   `events.jsonl`, `runs.jsonl`, `hashes.json`, `campaign.json`, `scorecard.csv`, `scorecard.json`, and any
//!   file inside a directory that holds `campaign.json`, `cells/`, `runs.jsonl` or `hashes.json`
//!   (a campaign output directory, so its `report.html` and `*.csv` are caught too);
//! - any SigMF recording and any raw IQ file (`.bin`, `.cf32`, `.ci16`, ...) anywhere;
//! - inside [`FIXTURE_DIR`]: any raw IQ or `.bin` file, any file above [`MAX_FIXTURE_BYTES`],
//!   and any unknown file that is not plain text;
//! - any fixture campaign whose name does not start with `synthetic-`, or whose `data_class`
//!   is not `synthetic`. The fixture must carry one, so the gate cannot go dormant.

use std::fs;
use std::io::Read;
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

/// Bytes read from the start of every candidate document to find its `schema`, however large
/// the file is.
const SNIFF_BYTES: usize = 4096;

/// A fixture file larger than this is not a fixture.
const MAX_FIXTURE_BYTES: u64 = 1 << 20;

/// Raw IQ and recording extensions: never allowed, anywhere.
const IQ_EXTS: &[&str] = &[
    "bin",
    "cf32",
    "cfile",
    "ci16",
    "ci8",
    "cs8",
    "cs16",
    "cu8",
    "iq",
    "sdrx",
    "sigmf-data",
    "sigmf-meta",
    "sigmf",
];

/// File names the engine's campaign writes (besides `cells/<key>.json`).
const OUTPUT_NAMES: &[&str] = &[
    "events.jsonl",
    "runs.jsonl",
    "hashes.json",
    "campaign.json",
    "scorecard.csv",
    "scorecard.json",
];

/// Quoted absolute-path prefixes that never belong in a fixture.
const LOCAL_PATH_PREFIXES: &[&str] = &[
    "\"/home/",
    "\"/tmp/",
    "\"/root/",
    "\"/Users/",
    "\"/workspace",
    "\"/var/",
    "\"/mnt/",
    "\"C:\\",
];

/// Marker files and directories of a campaign output directory.
const OUTPUT_DIR_MARKERS: &[&str] = &["campaign.json", "cells", "runs.jsonl", "hashes.json"];

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

fn ext(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// What the guard reads of a file, whatever its size: the first [`SNIFF_BYTES`] bytes and, for
/// a longer file, the last [`SNIFF_BYTES`] too (the engine writes keys sorted, so `schema` of a
/// campaign plan sits after its recordings and cells). A file that fits is read whole.
fn head(path: &Path) -> Vec<u8> {
    use std::io::{Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut buf = Vec::new();
    if len <= 2 * SNIFF_BYTES as u64 {
        let _ = f.read_to_end(&mut buf);
        return buf;
    }
    let mut first = vec![0u8; SNIFF_BYTES];
    let n = f.read(&mut first).unwrap_or(0);
    first.truncate(n);
    buf.extend_from_slice(&first);
    buf.push(b'\n');
    if f.seek(SeekFrom::Start(len - SNIFF_BYTES as u64)).is_ok() {
        let mut last = vec![0u8; SNIFF_BYTES];
        let n = f.read(&mut last).unwrap_or(0);
        buf.extend_from_slice(&last[..n]);
    }
    buf
}

/// The lab-replay schema a JSON, JSONL or TOML document names in its first bytes, if any.
fn lab_schema(path: &Path, head: &[u8]) -> Option<String> {
    if !matches!(ext(path).as_str(), "json" | "jsonl" | "toml") {
        return None;
    }
    let text = String::from_utf8_lossy(head);
    if !text.contains("schema") {
        return None;
    }
    SCHEMAS
        .iter()
        .find(|s| text.contains(&format!("\"{s}")))
        .map(|s| (*s).to_owned())
}

/// Whether `rel` (a path relative to the repository root, `/`-separated) names a campaign
/// output by itself.
fn is_output_by_name(rel: &str) -> bool {
    let p = Path::new(rel);
    let base = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    p.components().any(|c| c.as_os_str() == "epochs")
        || OUTPUT_NAMES.contains(&base)
        || ext(p) == "jsonl"
}

/// Judge one file. `rel` is relative to the repository root, `in_fixture` says whether it lies
/// under [`FIXTURE_DIR`], `size` is its length, `head` its first bytes and `output_dir` whether
/// its directory holds campaign-output markers. Returns the violations.
fn judge(rel: &str, in_fixture: bool, size: u64, head: &[u8], output_dir: bool) -> Vec<String> {
    let p = Path::new(rel);
    let e = ext(p);
    if IQ_EXTS.contains(&e.as_str()) {
        return vec![format!(
            "{rel}: raw IQ or SigMF recording in the repository (N6)"
        )];
    }
    if in_fixture {
        let mut v = Vec::new();
        if size > MAX_FIXTURE_BYTES {
            v.push(format!(
                "{rel}: {size} bytes in the synthetic fixtures (limit {MAX_FIXTURE_BYTES}); looks like a recording"
            ));
        }
        let text_ext = matches!(
            e.as_str(),
            "json" | "jsonl" | "toml" | "md" | "py" | "csv" | "html" | "txt" | ""
        );
        if !text_ext && (head.contains(&0) || std::str::from_utf8(head).is_err()) {
            v.push(format!(
                "{rel}: unknown non-text file in the synthetic fixtures"
            ));
        }
        // The engine's campaign.json holds absolute paths of the machine that ran it.
        let text = String::from_utf8_lossy(head);
        let is_data = matches!(e.as_str(), "json" | "jsonl" | "toml" | "csv");
        if let Some(p) = LOCAL_PATH_PREFIXES
            .iter()
            .find(|p| is_data && text.contains(**p))
        {
            v.push(format!(
                "{rel}: absolute local path ({p}...) in the synthetic fixtures"
            ));
        }
        if text_ext && head.contains(&0) {
            v.push(format!("{rel}: binary content behind a text extension"));
        }
        return v;
    }
    if let Some(schema) = lab_schema(p, head) {
        return vec![format!(
            "{rel}: `{schema}` document outside {FIXTURE_DIR} (client data must not be committed; N6)"
        )];
    }
    if is_output_by_name(rel) {
        return vec![format!("{rel}: campaign output outside {FIXTURE_DIR} (N6)")];
    }
    if output_dir {
        return vec![format!(
            "{rel}: file in a campaign output directory outside {FIXTURE_DIR} (N6)"
        )];
    }
    Vec::new()
}

fn is_output_dir(dir: &Path) -> bool {
    OUTPUT_DIR_MARKERS.iter().any(|m| dir.join(m).exists())
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
        let rel = p
            .strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        let in_fixture = p.starts_with(&fixtures);
        let size = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let h = head(p);
        let output_dir = p
            .ancestors()
            .skip(1)
            .take_while(|d| *d != root)
            .any(is_output_dir);
        violations.extend(judge(&rel, in_fixture, size, &h, output_dir));
        if !in_fixture {
            continue;
        }
        let Some(schema) = lab_schema(p, &h) else {
            continue;
        };
        if schema.starts_with("kshana.campaign-plan/") || schema.starts_with("kshana.campaign/") {
            fixture_campaigns += 1;
            let Ok(text) = fs::read_to_string(p) else {
                violations.push(format!("{rel}: fixture campaign is not UTF-8 text"));
                continue;
            };
            let v: serde_json::Value = if ext(p) == "json" {
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
            if v.get("data_class").and_then(|d| d.as_str()) != Some("synthetic") {
                violations.push(format!(
                    "{rel}: fixture campaign data_class must be \"synthetic\" (the gate needs it present)"
                ));
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

const OUT: &str = "elsewhere/run";

#[test]
fn a_schema_is_found_in_the_first_or_last_4_kb_of_a_huge_file() {
    // A scorecard far above any size cap is still recognised from its head.
    let mut head = br#"{"schema":"kshana.campaign-scorecard/1","rows":["#.to_vec();
    head.resize(SNIFF_BYTES, b' ');
    assert!(!judge("x/scorecard.json", false, 500 << 20, &head, false).is_empty());
    assert!(!judge("x/anything.json", false, 500 << 20, &head, false).is_empty());
}

#[test]
fn the_head_reader_reaches_past_the_first_4_kb() {
    // A 20 MB file whose schema is only at the end, as a sorted-key plan has it.
    let dir = std::env::temp_dir().join(format!("kshana-pro-n6-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("big.json");
    let mut body = br#"{"cells":["#.to_vec();
    body.resize(20 << 20, b' ');
    body.extend_from_slice(br#"],"schema":"kshana.campaign-plan/1"}"#);
    fs::write(&p, &body).unwrap();
    let h = head(&p);
    let _ = fs::remove_dir_all(&dir);
    assert!(h.len() <= 2 * SNIFF_BYTES + 1);
    assert!(lab_schema(Path::new("big.json"), &h).is_some());
}

#[test]
fn campaign_outputs_are_recognised_by_name() {
    for rel in [
        "elsewhere/epochs/x.bin",
        "elsewhere/epochs/cell-1.csv",
        "elsewhere/events.jsonl",
        "elsewhere/trace.jsonl",
        "elsewhere/hashes.json",
        "elsewhere/runs.jsonl",
        "elsewhere/scorecard.csv",
        "elsewhere/scorecard.json",
    ] {
        assert!(!judge(rel, false, 10, b"x", false).is_empty(), "{rel}");
    }
    // report.html and *.csv are caught inside a campaign output directory ...
    for rel in [&format!("{OUT}/report.html"), &format!("{OUT}/table.csv")] {
        assert!(!judge(rel, false, 10, b"x", true).is_empty(), "{rel}");
    }
    // ... and left alone elsewhere (papers carry their own report.html and CSV tables).
    assert!(judge("papers/p/report.html", false, 10, b"x", false).is_empty());
    assert!(judge("papers/p/table.csv", false, 10, b"x", false).is_empty());
}

#[test]
fn raw_iq_is_refused_everywhere() {
    for rel in [
        "a/x.bin",
        "a/x.cf32",
        "a/x.ci16",
        "a/x.sigmf-data",
        "a/x.sigmf-meta",
        "tests/fixtures/lab_campaign_synthetic/basic/x.cf32",
        "tests/fixtures/lab_campaign_synthetic/basic/epochs/x.bin",
    ] {
        let fx = rel.starts_with(FIXTURE_DIR);
        assert!(!judge(rel, fx, 10, b"x", false).is_empty(), "{rel}");
    }
}

#[test]
fn a_fixture_with_an_absolute_local_path_is_refused() {
    let rel = "tests/fixtures/lab_campaign_synthetic/basic/campaign.json";
    assert!(!judge(
        rel,
        true,
        50,
        br#"{"recording":"/home/lab/run1.cf32"}"#,
        false
    )
    .is_empty());
    assert!(!judge(rel, true, 50, br#"{"out":"/tmp/x"}"#, false).is_empty());
    assert!(judge(rel, true, 50, br#"{"recording":"synthetic-gap"}"#, false).is_empty());
}

#[test]
fn large_or_binary_unknown_files_in_the_fixtures_are_refused() {
    let rel = "tests/fixtures/lab_campaign_synthetic/basic/blob";
    assert!(!judge(rel, true, MAX_FIXTURE_BYTES + 1, b"text", false).is_empty());
    assert!(!judge(rel, true, 100, &[0, 1, 2, 255, 254], false).is_empty());
    assert!(!judge(&format!("{rel}.dat"), true, 100, &[0xff, 0xfe, 0x80], false).is_empty());
    assert!(judge(&format!("{rel}.json"), true, 100, b"{}", false).is_empty());
}

#[test]
fn the_guard_detects_documents_by_schema() {
    let cell = br#"{"schema":"kshana.campaign-cell/1","key":"00"}"#;
    assert_eq!(
        lab_schema(Path::new("x/cells/00.json"), cell).as_deref(),
        Some("kshana.campaign-cell/")
    );
    let cond = b"schema = \"kshana.test-conditions/1\"\n";
    assert!(lab_schema(Path::new("lab/run.toml"), cond).is_some());
    assert!(lab_schema(Path::new("Cargo.toml"), b"[package]\nname = \"x\"\n").is_none());
}
