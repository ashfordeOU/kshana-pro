// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Incident reconstruction and dispute evidence (research 2026-11-20 gaps E06), hung under
//! `kshana-pro forensics`:
//!
//! * `forensics log-time <request.toml> [--json] [--out <dir>] ...` and `log-time-synth`
//!   (B0603, feature `pro-log-time-integrity`);
//! * `forensics dispute <request.toml> --out <dir> --signer --key-file ...`,
//!   `dispute templates` and `dispute-synth` (B0606, feature `pro-dispute-evidence`).
//!
//! Producing a result needs the licence feature; the synthetic cases and the template list are
//! free, and a signed directory verifies free with `kshana-pro dossier verify`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::forensics::{dispute, logtime};
use crate::license;

use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix, utc_iso8601_now};

pub const LOG_TIME_USAGE: &str = "usage: kshana-pro forensics log-time <request.toml> [--json] [--timeline <file.csv>] [--out <dir> [--signer <name> --key-file <file>] [--utc <iso>]] [--license <token>] [--dev]\n\
        Time integrity of operational logs (VDR/ECDIS NMEA ZDA/RMC, ACARS/CPDLC JSON lines, the CSV/JSON schema): steps, backward time, frozen clocks, drift, GPS week rollover, leap seconds, ordering, cross-log disagreement; corrected timeline (Pro). Exit 3 when a fault is found\n\
    kshana-pro forensics log-time-synth <dir>\n\
        Write a synthetic case (VDR with a week rollover, CPDLC and a console clock running backwards) to try it (free)";

pub const DISPUTE_USAGE: &str = "usage: kshana-pro forensics dispute <request.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--json] [--utc <iso>] [--license <token>] [--dev]\n\
        Commercial dispute evidence (off-hire, delay/laycan, DP downtime, deviation, safe port): when GNSS position or timing integrity was lost, for how long, which sources agree, mapped to clause questions with pack IDs cited; MODELLED, not a legal finding; signed (Pro)\n\
    kshana-pro forensics dispute templates\n\
        List the clause templates (free)\n\
    kshana-pro forensics dispute-synth <dir>\n\
        Write a synthetic DP-charter case to try it (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

fn file_arg(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

fn base_of(path: &str) -> PathBuf {
    Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `forensics log-time`.
pub fn log_time(args: &[String]) -> ExitCode {
    let Some(path) = file_arg(args) else {
        return fail(LOG_TIME_USAGE);
    };
    if let Err(e) = gate(args, logtime::FEATURE) {
        return fail(e);
    }
    let res: Result<logtime::Report, String> = (|| {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let req = logtime::Request::parse(&text)?;
        let base = base_of(path);
        let r = logtime::check(&req, &base)?;
        if let Some(t) = flag(args, "--timeline") {
            std::fs::write(&t, logtime::timeline_csv(&r)).map_err(|e| format!("{t}: {e}"))?;
        }
        if let Some(out) = flag(args, "--out") {
            let files = req
                .logs
                .iter()
                .map(|l| {
                    let p = if Path::new(&l.path).is_relative() {
                        base.join(&l.path)
                    } else {
                        PathBuf::from(&l.path)
                    };
                    std::fs::read(&p)
                        .map(|b| {
                            (
                                p.file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                                b,
                            )
                        })
                        .map_err(|e| format!("{}: {e}", p.display()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            logtime::write_dir(Path::new(&out), &r, &text, &files)?;
            match (flag(args, "--signer"), flag(args, "--key-file")) {
                (Some(signer), Some(kf)) => {
                    let seed = read_seed(&kf)?;
                    let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
                    crate::signed_dossier::sign(Path::new(&out), &seed, &signer, &utc)
                        .map_err(|e| e.to_string())?;
                    eprintln!("[log-time] signed report in {out}");
                }
                (None, None) => eprintln!("[log-time] report in {out} (unsigned)"),
                _ => return Err("signing needs both --signer and --key-file".into()),
            }
        }
        Ok(r)
    })();
    match res {
        Ok(r) => {
            if has_flag(args, "--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&r).expect("report serialises")
                );
            } else {
                print!("{}", logtime::render_text(&r));
            }
            if r.verdict.starts_with("TIME INTEGRITY FAULTS") {
                ExitCode::from(3)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => fail(e),
    }
}

/// `forensics log-time-synth <dir>`.
pub fn log_time_synth(args: &[String]) -> ExitCode {
    let Some(dir) = file_arg(args) else {
        return fail(LOG_TIME_USAGE);
    };
    match logtime::synth::write_demo(Path::new(dir)) {
        Ok(()) => {
            eprintln!("[log-time] synthetic case in {dir} (request.toml)");
            ExitCode::SUCCESS
        }
        Err(e) => fail(format!("{dir}: {e}")),
    }
}

/// `forensics dispute`.
pub fn dispute(args: &[String]) -> ExitCode {
    let Some(path) = file_arg(args) else {
        return fail(DISPUTE_USAGE);
    };
    if path == "templates" {
        for (id, title, what) in dispute::TEMPLATES {
            println!("{id:<13} {title}\n{:<13} {what}", "");
        }
        return ExitCode::SUCCESS;
    }
    if let Err(e) = gate(args, dispute::FEATURE) {
        return fail(e);
    }
    let (Some(out), Some(signer), Some(kf)) = (
        flag(args, "--out"),
        flag(args, "--signer"),
        flag(args, "--key-file"),
    ) else {
        return fail("dispute needs --out, --signer and --key-file");
    };
    let res: Result<dispute::Report, String> = (|| {
        let seed = read_seed(&kf)?;
        let trust = keys(args, "--trust")?;
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let req = dispute::Request::parse(&text)?;
        let (r, asm) = dispute::assess(&req, &base_of(path), &trust)?;
        let dir = Path::new(&out);
        dispute::write_dir(dir, &r, &asm, &text)?;
        let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
        crate::signed_dossier::sign(dir, &seed, &signer, &utc).map_err(|e| e.to_string())?;
        Ok(r)
    })();
    match res {
        Ok(r) => {
            if has_flag(args, "--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&r).expect("report serialises")
                );
            } else {
                print!("{}", dispute::render_text(&r));
            }
            eprintln!(
                "[dispute] {} period(s), {:.0} s of lost integrity; signed evidence in {out}",
                r.periods.len(),
                r.integrity_lost_s
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `forensics dispute-synth <dir>`.
pub fn dispute_synth(args: &[String]) -> ExitCode {
    let Some(dir) = file_arg(args) else {
        return fail(DISPUTE_USAGE);
    };
    match dispute::synth::write_demo(Path::new(dir)) {
        Ok(()) => {
            eprintln!("[dispute] synthetic case in {dir} (request.toml)");
            ExitCode::SUCCESS
        }
        Err(e) => fail(format!("{dir}: {e}")),
    }
}
