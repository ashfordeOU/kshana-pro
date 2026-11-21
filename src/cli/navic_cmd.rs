// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro navic <scaffold|health|consistency|timing>` (research 2026-11-20 gaps, B1605;
//! docs/NAVIC.md).

use std::path::Path;
use std::process::ExitCode;

use crate::navic;

use super::flag;
use super::grid_cmd::{gate, signing};

pub const NAVIC_USAGE: &str = "usage: kshana-pro navic <command>\n\
    scaffold [--out <file>]                                          L1 SPS test-vector skeleton, every value a placeholder (no licence needed)\n\
    health <sats.csv> <health.csv> <fixes.csv> [--json <out>] [--license <token>] [--dev]   Unhealthy satellites used; NavIC-only fixes with fewer than four healthy satellites (Pro)\n\
    consistency <solutions.csv> [--gate-k <k>] [--join-s <s>] [--json <out>] [--license <token>] [--dev]   NavIC, GPS and Galileo solutions compared pairwise (Pro)\n\
    timing <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Signed NavIC timing evidence with the stated traceability chain (Pro)";

fn fail(m: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {m}");
    ExitCode::FAILURE
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
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

/// `kshana-pro navic ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let pos: Vec<&String> = args
        .iter()
        .skip(1)
        .take_while(|a| !a.starts_with("--"))
        .collect();
    if sub == "scaffold" {
        let t = serde_json::to_string_pretty(&navic::scaffold()).unwrap_or_default() + "\n";
        return match flag(args, "--out") {
            Some(o) => match std::fs::write(&o, t) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => fail(format!("{o}: {e}")),
            },
            None => {
                print!("{t}");
                ExitCode::SUCCESS
            }
        };
    }
    if let Err(e) = gate(args, navic::FEATURE) {
        return fail(e);
    }
    let r: Result<String, String> = (|| match (sub, pos.as_slice()) {
        ("health", [a, b, c]) => {
            let h = navic::health(&read(a)?, &read(b)?, &read(c)?)?;
            emit(args, &h)?;
            Ok(format!("[navic health] {}", h.verdict))
        }
        ("consistency", [a]) => {
            let k = flag(args, "--gate-k")
                .and_then(|x| x.parse().ok())
                .unwrap_or(5.0);
            let j = flag(args, "--join-s")
                .and_then(|x| x.parse().ok())
                .unwrap_or(60.0);
            let c = navic::consistency(&read(a)?, k, j)?;
            emit(args, &c)?;
            Ok(format!("[navic consistency] {}", c.verdict))
        }
        ("timing", [a]) => {
            let src = read(a)?;
            let q: navic::TimingRequest = toml::from_str(&src).map_err(|e| format!("{a}: {e}"))?;
            let base = Path::new(a.as_str()).parent().unwrap_or(Path::new("."));
            let csv = crate::timing::grid::read_text(base, &q.offsets)?;
            let t = navic::timing(&q, &csv)?;
            let s = signing(args, "navic timing")?;
            let html = crate::transport::page(
                &t.title,
                t.statement,
                &format!(
                    "NavIC minus GPS: mean {:.3e} s, RMS {:.3e} s over {} samples",
                    t.navic_minus_gps_mean_s, t.navic_minus_gps_rms_s, t.samples
                ),
                &t.traceability,
                &[],
            );
            crate::transport::write_pack(Path::new(&s.out), &t, &html, "request.toml", &src)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!("[navic timing] signed evidence in {}", s.out))
        }
        _ => Err(NAVIC_USAGE.into()),
    })();
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
