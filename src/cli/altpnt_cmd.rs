// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro altpnt <command>`: alternative-PNT evaluation and civil test-campaign evidence
//! (`crate::altpnt`, research backlog E40 B4004). Every result is a signed directory of test
//! evidence; nothing here computes a navigation solution.
//!
//! Licence: `failure-modes`, `labels` and `time-iface` need `pro-altpnt-evidence` (Pro),
//! `campaign` `pro-altpnt-campaign` (Pro). `check` and `synth` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::altpnt::{self, campaign, failure_modes, labels, time_iface, write_result};

use super::evidence_cmd::{producing, sign};
use super::forensics_cmd::keys;

pub const USAGE: &str = "usage: kshana-pro altpnt <command>\n\
    failure-modes <architecture.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Architecture failure-mode pack: common-cause resources, fallbacks the same failure takes out, single points of failure, undocumented sources, each mode's test evidence labelled by environment; cited signed evidence verified against --trust; signed (Pro)\n\
    labels <claims.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Digital-twin versus flight-test labels for stated claims: VALIDATED, MEASURED-OPERATIONAL, MEASURED-LAB, MODELLED (architectural target) or CLAIMED, and the model's gap to operation; signed (Pro)\n\
    campaign <campaign.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Civil test campaign: civil-releasability screen, then the system's outputs scored against the tester's truth per run and condition (availability, error percentiles, misleading and HMI epochs with intervals) and the stated claims checked; signed (Pro)\n\
    time-iface <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Resilient-time civil interface tests: locked accuracy, declared-quality honesty, holdover growth, recovery, continuity, MTIE against the declared mask; signed (Pro)\n\
    check <dir> --trust <signer public key hex>[,...]\n\
        Check a signed result directory, offline (free)\n\
    synth <dir>\n\
        Write synthetic inputs and request files for every command (free)\n\
  Evaluation and test evidence only: no navigation solution, sensor fusion, map matching, emitter location or antenna control. MODELLED.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn produce(args: &[String], gate: &str) -> Result<String, String> {
    let Some(file) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return Err(USAGE.into());
    };
    let (out, seed, signer, utc) = producing(args, gate)?;
    let path = Path::new(file);
    let dir = Path::new(&out);
    let headline = match args.first().map(String::as_str) {
        Some("failure-modes") => {
            let trust = keys(args, "--trust")?;
            let (req, inputs) = failure_modes::load(path)?;
            let p = failure_modes::analyse(&req, path.parent().unwrap_or(Path::new(".")), &trust)?;
            let h = format!(
                "{}: {} modes, {} findings, single points of failure: {}",
                p.system,
                p.modes.len(),
                p.findings.len(),
                if p.single_points_of_failure.is_empty() {
                    "none".to_string()
                } else {
                    p.single_points_of_failure.join(", ")
                }
            );
            write_result(
                dir,
                "failure-modes.json",
                &p,
                &altpnt::summary("failure-modes", &h),
                &[("failure-modes.md", failure_modes::markdown(&p))],
                &inputs,
            )?;
            h
        }
        Some("labels") => {
            let (req, inputs) = labels::load(path)?;
            let l = labels::label(&req)?;
            let h = l
                .counts
                .iter()
                .map(|(k, v)| format!("{v} {k}"))
                .collect::<Vec<_>>()
                .join(", ");
            write_result(
                dir,
                "labels.json",
                &l,
                &altpnt::summary("labels", &h),
                &[("labels.md", labels::markdown(&l))],
                &inputs,
            )?;
            h
        }
        Some("campaign") => {
            let l = campaign::load(path)?;
            let c = campaign::score(&l.req, &l.texts)?;
            let h = format!(
                "{} runs, {} conditions; claims: {}",
                c.runs.len(),
                c.conditions.len(),
                c.claims
                    .iter()
                    .map(|k| format!("{} {}", k.id, k.verdict))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            write_result(
                dir,
                "campaign.json",
                &c,
                &altpnt::summary("campaign", &h),
                &[("campaign.md", campaign::markdown(&c))],
                &l.inputs,
            )?;
            h
        }
        _ => {
            let l = time_iface::load(path)?;
            let (log, bad) = time_iface::parse_log(&l.log)?;
            let r = time_iface::test(&l.req, &log, bad)?;
            let h = r
                .tests
                .iter()
                .map(|t| format!("{} {}", t.id, t.verdict))
                .collect::<Vec<_>>()
                .join(", ");
            write_result(
                dir,
                "time-iface.json",
                &r,
                &altpnt::summary("time-iface", &h),
                &[("time-iface.md", time_iface::markdown(&r))],
                &l.inputs,
            )?;
            h
        }
    };
    sign(&out, &seed, &signer, &utc)?;
    Ok(format!("[altpnt] {headline}; signed in {out}"))
}

/// `kshana-pro altpnt ...`.
pub fn run(args: &[String]) -> ExitCode {
    let gate = match args.first().map(String::as_str) {
        Some("failure-modes" | "labels" | "time-iface") => altpnt::FEATURE_EVIDENCE,
        Some("campaign") => altpnt::FEATURE_CAMPAIGN,
        Some("check") => return super::scorecard_cmd::verify_dir(args),
        Some("synth") => {
            let Some(dir) = args.get(1) else {
                return fail(USAGE);
            };
            return match altpnt::synth::write_all(Path::new(dir)) {
                Ok(()) => {
                    println!("wrote synthetic architecture, claims, campaign runs and time log under {dir}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            };
        }
        _ => return fail(USAGE),
    };
    match produce(args, gate) {
        Ok(msg) => {
            eprintln!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
