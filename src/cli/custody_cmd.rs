// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro custody <capture|handover|verify|export>` (research 2026-11-20 gaps, B0604;
//! docs/CUSTODY.md). The capture needs its Pro feature; recording a handover, verifying and
//! exporting are free, so every party in the chain can take part.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::custody;

use super::grid_cmd::gate;
use super::{flag, utc_iso8601_now};

pub const CUSTODY_USAGE: &str = "usage: kshana-pro custody <command>\n\
    capture <request.toml> --out <dir> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Freeze a ring buffer around a trigger in a SigMF recording; sealed SigMF capture with calibration ID, custody form bound to its hash (Pro)\n\
    handover <dir> --from <name> --to <name> --purpose <text> --key-file <file> [--utc <iso>]   Append a signed, hash-chained handover (no licence needed)\n\
    verify <dir> [--trust <public-key-hex>]                         Verify data, seal, form and handover chain; any changed byte fails (no licence needed)\n\
    export <dir> [--json <out>] [--trust <public-key-hex>]          The custody chain as one JSON document (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn trust(args: &[String]) -> Result<Vec<[u8; crate::license::KEY_LEN]>, String> {
    match flag(args, "--trust") {
        Some(t) => Ok(vec![
            crate::license::parse_key_hex(&t).map_err(|e| e.to_string())?
        ]),
        None => Ok(Vec::new()),
    }
}

/// `kshana-pro custody ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(CUSTODY_USAGE);
    };
    let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
    let r: Result<String, String> = match sub {
        Some("capture") => (|| {
            gate(args, custody::FEATURE)?;
            let (Some(out), Some(kf)) = (flag(args, "--out"), flag(args, "--key-file")) else {
                return Err("capture needs --out and --key-file".into());
            };
            let seed = super::forensics_cmd::read_seed(&kf)?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: custody::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let c = custody::capture(&q, &base, Path::new(&out), &seed, &utc)?;
            Ok(format!(
                "[custody] {} samples ({} before the trigger) sealed as {} in {out}",
                c.samples, c.pre_samples, c.capture_id
            ))
        })(),
        Some("handover") => (|| {
            let (Some(from), Some(to), Some(purpose), Some(kf)) = (
                flag(args, "--from"),
                flag(args, "--to"),
                flag(args, "--purpose"),
                flag(args, "--key-file"),
            ) else {
                return Err("handover needs --from, --to, --purpose and --key-file".into());
            };
            let seed = super::forensics_cmd::read_seed(&kf)?;
            let h = custody::handover(Path::new(&arg), &from, &to, &purpose, &seed, &utc)?;
            Ok(format!(
                "[custody] handover {} recorded: {} to {}",
                h.seq, h.from, h.to
            ))
        })(),
        Some("verify") => (|| {
            let v = custody::verify(Path::new(&arg), &trust(args)?)?;
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            if v.passed() {
                Ok(format!("[custody] verified: {} handover(s)", v.handovers))
            } else {
                Err(format!("does not verify: {}", v.problems.join("; ")))
            }
        })(),
        Some("export") => (|| {
            let c = custody::export(Path::new(&arg), &trust(args)?)?;
            let text = serde_json::to_string_pretty(&c).map_err(|e| e.to_string())? + "\n";
            match flag(args, "--json") {
                Some(o) => std::fs::write(&o, text).map_err(|e| format!("{o}: {e}"))?,
                None => print!("{text}"),
            }
            Ok(format!(
                "[custody] chain exported ({})",
                if c.verification.passed() {
                    "verifies"
                } else {
                    "DOES NOT VERIFY"
                }
            ))
        })(),
        _ => return fail(CUSTODY_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
