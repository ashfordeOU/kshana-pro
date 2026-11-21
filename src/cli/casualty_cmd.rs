// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro casualty <run|synth>` (research 2026-11-20 gaps, B0601; docs/CASUALTY.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::forensics::casualty;

use super::grid_cmd::{gate, signing};

pub const CASUALTY_USAGE: &str = "usage: kshana-pro casualty <command>\n\
    run <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Grounding or collision reconstruction: displayed vs reconstructed track, interference timeline, causation statement with uncertainty, signed pack (Pro)\n\
    synth <grounding-spoofed|grounding-spoofed-radar|grounding-jammed|grounding-nav-error|collision-nav-error> <dir>   Write a SYNTHETIC casualty and its request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro casualty ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(CASUALTY_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            let case = casualty::synth::Case::parse(&arg).ok_or(CASUALTY_USAGE)?;
            let dir = args.get(2).ok_or(CASUALTY_USAGE)?;
            let d = Path::new(dir);
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            let s = casualty::synth::make(case);
            for (n, t) in &s.files {
                std::fs::write(d.join(n), t).map_err(|e| e.to_string())?;
            }
            std::fs::write(d.join("request.toml"), &s.request).map_err(|e| e.to_string())?;
            Ok(format!(
                "[casualty] SYNTHETIC {arg} case and request in {dir}"
            ))
        })(),
        Some("run") => (|| {
            gate(args, casualty::FEATURE)?;
            let s = signing(args, "casualty run")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: casualty::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let (r, m) = casualty::run(&q, &base)?;
            casualty::write(Path::new(&s.out), &r, &m, &q, &src, &base)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[casualty] {}\n[casualty] signed reconstruction pack in {}",
                r.verdict, s.out
            ))
        })(),
        _ => return fail(CASUALTY_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
