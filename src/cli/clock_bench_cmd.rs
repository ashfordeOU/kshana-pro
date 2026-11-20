// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro timing clock-bench <register|score> <bench.toml>` (E33 B3302). The usage line
//! lives in [`super::timing_cmd::USAGE`]. `register` (the protocol hash) needs no licence;
//! `score` needs `pro-timing-clock-bench`; verifying a signed scorecard is
//! `kshana-pro dossier verify` (free).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::timing::clock_bench as cb;

use super::timing_cmd::{fail, gate, write_or_print};
use super::{flag, utc_iso8601_now};

fn load(path: &str) -> Result<(String, cb::BenchSpec), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let spec: cb::BenchSpec = toml::from_str(&src).map_err(|e| format!("{path}: {e}"))?;
    Ok((src, spec))
}

/// `timing clock-bench <register|score> <bench.toml> ...`.
pub fn clock_bench(args: &[String], sub: &str) -> ExitCode {
    let Some(path) = args.get(2).filter(|a| !a.starts_with("--")).cloned() else {
        return fail("usage: kshana-pro timing clock-bench <register|score> <bench.toml> [--json <out>] [--csv <out>] [--out <dir> --signer <name> --key-file <file>] [--utc <iso>] [--license <token>] [--dev]");
    };
    let r = (|| -> Result<(), String> {
        match sub {
            "register" => {
                // Only the protocol table is read: a protocol is registered before any clock exists.
                let src = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                #[derive(serde::Deserialize)]
                struct Only {
                    protocol: cb::Protocol,
                }
                let only: Only = toml::from_str(&src).map_err(|e| format!("{path}: {e}"))?;
                cb::validate_protocol(&only.protocol)?;
                println!(
                    "protocol_sha256 = \"{}\"",
                    cb::protocol_sha256(&only.protocol)
                );
                if let Some(o) = flag(args, "--json") {
                    std::fs::write(&o, cb::canonical_protocol(&only.protocol))
                        .map_err(|e| format!("{o}: {e}"))?;
                }
                Ok(())
            }
            "score" => {
                gate(args, cb::FEATURE)?;
                let (src, spec) = load(&path)?;
                let base = Path::new(&path)
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."));
                let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
                let r = cb::score(&spec, &base, &utc)?;
                write_or_print(
                    flag(args, "--json"),
                    &serde_json::to_string_pretty(&r).expect("serialises"),
                )?;
                if let Some(c) = flag(args, "--csv") {
                    std::fs::write(&c, cb::csv(&r)).map_err(|e| format!("{c}: {e}"))?;
                }
                if let Some(out) = flag(args, "--out") {
                    let (Some(signer), Some(kf)) =
                        (flag(args, "--signer"), flag(args, "--key-file"))
                    else {
                        return Err("--out needs --signer and --key-file".into());
                    };
                    let seed = super::forensics_cmd::read_seed(&kf)?;
                    let dir = Path::new(&out);
                    cb::write_dir(dir, &r, &src)?;
                    crate::signed_dossier::sign(dir, &seed, &signer, &utc)
                        .map_err(|e| e.to_string())?;
                    eprintln!("signed in {out}");
                }
                Ok(())
            }
            other => Err(format!(
                "unknown clock-bench command {other} (register or score)"
            )),
        }
    })();
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}
