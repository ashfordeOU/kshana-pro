// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro tacho <verify|synth>` (research 2026-11-20 gaps, B1403; docs/TACHOGRAPH-OSNMA.md).

use std::path::Path;
use std::process::ExitCode;

use crate::tacho_osnma as t;

use super::grid_cmd::{gate, signing};

pub const TACHO_USAGE: &str = "usage: kshana-pro tacho <command>\n\
    verify <log.csv> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Field verification of OSNMA use against Annex IC of (EU) 2021/1228 as read; signed (Pro)\n\
    synth <good|bad> <file.csv>                                      A SYNTHETIC vehicle-unit log (no licence needed)";

fn fail(m: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {m}");
    ExitCode::FAILURE
}

/// `kshana-pro tacho ...`.
pub fn run(args: &[String]) -> ExitCode {
    let pos: Vec<&String> = args
        .iter()
        .skip(1)
        .take_while(|a| !a.starts_with("--"))
        .collect();
    let r: Result<String, String> = match (args.first().map(String::as_str), pos.as_slice()) {
        (Some("synth"), [kind, f]) => (|| {
            let good = match kind.as_str() {
                "good" => true,
                "bad" => false,
                _ => return Err(TACHO_USAGE.to_string()),
            };
            std::fs::write(f.as_str(), t::synth(good)).map_err(|e| format!("{f}: {e}"))?;
            Ok(format!("[tacho] SYNTHETIC {kind} log in {f}"))
        })(),
        (Some("verify"), [f]) => (|| {
            gate(args, t::FEATURE)?;
            let s = signing(args, "tacho verify")?;
            let csv = std::fs::read_to_string(f.as_str()).map_err(|e| format!("{f}: {e}"))?;
            let r = t::verify(&csv, &t::Thresholds::default())?;
            let rows = r
                .findings
                .iter()
                .map(|x| {
                    vec![
                        x.requirement.to_string(),
                        x.check.to_string(),
                        if x.ok { "as read" } else { "departure" }.to_string(),
                        x.detail.clone(),
                    ]
                })
                .collect();
            let html = crate::transport::page(
                "Tachograph OSNMA field verification",
                r.label,
                &r.verdict,
                &[r.source.to_string()],
                &[(
                    "Checks".into(),
                    vec!["requirement", "check", "result", "detail"],
                    rows,
                )],
            );
            crate::transport::write_pack(Path::new(&s.out), &r, &html, "log.csv", &csv)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[tacho] {}\n[tacho] signed pack in {}",
                r.verdict, s.out
            ))
        })(),
        _ => Err(TACHO_USAGE.into()),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
