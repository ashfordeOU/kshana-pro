// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro osnma-pack <list|export|score|synth-log>` (research 2026-11-20 gaps, B1402;
//! docs/OSNMA-SCENARIOS.md).

use std::path::Path;
use std::process::ExitCode;

use crate::osnma_scenarios as sc;

use super::grid_cmd::{gate, signing};

pub const OSNMA_PACK_USAGE: &str = "usage: kshana-pro osnma-pack <command>\n\
    list   List the scenarios and their expected states (no licence needed)\n\
    export <dir> [--license <token>] [--dev]   Write the scenario pack (conditions per subframe, expected states, reference-model and engine checks) as JSON (Pro)\n\
    score <receiver-log.json> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Score a receiver log against the expected states; signed (Pro)\n\
    synth-log <good|faulty> <file>   Write a SYNTHETIC receiver log over every scenario (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn pretty<T: serde::Serialize>(v: &T) -> Result<String, String> {
    serde_json::to_string_pretty(v)
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

/// `kshana-pro osnma-pack ...`.
pub fn run(args: &[String]) -> ExitCode {
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("list") => {
            for s in sc::scenarios() {
                println!("{:<18} {}  {}", s.id, s.expected, s.title);
            }
            Ok(String::new())
        }
        Some("export") => (|| {
            let dir = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .ok_or(OSNMA_PACK_USAGE)?;
            gate(args, sc::FEATURE)?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            std::fs::write(Path::new(dir).join("pack.json"), pretty(&sc::pack())?)
                .map_err(|e| e.to_string())?;
            Ok(format!("[osnma-pack] scenario pack in {dir}/pack.json"))
        })(),
        Some("synth-log") => (|| {
            let (Some(k), Some(f)) = (args.get(1), args.get(2)) else {
                return Err(OSNMA_PACK_USAGE.into());
            };
            let good = match k.as_str() {
                "good" => true,
                "faulty" => false,
                _ => return Err(OSNMA_PACK_USAGE.into()),
            };
            std::fs::write(f, pretty(&sc::synth_log(good))?).map_err(|e| e.to_string())?;
            Ok(format!("[osnma-pack] SYNTHETIC {k} receiver log in {f}"))
        })(),
        Some("score") => (|| {
            let file = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .ok_or(OSNMA_PACK_USAGE)?;
            gate(args, sc::FEATURE)?;
            let s = signing(args, "osnma-pack score")?;
            let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
            let log: crate::osnma_conformance::ReceiverLog =
                serde_json::from_slice(&bytes).map_err(|e| format!("{file}: {e}"))?;
            let score = sc::score(&log, &bytes)?;
            let out = Path::new(&s.out);
            crate::transport::write_pack(
                out,
                &score,
                &sc::html(&score),
                "receiver-log.json",
                &String::from_utf8_lossy(&bytes),
            )?;
            std::fs::write(out.join("pack.json"), pretty(&sc::pack())?)
                .map_err(|e| e.to_string())?;
            crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[osnma-pack] {}\n[osnma-pack] signed score in {}",
                score.verdict, s.out
            ))
        })(),
        _ => return fail(OSNMA_PACK_USAGE),
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
