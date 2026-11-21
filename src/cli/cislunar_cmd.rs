// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro cislunar-catalogue <run|synth>` (tracklet association and initial orbit
//! determination for cislunar objects; docs/CISLUNAR-CATALOGUE.md).

use std::path::Path;
use std::process::ExitCode;

use crate::cislunar_catalogue as cc;

use super::grid_cmd::{gate, signing};

pub const CISLUNAR_USAGE: &str = "usage: kshana-pro cislunar-catalogue <command>\n\
    run <request.json> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Associate angles-only tracklets of cislunar objects and give each object an initial orbit with covariance (CR3BP); planning and evidence only, signed (Pro)\n\
    synth <dir>   Write a SYNTHETIC request (three CR3BP objects, nine tracklets) (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro cislunar-catalogue ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(CISLUNAR_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            std::fs::create_dir_all(&arg).map_err(|e| e.to_string())?;
            let j = serde_json::to_string_pretty(&cc::synth()).map_err(|e| e.to_string())?;
            std::fs::write(Path::new(&arg).join("request.json"), j + "\n")
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[cislunar-catalogue] SYNTHETIC request in {arg}/request.json"
            ))
        })(),
        Some("run") => (|| {
            gate(args, cc::FEATURE)?;
            let s = signing(args, "cislunar-catalogue run")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: cc::Request = serde_json::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let r = cc::run(&q)?;
            crate::transport::write_pack(
                Path::new(&s.out),
                &r,
                &cc::html(&r),
                "request.json",
                &src,
            )?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[cislunar-catalogue] {}\n[cislunar-catalogue] signed in {}",
                r.verdict, s.out
            ))
        })(),
        _ => return fail(CISLUNAR_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
