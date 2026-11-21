// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro attack-iq`: synthetic attack IQ files with guardrails (roadmap 0.42 F7;
//! docs/ATTACK-IQ.md). The licence comes from `--license` only (never the stored licence
//! and never `--dev`), so each generation names the token it runs under.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::attack_iq::{self, Attestation, Request, Waveform};
use crate::license::{self, Product};
use kshana::api::KshanaError;

use super::{flag, now_unix};

const USAGE: &str = "usage: kshana-pro attack-iq <command>\n\
  generate --license <token> --attestation <file.json> --out <new file.ci16>\n\
           --waveform <cw|chirp|noise|pulsed|replay> --sample-rate <hz> --duration <s> [--inr-db <dB>] [--seed <n>]\n\
           [--offset-hz <hz>] [--sweep-hz <hz>] [--period-s <s>] [--duty <0..1>] [--bandwidth-hz <hz>]\n\
           [--chip-rate-hz <hz>] [--delay-s <s>] [--gain-db <dB>] [--log-dir <dir>]\n\
        Writes the file and <file>.attack-iq.json (metadata, watermark, attestation hash) and logs\n\
        the generation. Needs a licence granting pro-attack-iq and marked vetted_buyer, and an\n\
        attestation valid today. Files only: never a device, pipe or radio.\n\
  attestation-template --licensee <name>      Print an attestation to fill in and sign off\n\
  trace --file <file.ci16> --candidates <file.json>\n\
        Vendor side: which licence marked a file; candidates is [[licensee, token], ...] from the register\n";

fn num(args: &[String], name: &str, default: Option<f64>) -> Result<f64, KshanaError> {
    match flag(args, name) {
        Some(v) => v
            .parse::<f64>()
            .ok()
            .filter(|x| x.is_finite())
            .ok_or_else(|| KshanaError::InvalidInput(format!("{name} needs a finite number"))),
        None => default.ok_or_else(|| KshanaError::InvalidInput(format!("needs {name}"))),
    }
}

fn need(args: &[String], name: &str) -> Result<String, KshanaError> {
    flag(args, name).ok_or_else(|| KshanaError::InvalidInput(format!("needs {name}")))
}

fn generate(args: &[String]) -> Result<(), KshanaError> {
    let now = now_unix();
    let token = need(args, "--license")?;
    let lic = super::license_cmd::verify_licence_for(&token, now, Product::Cli)?;
    let ent_allows = license::check(&lic.entitlement, attack_iq::FEATURE, now).is_ok();
    let att_path = need(args, "--attestation")?;
    let att: Attestation = serde_json::from_str(
        &std::fs::read_to_string(&att_path)
            .map_err(|e| KshanaError::InvalidInput(format!("{att_path}: {e}")))?,
    )
    .map_err(|e| KshanaError::InvalidInput(format!("{att_path}: {e}")))?;
    let waveform = match need(args, "--waveform")?.as_str() {
        "cw" => Waveform::Cw {
            offset_hz: num(args, "--offset-hz", Some(0.0))?,
        },
        "chirp" => Waveform::Chirp {
            sweep_hz: num(args, "--sweep-hz", None)?,
            period_s: num(args, "--period-s", None)?,
        },
        "noise" => Waveform::Noise {
            bandwidth_hz: num(args, "--bandwidth-hz", None)?,
        },
        "pulsed" => Waveform::Pulsed {
            offset_hz: num(args, "--offset-hz", Some(0.0))?,
            period_s: num(args, "--period-s", None)?,
            duty: num(args, "--duty", None)?,
        },
        "replay" => Waveform::Replay {
            chip_rate_hz: num(args, "--chip-rate-hz", None)?,
            delay_s: num(args, "--delay-s", None)?,
            gain_db: num(args, "--gain-db", Some(3.0))?,
        },
        other => {
            return Err(KshanaError::InvalidInput(format!(
                "unknown waveform '{other}' (cw, chirp, noise, pulsed, replay)"
            )))
        }
    };
    let req = Request {
        waveform,
        sample_rate_hz: num(args, "--sample-rate", None)?,
        duration_s: num(args, "--duration", None)?,
        inr_db: num(args, "--inr-db", Some(20.0))?,
        seed: flag(args, "--seed")
            .map(|s| s.parse::<u64>())
            .transpose()
            .map_err(|_| KshanaError::InvalidInput("--seed needs a whole number".into()))?
            .unwrap_or(1),
    };
    let out = PathBuf::from(need(args, "--out")?);
    let log_dir = match flag(args, "--log-dir") {
        Some(d) => PathBuf::from(d),
        None => default_log_dir(),
    };
    let meta = attack_iq::generate(&lic, &token, ent_allows, &att, &req, &out, &log_dir, now)?;
    eprintln!("{}", attack_iq::NOTICE);
    println!(
        "wrote {} ({} samples, sha256 {}) and {}{}; logged in {}",
        out.display(),
        meta.samples,
        meta.data_sha256,
        out.display(),
        attack_iq::META_SUFFIX,
        attack_iq::log_path(&log_dir).display()
    );
    Ok(())
}

fn default_log_dir() -> PathBuf {
    if let Ok(h) = std::env::var("KSHANA_PRO_HOME") {
        return Path::new(&h).join("attack-iq");
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    Path::new(&home).join(".kshana-pro").join("attack-iq")
}

fn trace(args: &[String]) -> Result<(), KshanaError> {
    let samples = attack_iq::read_ci16(Path::new(&need(args, "--file")?))?;
    let c = need(args, "--candidates")?;
    let cands: Vec<(String, String)> = serde_json::from_str(
        &std::fs::read_to_string(&c).map_err(|e| KshanaError::InvalidInput(format!("{c}: {e}")))?,
    )
    .map_err(|e| KshanaError::InvalidInput(format!("{c}: {e}")))?;
    match attack_iq::trace(&samples, &cands) {
        Some(who) => {
            println!("marked by the licence of: {who}");
            Ok(())
        }
        None => Err(KshanaError::InvalidInput(
            "no candidate's mark found (not generated here, or the mark was removed)".into(),
        )),
    }
}

pub fn run(args: &[String]) -> ExitCode {
    let r = match args.first().map(String::as_str) {
        Some("generate") => generate(&args[1..]),
        Some("attestation-template") => match flag(&args[1..], "--licensee") {
            Some(l) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Attestation::template(&l)).unwrap_or_default()
                );
                Ok(())
            }
            None => Err(KshanaError::InvalidInput("needs --licensee".into())),
        },
        Some("trace") => trace(&args[1..]),
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
