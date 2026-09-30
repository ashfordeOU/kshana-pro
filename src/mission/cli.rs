// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro mission <command>`: the command-line front end. The binary resolves the
//! licence first (exactly as for the other Pro commands) and then calls [`run`].

use std::path::Path;

use super::{build, check, BuildOptions};
use crate::license::Entitlement;
use kshana::api::KshanaError;

/// Usage text for `kshana-pro mission`.
pub const USAGE: &str = "kshana-pro mission — one programme of requirements and scenarios to one dossier\n\
\n\
  kshana-pro mission build <programme.toml> --out <dir> [--utc <iso>]\n\
      Run every scenario, check every requirement against the runs, and write dossier.pdf,\n\
      dossier.html, dossier.json, runs/, trace.toml, the evidence pack, manifest.json and ledger.json\n\
  kshana-pro mission check <dir> [--rerun]\n\
      Re-hash every file of a delivered dossier; with --rerun, run every scenario again and compare\n\
\n\
Every command takes [--license <token>] or, in a debug build, [--dev]. See docs/MISSION-DOSSIER.md.";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// The first positional argument after the command (skipping flags and their values).
fn positional(args: &[String]) -> Option<String> {
    let takes_value = ["--out", "--utc", "--license"];
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if takes_value.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if a.starts_with("--") {
            i += 1;
            continue;
        }
        return Some(a.clone());
    }
    None
}

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// Run a `mission` command (`args[0]` is the command) under `ent`. Returns the process
/// exit code: 0 on success, 1 when a check fails, 2 on a usage error.
pub fn run(args: &[String], ent: &Entitlement, now_unix: i64) -> Result<u8, KshanaError> {
    match args.first().map(String::as_str) {
        Some("build") => {
            let programme = positional(args)
                .ok_or_else(|| err(format!("build needs a programme file\n\n{USAGE}")))?;
            let out = flag(args, "--out").ok_or_else(|| err("build needs --out <dir>"))?;
            let built = build(
                Path::new(&programme),
                Path::new(&out),
                &BuildOptions {
                    issued_utc: flag(args, "--utc"),
                    entitlement: ent.clone(),
                    now_unix,
                },
            )?;
            let s = &built.dossier.body.summary;
            eprintln!(
                "[mission] {} scenario runs{}; {} requirements: {} met ({} on a MODELLED or PARTNER basis), {} not met, {} not verified; {} open items",
                s.runs,
                if s.trade_studies > 0 { " and one trade study" } else { "" },
                s.requirements,
                s.met,
                s.met_on_modelled_basis,
                s.not_met,
                s.not_verified,
                s.open_items
            );
            eprintln!(
                "[wrote dossier.pdf ({} pages), dossier.html, dossier.json, evidence-pack.html, evidence-pack.json, trace.toml, manifest.json, ledger.json and runs/ to {out}]",
                built.pdf_pages
            );
            println!("{}", built.dossier.dossier_sha256);
            Ok(0)
        }
        Some("check") => {
            let dir = positional(args)
                .ok_or_else(|| err(format!("check needs a dossier directory\n\n{USAGE}")))?;
            let rerun = has(args, "--rerun");
            let report = check::check(Path::new(&dir), rerun.then_some((ent, now_unix)))?;
            for r in &report.reruns {
                println!(
                    "{}: re-run {}",
                    r.run,
                    if r.matches {
                        "gives the same result bytes"
                    } else {
                        "DIFFERS"
                    }
                );
            }
            if report.passed() {
                println!(
                    "OK: dossier {} is intact ({} files re-hashed{})",
                    report.dossier_sha256,
                    report.files_checked,
                    if rerun {
                        format!(", {} runs repeated", report.reruns.len())
                    } else {
                        String::new()
                    }
                );
                Ok(0)
            } else {
                for p in &report.problems {
                    println!("FAIL: {p}");
                }
                println!(
                    "FAILED: {} problems in dossier {}",
                    report.problems.len(),
                    report.dossier_sha256
                );
                Ok(1)
            }
        }
        _ => {
            println!("{USAGE}");
            Ok(if args.is_empty() { 0 } else { 2 })
        }
    }
}
