// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro twin-recal <run|synth>` (digital-twin recalibration evidence pack on the hosted
//! `calibrate` verdict; docs/TWIN-RECALIBRATION.md).

use std::path::Path;
use std::process::ExitCode;

use crate::license::activation::Store;
use crate::remote_verdict::CurlVerdictTransport;
use crate::twin_recal as tr;

use super::flag;
use super::grid_cmd::{gate, signing};
use super::verdict_cmd::{env, trusted_key};

pub const TWIN_RECAL_USAGE: &str = "usage: kshana-pro twin-recal <command>\n\
    run <request.toml> --out <dir> --signer <name> --key-file <file> [--url <https://service>] [--trust <key>] [--license <token>] [--utc <iso>] [--dev]   Recalibrate a model against flown telemetry window by window through the hosted calibrate verdict (each verdict verified), with local before/after residuals and parameter drift; signed (Pro; the fit is the service's)\n\
    synth <dir>   Write SYNTHETIC clock telemetry and a request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro twin-recal ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(TWIN_RECAL_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            std::fs::create_dir_all(&arg).map_err(|e| e.to_string())?;
            let (csv, req) = tr::synth_clock();
            std::fs::write(Path::new(&arg).join("telemetry.csv"), csv)
                .map_err(|e| e.to_string())?;
            std::fs::write(Path::new(&arg).join("request.toml"), req).map_err(|e| e.to_string())?;
            Ok(format!(
                "[twin-recal] SYNTHETIC clock telemetry and request in {arg}"
            ))
        })(),
        Some("run") => (|| {
            gate(args, tr::FEATURE)?;
            let s = signing(args, "twin-recal run")?;
            let url = flag(args, "--url")
                .or_else(|| env("KSHANA_VERDICT_URL"))
                .ok_or("the verdict service URL is required (--url or KSHANA_VERDICT_URL)")?;
            let key = trusted_key(args)?;
            let token = flag(args, "--license")
                .or_else(|| {
                    Store::default_dir()
                        .map(Store::open)
                        .and_then(|s| s.token())
                })
                .ok_or("a licence is required: --license <token>, or an activated licence")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: tr::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg).parent().unwrap_or(Path::new("."));
            let tel = crate::timing::grid::read_text(base, &q.telemetry)?;
            let t = CurlVerdictTransport { base_url: url };
            let run = tr::run(&q, &tel, &t, &token, &key)?;
            tr::write(Path::new(&s.out), &run, &src, &tel)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[twin-recal] {}\n[twin-recal] signed in {}",
                run.pack.verdict, s.out
            ))
        })(),
        _ => return fail(TWIN_RECAL_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
