// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro iface <template|check|vectors|score>` (research 2026-11-20 gaps, B1604;
//! docs/DECODER-REGRESSION.md).

use std::process::ExitCode;

use crate::interface_drift as id;

use super::flag;
use super::grid_cmd::gate;

pub const IFACE_USAGE: &str = "usage: kshana-pro iface <command>\n\
    template <stream>                                                An interface profile template, every value a placeholder (no licence needed)\n\
    check <profile.toml> <records.csv> [--json <out>] [--license <token>] [--dev]   Drift alarm: decoded header records against the profile (Pro)\n\
    vectors <profile.toml> [--out <file>] [--license <token>] [--dev]   Per-version regression vectors: known records to accept, mutations to flag (Pro)\n\
    score <vectors.json> <decoder-output.csv> [--json <out>] [--license <token>] [--dev]   Score a decoder's accept/flag answers (Pro)";

fn fail(m: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {m}");
    ExitCode::FAILURE
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

fn profile(p: &str) -> Result<id::Profile, String> {
    toml::from_str(&read(p)?).map_err(|e| format!("{p}: {e}"))
}

fn emit<T: serde::Serialize>(args: &[String], v: &T, key: &str) -> Result<(), String> {
    let t = serde_json::to_string_pretty(v).map_err(|e| e.to_string())? + "\n";
    match flag(args, key) {
        Some(o) => std::fs::write(&o, t).map_err(|e| format!("{o}: {e}")),
        None => {
            print!("{t}");
            Ok(())
        }
    }
}

/// `kshana-pro iface ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let pos: Vec<&String> = args
        .iter()
        .skip(1)
        .take_while(|a| !a.starts_with("--"))
        .collect();
    let Some(a1) = pos.first().map(|s| s.as_str()) else {
        return fail(IFACE_USAGE);
    };
    if sub == "template" {
        print!("{}", id::template(a1));
        return ExitCode::SUCCESS;
    }
    if let Err(e) = gate(args, id::FEATURE) {
        return fail(e);
    }
    let r: Result<String, String> = (|| match sub {
        "check" => {
            let a2 = pos.get(1).ok_or(IFACE_USAGE)?;
            let d = id::check(
                &profile(a1)?,
                &id::parse_records(&read(a2)?).map_err(|e| format!("{a2}: {e}"))?,
            )?;
            emit(args, &d, "--json")?;
            if d.alarms.is_empty() {
                Ok(format!("[iface] {}", d.verdict))
            } else {
                Err(d.verdict)
            }
        }
        "vectors" => {
            let v = id::vectors(&profile(a1)?)?;
            emit(args, &v, "--out")?;
            Ok(format!(
                "[iface] {} vectors for {} {}",
                v.vectors.len(),
                v.stream,
                v.version
            ))
        }
        "score" => {
            let a2 = pos.get(1).ok_or(IFACE_USAGE)?;
            let set: id::VectorSet =
                serde_json::from_str(&read(a1)?).map_err(|e| format!("{a1}: {e}"))?;
            let s = id::score(&set, &read(a2)?)?;
            emit(args, &s, "--json")?;
            Ok(format!("[iface] {}", s.verdict))
        }
        _ => Err(IFACE_USAGE.into()),
    })();
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
