// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro enforcement <pack|synth|checklist>` (research 2026-11-20 gaps, B0605;
//! docs/JAMMER-ENFORCEMENT.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::enforcement;

use super::grid_cmd::{gate, signing};

pub const ENFORCEMENT_USAGE: &str = "usage: kshana-pro enforcement <command>\n\
    pack <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Jammer enforcement evidence pack: victim timeline and effects, affected area from victim receivers only, custody chain, method statement, cited checklist (Pro)\n\
    checklist   Print the checklist and the published cases it is derived from (no licence needed)\n\
    synth <dir>   Write a SYNTHETIC case and its request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro enforcement ...`.
pub fn run(args: &[String]) -> ExitCode {
    if args.first().map(String::as_str) == Some("checklist") {
        for c in enforcement::CHECKLIST {
            println!(
                "{} {} [{}] ({})",
                c.id,
                c.item,
                c.supplied_by.unwrap_or("for the authority"),
                c.sources.join("; ")
            );
        }
        for (k, s) in enforcement::SOURCES {
            println!("{k}: {s}");
        }
        return ExitCode::SUCCESS;
    }
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(ENFORCEMENT_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            let d = Path::new(&arg);
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            let (csv, req) = enforcement::synth();
            std::fs::write(d.join("victims.csv"), csv).map_err(|e| e.to_string())?;
            std::fs::write(d.join("request.toml"), req).map_err(|e| e.to_string())?;
            Ok(format!("[enforcement] SYNTHETIC case and request in {arg}"))
        })(),
        Some("pack") => (|| {
            gate(args, enforcement::FEATURE)?;
            let s = signing(args, "enforcement pack")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: enforcement::Request =
                toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let k = enforcement::build(&q, &base)?;
            let out = Path::new(&s.out);
            crate::transport::write_pack(out, &k, &enforcement::html(&k), "request.toml", &src)?;
            std::fs::copy(
                base.join(&q.victims),
                out.join("inputs").join("victims.csv"),
            )
            .map_err(|e| format!("{}: {e}", q.victims))?;
            crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[enforcement] {}\n[enforcement] signed evidence pack in {}",
                k.verdict, s.out
            ))
        })(),
        _ => return fail(ENFORCEMENT_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
