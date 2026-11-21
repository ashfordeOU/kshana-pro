// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro fingerprint build` (research 2026-11-20 gaps, B2603; docs/RECEIVER-FINGERPRINT.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::fingerprint;

use super::grid_cmd::{gate, signing};

pub const FINGERPRINT_USAGE: &str = "usage: kshana-pro fingerprint build <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
    Signed per-firmware receiver behaviour fingerprint from verified Pro scorecards; with the customer's consent, a contribution for the population benchmark (Pro)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro fingerprint ...`.
pub fn run(args: &[String]) -> ExitCode {
    let (Some("build"), Some(arg)) = (
        args.first().map(String::as_str),
        args.get(1).filter(|a| !a.starts_with("--")),
    ) else {
        return fail(FINGERPRINT_USAGE);
    };
    let r = (|| -> Result<String, String> {
        gate(args, fingerprint::FEATURE)?;
        let s = signing(args, "fingerprint build")?;
        let src = std::fs::read_to_string(arg).map_err(|e| format!("{arg}: {e}"))?;
        let q: fingerprint::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
        let base = Path::new(arg)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let (f, c) = fingerprint::build(&q, &base)?;
        let out = Path::new(&s.out);
        let io = |e: std::io::Error| format!("{}: {e}", out.display());
        std::fs::create_dir_all(out).map_err(io)?;
        std::fs::write(
            out.join(fingerprint::FINGERPRINT_FILE),
            crate::transport::rounded_json(&f),
        )
        .map_err(io)?;
        std::fs::write(out.join("fingerprint.html"), fingerprint::html(&f)).map_err(io)?;
        if let Some(c) = &c {
            std::fs::write(
                out.join(fingerprint::CONTRIBUTION_FILE),
                crate::transport::rounded_json(c),
            )
            .map_err(io)?;
        }
        crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc).map_err(|e| e.to_string())?;
        Ok(format!(
            "[fingerprint] {}\n[fingerprint] signed in {}",
            f.verdict, s.out
        ))
    })();
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
