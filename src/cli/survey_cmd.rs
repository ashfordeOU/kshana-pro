// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro survey-qc <run|synth>` (research 2026-11-20 gaps, B0807; docs/SURVEY-QC.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::survey_qc;

use super::grid_cmd::{gate, signing};

pub const SURVEY_USAGE: &str = "usage: kshana-pro survey-qc <command>\n\
    run <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Per-line QC flags where interference degraded positioning, signed QC annex and site risk plan (Pro)\n\
    synth <dir>                                                      Write a SYNTHETIC survey with injected interference and a request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro survey-qc ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(SURVEY_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            let d = Path::new(&arg);
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            let (csv, _) = survey_qc::synth();
            std::fs::write(d.join("survey.csv"), csv).map_err(|e| e.to_string())?;
            std::fs::write(
                d.join("request.toml"),
                "title = \"SYNTHETIC survey QC\"\nlog = \"survey.csv\"\nhorizontal_tolerance_m = 2.0\ntolerance_source = \"SYNTHETIC survey specification\"\nrequired_fix = 4\n",
            )
            .map_err(|e| e.to_string())?;
            Ok(format!("[survey-qc] SYNTHETIC survey and request in {arg}"))
        })(),
        Some("run") => (|| {
            gate(args, survey_qc::FEATURE)?;
            let s = signing(args, "survey-qc run")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: survey_qc::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let a = survey_qc::run(&q, &base)?;
            crate::transport::write_pack(
                Path::new(&s.out),
                &a,
                &survey_qc::html(&a),
                "request.toml",
                &src,
            )?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[survey-qc] {}\n[survey-qc] signed QC annex in {}",
                a.verdict, s.out
            ))
        })(),
        _ => return fail(SURVEY_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
