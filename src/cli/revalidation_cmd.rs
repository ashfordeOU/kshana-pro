// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro revalidation plan`: a fleet revalidation plan across open product registers and
//! their revocation lists (docs/REVALIDATION-PLAN.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::revalidation as rv;

use super::flag;
use super::grid_cmd::{gate, signing};

pub const REVALIDATION_USAGE: &str = "usage: kshana-pro revalidation <command>\n\
    plan <request.toml> --out <dir> --signer <name> --key-file <file> [--date <YYYY-MM-DD>] [--utc <iso>] [--license <token>] [--dev]\n\
        Revalidation calendar across kshana.register/1 registers and kshana.register-crl/1 lists: OVERDUE / DUE SOON / SCHEDULED / REVOKED, by month and maker, CSV and iCalendar, signed (Pro)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro revalidation ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(REVALIDATION_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("plan") => (|| {
            gate(args, rv::FEATURE)?;
            let s = signing(args, "revalidation plan")?;
            let date = flag(args, "--date").unwrap_or_else(|| s.utc[..10].to_string());
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: rv::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let (p, inputs) = rv::run(&q, &base, &date)?;
            let out = Path::new(&s.out);
            crate::transport::write_pack(out, &p, &rv::html(&p), "request.toml", &src)?;
            for (name, b) in &inputs {
                let f = out.join("inputs").join(name);
                std::fs::create_dir_all(f.parent().expect("has a parent"))
                    .map_err(|e| e.to_string())?;
                std::fs::write(&f, b).map_err(|e| e.to_string())?;
            }
            std::fs::write(out.join("plan.csv"), rv::csv(&p)).map_err(|e| e.to_string())?;
            std::fs::write(out.join("plan.ics"), rv::ics(&p)).map_err(|e| e.to_string())?;
            crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[revalidation] {}\n[revalidation] signed plan in {}",
                p.verdict, s.out
            ))
        })(),
        _ => return fail(REVALIDATION_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
