// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro crisis-pnt <run|synth>` (crisis PNT service availability; docs/CRISIS-PNT.md).

use std::path::Path;
use std::process::ExitCode;

use crate::crisis_pnt;

use super::grid_cmd::{gate, signing};

pub const CRISIS_USAGE: &str = "usage: kshana-pro crisis-pnt <command>\n\
    run <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Modelled service availability, accuracy class and timing holdover per user class over an area when GNSS is denied and given terrestrial and LEO services remain; planning only, signed (Pro)\n\
    synth <dir>   Write a SYNTHETIC crisis request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro crisis-pnt ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(CRISIS_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            std::fs::create_dir_all(&arg).map_err(|e| e.to_string())?;
            std::fs::write(Path::new(&arg).join("request.toml"), crisis_pnt::synth())
                .map_err(|e| e.to_string())?;
            Ok(format!("[crisis-pnt] SYNTHETIC request in {arg}"))
        })(),
        Some("run") => (|| {
            gate(args, crisis_pnt::FEATURE)?;
            let s = signing(args, "crisis-pnt run")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: crisis_pnt::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let r = crisis_pnt::evaluate(&q)?;
            crate::transport::write_pack(
                Path::new(&s.out),
                &r,
                &crisis_pnt::html(&r),
                "request.toml",
                &src,
            )?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[crisis-pnt] {}\n[crisis-pnt] signed in {}",
                r.verdict, s.out
            ))
        })(),
        _ => return fail(CRISIS_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
