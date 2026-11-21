// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro method <schedule|budget>` (research 2026-11-20 gaps, B2505 Pro part;
//! docs/METHOD-UNCERTAINTY.md).

use std::path::Path;
use std::process::ExitCode;

use crate::method_uncertainty as mu;
use crate::method_validation as mv;

use super::flag;
use super::grid_cmd::{gate, signing};

pub const METHOD_USAGE: &str = "usage: kshana-pro method <command>\n\
    schedule <register.csv> [--interval-months <n>] [--warn-days <d>] [--utc <iso>] [--json <out>] [--license <token>] [--dev]   Re-scoring schedule: next due date per product and method, overdue and due soon (Pro)\n\
    budget <method.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   Per-test GUM uncertainty budgets and method report, \"scored under method X\" (Pro)\n\
    validate <study.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   Interlaboratory precision study: repeatability r and reproducibility R, Mandel h/k, Cochran and Grubbs, bias against a reference (Pro)\n\
    synth-study <file.toml>   A SYNTHETIC interlaboratory study (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn emit<T: serde::Serialize>(args: &[String], v: &T) -> Result<(), String> {
    let t = crate::transport::rounded_json(v);
    match flag(args, "--json") {
        Some(o) => std::fs::write(&o, t).map_err(|e| format!("{o}: {e}")),
        None => {
            println!("{t}");
            Ok(())
        }
    }
}

/// `kshana-pro method ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(METHOD_USAGE);
    };
    if args.first().map(String::as_str) == Some("synth-study") {
        return match std::fs::write(&arg, mv::synth()) {
            Ok(()) => {
                eprintln!("[method synth-study] SYNTHETIC study in {arg}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(format!("{arg}: {e}")),
        };
    }
    let feature = if args.first().map(String::as_str) == Some("validate") {
        mv::FEATURE
    } else {
        mu::FEATURE
    };
    if let Err(e) = gate(args, feature) {
        return fail(e);
    }
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("schedule") => (|| {
            let reg = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let mut p = mu::Policy::default();
            if let Some(m) = flag(args, "--interval-months") {
                p.interval_months = m.parse().map_err(|_| "bad --interval-months")?;
            }
            if let Some(w) = flag(args, "--warn-days") {
                p.warn_days = w.parse().map_err(|_| "bad --warn-days")?;
            }
            let utc = flag(args, "--utc").unwrap_or_else(super::utc_iso8601_now);
            let s = mu::schedule(&reg, &p, &utc)?;
            emit(args, &s)?;
            Ok(format!("[method schedule] {}", s.verdict))
        })(),
        Some("budget") => (|| {
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: mu::MethodRequest = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let r = mu::report(&q)?;
            if flag(args, "--json").is_some() || flag(args, "--out").is_none() {
                emit(args, &r)?;
            }
            if flag(args, "--out").is_some() {
                let s = signing(args, "method budget")?;
                crate::transport::write_pack(
                    Path::new(&s.out),
                    &r,
                    &mu::html(&r),
                    "method.toml",
                    &src,
                )?;
                crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                    .map_err(|e| e.to_string())?;
            }
            Ok(format!("[method budget] {}", r.verdict))
        })(),
        Some("validate") => (|| {
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: mv::Study = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let r = mv::study(&q)?;
            if flag(args, "--json").is_some() || flag(args, "--out").is_none() {
                emit(args, &r)?;
            }
            if flag(args, "--out").is_some() {
                let s = signing(args, "method validate")?;
                crate::transport::write_pack(
                    Path::new(&s.out),
                    &r,
                    &mv::html(&r),
                    "study.toml",
                    &src,
                )?;
                crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                    .map_err(|e| e.to_string())?;
            }
            Ok(format!("[method validate] {}", r.verdict))
        })(),
        _ => return fail(METHOD_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
