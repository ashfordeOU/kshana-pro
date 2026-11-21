// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro coexistence <catalogue|scenarios|synth>` (co-existence and compatibility
//! interference catalogue for civil links; docs/COEXISTENCE.md).

use std::path::Path;
use std::process::ExitCode;

use crate::coexistence as cx;

use super::grid_cmd::{gate, signing};

pub const COEXISTENCE_USAGE: &str = "usage: kshana-pro coexistence <command>\n\
    catalogue [--json]   The co-existence catalogue (co-channel, adjacent-band blocking, out-of-band boundary, spurious emissions), every value cited (no licence needed)\n\
    scenarios <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Scenario pack for a victim receiver and its neighbouring transmitters; signed (Pro)\n\
    synth <dir>   Write a SYNTHETIC request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro coexistence ...`.
pub fn run(args: &[String]) -> ExitCode {
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("catalogue") => {
            if args.iter().any(|a| a == "--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "scope": cx::SCOPE,
                        "entries": cx::CATALOGUE,
                        "sources": cx::SOURCES,
                    }))
                    .unwrap_or_default()
                );
            } else {
                for e in cx::CATALOGUE {
                    println!(
                        "{:<26} {:<24} {} | {} [{} {}]",
                        e.id,
                        format!("{:?}", e.category),
                        e.frequency,
                        e.level,
                        e.source,
                        e.clause
                    );
                }
            }
            Ok(String::new())
        }
        Some("synth") => (|| {
            let d = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .ok_or(COEXISTENCE_USAGE)?;
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            std::fs::write(Path::new(d).join("request.toml"), cx::synth())
                .map_err(|e| e.to_string())?;
            Ok(format!("[coexistence] SYNTHETIC request in {d}"))
        })(),
        Some("scenarios") => (|| {
            let f = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .ok_or(COEXISTENCE_USAGE)?;
            gate(args, cx::FEATURE)?;
            let s = signing(args, "coexistence scenarios")?;
            let src = std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"))?;
            let q: cx::Request = toml::from_str(&src).map_err(|e| format!("{f}: {e}"))?;
            let p = cx::build(&q)?;
            crate::transport::write_pack(
                Path::new(&s.out),
                &p,
                &cx::html(&p),
                "request.toml",
                &src,
            )?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[coexistence] {}\n[coexistence] signed in {}",
                p.verdict, s.out
            ))
        })(),
        _ => return fail(COEXISTENCE_USAGE),
    };
    match r {
        Ok(m) => {
            if !m.is_empty() {
                eprintln!("{m}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
