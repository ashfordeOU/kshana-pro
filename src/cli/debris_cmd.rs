// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro debris-obs <plan|synth>` (observation scheduling for space-debris sensors;
//! docs/DEBRIS-OBS.md).

use std::path::Path;
use std::process::ExitCode;

use crate::debris_obs;

use super::grid_cmd::{gate, signing};

pub const DEBRIS_USAGE: &str = "usage: kshana-pro debris-obs <command>\n\
    plan <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Visibility windows, a priority schedule and the expected orbit-covariance improvement per scheduled pass for a debris sensor; planning only, signed (Pro)\n\
    synth <dir>   Write a SYNTHETIC radar and target list (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro debris-obs ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(DEBRIS_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            std::fs::create_dir_all(&arg).map_err(|e| e.to_string())?;
            std::fs::write(Path::new(&arg).join("request.toml"), debris_obs::synth())
                .map_err(|e| e.to_string())?;
            Ok(format!("[debris-obs] SYNTHETIC request in {arg}"))
        })(),
        Some("plan") => (|| {
            gate(args, debris_obs::FEATURE)?;
            let s = signing(args, "debris-obs plan")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: debris_obs::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let p = debris_obs::plan(&q, &base)?;
            crate::transport::write_pack(
                Path::new(&s.out),
                &p,
                &debris_obs::html(&p),
                "request.toml",
                &src,
            )?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[debris-obs] {}\n[debris-obs] signed in {}",
                p.verdict, s.out
            ))
        })(),
        _ => return fail(DEBRIS_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
