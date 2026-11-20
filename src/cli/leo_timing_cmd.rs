// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro leo-timing <command>`: LEO timing verification (`crate::leo_timing`, research
//! backlog E21 B2102). `score` needs `pro-leo-timing-scorecard` (Pro); `hash`, `check` and
//! `synth` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::aviation::write_result;
use crate::leo_timing;

use super::evidence_cmd::{producing, sign};

pub const USAGE: &str = "usage: kshana-pro leo-timing <command>\n\
    hash <plan.toml>\n\
        Print the method hash of a plan (thresholds, claims, replay window) to register before taking logs (free)\n\
    score <plan.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Pre-registered LEO timing scorecard from field logs: per environment availability and time error, MTIE/TDEV, PRTC-A limit comparison, vendor claims, replay window; signed (Pro)\n\
    check <dir> --trust <signer public key hex>[,...]\n\
        Check a signed result directory, offline (free)\n\
    synth <dir>\n\
        Write synthetic logs and a plan (free)\n\
  Test evidence only: VALIDATED needs real logs and a matching pre-registered method hash.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn produce(args: &[String]) -> Result<String, String> {
    let Some(file) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return Err(USAGE.into());
    };
    let (out, seed, signer, utc) = producing(args, leo_timing::FEATURE)?;
    let l = leo_timing::load(Path::new(file))?;
    let s = leo_timing::score(&l)?;
    let summary = leo_timing::summary(&s);
    write_result(
        Path::new(&out),
        "scorecard.json",
        &s,
        &summary,
        &[],
        &l.inputs,
    )?;
    sign(&out, &seed, &signer, &utc)?;
    Ok(format!(
        "[leo-timing] {} ({}); signed in {out}",
        s.receiver, s.evidence_class
    ))
}

/// `kshana-pro leo-timing ...`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("hash") => {
            let Some(file) = args.get(1) else {
                return fail(USAGE);
            };
            let text = match std::fs::read_to_string(file) {
                Ok(t) => t,
                Err(e) => return fail(format!("{file}: {e}")),
            };
            match toml::from_str::<leo_timing::Plan>(&text) {
                Ok(p) => {
                    println!("{}", leo_timing::method_sha256(&p));
                    ExitCode::SUCCESS
                }
                Err(e) => fail(format!("{file}: {e}")),
            }
        }
        Some("check") => super::scorecard_cmd::verify_dir(args),
        Some("synth") => {
            let Some(dir) = args.get(1) else {
                return fail(USAGE);
            };
            match leo_timing::synth::write_all(Path::new(dir)) {
                Ok(()) => {
                    println!("wrote synthetic logs and plan.toml under {dir}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("score") => match produce(args) {
            Ok(msg) => {
                eprintln!("{msg}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        _ => fail(USAGE),
    }
}
