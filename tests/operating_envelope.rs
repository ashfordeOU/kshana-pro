// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Operating envelope of every Pro output (research 2026-11-20 gaps B2508): every format tag the
//! crate writes belongs to a family; envelopes follow the published schema; every signed Kshana
//! evidence directory carries its envelope under the signature, and a user's own directory is
//! left alone.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::envelope::{self, COMMON_EXCLUSIONS};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b2508-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Every `"kshana…/N"` format tag in the crate's source.
fn format_tags() -> Vec<String> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut tags = std::collections::BTreeSet::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let mut rest = text.as_str();
        while let Some(i) = rest.find("\"kshana") {
            let s = &rest[i + 1..];
            let end = s.find('"').unwrap_or(0);
            let tag = &s[..end];
            let ok = tag.split_once('/').is_some_and(|(n, v)| {
                !v.is_empty()
                    && v.chars().all(|c| c.is_ascii_digit())
                    && n.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.'
                    })
            });
            if ok {
                tags.insert(tag.to_string());
            }
            rest = &s[end.max(1)..];
        }
    }
    tags.into_iter().collect()
}

#[test]
fn every_format_tag_belongs_to_a_family() {
    let tags = format_tags();
    assert!(tags.len() > 100, "found only {} format tags", tags.len());
    let orphans: Vec<&String> = tags
        .iter()
        .filter(|t| envelope::family_of(t).is_none())
        .collect();
    assert!(
        orphans.is_empty(),
        "format tags with no envelope family: {orphans:?}"
    );
}

#[test]
fn envelopes_follow_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("docs/schemas/operating-envelope.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let allowed: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
    for t in format_tags() {
        let e = envelope::for_format(&t);
        let v = serde_json::to_value(&e).unwrap();
        let o = v.as_object().unwrap();
        for k in &required {
            assert!(o.contains_key(*k), "{t}: missing {k}");
        }
        for k in o.keys() {
            assert!(allowed.contains(&k), "{t}: {k} not in the schema");
        }
        assert_eq!(
            e.format,
            schema["properties"]["format"]["const"].as_str().unwrap()
        );
        assert!(!e.valid_inputs.is_empty() && e.valid_inputs.iter().all(|s| !s.is_empty()));
        assert!(e
            .ranges
            .iter()
            .all(|r| !r.quantity.is_empty() && !r.range.is_empty()));
        for c in COMMON_EXCLUSIONS {
            assert!(
                e.exclusions.iter().any(|x| x == c),
                "{t}: common exclusion missing"
            );
        }
        assert_ne!(e.family, "general", "{t}");
    }
}

#[test]
fn signed_directories_carry_the_envelope_under_the_signature() {
    let d = tmp("sign");
    std::fs::write(
        d.join("report.json"),
        r#"{"format": "kshana-pro-grid-pmu/1", "verdict": "x"}"#,
    )
    .unwrap();
    let seed = [3u8; 32];
    kshana_pro::signed_dossier::sign(&d, &seed, "t", "2026-11-20T00:00:00Z").unwrap();
    let e: envelope::Envelope =
        serde_json::from_str(&std::fs::read_to_string(d.join(envelope::FILE)).unwrap()).unwrap();
    assert_eq!(e.output_format, "kshana-pro-grid-pmu/1");
    assert_eq!(e.family, "timing");
    let key = [kshana_pro::license::public_key_for(&seed)];
    let v = kshana_pro::signed_dossier::verify(&d, &key).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    // The envelope is covered: changing it fails verification.
    std::fs::write(d.join(envelope::FILE), "{}").unwrap();
    assert!(!kshana_pro::signed_dossier::verify(&d, &key)
        .unwrap()
        .passed());

    // A user's own directory is signed as it is.
    let u = tmp("user");
    std::fs::write(u.join("notes.txt"), "mine").unwrap();
    std::fs::write(u.join("data.json"), r#"{"format": "someone-else/1"}"#).unwrap();
    kshana_pro::signed_dossier::sign(&u, &seed, "t", "2026-11-20T00:00:00Z").unwrap();
    assert!(!u.join(envelope::FILE).exists());
}

#[test]
fn signed_pack_from_a_command_carries_its_envelope_and_cli_prints_one() {
    let d = tmp("cli");
    let home = d.join("home");
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    };
    let (ok, out) = run(&["envelope", "kshana-pro-ais-integrity/1"]);
    assert!(ok);
    let e: envelope::Envelope = serde_json::from_str(&out).unwrap();
    assert_eq!(e.family, "maritime");
    // A signed pack written by a command: the clock-bench protocol is free to register.
    let sig = d.join("sig");
    std::fs::create_dir_all(&sig).unwrap();
    std::fs::write(
        sig.join("result.json"),
        r#"{"format": "kshana-pro-rail-thr/1"}"#,
    )
    .unwrap();
    kshana_pro::signed_dossier::sign(&sig, &[4u8; 32], "t", "2026-11-20T00:00:00Z").unwrap();
    let (ok, out) = run(&["envelope", sig.to_str().unwrap()]);
    assert!(ok);
    assert!(
        out.contains("land-transport-agriculture-and-cellular"),
        "{out}"
    );
    let (ok, _) = run(&["envelope"]);
    assert!(!ok);
}
